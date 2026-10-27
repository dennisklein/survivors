//! Game rules: the power grid, energy-costed weapons, EMP bot waves,
//! level-ups and the win/lose condition.

use crate::math::V2;
use crate::rng::Rng;

pub const ARENA: f32 = 1400.0; // half-size of the square arena
pub const WAVES: u32 = 12;
pub const WAVE_SECS: f32 = 35.0;
pub const BUILD_RANGE: f32 = 260.0;
pub const PLAYER_R: f32 = 16.0;
pub const PICKUP_R: f32 = 90.0;
/// Commander's own trickle reactor so the grid can never fully lock you out.
pub const BASE_GEN: f32 = 1.5;
pub const BASE_STORAGE: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wind,
    Solar,
    Storage,
    Turret,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Wind, Kind::Solar, Kind::Storage, Kind::Turret];

    pub fn cost(self) -> u32 {
        match self {
            Kind::Wind => 12,
            Kind::Solar => 8,
            Kind::Storage => 20,
            Kind::Turret => 30,
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            Kind::Wind => 70.0,
            Kind::Solar => 25.0,
            Kind::Storage => 90.0,
            Kind::Turret => 80.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            Kind::Wind => 18.0,
            Kind::Solar => 16.0,
            Kind::Storage => 17.0,
            Kind::Turret => 16.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Wind => "Wind Turbine",
            Kind::Solar => "Solar Collector",
            Kind::Storage => "Energy Storage",
            Kind::Turret => "Light Laser Tower",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Kind::Wind => "0-5 E/s, follows the gusts",
            Kind::Solar => "2 E/s steady, fragile",
            Kind::Storage => "+150 capacity",
            Kind::Turret => "auto-fires, 3 E/shot",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: Kind,
    pub pos: V2,
    pub hp: f32,
    pub cooldown: f32,
    pub flash: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Fast, weak, goes for the commander.
    Spark,
    /// Slow, drains energy in an aura, goes for buildings.
    Leech,
    /// Tanky bruiser that smashes buildings.
    Brute,
    /// Final-wave boss with a huge drain aura.
    Overload,
}

impl EnemyKind {
    fn stats(self) -> (f32, f32, f32, f32) {
        // (hp, speed, radius, contact dps)
        match self {
            EnemyKind::Spark => (14.0, 120.0, 10.0, 12.0),
            EnemyKind::Leech => (34.0, 60.0, 14.0, 8.0),
            EnemyKind::Brute => (110.0, 50.0, 20.0, 25.0),
            EnemyKind::Overload => (2600.0, 45.0, 42.0, 40.0),
        }
    }

    /// Energy drained per second while within `drain_range`.
    pub fn drain(self) -> f32 {
        match self {
            EnemyKind::Spark => 2.0,
            EnemyKind::Leech => 6.0,
            EnemyKind::Brute => 3.0,
            EnemyKind::Overload => 25.0,
        }
    }

    pub fn drain_range(self) -> f32 {
        match self {
            EnemyKind::Overload => 260.0,
            EnemyKind::Leech => 150.0,
            _ => 60.0,
        }
    }

    fn xp(self) -> u32 {
        match self {
            EnemyKind::Spark => 1,
            EnemyKind::Leech => 2,
            EnemyKind::Brute => 4,
            EnemyKind::Overload => 40,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub radius: f32,
    pub dps: f32,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Bolt {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: u32,
    pub from_turret: bool,
}

#[derive(Clone, Debug)]
pub struct Orb {
    pub pos: V2,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    LaserEfficiency,
    LaserDamage,
    FireRate,
    Multishot,
    ShieldEfficiency,
    TurbineBlades,
    SolarCells,
    Capacitors,
    Armor,
    Thrusters,
    Hardening,
}

impl Upgrade {
    pub const ALL: [Upgrade; 11] = [
        Upgrade::LaserEfficiency,
        Upgrade::LaserDamage,
        Upgrade::FireRate,
        Upgrade::Multishot,
        Upgrade::ShieldEfficiency,
        Upgrade::TurbineBlades,
        Upgrade::SolarCells,
        Upgrade::Capacitors,
        Upgrade::Armor,
        Upgrade::Thrusters,
        Upgrade::Hardening,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::LaserEfficiency => "Focusing Lens",
            Upgrade::LaserDamage => "Overcharged Emitter",
            Upgrade::FireRate => "Rapid Capacitor",
            Upgrade::Multishot => "Split Beam",
            Upgrade::ShieldEfficiency => "Shield Harmonics",
            Upgrade::TurbineBlades => "Turbine Blades",
            Upgrade::SolarCells => "Solar Cells",
            Upgrade::Capacitors => "Capacitor Banks",
            Upgrade::Armor => "Commander Armor",
            Upgrade::Thrusters => "Leg Servos",
            Upgrade::Hardening => "EMP Hardening",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::LaserEfficiency => "Laser costs 20% less energy",
            Upgrade::LaserDamage => "+30% laser damage",
            Upgrade::FireRate => "+20% fire rate",
            Upgrade::Multishot => "+1 laser bolt per shot",
            Upgrade::ShieldEfficiency => "Shield drains 25% less",
            Upgrade::TurbineBlades => "+25% wind output",
            Upgrade::SolarCells => "+30% solar output, +50% solar hp",
            Upgrade::Capacitors => "+80 storage, refill energy",
            Upgrade::Armor => "+30 max hp and full heal",
            Upgrade::Thrusters => "+12% move speed",
            Upgrade::Hardening => "EMP drain -20%",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub movement: V2,
    pub aim: V2,
    pub fire: bool,
    pub shield: bool,
    pub build: Option<(Kind, V2)>,
    pub choose: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot,
    TurretShot,
    Hit,
    Kill { big: bool, pos: V2 },
    PlayerHit,
    Shielded,
    Drained,
    BuildingLost { pos: V2 },
    Built,
    Denied,
    Pickup,
    LevelUp,
    WaveStart(u32),
    BossSpawn,
    Brownout,
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
pub struct Stats {
    pub laser_cost: f32,
    pub laser_dmg: f32,
    pub fire_rate: f32,
    pub bolts: u32,
    pub shield_drain: f32,
    pub wind_mult: f32,
    pub solar_mult: f32,
    pub solar_hp_mult: f32,
    pub extra_storage: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub emp_resist: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            laser_cost: 3.0,
            laser_dmg: 12.0,
            fire_rate: 5.0,
            bolts: 1,
            shield_drain: 14.0,
            wind_mult: 1.0,
            solar_mult: 1.0,
            solar_hp_mult: 1.0,
            extra_storage: 0.0,
            max_hp: 100.0,
            speed: 210.0,
            emp_resist: 1.0,
        }
    }
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub phase: Phase,
    pub player: V2,
    pub hp: f32,
    pub energy: f32,
    pub scrap: u32,
    pub xp: u32,
    pub level: u32,
    pub stats: Stats,
    pub buildings: Vec<Building>,
    pub enemies: Vec<Enemy>,
    pub bolts: Vec<Bolt>,
    pub orbs: Vec<Orb>,
    pub wave: u32,
    pub wave_time: f32,
    spawn_acc: f32,
    pub boss_spawned: bool,
    pub kills: u32,
    fire_cd: f32,
    pub shield_on: bool,
    pub choices: [Upgrade; 3],
    pending_levels: u32,
    pub hurt_flash: f32,
    /// Seed offset for the gust curve.
    gust_seed: f32,
    /// Energy produced / consumed over the last tick, per second.
    pub income: f32,
    pub spend: f32,
    was_empty: bool,
    pub events: Vec<Event>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let gust_seed = rng.range(0.0, 100.0);
        let mut g = Game {
            rng,
            time: 0.0,
            phase: Phase::Playing,
            player: V2::ZERO,
            hp: 100.0,
            energy: 60.0,
            scrap: 10,
            xp: 0,
            level: 1,
            stats: Stats::default(),
            buildings: Vec::new(),
            enemies: Vec::new(),
            bolts: Vec::new(),
            orbs: Vec::new(),
            wave: 1,
            wave_time: 0.0,
            spawn_acc: 0.0,
            boss_spawned: false,
            kills: 0,
            fire_cd: 0.0,
            shield_on: false,
            choices: [Upgrade::LaserDamage; 3],
            pending_levels: 0,
            hurt_flash: 0.0,
            gust_seed,
            income: 0.0,
            spend: 0.0,
            was_empty: false,
            events: Vec::new(),
        };
        // Starter grid: one turbine and one solar next to the commander.
        g.buildings
            .push(Building::new(Kind::Wind, V2::new(-60.0, 40.0)));
        g.buildings
            .push(Building::new(Kind::Solar, V2::new(60.0, 40.0)));
        g
    }

    /// Wind strength multiplier in `[0, 2]`: overlapping slow waves give
    /// lulls and gusts the player can read off the HUD.
    pub fn gust_at(&self, t: f32) -> f32 {
        let s = self.gust_seed;
        let v = 1.0
            + 0.55 * ((t + s) * 0.21).sin()
            + 0.3 * ((t + s * 1.7) * 0.53).sin()
            + 0.15 * ((t + s * 0.3) * 1.37).sin();
        v.clamp(0.0, 2.0)
    }

    pub fn gust(&self) -> f32 {
        self.gust_at(self.time)
    }

    pub fn capacity(&self) -> f32 {
        BASE_STORAGE
            + self.stats.extra_storage
            + 150.0
                * self
                    .buildings
                    .iter()
                    .filter(|b| b.kind == Kind::Storage)
                    .count() as f32
    }

    pub fn building_output(&self, b: &Building) -> f32 {
        match b.kind {
            Kind::Wind => 2.5 * self.gust() * self.stats.wind_mult,
            Kind::Solar => 2.0 * self.stats.solar_mult,
            _ => 0.0,
        }
    }

    /// Total generation per second right now.
    pub fn generation(&self) -> f32 {
        BASE_GEN
            + self
                .buildings
                .iter()
                .map(|b| self.building_output(b))
                .sum::<f32>()
    }

    pub fn xp_to_next(&self) -> u32 {
        5 + self.level * 4
    }

    fn building_max_hp(&self, k: Kind) -> f32 {
        if k == Kind::Solar {
            k.max_hp() * self.stats.solar_hp_mult
        } else {
            k.max_hp()
        }
    }

    /// Tries to spend `amount` energy; returns whether it was available.
    fn spend_energy(&mut self, amount: f32) -> bool {
        if self.energy >= amount {
            self.energy -= amount;
            self.spend += amount;
            true
        } else {
            false
        }
    }

    pub fn can_build(&self, kind: Kind, at: V2) -> bool {
        if self.scrap < kind.cost() || at.dist(self.player) > BUILD_RANGE {
            return false;
        }
        if at.x.abs() > ARENA || at.y.abs() > ARENA {
            return false;
        }
        self.buildings
            .iter()
            .all(|b| b.pos.dist(at) > b.kind.radius() + kind.radius() + 4.0)
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        self.events.clear();
        match self.phase {
            Phase::Won | Phase::Lost => return,
            Phase::LevelUp => {
                if let Some(i) = input.choose
                    && i < 3
                {
                    self.apply(self.choices[i]);
                    self.pending_levels -= 1;
                    if self.pending_levels > 0 {
                        self.roll_choices();
                        self.events.push(Event::LevelUp);
                    } else {
                        self.phase = Phase::Playing;
                    }
                }
                return;
            }
            Phase::Playing => {}
        }
        let dt = dt.min(0.05);
        self.time += dt;
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);
        self.spend = 0.0;

        // Generation.
        let gen_rate = self.generation();
        self.income = gen_rate;
        let cap = self.capacity();
        self.energy = (self.energy + gen_rate * dt).min(cap);

        // Movement.
        let mv = input.movement.norm();
        self.player += mv * (self.stats.speed * dt);
        self.player.x = self.player.x.clamp(-ARENA, ARENA);
        self.player.y = self.player.y.clamp(-ARENA, ARENA);

        // Shield.
        let shield_cost = self.stats.shield_drain * dt;
        let was_on = self.shield_on;
        self.shield_on = input.shield && self.spend_energy(shield_cost);
        if self.shield_on && !was_on {
            self.events.push(Event::Shielded);
        }

        // Laser.
        self.fire_cd -= dt;
        if input.fire && self.fire_cd <= 0.0 {
            if self.spend_energy(self.stats.laser_cost) {
                self.fire_cd = 1.0 / self.stats.fire_rate;
                let dir = (input.aim - self.player).norm();
                let dir = if dir == V2::ZERO {
                    V2::new(1.0, 0.0)
                } else {
                    dir
                };
                let n = self.stats.bolts;
                let base = dir.angle();
                for i in 0..n {
                    let off = (i as f32 - (n - 1) as f32 / 2.0) * 0.12;
                    self.bolts.push(Bolt {
                        pos: self.player + dir * PLAYER_R,
                        vel: V2::from_angle(base + off) * 760.0,
                        dmg: self.stats.laser_dmg,
                        life: 0.9,
                        pierce: 1,
                        from_turret: false,
                    });
                }
                self.events.push(Event::Shot);
            } else {
                self.fire_cd = 0.2;
            }
        }

        // Building placement.
        if let Some((kind, at)) = input.build {
            if self.can_build(kind, at) {
                self.scrap -= kind.cost();
                let mut b = Building::new(kind, at);
                b.hp = self.building_max_hp(kind);
                self.buildings.push(b);
                self.events.push(Event::Built);
            } else {
                self.events.push(Event::Denied);
            }
        }

        self.update_turrets(dt);
        self.update_bolts(dt);
        self.update_enemies(dt);
        self.update_orbs(dt);
        self.update_waves(dt);

        // Brownout warning when the bank runs dry.
        let empty = self.energy < 1.0;
        if empty && !self.was_empty {
            self.events.push(Event::Brownout);
        }
        self.was_empty = empty;

        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        }
    }

    fn update_turrets(&mut self, dt: f32) {
        for i in 0..self.buildings.len() {
            let b = &mut self.buildings[i];
            b.flash = (b.flash - dt).max(0.0);
            if b.kind != Kind::Turret {
                continue;
            }
            b.cooldown -= dt;
            if b.cooldown > 0.0 {
                continue;
            }
            let pos = b.pos;
            let target = self
                .enemies
                .iter()
                .filter(|e| e.pos.dist2(pos) < 320.0 * 320.0)
                .min_by(|a, c| a.pos.dist2(pos).total_cmp(&c.pos.dist2(pos)))
                .map(|e| e.pos);
            if let Some(t) = target {
                if self.energy >= 3.0 {
                    self.energy -= 3.0;
                    self.spend += 3.0;
                    self.buildings[i].cooldown = 0.55;
                    self.bolts.push(Bolt {
                        pos,
                        vel: (t - pos).norm() * 650.0,
                        dmg: 10.0 + self.stats.laser_dmg * 0.5,
                        life: 0.6,
                        pierce: 1,
                        from_turret: true,
                    });
                    self.events.push(Event::TurretShot);
                } else {
                    self.buildings[i].cooldown = 0.3;
                }
            }
        }
    }

    fn update_bolts(&mut self, dt: f32) {
        let mut bolts = std::mem::take(&mut self.bolts);
        for b in &mut bolts {
            b.pos += b.vel * dt;
            b.life -= dt;
            for e in &mut self.enemies {
                if b.pierce == 0 || e.hp <= 0.0 {
                    continue;
                }
                if e.pos.dist2(b.pos) < (e.radius + 4.0).powi(2) {
                    e.hp -= b.dmg;
                    e.flash = 0.1;
                    b.pierce -= 1;
                    self.events.push(Event::Hit);
                }
            }
        }
        bolts.retain(|b| b.life > 0.0 && b.pierce > 0);
        self.bolts = bolts;
        self.reap();
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                self.kills += 1;
                let big = matches!(e.kind, EnemyKind::Brute | EnemyKind::Overload);
                self.events.push(Event::Kill { big, pos: e.pos });
                let value = e.kind.xp();
                self.orbs.push(Orb { pos: e.pos, value });
                if e.kind == EnemyKind::Overload {
                    self.phase = Phase::Won;
                    self.events.push(Event::Won);
                }
            } else {
                i += 1;
            }
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let resist = self.stats.emp_resist;
        let mut drained = 0.0;
        let mut player_dmg = 0.0;
        for e in &mut self.enemies {
            e.flash = (e.flash - dt).max(0.0);
            // Leeches and Brutes hunt the grid; everything else the commander.
            let hunts_grid = matches!(e.kind, EnemyKind::Leech | EnemyKind::Brute);
            let mut target = self.player;
            if hunts_grid
                && let Some(b) = self
                    .buildings
                    .iter()
                    .min_by(|a, c| a.pos.dist2(e.pos).total_cmp(&c.pos.dist2(e.pos)))
                && b.pos.dist2(e.pos) < self.player.dist2(e.pos) * 2.5
            {
                target = b.pos;
            }
            let to = target - e.pos;
            if to.len() > e.radius {
                e.pos += to.norm() * (e.speed * dt);
            }
            // EMP drain.
            let d = e.pos.dist(self.player);
            if d < e.kind.drain_range() {
                drained += e.kind.drain() * resist * dt;
            }
            // Contact damage to the commander.
            if d < e.radius + PLAYER_R {
                player_dmg += e.dps * dt;
            }
            // Contact damage to buildings.
            for b in &mut self.buildings {
                if b.pos.dist(e.pos) < e.radius + b.kind.radius() + 2.0 {
                    b.hp -= e.dps * dt;
                    b.flash = 0.1;
                }
            }
        }
        // Simple separation so hordes do not stack into one dot.
        let n = self.enemies.len();
        for i in 0..n {
            for j in (i + 1)..n.min(i + 12) {
                let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
                let min = self.enemies[i].radius + self.enemies[j].radius;
                let d = a.dist(b);
                if d < min && d > 0.01 {
                    let push = (a - b).norm() * ((min - d) * 0.5);
                    self.enemies[i].pos += push;
                    self.enemies[j].pos += push * -1.0;
                }
            }
        }
        if drained > 0.0 {
            // Shield blocks EMP, but at the cost of extra drain on itself.
            let d = if self.shield_on {
                drained * 0.3
            } else {
                drained
            };
            self.energy = (self.energy - d).max(0.0);
            if self.rng.chance(dt * 3.0) {
                self.events.push(Event::Drained);
            }
        }
        if player_dmg > 0.0 {
            let dmg = if self.shield_on {
                player_dmg * 0.15
            } else {
                player_dmg
            };
            self.hp -= dmg;
            if self.hurt_flash <= 0.0 {
                self.hurt_flash = 0.25;
                self.events.push(Event::PlayerHit);
            }
        }
        let mut lost = Vec::new();
        self.buildings.retain(|b| {
            if b.hp <= 0.0 {
                lost.push(b.pos);
                false
            } else {
                true
            }
        });
        for pos in lost {
            self.events.push(Event::BuildingLost { pos });
        }
        let cap = self.capacity();
        self.energy = self.energy.min(cap);
    }

    fn update_orbs(&mut self, dt: f32) {
        let p = self.player;
        let mut gained = 0;
        self.orbs.retain_mut(|o| {
            let d = o.pos.dist(p);
            if d < PLAYER_R + 8.0 {
                gained += o.value;
                return false;
            }
            if d < PICKUP_R {
                o.pos += (p - o.pos).norm() * (420.0 * dt);
            }
            true
        });
        if gained > 0 {
            self.scrap += gained;
            self.xp += gained;
            self.events.push(Event::Pickup);
            while self.xp >= self.xp_to_next() {
                self.xp -= self.xp_to_next();
                self.level += 1;
                self.pending_levels += 1;
            }
            if self.pending_levels > 0 && self.phase == Phase::Playing {
                self.phase = Phase::LevelUp;
                self.roll_choices();
                self.events.push(Event::LevelUp);
            }
        }
    }

    fn roll_choices(&mut self) {
        let mut pool: Vec<Upgrade> = Upgrade::ALL.to_vec();
        for c in &mut self.choices {
            let i = self.rng.below(pool.len());
            *c = pool.swap_remove(i);
        }
    }

    pub fn apply(&mut self, u: Upgrade) {
        let s = &mut self.stats;
        match u {
            Upgrade::LaserEfficiency => s.laser_cost *= 0.8,
            Upgrade::LaserDamage => s.laser_dmg *= 1.3,
            Upgrade::FireRate => s.fire_rate *= 1.2,
            Upgrade::Multishot => s.bolts += 1,
            Upgrade::ShieldEfficiency => s.shield_drain *= 0.75,
            Upgrade::TurbineBlades => s.wind_mult *= 1.25,
            Upgrade::SolarCells => {
                s.solar_mult *= 1.3;
                s.solar_hp_mult *= 1.5;
                for b in &mut self.buildings {
                    if b.kind == Kind::Solar {
                        b.hp *= 1.5;
                    }
                }
            }
            Upgrade::Capacitors => {
                s.extra_storage += 80.0;
                self.energy = self.capacity();
            }
            Upgrade::Armor => {
                s.max_hp += 30.0;
                self.hp = s.max_hp;
            }
            Upgrade::Thrusters => s.speed *= 1.12,
            Upgrade::Hardening => s.emp_resist *= 0.8,
        }
    }

    /// Enemies per second for the current wave.
    pub fn spawn_rate(&self) -> f32 {
        0.8 + self.wave as f32 * 0.55
    }

    fn update_waves(&mut self, dt: f32) {
        self.wave_time += dt;
        if self.wave_time >= WAVE_SECS && self.wave < WAVES {
            self.wave += 1;
            self.wave_time = 0.0;
            self.events.push(Event::WaveStart(self.wave));
        }
        if self.wave == WAVES && !self.boss_spawned {
            self.boss_spawned = true;
            self.spawn(EnemyKind::Overload);
            self.events.push(Event::BossSpawn);
        }
        if self.enemies.len() >= 260 {
            return;
        }
        self.spawn_acc += self.spawn_rate() * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let roll = self.rng.f32();
            let w = self.wave as f32;
            let kind = if roll < 0.02 * w {
                EnemyKind::Brute
            } else if roll < 0.08 + 0.04 * w {
                EnemyKind::Leech
            } else {
                EnemyKind::Spark
            };
            self.spawn(kind);
        }
    }

    pub fn spawn(&mut self, kind: EnemyKind) {
        let (hp, speed, radius, dps) = kind.stats();
        let scale = 1.0 + (self.wave as f32 - 1.0) * 0.12;
        let ang = self.rng.range(0.0, std::f32::consts::TAU);
        let pos = self.player + V2::from_angle(ang) * self.rng.range(620.0, 760.0);
        let hp = if kind == EnemyKind::Overload {
            hp
        } else {
            hp * scale
        };
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            speed: speed * self.rng.range(0.9, 1.1),
            radius,
            dps,
            flash: 0.0,
        });
    }

    pub fn boss(&self) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.kind == EnemyKind::Overload)
    }
}

impl Building {
    pub fn new(kind: Kind, pos: V2) -> Self {
        Building {
            kind,
            pos,
            hp: kind.max_hp(),
            cooldown: 0.0,
            flash: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn gust_stays_in_range_and_varies() {
        let g = Game::new(1);
        let samples: Vec<f32> = (0..600).map(|i| g.gust_at(i as f32 * 0.5)).collect();
        assert!(samples.iter().all(|v| (0.0..=2.0).contains(v)));
        let (lo, hi) = samples
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo > 0.8, "wind should gust noticeably");
    }

    #[test]
    fn generators_fill_storage_up_to_capacity() {
        let mut g = Game::new(2);
        g.energy = 0.0;
        g.enemies.clear();
        for _ in 0..2000 {
            g.wave_time = 0.0;
            g.spawn_acc = 0.0;
            g.update(0.05, &idle());
        }
        assert!((g.energy - g.capacity()).abs() < 1e-3);
    }

    #[test]
    fn storage_raises_capacity() {
        let mut g = Game::new(3);
        let before = g.capacity();
        g.buildings
            .push(Building::new(Kind::Storage, V2::new(0.0, 100.0)));
        assert_eq!(g.capacity(), before + 150.0);
    }

    #[test]
    fn firing_costs_energy_and_fails_when_empty() {
        let mut g = Game::new(4);
        g.energy = 50.0;
        let input = Input {
            fire: true,
            aim: V2::new(100.0, 0.0),
            ..Input::default()
        };
        g.update(0.01, &input);
        assert_eq!(g.bolts.len(), 1);
        assert!(g.events.contains(&Event::Shot));
        assert!(g.energy < 50.0);
        let mut g = Game::new(4);
        g.energy = 0.0;
        g.buildings.clear();
        g.update(0.001, &input);
        assert!(g.bolts.is_empty());
    }

    #[test]
    fn shield_drains_energy_and_reduces_damage() {
        let mut a = Game::new(5);
        let mut b = Game::new(5);
        for g in [&mut a, &mut b] {
            g.energy = 100.0;
            g.spawn(EnemyKind::Brute);
            g.enemies[0].pos = g.player;
            g.enemies[0].speed = 0.0;
        }
        a.update(
            0.05,
            &Input {
                shield: true,
                ..idle()
            },
        );
        b.update(0.05, &idle());
        assert!(a.shield_on);
        assert!(a.hp > b.hp);
    }

    #[test]
    fn emp_bots_drain_energy() {
        let mut g = Game::new(6);
        g.buildings.clear();
        g.energy = 50.0;
        g.spawn(EnemyKind::Leech);
        g.enemies[0].pos = V2::new(100.0, 0.0);
        g.update(0.05, &idle());
        assert!(g.energy < 50.0 + BASE_GEN * 0.05 - 0.1);
    }

    #[test]
    fn building_needs_scrap_and_range() {
        let mut g = Game::new(7);
        g.scrap = 100;
        let far = V2::new(BUILD_RANGE + 50.0, 0.0);
        assert!(!g.can_build(Kind::Wind, far));
        let near = V2::new(0.0, -120.0);
        assert!(g.can_build(Kind::Wind, near));
        g.update(
            0.01,
            &Input {
                build: Some((Kind::Wind, near)),
                ..idle()
            },
        );
        assert_eq!(g.scrap, 100 - Kind::Wind.cost());
        assert!(g.events.contains(&Event::Built));
        assert!(!g.can_build(Kind::Solar, near), "no overlapping buildings");
        g.scrap = 0;
        assert!(!g.can_build(Kind::Solar, V2::new(0.0, 150.0)));
    }

    #[test]
    fn killing_drops_orbs_that_level_up() {
        let mut g = Game::new(8);
        g.spawn(EnemyKind::Brute);
        g.enemies[0].hp = 0.0;
        g.reap();
        assert_eq!(g.orbs.len(), 1);
        g.orbs[0].value = 100;
        g.orbs[0].pos = g.player;
        g.update(0.01, &idle());
        assert_eq!(g.phase, Phase::LevelUp);
        let lvl = g.level;
        assert!(lvl > 1);
        // Choosing resolves all pending levels one at a time.
        for _ in 0..lvl {
            if g.phase != Phase::LevelUp {
                break;
            }
            g.update(
                0.01,
                &Input {
                    choose: Some(0),
                    ..idle()
                },
            );
        }
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn choices_are_distinct() {
        let mut g = Game::new(9);
        for _ in 0..50 {
            g.roll_choices();
            let c = g.choices;
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        }
    }

    #[test]
    fn waves_escalate_and_boss_kill_wins() {
        let mut g = Game::new(10);
        let r1 = g.spawn_rate();
        g.wave_time = WAVE_SECS;
        g.update(0.01, &idle());
        assert_eq!(g.wave, 2);
        assert!(g.spawn_rate() > r1);
        g.wave = WAVES;
        g.update(0.01, &idle());
        assert!(g.boss_spawned);
        assert!(g.events.contains(&Event::BossSpawn));
        for e in &mut g.enemies {
            e.hp = 0.0;
        }
        g.reap();
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn commander_dies_and_game_stops() {
        let mut g = Game::new(11);
        g.hp = 0.5;
        g.spawn(EnemyKind::Brute);
        g.enemies[0].pos = g.player;
        g.enemies[0].speed = 0.0;
        for _ in 0..20 {
            g.update(0.05, &idle());
        }
        assert_eq!(g.phase, Phase::Lost);
        let t = g.time;
        g.update(0.05, &idle());
        assert_eq!(g.time, t);
    }

    #[test]
    fn buildings_get_destroyed() {
        let mut g = Game::new(12);
        g.player = V2::new(1000.0, 1000.0);
        let solar = g.buildings[1].pos;
        g.spawn(EnemyKind::Brute);
        g.enemies[0].pos = solar;
        g.enemies[0].speed = 0.0;
        let mut lost = false;
        for _ in 0..40 {
            g.update(0.05, &idle());
            lost |= g
                .events
                .iter()
                .any(|e| matches!(e, Event::BuildingLost { .. }));
        }
        assert!(lost);
        assert!(g.buildings.iter().all(|b| b.kind != Kind::Solar));
    }

    #[test]
    fn upgrades_apply() {
        let mut g = Game::new(13);
        let c = g.stats.laser_cost;
        g.apply(Upgrade::LaserEfficiency);
        assert!(g.stats.laser_cost < c);
        g.apply(Upgrade::Multishot);
        assert_eq!(g.stats.bolts, 2);
        let cap = g.capacity();
        g.apply(Upgrade::Capacitors);
        assert_eq!(g.capacity(), cap + 80.0);
        assert_eq!(g.energy, g.capacity());
    }
}
