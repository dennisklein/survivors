//! Raptor Nest game rules: build phases, raptor waves, towers, nests,
//! loot and Commander level-ups.

use crate::math::V2;
use crate::rng::Rng;

pub const WORLD: f32 = 2400.0;
pub const CORE_POS: V2 = V2::new(WORLD / 2.0, WORLD / 2.0);
pub const BUILD_TIME: f32 = 60.0;
pub const NEST_COUNT: usize = 6;
const CORE_HP: f32 = 2000.0;
const CORE_BUILD_RADIUS: f32 = 320.0;
const STRUCT_BUILD_RADIUS: f32 = 190.0;
const MIN_SPACING: f32 = 44.0;
const NEST_KEEPOUT: f32 = 130.0;
const NEST_HP: f32 = 1400.0;
const NEST_RADIUS: f32 = 46.0;
const PICKUP_RADIUS: f32 = 18.0;
const AGGRO_RANGE: f32 = 280.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerKind {
    Llt,
    Hlt,
    Rocket,
    Wall,
    Nano,
}

impl TowerKind {
    pub const ALL: [TowerKind; 5] = [
        TowerKind::Llt,
        TowerKind::Hlt,
        TowerKind::Rocket,
        TowerKind::Wall,
        TowerKind::Nano,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TowerKind::Llt => "Light Laser",
            TowerKind::Hlt => "Heavy Laser",
            TowerKind::Rocket => "Rocket Battery",
            TowerKind::Wall => "Dragon's Teeth",
            TowerKind::Nano => "Nano Tower",
        }
    }

    pub fn cost(self) -> u32 {
        match self {
            TowerKind::Llt => 60,
            TowerKind::Hlt => 150,
            TowerKind::Rocket => 220,
            TowerKind::Wall => 20,
            TowerKind::Nano => 120,
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            TowerKind::Llt => 320.0,
            TowerKind::Hlt => 750.0,
            TowerKind::Rocket => 420.0,
            TowerKind::Wall => 700.0,
            TowerKind::Nano => 300.0,
        }
    }

    /// Weapon range (heal range for nano towers; zero for walls).
    pub fn range(self) -> f32 {
        match self {
            TowerKind::Llt => 240.0,
            TowerKind::Hlt => 310.0,
            TowerKind::Rocket => 470.0,
            TowerKind::Wall => 0.0,
            TowerKind::Nano => 210.0,
        }
    }

    pub fn damage(self) -> f32 {
        match self {
            TowerKind::Llt => 13.0,
            TowerKind::Hlt => 50.0,
            TowerKind::Rocket => 34.0,
            TowerKind::Wall => 0.0,
            TowerKind::Nano => 18.0,
        }
    }

    pub fn cooldown(self) -> f32 {
        match self {
            TowerKind::Llt => 0.4,
            TowerKind::Hlt => 1.1,
            TowerKind::Rocket => 1.6,
            TowerKind::Wall | TowerKind::Nano => 1.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            TowerKind::Wall => 16.0,
            TowerKind::Hlt => 22.0,
            _ => 19.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaptorKind {
    Swarmer,
    Brute,
    Spitter,
    Matriarch,
}

impl RaptorKind {
    /// (hp, speed, melee dps, radius, loot value)
    fn stats(self) -> (f32, f32, f32, f32, u32) {
        match self {
            RaptorKind::Swarmer => (28.0, 125.0, 22.0, 10.0, 3),
            RaptorKind::Brute => (160.0, 70.0, 60.0, 17.0, 10),
            RaptorKind::Spitter => (55.0, 85.0, 10.0, 12.0, 6),
            RaptorKind::Matriarch => (1500.0, 55.0, 140.0, 30.0, 80),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tower {
    pub kind: TowerKind,
    pub pos: V2,
    pub hp: f32,
    pub cd: f32,
    /// Seconds since the last shot, for muzzle flashes.
    pub fired: f32,
}

#[derive(Clone, Debug)]
pub struct Raptor {
    pub kind: RaptorKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub dps: f32,
    pub radius: f32,
    pub cd: f32,
    pub flash: f32,
    pub facing: f32,
}

#[derive(Clone, Debug)]
pub struct Nest {
    pub pos: V2,
    pub hp: f32,
    pub alive: bool,
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub ttl: f32,
    /// Splash radius applied when the shot ends (rockets).
    pub splash: f32,
    pub hostile: bool,
    pub pierce: u32,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub pos: V2,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Build { timer: f32 },
    Wave,
    LevelUp,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Multishot,
    Speed,
    Armor,
    TowerPower,
    Magnet,
    Pierce,
}

impl Upgrade {
    pub const ALL: [Upgrade; 8] = [
        Upgrade::Damage,
        Upgrade::FireRate,
        Upgrade::Multishot,
        Upgrade::Speed,
        Upgrade::Armor,
        Upgrade::TowerPower,
        Upgrade::Magnet,
        Upgrade::Pierce,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Upgrade::Damage => "Plasma Core: +25% Commander damage",
            Upgrade::FireRate => "Autoloader: +20% fire rate",
            Upgrade::Multishot => "Split Barrel: +1 projectile",
            Upgrade::Speed => "Servo Legs: +12% move speed",
            Upgrade::Armor => "Armor Plating: +100 max HP, full heal",
            Upgrade::TowerPower => "Overclock: +20% tower damage",
            Upgrade::Magnet => "Tractor Beam: +60% pickup range",
            Upgrade::Pierce => "Railgun Rounds: shots pierce +1",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot,
    Beam { from: V2, to: V2, heavy: bool },
    RocketLaunch,
    Spit,
    Hit(V2),
    Kill { pos: V2, big: bool },
    Explosion { pos: V2, radius: f32 },
    Pickup,
    LevelUp,
    Built,
    Denied,
    StructureLost(V2),
    NestDestroyed(V2),
    WaveStart(u32),
    WaveCleared(u32),
    CommanderHurt,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Desired move direction (any length; normalized internally).
    pub mv: V2,
    /// Ends the build phase early.
    pub ready: bool,
}

#[derive(Clone, Debug)]
pub struct Commander {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub cooldown: f32,
    pub cd: f32,
    pub shots: u32,
    pub pierce: u32,
    pub magnet: f32,
    pub facing: f32,
    pub hurt: f32,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub phase: Phase,
    pub wave: u32,
    pub metal: u32,
    pub xp: u32,
    pub level: u32,
    pub cmd: Commander,
    pub core_hp: f32,
    pub towers: Vec<Tower>,
    pub raptors: Vec<Raptor>,
    pub nests: Vec<Nest>,
    pub shots: Vec<Shot>,
    pub pickups: Vec<Pickup>,
    pub choices: Vec<Upgrade>,
    pub tower_mult: f32,
    pub kills: u32,
    pub time: f32,
    pub events: Vec<Event>,
    /// Raptors still to spawn this wave.
    pub to_spawn: u32,
    spawn_timer: f32,
    pending_levels: u32,
    rng: Rng,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let offset = rng.range(0.0, std::f32::consts::TAU);
        let nests = (0..NEST_COUNT)
            .map(|i| {
                let a = offset + i as f32 / NEST_COUNT as f32 * std::f32::consts::TAU;
                let r = rng.range(640.0, 980.0);
                Nest {
                    pos: CORE_POS + V2::from_angle(a) * r,
                    hp: NEST_HP,
                    alive: true,
                }
            })
            .collect();
        let mut g = Game {
            phase: Phase::Build { timer: BUILD_TIME },
            wave: 0,
            metal: 240,
            xp: 0,
            level: 1,
            cmd: Commander {
                pos: CORE_POS + V2::new(0.0, 70.0),
                hp: 300.0,
                max_hp: 300.0,
                speed: 210.0,
                dmg: 14.0,
                cooldown: 0.28,
                cd: 0.0,
                shots: 1,
                pierce: 0,
                magnet: 90.0,
                facing: 0.0,
                hurt: 0.0,
            },
            core_hp: CORE_HP,
            towers: Vec::new(),
            raptors: Vec::new(),
            nests,
            shots: Vec::new(),
            pickups: Vec::new(),
            choices: Vec::new(),
            tower_mult: 1.0,
            kills: 0,
            time: 0.0,
            events: Vec::new(),
            to_spawn: 0,
            spawn_timer: 0.0,
            pending_levels: 0,
            rng,
        };
        // Free starter defence next to the core.
        g.towers
            .push(new_tower(TowerKind::Llt, CORE_POS + V2::new(90.0, 0.0)));
        g.towers
            .push(new_tower(TowerKind::Llt, CORE_POS + V2::new(-90.0, 0.0)));
        g
    }

    pub fn core_max_hp(&self) -> f32 {
        CORE_HP
    }

    pub fn nest_max_hp(&self) -> f32 {
        NEST_HP
    }

    pub fn nests_alive(&self) -> usize {
        self.nests.iter().filter(|n| n.alive).count()
    }

    pub fn xp_needed(&self) -> u32 {
        15 + self.level * 12
    }

    /// Why `pos` cannot hold a new `kind`, or `None` when it can.
    pub fn placement_error(&self, kind: TowerKind, pos: V2) -> Option<&'static str> {
        if !matches!(self.phase, Phase::Build { .. }) {
            return Some("Build only between waves");
        }
        if self.metal < kind.cost() {
            return Some("Not enough metal");
        }
        let m = 40.0;
        if pos.x < m || pos.y < m || pos.x > WORLD - m || pos.y > WORLD - m {
            return Some("Out of bounds");
        }
        if pos.dist(CORE_POS) < 60.0 || self.towers.iter().any(|t| t.pos.dist(pos) < MIN_SPACING) {
            return Some("Too close to another structure");
        }
        if self
            .nests
            .iter()
            .any(|n| n.alive && n.pos.dist(pos) < NEST_KEEPOUT)
        {
            return Some("Too close to a nest");
        }
        if !self.in_build_range(pos) {
            return Some("Outside build range");
        }
        None
    }

    /// Structures project a build radius: towers push the frontier outward.
    pub fn in_build_range(&self, pos: V2) -> bool {
        pos.dist(CORE_POS) <= CORE_BUILD_RADIUS
            || self
                .towers
                .iter()
                .any(|t| t.pos.dist(pos) <= build_radius(t.kind))
    }

    pub fn build(&mut self, kind: TowerKind, pos: V2) -> bool {
        if self.placement_error(kind, pos).is_some() {
            self.events.push(Event::Denied);
            return false;
        }
        self.metal -= kind.cost();
        self.towers.push(new_tower(kind, pos));
        self.events.push(Event::Built);
        true
    }

    pub fn choose_upgrade(&mut self, i: usize) {
        if self.phase != Phase::LevelUp {
            return;
        }
        let Some(&u) = self.choices.get(i) else {
            return;
        };
        let c = &mut self.cmd;
        match u {
            Upgrade::Damage => c.dmg *= 1.25,
            Upgrade::FireRate => c.cooldown /= 1.2,
            Upgrade::Multishot => c.shots += 1,
            Upgrade::Speed => c.speed *= 1.12,
            Upgrade::Armor => {
                c.max_hp += 100.0;
                c.hp = c.max_hp;
            }
            Upgrade::TowerPower => self.tower_mult *= 1.2,
            Upgrade::Magnet => c.magnet *= 1.6,
            Upgrade::Pierce => c.pierce += 1,
        }
        self.choices.clear();
        self.pending_levels -= 1;
        if self.pending_levels > 0 {
            self.roll_choices();
        } else if self.after_levelup_phase() {
            self.phase = Phase::Build { timer: BUILD_TIME };
        } else {
            self.phase = Phase::Wave;
        }
    }

    fn roll_choices(&mut self) {
        let mut pool = Upgrade::ALL.to_vec();
        self.choices.clear();
        for _ in 0..3 {
            let i = self.rng.below(pool.len());
            self.choices.push(pool.swap_remove(i));
        }
    }

    pub fn start_wave(&mut self) {
        self.wave += 1;
        let n = self.wave;
        let per_nest = 3 + n * 2 + n * n / 3;
        self.to_spawn = per_nest * self.nests_alive() as u32;
        self.spawn_timer = 0.0;
        self.phase = Phase::Wave;
        self.events.push(Event::WaveStart(n));
        if n.is_multiple_of(4) {
            let nests: Vec<V2> = self
                .nests
                .iter()
                .filter(|n| n.alive)
                .map(|n| n.pos)
                .collect();
            if !nests.is_empty() {
                let p = nests[self.rng.below(nests.len())];
                self.spawn_raptor(RaptorKind::Matriarch, p);
            }
        }
    }

    fn spawn_raptor(&mut self, kind: RaptorKind, at: V2) {
        let (hp, speed, dps, radius, _) = kind.stats();
        let scale = 1.0 + 0.22 * self.wave.saturating_sub(1) as f32;
        let jitter = V2::from_angle(self.rng.range(0.0, std::f32::consts::TAU))
            * self.rng.range(0.0, NEST_RADIUS);
        self.raptors.push(Raptor {
            kind,
            pos: at + jitter,
            hp: hp * scale,
            max_hp: hp * scale,
            speed: speed * self.rng.range(0.9, 1.1) * (1.0 + 0.02 * self.wave as f32),
            dps: dps * (1.0 + 0.08 * self.wave as f32),
            radius,
            cd: self.rng.range(0.5, 2.0),
            flash: 0.0,
            facing: 0.0,
        });
    }

    fn pick_kind(&mut self) -> RaptorKind {
        let roll = self.rng.f32();
        let w = self.wave;
        if w >= 3 && roll < 0.14 + 0.01 * w as f32 {
            RaptorKind::Brute
        } else if w >= 2 && roll > 0.82 - 0.01 * w as f32 {
            RaptorKind::Spitter
        } else {
            RaptorKind::Swarmer
        }
    }

    pub fn update(&mut self, dt: f32, input: Input) {
        match self.phase {
            Phase::Won | Phase::Lost | Phase::LevelUp => return,
            Phase::Build { timer } => {
                self.time += dt;
                self.move_commander(dt, input.mv);
                self.cmd.hp = (self.cmd.hp + self.cmd.max_hp * 0.1 * dt).min(self.cmd.max_hp);
                self.core_hp = (self.core_hp + 20.0 * dt).min(CORE_HP);
                for t in &mut self.towers {
                    t.hp = (t.hp + t.kind.max_hp() * 0.05 * dt).min(t.kind.max_hp());
                }
                let t = timer - dt;
                if t <= 0.0 || input.ready {
                    self.start_wave();
                } else {
                    self.phase = Phase::Build { timer: t };
                }
                return;
            }
            Phase::Wave => {}
        }
        self.time += dt;
        self.cmd.hurt = (self.cmd.hurt - dt).max(0.0);
        self.move_commander(dt, input.mv);
        self.spawn(dt);
        self.commander_fire(dt);
        self.towers_act(dt);
        self.move_raptors(dt);
        self.move_shots(dt);
        self.collect(dt);
        self.reap();
        self.check_end();
    }

    fn move_commander(&mut self, dt: f32, mv: V2) {
        let d = mv.norm();
        let c = &mut self.cmd;
        c.pos += d * c.speed * dt;
        c.pos.x = c.pos.x.clamp(20.0, WORLD - 20.0);
        c.pos.y = c.pos.y.clamp(20.0, WORLD - 20.0);
        if d.len() > 0.0 {
            c.facing = d.angle();
        }
    }

    fn spawn(&mut self, dt: f32) {
        if self.to_spawn == 0 {
            return;
        }
        self.spawn_timer -= dt;
        let alive: Vec<V2> = self
            .nests
            .iter()
            .filter(|n| n.alive)
            .map(|n| n.pos)
            .collect();
        if alive.is_empty() {
            self.to_spawn = 0;
            return;
        }
        while self.spawn_timer <= 0.0 && self.to_spawn > 0 {
            // Burst from every nest so pressure comes from all sides.
            for p in &alive {
                if self.to_spawn == 0 {
                    break;
                }
                let k = self.pick_kind();
                self.spawn_raptor(k, *p);
                self.to_spawn -= 1;
            }
            self.spawn_timer += (1.6 - 0.05 * self.wave as f32).max(0.6);
        }
    }

    fn nearest_raptor(&self, from: V2, range: f32) -> Option<usize> {
        let r2 = range * range;
        self.raptors
            .iter()
            .enumerate()
            .filter(|(_, r)| r.hp > 0.0)
            .map(|(i, r)| (i, r.pos.dist2(from)))
            .filter(|&(_, d)| d <= r2)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn nearest_nest(&self, from: V2, range: f32) -> Option<usize> {
        self.nests
            .iter()
            .enumerate()
            .filter(|(_, n)| n.alive && n.pos.dist(from) <= range + NEST_RADIUS)
            .min_by(|a, b| a.1.pos.dist2(from).total_cmp(&b.1.pos.dist2(from)))
            .map(|(i, _)| i)
    }

    fn commander_fire(&mut self, dt: f32) {
        self.cmd.cd -= dt;
        if self.cmd.cd > 0.0 {
            return;
        }
        let range = 340.0;
        let from = self.cmd.pos;
        let target = match self.nearest_raptor(from, range) {
            Some(i) => self.raptors[i].pos,
            None => match self.nearest_nest(from, range) {
                Some(i) => self.nests[i].pos,
                None => return,
            },
        };
        self.cmd.cd = self.cmd.cooldown;
        let base = (target - from).angle();
        let n = self.cmd.shots;
        for k in 0..n {
            let a = base + (k as f32 - (n - 1) as f32 / 2.0) * 0.12;
            self.shots.push(Shot {
                pos: from,
                vel: V2::from_angle(a) * 620.0,
                dmg: self.cmd.dmg,
                ttl: 0.65,
                splash: 0.0,
                hostile: false,
                pierce: self.cmd.pierce,
            });
        }
        self.events.push(Event::Shot);
    }

    fn towers_act(&mut self, dt: f32) {
        for ti in 0..self.towers.len() {
            let t = &mut self.towers[ti];
            t.cd -= dt;
            t.fired += dt;
            let (kind, pos) = (t.kind, t.pos);
            if kind == TowerKind::Wall || t.cd > 0.0 {
                continue;
            }
            if kind == TowerKind::Nano {
                let heal = kind.damage() * dt;
                self.core_hp = if CORE_POS.dist(pos) <= kind.range() {
                    (self.core_hp + heal).min(CORE_HP)
                } else {
                    self.core_hp
                };
                for o in &mut self.towers {
                    if o.pos.dist(pos) <= kind.range() {
                        o.hp = (o.hp + heal).min(o.kind.max_hp());
                    }
                }
                continue;
            }
            let range = kind.range();
            let target = match self.nearest_raptor(pos, range) {
                Some(i) => Some((self.raptors[i].pos, Some(i), None)),
                None => self
                    .nearest_nest(pos, range)
                    .map(|i| (self.nests[i].pos, None, Some(i))),
            };
            let Some((tpos, ri, ni)) = target else {
                continue;
            };
            let dmg = kind.damage() * self.tower_mult;
            let t = &mut self.towers[ti];
            t.cd = kind.cooldown();
            t.fired = 0.0;
            match kind {
                TowerKind::Rocket => {
                    let d = tpos - pos;
                    let speed = 420.0;
                    self.shots.push(Shot {
                        pos,
                        vel: d.norm() * speed,
                        dmg,
                        ttl: d.len() / speed,
                        splash: 70.0,
                        hostile: false,
                        pierce: 0,
                    });
                    self.events.push(Event::RocketLaunch);
                }
                _ => {
                    if let Some(i) = ri {
                        self.raptors[i].hp -= dmg;
                        self.raptors[i].flash = 0.1;
                    }
                    if let Some(i) = ni {
                        self.nests[i].hp -= dmg;
                    }
                    self.events.push(Event::Beam {
                        from: pos,
                        to: tpos,
                        heavy: kind == TowerKind::Hlt,
                    });
                }
            }
        }
    }

    fn move_raptors(&mut self, dt: f32) {
        let cmd_pos = self.cmd.pos;
        for i in 0..self.raptors.len() {
            let r = &self.raptors[i];
            if r.hp <= 0.0 {
                continue;
            }
            let pos = r.pos;
            // Target: Commander when close, otherwise nearest structure.
            let mut tgt = CORE_POS;
            let mut tgt_r = 40.0;
            let mut best = pos.dist2(CORE_POS);
            let mut tgt_tower = None;
            for (ti, t) in self.towers.iter().enumerate() {
                let d = pos.dist2(t.pos);
                if d < best {
                    best = d;
                    tgt = t.pos;
                    tgt_r = t.kind.radius();
                    tgt_tower = Some(ti);
                }
            }
            let mut tgt_cmd = false;
            if pos.dist(cmd_pos) < AGGRO_RANGE || pos.dist2(cmd_pos) < best {
                tgt = cmd_pos;
                tgt_r = 14.0;
                tgt_tower = None;
                tgt_cmd = true;
            }
            let r = &mut self.raptors[i];
            r.flash = (r.flash - dt).max(0.0);
            r.cd -= dt;
            let to = tgt - pos;
            r.facing = to.angle();
            let dist = to.len();
            let reach = r.radius + tgt_r + 4.0;
            if r.kind == RaptorKind::Spitter && dist < 230.0 {
                if r.cd <= 0.0 {
                    r.cd = 1.8;
                    let dmg = 14.0 + self.wave as f32 * 1.5;
                    self.shots.push(Shot {
                        pos,
                        vel: to.norm() * 300.0,
                        dmg,
                        ttl: 1.0,
                        splash: 0.0,
                        hostile: true,
                        pierce: 0,
                    });
                    self.events.push(Event::Spit);
                }
                if dist > 160.0 {
                    r.pos += to.norm() * r.speed * 0.4 * dt;
                }
                continue;
            }
            if dist > reach {
                r.pos += to.norm() * r.speed * dt;
                continue;
            }
            let dmg = r.dps * dt;
            if tgt_cmd {
                self.hurt_commander(dmg);
            } else if let Some(ti) = tgt_tower {
                self.towers[ti].hp -= dmg;
            } else {
                self.core_hp -= dmg;
            }
        }
        // Light separation so packs spread out.
        let n = self.raptors.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.raptors[i].pos, self.raptors[j].pos);
                let min = self.raptors[i].radius + self.raptors[j].radius;
                let d = b - a;
                let l = d.len();
                if l < min && l > 1e-3 {
                    let push = d * ((min - l) / l * 0.5);
                    self.raptors[i].pos = a - push;
                    self.raptors[j].pos = b + push;
                }
            }
        }
    }

    fn hurt_commander(&mut self, dmg: f32) {
        self.cmd.hp -= dmg;
        if self.cmd.hurt <= 0.0 {
            self.cmd.hurt = 0.3;
            self.events.push(Event::CommanderHurt);
        }
    }

    fn move_shots(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|s| {
            s.pos += s.vel * dt;
            s.ttl -= dt;
            if s.hostile {
                if s.pos.dist(self.cmd.pos) < 16.0 {
                    self.hurt_commander(s.dmg);
                    return false;
                }
                if let Some(t) = self
                    .towers
                    .iter_mut()
                    .find(|t| t.pos.dist(s.pos) < t.kind.radius())
                {
                    t.hp -= s.dmg;
                    return false;
                }
                if s.pos.dist(CORE_POS) < 40.0 {
                    self.core_hp -= s.dmg;
                    return false;
                }
                return s.ttl > 0.0;
            }
            if s.splash > 0.0 {
                if s.ttl <= 0.0 {
                    self.explode(s.pos, s.splash, s.dmg);
                    return false;
                }
                return true;
            }
            if let Some(r) = self
                .raptors
                .iter_mut()
                .find(|r| r.hp > 0.0 && r.pos.dist(s.pos) < r.radius + 4.0)
            {
                r.hp -= s.dmg;
                r.flash = 0.1;
                self.events.push(Event::Hit(s.pos));
                if s.pierce == 0 {
                    return false;
                }
                s.pierce -= 1;
                s.pos += s.vel.norm() * (r.radius * 2.0 + 2.0);
            } else if let Some(n) = self
                .nests
                .iter_mut()
                .find(|n| n.alive && n.pos.dist(s.pos) < NEST_RADIUS)
            {
                n.hp -= s.dmg;
                self.events.push(Event::Hit(s.pos));
                return false;
            }
            s.ttl > 0.0
        });
        shots.append(&mut self.shots);
        self.shots = shots;
    }

    fn explode(&mut self, pos: V2, radius: f32, dmg: f32) {
        for r in &mut self.raptors {
            if r.pos.dist(pos) < radius + r.radius {
                r.hp -= dmg;
                r.flash = 0.1;
            }
        }
        for n in &mut self.nests {
            if n.alive && n.pos.dist(pos) < radius + NEST_RADIUS {
                n.hp -= dmg;
            }
        }
        self.events.push(Event::Explosion { pos, radius });
    }

    fn collect(&mut self, dt: f32) {
        let c = self.cmd.pos;
        let magnet = self.cmd.magnet;
        let mut got = 0;
        self.pickups.retain_mut(|p| {
            let d = p.pos.dist(c);
            if d < PICKUP_RADIUS {
                got += p.value;
                return false;
            }
            if d < magnet {
                p.pos += (c - p.pos).norm() * (500.0 * dt).min(d);
            }
            true
        });
        if got > 0 {
            self.gain(got);
            self.events.push(Event::Pickup);
        }
    }

    fn gain(&mut self, value: u32) {
        self.metal += value;
        self.xp += value;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.pending_levels += 1;
            self.events.push(Event::LevelUp);
        }
    }

    fn reap(&mut self) {
        let mut dead = Vec::new();
        self.raptors.retain(|r| {
            if r.hp <= 0.0 {
                dead.push((r.pos, r.kind));
                false
            } else {
                true
            }
        });
        for (pos, kind) in dead {
            self.kills += 1;
            let big = matches!(kind, RaptorKind::Brute | RaptorKind::Matriarch);
            self.events.push(Event::Kill { pos, big });
            self.pickups.push(Pickup {
                pos,
                value: kind.stats().4,
            });
        }
        let mut lost = Vec::new();
        self.towers.retain(|t| {
            if t.hp <= 0.0 {
                lost.push(t.pos);
                false
            } else {
                true
            }
        });
        for p in lost {
            self.events.push(Event::StructureLost(p));
        }
        for i in 0..self.nests.len() {
            let n = &mut self.nests[i];
            if n.alive && n.hp <= 0.0 {
                n.alive = false;
                let p = n.pos;
                self.events.push(Event::NestDestroyed(p));
                for k in 0..10 {
                    let off = V2::from_angle(k as f32 * 0.628) * 30.0;
                    self.pickups.push(Pickup {
                        pos: p + off,
                        value: 15,
                    });
                }
            }
        }
    }

    fn check_end(&mut self) {
        if self.core_hp <= 0.0 || self.cmd.hp <= 0.0 {
            self.core_hp = self.core_hp.max(0.0);
            self.cmd.hp = self.cmd.hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
            return;
        }
        if self.to_spawn == 0 && self.raptors.is_empty() {
            // Loot: everything left on the field is swept up.
            let rest: u32 = self.pickups.drain(..).map(|p| p.value).sum();
            let bonus = 40 + 15 * self.wave;
            self.gain(rest + bonus);
            self.events.push(Event::WaveCleared(self.wave));
            if self.nests_alive() == 0 {
                self.phase = Phase::Won;
                self.events.push(Event::Won);
            } else if self.pending_levels > 0 {
                self.phase = Phase::LevelUp;
                self.roll_choices();
            } else {
                self.phase = Phase::Build { timer: BUILD_TIME };
            }
        } else if self.pending_levels > 0 {
            self.phase = Phase::LevelUp;
            self.roll_choices();
        }
    }

    /// After level-ups granted by wave-clear loot, resume into the build phase.
    pub fn after_levelup_phase(&self) -> bool {
        self.to_spawn == 0 && self.raptors.is_empty()
    }
}

pub fn build_radius(kind: TowerKind) -> f32 {
    match kind {
        TowerKind::Nano => STRUCT_BUILD_RADIUS * 1.4,
        TowerKind::Wall => STRUCT_BUILD_RADIUS * 0.5,
        _ => STRUCT_BUILD_RADIUS,
    }
}

fn new_tower(kind: TowerKind, pos: V2) -> Tower {
    Tower {
        kind,
        pos,
        hp: kind.max_hp(),
        cd: 0.0,
        fired: 10.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Game, secs: f32) {
        let steps = (secs / 0.02) as usize;
        for _ in 0..steps {
            g.update(0.02, Input::default());
        }
    }

    #[test]
    fn starts_in_build_phase_with_nests() {
        let g = Game::new(1);
        assert!(matches!(g.phase, Phase::Build { .. }));
        assert_eq!(g.nests_alive(), NEST_COUNT);
        assert_eq!(g.towers.len(), 2);
    }

    #[test]
    fn build_spends_metal_and_respects_range() {
        let mut g = Game::new(1);
        let m = g.metal;
        assert!(g.build(TowerKind::Llt, CORE_POS + V2::new(0.0, -150.0)));
        assert_eq!(g.metal, m - 60);
        assert!(!g.build(TowerKind::Llt, CORE_POS + V2::new(0.0, -150.0)));
        assert_eq!(
            g.placement_error(TowerKind::Wall, CORE_POS + V2::new(0.0, -900.0)),
            Some("Outside build range")
        );
        // Towers extend the frontier.
        assert!(g.in_build_range(CORE_POS + V2::new(0.0, -320.0)));
    }

    #[test]
    fn cannot_build_without_metal_or_during_wave() {
        let mut g = Game::new(2);
        g.metal = 10;
        assert_eq!(
            g.placement_error(TowerKind::Llt, CORE_POS + V2::new(150.0, 150.0)),
            Some("Not enough metal")
        );
        g.metal = 1000;
        g.update(
            0.1,
            Input {
                ready: true,
                ..Default::default()
            },
        );
        assert_eq!(g.phase, Phase::Wave);
        assert!(!g.build(TowerKind::Llt, CORE_POS + V2::new(150.0, 150.0)));
    }

    #[test]
    fn build_timer_starts_wave() {
        let mut g = Game::new(3);
        run(&mut g, BUILD_TIME + 0.1);
        assert_eq!(g.wave, 1);
        assert!(g.to_spawn > 0 || !g.raptors.is_empty());
        assert!(g.events.contains(&Event::WaveStart(1)));
    }

    #[test]
    fn waves_escalate() {
        let mut g = Game::new(4);
        g.start_wave();
        let first = g.to_spawn;
        g.to_spawn = 0;
        g.start_wave();
        assert!(g.to_spawn > first);
    }

    #[test]
    fn killing_raptors_drops_loot_and_levels_up() {
        let mut g = Game::new(5);
        g.start_wave();
        g.to_spawn = 0;
        g.raptors.clear();
        g.spawn_raptor(RaptorKind::Brute, g.cmd.pos + V2::new(400.0, 0.0));
        g.raptors[0].hp = 0.0;
        g.reap();
        assert_eq!(g.kills, 1);
        assert_eq!(g.pickups.len(), 1);
        let need = g.xp_needed();
        g.gain(need);
        assert_eq!(g.level, 2);
        g.check_end();
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        let dmg = g.cmd.dmg;
        let hp = g.cmd.max_hp;
        let rate = g.cmd.cooldown;
        let shots = g.cmd.shots;
        g.choose_upgrade(0);
        assert!(
            g.cmd.dmg > dmg
                || g.cmd.max_hp > hp
                || g.cmd.cooldown < rate
                || g.cmd.shots > shots
                || g.tower_mult > 1.0
                || g.cmd.speed > 210.0
                || g.cmd.magnet > 90.0
                || g.cmd.pierce > 0
        );
        while g.phase == Phase::LevelUp {
            g.choose_upgrade(1);
        }
        assert!(matches!(g.phase, Phase::Build { .. }));
    }

    #[test]
    fn wave_clear_returns_to_build_with_bonus() {
        let mut g = Game::new(6);
        g.start_wave();
        g.to_spawn = 0;
        g.raptors.clear();
        let m = g.metal;
        g.update(0.02, Input::default());
        while g.phase == Phase::LevelUp {
            g.choose_upgrade(0);
        }
        assert!(matches!(g.phase, Phase::Build { .. }));
        assert!(g.metal > m);
    }

    #[test]
    fn destroying_all_nests_wins() {
        let mut g = Game::new(7);
        g.start_wave();
        g.to_spawn = 0;
        g.raptors.clear();
        for n in &mut g.nests {
            n.hp = 0.0;
        }
        g.update(0.02, Input::default());
        while g.phase == Phase::LevelUp {
            g.choose_upgrade(0);
        }
        assert_eq!(g.nests_alive(), 0);
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn core_destroyed_loses() {
        let mut g = Game::new(8);
        g.start_wave();
        g.core_hp = 0.0;
        g.update(0.02, Input::default());
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.events.contains(&Event::Lost));
    }

    #[test]
    fn raptors_attack_and_towers_shoot() {
        let mut g = Game::new(9);
        g.start_wave();
        g.to_spawn = 0;
        g.spawn_raptor(RaptorKind::Swarmer, CORE_POS + V2::new(200.0, 0.0));
        let before = g.raptors[0].hp;
        g.update(0.05, Input::default());
        assert!(g.raptors.is_empty() || g.raptors[0].hp < before);
    }

    #[test]
    fn towers_damage_nests_in_range() {
        let mut g = Game::new(10);
        let np = g.nests[0].pos;
        g.towers
            .push(new_tower(TowerKind::Hlt, np + V2::new(200.0, 0.0)));
        g.start_wave();
        g.to_spawn = 0;
        g.raptors.clear();
        g.cmd.pos = CORE_POS;
        g.towers_act(0.02);
        assert!(g.nests[0].hp < NEST_HP);
    }

    /// A passive player (no building, standing at the core) must survive the
    /// opening but eventually be overrun, so the waves actually escalate.
    #[test]
    fn passive_play_survives_early_and_falls_later() {
        let mut g = Game::new(11);
        let mut steps = 0;
        while !matches!(g.phase, Phase::Lost | Phase::Won) && steps < 200_000 {
            if g.phase == Phase::LevelUp {
                g.choose_upgrade(0);
            }
            let ready = matches!(g.phase, Phase::Build { .. });
            g.update(
                0.02,
                Input {
                    mv: V2::ZERO,
                    ready,
                },
            );
            g.events.clear();
            steps += 1;
        }
        println!("passive run ended at wave {} level {}", g.wave, g.level);
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.wave >= 2, "lost too early: wave {}", g.wave);
        assert!(g.wave <= 12, "never threatened: wave {}", g.wave);
    }

    /// A player that keeps ringing the core with towers outlasts a passive one.
    #[test]
    fn building_towers_helps() {
        let mut g = Game::new(11);
        let mut steps = 0;
        let mut ring = 0u32;
        while !matches!(g.phase, Phase::Lost | Phase::Won) && steps < 400_000 && g.wave < 10 {
            if g.phase == Phase::LevelUp {
                g.choose_upgrade(0);
            }
            let building = matches!(g.phase, Phase::Build { .. });
            if building {
                for _ in 0..40 {
                    let kind = if ring % 3 == 2 {
                        TowerKind::Hlt
                    } else {
                        TowerKind::Llt
                    };
                    if g.metal < kind.cost() {
                        break;
                    }
                    let a = ring as f32 * 0.7;
                    let r = 120.0 + (ring / 9) as f32 * 60.0;
                    ring += 1;
                    g.build(kind, CORE_POS + V2::from_angle(a) * r);
                }
            }
            g.update(
                0.02,
                Input {
                    mv: V2::ZERO,
                    ready: building,
                },
            );
            g.events.clear();
            steps += 1;
        }
        println!(
            "builder run ended at wave {} towers {}",
            g.wave,
            g.towers.len()
        );
        assert!(g.wave >= 4, "builder lost at wave {}", g.wave);
    }
}
