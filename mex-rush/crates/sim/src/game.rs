//! Mex Rush rules: capture metal spots, defend extractors from raids, spend
//! income on global upgrades and survive until extraction.

use crate::math::V2;
use crate::rng::Rng;

pub const WORLD: f32 = 2800.0;
pub const SPOT_RADIUS: f32 = 34.0;
pub const WIN_TIME: f32 = 600.0;
pub const BOSS_TIME: f32 = 540.0;
pub const STARVE_TIME: f32 = 20.0;
pub const GRACE: f32 = 40.0;
pub const START_METAL: f32 = 40.0;
pub const LLT_MAX: u8 = 3;
const PLAYER_R: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub build: bool,
    pub fortify: bool,
    /// Shop purchase (0..4) while playing, card pick (0..3) on level-up.
    pub pick: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp,
    Won,
    Lost(LossReason),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LossReason {
    Destroyed,
    Starved,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot,
    TowerShot,
    Hit,
    Kill { pos: V2, big: bool },
    MexBuilt,
    Fortified,
    MexLost { pos: V2 },
    MexHit,
    Pickup,
    LevelUp,
    Bought,
    Denied,
    Raid { spot: usize },
    BossSpawn,
    Starving,
    PlayerHit,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Raider,
    Grunt,
    Tank,
    Boss,
}

impl Kind {
    /// (hp, speed, radius, damage per second, xp)
    pub fn stats(self) -> (f32, f32, f32, f32, f32) {
        match self {
            Kind::Raider => (14.0, 120.0, 10.0, 10.0, 1.0),
            Kind::Grunt => (34.0, 75.0, 13.0, 16.0, 2.0),
            Kind::Tank => (140.0, 48.0, 20.0, 34.0, 6.0),
            Kind::Boss => (2600.0, 42.0, 40.0, 90.0, 60.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: Kind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub flash: f32,
    /// Preferred spot for raiders that were sent on a raid.
    pub raid: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Mex {
    pub hp: f32,
    pub llt: u8,
    pub cooldown: f32,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Spot {
    pub pos: V2,
    pub mex: Option<Mex>,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub tower: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Gem {
    pub pos: V2,
    pub xp: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Card {
    Damage,
    FireRate,
    Multishot,
    Speed,
    Armor,
    Range,
    Magnet,
}

pub const CARDS: [Card; 7] = [
    Card::Damage,
    Card::FireRate,
    Card::Multishot,
    Card::Speed,
    Card::Armor,
    Card::Range,
    Card::Magnet,
];

impl Card {
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            Card::Damage => ("Heavy Rounds", "+25% weapon damage"),
            Card::FireRate => ("Autoloader", "+20% fire rate"),
            Card::Multishot => ("Twin Barrels", "+1 projectile"),
            Card::Speed => ("Servo Legs", "+10% move speed"),
            Card::Armor => ("Armor Plating", "+30 max HP, full heal"),
            Card::Range => ("Targeting Array", "+15% weapon range"),
            Card::Magnet => ("Reclaim Field", "+50% pickup radius"),
        }
    }
}

/// Global upgrades bought with metal income.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shop {
    Output,
    Armor,
    Turrets,
    Repair,
}

pub const SHOP: [Shop; 4] = [Shop::Output, Shop::Armor, Shop::Turrets, Shop::Repair];

impl Shop {
    pub fn label(self) -> &'static str {
        match self {
            Shop::Output => "Mex output +25%",
            Shop::Armor => "Mex armor +40%",
            Shop::Turrets => "LLT damage +30%",
            Shop::Repair => "Repair all",
        }
    }

    fn base_cost(self) -> f32 {
        match self {
            Shop::Output => 60.0,
            Shop::Armor => 50.0,
            Shop::Turrets => 70.0,
            Shop::Repair => 40.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub fire_delay: f32,
    pub cooldown: f32,
    pub shots: u32,
    pub range: f32,
    pub magnet: f32,
    pub hurt: f32,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub time: f32,
    pub phase: Phase,
    pub player: Player,
    pub spots: Vec<Spot>,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub gems: Vec<Gem>,
    pub metal: f32,
    pub xp: f32,
    pub level: u32,
    pub choices: [Card; 3],
    pub pending_levels: u32,
    /// Levels bought per shop item.
    pub shop_levels: [u32; 4],
    pub starve: f32,
    pub kills: u32,
    pub mexes_built: u32,
    pub boss_spawned: bool,
    spawn_acc: f32,
    next_raid: f32,
    rng: Rng,
    pub events: Vec<Event>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut spots = Vec::new();
        // A 4x4 grid of metal spots with jitter; the centre one is free.
        let cell = WORLD / 4.0;
        for gy in 0..4 {
            for gx in 0..4 {
                let c = V2::new((gx as f32 + 0.5) * cell, (gy as f32 + 0.5) * cell);
                let j = V2::new(rng.range(-0.25, 0.25), rng.range(-0.25, 0.25)) * cell;
                spots.push(Spot {
                    pos: c + j,
                    mex: None,
                });
            }
        }
        let centre = V2::new(WORLD / 2.0, WORLD / 2.0);
        let player = Player {
            pos: centre,
            hp: 100.0,
            max_hp: 100.0,
            speed: 210.0,
            dmg: 10.0,
            fire_delay: 0.35,
            cooldown: 0.0,
            shots: 1,
            range: 330.0,
            magnet: 70.0,
            hurt: 0.0,
        };
        Game {
            time: 0.0,
            phase: Phase::Playing,
            player,
            spots,
            enemies: Vec::new(),
            bullets: Vec::new(),
            gems: Vec::new(),
            metal: START_METAL,
            xp: 0.0,
            level: 1,
            choices: [Card::Damage, Card::FireRate, Card::Speed],
            pending_levels: 0,
            shop_levels: [0; 4],
            starve: STARVE_TIME,
            kills: 0,
            mexes_built: 0,
            boss_spawned: false,
            spawn_acc: 0.0,
            next_raid: 35.0,
            rng,
            events: Vec::new(),
        }
    }

    pub fn owned(&self) -> usize {
        self.spots.iter().filter(|s| s.mex.is_some()).count()
    }

    pub fn mex_cost(&self) -> f32 {
        if self.mexes_built == 0 {
            0.0
        } else {
            25.0 + 5.0 * self.owned() as f32
        }
    }

    pub fn llt_cost(level: u8) -> f32 {
        40.0 + 40.0 * level as f32
    }

    pub fn shop_cost(&self, i: usize) -> f32 {
        SHOP[i].base_cost() * 1.5f32.powi(self.shop_levels[i] as i32)
    }

    pub fn income_per_mex(&self) -> f32 {
        2.0 * (1.0 + 0.25 * self.shop_levels[0] as f32)
    }

    pub fn income(&self) -> f32 {
        self.income_per_mex() * self.owned() as f32
    }

    pub fn mex_max_hp(&self) -> f32 {
        120.0 * (1.0 + 0.4 * self.shop_levels[1] as f32)
    }

    pub fn llt_dmg(&self, level: u8) -> f32 {
        (6.0 + 4.0 * level as f32) * (1.0 + 0.3 * self.shop_levels[2] as f32)
    }

    pub fn xp_to_next(&self) -> f32 {
        5.0 + 4.0 * self.level as f32 + 0.3 * (self.level * self.level) as f32
    }

    /// Index of the metal spot the commander stands on.
    pub fn spot_here(&self) -> Option<usize> {
        self.spots
            .iter()
            .position(|s| s.pos.dist(self.player.pos) < SPOT_RADIUS + PLAYER_R)
    }

    pub fn step(&mut self, dt: f32, input: &Input) {
        match self.phase {
            Phase::Playing => self.play(dt, input),
            Phase::LevelUp => {
                if let Some(i) = input.pick.filter(|i| *i < 3) {
                    self.apply_card(self.choices[i]);
                    self.pending_levels -= 1;
                    if self.pending_levels > 0 {
                        self.roll_choices();
                    } else {
                        self.phase = Phase::Playing;
                    }
                }
            }
            Phase::Won | Phase::Lost(_) => {}
        }
    }

    fn play(&mut self, dt: f32, input: &Input) {
        self.time += dt;
        let p = &mut self.player;
        p.pos += input.dir.norm() * (p.speed * dt);
        p.pos.x = p.pos.x.clamp(0.0, WORLD);
        p.pos.y = p.pos.y.clamp(0.0, WORLD);
        p.hurt = (p.hurt - dt).max(0.0);

        if input.build {
            self.try_build();
        }
        if input.fortify {
            self.try_fortify();
        }
        if let Some(i) = input.pick.filter(|i| *i < SHOP.len()) {
            self.try_buy(i);
        }

        self.metal += self.income() * dt;
        self.spawn(dt);
        self.fire(dt);
        self.towers(dt);
        self.move_bullets(dt);
        self.move_enemies(dt);
        self.collect(dt);
        self.check_end(dt);
    }

    pub fn try_build(&mut self) {
        let Some(i) = self.spot_here() else {
            self.events.push(Event::Denied);
            return;
        };
        let cost = self.mex_cost();
        if self.spots[i].mex.is_some() || self.metal < cost {
            self.events.push(Event::Denied);
            return;
        }
        self.metal -= cost;
        self.mexes_built += 1;
        self.spots[i].mex = Some(Mex {
            hp: self.mex_max_hp(),
            llt: 0,
            cooldown: 0.0,
            flash: 0.0,
        });
        self.events.push(Event::MexBuilt);
    }

    pub fn try_fortify(&mut self) {
        let Some(i) = self.spot_here() else {
            self.events.push(Event::Denied);
            return;
        };
        let metal = self.metal;
        match &mut self.spots[i].mex {
            Some(m) if m.llt < LLT_MAX && metal >= Self::llt_cost(m.llt) => {
                self.metal -= Self::llt_cost(m.llt);
                m.llt += 1;
                self.events.push(Event::Fortified);
            }
            _ => self.events.push(Event::Denied),
        }
    }

    pub fn try_buy(&mut self, i: usize) {
        let cost = self.shop_cost(i);
        if self.metal < cost {
            self.events.push(Event::Denied);
            return;
        }
        self.metal -= cost;
        let old_max = self.mex_max_hp();
        self.shop_levels[i] += 1;
        let max = self.mex_max_hp();
        match SHOP[i] {
            Shop::Armor => {
                for m in self.spots.iter_mut().filter_map(|s| s.mex.as_mut()) {
                    m.hp *= max / old_max;
                }
            }
            Shop::Repair => {
                for m in self.spots.iter_mut().filter_map(|s| s.mex.as_mut()) {
                    m.hp = max;
                }
                self.player.hp = self.player.max_hp;
            }
            _ => {}
        }
        self.events.push(Event::Bought);
    }

    /// Enemies per second at the current time.
    pub fn spawn_rate(&self) -> f32 {
        0.6 + self.time / 40.0 + (self.time / 150.0).powi(2)
    }

    fn spawn_pos(&mut self) -> V2 {
        // Spawn on a ring just outside the commander's view.
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let p = self.player.pos + V2::from_angle(a) * self.rng.range(620.0, 760.0);
        V2::new(
            p.x.clamp(-40.0, WORLD + 40.0),
            p.y.clamp(-40.0, WORLD + 40.0),
        )
    }

    fn pick_kind(&mut self) -> Kind {
        let r = self.rng.f32();
        let t = self.time;
        if t > 180.0 && r < 0.05 + t / 6000.0 {
            Kind::Tank
        } else if t > 45.0 && r < 0.45 {
            Kind::Grunt
        } else {
            Kind::Raider
        }
    }

    fn add_enemy(&mut self, kind: Kind, pos: V2, raid: Option<usize>) {
        let (hp, ..) = kind.stats();
        let hp = hp * (1.0 + self.time / 300.0);
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            flash: 0.0,
            raid,
        });
    }

    fn spawn(&mut self, dt: f32) {
        self.spawn_acc += self.spawn_rate() * dt;
        while self.spawn_acc >= 1.0 && self.enemies.len() < 400 {
            self.spawn_acc -= 1.0;
            let k = self.pick_kind();
            let p = self.spawn_pos();
            self.add_enemy(k, p, None);
        }
        if self.time >= self.next_raid {
            self.next_raid += (45.0 - self.time / 30.0).max(22.0);
            let owned: Vec<usize> = (0..self.spots.len())
                .filter(|&i| self.spots[i].mex.is_some())
                .collect();
            if !owned.is_empty() {
                let target = owned[self.rng.below(owned.len())];
                let n = 6 + (self.time / 25.0) as usize;
                let tp = self.spots[target].pos;
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let origin = tp + V2::from_angle(a) * 650.0;
                for _ in 0..n {
                    let jitter = V2::new(self.rng.range(-80.0, 80.0), self.rng.range(-80.0, 80.0));
                    let k = if self.rng.chance(0.15 + self.time / 1200.0) {
                        Kind::Grunt
                    } else {
                        Kind::Raider
                    };
                    self.add_enemy(k, origin + jitter, Some(target));
                }
                self.events.push(Event::Raid { spot: target });
            }
        }
        if !self.boss_spawned && self.time >= BOSS_TIME {
            self.boss_spawned = true;
            let p = self.spawn_pos();
            self.add_enemy(Kind::Boss, p, None);
            self.events.push(Event::BossSpawn);
        }
    }

    fn nearest_enemy(&self, from: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range * range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist2(from);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn fire(&mut self, dt: f32) {
        let p = &mut self.player;
        p.cooldown -= dt;
        if p.cooldown > 0.0 {
            return;
        }
        let (pos, range, shots, dmg) = (p.pos, p.range, p.shots, p.dmg);
        let Some(t) = self.nearest_enemy(pos, range) else {
            return;
        };
        self.player.cooldown = self.player.fire_delay;
        let base = (self.enemies[t].pos - pos).angle();
        for k in 0..shots {
            let off = (k as f32 - (shots - 1) as f32 / 2.0) * 0.14;
            self.bullets.push(Bullet {
                pos,
                vel: V2::from_angle(base + off) * 620.0,
                dmg,
                life: range / 620.0 + 0.1,
                tower: false,
            });
        }
        self.events.push(Event::Shot);
    }

    fn towers(&mut self, dt: f32) {
        for i in 0..self.spots.len() {
            let pos = self.spots[i].pos;
            let Some(m) = self.spots[i].mex.as_mut() else {
                continue;
            };
            m.flash = (m.flash - dt).max(0.0);
            if m.llt == 0 {
                continue;
            }
            m.cooldown -= dt;
            if m.cooldown > 0.0 {
                continue;
            }
            let lvl = m.llt;
            let range = 230.0 + 30.0 * lvl as f32;
            if let Some(t) = self.nearest_enemy(pos, range) {
                let dmg = self.llt_dmg(lvl);
                let m = self.spots[i].mex.as_mut().expect("checked above");
                m.cooldown = 0.5 - 0.08 * lvl as f32;
                let dir = (self.enemies[t].pos - pos).norm();
                self.bullets.push(Bullet {
                    pos,
                    vel: dir * 700.0,
                    dmg,
                    life: range / 700.0 + 0.1,
                    tower: true,
                });
                self.events.push(Event::TowerShot);
            }
        }
    }

    fn move_bullets(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.bullets.len() {
            let b = &mut self.bullets[i];
            b.pos += b.vel * dt;
            b.life -= dt;
            let mut dead = b.life <= 0.0;
            if !dead {
                let (bp, dmg) = (b.pos, b.dmg);
                if let Some(e) = self
                    .enemies
                    .iter_mut()
                    .find(|e| e.pos.dist(bp) < e.kind.stats().2 + 4.0)
                {
                    e.hp -= dmg;
                    e.flash = 0.08;
                    dead = true;
                    self.events.push(Event::Hit);
                }
            }
            if dead {
                self.bullets.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Remove the dead and drop xp gems.
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let (.., xp) = e.kind.stats();
                self.kills += 1;
                self.gems.push(Gem { pos: e.pos, xp });
                let big = matches!(e.kind, Kind::Tank | Kind::Boss);
                if e.kind == Kind::Boss {
                    self.metal += 300.0;
                }
                self.events.push(Event::Kill { pos: e.pos, big });
            } else {
                i += 1;
            }
        }
    }

    /// Where an enemy wants to go: the commander when close, otherwise the
    /// nearest extractor (raiders stick to their raid target).
    pub fn enemy_target(&self, e: &Enemy) -> V2 {
        let pd = e.pos.dist(self.player.pos);
        if pd < 180.0 || e.kind == Kind::Boss {
            return self.player.pos;
        }
        if let Some(s) = e.raid.filter(|&s| self.spots[s].mex.is_some()) {
            return self.spots[s].pos;
        }
        let mut best = self.player.pos;
        // Mexes look 1.6x closer than the commander: they are the bait.
        let mut bd = pd;
        for s in self.spots.iter().filter(|s| s.mex.is_some()) {
            let d = s.pos.dist(e.pos) / 1.6;
            if d < bd {
                bd = d;
                best = s.pos;
            }
        }
        best
    }

    fn move_enemies(&mut self, dt: f32) {
        let mut player_dmg = 0.0;
        for i in 0..self.enemies.len() {
            let target = self.enemy_target(&self.enemies[i]);
            let e = &mut self.enemies[i];
            e.flash = (e.flash - dt).max(0.0);
            let (_, speed, r, dps, _) = e.kind.stats();
            let to = target - e.pos;
            if to.len() > r + 12.0 {
                e.pos += to.norm() * (speed * dt);
            }
            if e.pos.dist(self.player.pos) < r + PLAYER_R {
                player_dmg += dps * dt;
            }
            let epos = e.pos;
            for s in self.spots.iter_mut() {
                if let Some(m) = s.mex.as_mut()
                    && s.pos.dist(epos) < r + SPOT_RADIUS
                {
                    m.hp -= dps * dt;
                    m.flash = 0.1;
                }
            }
        }
        // Light separation so hordes do not collapse into one point.
        let n = self.enemies.len();
        for i in 0..n {
            for j in (i + 1)..n.min(i + 24) {
                let d = self.enemies[i].pos - self.enemies[j].pos;
                let min = self.enemies[i].kind.stats().2 + self.enemies[j].kind.stats().2;
                let l = d.len();
                if l < min && l > 0.01 {
                    let push = d * ((min - l) / l * 0.5);
                    self.enemies[i].pos += push;
                    self.enemies[j].pos = self.enemies[j].pos - push;
                }
            }
        }
        if player_dmg > 0.0 {
            self.player.hp -= player_dmg;
            if self.player.hurt <= 0.0 {
                self.player.hurt = 0.25;
                self.events.push(Event::PlayerHit);
            }
        }
        for s in self.spots.iter_mut() {
            if let Some(m) = &s.mex {
                if m.hp <= 0.0 {
                    s.mex = None;
                    self.events.push(Event::MexLost { pos: s.pos });
                } else if m.flash >= 0.1 - 1e-6 && self.rng.chance(0.05) {
                    self.events.push(Event::MexHit);
                }
            }
        }
    }

    fn collect(&mut self, dt: f32) {
        let pp = self.player.pos;
        let mag = self.player.magnet;
        let mut gained = 0.0;
        self.gems.retain_mut(|g| {
            let d = g.pos.dist(pp);
            if d < PLAYER_R + 8.0 {
                gained += g.xp;
                return false;
            }
            if d < mag {
                g.pos += (pp - g.pos).norm() * (500.0 * dt);
            }
            true
        });
        if gained > 0.0 {
            self.events.push(Event::Pickup);
            self.add_xp(gained);
        }
    }

    pub fn add_xp(&mut self, xp: f32) {
        self.xp += xp;
        while self.xp >= self.xp_to_next() {
            self.xp -= self.xp_to_next();
            self.level += 1;
            self.pending_levels += 1;
        }
        if self.pending_levels > 0 && self.phase == Phase::Playing {
            self.roll_choices();
            self.phase = Phase::LevelUp;
            self.events.push(Event::LevelUp);
        }
    }

    fn roll_choices(&mut self) {
        let mut pool: Vec<Card> = CARDS.to_vec();
        for c in self.choices.iter_mut() {
            *c = pool.swap_remove(self.rng.below(pool.len()));
        }
    }

    pub fn apply_card(&mut self, c: Card) {
        let p = &mut self.player;
        match c {
            Card::Damage => p.dmg *= 1.25,
            Card::FireRate => p.fire_delay /= 1.2,
            Card::Multishot => p.shots += 1,
            Card::Speed => p.speed *= 1.1,
            Card::Armor => {
                p.max_hp += 30.0;
                p.hp = p.max_hp;
            }
            Card::Range => p.range *= 1.15,
            Card::Magnet => p.magnet *= 1.5,
        }
    }

    fn check_end(&mut self, dt: f32) {
        if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = Phase::Lost(LossReason::Destroyed);
            self.events.push(Event::Lost);
            return;
        }
        if self.owned() == 0 && (self.mexes_built > 0 || self.time > GRACE) {
            let before = self.starve;
            self.starve -= dt;
            if before.ceil() != self.starve.ceil() {
                self.events.push(Event::Starving);
            }
            if self.starve <= 0.0 {
                self.phase = Phase::Lost(LossReason::Starved);
                self.events.push(Event::Lost);
                return;
            }
        } else {
            self.starve = (self.starve + dt * 2.0).min(STARVE_TIME);
        }
        if self.time >= WIN_TIME {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_spot(g: &mut Game, i: usize) {
        g.player.pos = g.spots[i].pos;
    }

    #[test]
    fn first_mex_is_free_and_generates_income() {
        let mut g = Game::new(1);
        on_spot(&mut g, 0);
        g.try_build();
        assert_eq!(g.owned(), 1);
        assert_eq!(g.metal, START_METAL);
        let m0 = g.metal;
        g.metal += g.income() * 10.0;
        assert!(g.metal > m0);
        assert!(g.mex_cost() > 0.0);
    }

    #[test]
    fn cannot_build_off_spot_or_twice() {
        let mut g = Game::new(1);
        g.player.pos = V2::new(-500.0, -500.0);
        g.try_build();
        assert_eq!(g.owned(), 0);
        assert_eq!(g.events.last(), Some(&Event::Denied));
        on_spot(&mut g, 3);
        g.try_build();
        g.try_build();
        assert_eq!(g.owned(), 1);
    }

    #[test]
    fn fortify_costs_metal_and_caps_level() {
        let mut g = Game::new(2);
        on_spot(&mut g, 5);
        g.try_build();
        g.metal = 10_000.0;
        for _ in 0..5 {
            g.try_fortify();
        }
        assert_eq!(g.spots[5].mex.as_ref().unwrap().llt, LLT_MAX);
        let spent = (0..LLT_MAX).map(Game::llt_cost).sum::<f32>();
        assert!((10_000.0 - g.metal - spent).abs() < 1e-3);
    }

    #[test]
    fn enemies_prefer_mexes_over_distant_commander() {
        let mut g = Game::new(3);
        on_spot(&mut g, 0);
        g.try_build();
        g.player.pos = V2::new(WORLD, WORLD);
        let e = Enemy {
            kind: Kind::Raider,
            pos: g.spots[0].pos + V2::new(400.0, 0.0),
            hp: 1.0,
            max_hp: 1.0,
            flash: 0.0,
            raid: None,
        };
        assert_eq!(g.enemy_target(&e), g.spots[0].pos);
    }

    #[test]
    fn mex_is_destroyed_by_raiders() {
        let mut g = Game::new(4);
        on_spot(&mut g, 0);
        g.try_build();
        g.player.pos = V2::new(WORLD, WORLD);
        let p = g.spots[0].pos;
        g.add_enemy(Kind::Tank, p, Some(0));
        g.player.range = 0.0;
        for _ in 0..600 {
            g.step(1.0 / 60.0, &Input::default());
        }
        assert_eq!(g.owned(), 0);
        assert!(g.events.iter().any(|e| matches!(e, Event::MexLost { .. })));
    }

    #[test]
    fn starving_without_mexes_loses() {
        let mut g = Game::new(5);
        g.mexes_built = 1;
        g.player.pos = V2::new(-1000.0, -1000.0);
        g.next_raid = f32::MAX;
        let mut t = 0.0;
        while g.phase == Phase::Playing && t < STARVE_TIME + 2.0 {
            g.enemies.clear();
            g.step(0.1, &Input::default());
            t += 0.1;
        }
        assert_eq!(g.phase, Phase::Lost(LossReason::Starved));
    }

    #[test]
    fn xp_triggers_level_up_and_card_applies() {
        let mut g = Game::new(6);
        g.add_xp(1000.0);
        assert_eq!(g.phase, Phase::LevelUp);
        let lvls = g.pending_levels;
        assert!(lvls > 1);
        for _ in 0..lvls {
            g.step(
                0.016,
                &Input {
                    pick: Some(0),
                    ..Input::default()
                },
            );
        }
        assert_eq!(g.phase, Phase::Playing);
        let c = g.choices;
        assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
    }

    #[test]
    fn shop_scales_cost_and_buffs() {
        let mut g = Game::new(7);
        g.metal = 1000.0;
        let before = g.income_per_mex();
        let c0 = g.shop_cost(0);
        g.try_buy(0);
        assert!(g.income_per_mex() > before);
        assert!(g.shop_cost(0) > c0);
        g.metal = 0.0;
        g.try_buy(1);
        assert_eq!(g.shop_levels[1], 0);
    }

    #[test]
    fn surviving_wins_and_waves_escalate() {
        let g = Game::new(8);
        let mut late = g.clone();
        late.time = 400.0;
        assert!(late.spawn_rate() > 3.0 * g.spawn_rate());
        let mut w = Game::new(8);
        w.time = WIN_TIME - 0.01;
        w.player.hp = 1e9;
        w.spots[0].mex = Some(Mex {
            hp: 1e9,
            llt: 0,
            cooldown: 0.0,
            flash: 0.0,
        });
        w.boss_spawned = true;
        w.step(0.1, &Input::default());
        assert_eq!(w.phase, Phase::Won);
    }
}
