//! Game rules: Commander in a Transport, air-dropped structures, escalating
//! waves, reclaimable wrecks, level-up upgrades and the win/lose condition.

use crate::math::V2;
use crate::rng::Rng;

pub const WORLD: f32 = 3000.0;
pub const WAVE_SECS: f32 = 40.0;
pub const WAVES: u32 = 10;
pub const LAND_TIME: f32 = 0.6;
pub const CRATE_FALL: f32 = 1.2;
pub const CMD_RADIUS: f32 = 16.0;
const SPAWN_RING: f32 = 720.0;
const MAX_ENEMIES: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    Air,
    Ground,
    /// Descending; the value counts down to touchdown.
    Landing(f32),
    /// Climbing; the value counts down to being airborne.
    Lifting(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Raider,
    Grunt,
    Tank,
    Boss,
}

impl EnemyKind {
    /// (hp, speed, contact dps, radius, metal)
    pub fn stats(self) -> (f32, f32, f32, f32, u32) {
        match self {
            EnemyKind::Raider => (14.0, 105.0, 10.0, 9.0, 1),
            EnemyKind::Grunt => (32.0, 70.0, 16.0, 12.0, 2),
            EnemyKind::Tank => (130.0, 45.0, 30.0, 20.0, 6),
            EnemyKind::Boss => (2600.0, 40.0, 60.0, 42.0, 100),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Light laser turret.
    Turret,
    /// Dragon's teeth: cheap, sturdy wall that draws aggro.
    Wall,
    /// Metal extractor: passive income.
    Mex,
    /// Repair pad: heals the grounded Commander nearby and refuels.
    Repair,
}

pub const KINDS: [Kind; 4] = [Kind::Turret, Kind::Wall, Kind::Mex, Kind::Repair];

impl Kind {
    pub fn cost(self) -> u32 {
        match self {
            Kind::Turret => 50,
            Kind::Wall => 20,
            Kind::Mex => 70,
            Kind::Repair => 60,
        }
    }
    pub fn max_hp(self) -> f32 {
        match self {
            Kind::Turret => 160.0,
            Kind::Wall => 500.0,
            Kind::Mex => 120.0,
            Kind::Repair => 140.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Turret => "Laser Turret",
            Kind::Wall => "Dragon's Teeth",
            Kind::Mex => "Metal Extractor",
            Kind::Repair => "Repair Pad",
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Kind::Wall => 22.0,
            _ => 18.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Multishot,
    Speed,
    Armor,
    Shockwave,
    CrateCrush,
    TurretPower,
    Fuel,
    Magnet,
    Salvage,
}

pub const UPGRADES: [Upgrade; 11] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::Multishot,
    Upgrade::Speed,
    Upgrade::Armor,
    Upgrade::Shockwave,
    Upgrade::CrateCrush,
    Upgrade::TurretPower,
    Upgrade::Fuel,
    Upgrade::Magnet,
    Upgrade::Salvage,
];

impl Upgrade {
    pub fn title(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavier Plasma",
            Upgrade::FireRate => "Rapid Cycler",
            Upgrade::Multishot => "Split Barrel",
            Upgrade::Speed => "Servo Legs",
            Upgrade::Armor => "Armor Plating",
            Upgrade::Shockwave => "Landing Shock",
            Upgrade::CrateCrush => "Ballast Crates",
            Upgrade::TurretPower => "Turret Optics",
            Upgrade::Fuel => "Aux Fuel Tank",
            Upgrade::Magnet => "Nano Reclaim",
            Upgrade::Salvage => "Salvage Rights",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+30% shot damage",
            Upgrade::FireRate => "+20% fire rate",
            Upgrade::Multishot => "+1 projectile per volley",
            Upgrade::Speed => "+12% ground speed",
            Upgrade::Armor => "+30 max HP, full heal",
            Upgrade::Shockwave => "Touchdown blast +50% dmg & radius",
            Upgrade::CrateCrush => "Crates crush +60% harder",
            Upgrade::TurretPower => "Turrets +35% damage & range",
            Upgrade::Fuel => "+40% Transport fuel",
            Upgrade::Magnet => "+50% reclaim range",
            Upgrade::Salvage => "Drops cost 15% less",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub damage: f32,
    pub fire_delay: f32,
    pub shots: u32,
    pub ground_speed: f32,
    pub max_hp: f32,
    pub shock_dmg: f32,
    pub shock_radius: f32,
    pub crush_dmg: f32,
    pub turret_mult: f32,
    pub max_fuel: f32,
    pub magnet: f32,
    pub cost_mult: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            damage: 12.0,
            fire_delay: 0.32,
            shots: 1,
            ground_speed: 150.0,
            max_hp: 120.0,
            shock_dmg: 40.0,
            shock_radius: 110.0,
            crush_dmg: 90.0,
            turret_mult: 1.0,
            max_fuel: 8.0,
            magnet: 70.0,
            cost_mult: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub kind: EnemyKind,
    pub flash: f32,
    pub cooldown: f32,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub ttl: f32,
    /// Fired by the enemy boss (hurts the Commander/structures).
    pub hostile: bool,
}

#[derive(Clone, Debug)]
pub struct Wreck {
    pub pos: V2,
    pub metal: u32,
}

#[derive(Clone, Debug)]
pub struct Crate {
    pub pos: V2,
    pub kind: Kind,
    pub t: f32,
}

#[derive(Clone, Debug)]
pub struct Structure {
    pub pos: V2,
    pub kind: Kind,
    pub hp: f32,
    pub cooldown: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot,
    TurretShot { from: V2, to: V2 },
    Hit(V2),
    Kill { pos: V2, big: bool },
    Pickup,
    LevelUp,
    DropCalled,
    CrateLanded(V2),
    Denied,
    Touchdown(V2),
    Liftoff,
    StructureLost(V2),
    WaveStart(u32),
    BossSpawn,
    Hurt,
    LowFuel,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    LevelUp,
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct Input {
    /// Movement direction (any length; normalised internally).
    pub dir: V2,
    pub toggle_flight: bool,
    /// Call a drop of `selected` at this world position.
    pub drop_at: Option<V2>,
}

pub struct Game {
    pub rng: Rng,
    pub phase: Phase,
    pub time: f32,
    pub wave: u32,
    pub pos: V2,
    pub hp: f32,
    pub flight: Flight,
    pub fuel: f32,
    pub stats: Stats,
    pub metal: u32,
    pub xp: u32,
    pub level: u32,
    pub kills: u32,
    pub selected: Kind,
    pub choices: Vec<Upgrade>,
    pending_levels: u32,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub wrecks: Vec<Wreck>,
    pub crates: Vec<Crate>,
    pub structures: Vec<Structure>,
    pub events: Vec<Event>,
    fire_cd: f32,
    spawn_acc: f32,
    mex_acc: f32,
    pub boss_spawned: bool,
    hurt_cd: f32,
    fuel_warned: bool,
}

pub fn xp_needed(level: u32) -> u32 {
    6 + level * 5
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let stats = Stats::default();
        Game {
            rng: Rng::new(seed),
            phase: Phase::Playing,
            time: 0.0,
            wave: 1,
            pos: V2::new(WORLD / 2.0, WORLD / 2.0),
            hp: stats.max_hp,
            flight: Flight::Air,
            fuel: stats.max_fuel,
            stats,
            metal: 80,
            xp: 0,
            level: 1,
            kills: 0,
            selected: Kind::Turret,
            choices: Vec::new(),
            pending_levels: 0,
            enemies: Vec::new(),
            bullets: Vec::new(),
            wrecks: Vec::new(),
            crates: Vec::new(),
            structures: Vec::new(),
            events: vec![Event::WaveStart(1)],
            fire_cd: 0.0,
            spawn_acc: 0.0,
            mex_acc: 0.0,
            boss_spawned: false,
            hurt_cd: 0.0,
            fuel_warned: false,
        }
    }

    pub fn cost(&self, k: Kind) -> u32 {
        (k.cost() as f32 * self.stats.cost_mult).round() as u32
    }

    /// True while the Commander is safe in the Transport.
    pub fn airborne(&self) -> bool {
        !matches!(self.flight, Flight::Ground)
    }

    /// 0 = on the ground, 1 = fully airborne.
    pub fn altitude(&self) -> f32 {
        match self.flight {
            Flight::Air => 1.0,
            Flight::Ground => 0.0,
            Flight::Landing(t) => t / LAND_TIME,
            Flight::Lifting(t) => 1.0 - t / LAND_TIME,
        }
    }

    pub fn wave_progress(&self) -> f32 {
        ((self.time / WAVE_SECS).fract()).clamp(0.0, 1.0)
    }

    pub fn choose(&mut self, i: usize) {
        if self.phase != Phase::LevelUp || i >= self.choices.len() {
            return;
        }
        let up = self.choices[i];
        self.apply(up);
        self.pending_levels -= 1;
        if self.pending_levels > 0 {
            self.roll_choices();
        } else {
            self.choices.clear();
            self.phase = Phase::Playing;
        }
    }

    pub fn apply(&mut self, up: Upgrade) {
        let s = &mut self.stats;
        match up {
            Upgrade::Damage => s.damage *= 1.3,
            Upgrade::FireRate => s.fire_delay /= 1.2,
            Upgrade::Multishot => s.shots += 1,
            Upgrade::Speed => s.ground_speed *= 1.12,
            Upgrade::Armor => {
                s.max_hp += 30.0;
                self.hp = s.max_hp;
            }
            Upgrade::Shockwave => {
                s.shock_dmg *= 1.5;
                s.shock_radius *= 1.5;
            }
            Upgrade::CrateCrush => s.crush_dmg *= 1.6,
            Upgrade::TurretPower => s.turret_mult *= 1.35,
            Upgrade::Fuel => {
                s.max_fuel *= 1.4;
                self.fuel = s.max_fuel;
            }
            Upgrade::Magnet => s.magnet *= 1.5,
            Upgrade::Salvage => s.cost_mult *= 0.85,
        }
    }

    fn roll_choices(&mut self) {
        let mut pool: Vec<Upgrade> = UPGRADES.to_vec();
        self.choices.clear();
        for _ in 0..3 {
            let i = self.rng.below(pool.len());
            self.choices.push(pool.swap_remove(i));
        }
    }

    fn gain_xp(&mut self, n: u32) {
        self.xp += n;
        while self.xp >= xp_needed(self.level) {
            self.xp -= xp_needed(self.level);
            self.level += 1;
            self.pending_levels += 1;
            self.events.push(Event::LevelUp);
        }
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        let dt = dt.min(0.05);
        self.time += dt;
        self.hurt_cd -= dt;

        let wave = ((self.time / WAVE_SECS) as u32 + 1).min(WAVES);
        if wave != self.wave {
            self.wave = wave;
            self.events.push(Event::WaveStart(wave));
        }
        if self.wave == WAVES && !self.boss_spawned && self.wave_progress() > 0.15 {
            self.boss_spawned = true;
            let p = self.spawn_point();
            self.spawn(EnemyKind::Boss, p);
            self.events.push(Event::BossSpawn);
        }

        self.update_flight(dt, input);
        self.update_drop(input);
        self.spawn_waves(dt);
        self.update_crates(dt);
        self.update_commander_fire(dt);
        self.update_structures(dt);
        self.update_enemies(dt);
        self.update_bullets(dt);
        self.collect_wrecks(dt);
        self.reap();

        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        } else if self.boss_spawned && !self.enemies.iter().any(|e| e.kind == EnemyKind::Boss) {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
        } else if self.pending_levels > 0 {
            self.roll_choices();
            self.phase = Phase::LevelUp;
        }
    }

    fn update_flight(&mut self, dt: f32, input: &Input) {
        if input.toggle_flight {
            match self.flight {
                Flight::Air => self.flight = Flight::Landing(LAND_TIME),
                Flight::Ground => {
                    if self.fuel > 0.5 {
                        self.flight = Flight::Lifting(LAND_TIME);
                        self.events.push(Event::Liftoff);
                    } else {
                        self.events.push(Event::Denied);
                    }
                }
                _ => {}
            }
        }
        match self.flight {
            Flight::Landing(t) => {
                let t = t - dt;
                if t <= 0.0 {
                    self.touchdown();
                } else {
                    self.flight = Flight::Landing(t);
                }
            }
            Flight::Lifting(t) => {
                let t = t - dt;
                self.flight = if t <= 0.0 {
                    Flight::Air
                } else {
                    Flight::Lifting(t)
                };
            }
            _ => {}
        }
        // Fuel drains in the air and refills on the ground.
        if self.airborne() {
            self.fuel -= dt;
            if self.fuel < 2.0 && !self.fuel_warned && self.flight == Flight::Air {
                self.fuel_warned = true;
                self.events.push(Event::LowFuel);
            }
            if self.fuel <= 0.0 {
                self.fuel = 0.0;
                if self.flight == Flight::Air {
                    self.flight = Flight::Landing(LAND_TIME);
                }
            }
        } else {
            self.fuel = (self.fuel + dt * 0.8).min(self.stats.max_fuel);
            self.fuel_warned = false;
        }

        let speed = if self.airborne() {
            260.0
        } else {
            self.stats.ground_speed
        };
        self.pos += input.dir.norm() * speed * dt;
        self.pos.x = self.pos.x.clamp(0.0, WORLD);
        self.pos.y = self.pos.y.clamp(0.0, WORLD);
    }

    fn touchdown(&mut self) {
        self.flight = Flight::Ground;
        let (r, d) = (self.stats.shock_radius, self.stats.shock_dmg);
        let pos = self.pos;
        self.blast(pos, r, d, 220.0);
        self.events.push(Event::Touchdown(pos));
    }

    /// Damages and pushes back all enemies within `r` of `at`.
    fn blast(&mut self, at: V2, r: f32, dmg: f32, push: f32) {
        for e in &mut self.enemies {
            let d = e.pos.dist(at);
            if d < r + e.kind.stats().3 {
                e.hp -= dmg;
                e.flash = 0.12;
                if e.kind != EnemyKind::Boss {
                    e.pos = e.pos + (e.pos - at).norm() * push * (1.0 - d / (r * 1.5));
                }
            }
        }
    }

    fn update_drop(&mut self, input: &Input) {
        let Some(at) = input.drop_at else { return };
        let cost = self.cost(self.selected);
        if self.metal < cost {
            self.events.push(Event::Denied);
            return;
        }
        self.metal -= cost;
        let at = V2::new(at.x.clamp(0.0, WORLD), at.y.clamp(0.0, WORLD));
        self.crates.push(Crate {
            pos: at,
            kind: self.selected,
            t: CRATE_FALL,
        });
        self.events.push(Event::DropCalled);
    }

    fn update_crates(&mut self, dt: f32) {
        let mut landed = Vec::new();
        self.crates.retain_mut(|c| {
            c.t -= dt;
            if c.t <= 0.0 {
                landed.push((c.pos, c.kind));
                false
            } else {
                true
            }
        });
        for (pos, kind) in landed {
            let crush = self.stats.crush_dmg;
            self.blast(pos, kind.radius() + 34.0, crush, 60.0);
            self.structures.push(Structure {
                pos,
                kind,
                hp: kind.max_hp(),
                cooldown: 0.3,
            });
            self.events.push(Event::CrateLanded(pos));
        }
    }

    fn spawn_point(&mut self) -> V2 {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let p = self.pos + V2::from_angle(a) * SPAWN_RING;
        V2::new(p.x.clamp(0.0, WORLD), p.y.clamp(0.0, WORLD))
    }

    fn spawn(&mut self, kind: EnemyKind, pos: V2) {
        let (hp, ..) = kind.stats();
        let hp = hp * (1.0 + 0.12 * (self.wave - 1) as f32);
        self.enemies.push(Enemy {
            pos,
            hp,
            max_hp: hp,
            kind,
            flash: 0.0,
            cooldown: 2.0,
        });
    }

    /// Enemies per second for the current wave.
    pub fn spawn_rate(wave: u32, progress: f32) -> f32 {
        0.8 + wave as f32 * 0.75 + progress * wave as f32 * 0.35
    }

    fn spawn_waves(&mut self, dt: f32) {
        self.spawn_acc += dt * Self::spawn_rate(self.wave, self.wave_progress());
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            if self.enemies.len() >= MAX_ENEMIES {
                continue;
            }
            let roll = self.rng.f32();
            let w = self.wave as f32;
            let kind = if roll < 0.02 * w {
                EnemyKind::Tank
            } else if roll < 0.02 * w + 0.35 {
                EnemyKind::Raider
            } else {
                EnemyKind::Grunt
            };
            let p = self.spawn_point();
            // Packs: spawn small groups around the same point.
            let pack = if kind == EnemyKind::Raider { 3 } else { 1 };
            for _ in 0..pack {
                let j = V2::new(self.rng.range(-30.0, 30.0), self.rng.range(-30.0, 30.0));
                self.spawn(kind, p + j);
            }
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

    fn update_commander_fire(&mut self, dt: f32) {
        self.fire_cd -= dt;
        if self.airborne() || self.fire_cd > 0.0 {
            return;
        }
        let Some(i) = self.nearest_enemy(self.pos, 420.0) else {
            return;
        };
        self.fire_cd = self.stats.fire_delay;
        let base = (self.enemies[i].pos - self.pos).angle();
        let n = self.stats.shots;
        for k in 0..n {
            let off = (k as f32 - (n - 1) as f32 / 2.0) * 0.14;
            self.bullets.push(Bullet {
                pos: self.pos,
                vel: V2::from_angle(base + off) * 560.0,
                dmg: self.stats.damage,
                ttl: 0.9,
                hostile: false,
            });
        }
        self.events.push(Event::Shot);
    }

    fn update_structures(&mut self, dt: f32) {
        let grounded = !self.airborne();
        for si in 0..self.structures.len() {
            let s = &mut self.structures[si];
            s.cooldown -= dt;
            let (pos, kind) = (s.pos, s.kind);
            match kind {
                Kind::Turret => {
                    if s.cooldown > 0.0 {
                        continue;
                    }
                    let range = 260.0 * self.stats.turret_mult.sqrt();
                    if let Some(i) = self.nearest_enemy(pos, range) {
                        self.structures[si].cooldown = 0.55;
                        let e = &mut self.enemies[i];
                        e.hp -= 14.0 * self.stats.turret_mult * (1.0 + 0.1 * self.wave as f32);
                        e.flash = 0.1;
                        let to = e.pos;
                        self.events.push(Event::TurretShot { from: pos, to });
                    }
                }
                Kind::Mex => {
                    self.mex_acc += dt * 1.6;
                }
                Kind::Repair => {
                    if grounded && pos.dist(self.pos) < 90.0 {
                        self.hp = (self.hp + 12.0 * dt).min(self.stats.max_hp);
                        self.fuel = (self.fuel + 2.0 * dt).min(self.stats.max_fuel);
                    }
                }
                Kind::Wall => {}
            }
        }
        while self.mex_acc >= 1.0 {
            self.mex_acc -= 1.0;
            self.metal += 1;
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let cmd = self.pos;
        let grounded = !self.airborne();
        let n = self.enemies.len();
        for i in 0..n {
            let (pos, kind) = (self.enemies[i].pos, self.enemies[i].kind);
            let (_, speed, dps, radius, _) = kind.stats();
            // Target: the Commander when grounded, otherwise the nearest
            // structure; with nothing to hit they mill around below.
            let near_struct = self
                .structures
                .iter()
                .enumerate()
                .map(|(si, s)| (si, s.pos.dist2(pos)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let mut target = cmd;
            let mut target_struct = None;
            if let Some((si, d2)) = near_struct
                && (!grounded || d2 < pos.dist2(cmd) * 0.5)
            {
                target = self.structures[si].pos;
                target_struct = Some(si);
            }
            let to = target - pos;
            let dist = to.len();
            let reach =
                radius + target_struct.map_or(CMD_RADIUS, |si| self.structures[si].kind.radius());
            let e = &mut self.enemies[i];
            e.flash = (e.flash - dt).max(0.0);
            if !grounded && target_struct.is_none() && dist < 120.0 {
                // Circle under the Transport.
                let tangent = V2::new(-to.y, to.x).norm();
                e.pos += tangent * speed * 0.4 * dt;
            } else if dist > reach {
                e.pos += to.norm() * speed * dt;
            } else if let Some(si) = target_struct {
                self.structures[si].hp -= dps * dt;
            } else if grounded {
                self.hp -= dps * dt;
                if self.hurt_cd <= 0.0 {
                    self.hurt_cd = 0.4;
                    self.events.push(Event::Hurt);
                }
            }
            // Boss fires rings of shells.
            if kind == EnemyKind::Boss {
                let e = &mut self.enemies[i];
                e.cooldown -= dt;
                if e.cooldown <= 0.0 {
                    e.cooldown = 2.4;
                    let p = e.pos;
                    for k in 0..16 {
                        let a = k as f32 / 16.0 * std::f32::consts::TAU + self.time;
                        self.bullets.push(Bullet {
                            pos: p,
                            vel: V2::from_angle(a) * 190.0,
                            dmg: 14.0,
                            ttl: 3.5,
                            hostile: true,
                        });
                    }
                }
            }
        }
        // Light separation so hordes spread out.
        for i in 0..n {
            let j = (i * 7 + 3) % n;
            if i == j {
                continue;
            }
            let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
            let min = self.enemies[i].kind.stats().3 + self.enemies[j].kind.stats().3;
            let d = a.dist(b);
            if d < min && d > 0.01 {
                let push = (a - b).norm() * (min - d) * 0.5;
                self.enemies[i].pos = a + push;
                self.enemies[j].pos = b - push;
            }
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let mut bullets = std::mem::take(&mut self.bullets);
        let grounded = !self.airborne();
        bullets.retain_mut(|b| {
            b.pos += b.vel * dt;
            b.ttl -= dt;
            if b.ttl <= 0.0 {
                return false;
            }
            if b.hostile {
                if grounded && b.pos.dist(self.pos) < CMD_RADIUS + 4.0 {
                    self.hp -= b.dmg;
                    self.events.push(Event::Hurt);
                    return false;
                }
                for s in &mut self.structures {
                    if b.pos.dist(s.pos) < s.kind.radius() {
                        s.hp -= b.dmg;
                        return false;
                    }
                }
                return true;
            }
            for e in &mut self.enemies {
                if e.hp > 0.0 && b.pos.dist(e.pos) < e.kind.stats().3 + 3.0 {
                    e.hp -= b.dmg;
                    e.flash = 0.1;
                    self.events.push(Event::Hit(b.pos));
                    return false;
                }
            }
            true
        });
        self.bullets = bullets;
    }

    fn collect_wrecks(&mut self, dt: f32) {
        if self.airborne() {
            return;
        }
        let (pos, magnet) = (self.pos, self.stats.magnet);
        let mut got = 0;
        self.wrecks.retain_mut(|w| {
            let d = w.pos.dist(pos);
            if d < CMD_RADIUS + 6.0 {
                got += w.metal;
                false
            } else {
                if d < magnet {
                    w.pos = w.pos + (pos - w.pos).norm() * 380.0 * dt;
                }
                true
            }
        });
        if got > 0 {
            self.metal += got;
            self.gain_xp(got);
            self.events.push(Event::Pickup);
        }
    }

    fn reap(&mut self) {
        let mut dead = Vec::new();
        self.enemies.retain(|e| {
            if e.hp <= 0.0 {
                dead.push((e.pos, e.kind));
                false
            } else {
                true
            }
        });
        for (pos, kind) in dead {
            self.kills += 1;
            let big = matches!(kind, EnemyKind::Tank | EnemyKind::Boss);
            self.events.push(Event::Kill { pos, big });
            self.wrecks.push(Wreck {
                pos,
                metal: kind.stats().4,
            });
        }
        let mut lost = Vec::new();
        self.structures.retain(|s| {
            if s.hp <= 0.0 {
                lost.push(s.pos);
                false
            } else {
                true
            }
        });
        for p in lost {
            self.events.push(Event::StructureLost(p));
        }
        if self.wrecks.len() > 600 {
            let extra = self.wrecks.len() - 600;
            self.wrecks.drain(..extra);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input {
            dir: V2::ZERO,
            toggle_flight: false,
            drop_at: None,
        }
    }

    fn land(g: &mut Game) {
        g.update(
            0.016,
            &Input {
                toggle_flight: true,
                ..idle()
            },
        );
        for _ in 0..60 {
            g.update(0.016, &idle());
        }
        assert_eq!(g.flight, Flight::Ground);
    }

    #[test]
    fn starts_airborne_and_safe() {
        let mut g = Game::new(1);
        g.enemies.clear();
        g.spawn(EnemyKind::Grunt, g.pos);
        let hp = g.hp;
        for _ in 0..30 {
            g.update(0.016, &idle());
        }
        assert!(g.airborne());
        assert_eq!(g.hp, hp);
    }

    #[test]
    fn touchdown_blasts_nearby_enemies() {
        let mut g = Game::new(2);
        let p = g.pos + V2::new(40.0, 0.0);
        g.spawn(EnemyKind::Raider, p);
        land(&mut g);
        assert!(g.kills >= 1 || !g.wrecks.is_empty());
    }

    #[test]
    fn fuel_runs_out_and_forces_landing() {
        let mut g = Game::new(3);
        for _ in 0..((g.stats.max_fuel + 1.5) / 0.016) as usize {
            g.update(0.016, &idle());
            g.enemies.clear();
        }
        assert_eq!(g.flight, Flight::Ground);
    }

    #[test]
    fn cannot_lift_without_fuel() {
        let mut g = Game::new(3);
        land(&mut g);
        g.fuel = 0.0;
        g.update(
            0.016,
            &Input {
                toggle_flight: true,
                ..idle()
            },
        );
        assert_eq!(g.flight, Flight::Ground);
        assert!(g.events.contains(&Event::Denied));
    }

    #[test]
    fn drop_costs_metal_and_crushes_on_landing() {
        let mut g = Game::new(4);
        g.metal = 100;
        let at = g.pos + V2::new(300.0, 0.0);
        g.update(
            0.016,
            &Input {
                drop_at: Some(at),
                ..idle()
            },
        );
        assert_eq!(g.metal, 100 - Kind::Turret.cost());
        assert_eq!(g.crates.len(), 1);
        for i in 0..80 {
            if i == 70 {
                g.enemies.clear();
                g.spawn(EnemyKind::Grunt, at);
            }
            g.update(0.016, &idle());
        }
        assert_eq!(g.structures.len(), 1);
        assert!(g.kills >= 1);
    }

    #[test]
    fn drop_denied_when_broke() {
        let mut g = Game::new(5);
        g.metal = 0;
        g.update(
            0.016,
            &Input {
                drop_at: Some(g.pos),
                ..idle()
            },
        );
        assert!(g.crates.is_empty());
        assert!(g.events.contains(&Event::Denied));
    }

    #[test]
    fn wrecks_only_collected_on_ground() {
        let mut g = Game::new(6);
        g.wrecks.push(Wreck {
            pos: g.pos,
            metal: 5,
        });
        let m = g.metal;
        g.update(0.016, &idle());
        assert_eq!(g.metal, m);
        land(&mut g);
        assert!(g.metal >= m + 5);
    }

    #[test]
    fn level_up_pauses_and_applies_choice() {
        let mut g = Game::new(7);
        g.gain_xp(xp_needed(1));
        g.update(0.016, &idle());
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        let before = g.stats.clone();
        let up = g.choices[0];
        g.choose(0);
        assert_eq!(g.phase, Phase::Playing);
        let changed = format!("{before:?}") != format!("{:?}", g.stats);
        assert!(changed, "{up:?} changed nothing");
    }

    #[test]
    fn waves_escalate_and_boss_win() {
        assert!(Game::spawn_rate(5, 0.0) > Game::spawn_rate(1, 0.0));
        let mut g = Game::new(8);
        g.time = WAVE_SECS * (WAVES - 1) as f32 + WAVE_SECS * 0.2;
        g.update(0.016, &idle());
        assert_eq!(g.wave, WAVES);
        assert!(g.boss_spawned);
        g.enemies.clear();
        g.update(0.016, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn grounded_commander_dies_to_horde() {
        let mut g = Game::new(9);
        land(&mut g);
        g.stats.damage = 0.0;
        for _ in 0..40 {
            g.spawn(EnemyKind::Tank, g.pos + V2::new(10.0, 0.0));
        }
        for _ in 0..2000 {
            g.update(0.016, &idle());
            if g.phase != Phase::Playing {
                break;
            }
        }
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn mex_generates_metal() {
        let mut g = Game::new(10);
        g.structures.push(Structure {
            pos: V2::ZERO,
            kind: Kind::Mex,
            hp: 999.0,
            cooldown: 0.0,
        });
        let m = g.metal;
        for _ in 0..200 {
            g.update(0.016, &idle());
            g.enemies.clear();
        }
        assert!(g.metal > m);
    }
}
