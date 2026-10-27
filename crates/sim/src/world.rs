//! The world: map, train, enemies and the fixed-step simulation.

use crate::director::{self, Director};
use crate::enemy::{self, Enemy};
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
pub const MAX_WAGONS: usize = 5;

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
    Lost,
}

pub struct World {
    pub map: TileMap,
    pub train: Train,
    pub enemies: Vec<Enemy>,
    pub wagons: Vec<Wagon>,
    pub shots: Vec<Shot>,
    pub blasts: Vec<Blast>,
    pub fires: Fires,
    /// Weapon cooldown factor from upgrades.
    pub cooldown_factor: f32,
    pub rng: Pcg32,
    pub time: f32,
    pub state: State,
    pub hp: f32,
    pub max_hp: f32,
    pub kills: u32,
    iframes: f32,
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
            blasts: Vec::new(),
            fires: Fires::new(MAP_SIZE, MAP_SIZE),
            cooldown_factor: 1.0,
            rng,
            time: 0.0,
            state: State::Running,
            hp: START_HP,
            max_hp: START_HP,
            kills: 0,
            iframes: 0.0,
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
        // 4. Enemies.
        self.move_enemies();
        // 5. Weapons.
        self.fire_weapons();
        self.move_shots();
        self.burn();
        // 6. Hits.
        self.ram_and_touch();
        self.remove_dead();
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

    fn move_enemies(&mut self) {
        let cars: Vec<V2> = self.train.cars().iter().map(|c| c.pos).collect();
        let size = self.map.width() as f32;
        for e in &mut self.enemies {
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
                // Enemies cannot stand on the train.
                e.pos = car.pos + away * reach;
            });
        }
        if hurt > 0.0 && self.iframes <= 0.0 {
            self.damage(hurt);
            self.iframes = TOUCH_IFRAMES;
        }
    }

    fn fire_weapons(&mut self) {
        let factor = self.cooldown_factor;
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
                self.enemies.swap_remove(i);
                self.kills += 1;
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
    use crate::enemy::EnemyKind;

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
    use crate::enemy::EnemyKind;

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
