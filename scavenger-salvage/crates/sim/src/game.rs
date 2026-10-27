//! Pure game rules for Scavenger Salvage: no rendering, no audio, fully
//! deterministic for a given seed so it can be unit tested.
//!
//! The Commander kills scavenger drones, which leave wrecks. Wrecks are towed
//! (slowly) back to the Recycler at the centre of the map. Deposited salvage
//! fills quotas; each met quota auto-builds the next base structure. Building
//! the final structure, the Evac Gate, wins the game.

use crate::rng::Rng;

pub const WORLD: f32 = 2400.0;
pub const RECYCLER_RADIUS: f32 = 70.0;
pub const GRAB_RANGE: f32 = 60.0;
pub const WAVE_TIME: f32 = 35.0;
pub const BUILD_TIME: f32 = 2.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        V2 { x, y }
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V2::default()
        }
    }
    pub fn from_angle(a: f32) -> V2 {
        V2::new(a.cos(), a.sin())
    }
}

impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}

pub fn center() -> V2 {
    V2::new(WORLD / 2.0, WORLD / 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Drone,
    Raider,
    Brute,
    Boss,
}

impl EnemyKind {
    /// (hp, speed, radius, contact damage per second, xp, wreck value)
    pub fn stats(self) -> (f32, f32, f32, f32, u32, u32) {
        match self {
            EnemyKind::Drone => (14.0, 95.0, 11.0, 12.0, 1, 0),
            EnemyKind::Raider => (40.0, 120.0, 14.0, 18.0, 3, 6),
            EnemyKind::Brute => (160.0, 60.0, 22.0, 30.0, 8, 18),
            EnemyKind::Boss => (1400.0, 55.0, 40.0, 50.0, 40, 70),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Wreck {
    pub id: u32,
    pub pos: V2,
    pub value: u32,
    pub radius: f32,
    pub towed: bool,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: u32,
    pub from_turret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructKind {
    Llt,
    RepairPad,
    Radar,
    Hlt,
    NanoTower,
    Flak,
    Annihilator,
    EvacGate,
}

impl StructKind {
    pub const ORDER: [StructKind; 8] = [
        StructKind::Llt,
        StructKind::RepairPad,
        StructKind::Radar,
        StructKind::Hlt,
        StructKind::NanoTower,
        StructKind::Flak,
        StructKind::Annihilator,
        StructKind::EvacGate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            StructKind::Llt => "Light Laser Tower",
            StructKind::RepairPad => "Repair Pad",
            StructKind::Radar => "Radar Tower",
            StructKind::Hlt => "Heavy Laser Tower",
            StructKind::NanoTower => "Nano Tower",
            StructKind::Flak => "Flak Battery",
            StructKind::Annihilator => "Annihilator",
            StructKind::EvacGate => "Evac Gate",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            StructKind::Llt => "shoots drones near the base",
            StructKind::RepairPad => "heals you inside the base",
            StructKind::Radar => "points to wrecks off-screen",
            StructKind::Hlt => "long range, hard hitting",
            StructKind::NanoTower => "repairs the Recycler",
            StructKind::Flak => "rapid splash fire",
            StructKind::Annihilator => "obliterates big targets",
            StructKind::EvacGate => "escape: you win!",
        }
    }

    /// (range, damage, cooldown) for turrets.
    pub fn turret(self) -> Option<(f32, f32, f32)> {
        match self {
            StructKind::Llt => Some((260.0, 12.0, 0.45)),
            StructKind::Hlt => Some((380.0, 40.0, 0.9)),
            StructKind::Flak => Some((300.0, 9.0, 0.15)),
            StructKind::Annihilator => Some((520.0, 220.0, 2.4)),
            _ => None,
        }
    }

    /// Fixed slot around the Recycler.
    pub fn slot(self) -> V2 {
        let i = Self::ORDER.iter().position(|k| *k == self).unwrap_or(0);
        if self == StructKind::EvacGate {
            return center() + V2::new(0.0, -170.0);
        }
        let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.6;
        center() + V2::from_angle(a) * 150.0
    }
}

/// Salvage quota to build the `i`-th structure.
pub fn quota(i: usize) -> u32 {
    [30, 45, 60, 80, 100, 125, 150, 200]
        .get(i)
        .copied()
        .unwrap_or(9999)
}

#[derive(Clone, Debug)]
pub struct Structure {
    pub kind: StructKind,
    pub pos: V2,
    pub progress: f32,
    pub cooldown: f32,
    pub aim: f32,
}

impl Structure {
    pub fn built(&self) -> bool {
        self.progress >= 1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    MultiShot,
    Speed,
    Armor,
    Winch,
    Cable,
    Pierce,
    Recycler,
}

impl Upgrade {
    pub const ALL: [Upgrade; 9] = [
        Upgrade::Damage,
        Upgrade::FireRate,
        Upgrade::MultiShot,
        Upgrade::Speed,
        Upgrade::Armor,
        Upgrade::Winch,
        Upgrade::Cable,
        Upgrade::Pierce,
        Upgrade::Recycler,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavier Rounds",
            Upgrade::FireRate => "Rapid Fire",
            Upgrade::MultiShot => "Spread Shot",
            Upgrade::Speed => "Servo Legs",
            Upgrade::Armor => "Armor Plating",
            Upgrade::Winch => "Power Winch",
            Upgrade::Cable => "Extra Tow Cable",
            Upgrade::Pierce => "Piercing Slugs",
            Upgrade::Recycler => "Recycler Refit",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+30% weapon damage",
            Upgrade::FireRate => "+25% fire rate",
            Upgrade::MultiShot => "+1 projectile",
            Upgrade::Speed => "+12% move speed",
            Upgrade::Armor => "+30 max HP, full heal",
            Upgrade::Winch => "towing slows you 35% less",
            Upgrade::Cable => "tow one more wreck",
            Upgrade::Pierce => "shots pass through +1 enemy",
            Upgrade::Recycler => "+25% salvage, repair Recycler",
        }
    }

    fn maxed(self, p: &Player) -> bool {
        match self {
            Upgrade::MultiShot => p.shots >= 6,
            Upgrade::Cable => p.cables >= 4,
            Upgrade::Pierce => p.pierce >= 4,
            Upgrade::Winch => p.winch >= 3,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub fire_cd: f32,
    pub fire_rate: f32,
    pub shots: u32,
    pub pierce: u32,
    pub cables: u32,
    pub winch: u32,
    pub salvage_mult: f32,
    pub aim: f32,
    pub hurt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    LevelUp,
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot,
    TurretShot(StructKind),
    Hit,
    Kill { pos: V2, big: bool },
    Grab,
    Release,
    Deposit(u32),
    BuildStart(StructKind),
    Built(StructKind),
    LevelUp,
    WaveStart(u32),
    PlayerHurt,
    RecyclerHit,
    Won,
    Lost,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub grab: bool,
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub phase: Phase,
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub wrecks: Vec<Wreck>,
    pub bullets: Vec<Bullet>,
    pub structures: Vec<Structure>,
    /// Ids of wrecks currently towed, in cable order.
    pub towing: Vec<u32>,
    pub salvage: u32,
    pub total_salvage: u32,
    pub recycler_hp: f32,
    pub recycler_max: f32,
    pub xp: u32,
    pub level: u32,
    pub choices: Vec<Upgrade>,
    pub pending_levels: u32,
    pub wave: u32,
    pub wave_timer: f32,
    pub spawn_acc: f32,
    pub kills: u32,
    next_wreck: u32,
    pub events: Vec<Event>,
}

pub fn xp_for(level: u32) -> u32 {
    5 + level * 4 + level * level / 2
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Game {
            rng: Rng::new(seed),
            time: 0.0,
            phase: Phase::Playing,
            player: Player {
                pos: center() + V2::new(0.0, 110.0),
                hp: 100.0,
                max_hp: 100.0,
                speed: 210.0,
                damage: 10.0,
                fire_cd: 0.0,
                fire_rate: 3.0,
                shots: 1,
                pierce: 0,
                cables: 1,
                winch: 0,
                salvage_mult: 1.0,
                aim: 0.0,
                hurt: 0.0,
            },
            enemies: Vec::new(),
            wrecks: Vec::new(),
            bullets: Vec::new(),
            structures: Vec::new(),
            towing: Vec::new(),
            salvage: 0,
            total_salvage: 0,
            recycler_hp: 600.0,
            recycler_max: 600.0,
            xp: 0,
            level: 1,
            choices: Vec::new(),
            pending_levels: 0,
            wave: 1,
            wave_timer: WAVE_TIME,
            spawn_acc: 0.0,
            kills: 0,
            next_wreck: 0,
            events: Vec::new(),
        };
        // A few starter wrecks so the first haul can begin immediately.
        for i in 0..4 {
            let a = i as f32 * 1.7 + 0.4;
            let p = center() + V2::from_angle(a) * (380.0 + i as f32 * 60.0);
            g.add_wreck(p, 8);
        }
        g
    }

    pub fn add_wreck(&mut self, pos: V2, value: u32) {
        self.next_wreck += 1;
        self.wrecks.push(Wreck {
            id: self.next_wreck,
            pos,
            value,
            radius: 10.0 + (value as f32).sqrt() * 3.0,
            towed: false,
        });
    }

    /// Total value of wrecks currently in tow.
    pub fn tow_load(&self) -> u32 {
        self.wrecks
            .iter()
            .filter(|w| w.towed)
            .map(|w| w.value)
            .sum()
    }

    /// Movement multiplier from the towed load: heavier hauls are slower.
    pub fn tow_factor(&self) -> f32 {
        let load = self.tow_load() as f32;
        let k = 0.012 * 0.65f32.powi(self.player.winch as i32);
        (1.0 / (1.0 + load * k)).max(0.3)
    }

    pub fn structures_built(&self) -> usize {
        self.structures.len()
    }

    pub fn has(&self, k: StructKind) -> bool {
        self.structures.iter().any(|s| s.kind == k && s.built())
    }

    pub fn next_structure(&self) -> Option<StructKind> {
        StructKind::ORDER.get(self.structures.len()).copied()
    }

    pub fn next_quota(&self) -> u32 {
        quota(self.structures.len())
    }

    pub fn step(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        self.time += dt;
        self.waves(dt);
        self.move_player(dt, input);
        if input.grab {
            self.grab_or_release();
        }
        self.tow(dt);
        self.deposit();
        self.build(dt);
        self.fire(dt);
        self.turrets(dt);
        self.bullets_step(dt);
        self.enemies_step(dt);
        self.base_support(dt);
        self.check_end();
    }

    fn waves(&mut self, dt: f32) {
        self.wave_timer -= dt;
        if self.wave_timer <= 0.0 {
            self.wave += 1;
            self.wave_timer = WAVE_TIME;
            self.events.push(Event::WaveStart(self.wave));
            if self.wave.is_multiple_of(5) {
                self.spawn(EnemyKind::Boss);
            }
        }
        let rate = 0.8 + self.wave as f32 * 0.55;
        self.spawn_acc += rate * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let w = self.wave;
            let r = self.rng.f32();
            let kind = if w >= 3 && r < 0.04 + w as f32 * 0.012 {
                EnemyKind::Brute
            } else if w >= 2 && r < 0.25 + w as f32 * 0.025 {
                EnemyKind::Raider
            } else {
                EnemyKind::Drone
            };
            self.spawn(kind);
        }
    }

    pub fn spawn(&mut self, kind: EnemyKind) {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let d = self.rng.range(520.0, 700.0);
        let mut pos = self.player.pos + V2::from_angle(a) * d;
        pos.x = pos.x.clamp(0.0, WORLD);
        pos.y = pos.y.clamp(0.0, WORLD);
        let (hp, ..) = kind.stats();
        let hp = hp * (1.0 + (self.wave as f32 - 1.0) * 0.14);
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            flash: 0.0,
        });
    }

    fn move_player(&mut self, dt: f32, input: &Input) {
        let sp = self.player.speed * self.tow_factor();
        let p = &mut self.player;
        p.pos = p.pos + input.dir.norm() * sp * dt;
        p.pos.x = p.pos.x.clamp(20.0, WORLD - 20.0);
        p.pos.y = p.pos.y.clamp(20.0, WORLD - 20.0);
        p.hurt = (p.hurt - dt).max(0.0);
    }

    /// E: grab the nearest free wreck in range if a cable is free, otherwise
    /// release everything in tow.
    pub fn grab_or_release(&mut self) {
        let pp = self.player.pos;
        let free = (self.towing.len() as u32) < self.player.cables;
        let near = self
            .wrecks
            .iter()
            .filter(|w| !w.towed && w.pos.dist(pp) < GRAB_RANGE + w.radius)
            .min_by(|a, b| a.pos.dist(pp).total_cmp(&b.pos.dist(pp)))
            .map(|w| w.id);
        match near {
            Some(id) if free => {
                if let Some(w) = self.wrecks.iter_mut().find(|w| w.id == id) {
                    w.towed = true;
                }
                self.towing.push(id);
                self.events.push(Event::Grab);
            }
            _ if !self.towing.is_empty() => {
                for w in &mut self.wrecks {
                    w.towed = false;
                }
                self.towing.clear();
                self.events.push(Event::Release);
            }
            _ => {}
        }
    }

    fn tow(&mut self, dt: f32) {
        let mut anchor = self.player.pos;
        let mut anchor_r = 16.0;
        for id in self.towing.clone() {
            if let Some(w) = self.wrecks.iter_mut().find(|w| w.id == id) {
                let len = anchor_r + w.radius + 18.0;
                let d = w.pos - anchor;
                if d.len() > len {
                    let target = anchor + d.norm() * len;
                    w.pos = w.pos + (target - w.pos) * (1.0 - (-dt * 20.0).exp());
                }
                anchor = w.pos;
                anchor_r = w.radius;
            }
        }
    }

    fn deposit(&mut self) {
        let c = center();
        let mut gained = 0;
        let mult = self.player.salvage_mult;
        self.wrecks.retain(|w| {
            if w.towed && w.pos.dist(c) < RECYCLER_RADIUS + w.radius {
                gained += (w.value as f32 * mult).round() as u32;
                false
            } else {
                true
            }
        });
        if gained > 0 {
            let ids: Vec<u32> = self.wrecks.iter().map(|w| w.id).collect();
            self.towing.retain(|id| ids.contains(id));
            self.salvage += gained;
            self.total_salvage += gained;
            self.events.push(Event::Deposit(gained));
        }
    }

    fn build(&mut self, dt: f32) {
        if let Some(s) = self.structures.iter_mut().find(|s| !s.built()) {
            s.progress = (s.progress + dt / BUILD_TIME).min(1.0);
            if s.built() {
                let k = s.kind;
                self.events.push(Event::Built(k));
            }
            return;
        }
        if let Some(k) = self.next_structure() {
            let q = self.next_quota();
            if self.salvage >= q {
                self.salvage -= q;
                self.structures.push(Structure {
                    kind: k,
                    pos: k.slot(),
                    progress: 0.0,
                    cooldown: 0.0,
                    aim: 0.0,
                });
                self.events.push(Event::BuildStart(k));
            }
        }
    }

    fn nearest_enemy(&self, from: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist(from);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn fire(&mut self, dt: f32) {
        let p = &mut self.player;
        p.fire_cd -= dt;
        if p.fire_cd > 0.0 {
            return;
        }
        let Some(t) = self.nearest_enemy(self.player.pos, 420.0) else {
            return;
        };
        let p = &mut self.player;
        p.fire_cd = 1.0 / p.fire_rate;
        let dir = self.enemies[t].pos - p.pos;
        let base = dir.y.atan2(dir.x);
        p.aim = base;
        let n = p.shots;
        for i in 0..n {
            let off = (i as f32 - (n as f32 - 1.0) / 2.0) * 0.14;
            self.bullets.push(Bullet {
                pos: p.pos,
                vel: V2::from_angle(base + off) * 620.0,
                dmg: p.damage,
                life: 0.8,
                pierce: p.pierce,
                from_turret: false,
            });
        }
        self.events.push(Event::Shot);
    }

    fn turrets(&mut self, dt: f32) {
        for i in 0..self.structures.len() {
            let s = &self.structures[i];
            let Some((range, dmg, cd)) = s.kind.turret() else {
                continue;
            };
            if !s.built() {
                continue;
            }
            let s = &mut self.structures[i];
            s.cooldown -= dt;
            if s.cooldown > 0.0 {
                continue;
            }
            let pos = s.pos;
            let Some(t) = self.nearest_enemy(pos, range) else {
                continue;
            };
            let d = self.enemies[t].pos - pos;
            let s = &mut self.structures[i];
            s.cooldown = cd;
            s.aim = d.y.atan2(d.x);
            let kind = s.kind;
            self.bullets.push(Bullet {
                pos,
                vel: d.norm() * 760.0,
                dmg,
                life: range / 760.0 + 0.1,
                pierce: if kind == StructKind::Annihilator {
                    3
                } else {
                    0
                },
                from_turret: true,
            });
            self.events.push(Event::TurretShot(kind));
        }
    }

    fn bullets_step(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.bullets.len() {
            let b = &mut self.bullets[i];
            b.pos = b.pos + b.vel * dt;
            b.life -= dt;
            let mut dead = b.life <= 0.0;
            if !dead {
                let bp = b.pos;
                if let Some(e) = self
                    .enemies
                    .iter_mut()
                    .find(|e| e.hp > 0.0 && e.pos.dist(bp) < e.kind.stats().2 + 4.0)
                {
                    let b = &mut self.bullets[i];
                    e.hp -= b.dmg;
                    e.flash = 0.1;
                    self.events.push(Event::Hit);
                    if b.pierce == 0 {
                        dead = true;
                    } else {
                        b.pierce -= 1;
                        // Nudge past the enemy so it is not hit twice.
                        b.pos = b.pos + b.vel.norm() * (e.kind.stats().2 * 2.0 + 6.0);
                    }
                }
            }
            if dead {
                self.bullets.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Resolve kills.
        let mut j = 0;
        while j < self.enemies.len() {
            if self.enemies[j].hp <= 0.0 {
                let e = self.enemies.swap_remove(j);
                self.on_kill(&e);
            } else {
                j += 1;
            }
        }
    }

    fn on_kill(&mut self, e: &Enemy) {
        let (.., xp, value) = e.kind.stats();
        self.kills += 1;
        self.events.push(Event::Kill {
            pos: e.pos,
            big: matches!(e.kind, EnemyKind::Brute | EnemyKind::Boss),
        });
        // Drones only sometimes leave a small wreck.
        let value = if e.kind == EnemyKind::Drone {
            if self.rng.chance(0.3) { 3 } else { 0 }
        } else {
            value
        };
        if value > 0 {
            self.add_wreck(e.pos, value);
        }
        self.gain_xp(xp);
    }

    pub fn gain_xp(&mut self, xp: u32) {
        self.xp += xp;
        while self.xp >= xp_for(self.level) {
            self.xp -= xp_for(self.level);
            self.level += 1;
            self.pending_levels += 1;
        }
        if self.pending_levels > 0 && self.phase == Phase::Playing {
            self.open_level_up();
        }
    }

    fn open_level_up(&mut self) {
        let mut pool: Vec<Upgrade> = Upgrade::ALL
            .iter()
            .copied()
            .filter(|u| !u.maxed(&self.player))
            .collect();
        self.choices.clear();
        while self.choices.len() < 3 && !pool.is_empty() {
            let i = self.rng.below(pool.len());
            self.choices.push(pool.swap_remove(i));
        }
        self.phase = Phase::LevelUp;
        self.events.push(Event::LevelUp);
    }

    pub fn choose(&mut self, idx: usize) {
        if self.phase != Phase::LevelUp {
            return;
        }
        let Some(&u) = self.choices.get(idx) else {
            return;
        };
        let p = &mut self.player;
        match u {
            Upgrade::Damage => p.damage *= 1.3,
            Upgrade::FireRate => p.fire_rate *= 1.25,
            Upgrade::MultiShot => p.shots += 1,
            Upgrade::Speed => p.speed *= 1.12,
            Upgrade::Armor => {
                p.max_hp += 30.0;
                p.hp = p.max_hp;
            }
            Upgrade::Winch => p.winch += 1,
            Upgrade::Cable => p.cables += 1,
            Upgrade::Pierce => p.pierce += 1,
            Upgrade::Recycler => {
                p.salvage_mult += 0.25;
                self.recycler_hp = (self.recycler_hp + 200.0).min(self.recycler_max);
            }
        }
        self.pending_levels -= 1;
        self.phase = Phase::Playing;
        if self.pending_levels > 0 {
            self.open_level_up();
        }
    }

    fn enemies_step(&mut self, dt: f32) {
        let pp = self.player.pos;
        let c = center();
        let mut hurt = 0.0;
        let mut rec_hurt = 0.0;
        for e in &mut self.enemies {
            let (_, speed, r, dps, ..) = e.kind.stats();
            e.flash = (e.flash - dt).max(0.0);
            // Raiders and bosses go for the Recycler when the player is far.
            let to_base = matches!(e.kind, EnemyKind::Raider | EnemyKind::Boss)
                && e.pos.dist(c) + 150.0 < e.pos.dist(pp);
            let target = if to_base { c } else { pp };
            let reach = if to_base { RECYCLER_RADIUS } else { 16.0 };
            let d = target - e.pos;
            if d.len() > r + reach - 4.0 {
                e.pos = e.pos + d.norm() * speed * dt;
            } else if to_base {
                rec_hurt += dps * dt;
            } else {
                hurt += dps * dt;
            }
        }
        // Light separation so swarms do not collapse into a single dot.
        let n = self.enemies.len();
        if n < 400 {
            for i in 0..n {
                for j in (i + 1)..n {
                    let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
                    let min = self.enemies[i].kind.stats().2 + self.enemies[j].kind.stats().2;
                    let d = b - a;
                    let l = d.len();
                    if l < min && l > 0.01 {
                        let push = d.norm() * ((min - l) * 0.5);
                        self.enemies[i].pos = a - push;
                        self.enemies[j].pos = b + push;
                    }
                }
            }
        }
        if hurt > 0.0 {
            self.player.hp -= hurt;
            if self.player.hurt <= 0.0 {
                self.player.hurt = 0.25;
                self.events.push(Event::PlayerHurt);
            }
        }
        if rec_hurt > 0.0 {
            self.recycler_hp -= rec_hurt;
            if self.rng.chance(dt * 4.0) {
                self.events.push(Event::RecyclerHit);
            }
        }
    }

    fn base_support(&mut self, dt: f32) {
        let c = center();
        if self.has(StructKind::RepairPad) && self.player.pos.dist(c) < 260.0 {
            self.player.hp = (self.player.hp + 8.0 * dt).min(self.player.max_hp);
        }
        if self.has(StructKind::NanoTower) {
            self.recycler_hp = (self.recycler_hp + 6.0 * dt).min(self.recycler_max);
        }
    }

    fn check_end(&mut self) {
        if self.player.hp <= 0.0 || self.recycler_hp <= 0.0 {
            self.player.hp = self.player.hp.max(0.0);
            self.recycler_hp = self.recycler_hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        } else if self.has(StructKind::EvacGate) {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
        }
    }

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
    fn grab_tow_and_deposit_fills_salvage() {
        let mut g = Game::new(1);
        g.wrecks.clear();
        g.add_wreck(center() + V2::new(300.0, 0.0), 10);
        g.player.pos = center() + V2::new(330.0, 0.0);
        g.grab_or_release();
        assert_eq!(g.towing.len(), 1);
        assert!(g.tow_factor() < 1.0);
        g.player.pos = center();
        for _ in 0..30 {
            g.tow(1.0 / 60.0);
            g.deposit();
        }
        assert_eq!(g.salvage, 10);
        assert!(g.wrecks.is_empty() && g.towing.is_empty());
    }

    #[test]
    fn heavier_loads_are_slower_and_winch_helps() {
        let mut g = Game::new(2);
        g.wrecks.clear();
        g.add_wreck(V2::new(0.0, 0.0), 5);
        g.wrecks[0].towed = true;
        let light = g.tow_factor();
        g.wrecks[0].value = 60;
        let heavy = g.tow_factor();
        assert!(heavy < light);
        g.player.winch = 2;
        assert!(g.tow_factor() > heavy);
        assert!(g.tow_factor() >= 0.3);
    }

    #[test]
    fn cable_limit_and_release() {
        let mut g = Game::new(3);
        g.wrecks.clear();
        let p = g.player.pos;
        g.add_wreck(p + V2::new(10.0, 0.0), 5);
        g.add_wreck(p + V2::new(-10.0, 0.0), 5);
        g.grab_or_release();
        assert_eq!(g.towing.len(), 1);
        // Only one cable: second press releases.
        g.grab_or_release();
        assert!(g.towing.is_empty());
        assert!(g.wrecks.iter().all(|w| !w.towed));
        g.player.cables = 2;
        g.grab_or_release();
        g.grab_or_release();
        assert_eq!(g.towing.len(), 2);
    }

    #[test]
    fn quota_auto_builds_structure_in_order() {
        let mut g = Game::new(4);
        g.salvage = quota(0) + 5;
        g.build(0.0);
        assert_eq!(g.structures.len(), 1);
        assert_eq!(g.salvage, 5);
        assert!(!g.has(StructKind::Llt));
        for _ in 0..200 {
            g.build(0.05);
        }
        assert!(g.has(StructKind::Llt));
        assert!(g.drain_events().contains(&Event::Built(StructKind::Llt)));
    }

    #[test]
    fn evac_gate_wins() {
        let mut g = Game::new(5);
        for k in StructKind::ORDER {
            g.structures.push(Structure {
                kind: k,
                pos: k.slot(),
                progress: 1.0,
                cooldown: 0.0,
                aim: 0.0,
            });
        }
        g.check_end();
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn dying_loses() {
        let mut g = Game::new(6);
        g.player.hp = -1.0;
        g.step(0.016, &idle());
        assert_eq!(g.phase, Phase::Lost);
        let mut g = Game::new(6);
        g.recycler_hp = 0.0;
        g.step(0.016, &idle());
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn kills_give_xp_and_level_up_offers_choices() {
        let mut g = Game::new(7);
        g.gain_xp(xp_for(1));
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        let before = g.player.clone();
        g.choose(0);
        assert_eq!(g.phase, Phase::Playing);
        let p = &g.player;
        let changed = p.damage != before.damage
            || p.fire_rate != before.fire_rate
            || p.shots != before.shots
            || p.speed != before.speed
            || p.max_hp != before.max_hp
            || p.winch != before.winch
            || p.cables != before.cables
            || p.pierce != before.pierce
            || p.salvage_mult != before.salvage_mult;
        assert!(changed);
    }

    #[test]
    fn waves_escalate_and_bosses_appear() {
        let mut g = Game::new(8);
        g.player.hp = 1e9;
        g.recycler_hp = 1e9;
        g.recycler_max = 1e9;
        let mut boss = false;
        for _ in 0..(60 * WAVE_TIME as usize * 5) {
            g.step(1.0 / 60.0, &idle());
            if g.phase == Phase::LevelUp {
                g.choose(0);
            }
            boss |= g.enemies.iter().any(|e| e.kind == EnemyKind::Boss);
        }
        assert!(g.wave >= 5);
        assert!(boss);
        assert!(g.kills > 0);
    }

    #[test]
    fn player_shoots_and_kills_enemy() {
        let mut g = Game::new(9);
        g.enemies.push(Enemy {
            kind: EnemyKind::Drone,
            pos: g.player.pos + V2::new(150.0, 0.0),
            hp: 5.0,
            max_hp: 5.0,
            flash: 0.0,
        });
        for _ in 0..60 {
            g.fire(1.0 / 60.0);
            g.bullets_step(1.0 / 60.0);
        }
        assert_eq!(g.kills, 1);
        let ev = g.drain_events();
        assert!(ev.contains(&Event::Shot));
    }

    #[test]
    fn quotas_increase() {
        for i in 1..8 {
            assert!(quota(i) > quota(i - 1));
        }
    }
}
