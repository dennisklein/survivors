//! Core simulation: the commander, drone waves, wrecks, buildings,
//! level-ups and the boss. Advance it with [`Game::step`].

use crate::math::{V2, seg_dist};
use crate::rng::Rng;

/// Survive this long (seconds) and the T3 boss drops in.
pub const BOSS_TIME: f32 = 20.0 * 60.0;
/// Buildings must be placed within this distance of the commander.
pub const BUILD_RANGE: f32 = 280.0;
pub const BUILDING_R: f32 = 18.0;
pub const COMMANDER_R: f32 = 18.0;
pub const DGUN_LEN: f32 = 620.0;
pub const DGUN_WIDTH: f32 = 34.0;
pub const MAX_ENEMIES: usize = 380;
const SPAWN_DIST: f32 = 760.0;
const WRECK_LIFE: f32 = 45.0;
const STORAGE: f32 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Tiny fast swarmer.
    Tick,
    /// Basic light bot.
    Grunt,
    /// Ranged rocket bot that keeps its distance.
    Gunner,
    /// Slow armoured brawler.
    Brute,
    /// Periodic mini-boss.
    Elite,
    /// The T3 boss.
    Boss,
}

pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub radius: f32,
    pub xp: u32,
    pub metal: f32,
}

impl EnemyKind {
    pub fn stats(self) -> EnemyStats {
        let s = |hp, speed, dmg, radius, xp, metal| EnemyStats {
            hp,
            speed,
            dmg,
            radius,
            xp,
            metal,
        };
        match self {
            EnemyKind::Tick => s(7.0, 150.0, 4.0, 9.0, 1, 4.0),
            EnemyKind::Grunt => s(20.0, 88.0, 8.0, 13.0, 2, 8.0),
            EnemyKind::Gunner => s(32.0, 70.0, 10.0, 14.0, 4, 14.0),
            EnemyKind::Brute => s(85.0, 55.0, 20.0, 20.0, 7, 28.0),
            EnemyKind::Elite => s(900.0, 62.0, 40.0, 32.0, 40, 160.0),
            EnemyKind::Boss => s(9000.0, 48.0, 70.0, 52.0, 0, 0.0),
        }
    }

    pub fn is_big(self) -> bool {
        matches!(self, EnemyKind::Elite | EnemyKind::Boss)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildKind {
    LaserTower,
    Solar,
    HeavyLaser,
    Nano,
    Converter,
}

pub const BUILD_KINDS: [BuildKind; 5] = [
    BuildKind::LaserTower,
    BuildKind::Solar,
    BuildKind::HeavyLaser,
    BuildKind::Nano,
    BuildKind::Converter,
];

impl BuildKind {
    pub fn name(self) -> &'static str {
        match self {
            BuildKind::LaserTower => "Light Laser Tower",
            BuildKind::Solar => "Solar Collector",
            BuildKind::HeavyLaser => "Heavy Laser Tower",
            BuildKind::Nano => "Nano Turret",
            BuildKind::Converter => "Energy Converter",
        }
    }

    /// Base (metal, energy) cost.
    pub fn base_cost(self) -> (f32, f32) {
        match self {
            BuildKind::LaserTower => (50.0, 60.0),
            BuildKind::Solar => (35.0, 0.0),
            BuildKind::HeavyLaser => (160.0, 300.0),
            BuildKind::Nano => (90.0, 150.0),
            BuildKind::Converter => (60.0, 100.0),
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            BuildKind::LaserTower => 220.0,
            BuildKind::Solar => 160.0,
            BuildKind::HeavyLaser => 600.0,
            BuildKind::Nano => 180.0,
            BuildKind::Converter => 140.0,
        }
    }

    /// (range, damage, seconds between shots) for towers.
    pub fn weapon(self) -> Option<(f32, f32, f32)> {
        match self {
            BuildKind::LaserTower => Some((260.0, 9.0, 0.45)),
            BuildKind::HeavyLaser => Some((350.0, 45.0, 1.1)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    LaserDamage,
    FireRate,
    MultiLaser,
    Speed,
    Armor,
    Regen,
    Reclaim,
    DGunCooldown,
    TowerDamage,
    TowerRange,
    TowerRate,
    Fortify,
    SolarOutput,
    CheapBuild,
    Income,
}

pub const UPGRADES: [Upgrade; 15] = [
    Upgrade::LaserDamage,
    Upgrade::FireRate,
    Upgrade::MultiLaser,
    Upgrade::Speed,
    Upgrade::Armor,
    Upgrade::Regen,
    Upgrade::Reclaim,
    Upgrade::DGunCooldown,
    Upgrade::TowerDamage,
    Upgrade::TowerRange,
    Upgrade::TowerRate,
    Upgrade::Fortify,
    Upgrade::SolarOutput,
    Upgrade::CheapBuild,
    Upgrade::Income,
];

impl Upgrade {
    pub fn for_buildings(self) -> bool {
        matches!(
            self,
            Upgrade::TowerDamage
                | Upgrade::TowerRange
                | Upgrade::TowerRate
                | Upgrade::Fortify
                | Upgrade::SolarOutput
                | Upgrade::CheapBuild
                | Upgrade::Income
        )
    }

    pub fn title(self) -> &'static str {
        match self {
            Upgrade::LaserDamage => "Overcharged Laser",
            Upgrade::FireRate => "Rapid Capacitors",
            Upgrade::MultiLaser => "Split Beam",
            Upgrade::Speed => "Servo Legs",
            Upgrade::Armor => "Reinforced Hull",
            Upgrade::Regen => "Field Repair",
            Upgrade::Reclaim => "Reclaim Nanolathe",
            Upgrade::DGunCooldown => "D-Gun Coolant",
            Upgrade::TowerDamage => "Tower Focusing Lens",
            Upgrade::TowerRange => "Tower Targeting",
            Upgrade::TowerRate => "Tower Heat Sinks",
            Upgrade::Fortify => "Fortified Frames",
            Upgrade::SolarOutput => "Solar Efficiency",
            Upgrade::CheapBuild => "Efficient Blueprints",
            Upgrade::Income => "Metal Extraction",
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Upgrade::LaserDamage => "Commander laser +30% damage",
            Upgrade::FireRate => "Commander fires 18% faster",
            Upgrade::MultiLaser => "Commander laser hits +1 target",
            Upgrade::Speed => "Commander moves 12% faster",
            Upgrade::Armor => "+80 max HP and full repair",
            Upgrade::Regen => "+2 HP/s regeneration",
            Upgrade::Reclaim => "Reclaim radius +25%, speed +40%",
            Upgrade::DGunCooldown => "D-Gun cooldown -18%",
            Upgrade::TowerDamage => "Towers +30% damage",
            Upgrade::TowerRange => "Towers +15% range",
            Upgrade::TowerRate => "Towers fire 18% faster",
            Upgrade::Fortify => "Buildings +50% HP, all repaired",
            Upgrade::SolarOutput => "Solar collectors +60% energy",
            Upgrade::CheapBuild => "Buildings cost 15% less",
            Upgrade::Income => "+1.5 metal/s passive income",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Commander {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub regen: f32,
    pub speed: f32,
    pub laser_dmg: f32,
    pub fire_every: f32,
    pub fire_cd: f32,
    pub range: f32,
    pub targets: usize,
    pub reclaim_r: f32,
    pub reclaim_rate: f32,
    pub dgun_cd: f32,
    pub dgun_every: f32,
    pub facing: f32,
    pub hurt: f32,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub atk_cd: f32,
    pub flash: f32,
    pub id: u32,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cd: f32,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Wreck {
    pub pos: V2,
    pub metal: f32,
    pub age: f32,
    pub big: bool,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub ttl: f32,
    pub big: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamKind {
    Commander,
    Tower,
    Heavy,
    Reclaim,
    Nano,
    DGun,
}

#[derive(Clone, Debug)]
pub struct Beam {
    pub a: V2,
    pub b: V2,
    pub ttl: f32,
    pub kind: BeamKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot { pos: V2 },
    TowerShot { pos: V2, heavy: bool },
    Hit { pos: V2 },
    Kill { pos: V2, big: bool },
    Reclaimed { pos: V2 },
    CommanderHit,
    BuildingLost { pos: V2 },
    Built { kind: BuildKind, pos: V2 },
    BuildFailed,
    DGun { from: V2, to: V2 },
    LevelUp,
    Surge,
    EliteSpawn,
    BossWarning,
    BossSpawn,
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp([Upgrade; 3]),
    Won,
    Lost,
}

/// Per-frame player intent.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Desired movement direction (any length; normalised internally).
    pub mv: V2,
    /// World-space aim point for the D-Gun.
    pub aim: V2,
    pub dgun: bool,
    pub place: Option<(BuildKind, V2)>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub kills: u32,
    pub metal_reclaimed: f32,
    pub built: u32,
}

pub struct Game {
    pub time: f32,
    pub phase: Phase,
    pub cmd: Commander,
    pub enemies: Vec<Enemy>,
    pub buildings: Vec<Building>,
    pub wrecks: Vec<Wreck>,
    pub bullets: Vec<Bullet>,
    pub beams: Vec<Beam>,
    pub metal: f32,
    pub energy: f32,
    pub metal_income: f32,
    pub energy_income: f32,
    pub level: u32,
    pub xp: u32,
    pub pending_levels: u32,
    pub tower_dmg: f32,
    pub tower_range: f32,
    pub tower_rate: f32,
    pub building_hp: f32,
    pub solar_mult: f32,
    pub cost_mult: f32,
    pub stats: Stats,
    pub boss_spawned: bool,
    pub events: Vec<Event>,
    rng: Rng,
    spawn_acc: f32,
    next_surge: f32,
    next_elite: f32,
    warned: bool,
    next_id: u32,
}

/// Experience needed to go from `level` to `level + 1`.
pub fn xp_to_next(level: u32) -> u32 {
    5 + level * 5 + level * level / 2
}

/// Enemies spawned per second at time `t`.
pub fn spawn_rate(t: f32) -> f32 {
    0.8 + t * 0.006 + (t / 400.0).powi(2)
}

/// Enemy health multiplier at time `t`.
pub fn hp_scale(t: f32) -> f32 {
    1.0 + t / 260.0
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Game {
            time: 0.0,
            phase: Phase::Playing,
            cmd: Commander {
                pos: V2::ZERO,
                hp: 320.0,
                max_hp: 320.0,
                regen: 1.0,
                speed: 165.0,
                laser_dmg: 10.0,
                fire_every: 0.32,
                fire_cd: 0.0,
                range: 290.0,
                targets: 1,
                reclaim_r: 140.0,
                reclaim_rate: 30.0,
                dgun_cd: 0.0,
                dgun_every: 9.0,
                facing: 0.0,
                hurt: 0.0,
            },
            enemies: Vec::new(),
            buildings: Vec::new(),
            wrecks: Vec::new(),
            bullets: Vec::new(),
            beams: Vec::new(),
            metal: 120.0,
            energy: 150.0,
            metal_income: 1.0,
            energy_income: 3.0,
            level: 1,
            xp: 0,
            pending_levels: 0,
            tower_dmg: 1.0,
            tower_range: 1.0,
            tower_rate: 1.0,
            building_hp: 1.0,
            solar_mult: 1.0,
            cost_mult: 1.0,
            stats: Stats::default(),
            boss_spawned: false,
            events: Vec::new(),
            rng: Rng::new(seed),
            spawn_acc: 0.0,
            next_surge: 150.0,
            next_elite: 300.0,
            warned: false,
            next_id: 0,
        }
    }

    pub fn cost(&self, kind: BuildKind) -> (f32, f32) {
        let (m, e) = kind.base_cost();
        ((m * self.cost_mult).round(), (e * self.cost_mult).round())
    }

    pub fn can_afford(&self, kind: BuildKind) -> bool {
        let (m, e) = self.cost(kind);
        self.metal >= m && self.energy >= e
    }

    /// Whether a building of any kind may go at `pos` (ignoring cost).
    pub fn placement_ok(&self, pos: V2) -> bool {
        pos.dist(self.cmd.pos) <= BUILD_RANGE
            && pos.dist(self.cmd.pos) >= BUILDING_R + COMMANDER_R
            && self
                .buildings
                .iter()
                .all(|b| b.pos.dist(pos) >= BUILDING_R * 2.0)
    }

    /// Tries to build `kind` at `pos`; returns whether it was placed.
    pub fn place(&mut self, kind: BuildKind, pos: V2) -> bool {
        if self.phase != Phase::Playing || !self.can_afford(kind) || !self.placement_ok(pos) {
            self.events.push(Event::BuildFailed);
            return false;
        }
        let (m, e) = self.cost(kind);
        self.metal -= m;
        self.energy -= e;
        let max_hp = kind.max_hp() * self.building_hp;
        self.buildings.push(Building {
            kind,
            pos,
            hp: max_hp,
            max_hp,
            cd: 0.0,
            flash: 0.0,
        });
        self.stats.built += 1;
        self.events.push(Event::Built { kind, pos });
        true
    }

    pub fn energy_rate(&self) -> f32 {
        let solar = self
            .buildings
            .iter()
            .filter(|b| b.kind == BuildKind::Solar)
            .count() as f32;
        self.energy_income + solar * 5.0 * self.solar_mult
    }

    fn converters(&self) -> f32 {
        self.buildings
            .iter()
            .filter(|b| b.kind == BuildKind::Converter)
            .count() as f32
    }

    /// Picks the level-up option `i` (0..3).
    pub fn choose(&mut self, i: usize) {
        let Phase::LevelUp(opts) = self.phase.clone() else {
            return;
        };
        self.apply(opts[i.min(2)]);
        self.pending_levels -= 1;
        self.phase = Phase::Playing;
        self.offer_level();
    }

    pub fn apply(&mut self, u: Upgrade) {
        let c = &mut self.cmd;
        match u {
            Upgrade::LaserDamage => c.laser_dmg *= 1.3,
            Upgrade::FireRate => c.fire_every *= 0.82,
            Upgrade::MultiLaser => c.targets += 1,
            Upgrade::Speed => c.speed *= 1.12,
            Upgrade::Armor => {
                c.max_hp += 80.0;
                c.hp = c.max_hp;
            }
            Upgrade::Regen => c.regen += 2.0,
            Upgrade::Reclaim => {
                c.reclaim_r *= 1.25;
                c.reclaim_rate *= 1.4;
            }
            Upgrade::DGunCooldown => c.dgun_every *= 0.82,
            Upgrade::TowerDamage => self.tower_dmg *= 1.3,
            Upgrade::TowerRange => self.tower_range *= 1.15,
            Upgrade::TowerRate => self.tower_rate *= 0.82,
            Upgrade::Fortify => {
                self.building_hp *= 1.5;
                for b in &mut self.buildings {
                    b.max_hp *= 1.5;
                    b.hp = b.max_hp;
                }
            }
            Upgrade::SolarOutput => self.solar_mult *= 1.6,
            Upgrade::CheapBuild => self.cost_mult *= 0.85,
            Upgrade::Income => self.metal_income += 1.5,
        }
    }

    fn offer_level(&mut self) {
        if self.pending_levels == 0 || self.phase != Phase::Playing {
            return;
        }
        // One commander option, one building option, one wildcard.
        let cmd: Vec<_> = UPGRADES.iter().filter(|u| !u.for_buildings()).collect();
        let bld: Vec<_> = UPGRADES.iter().filter(|u| u.for_buildings()).collect();
        let a = *cmd[self.rng.below(cmd.len())];
        let b = *bld[self.rng.below(bld.len())];
        let c = loop {
            let u = UPGRADES[self.rng.below(UPGRADES.len())];
            if u != a && u != b {
                break u;
            }
        };
        self.phase = Phase::LevelUp([a, b, c]);
    }

    fn gain_xp(&mut self, xp: u32) {
        self.xp += xp;
        while self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            self.pending_levels += 1;
            self.events.push(Event::LevelUp);
        }
    }

    pub fn spawn(&mut self, kind: EnemyKind, pos: V2) {
        let st = kind.stats();
        let scale = if kind == EnemyKind::Boss {
            1.0
        } else {
            hp_scale(self.time)
        };
        self.next_id += 1;
        self.enemies.push(Enemy {
            kind,
            pos,
            hp: st.hp * scale,
            max_hp: st.hp * scale,
            atk_cd: 0.0,
            flash: 0.0,
            id: self.next_id,
        });
    }

    fn ring_pos(&mut self, dist: f32) -> V2 {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        self.cmd.pos + V2::from_angle(a) * (dist + self.rng.range(0.0, 80.0))
    }

    fn pick_kind(&mut self) -> EnemyKind {
        let t = self.time;
        let mut table = vec![(EnemyKind::Tick, 5.0), (EnemyKind::Grunt, 3.0)];
        if t > 100.0 {
            table.push((EnemyKind::Gunner, 1.0 + t / 300.0));
        }
        if t > 220.0 {
            table.push((EnemyKind::Brute, 0.5 + t / 250.0));
        }
        let total: f32 = table.iter().map(|x| x.1).sum();
        let mut r = self.rng.range(0.0, total);
        for (k, w) in &table {
            if r < *w {
                return *k;
            }
            r -= w;
        }
        EnemyKind::Grunt
    }

    fn spawn_waves(&mut self, dt: f32) {
        if self.time >= BOSS_TIME - 30.0 && !self.warned {
            self.warned = true;
            self.events.push(Event::BossWarning);
        }
        if self.time >= BOSS_TIME && !self.boss_spawned {
            self.boss_spawned = true;
            let p = self.ring_pos(620.0);
            self.spawn(EnemyKind::Boss, p);
            self.events.push(Event::BossSpawn);
        }
        if self.time >= self.next_elite && self.time < BOSS_TIME {
            self.next_elite += 300.0;
            let p = self.ring_pos(SPAWN_DIST);
            self.spawn(EnemyKind::Elite, p);
            self.events.push(Event::EliteSpawn);
        }
        if self.time >= self.next_surge {
            self.next_surge += 150.0;
            let n = 20 + (self.time / 20.0) as usize;
            let a0 = self.rng.range(0.0, std::f32::consts::TAU);
            for i in 0..n {
                if self.enemies.len() >= MAX_ENEMIES {
                    break;
                }
                let a = a0 + i as f32 / n as f32 * std::f32::consts::TAU;
                let p = self.cmd.pos + V2::from_angle(a) * SPAWN_DIST;
                let k = if i % 3 == 0 {
                    EnemyKind::Grunt
                } else {
                    EnemyKind::Tick
                };
                self.spawn(k, p);
            }
            self.events.push(Event::Surge);
        }
        let rate = if self.boss_spawned {
            spawn_rate(self.time) * 0.5
        } else {
            spawn_rate(self.time)
        };
        self.spawn_acc += rate * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            if self.enemies.len() < MAX_ENEMIES {
                let k = self.pick_kind();
                let p = self.ring_pos(SPAWN_DIST);
                self.spawn(k, p);
            }
        }
    }

    /// Advances the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        let dt = dt.min(0.05);
        self.time += dt;
        for b in &mut self.beams {
            b.ttl -= dt;
        }
        self.beams.retain(|b| b.ttl > 0.0);

        if let Some((k, p)) = input.place {
            self.place(k, p);
        }
        self.economy(dt);
        self.move_commander(dt, input);
        if input.dgun {
            self.fire_dgun(input.aim);
        }
        self.spawn_waves(dt);
        self.move_enemies(dt);
        self.commander_fire(dt);
        self.towers(dt);
        self.nanos(dt);
        self.update_bullets(dt);
        self.reclaim(dt);
        self.reap();

        if self.cmd.hp <= 0.0 {
            self.cmd.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
            return;
        }
        if self.boss_spawned && !self.enemies.iter().any(|e| e.kind == EnemyKind::Boss) {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }
        self.offer_level();
    }

    fn economy(&mut self, dt: f32) {
        self.energy += self.energy_rate() * dt;
        self.metal += self.metal_income * dt;
        // Each converter turns 10 E/s into 1 M/s while energy lasts.
        let conv = self.converters();
        let want = conv * 10.0 * dt;
        let used = want.min(self.energy.max(0.0));
        self.energy -= used;
        self.metal += used / 10.0;
        self.metal = self.metal.min(STORAGE);
        self.energy = self.energy.min(STORAGE);
    }

    fn move_commander(&mut self, dt: f32, input: &Input) {
        let c = &mut self.cmd;
        let d = input.mv.norm();
        if d != V2::ZERO {
            c.facing = d.angle();
        }
        c.pos += d * (c.speed * dt);
        // Buildings block the commander.
        for b in &self.buildings {
            let off = c.pos - b.pos;
            let min = BUILDING_R + COMMANDER_R;
            if off.len() < min {
                c.pos = b.pos + off.norm() * min;
            }
        }
        c.hp = (c.hp + c.regen * dt).min(c.max_hp);
        c.fire_cd -= dt;
        c.dgun_cd = (c.dgun_cd - dt).max(0.0);
        c.hurt = (c.hurt - dt).max(0.0);
    }

    pub fn dgun_ready(&self) -> bool {
        self.cmd.dgun_cd <= 0.0
    }

    fn fire_dgun(&mut self, aim: V2) {
        if !self.dgun_ready() {
            return;
        }
        let from = self.cmd.pos;
        let mut dir = (aim - from).norm();
        if dir == V2::ZERO {
            dir = V2::from_angle(self.cmd.facing);
        }
        let to = from + dir * DGUN_LEN;
        self.cmd.dgun_cd = self.cmd.dgun_every;
        for e in &mut self.enemies {
            if seg_dist(e.pos, from, to) <= DGUN_WIDTH + e.kind.stats().radius {
                e.hp -= if e.kind.is_big() { 1200.0 } else { f32::MAX };
                e.flash = 0.2;
            }
        }
        self.bullets
            .retain(|b| seg_dist(b.pos, from, to) > DGUN_WIDTH);
        self.beams.push(Beam {
            a: from,
            b: to,
            ttl: 0.45,
            kind: BeamKind::DGun,
        });
        self.events.push(Event::DGun { from, to });
    }

    fn move_enemies(&mut self, dt: f32) {
        let cpos = self.cmd.pos;
        // Separation uses a snapshot so order does not matter.
        let snap: Vec<(V2, f32)> = self
            .enemies
            .iter()
            .map(|e| (e.pos, e.kind.stats().radius))
            .collect();
        for (i, e) in self.enemies.iter_mut().enumerate() {
            let st = e.kind.stats();
            e.flash = (e.flash - dt).max(0.0);
            e.atk_cd -= dt;
            // Target: the commander, unless a building is close by.
            let mut target = cpos;
            let mut tr = COMMANDER_R;
            let mut tb = None;
            let mut best = e.pos.dist(cpos);
            for (bi, b) in self.buildings.iter().enumerate() {
                let d = e.pos.dist(b.pos);
                if d < 130.0 && d < best {
                    best = d;
                    target = b.pos;
                    tr = BUILDING_R;
                    tb = Some(bi);
                }
            }
            let to = target - e.pos;
            let dist = to.len();
            let mut vel = to.norm() * st.speed;
            if matches!(e.kind, EnemyKind::Gunner | EnemyKind::Boss) && dist < 240.0 {
                vel = vel * -0.4;
            }
            let mut push = V2::ZERO;
            if e.kind != EnemyKind::Boss {
                for (j, (p, r)) in snap.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let off = e.pos - *p;
                    let min = st.radius + r;
                    let d2 = off.dot(off);
                    if d2 < min * min && d2 > 1e-4 {
                        let d = d2.sqrt();
                        push += off * ((min - d) / d);
                    }
                }
            }
            e.pos += vel * dt + push * 0.5;
            // Contact with buildings: blocked and chewing on them.
            for b in &mut self.buildings {
                let off = e.pos - b.pos;
                let min = BUILDING_R + st.radius;
                if off.len() < min {
                    e.pos = b.pos + off.norm() * min;
                }
            }
            let touching = e.pos.dist(target) <= tr + st.radius + 3.0;
            if touching && e.atk_cd <= 0.0 && e.kind != EnemyKind::Gunner {
                e.atk_cd = 0.8;
                match tb {
                    Some(bi) => {
                        self.buildings[bi].hp -= st.dmg;
                        self.buildings[bi].flash = 0.15;
                    }
                    None => {
                        self.cmd.hp -= st.dmg;
                        self.cmd.hurt = 0.2;
                        self.events.push(Event::CommanderHit);
                    }
                }
            }
            // Ranged attacks.
            if e.atk_cd <= 0.0 && dist < 420.0 {
                match e.kind {
                    EnemyKind::Gunner => {
                        e.atk_cd = 2.2;
                        self.bullets.push(Bullet {
                            pos: e.pos,
                            vel: to.norm() * 260.0,
                            dmg: st.dmg,
                            ttl: 2.5,
                            big: false,
                        });
                    }
                    EnemyKind::Boss | EnemyKind::Elite => {
                        e.atk_cd = if e.kind == EnemyKind::Boss { 2.0 } else { 3.5 };
                        let n = if e.kind == EnemyKind::Boss { 16 } else { 8 };
                        let a0 = to.angle() + self.time;
                        for k in 0..n {
                            let a = a0 + k as f32 / n as f32 * std::f32::consts::TAU;
                            self.bullets.push(Bullet {
                                pos: e.pos,
                                vel: V2::from_angle(a) * 210.0,
                                dmg: 18.0,
                                ttl: 3.0,
                                big: true,
                            });
                        }
                    }
                    _ => {}
                }
            }
            // Despawn stragglers far behind (except bosses).
            if !e.kind.is_big() && e.pos.dist(cpos) > 1500.0 {
                e.hp = -1.0;
                e.max_hp = 0.0; // marks a silent despawn
            }
        }
    }

    fn nearest_enemies(&self, from: V2, range: f32, n: usize) -> Vec<usize> {
        let mut v: Vec<(f32, usize)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0.0)
            .map(|(i, e)| (e.pos.dist2(from), i))
            .filter(|(d, i)| {
                let r = range + self.enemies[*i].kind.stats().radius;
                *d <= r * r
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().take(n).map(|x| x.1).collect()
    }

    fn damage(&mut self, i: usize, dmg: f32) {
        let e = &mut self.enemies[i];
        e.hp -= dmg;
        e.flash = 0.08;
        let pos = e.pos;
        self.events.push(Event::Hit { pos });
    }

    fn commander_fire(&mut self, _dt: f32) {
        if self.cmd.fire_cd > 0.0 {
            return;
        }
        let targets = self.nearest_enemies(self.cmd.pos, self.cmd.range, self.cmd.targets);
        if targets.is_empty() {
            return;
        }
        self.cmd.fire_cd = self.cmd.fire_every;
        for i in targets {
            let b = self.enemies[i].pos;
            self.beams.push(Beam {
                a: self.cmd.pos,
                b,
                ttl: 0.08,
                kind: BeamKind::Commander,
            });
            self.damage(i, self.cmd.laser_dmg);
        }
        self.events.push(Event::Shot { pos: self.cmd.pos });
    }

    fn towers(&mut self, dt: f32) {
        for bi in 0..self.buildings.len() {
            let b = &mut self.buildings[bi];
            b.flash = (b.flash - dt).max(0.0);
            let Some((range, dmg, every)) = b.kind.weapon() else {
                continue;
            };
            b.cd -= dt;
            if b.cd > 0.0 {
                continue;
            }
            let (pos, kind) = (b.pos, b.kind);
            let t = self.nearest_enemies(pos, range * self.tower_range, 1);
            let Some(&i) = t.first() else {
                continue;
            };
            self.buildings[bi].cd = every * self.tower_rate;
            let heavy = kind == BuildKind::HeavyLaser;
            self.beams.push(Beam {
                a: pos,
                b: self.enemies[i].pos,
                ttl: if heavy { 0.15 } else { 0.07 },
                kind: if heavy {
                    BeamKind::Heavy
                } else {
                    BeamKind::Tower
                },
            });
            self.damage(i, dmg * self.tower_dmg);
            self.events.push(Event::TowerShot { pos, heavy });
        }
    }

    fn nanos(&mut self, dt: f32) {
        const R: f32 = 230.0;
        let nanos: Vec<V2> = self
            .buildings
            .iter()
            .filter(|b| b.kind == BuildKind::Nano)
            .map(|b| b.pos)
            .collect();
        for n in nanos {
            let mut beam_to = None;
            if self.cmd.pos.dist(n) < R && self.cmd.hp < self.cmd.max_hp {
                self.cmd.hp = (self.cmd.hp + 8.0 * dt).min(self.cmd.max_hp);
                beam_to = Some(self.cmd.pos);
            }
            if let Some(b) = self
                .buildings
                .iter_mut()
                .filter(|b| b.pos.dist(n) < R && b.hp < b.max_hp)
                .min_by(|a, b| (a.hp / a.max_hp).total_cmp(&(b.hp / b.max_hp)))
            {
                b.hp = (b.hp + 12.0 * dt).min(b.max_hp);
                beam_to = beam_to.or(Some(b.pos));
            }
            // Nanos also reclaim wrecks in reach.
            if let Some(w) = self
                .wrecks
                .iter_mut()
                .filter(|w| w.pos.dist(n) < R && w.metal > 0.0)
                .min_by(|a, b| a.pos.dist2(n).total_cmp(&b.pos.dist2(n)))
            {
                let take = (12.0 * dt).min(w.metal);
                w.metal -= take;
                self.metal = (self.metal + take).min(STORAGE);
                self.stats.metal_reclaimed += take;
                beam_to = beam_to.or(Some(w.pos));
            }
            if let Some(b) = beam_to {
                self.beams.push(Beam {
                    a: n,
                    b,
                    ttl: dt * 1.5,
                    kind: BeamKind::Nano,
                });
            }
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let cpos = self.cmd.pos;
        for b in &mut self.bullets {
            b.pos += b.vel * dt;
            b.ttl -= dt;
            if b.pos.dist(cpos) < COMMANDER_R + 4.0 {
                self.cmd.hp -= b.dmg;
                self.cmd.hurt = 0.2;
                b.ttl = 0.0;
                self.events.push(Event::CommanderHit);
                continue;
            }
            for bl in &mut self.buildings {
                if b.pos.dist(bl.pos) < BUILDING_R + 4.0 {
                    bl.hp -= b.dmg;
                    bl.flash = 0.15;
                    b.ttl = 0.0;
                    break;
                }
            }
        }
        self.bullets.retain(|b| b.ttl > 0.0);
    }

    fn reclaim(&mut self, dt: f32) {
        let c = self.cmd.pos;
        let r = self.cmd.reclaim_r;
        let mut budget = self.cmd.reclaim_rate * dt;
        // Drain the nearest wrecks first, up to two beams.
        let mut idx: Vec<usize> = (0..self.wrecks.len())
            .filter(|&i| self.wrecks[i].pos.dist(c) <= r)
            .collect();
        idx.sort_by(|&a, &b| {
            self.wrecks[a]
                .pos
                .dist2(c)
                .total_cmp(&self.wrecks[b].pos.dist2(c))
        });
        for i in idx.into_iter().take(2) {
            if budget <= 0.0 {
                break;
            }
            let w = &mut self.wrecks[i];
            let take = budget.min(w.metal);
            w.metal -= take;
            budget -= take;
            self.metal = (self.metal + take).min(STORAGE);
            self.stats.metal_reclaimed += take;
            self.beams.push(Beam {
                a: c,
                b: w.pos,
                ttl: dt * 1.5,
                kind: BeamKind::Reclaim,
            });
            if w.metal <= 0.0 {
                let pos = w.pos;
                self.events.push(Event::Reclaimed { pos });
            }
        }
        for w in &mut self.wrecks {
            w.age += dt;
        }
        self.wrecks.retain(|w| w.metal > 0.0 && w.age < WRECK_LIFE);
    }

    fn reap(&mut self) {
        let mut xp = 0;
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let e = self.enemies.swap_remove(i);
            if e.max_hp == 0.0 {
                continue; // despawned, not killed
            }
            let st = e.kind.stats();
            xp += st.xp;
            self.stats.kills += 1;
            if st.metal > 0.0 {
                self.wrecks.push(Wreck {
                    pos: e.pos,
                    metal: st.metal,
                    age: 0.0,
                    big: e.kind.is_big() || e.kind == EnemyKind::Brute,
                });
            }
            self.events.push(Event::Kill {
                pos: e.pos,
                big: e.kind.is_big() || e.kind == EnemyKind::Brute,
            });
        }
        let before = self.buildings.len();
        let mut lost = Vec::new();
        self.buildings.retain(|b| {
            if b.hp <= 0.0 {
                lost.push(b.pos);
                false
            } else {
                true
            }
        });
        debug_assert_eq!(before, self.buildings.len() + lost.len());
        for pos in lost {
            self.events.push(Event::BuildingLost { pos });
        }
        if xp > 0 {
            self.gain_xp(xp);
        }
    }

    /// Takes the events produced since the last call.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn placing_costs_resources_and_respects_range() {
        let mut g = Game::new(1);
        let (m, e) = g.cost(BuildKind::LaserTower);
        let (m0, e0) = (g.metal, g.energy);
        assert!(g.place(BuildKind::LaserTower, V2::new(80.0, 0.0)));
        assert_eq!(g.metal, m0 - m);
        assert_eq!(g.energy, e0 - e);
        // Too far, overlapping, and on top of the commander all fail.
        assert!(!g.place(BuildKind::Solar, V2::new(BUILD_RANGE + 10.0, 0.0)));
        assert!(!g.place(BuildKind::Solar, V2::new(85.0, 0.0)));
        assert!(!g.place(BuildKind::Solar, V2::new(5.0, 0.0)));
        assert_eq!(g.buildings.len(), 1);
    }

    #[test]
    fn cannot_build_without_resources() {
        let mut g = Game::new(1);
        g.metal = 0.0;
        assert!(!g.place(BuildKind::Solar, V2::new(60.0, 0.0)));
        assert!(g.drain_events().contains(&Event::BuildFailed));
    }

    #[test]
    fn solar_and_converter_drive_economy() {
        let mut g = Game::new(1);
        let base = g.energy_rate();
        g.place(BuildKind::Solar, V2::new(60.0, 0.0));
        assert!(g.energy_rate() > base);
        g.metal = 0.0;
        g.energy = 100.0;
        g.cost_mult = 0.0;
        g.place(BuildKind::Converter, V2::new(-60.0, 0.0));
        g.economy(1.0);
        assert!(g.metal > g.metal_income, "converter adds metal");
    }

    #[test]
    fn dgun_obliterates_a_line_and_goes_on_cooldown() {
        let mut g = Game::new(2);
        for x in [100.0, 250.0, 400.0] {
            g.spawn(EnemyKind::Brute, V2::new(x, 0.0));
        }
        g.spawn(EnemyKind::Brute, V2::new(0.0, 300.0));
        let inp = Input {
            dgun: true,
            aim: V2::new(500.0, 0.0),
            ..idle()
        };
        g.step(0.016, &inp);
        assert_eq!(g.stats.kills, 3);
        assert_eq!(g.enemies.len(), 1);
        assert!(!g.dgun_ready());
        g.step(0.016, &inp);
        assert_eq!(g.enemies.len(), 1, "cooldown blocks a second shot");
    }

    #[test]
    fn kills_leave_wrecks_that_reclaim_into_metal() {
        let mut g = Game::new(3);
        g.spawn(EnemyKind::Grunt, V2::new(60.0, 0.0));
        g.enemies[0].hp = 0.0;
        g.reap();
        assert_eq!(g.wrecks.len(), 1);
        let m0 = g.metal;
        for _ in 0..60 {
            g.reclaim(0.05);
        }
        assert!(g.wrecks.is_empty());
        assert!((g.metal - m0 - EnemyKind::Grunt.stats().metal).abs() < 0.01);
    }

    #[test]
    fn enough_xp_triggers_a_level_up_choice() {
        let mut g = Game::new(4);
        g.gain_xp(xp_to_next(1));
        assert_eq!(g.level, 2);
        g.step(0.01, &idle());
        let Phase::LevelUp(opts) = g.phase.clone() else {
            panic!("expected level-up");
        };
        assert!(!opts[0].for_buildings());
        assert!(opts[1].for_buildings());
        assert!(opts[2] != opts[0] && opts[2] != opts[1]);
        let t = g.time;
        g.step(1.0, &idle());
        assert_eq!(g.time, t, "paused while choosing");
        g.choose(0);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn upgrades_change_stats() {
        let mut g = Game::new(5);
        let d = g.cmd.laser_dmg;
        g.apply(Upgrade::LaserDamage);
        assert!(g.cmd.laser_dmg > d);
        g.place(BuildKind::Solar, V2::new(60.0, 0.0));
        let hp = g.buildings[0].max_hp;
        g.apply(Upgrade::Fortify);
        assert!(g.buildings[0].max_hp > hp);
        let c = g.cost(BuildKind::HeavyLaser).0;
        g.apply(Upgrade::CheapBuild);
        assert!(g.cost(BuildKind::HeavyLaser).0 < c);
    }

    #[test]
    fn waves_escalate_over_time() {
        assert!(spawn_rate(600.0) > spawn_rate(60.0) * 3.0);
        assert!(hp_scale(1200.0) > 4.0);
    }

    #[test]
    fn commander_auto_fires_and_kills() {
        let mut g = Game::new(6);
        g.spawn(EnemyKind::Tick, V2::new(150.0, 0.0));
        g.enemies[0].atk_cd = 99.0;
        for _ in 0..30 {
            g.step(0.02, &idle());
        }
        assert!(g.stats.kills >= 1);
    }

    #[test]
    fn dying_loses_the_game() {
        let mut g = Game::new(7);
        g.cmd.hp = 1.0;
        g.cmd.regen = 0.0;
        g.spawn(EnemyKind::Brute, V2::new(30.0, 0.0));
        for _ in 0..20 {
            g.step(0.02, &idle());
        }
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.drain_events().contains(&Event::Lost));
    }

    #[test]
    fn boss_arrives_at_twenty_minutes_and_killing_it_wins() {
        let mut g = Game::new(8);
        g.time = BOSS_TIME - 0.01;
        g.cmd.hp = 1e9;
        g.cmd.max_hp = 1e9;
        g.step(0.02, &idle());
        assert!(g.boss_spawned);
        assert!(g.drain_events().contains(&Event::BossSpawn));
        for e in &mut g.enemies {
            e.hp = 0.0;
        }
        g.step(0.02, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn enemies_attack_nearby_buildings() {
        let mut g = Game::new(9);
        g.place(BuildKind::Solar, V2::new(200.0, 0.0));
        g.spawn(EnemyKind::Brute, V2::new(240.0, 0.0));
        let hp = g.buildings[0].hp;
        for _ in 0..20 {
            g.move_enemies(0.05);
        }
        assert!(g.buildings[0].hp < hp);
    }
}
