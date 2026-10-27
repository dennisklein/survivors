//! The world: map, train, enemies and the fixed-step simulation.

use crate::cards::{self, Card, MAX_WAGONS, REPAIR_HP, Upgrade, Upgrades};
use crate::director::{self, Director};
use crate::enemy::{self, BOSS_CALL_INTERVAL, Enemy, EnemyKind, UFO_FIRE_INTERVAL};
use crate::grid::Grid;
use crate::math::V2;
use crate::rng::Pcg32;
use crate::track::{Dir, TileMap};
use crate::train::{Train, Turn};
use crate::weapon::{self, BLAST_TTL, Blast, Fires, STEAM_PUSH, Shot, Wagon, WagonKind};

/// One simulation step: 60 steps per second.
pub const DT: f32 = 1.0 / 60.0;
pub const MAP_SIZE: i32 = 64;
pub const START_HP: f32 = 100.0;
pub const SELF_HIT_DAMAGE: f32 = 20.0;
pub const CAR_RADIUS: f32 = 0.4;
/// Ram damage is this factor times the train speed.
pub const RAM_FACTOR: f32 = 10.0;
pub const RAM_COOLDOWN: f32 = 0.5;
/// After touch damage, the train cannot take touch damage for this time.
pub const TOUCH_IFRAMES: f32 = 1.0;
pub const SMALL_UFO_TIMES: [f32; 2] = [300.0, 480.0];
pub const BOSS_TIME: f32 = 600.0;
pub const PLASMA_SPEED: f32 = 3.0;
pub const PLASMA_DAMAGE: f32 = 15.0;
pub const PLASMA_RADIUS: f32 = 0.25;
pub const BOSS_CALL_SIZE: u32 = 8;
pub const CRATE_PULL_SPEED: f32 = 10.0;
pub const MAX_CRATES: usize = 400;

/// Player input for one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Turn key held now.
    pub turn: Option<Turn>,
    /// Reverse key pressed since the last step.
    pub reverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Running,
    /// Paused: the player picks one of three cards.
    LevelUp([Card; 3]),
    Lost,
    Won,
}

/// A slow plasma ball that a UFO fires at the locomotive.
#[derive(Clone, Copy, Debug)]
pub struct Plasma {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
}

/// Cargo that a dead enemy drops. Holds the XP value of the enemy.
#[derive(Clone, Copy, Debug)]
pub struct Crate {
    pub pos: V2,
    pub xp: u32,
    pub pulled: bool,
}

pub struct World {
    pub map: TileMap,
    pub train: Train,
    pub enemies: Vec<Enemy>,
    pub wagons: Vec<Wagon>,
    pub shots: Vec<Shot>,
    pub plasma: Vec<Plasma>,
    pub blasts: Vec<Blast>,
    pub fires: Fires,
    pub crates: Vec<Crate>,
    pub upgrades: Upgrades,
    pub level: u32,
    pub xp: u32,
    /// Level-ups that still wait for a card.
    pending: u32,
    pub rng: Pcg32,
    pub time: f32,
    pub state: State,
    pub hp: f32,
    pub max_hp: f32,
    pub kills: u32,
    iframes: f32,
    /// Index of the next scheduled small UFO, or 2 when both came.
    ufos_sent: usize,
    pub boss_sent: bool,
    director: Director,
    grid: Grid,
    next_id: u32,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let mut map = TileMap::generate(MAP_SIZE, MAP_SIZE, 9.0, &mut rng);
        let c = MAP_SIZE / 2;
        let train = Train::new(&mut map, (c, c), Dir::E, 2);
        Self {
            map,
            train,
            enemies: Vec::new(),
            wagons: vec![Wagon::new(WagonKind::Armoured)],
            shots: Vec::new(),
            plasma: Vec::new(),
            blasts: Vec::new(),
            fires: Fires::new(MAP_SIZE, MAP_SIZE),
            crates: Vec::new(),
            upgrades: Upgrades::default(),
            level: 1,
            xp: 0,
            pending: 0,
            rng,
            time: 0.0,
            state: State::Running,
            hp: START_HP,
            max_hp: START_HP,
            kills: 0,
            iframes: 0.0,
            ufos_sent: 0,
            boss_sent: false,
            director: Director::default(),
            grid: Grid::new(MAP_SIZE, MAP_SIZE),
            next_id: 0,
        }
    }

    pub fn head_pos(&self) -> V2 {
        self.train.head().pos
    }

    pub fn step(&mut self, input: &Input) {
        if self.state != State::Running {
            return;
        }
        self.time += DT;
        // 1. Input.
        if let Some(turn) = input.turn {
            self.train.latch(turn);
        }
        if input.reverse && !self.train.is_stopped() {
            self.train.reverse();
        }
        // 2. Train.
        if self.train.step(&mut self.map, DT) {
            self.damage(SELF_HIT_DAMAGE);
        }
        // 3. Spawns.
        self.spawn();
        self.schedule_ufos();
        // 4. Enemies.
        self.move_enemies();
        self.ufo_actions();
        // 5. Weapons.
        self.fire_weapons();
        self.move_shots();
        self.move_plasma();
        self.burn();
        // 6. Hits.
        self.ram_and_touch();
        self.remove_dead();
        // 7. Cargo and XP.
        self.collect_crates();
    }

    /// XP needed for the next level.
    pub fn xp_needed(&self) -> u32 {
        cards::xp_needed(self.level)
    }

    /// Applies card `i` of the level-up choice and resumes the run.
    pub fn pick_card(&mut self, i: usize) {
        let State::LevelUp(cards) = self.state else {
            return;
        };
        let Some(&card) = cards.get(i) else {
            return;
        };
        match card {
            Card::NewWagon(k) => self.add_wagon(k),
            Card::LevelWagon(slot) => self.wagons[slot].level += 1,
            Card::Upgrade(u) => {
                self.upgrades.0[u as usize] += 1;
                if u == Upgrade::Armour {
                    self.max_hp += 20.0;
                    self.hp = (self.hp + 20.0).min(self.max_hp);
                }
                self.train.speed_bonus = self.upgrades.speed_bonus();
            }
            Card::Repair => self.hp = (self.hp + REPAIR_HP).min(self.max_hp),
        }
        self.pending -= 1;
        self.state = State::Running;
        self.offer_cards();
    }

    fn offer_cards(&mut self) {
        if self.pending > 0 && self.state == State::Running {
            self.state = State::LevelUp(cards::roll(&self.wagons, &self.upgrades, &mut self.rng));
        }
    }

    fn collect_crates(&mut self) {
        let radius = self.upgrades.pickup_radius();
        let cars: Vec<V2> = self.train.cars().iter().map(|c| c.pos).collect();
        let mut gained = 0;
        self.crates.retain_mut(|c| {
            let near = cars
                .iter()
                .copied()
                .min_by(|a, b| a.dist_sq(c.pos).total_cmp(&b.dist_sq(c.pos)))
                .unwrap_or(c.pos);
            let d = near.dist(c.pos);
            c.pulled |= d < radius;
            if !c.pulled {
                return true;
            }
            if d < 0.35 {
                gained += c.xp;
                return false;
            }
            c.pos += (near - c.pos).norm() * (CRATE_PULL_SPEED * DT).min(d);
            true
        });
        self.xp += gained;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.pending += 1;
        }
        self.offer_cards();
    }

    fn drop_crate(&mut self, pos: V2, xp: u32) {
        if self.crates.len() >= MAX_CRATES {
            // Too much cargo on the map: add the XP to an old crate.
            let i = self.rng.below(self.crates.len() as u32) as usize;
            self.crates[i].xp += xp;
        } else {
            self.crates.push(Crate {
                pos,
                xp,
                pulled: false,
            });
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.hp -= amount;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.state = State::Lost;
        }
    }

    pub fn spawn_enemy(&mut self, kind: enemy::EnemyKind, at: V2) {
        let e = Enemy::new(self.next_id, kind, at, director::hp_scale(self.time));
        self.next_id += 1;
        self.enemies.push(e);
    }

    /// Adds a wagon at the end of the train.
    pub fn add_wagon(&mut self, kind: WagonKind) {
        if self.wagons.len() < MAX_WAGONS {
            self.wagons.push(Wagon::new(kind));
            self.train.set_car_count(1 + self.wagons.len());
        }
    }

    fn spawn(&mut self) {
        let size = self.map.width() as f32;
        let centre = self.head_pos();
        let live = self.enemies.len();
        let Some(g) = self
            .director
            .tick(DT, self.time, live, centre, size, &mut self.rng)
        else {
            return;
        };
        for _ in 0..g.count {
            let jitter = V2::new(self.rng.range(-1.2, 1.2), self.rng.range(-1.2, 1.2));
            self.spawn_enemy(g.kind, g.at + jitter);
        }
    }

    pub fn boss(&self) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.kind == EnemyKind::LargeUfo)
    }

    fn schedule_ufos(&mut self) {
        let size = self.map.width() as f32;
        if let Some(&at) = SMALL_UFO_TIMES.get(self.ufos_sent)
            && self.time >= at
        {
            self.ufos_sent += 1;
            let p = director::ring_point(self.head_pos(), size, &mut self.rng);
            self.spawn_enemy(EnemyKind::SmallUfo, p);
        }
        if !self.boss_sent && self.time >= BOSS_TIME {
            self.boss_sent = true;
            let p = director::ring_point(self.head_pos(), size, &mut self.rng);
            let e = Enemy::new(self.next_id, EnemyKind::LargeUfo, p, 1.0);
            self.next_id += 1;
            self.enemies.push(e);
        }
    }

    /// Small UFOs fire plasma. The boss removes track and calls residents.
    fn ufo_actions(&mut self) {
        let loco = self.train.slot(0).pos;
        let mut calls = Vec::new();
        for e in &mut self.enemies {
            if e.timer > 0.0 {
                continue;
            }
            match e.kind {
                EnemyKind::SmallUfo => {
                    e.timer = UFO_FIRE_INTERVAL;
                    self.plasma.push(Plasma {
                        pos: e.pos,
                        vel: (loco - e.pos).norm() * PLASMA_SPEED,
                        life: 6.0,
                    });
                }
                EnemyKind::LargeUfo => {
                    e.timer = BOSS_CALL_INTERVAL;
                    calls.push(e.pos);
                }
                _ => {}
            }
        }
        for at in calls {
            for _ in 0..BOSS_CALL_SIZE {
                let off = V2::from_angle(self.rng.range(0.0, std::f32::consts::TAU))
                    * self.rng.range(1.5, 3.0);
                self.spawn_enemy(EnemyKind::Resident, at + off);
            }
        }
        if let Some(b) = self.boss() {
            let (p, r) = (b.pos, b.radius());
            for y in (p.y - r).floor() as i32..=(p.y + r).floor() as i32 {
                for x in (p.x - r).floor() as i32..=(p.x + r).floor() as i32 {
                    self.map.clear_track((x, y));
                }
            }
        }
    }

    fn move_plasma(&mut self) {
        let mut hurt = 0.0;
        let cars = self.train.cars();
        self.plasma.retain_mut(|b| {
            b.pos += b.vel * DT;
            b.life -= DT;
            let reach = PLASMA_RADIUS + CAR_RADIUS;
            if cars.iter().any(|c| c.pos.dist_sq(b.pos) < reach * reach) {
                hurt += PLASMA_DAMAGE;
                return false;
            }
            b.life > 0.0
        });
        if hurt > 0.0 {
            self.damage(hurt);
        }
    }

    fn move_enemies(&mut self) {
        let cars: Vec<V2> = self.train.cars().iter().map(|c| c.pos).collect();
        let loco = self.train.slot(0).pos;
        let size = self.map.width() as f32;
        for e in &mut self.enemies {
            if e.kind.flies() {
                e.think(loco, DT);
                continue;
            }
            let target = cars
                .iter()
                .copied()
                .min_by(|a, b| a.dist_sq(e.pos).total_cmp(&b.dist_sq(e.pos)))
                .unwrap_or(e.pos);
            e.think(target, DT);
            e.pos = V2::new(e.pos.x.clamp(0.0, size), e.pos.y.clamp(0.0, size));
        }
        self.rebuild_grid();
        enemy::separate(&mut self.enemies, &self.grid);
    }

    fn rebuild_grid(&mut self) {
        let pts: Vec<V2> = self.enemies.iter().map(|e| e.pos).collect();
        self.grid.build(&pts);
    }

    /// The head rams enemies. Enemies that touch any car hurt the train.
    fn ram_and_touch(&mut self) {
        let speed = self.train.speed();
        // The strongest touching enemy hurts the train, then a short grace time.
        let mut hurt: f32 = 0.0;
        self.iframes -= DT;
        for (ci, car) in self.train.cars().iter().enumerate() {
            let enemies = &mut self.enemies;
            self.grid.query(car.pos, CAR_RADIUS + 1.5, |i| {
                let e = &mut enemies[i];
                let reach = CAR_RADIUS + e.radius();
                let d = e.pos - car.pos;
                if d.len_sq() >= reach * reach {
                    return;
                }
                let away = d.norm();
                if ci == 0 && speed > 0.5 && e.ram_cd <= 0.0 {
                    e.hit(RAM_FACTOR * speed);
                    e.ram_cd = RAM_COOLDOWN;
                    e.push = away * 6.0;
                }
                if e.hp > 0.0 {
                    hurt = hurt.max(e.stats().touch);
                }
                // Enemies cannot stand on the train. UFOs fly over it.
                if !e.kind.flies() {
                    e.pos = car.pos + away * reach;
                }
            });
        }
        if hurt > 0.0 && self.iframes <= 0.0 {
            self.damage(hurt);
            self.iframes = TOUCH_IFRAMES;
        }
    }

    fn fire_weapons(&mut self) {
        let factor = self.upgrades.cooldown_factor();
        for slot in 0..self.wagons.len() {
            let car = *self.train.slot(slot + 1);
            let w = &mut self.wagons[slot];
            w.cooldown -= DT;
            match w.kind {
                WagonKind::Armoured => {
                    if w.cooldown > 0.0 {
                        continue;
                    }
                    let stats = weapon::armoured(w.level);
                    let targets = nearest(
                        &self.enemies,
                        car.pos,
                        weapon::ARMOURED_RANGE,
                        stats.bullets,
                    );
                    if targets.is_empty() {
                        continue;
                    }
                    for t in targets {
                        let aim = (self.enemies[t].pos - car.pos).norm();
                        w.aim = aim;
                        self.shots.push(Shot {
                            pos: car.pos,
                            vel: aim * weapon::SHOT_SPEED,
                            damage: stats.damage,
                            pierce: stats.pierce,
                            life: weapon::ARMOURED_RANGE * 1.4 / weapon::SHOT_SPEED,
                            hit: Vec::new(),
                        });
                    }
                    w.cooldown = factor / stats.rate;
                }
                WagonKind::Hopper => {
                    let tile = (car.pos.x.floor() as i32, car.pos.y.floor() as i32);
                    if w.last_tile == Some(tile) {
                        continue;
                    }
                    w.last_tile = Some(tile);
                    let h = weapon::hopper(w.level);
                    self.fires.light(tile, h.duration, h.dps);
                    if h.adjacent {
                        for d in Dir::ALL {
                            let (dx, dy) = d.delta();
                            self.fires
                                .light((tile.0 + dx, tile.1 + dy), h.duration * 0.5, h.dps);
                        }
                    }
                }
                WagonKind::Steam => {
                    if w.cooldown > 0.0 {
                        continue;
                    }
                    let s = weapon::steam(w.level);
                    let r2 = s.radius * s.radius;
                    if !self.enemies.iter().any(|e| e.pos.dist_sq(car.pos) < r2) {
                        continue;
                    }
                    for e in &mut self.enemies {
                        if e.pos.dist_sq(car.pos) < r2 {
                            e.hit(s.damage);
                            e.push = (e.pos - car.pos).norm() * STEAM_PUSH;
                        }
                    }
                    self.blasts.push(Blast {
                        pos: car.pos,
                        radius: s.radius,
                        ttl: BLAST_TTL,
                    });
                    w.cooldown = s.cooldown * factor;
                }
            }
        }
        for b in &mut self.blasts {
            b.ttl -= DT;
        }
        self.blasts.retain(|b| b.ttl > 0.0);
    }

    fn move_shots(&mut self) {
        let enemies = &mut self.enemies;
        let grid = &self.grid;
        self.shots.retain_mut(|s| {
            s.pos += s.vel * DT;
            s.life -= DT;
            grid.query(s.pos, 1.0, |i| {
                let e = &mut enemies[i];
                let reach = e.radius() + weapon::SHOT_RADIUS;
                if s.pierce == 0
                    || e.hp <= 0.0
                    || s.hit.contains(&e.id)
                    || e.pos.dist_sq(s.pos) > reach * reach
                {
                    return;
                }
                e.hit(s.damage);
                s.hit.push(e.id);
                s.pierce -= 1;
            });
            s.life > 0.0 && s.pierce > 0
        });
    }

    fn burn(&mut self) {
        self.fires.tick(DT);
        for e in &mut self.enemies {
            let dps = self.fires.dps_at(e.pos);
            if dps > 0.0 {
                e.hp -= dps * DT;
            }
        }
    }

    fn remove_dead(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                self.kills += 1;
                if e.kind == EnemyKind::LargeUfo {
                    self.state = State::Won;
                } else {
                    self.drop_crate(e.pos, e.stats().xp);
                }
            } else {
                i += 1;
            }
        }
    }
}

/// Indices of up to `n` enemies nearest to `p` within `range`.
fn nearest(enemies: &[Enemy], p: V2, range: f32, n: usize) -> Vec<usize> {
    let mut found: Vec<(f32, usize)> = enemies
        .iter()
        .enumerate()
        .map(|(i, e)| (e.pos.dist_sq(p), i))
        .filter(|&(d, _)| d < range * range)
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().take(n).map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(w: &mut World, steps: usize, turn: impl Fn(usize) -> Option<Turn>) {
        for i in 0..steps {
            w.step(&Input {
                turn: turn(i),
                reverse: i % 700 == 699,
            });
        }
    }

    #[test]
    fn self_hit_costs_hp_but_not_the_run() {
        let mut w = World::new(1);
        for _ in 0..3 {
            w.add_wagon(WagonKind::Steam);
        }
        for _ in 0..600 {
            w.step(&Input {
                turn: Some(Turn::Right),
                reverse: false,
            });
            w.enemies.clear();
        }
        assert!(w.hp < START_HP);
        assert_eq!(w.state, State::Running);
    }

    #[test]
    fn same_seed_gives_the_same_run() {
        let turns = |i: usize| match i % 240 {
            0..40 => Some(Turn::Left),
            120..130 => Some(Turn::Right),
            _ => None,
        };
        let mut a = World::new(77);
        let mut b = World::new(77);
        run(&mut a, 3000, turns);
        run(&mut b, 3000, turns);
        assert!(!a.enemies.is_empty());
        assert_eq!(a.hp, b.hp);
        assert_eq!(a.kills, b.kills);
        assert_eq!(a.head_pos(), b.head_pos());
        let pa: Vec<V2> = a.enemies.iter().map(|e| e.pos).collect();
        let pb: Vec<V2> = b.enemies.iter().map(|e| e.pos).collect();
        assert_eq!(pa, pb);
    }

    #[test]
    fn ram_kills_a_resident() {
        let mut w = World::new(5);
        let ahead = w.head_pos() + V2::new(1.5, 0.0);
        w.spawn_enemy(EnemyKind::Resident, ahead);
        for _ in 0..30 {
            w.step(&Input::default());
        }
        assert_eq!(w.kills, 1);
    }

    #[test]
    fn touch_hurts_the_train() {
        let mut w = World::new(5);
        let tail = w.train.cars().last().unwrap().pos;
        w.spawn_enemy(EnemyKind::Bus, tail + V2::new(0.0, 0.5));
        w.step(&Input::default());
        assert_eq!(w.hp, START_HP - 15.0);
    }
}

#[cfg(test)]
mod weapon_tests {
    use super::*;

    /// A world with no spawns and one wagon of `kind`.
    fn quiet(kind: WagonKind) -> World {
        let mut w = World::new(3);
        w.wagons = vec![Wagon::new(kind)];
        w.director = Director::default();
        w
    }

    fn step(w: &mut World, n: usize) {
        for _ in 0..n {
            w.step(&Input::default());
            // Keep the director quiet: drop enemies that it spawns far away.
            let head = w.head_pos();
            w.enemies.retain(|e| e.pos.dist(head) < 8.0);
        }
    }

    #[test]
    fn armoured_wagon_shoots_nearest_enemy() {
        let mut w = quiet(WagonKind::Armoured);
        let wagon = w.train.slot(1).pos;
        w.spawn_enemy(EnemyKind::Resident, wagon + V2::new(-1.0, 4.0));
        w.spawn_enemy(EnemyKind::Resident, wagon + V2::new(-1.0, -5.5));
        step(&mut w, 1);
        assert_eq!(w.shots.len(), 1);
        assert!(w.shots[0].vel.y > 0.0, "aims at the nearer enemy");
        step(&mut w, 30);
        assert_eq!(w.kills, 1);
    }

    #[test]
    fn hopper_lights_the_tiles_it_passes() {
        let mut w = quiet(WagonKind::Hopper);
        step(&mut w, 60);
        assert!(w.fires.iter().count() >= 4);
        let tail = w.train.slot(1).pos;
        assert!(w.fires.dps_at(tail) > 0.0);
    }

    #[test]
    fn steam_blast_hits_in_radius_only() {
        let mut w = quiet(WagonKind::Steam);
        let wagon = w.train.slot(1).pos;
        w.spawn_enemy(EnemyKind::Bus, wagon + V2::new(0.0, 2.0));
        w.spawn_enemy(EnemyKind::Bus, wagon + V2::new(0.0, -4.0));
        step(&mut w, 1);
        assert_eq!(w.blasts.len(), 1);
        let near = w.enemies.iter().find(|e| e.id == 0).unwrap();
        let far = w.enemies.iter().find(|e| e.id == 1).unwrap();
        assert!(near.hp < near.max_hp);
        assert_eq!(far.hp, far.max_hp);
    }

    #[test]
    fn wagons_add_cars() {
        let mut w = World::new(1);
        for _ in 0..10 {
            w.add_wagon(WagonKind::Hopper);
        }
        assert_eq!(w.wagons.len(), MAX_WAGONS);
        assert_eq!(w.train.car_count(), MAX_WAGONS + 1);
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;

    #[test]
    fn crates_give_xp_and_level_up_pauses() {
        let mut w = World::new(8);
        let head = w.head_pos();
        w.drop_crate(head + V2::new(0.5, 0.5), 5);
        for _ in 0..5 {
            w.step(&Input::default());
        }
        assert!(w.crates.is_empty());
        assert_eq!(w.level, 2);
        assert!(matches!(w.state, State::LevelUp(_)));
        let t = w.time;
        w.step(&Input::default());
        assert_eq!(w.time, t, "paused while the cards show");
        w.pick_card(0);
        assert_eq!(w.state, State::Running);
    }

    #[test]
    fn far_crates_stay() {
        let mut w = World::new(8);
        let head = w.head_pos();
        w.drop_crate(head + V2::new(0.0, 5.0), 1);
        w.step(&Input::default());
        assert_eq!(w.crates.len(), 1);
        assert!(!w.crates[0].pulled);
    }

    #[test]
    fn several_level_ups_offer_several_choices() {
        let mut w = World::new(8);
        let head = w.head_pos();
        w.drop_crate(head, 5 + 9);
        w.step(&Input::default());
        assert_eq!(w.level, 3);
        w.pick_card(0);
        assert!(matches!(w.state, State::LevelUp(_)));
        w.pick_card(0);
        assert_eq!(w.state, State::Running);
    }

    #[test]
    fn cards_apply() {
        let mut w = World::new(8);
        w.hp = 50.0;
        w.pending = 1;
        w.state = State::LevelUp([Card::Upgrade(Upgrade::Armour), Card::Repair, Card::Repair]);
        w.pick_card(0);
        assert_eq!((w.hp, w.max_hp), (70.0, 120.0));
        w.pending = 1;
        w.state = State::LevelUp([Card::NewWagon(WagonKind::Steam), Card::Repair, Card::Repair]);
        w.pick_card(0);
        assert_eq!(w.wagons.len(), 2);
        assert_eq!(w.train.car_count(), 3);
        w.pending = 1;
        w.state = State::LevelUp([Card::LevelWagon(1), Card::Repair, Card::Repair]);
        w.pick_card(0);
        assert_eq!(w.wagons[1].level, 2);
    }
}

#[cfg(test)]
mod ufo_tests {
    use super::*;

    #[test]
    fn ufos_come_on_time_and_the_boss_wins_the_run() {
        let mut w = World::new(2);
        w.time = SMALL_UFO_TIMES[0] - DT / 2.0;
        w.step(&Input::default());
        assert_eq!(
            w.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SmallUfo)
                .count(),
            1
        );
        w.time = BOSS_TIME;
        w.step(&Input::default());
        assert_eq!(
            w.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SmallUfo)
                .count(),
            2
        );
        assert!(w.boss().is_some());
        w.enemies
            .iter_mut()
            .find(|e| e.kind == EnemyKind::LargeUfo)
            .unwrap()
            .hp = 0.0;
        w.step(&Input::default());
        assert_eq!(w.state, State::Won);
    }

    #[test]
    fn small_ufo_fires_plasma_that_hurts() {
        let mut w = World::new(2);
        let loco = w.train.slot(0).pos;
        w.spawn_enemy(EnemyKind::SmallUfo, loco + V2::new(5.0, 0.0));
        w.step(&Input::default());
        assert_eq!(w.plasma.len(), 1);
        let hp = w.hp;
        for _ in 0..90 {
            w.step(&Input::default());
        }
        assert!(w.plasma.is_empty(), "the plasma ball hit the train");
        assert!(w.hp <= hp - PLASMA_DAMAGE);
    }

    #[test]
    fn boss_removes_track_and_calls_residents() {
        let mut w = World::new(2);
        let c = MAP_SIZE / 2;
        let at = V2::new(c as f32 - 3.5, c as f32 + 0.5);
        assert_ne!(w.map.track(c - 4, c), 0);
        let e = Enemy::new(99, EnemyKind::LargeUfo, at, 1.0);
        w.enemies.push(e);
        w.step(&Input::default());
        assert_eq!(w.map.track(c - 4, c), 0);
        let residents = w
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Resident)
            .count();
        assert!(residents >= BOSS_CALL_SIZE as usize);
    }
}
