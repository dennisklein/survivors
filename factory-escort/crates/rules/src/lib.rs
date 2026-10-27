//! Factory Escort game rules: pure, deterministic simulation without any
//! rendering or audio dependencies.

pub mod synth;

use std::collections::VecDeque;
use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V2::default()
        }
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn from_angle(a: f32) -> V2 {
        v2(a.cos(), a.sin())
    }
}
impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        *self = *self + o;
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
    }
}

pub const WORLD: f32 = 3200.0;
pub const LAB_POS: V2 = v2(WORLD / 2.0, WORLD / 2.0);
pub const LAB_RADIUS: f32 = 48.0;
pub const WIN_TIME: f32 = 600.0;
pub const BUILD_RADIUS: f32 = 360.0;
pub const LAB_TURRET_RANGE: f32 = 240.0;
pub const MAX_QUEUE: usize = 8;
pub const BOSS_TIMES: [f32; 3] = [180.0, 360.0, 540.0];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Pawn,
    Grunt,
    Rocko,
    Hammer,
    Warrior,
}

#[derive(Clone, Copy, Debug)]
pub struct UnitStats {
    pub hp: f32,
    pub speed: f32,
    pub range: f32,
    pub damage: f32,
    pub cooldown: f32,
    pub splash: f32,
    pub cost: f32,
    pub build_time: f32,
    pub radius: f32,
}

impl UnitKind {
    pub const ALL: [UnitKind; 5] = [
        UnitKind::Pawn,
        UnitKind::Grunt,
        UnitKind::Rocko,
        UnitKind::Hammer,
        UnitKind::Warrior,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        ["Pawn", "Grunt", "Rocko", "Hammer", "Warrior"][self.index()]
    }
    pub fn stats(self) -> UnitStats {
        let s = |hp, speed, range, damage, cooldown, splash, cost, build_time, radius| UnitStats {
            hp,
            speed,
            range,
            damage,
            cooldown,
            splash,
            cost,
            build_time,
            radius,
        };
        match self {
            UnitKind::Pawn => s(45.0, 165.0, 120.0, 7.0, 0.4, 0.0, 20.0, 1.6, 9.0),
            UnitKind::Grunt => s(100.0, 115.0, 150.0, 16.0, 0.8, 0.0, 35.0, 2.8, 11.0),
            UnitKind::Rocko => s(70.0, 105.0, 270.0, 30.0, 1.7, 45.0, 55.0, 4.0, 11.0),
            UnitKind::Hammer => s(170.0, 95.0, 220.0, 50.0, 2.1, 70.0, 80.0, 5.5, 13.0),
            UnitKind::Warrior => s(340.0, 105.0, 160.0, 11.0, 0.22, 0.0, 110.0, 6.5, 14.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Tick,
    Raider,
    Thud,
    Tank,
    Boss,
}

#[derive(Clone, Copy, Debug)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub range: f32,
    pub damage: f32,
    pub cooldown: f32,
    pub xp: u32,
    pub radius: f32,
}

impl EnemyKind {
    pub fn stats(self) -> EnemyStats {
        let s = |hp, speed, range, damage, cooldown, xp, radius| EnemyStats {
            hp,
            speed,
            range,
            damage,
            cooldown,
            xp,
            radius,
        };
        match self {
            EnemyKind::Tick => s(18.0, 135.0, 20.0, 5.0, 0.6, 1, 8.0),
            EnemyKind::Raider => s(40.0, 165.0, 100.0, 5.0, 0.5, 2, 10.0),
            EnemyKind::Thud => s(70.0, 75.0, 140.0, 7.0, 1.6, 3, 12.0),
            EnemyKind::Tank => s(280.0, 58.0, 200.0, 22.0, 2.2, 10, 18.0),
            EnemyKind::Boss => s(3500.0, 45.0, 280.0, 40.0, 1.2, 80, 34.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    Solar,
    Mex,
}

impl BuildingKind {
    pub fn cost(self) -> f32 {
        match self {
            BuildingKind::Solar => 60.0,
            BuildingKind::Mex => 45.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            BuildingKind::Solar => "Solar",
            BuildingKind::Mex => "Metal Extractor",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub kind: UnitKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cd: f32,
    pub facing: f32,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cd: f32,
    pub facing: f32,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildingKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
}

#[derive(Clone, Debug)]
pub struct Gem {
    pub pos: V2,
    pub xp: u32,
}

#[derive(Clone, Debug)]
pub struct Commander {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cd: f32,
    pub damage: f32,
    pub range: f32,
    pub speed: f32,
    pub facing: f32,
}

#[derive(Clone, Debug)]
pub struct Lab {
    pub hp: f32,
    pub max_hp: f32,
    pub queue: VecDeque<UnitKind>,
    pub progress: f32,
    pub repeat: Option<UnitKind>,
    pub hit_timer: f32,
    pub turret_cd: f32,
    pub turret_damage: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    Follow,
    Rally(V2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Unlock(UnitKind),
    CmdDamage,
    CmdSpeed,
    ArmyHp,
    ArmyDamage,
    LabArmor,
    FreeMex,
    FreeSolar,
    ArmyCap,
    Magnet,
}

impl Upgrade {
    pub fn title(self) -> String {
        match self {
            Upgrade::Unlock(k) => format!("Blueprint: {}", k.name()),
            Upgrade::CmdDamage => "Commander D-Gun +35% damage".into(),
            Upgrade::CmdSpeed => "Commander +12% speed".into(),
            Upgrade::ArmyHp => "Army armor +20% HP".into(),
            Upgrade::ArmyDamage => "Army weapons +20% damage".into(),
            Upgrade::LabArmor => "Lab plating +400 HP, turret +30%, full repair".into(),
            Upgrade::FreeMex => "Free Metal Extractor".into(),
            Upgrade::FreeSolar => "Free Solar Collector".into(),
            Upgrade::ArmyCap => "Bot Lab capacity +8 units".into(),
            Upgrade::Magnet => "Salvage magnet +40% range".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShotKind {
    Laser,
    Cannon,
    Rocket,
    Enemy,
    DGun,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot { from: V2, to: V2, kind: ShotKind },
    Hit { pos: V2 },
    Explosion { pos: V2, size: f32 },
    Pickup,
    LevelUp,
    Built { pos: V2 },
    UnitSpawned { pos: V2 },
    Denied,
    LabHit,
    BossSpawned,
    GameOver,
    Victory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Won,
    LostCommander,
    LostLab,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub rally: Option<V2>,
    pub follow: bool,
    pub queue: Option<UnitKind>,
    pub build: Option<BuildingKind>,
    pub choose: Option<usize>,
}

pub struct Game {
    pub t: f32,
    rng: Rng,
    pub phase: Phase,
    pub cmd: Commander,
    pub lab: Lab,
    pub units: Vec<Unit>,
    pub enemies: Vec<Enemy>,
    pub buildings: Vec<Building>,
    pub gems: Vec<Gem>,
    pub metal: f32,
    pub order: Order,
    pub xp: u32,
    pub level: u32,
    pub xp_next: u32,
    pub choices: Option<[Upgrade; 3]>,
    pub unlocked: [bool; 5],
    pub army_hp_mult: f32,
    pub army_dmg_mult: f32,
    pub cap: usize,
    pub magnet: f32,
    pub kills: u32,
    pub events: Vec<Event>,
    spawn_acc: f32,
    bosses_spawned: usize,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Game {
            t: 0.0,
            rng: Rng(seed.max(1)),
            phase: Phase::Playing,
            cmd: Commander {
                pos: LAB_POS + v2(0.0, 110.0),
                hp: 500.0,
                max_hp: 500.0,
                cd: 0.0,
                damage: 18.0,
                range: 200.0,
                speed: 210.0,
                facing: 0.0,
            },
            lab: Lab {
                hp: 2000.0,
                max_hp: 2000.0,
                queue: VecDeque::new(),
                progress: 0.0,
                repeat: Some(UnitKind::Pawn),
                hit_timer: 0.0,
                turret_cd: 0.0,
                turret_damage: 14.0,
            },
            units: Vec::new(),
            enemies: Vec::new(),
            buildings: Vec::new(),
            gems: Vec::new(),
            metal: 120.0,
            order: Order::Follow,
            xp: 0,
            level: 1,
            xp_next: 6,
            choices: None,
            unlocked: [true, true, false, false, false],
            army_hp_mult: 1.0,
            army_dmg_mult: 1.0,
            cap: 24,
            magnet: 90.0,
            kills: 0,
            events: Vec::new(),
            spawn_acc: 0.0,
            bosses_spawned: 0,
        };
        g.add_building(BuildingKind::Mex, LAB_POS + v2(-120.0, -90.0));
        g.add_building(BuildingKind::Solar, LAB_POS + v2(120.0, -90.0));
        g
    }

    fn add_building(&mut self, kind: BuildingKind, pos: V2) {
        self.buildings.push(Building {
            kind,
            pos,
            hp: 300.0,
            max_hp: 300.0,
        });
    }

    pub fn count(&self, kind: BuildingKind) -> usize {
        self.buildings.iter().filter(|b| b.kind == kind).count()
    }
    /// Metal per second.
    pub fn income(&self) -> f32 {
        4.0 + 2.2 * self.count(BuildingKind::Mex) as f32
    }
    /// Lab build speed multiplier (energy).
    pub fn build_speed(&self) -> f32 {
        1.0 + 0.3 * self.count(BuildingKind::Solar) as f32
    }
    pub fn lab_under_attack(&self) -> bool {
        self.lab.hit_timer > 0.0
    }

    pub fn queue_unit(&mut self, kind: UnitKind) -> bool {
        let cost = kind.stats().cost;
        if !self.unlocked[kind.index()] || self.lab.queue.len() >= MAX_QUEUE || self.metal < cost {
            self.events.push(Event::Denied);
            return false;
        }
        self.metal -= cost;
        self.lab.queue.push_back(kind);
        self.lab.repeat = Some(kind);
        true
    }

    pub fn place_building(&mut self, kind: BuildingKind) -> bool {
        let pos = self.cmd.pos;
        let blocked = pos.dist(LAB_POS) < LAB_RADIUS + 40.0
            || self.buildings.iter().any(|b| b.pos.dist(pos) < 50.0);
        if pos.dist(LAB_POS) > BUILD_RADIUS || blocked || self.metal < kind.cost() {
            self.events.push(Event::Denied);
            return false;
        }
        self.metal -= kind.cost();
        self.add_building(kind, pos);
        self.events.push(Event::Built { pos });
        true
    }

    pub fn apply(&mut self, up: Upgrade) {
        match up {
            Upgrade::Unlock(k) => self.unlocked[k.index()] = true,
            Upgrade::CmdDamage => self.cmd.damage *= 1.35,
            Upgrade::CmdSpeed => self.cmd.speed *= 1.12,
            Upgrade::ArmyHp => {
                self.army_hp_mult *= 1.2;
                for u in &mut self.units {
                    u.max_hp *= 1.2;
                    u.hp *= 1.2;
                }
            }
            Upgrade::ArmyDamage => self.army_dmg_mult *= 1.2,
            Upgrade::LabArmor => {
                self.lab.max_hp += 400.0;
                self.lab.hp = self.lab.max_hp;
                self.lab.turret_damage *= 1.3;
            }
            Upgrade::FreeMex | Upgrade::FreeSolar => {
                let kind = if up == Upgrade::FreeMex {
                    BuildingKind::Mex
                } else {
                    BuildingKind::Solar
                };
                let n = self.buildings.len() as f32;
                let pos = LAB_POS + V2::from_angle(n * 2.4) * (130.0 + 14.0 * n);
                self.add_building(kind, pos);
                self.events.push(Event::Built { pos });
            }
            Upgrade::ArmyCap => self.cap += 8,
            Upgrade::Magnet => self.magnet *= 1.4,
        }
    }

    fn roll_choices(&mut self) -> [Upgrade; 3] {
        let mut pool: Vec<Upgrade> = vec![
            Upgrade::CmdDamage,
            Upgrade::CmdSpeed,
            Upgrade::ArmyHp,
            Upgrade::ArmyDamage,
            Upgrade::LabArmor,
            Upgrade::FreeMex,
            Upgrade::FreeSolar,
            Upgrade::ArmyCap,
            Upgrade::Magnet,
        ];
        // Locked blueprints are offered first and in order, so they appear early.
        if let Some(k) = UnitKind::ALL.iter().find(|k| !self.unlocked[k.index()]) {
            pool.insert(0, Upgrade::Unlock(*k));
            pool.insert(0, Upgrade::Unlock(*k));
        }
        let mut out = [Upgrade::CmdDamage; 3];
        for slot in &mut out {
            let i = ((self.rng.next() * pool.len() as f32) as usize).min(pool.len() - 1);
            *slot = pool[i];
            pool.retain(|u| u != slot);
        }
        out
    }

    fn gain_xp(&mut self, xp: u32) {
        self.xp += xp;
        if self.xp >= self.xp_next && self.choices.is_none() {
            self.xp -= self.xp_next;
            self.level += 1;
            self.xp_next = 6 + self.level * 5 + self.level * self.level / 2;
            self.choices = Some(self.roll_choices());
            self.events.push(Event::LevelUp);
        }
    }

    fn spawn_enemy(&mut self, kind: EnemyKind) {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let pos = LAB_POS + V2::from_angle(a) * self.rng.range(1250.0, 1450.0);
        let st = kind.stats();
        let scale = 1.0 + self.t / 260.0;
        let hp = if kind == EnemyKind::Boss {
            st.hp * (1.0 + self.bosses_spawned as f32 * 0.8)
        } else {
            st.hp * scale
        };
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            cd: 1.0,
            facing: a + std::f32::consts::PI,
            flash: 0.0,
        });
    }

    /// Enemies spawned per second at the current time.
    pub fn spawn_rate(&self) -> f32 {
        0.6 + self.t / 110.0
    }

    pub fn wave(&self) -> u32 {
        (self.t / 30.0) as u32 + 1
    }

    fn pick_enemy(&mut self) -> EnemyKind {
        let r = self.rng.next();
        let t = self.t;
        if t > 240.0 && r < 0.08 + t / 6000.0 {
            EnemyKind::Tank
        } else if t > 90.0 && r < 0.35 {
            EnemyKind::Thud
        } else if t > 45.0 && r < 0.6 {
            EnemyKind::Raider
        } else {
            EnemyKind::Tick
        }
    }

    pub fn step(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        if let Some(choices) = self.choices {
            if let Some(i) = input.choose.filter(|i| *i < 3) {
                self.choices = None;
                self.apply(choices[i]);
                self.gain_xp(0);
            }
            return;
        }
        self.t += dt;
        // Commands.
        if let Some(p) = input.rally {
            self.order = Order::Rally(p);
        }
        if input.follow {
            self.order = Order::Follow;
        }
        if let Some(k) = input.queue {
            self.queue_unit(k);
        }
        if let Some(b) = input.build {
            self.place_building(b);
        }

        // Commander.
        let d = input.dir.norm();
        if d.len() > 0.0 {
            self.cmd.facing = d.y.atan2(d.x);
        }
        self.cmd.pos += d * (self.cmd.speed * dt);
        self.cmd.pos.x = self.cmd.pos.x.clamp(20.0, WORLD - 20.0);
        self.cmd.pos.y = self.cmd.pos.y.clamp(20.0, WORLD - 20.0);
        self.cmd.hp = (self.cmd.hp + 4.0 * dt).min(self.cmd.max_hp);
        self.cmd.cd -= dt;
        if self.cmd.cd <= 0.0
            && let Some(i) = self.nearest_enemy(self.cmd.pos, self.cmd.range)
        {
            let to = self.enemies[i].pos;
            self.cmd.cd = 0.35;
            self.cmd.facing = (to.y - self.cmd.pos.y).atan2(to.x - self.cmd.pos.x);
            self.damage_enemy(i, self.cmd.damage);
            self.events.push(Event::Shot {
                from: self.cmd.pos,
                to,
                kind: ShotKind::DGun,
            });
        }

        // Economy and lab production.
        self.metal += self.income() * dt;
        self.lab.hp = (self.lab.hp + 3.0 * dt).min(self.lab.max_hp);
        self.lab.hit_timer -= dt;
        self.lab.turret_cd -= dt;
        if self.lab.turret_cd <= 0.0
            && let Some(i) = self.nearest_enemy(LAB_POS, LAB_TURRET_RANGE)
        {
            self.lab.turret_cd = 0.4;
            let to = self.enemies[i].pos;
            self.damage_enemy(i, self.lab.turret_damage);
            self.events.push(Event::Shot {
                from: LAB_POS,
                to,
                kind: ShotKind::Laser,
            });
        }
        if self.lab.queue.is_empty()
            && let Some(k) = self.lab.repeat
            && self.metal >= k.stats().cost
            && self.units.len() < self.cap
        {
            self.metal -= k.stats().cost;
            self.lab.queue.push_back(k);
        }
        if let Some(&k) = self.lab.queue.front()
            && self.units.len() < self.cap
        {
            self.lab.progress += dt * self.build_speed();
            if self.lab.progress >= k.stats().build_time {
                self.lab.progress = 0.0;
                self.lab.queue.pop_front();
                let st = k.stats();
                let pos = LAB_POS + v2(self.rng.range(-20.0, 20.0), LAB_RADIUS + 12.0);
                self.units.push(Unit {
                    kind: k,
                    pos,
                    hp: st.hp * self.army_hp_mult,
                    max_hp: st.hp * self.army_hp_mult,
                    cd: 0.0,
                    facing: 1.57,
                });
                self.events.push(Event::UnitSpawned { pos });
            }
        }

        self.step_units(dt);
        self.step_enemies(dt);
        self.cleanup();
        self.step_gems(dt);
        self.step_spawns(dt);

        if self.cmd.hp <= 0.0 {
            self.phase = Phase::LostCommander;
            self.events.push(Event::GameOver);
        } else if self.lab.hp <= 0.0 {
            self.phase = Phase::LostLab;
            self.events.push(Event::GameOver);
        } else if self.t >= WIN_TIME && !self.enemies.iter().any(|e| e.kind == EnemyKind::Boss) {
            self.phase = Phase::Won;
            self.events.push(Event::Victory);
        }
    }

    fn nearest_enemy(&self, pos: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist(pos) - e.kind.stats().radius;
            if d < bd && e.hp > 0.0 {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn damage_enemy(&mut self, i: usize, dmg: f32) {
        let e = &mut self.enemies[i];
        e.hp -= dmg;
        e.flash = 0.08;
        let pos = e.pos;
        self.events.push(Event::Hit { pos });
    }

    fn step_units(&mut self, dt: f32) {
        let n = self.units.len();
        for i in 0..n {
            let (kind, pos) = (self.units[i].kind, self.units[i].pos);
            let st = kind.stats();
            let anchor = match self.order {
                Order::Follow => self.cmd.pos,
                Order::Rally(p) => p,
            };
            // Formation slot: rings around the anchor.
            let ring = (i / 10) as f32;
            let slot =
                anchor + V2::from_angle(i as f32 * 0.628 + ring * 0.3) * (55.0 + ring * 32.0);
            let target = self
                .nearest_enemy(pos, st.range + 140.0)
                .filter(|&j| self.enemies[j].pos.dist(anchor) < 420.0);
            let mut goal = slot;
            if let Some(j) = target {
                let ep = self.enemies[j].pos;
                let d = ep.dist(pos);
                goal = if d > st.range * 0.9 { ep } else { pos };
                self.units[i].cd -= dt;
                if d <= st.range + self.enemies[j].kind.stats().radius && self.units[i].cd <= 0.0 {
                    self.units[i].cd = st.cooldown;
                    self.units[i].facing = (ep.y - pos.y).atan2(ep.x - pos.x);
                    let dmg = st.damage * self.army_dmg_mult;
                    let shot = match kind {
                        UnitKind::Rocko => ShotKind::Rocket,
                        UnitKind::Hammer => ShotKind::Cannon,
                        _ => ShotKind::Laser,
                    };
                    self.events.push(Event::Shot {
                        from: pos,
                        to: ep,
                        kind: shot,
                    });
                    if st.splash > 0.0 {
                        for k in 0..self.enemies.len() {
                            if self.enemies[k].pos.dist(ep) < st.splash {
                                self.damage_enemy(k, dmg);
                            }
                        }
                        self.events.push(Event::Explosion {
                            pos: ep,
                            size: st.splash * 0.6,
                        });
                    } else {
                        self.damage_enemy(j, dmg);
                    }
                }
            } else {
                self.units[i].cd -= dt;
            }
            let delta = goal - pos;
            if delta.len() > 6.0 {
                let mv = delta.norm() * (st.speed * dt).min(delta.len());
                self.units[i].pos += mv;
                if target.is_none() {
                    self.units[i].facing = mv.y.atan2(mv.x);
                }
            }
        }
        separate(
            self.units
                .iter_mut()
                .map(|u| (&mut u.pos, u.kind.stats().radius)),
        );
    }

    fn step_enemies(&mut self, dt: f32) {
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            let st = e.kind.stats();
            let pos = e.pos;
            // Target selection: nearest of commander, army, buildings within aggro,
            // otherwise the lab.
            let aggro = 260.0;
            let mut best: (f32, Target) = (pos.dist(LAB_POS) - LAB_RADIUS, Target::Lab);
            let mut consider = |d: f32, t: Target| {
                if d < aggro && d < best.0 {
                    best = (d, t);
                }
            };
            consider(pos.dist(self.cmd.pos) + 60.0, Target::Cmd);
            for (k, u) in self.units.iter().enumerate() {
                consider(pos.dist(u.pos), Target::Unit(k));
            }
            for (k, b) in self.buildings.iter().enumerate() {
                consider(pos.dist(b.pos) - 16.0, Target::Building(k));
            }
            let (dist, target) = best;
            let tpos = match target {
                Target::Lab => LAB_POS,
                Target::Cmd => self.cmd.pos,
                Target::Unit(k) => self.units[k].pos,
                Target::Building(k) => self.buildings[k].pos,
            };
            let e = &mut self.enemies[i];
            e.flash -= dt;
            e.cd -= dt;
            e.facing = (tpos.y - pos.y).atan2(tpos.x - pos.x);
            if dist > st.range {
                e.pos += (tpos - pos).norm() * (st.speed * dt);
            } else if e.cd <= 0.0 {
                e.cd = st.cooldown;
                let dmg = st.damage * (1.0 + self.t / 600.0);
                match target {
                    Target::Lab => {
                        self.lab.hp -= dmg;
                        if self.lab.hit_timer <= 0.0 {
                            self.events.push(Event::LabHit);
                        }
                        self.lab.hit_timer = 3.0;
                    }
                    Target::Cmd => self.cmd.hp -= dmg,
                    Target::Unit(k) => self.units[k].hp -= dmg,
                    Target::Building(k) => self.buildings[k].hp -= dmg,
                }
                if st.range > 40.0 {
                    self.events.push(Event::Shot {
                        from: pos,
                        to: tpos,
                        kind: ShotKind::Enemy,
                    });
                }
            }
        }
        separate(
            self.enemies
                .iter_mut()
                .map(|e| (&mut e.pos, e.kind.stats().radius)),
        );
    }

    fn cleanup(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let st = e.kind.stats();
                self.kills += 1;
                self.gems.push(Gem {
                    pos: e.pos,
                    xp: st.xp,
                });
                self.events.push(Event::Explosion {
                    pos: e.pos,
                    size: st.radius * 2.0,
                });
            } else {
                i += 1;
            }
        }
        for u in self.units.iter().filter(|u| u.hp <= 0.0) {
            self.events.push(Event::Explosion {
                pos: u.pos,
                size: u.kind.stats().radius * 2.0,
            });
        }
        self.units.retain(|u| u.hp > 0.0);
        for b in self.buildings.iter().filter(|b| b.hp <= 0.0) {
            self.events.push(Event::Explosion {
                pos: b.pos,
                size: 40.0,
            });
        }
        self.buildings.retain(|b| b.hp > 0.0);
    }

    fn step_gems(&mut self, dt: f32) {
        let mut gained = 0;
        let cp = self.cmd.pos;
        let magnet = self.magnet;
        self.gems.retain_mut(|g| {
            let d = g.pos.dist(cp);
            if d < 18.0 {
                gained += g.xp;
                return false;
            }
            if d < magnet {
                g.pos += (cp - g.pos).norm() * (420.0 * dt).min(d);
            }
            true
        });
        if gained > 0 {
            self.events.push(Event::Pickup);
            self.gain_xp(gained);
        }
    }

    fn step_spawns(&mut self, dt: f32) {
        if self.t < WIN_TIME {
            self.spawn_acc += self.spawn_rate() * dt;
            while self.spawn_acc >= 1.0 {
                self.spawn_acc -= 1.0;
                let k = self.pick_enemy();
                self.spawn_enemy(k);
            }
        }
        if self.bosses_spawned < BOSS_TIMES.len() && self.t >= BOSS_TIMES[self.bosses_spawned] {
            self.spawn_enemy(EnemyKind::Boss);
            self.bosses_spawned += 1;
            self.events.push(Event::BossSpawned);
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Lab,
    Cmd,
    Unit(usize),
    Building(usize),
}

/// Push overlapping circles apart (and out of the lab).
fn separate<'a>(items: impl Iterator<Item = (&'a mut V2, f32)>) {
    let mut v: Vec<(&mut V2, f32)> = items.collect();
    let n = v.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let d = *v[j].0 - *v[i].0;
            let min = v[i].1 + v[j].1;
            let l = d.len();
            if l < min && l > 1e-3 {
                let push = d * ((min - l) / l * 0.5);
                *v[i].0 = *v[i].0 - push;
                *v[j].0 += push;
            }
        }
        let d = *v[i].0 - LAB_POS;
        let min = LAB_RADIUS + v[i].1;
        if d.len() < min {
            *v[i].0 = LAB_POS + d.norm() * min;
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
    fn queueing_costs_metal_and_respects_unlocks() {
        let mut g = Game::new(1);
        let m = g.metal;
        assert!(g.queue_unit(UnitKind::Pawn));
        assert_eq!(g.metal, m - UnitKind::Pawn.stats().cost);
        assert!(!g.queue_unit(UnitKind::Warrior));
        g.apply(Upgrade::Unlock(UnitKind::Warrior));
        g.metal = 1000.0;
        assert!(g.queue_unit(UnitKind::Warrior));
    }

    #[test]
    fn lab_produces_units() {
        let mut g = Game::new(2);
        for _ in 0..200 {
            g.step(0.05, &idle());
        }
        assert!(!g.units.is_empty());
    }

    #[test]
    fn buildings_only_near_lab() {
        let mut g = Game::new(3);
        g.metal = 500.0;
        g.cmd.pos = LAB_POS + v2(0.0, 200.0);
        assert!(g.place_building(BuildingKind::Mex));
        assert_eq!(g.count(BuildingKind::Mex), 2);
        g.cmd.pos = LAB_POS + v2(0.0, 1000.0);
        assert!(!g.place_building(BuildingKind::Solar));
        assert!(g.income() > 4.0 + 2.2);
    }

    #[test]
    fn xp_triggers_level_up_and_pauses() {
        let mut g = Game::new(4);
        g.gain_xp(100);
        assert_eq!(g.level, 2);
        assert!(g.choices.is_some());
        let t = g.t;
        g.step(0.1, &idle());
        assert_eq!(g.t, t, "paused while choosing");
        g.step(
            0.1,
            &Input {
                choose: Some(0),
                ..idle()
            },
        );
        // Remaining XP immediately grants the next level.
        assert!(g.level >= 3);
    }

    #[test]
    fn choices_are_distinct() {
        let mut g = Game::new(5);
        for _ in 0..50 {
            let c = g.roll_choices();
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        }
    }

    #[test]
    fn lab_destruction_loses() {
        let mut g = Game::new(6);
        g.lab.hp = -1.0;
        g.step(0.016, &idle());
        assert_eq!(g.phase, Phase::LostLab);
        assert!(g.events.contains(&Event::GameOver));
    }

    #[test]
    fn surviving_wins() {
        let mut g = Game::new(7);
        g.t = WIN_TIME;
        g.enemies.clear();
        g.bosses_spawned = BOSS_TIMES.len();
        g.step(0.016, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn enemies_attack_undefended_lab() {
        let mut g = Game::new(8);
        g.cmd.pos = v2(100.0, 100.0);
        g.lab.repeat = None;
        g.spawn_enemy(EnemyKind::Thud);
        g.enemies[0].pos = LAB_POS + v2(300.0, 0.0);
        for _ in 0..300 {
            g.step(0.05, &idle());
        }
        assert!(g.lab.hp < g.lab.max_hp);
    }

    #[test]
    fn spawn_rate_escalates() {
        let mut g = Game::new(9);
        let r0 = g.spawn_rate();
        g.t = 300.0;
        assert!(g.spawn_rate() > r0 * 4.0);
    }

    #[test]
    fn rally_and_follow_orders() {
        let mut g = Game::new(10);
        let p = v2(500.0, 500.0);
        g.step(
            0.01,
            &Input {
                rally: Some(p),
                ..idle()
            },
        );
        assert_eq!(g.order, Order::Rally(p));
        g.step(
            0.01,
            &Input {
                follow: true,
                ..idle()
            },
        );
        assert_eq!(g.order, Order::Follow);
    }
}
