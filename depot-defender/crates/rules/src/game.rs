//! The game simulation: player, depot, supply lines, enemies, weapons and waves.

use crate::data::{Cargo, EnemyKind, START_TOWN, TOWNS, Upgrade};
use crate::math::V2;
use crate::rng::Rng;

/// Half extents of the rectangular arena, centred on the depot.
pub const ARENA: V2 = V2::new(1100.0, 800.0);
pub const DEPOT_RADIUS: f32 = 60.0;
pub const DEPOT_MAX_HP: f32 = 400.0;
/// Distance of a line's platform from the depot centre.
pub const PLATFORM_DIST: f32 = 125.0;
/// Where trains come from, off the edge of the arena.
pub const TRACK_LEN: f32 = 1600.0;
pub const UNLOAD_RANGE: f32 = 95.0;
pub const PLAYER_RADIUS: f32 = 14.0;
pub const FINAL_WAVE: u32 = 10;
pub const MAX_WEAPON_LEVEL: u32 = 5;
pub const MAX_LINE_LEVEL: u32 = 3;
pub const TRAIN_MOVE_TIME: f32 = 3.0;
pub const DOCK_TIME: f32 = 10.0;
pub const ORBIT_RADIUS: f32 = 85.0;
const SEEK_PLAYER_RANGE: f32 = 360.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp([Upgrade; 3]),
    /// Between waves: buy and extend lines on the network map.
    Shop,
    Won,
    Lost,
}

/// Player intent for one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub move_dir: V2,
    pub unload: bool,
}

/// Something the front end may want to show or play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot(Cargo),
    Hit(V2),
    Killed(V2, EnemyKind),
    Explosion(V2, f32),
    Gem,
    Crate(Cargo),
    WeaponUnlocked(Cargo),
    LevelUp,
    TrainArriving(usize),
    TrainDocked(usize),
    Unloaded(usize),
    TrainMissed(usize),
    LineBought(usize),
    LineExtended(usize),
    PlayerHurt,
    DepotHurt,
    WaveStart(u32),
    WaveCleared(u32),
    BossSpawned,
    Victory,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrainState {
    /// Seconds until the next train starts approaching.
    Away(f32),
    /// Approach progress 0..1.
    Arriving(f32),
    /// Seconds left at the platform.
    Docked(f32),
    /// Departure progress 0..1.
    Leaving(f32),
}

#[derive(Clone, Debug)]
pub struct Line {
    pub town: usize,
    pub cargo: Cargo,
    pub angle: f32,
    pub level: u32,
    pub state: TrainState,
}

impl Line {
    fn new(town: usize, first_arrival: f32) -> Self {
        let t = TOWNS[town];
        Self {
            town,
            cargo: t.cargo,
            angle: t.angle,
            level: 0,
            state: TrainState::Away(first_arrival),
        }
    }

    /// Crates per train.
    pub fn capacity(&self) -> u32 {
        3 + 2 * self.level
    }

    /// Seconds between departures and the next arrival.
    pub fn interval(&self) -> f32 {
        16.0 * 0.85f32.powi(self.level as i32)
    }

    /// Wagons drawn behind the locomotive.
    pub fn wagons(&self) -> u32 {
        2 + self.level
    }

    pub fn extend_cost(&self) -> u32 {
        70 + 50 * self.level
    }

    pub fn dir(&self) -> V2 {
        V2::from_angle(self.angle)
    }

    pub fn platform(&self) -> V2 {
        self.dir() * PLATFORM_DIST
    }

    pub fn far_end(&self) -> V2 {
        self.dir() * TRACK_LEN
    }

    /// Front of the locomotive, if a train is on the visible track.
    pub fn train_pos(&self) -> Option<V2> {
        let ease = |p: f32| 1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(2);
        match self.state {
            TrainState::Away(_) => None,
            TrainState::Arriving(p) => Some(self.far_end().lerp(self.platform(), ease(p))),
            TrainState::Docked(_) => Some(self.platform()),
            TrainState::Leaving(p) => Some(self.platform().lerp(self.far_end(), p * p)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub radius: f32,
    pub damage: f32,
    /// Seconds of white hit-flash left (for rendering).
    pub flash: f32,
    orbit_cd: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShotKind {
    Rivet { pierce: u32 },
    Shell { radius: f32 },
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub pos: V2,
    pub vel: V2,
    pub damage: f32,
    pub life: f32,
    pub kind: ShotKind,
    hits: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupKind {
    Gem(u32),
    Crate(Cargo),
}

#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub pos: V2,
    pub kind: PickupKind,
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: V2,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub magnet: f32,
    pub invuln: f32,
    pub xp: u32,
    pub xp_next: u32,
    pub level: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub kills: u32,
    pub crates: u32,
    pub trains_unloaded: u32,
    pub trains_missed: u32,
    pub time: f32,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub phase: Phase,
    pub player: Player,
    pub depot_hp: f32,
    pub lines: Vec<Line>,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub pickups: Vec<Pickup>,
    /// Weapon level per cargo (0 = locked).
    pub weapons: [u32; 3],
    cooldowns: [f32; 3],
    pub haste: f32,
    pub orbit_angle: f32,
    pub cash: u32,
    pub cargo_stock: [u32; 3],
    pub wave: u32,
    pub wave_time: f32,
    pub wave_len: f32,
    spawn_acc: f32,
    pending_levels: u32,
    next_id: u32,
    pub stats: Stats,
    pub events: Vec<Event>,
    rng: Rng,
}

pub fn wave_length(wave: u32) -> f32 {
    (35.0 + 5.0 * wave as f32).min(75.0)
}

/// Enemies spawned per second at `elapsed` seconds into `wave`.
pub fn spawn_rate(wave: u32, elapsed: f32) -> f32 {
    1.1 + 0.65 * (wave as f32 - 1.0) + 0.025 * elapsed
}

/// Enemy health multiplier for a wave.
pub fn hp_scale(wave: u32) -> f32 {
    1.0 + 0.33 * (wave as f32 - 1.0)
}

pub fn is_boss_wave(wave: u32) -> bool {
    wave == 5 || wave == FINAL_WAVE
}

fn xp_for_level(level: u32) -> u32 {
    5 + level * 6
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Self {
            phase: Phase::Playing,
            player: Player {
                pos: V2::new(0.0, 150.0),
                facing: V2::new(1.0, 0.0),
                hp: 100.0,
                max_hp: 100.0,
                speed: 190.0,
                magnet: 80.0,
                invuln: 0.0,
                xp: 0,
                xp_next: xp_for_level(1),
                level: 1,
            },
            depot_hp: DEPOT_MAX_HP,
            lines: vec![Line::new(START_TOWN, 4.0)],
            enemies: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            weapons: [0; 3],
            cooldowns: [0.0; 3],
            haste: 1.0,
            orbit_angle: 0.0,
            cash: 0,
            cargo_stock: [0; 3],
            wave: 0,
            wave_time: 0.0,
            wave_len: 0.0,
            spawn_acc: 0.0,
            pending_levels: 0,
            next_id: 1,
            stats: Stats::default(),
            events: Vec::new(),
            rng: Rng::new(seed),
        };
        g.weapons[Cargo::Steel.index()] = 1;
        g.start_wave();
        g
    }

    fn start_wave(&mut self) {
        self.wave += 1;
        self.wave_len = wave_length(self.wave);
        self.wave_time = self.wave_len;
        self.spawn_acc = 0.0;
        self.phase = Phase::Playing;
        for (i, line) in self.lines.iter_mut().enumerate() {
            line.state = TrainState::Away(3.0 + 4.0 * i as f32);
        }
        self.events.push(Event::WaveStart(self.wave));
        if is_boss_wave(self.wave) {
            let pos = self.edge_point();
            self.spawn(EnemyKind::Boss, pos);
            self.events.push(Event::BossSpawned);
        }
    }

    pub fn boss_alive(&self) -> bool {
        self.enemies.iter().any(|e| e.kind == EnemyKind::Boss)
    }

    /// Advance the simulation by `dt` seconds. Does nothing outside [`Phase::Playing`].
    pub fn step(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        if self.pending_levels > 0 {
            self.pending_levels -= 1;
            self.offer_level_up();
            return;
        }
        self.stats.time += dt;
        self.wave_time = (self.wave_time - dt).max(0.0);
        self.spawn_enemies(dt);
        self.move_player(dt, input);
        self.update_lines(dt, input.unload);
        self.fire_weapons(dt);
        self.update_projectiles(dt);
        self.orbit_hits(dt);
        self.move_enemies(dt);
        self.reap_enemies();
        self.update_pickups(dt);
        self.check_progress();
    }

    fn edge_point(&mut self) -> V2 {
        let r = &mut self.rng;
        match r.below(4) {
            0 => V2::new(r.range(-ARENA.x, ARENA.x), -ARENA.y),
            1 => V2::new(r.range(-ARENA.x, ARENA.x), ARENA.y),
            2 => V2::new(-ARENA.x, r.range(-ARENA.y, ARENA.y)),
            _ => V2::new(ARENA.x, r.range(-ARENA.y, ARENA.y)),
        }
    }

    pub fn spawn(&mut self, kind: EnemyKind, pos: V2) {
        let s = kind.stats();
        let hp = s.hp * hp_scale(self.wave);
        self.enemies.push(Enemy {
            id: self.next_id,
            kind,
            pos,
            hp,
            max_hp: hp,
            speed: s.speed * self.rng.range(0.9, 1.1),
            radius: s.radius,
            damage: s.damage,
            flash: 0.0,
            orbit_cd: 0.0,
        });
        self.next_id += 1;
    }

    fn spawn_enemies(&mut self, dt: f32) {
        if self.wave_time <= 0.0 {
            return;
        }
        let elapsed = self.wave_len - self.wave_time;
        self.spawn_acc += spawn_rate(self.wave, elapsed) * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let w = self.wave as f32;
            let roll = self.rng.f32();
            let kind = if self.wave >= 3 && roll < 0.06 + 0.012 * w {
                EnemyKind::Brute
            } else if self.wave >= 2 && roll < 0.35 {
                EnemyKind::Sprinter
            } else {
                EnemyKind::Rustbug
            };
            // Spawn in small packs along one edge.
            let base = self.edge_point();
            let pack = if kind == EnemyKind::Brute {
                1
            } else {
                1 + self.rng.below(2)
            };
            for _ in 0..pack {
                let jitter = V2::new(self.rng.range(-30.0, 30.0), self.rng.range(-30.0, 30.0));
                self.spawn(kind, base + jitter);
            }
        }
    }

    fn move_player(&mut self, dt: f32, input: &Input) {
        let p = &mut self.player;
        p.invuln = (p.invuln - dt).max(0.0);
        let dir = input.move_dir.norm();
        if dir != V2::ZERO {
            p.facing = dir;
        }
        p.pos += dir * (p.speed * dt);
        p.pos.x = p.pos.x.clamp(-ARENA.x, ARENA.x);
        p.pos.y = p.pos.y.clamp(-ARENA.y, ARENA.y);
        // The depot building is solid.
        let min = DEPOT_RADIUS + PLAYER_RADIUS;
        let d = p.pos.len();
        if d < min {
            p.pos = if d > 1e-3 {
                p.pos * (min / d)
            } else {
                V2::new(min, 0.0)
            };
        }
    }

    /// Index of a docked line whose platform the player is standing at.
    pub fn unloadable_line(&self) -> Option<usize> {
        self.lines.iter().position(|l| {
            matches!(l.state, TrainState::Docked(_))
                && l.platform().dist(self.player.pos) <= UNLOAD_RANGE
        })
    }

    fn update_lines(&mut self, dt: f32, unload: bool) {
        let target = if unload { self.unloadable_line() } else { None };
        for i in 0..self.lines.len() {
            let interval = self.lines[i].interval();
            let state = self.lines[i].state;
            self.lines[i].state = match state {
                TrainState::Away(t) if t - dt <= 0.0 => {
                    self.events.push(Event::TrainArriving(i));
                    TrainState::Arriving(0.0)
                }
                TrainState::Away(t) => TrainState::Away(t - dt),
                TrainState::Arriving(p) if p + dt / TRAIN_MOVE_TIME >= 1.0 => {
                    self.events.push(Event::TrainDocked(i));
                    TrainState::Docked(DOCK_TIME)
                }
                TrainState::Arriving(p) => TrainState::Arriving(p + dt / TRAIN_MOVE_TIME),
                TrainState::Docked(_) if target == Some(i) => {
                    self.unload(i);
                    TrainState::Leaving(0.0)
                }
                TrainState::Docked(t) if t - dt <= 0.0 => {
                    self.stats.trains_missed += 1;
                    self.events.push(Event::TrainMissed(i));
                    TrainState::Leaving(0.0)
                }
                TrainState::Docked(t) => TrainState::Docked(t - dt),
                TrainState::Leaving(p) if p + dt / TRAIN_MOVE_TIME >= 1.0 => {
                    TrainState::Away(interval)
                }
                TrainState::Leaving(p) => TrainState::Leaving(p + dt / TRAIN_MOVE_TIME),
            };
        }
    }

    fn unload(&mut self, i: usize) {
        let line = &self.lines[i];
        let (cargo, n, platform, dir) = (line.cargo, line.capacity(), line.platform(), line.dir());
        let side = V2::new(-dir.y, dir.x);
        for k in 0..n {
            let along = dir * (20.0 + 18.0 * (k / 2) as f32);
            let off = side * if k % 2 == 0 { 34.0 } else { -34.0 };
            self.pickups.push(Pickup {
                pos: platform + along + off,
                kind: PickupKind::Crate(cargo),
                age: 0.0,
            });
        }
        self.cash += 10;
        self.stats.trains_unloaded += 1;
        self.events.push(Event::Unloaded(i));
    }

    fn nearest_enemy(&self, from: V2, max: f32) -> Option<usize> {
        let mut best = None;
        let mut best_d = max;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist(from);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        best
    }

    fn fire_weapons(&mut self, dt: f32) {
        self.orbit_angle += dt * (2.2 + 0.25 * self.weapons[Cargo::Goods.index()] as f32);
        for c in [Cargo::Steel, Cargo::Coal] {
            let lvl = self.weapons[c.index()];
            if lvl == 0 {
                continue;
            }
            self.cooldowns[c.index()] -= dt;
            if self.cooldowns[c.index()] > 0.0 {
                continue;
            }
            let from = self.player.pos;
            let Some(t) = self.nearest_enemy(from, 520.0) else {
                self.cooldowns[c.index()] = 0.0;
                continue;
            };
            let target = self.enemies[t].pos;
            let l = lvl as f32;
            match c {
                Cargo::Steel => {
                    self.cooldowns[c.index()] = (0.55 - 0.05 * l) / self.haste;
                    let count = 1 + u32::from(lvl >= 3) + u32::from(lvl >= 5);
                    let base = (target - from).angle();
                    for k in 0..count {
                        let spread = (k as f32 - (count - 1) as f32 / 2.0) * 0.14;
                        self.projectiles.push(Projectile {
                            pos: from,
                            vel: V2::from_angle(base + spread) * 560.0,
                            damage: 8.0 + 3.0 * l,
                            life: 1.1,
                            kind: ShotKind::Rivet { pierce: lvl / 2 },
                            hits: Vec::new(),
                        });
                    }
                }
                Cargo::Coal => {
                    self.cooldowns[c.index()] = (1.7 - 0.15 * l) / self.haste;
                    let shells = 1 + u32::from(lvl >= 4);
                    for k in 0..shells {
                        let aim = if k == 0 {
                            target
                        } else {
                            let n = self.enemies.len();
                            self.enemies[self.rng.below(n)].pos
                        };
                        let d = aim.dist(from).max(1.0);
                        self.projectiles.push(Projectile {
                            pos: from,
                            vel: (aim - from) * (380.0 / d),
                            damage: 14.0 + 7.0 * l,
                            life: d / 380.0,
                            kind: ShotKind::Shell {
                                radius: 55.0 + 10.0 * l,
                            },
                            hits: Vec::new(),
                        });
                    }
                }
                Cargo::Goods => {}
            }
            self.events.push(Event::Shot(c));
        }
    }

    /// World positions of the orbiting parcel crates.
    pub fn orbit_positions(&self) -> Vec<V2> {
        let lvl = self.weapons[Cargo::Goods.index()];
        if lvl == 0 {
            return Vec::new();
        }
        let n = 1 + lvl;
        (0..n)
            .map(|k| {
                let a = self.orbit_angle + k as f32 * std::f32::consts::TAU / n as f32;
                self.player.pos + V2::from_angle(a) * ORBIT_RADIUS
            })
            .collect()
    }

    fn orbit_hits(&mut self, dt: f32) {
        let lvl = self.weapons[Cargo::Goods.index()];
        let crates = self.orbit_positions();
        let dmg = 7.0 + 4.0 * lvl as f32;
        for e in &mut self.enemies {
            e.orbit_cd -= dt;
            e.flash = (e.flash - dt).max(0.0);
            if e.orbit_cd > 0.0 {
                continue;
            }
            if crates.iter().any(|c| c.dist(e.pos) < e.radius + 12.0) {
                e.hp -= dmg;
                e.flash = 0.1;
                e.orbit_cd = 0.4;
                self.events.push(Event::Hit(e.pos));
            }
        }
    }

    fn update_projectiles(&mut self, dt: f32) {
        let mut projectiles = std::mem::take(&mut self.projectiles);
        projectiles.retain_mut(|p| {
            p.pos += p.vel * dt;
            p.life -= dt;
            match p.kind {
                ShotKind::Rivet { pierce } => {
                    for e in &mut self.enemies {
                        if e.hp <= 0.0 || p.hits.contains(&e.id) {
                            continue;
                        }
                        if e.pos.dist(p.pos) < e.radius + 4.0 {
                            e.hp -= p.damage;
                            e.flash = 0.1;
                            p.hits.push(e.id);
                            self.events.push(Event::Hit(e.pos));
                            if p.hits.len() as u32 > pierce {
                                return false;
                            }
                        }
                    }
                    p.life > 0.0
                }
                ShotKind::Shell { radius } => {
                    if p.life > 0.0 {
                        return true;
                    }
                    for e in &mut self.enemies {
                        if e.pos.dist(p.pos) < radius + e.radius {
                            e.hp -= p.damage;
                            e.flash = 0.15;
                        }
                    }
                    self.events.push(Event::Explosion(p.pos, radius));
                    false
                }
            }
        });
        self.projectiles = projectiles;
    }

    fn move_enemies(&mut self, dt: f32) {
        let player = self.player.pos;
        let mut depot_damage = 0.0;
        let mut player_damage: f32 = 0.0;
        for e in &mut self.enemies {
            let to_player = player - e.pos;
            let target = if to_player.len() < SEEK_PLAYER_RANGE || e.kind == EnemyKind::Sprinter {
                player
            } else {
                V2::ZERO
            };
            let dir = (target - e.pos).norm();
            e.pos += dir * (e.speed * dt);
            let d = e.pos.len();
            let min = DEPOT_RADIUS + e.radius;
            if d < min {
                e.pos = e.pos * (min / d.max(1e-3));
                depot_damage += e.damage * dt;
            }
            if to_player.len() < e.radius + PLAYER_RADIUS {
                player_damage = player_damage.max(e.damage);
            }
        }
        // Cheap separation so swarms spread into a crowd instead of a single dot.
        let n = self.enemies.len();
        if n <= 450 {
            for i in 0..n {
                for j in (i + 1)..n {
                    let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
                    let min = (self.enemies[i].radius + self.enemies[j].radius) * 0.9;
                    let delta = b - a;
                    let d = delta.len();
                    if d < min && d > 1e-3 {
                        let push = delta * ((min - d) / d * 0.5);
                        self.enemies[i].pos -= push;
                        self.enemies[j].pos += push;
                    }
                }
            }
        }
        if depot_damage > 0.0 {
            self.depot_hp -= depot_damage;
            self.events.push(Event::DepotHurt);
        }
        if player_damage > 0.0 && self.player.invuln <= 0.0 {
            self.player.hp -= player_damage;
            self.player.invuln = 0.6;
            self.events.push(Event::PlayerHurt);
        }
    }

    fn reap_enemies(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let e = self.enemies.swap_remove(i);
            self.stats.kills += 1;
            self.cash += if e.kind == EnemyKind::Boss { 150 } else { 1 };
            self.events.push(Event::Killed(e.pos, e.kind));
            let xp = e.kind.stats().xp;
            self.pickups.push(Pickup {
                pos: e.pos,
                kind: PickupKind::Gem(xp),
                age: 0.0,
            });
            if e.kind == EnemyKind::Boss {
                for c in Cargo::ALL {
                    self.pickups.push(Pickup {
                        pos: e.pos + V2::from_angle(c.index() as f32 * 2.1) * 30.0,
                        kind: PickupKind::Crate(c),
                        age: 0.0,
                    });
                }
            }
        }
    }

    fn update_pickups(&mut self, dt: f32) {
        let player = self.player.pos;
        let magnet = self.player.magnet;
        let mut collected = Vec::new();
        self.pickups.retain_mut(|p| {
            p.age += dt;
            let d = p.pos.dist(player);
            if d < PLAYER_RADIUS + 10.0 {
                collected.push(p.kind);
                return false;
            }
            if d < magnet {
                p.pos += (player - p.pos).norm() * (380.0 * dt);
            }
            true
        });
        for kind in collected {
            self.collect(kind);
        }
    }

    fn collect(&mut self, kind: PickupKind) {
        match kind {
            PickupKind::Gem(xp) => {
                self.gain_xp(xp);
                self.events.push(Event::Gem);
            }
            PickupKind::Crate(c) => {
                self.cash += 4;
                self.cargo_stock[c.index()] += 1;
                self.stats.crates += 1;
                self.events.push(Event::Crate(c));
                if self.weapons[c.index()] == 0 {
                    self.weapons[c.index()] = 1;
                    self.events.push(Event::WeaponUnlocked(c));
                }
                self.gain_xp(3);
            }
        }
    }

    fn gain_xp(&mut self, xp: u32) {
        let p = &mut self.player;
        p.xp += xp;
        while p.xp >= p.xp_next {
            p.xp -= p.xp_next;
            p.level += 1;
            p.xp_next = xp_for_level(p.level);
            self.pending_levels += 1;
        }
    }

    fn offer_level_up(&mut self) {
        let mut pool: Vec<Upgrade> = Vec::new();
        for c in Cargo::ALL {
            let lvl = self.weapons[c.index()];
            let owned = self.lines.iter().any(|l| l.cargo == c);
            if (lvl > 0 || owned) && lvl < MAX_WEAPON_LEVEL {
                // Weapons are weighted so they show up more often.
                pool.push(Upgrade::Weapon(c));
                pool.push(Upgrade::Weapon(c));
            }
        }
        pool.extend([
            Upgrade::Boots,
            Upgrade::Plating,
            Upgrade::Magnet,
            Upgrade::Overtime,
        ]);
        if self.depot_hp < DEPOT_MAX_HP * 0.9 {
            pool.push(Upgrade::Repair);
        }
        let mut choices = Vec::with_capacity(3);
        while choices.len() < 3 {
            let u = pool[self.rng.below(pool.len())];
            if !choices.contains(&u) {
                choices.push(u);
            }
        }
        self.phase = Phase::LevelUp([choices[0], choices[1], choices[2]]);
        self.events.push(Event::LevelUp);
    }

    /// Apply the level-up choice at `index` (0..3).
    pub fn choose_upgrade(&mut self, index: usize) {
        let Phase::LevelUp(choices) = self.phase.clone() else {
            return;
        };
        let Some(&u) = choices.get(index) else {
            return;
        };
        match u {
            Upgrade::Weapon(c) => {
                let l = &mut self.weapons[c.index()];
                *l = (*l + 1).min(MAX_WEAPON_LEVEL);
            }
            Upgrade::Boots => self.player.speed *= 1.12,
            Upgrade::Plating => {
                self.player.max_hp += 20.0;
                self.player.hp = (self.player.hp + 20.0).min(self.player.max_hp);
            }
            Upgrade::Repair => {
                self.depot_hp = (self.depot_hp + DEPOT_MAX_HP * 0.3).min(DEPOT_MAX_HP);
            }
            Upgrade::Magnet => self.player.magnet += 35.0,
            Upgrade::Overtime => self.haste *= 1.1,
        }
        self.phase = Phase::Playing;
    }

    fn check_progress(&mut self) {
        if self.player.hp <= 0.0 || self.depot_hp <= 0.0 {
            self.player.hp = self.player.hp.max(0.0);
            self.depot_hp = self.depot_hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::GameOver);
            return;
        }
        if self.wave_time > 0.0 || self.boss_alive() {
            return;
        }
        // Wave over: the remaining swarm scatters, loose pickups are banked.
        for e in std::mem::take(&mut self.enemies) {
            self.events.push(Event::Killed(e.pos, e.kind));
        }
        self.projectiles.clear();
        for p in std::mem::take(&mut self.pickups) {
            self.collect(p.kind);
        }
        self.events.push(Event::WaveCleared(self.wave));
        if self.wave >= FINAL_WAVE {
            self.phase = Phase::Won;
            self.events.push(Event::Victory);
            return;
        }
        self.cash += 30 + 10 * self.wave;
        self.depot_hp = (self.depot_hp + DEPOT_MAX_HP * 0.15).min(DEPOT_MAX_HP);
        self.player.hp = (self.player.hp + 25.0).min(self.player.max_hp);
        self.phase = Phase::Shop;
    }

    pub fn owns_town(&self, town: usize) -> bool {
        self.lines.iter().any(|l| l.town == town)
    }

    /// Lease a new line to `town`. Only allowed in the shop.
    pub fn buy_line(&mut self, town: usize) -> bool {
        if self.phase != Phase::Shop || town >= TOWNS.len() || self.owns_town(town) {
            return false;
        }
        let cost = TOWNS[town].cost;
        if self.cash < cost {
            return false;
        }
        self.cash -= cost;
        self.lines.push(Line::new(town, 0.0));
        self.events.push(Event::LineBought(town));
        true
    }

    /// Run bigger, more frequent trains on an owned line. Only allowed in the shop.
    pub fn extend_line(&mut self, line: usize) -> bool {
        if self.phase != Phase::Shop {
            return false;
        }
        let Some(l) = self.lines.get_mut(line) else {
            return false;
        };
        let cost = l.extend_cost();
        if l.level >= MAX_LINE_LEVEL || self.cash < cost {
            return false;
        }
        self.cash -= cost;
        l.level += 1;
        self.events.push(Event::LineExtended(line));
        true
    }

    /// Leave the shop and start the next wave.
    pub fn depart(&mut self) {
        if self.phase == Phase::Shop {
            self.start_wave();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn idle(g: &mut Game, secs: f32) {
        let steps = (secs / DT) as usize;
        for _ in 0..steps {
            g.step(DT, &Input::default());
            if let Phase::LevelUp(_) = g.phase {
                g.choose_upgrade(0);
            }
        }
    }

    #[test]
    fn starts_in_wave_one_with_rivet_gun() {
        let g = Game::new(1);
        assert_eq!(g.wave, 1);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.weapons, [1, 0, 0]);
        assert_eq!(g.lines.len(), 1);
    }

    #[test]
    fn waves_escalate() {
        assert!(spawn_rate(5, 0.0) > spawn_rate(1, 0.0));
        assert!(hp_scale(8) > hp_scale(2));
        assert!(wave_length(3) > wave_length(1));
        assert!(is_boss_wave(FINAL_WAVE));
    }

    #[test]
    fn player_is_clamped_to_arena_and_out_of_depot() {
        let mut g = Game::new(1);
        g.enemies.clear();
        let input = Input {
            move_dir: V2::new(1.0, 1.0),
            unload: false,
        };
        for _ in 0..2000 {
            g.player.hp = 100.0;
            g.step(DT, &input);
            if let Phase::LevelUp(_) = g.phase {
                g.choose_upgrade(0);
            }
        }
        assert!(g.player.pos.x <= ARENA.x && g.player.pos.y <= ARENA.y);
        g.phase = Phase::Playing;
        g.pending_levels = 0;
        g.player.pos = V2::new(1.0, 0.0);
        g.step(DT, &Input::default());
        assert!(g.player.pos.len() >= DEPOT_RADIUS + PLAYER_RADIUS - 0.01);
    }

    #[test]
    fn train_arrives_docks_and_can_be_unloaded() {
        let mut g = Game::new(3);
        let platform = g.lines[0].platform();
        let mut docked = false;
        for _ in 0..(12.0 / DT) as usize {
            g.player.hp = 100.0;
            g.depot_hp = DEPOT_MAX_HP;
            g.step(DT, &Input::default());
            if matches!(g.lines[0].state, TrainState::Docked(_)) {
                docked = true;
                break;
            }
        }
        assert!(docked, "train should dock within 12 s");
        g.player.pos = platform + V2::new(0.0, 10.0);
        assert_eq!(g.unloadable_line(), Some(0));
        g.step(
            DT,
            &Input {
                move_dir: V2::ZERO,
                unload: true,
            },
        );
        assert!(matches!(g.lines[0].state, TrainState::Leaving(_)));
        let crates = g
            .pickups
            .iter()
            .filter(|p| matches!(p.kind, PickupKind::Crate(_)))
            .count();
        assert_eq!(crates as u32, g.lines[0].capacity());
        assert!(g.events.contains(&Event::Unloaded(0)));
    }

    #[test]
    fn missed_train_leaves() {
        let mut g = Game::new(3);
        g.lines[0].state = TrainState::Docked(0.01);
        g.player.pos = V2::new(900.0, 700.0);
        g.step(DT, &Input::default());
        assert!(matches!(g.lines[0].state, TrainState::Leaving(_)));
        assert_eq!(g.stats.trains_missed, 1);
    }

    #[test]
    fn collecting_a_new_cargo_unlocks_its_weapon() {
        let mut g = Game::new(5);
        g.collect(PickupKind::Crate(Cargo::Coal));
        assert_eq!(g.weapons[Cargo::Coal.index()], 1);
        assert!(g.events.contains(&Event::WeaponUnlocked(Cargo::Coal)));
        assert_eq!(g.cargo_stock[Cargo::Coal.index()], 1);
    }

    #[test]
    fn rivet_gun_kills_an_approaching_enemy() {
        let mut g = Game::new(9);
        g.enemies.clear();
        g.wave_time = 1000.0;
        g.spawn_acc = -1e9;
        g.spawn(EnemyKind::Rustbug, g.player.pos + V2::new(200.0, 0.0));
        idle(&mut g, 2.0);
        assert_eq!(g.stats.kills, 1);
        assert!(
            g.pickups
                .iter()
                .any(|p| matches!(p.kind, PickupKind::Gem(_)))
        );
    }

    #[test]
    fn xp_triggers_level_up_choice() {
        let mut g = Game::new(2);
        g.gain_xp(100);
        g.step(DT, &Input::default());
        let Phase::LevelUp(choices) = g.phase.clone() else {
            panic!("expected level up");
        };
        assert_ne!(choices[0], choices[1]);
        assert_ne!(choices[1], choices[2]);
        g.choose_upgrade(0);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn depot_destruction_loses() {
        let mut g = Game::new(4);
        g.depot_hp = 0.5;
        g.spawn(EnemyKind::Brute, V2::new(DEPOT_RADIUS + 5.0, 0.0));
        idle(&mut g, 1.0);
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.events.contains(&Event::GameOver));
    }

    #[test]
    fn wave_end_opens_shop_and_lines_can_be_bought() {
        let mut g = Game::new(6);
        g.wave_time = 0.0;
        g.enemies.clear();
        g.step(DT, &Input::default());
        assert_eq!(g.phase, Phase::Shop);
        g.cash = 0;
        assert!(!g.buy_line(1), "cannot buy without cash");
        g.cash = 1000;
        assert!(g.buy_line(1));
        assert!(!g.buy_line(1), "cannot buy twice");
        assert!(g.extend_line(0));
        assert_eq!(g.lines[0].level, 1);
        assert_eq!(g.lines[0].capacity(), 5);
        g.depart();
        assert_eq!(g.wave, 2);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn boss_wave_needs_the_boss_dead_and_final_wave_wins() {
        let mut g = Game::new(7);
        g.wave = FINAL_WAVE - 1;
        g.phase = Phase::Shop;
        g.depart();
        assert!(g.boss_alive());
        g.wave_time = 0.0;
        g.player.pos = V2::new(-1000.0, -700.0);
        g.step(DT, &Input::default());
        assert_eq!(g.phase, Phase::Playing, "boss still alive");
        for e in &mut g.enemies {
            e.hp = -1.0;
        }
        g.step(DT, &Input::default());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn a_long_idle_run_eventually_ends() {
        let mut g = Game::new(11);
        idle(&mut g, 600.0);
        assert!(matches!(g.phase, Phase::Lost | Phase::Shop));
    }
}
