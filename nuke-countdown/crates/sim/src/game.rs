//! Game rules for Nuke Countdown.
//!
//! The player gathers metal and energy, feeds the Nuclear Silo at the map
//! centre and defends it. A charged silo can launch, wiping the map and
//! starting a harder cycle. Each cycle the enemy Juggernaut marches on the
//! silo when its countdown expires, and from the second cycle on the enemy
//! fires nukes back, which only an Anti-Nuke can intercept.

use crate::math::{V2, v2};
use crate::rng::Rng;

pub const WORLD: f32 = 1400.0;
pub const PLAYER_RADIUS: f32 = 14.0;
pub const SILO_RADIUS: f32 = 44.0;
/// Launches needed to win.
pub const CYCLES: u32 = 5;
/// Distance from the silo within which cargo can be deposited.
pub const DEPOSIT_RANGE: f32 = 110.0;
pub const PICKUP_RANGE: f32 = 70.0;
pub const NUKE_FLIGHT: f32 = 3.0;
pub const ENEMY_NUKE_FLIGHT: f32 = 8.0;
pub const ENEMY_NUKE_RADIUS: f32 = 260.0;
pub const ANTI_CHARGE: f32 = 30.0;
pub const MINE_RATE: f32 = 6.0;

/// Silo metal needed for launch number `cycle` (1-based).
pub fn metal_needed(cycle: u32) -> u32 {
    60 + 40 * (cycle - 1)
}

/// Silo energy needed for launch number `cycle`.
pub fn energy_needed(cycle: u32) -> u32 {
    80 + 50 * (cycle - 1)
}

/// Seconds before the Juggernaut starts marching in cycle `cycle`.
pub fn jugg_timer(cycle: u32) -> f32 {
    (150.0 - 12.0 * (cycle - 1) as f32).max(90.0)
}

pub fn xp_needed(level: u32) -> u32 {
    5 + level * 4
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildKind {
    Turret,
    Solar,
    AntiNuke,
}

impl BuildKind {
    pub const ALL: [BuildKind; 3] = [BuildKind::Turret, BuildKind::Solar, BuildKind::AntiNuke];

    pub fn cost(self) -> u32 {
        match self {
            BuildKind::Turret => 12,
            BuildKind::Solar => 10,
            BuildKind::AntiNuke => 35,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BuildKind::Turret => "LLT Turret",
            BuildKind::Solar => "Solar Collector",
            BuildKind::AntiNuke => "Anti-Nuke",
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            BuildKind::Turret => 120.0,
            BuildKind::Solar => 80.0,
            BuildKind::AntiNuke => 200.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            BuildKind::Turret => 14.0,
            BuildKind::Solar => 16.0,
            BuildKind::AntiNuke => 22.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildKind,
    pub pos: V2,
    pub hp: f32,
    pub cd: f32,
    /// Anti-Nuke interceptor charge, `0..=ANTI_CHARGE`.
    pub charge: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Grunt,
    Raider,
    Brute,
    Juggernaut,
}

impl EnemyKind {
    /// (hp, speed, radius, contact dps, xp)
    pub fn stats(self) -> (f32, f32, f32, f32, u32) {
        match self {
            EnemyKind::Grunt => (14.0, 70.0, 11.0, 10.0, 1),
            EnemyKind::Raider => (8.0, 130.0, 9.0, 8.0, 1),
            EnemyKind::Brute => (60.0, 45.0, 18.0, 25.0, 4),
            EnemyKind::Juggernaut => (1500.0, 22.0, 40.0, 120.0, 40),
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
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub turret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Metal,
    Energy,
    Xp,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
    pub pos: V2,
    pub amount: u32,
}

#[derive(Clone, Debug)]
pub struct Deposit {
    pub pos: V2,
    pub metal: f32,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub fire_cd: f32,
    pub fire_rate: f32,
    pub range: f32,
    pub multishot: u32,
    pub metal: u32,
    pub energy: u32,
    pub cargo_max: u32,
    pub mine_acc: f32,
    pub hurt: f32,
    pub regen: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Multishot,
    Speed,
    Cargo,
    Armor,
    Range,
    Regen,
}

impl Upgrade {
    pub const ALL: [Upgrade; 8] = [
        Upgrade::Damage,
        Upgrade::FireRate,
        Upgrade::Multishot,
        Upgrade::Speed,
        Upgrade::Cargo,
        Upgrade::Armor,
        Upgrade::Range,
        Upgrade::Regen,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavy rounds: +30% damage",
            Upgrade::FireRate => "Autoloader: +25% fire rate",
            Upgrade::Multishot => "Twin barrel: +1 projectile",
            Upgrade::Speed => "Servos: +12% move speed",
            Upgrade::Cargo => "Storage pod: +15 cargo",
            Upgrade::Armor => "Armor plates: +30 max HP, heal",
            Upgrade::Range => "Targeting: +20% range",
            Upgrade::Regen => "Nano repair: +1.5 HP/s",
        }
    }
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
    TurretShot,
    Hit,
    Kill { pos: V2, big: bool },
    PlayerHurt,
    Pickup(PickupKind),
    Deposited,
    Denied,
    Built(BuildKind),
    BuildingLost,
    LevelUp,
    Armed,
    Launch,
    Detonate { cycle: u32 },
    JuggernautSpawn,
    EnemyNukeLaunch,
    Intercepted { pos: V2 },
    EnemyNukeHit { pos: V2 },
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct EnemyNuke {
    pub from: V2,
    pub to: V2,
    pub t: f32,
}

impl EnemyNuke {
    pub fn pos(&self) -> V2 {
        let k = (self.t / ENEMY_NUKE_FLIGHT).min(1.0);
        self.from + (self.to - self.from) * k
    }
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub deposit: bool,
    pub launch: bool,
    pub build: Option<BuildKind>,
    pub choice: Option<usize>,
}

pub struct Game {
    pub rng: Rng,
    pub phase: Phase,
    pub time: f32,
    pub player: Player,
    pub silo_hp: f32,
    pub silo_max: f32,
    pub silo_metal: u32,
    pub silo_energy: u32,
    pub energy_acc: f32,
    /// Launch number currently being prepared (1-based).
    pub cycle: u32,
    pub launches: u32,
    pub jugg_clock: f32,
    pub jugg_spawned: bool,
    /// Our nuke in flight, seconds since launch.
    pub nuke: Option<f32>,
    pub enemy_nuke: Option<EnemyNuke>,
    pub enemy_nuke_clock: f32,
    pub buildings: Vec<Building>,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub pickups: Vec<Pickup>,
    pub deposits: Vec<Deposit>,
    pub spawn_acc: f32,
    pub xp: u32,
    pub level: u32,
    pub kills: u32,
    pub choices: Vec<Upgrade>,
    pub events: Vec<Event>,
    armed_announced: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut deposits = Vec::new();
        for i in 0..14 {
            let a = i as f32 / 14.0 * std::f32::consts::TAU + rng.range(-0.2, 0.2);
            let d = rng.range(280.0, WORLD - 150.0);
            deposits.push(Deposit {
                pos: V2::from_angle(a) * d,
                metal: rng.range(40.0, 80.0),
            });
        }
        Game {
            rng,
            phase: Phase::Playing,
            time: 0.0,
            player: Player {
                pos: v2(0.0, 120.0),
                hp: 100.0,
                max_hp: 100.0,
                speed: 220.0,
                dmg: 7.0,
                fire_cd: 0.0,
                fire_rate: 3.0,
                range: 300.0,
                multishot: 1,
                metal: 0,
                energy: 0,
                cargo_max: 30,
                mine_acc: 0.0,
                hurt: 0.0,
                regen: 0.0,
            },
            silo_hp: 1000.0,
            silo_max: 1000.0,
            silo_metal: 0,
            silo_energy: 0,
            energy_acc: 0.0,
            cycle: 1,
            launches: 0,
            jugg_clock: jugg_timer(1),
            jugg_spawned: false,
            nuke: None,
            enemy_nuke: None,
            enemy_nuke_clock: 0.0,
            buildings: Vec::new(),
            enemies: Vec::new(),
            bullets: Vec::new(),
            pickups: Vec::new(),
            deposits,
            spawn_acc: 0.0,
            xp: 0,
            level: 1,
            kills: 0,
            choices: Vec::new(),
            events: Vec::new(),
            armed_announced: false,
        }
    }

    pub fn armed(&self) -> bool {
        self.silo_metal >= metal_needed(self.cycle) && self.silo_energy >= energy_needed(self.cycle)
    }

    pub fn near_silo(&self) -> bool {
        self.player.pos.len() < DEPOSIT_RANGE
    }

    pub fn cargo(&self) -> u32 {
        self.player.metal + self.player.energy
    }

    pub fn solars(&self) -> usize {
        self.count(BuildKind::Solar)
    }

    pub fn count(&self, k: BuildKind) -> usize {
        self.buildings.iter().filter(|b| b.kind == k).count()
    }

    /// Ready Anti-Nuke interceptors.
    pub fn interceptors(&self) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.kind == BuildKind::AntiNuke && b.charge >= ANTI_CHARGE)
            .count()
    }

    /// Enemy spawns per second at the current difficulty.
    pub fn spawn_rate(&self) -> f32 {
        let cycle_t = jugg_timer(self.cycle) - self.jugg_clock;
        0.6 + 0.45 * (self.cycle - 1) as f32 + cycle_t / 60.0
    }

    pub fn can_place(&self, pos: V2, kind: BuildKind) -> bool {
        if pos.len() < SILO_RADIUS + kind.radius() + 6.0 || pos.len() > WORLD - 20.0 {
            return false;
        }
        self.buildings
            .iter()
            .all(|b| b.pos.dist(pos) > b.kind.radius() + kind.radius() + 4.0)
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        match self.phase {
            Phase::LevelUp => {
                if let Some(c) = input.choice
                    && c < self.choices.len()
                {
                    let u = self.choices[c];
                    self.apply(u);
                    self.choices.clear();
                    self.phase = Phase::Playing;
                }
                return;
            }
            Phase::Won | Phase::Lost => return,
            Phase::Playing => {}
        }
        self.time += dt;
        self.move_player(dt, input);
        self.gather(dt);
        if input.deposit {
            self.deposit();
        }
        if let Some(k) = input.build {
            self.build(k);
        }
        if input.launch {
            self.try_launch();
        }
        self.produce_energy(dt);
        self.tick_nukes(dt);
        self.tick_juggernaut(dt);
        self.spawn(dt);
        self.fire(dt);
        self.tick_buildings(dt);
        self.tick_bullets(dt);
        self.tick_enemies(dt);
        self.check_end();
        if !self.armed_announced && self.armed() && self.nuke.is_none() {
            self.armed_announced = true;
            self.events.push(Event::Armed);
        }
    }

    fn move_player(&mut self, dt: f32, input: &Input) {
        let p = &mut self.player;
        p.pos += input.dir.norm() * p.speed * dt;
        p.pos.x = p.pos.x.clamp(-WORLD, WORLD);
        p.pos.y = p.pos.y.clamp(-WORLD, WORLD);
        if p.pos.len() < SILO_RADIUS + PLAYER_RADIUS {
            p.pos = p.pos.norm() * (SILO_RADIUS + PLAYER_RADIUS);
            if p.pos == V2::ZERO {
                p.pos = v2(0.0, SILO_RADIUS + PLAYER_RADIUS);
            }
        }
        p.hurt = (p.hurt - dt).max(0.0);
        p.hp = (p.hp + p.regen * dt).min(p.max_hp);
    }

    fn gather(&mut self, dt: f32) {
        // Mining metal from deposits while standing on them.
        let cap = self.player.cargo_max;
        if self.cargo() < cap {
            let pos = self.player.pos;
            if let Some(d) = self
                .deposits
                .iter_mut()
                .find(|d| d.metal > 0.0 && d.pos.dist(pos) < 40.0)
            {
                self.player.mine_acc += MINE_RATE * dt;
                if self.player.mine_acc >= 1.0 {
                    self.player.mine_acc -= 1.0;
                    d.metal -= 1.0;
                    self.player.metal += 1;
                    self.events.push(Event::Pickup(PickupKind::Metal));
                }
            }
        }
        self.deposits.retain(|d| d.metal > 0.0);
        if self.deposits.len() < 8 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let d = self.rng.range(300.0, WORLD - 150.0);
            let metal = self.rng.range(40.0, 80.0);
            self.deposits.push(Deposit {
                pos: V2::from_angle(a) * d,
                metal,
            });
        }
        // Collecting drops.
        let mut i = 0;
        while i < self.pickups.len() {
            let pk = &self.pickups[i];
            let d = pk.pos.dist(self.player.pos);
            let full = pk.kind != PickupKind::Xp && self.cargo() >= cap;
            if d < PICKUP_RANGE && !full {
                let pk = self.pickups.swap_remove(i);
                match pk.kind {
                    PickupKind::Metal => {
                        self.player.metal =
                            (self.player.metal + pk.amount).min(cap - self.player.energy)
                    }
                    PickupKind::Energy => {
                        self.player.energy =
                            (self.player.energy + pk.amount).min(cap - self.player.metal)
                    }
                    PickupKind::Xp => self.gain_xp(pk.amount),
                }
                self.events.push(Event::Pickup(pk.kind));
            } else {
                if d < PICKUP_RANGE * 2.0 && !full {
                    let dir = (self.player.pos - pk.pos).norm();
                    self.pickups[i].pos += dir * 300.0 * dt;
                }
                i += 1;
            }
        }
    }

    fn gain_xp(&mut self, n: u32) {
        self.xp += n;
        if self.xp >= xp_needed(self.level) {
            self.xp -= xp_needed(self.level);
            self.level += 1;
            self.phase = Phase::LevelUp;
            let mut pool = Upgrade::ALL.to_vec();
            self.choices.clear();
            for _ in 0..3 {
                let i = self.rng.below(pool.len());
                self.choices.push(pool.swap_remove(i));
            }
            self.events.push(Event::LevelUp);
        }
    }

    pub fn apply(&mut self, u: Upgrade) {
        let p = &mut self.player;
        match u {
            Upgrade::Damage => p.dmg *= 1.3,
            Upgrade::FireRate => p.fire_rate *= 1.25,
            Upgrade::Multishot => p.multishot += 1,
            Upgrade::Speed => p.speed *= 1.12,
            Upgrade::Cargo => p.cargo_max += 15,
            Upgrade::Armor => {
                p.max_hp += 30.0;
                p.hp = p.max_hp;
            }
            Upgrade::Range => p.range *= 1.2,
            Upgrade::Regen => p.regen += 1.5,
        }
    }

    /// Unloads carried metal and energy into the silo (E near the silo).
    pub fn deposit(&mut self) {
        if !self.near_silo() || self.cargo() == 0 || self.nuke.is_some() {
            self.events.push(Event::Denied);
            return;
        }
        self.silo_metal += self.player.metal;
        self.silo_energy += self.player.energy;
        self.player.metal = 0;
        self.player.energy = 0;
        self.events.push(Event::Deposited);
    }

    /// Builds a structure at the player position, paid from carried metal.
    pub fn build(&mut self, kind: BuildKind) {
        let pos = self.player.pos;
        if self.player.metal < kind.cost() || !self.can_place(pos, kind) {
            self.events.push(Event::Denied);
            return;
        }
        self.player.metal -= kind.cost();
        self.buildings.push(Building {
            kind,
            pos,
            hp: kind.max_hp(),
            cd: 0.0,
            charge: 0.0,
        });
        self.events.push(Event::Built(kind));
    }

    pub fn try_launch(&mut self) {
        if !self.armed() || self.nuke.is_some() {
            self.events.push(Event::Denied);
            return;
        }
        self.silo_metal -= metal_needed(self.cycle);
        self.silo_energy -= energy_needed(self.cycle);
        self.nuke = Some(0.0);
        self.events.push(Event::Launch);
    }

    fn produce_energy(&mut self, dt: f32) {
        // Each solar trickles energy straight into the silo.
        self.energy_acc += self.solars() as f32 * 0.6 * dt;
        while self.energy_acc >= 1.0 {
            self.energy_acc -= 1.0;
            self.silo_energy += 1;
        }
    }

    fn tick_nukes(&mut self, dt: f32) {
        if let Some(t) = &mut self.nuke {
            *t += dt;
            if *t >= NUKE_FLIGHT {
                self.nuke = None;
                self.detonate();
            }
        }
        // Enemy retaliation from the second cycle on.
        if self.cycle >= 2 && self.enemy_nuke.is_none() {
            self.enemy_nuke_clock -= dt;
            if self.enemy_nuke_clock <= 0.0 {
                self.enemy_nuke_clock = (75.0 - 8.0 * self.cycle as f32).max(35.0);
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                // Aim at the silo or at the player, whichever hurts more.
                let to = if self.rng.chance(0.6) {
                    V2::ZERO
                } else {
                    self.player.pos
                };
                self.enemy_nuke = Some(EnemyNuke {
                    from: V2::from_angle(a) * (WORLD * 1.4),
                    to,
                    t: 0.0,
                });
                self.events.push(Event::EnemyNukeLaunch);
            }
        }
        if let Some(n) = &mut self.enemy_nuke {
            n.t += dt;
            let pos = n.pos();
            let to = n.to;
            let arriving = n.t >= ENEMY_NUKE_FLIGHT * 0.6;
            if arriving
                && let Some(b) = self.buildings.iter_mut().find(|b| {
                    b.kind == BuildKind::AntiNuke
                        && b.charge >= ANTI_CHARGE
                        && b.pos.dist(to) < 1200.0
                })
            {
                b.charge = 0.0;
                self.enemy_nuke = None;
                self.events.push(Event::Intercepted { pos });
            } else if n.t >= ENEMY_NUKE_FLIGHT {
                self.enemy_nuke = None;
                self.enemy_nuke_hit(to);
            }
        }
    }

    fn enemy_nuke_hit(&mut self, at: V2) {
        if at.len() < ENEMY_NUKE_RADIUS + SILO_RADIUS {
            self.silo_hp -= 450.0 * (1.0 - at.len() / (ENEMY_NUKE_RADIUS + SILO_RADIUS)).max(0.4);
        }
        if self.player.pos.dist(at) < ENEMY_NUKE_RADIUS {
            self.player.hp -= 70.0;
            self.player.hurt = 0.5;
        }
        let before = self.buildings.len();
        self.buildings
            .retain(|b| b.pos.dist(at) > ENEMY_NUKE_RADIUS || b.kind == BuildKind::AntiNuke);
        if self.buildings.len() < before {
            self.events.push(Event::BuildingLost);
        }
        self.events.push(Event::EnemyNukeHit { pos: at });
    }

    fn detonate(&mut self) {
        // Our nuke strikes the enemy staging grounds: the whole map is wiped.
        for e in std::mem::take(&mut self.enemies) {
            self.kills += 1;
            self.pickups.push(Pickup {
                kind: PickupKind::Xp,
                pos: e.pos,
                amount: e.kind.stats().4.min(3),
            });
        }
        self.bullets.clear();
        self.launches += 1;
        self.events.push(Event::Detonate {
            cycle: self.launches,
        });
        if self.launches >= CYCLES {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }
        self.cycle += 1;
        self.jugg_clock = jugg_timer(self.cycle);
        self.jugg_spawned = false;
        self.armed_announced = false;
        self.enemy_nuke_clock = 25.0;
        self.silo_hp = (self.silo_hp + 250.0).min(self.silo_max);
    }

    fn tick_juggernaut(&mut self, dt: f32) {
        if self.jugg_spawned {
            return;
        }
        self.jugg_clock -= dt;
        if self.jugg_clock <= 0.0 {
            self.jugg_spawned = true;
            let pos = self.edge_point();
            self.add_enemy(EnemyKind::Juggernaut, pos);
            self.events.push(Event::JuggernautSpawn);
        }
    }

    fn edge_point(&mut self) -> V2 {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        V2::from_angle(a) * (WORLD * 0.98)
    }

    fn add_enemy(&mut self, kind: EnemyKind, pos: V2) {
        let scale = 1.0 + 0.35 * (self.cycle - 1) as f32;
        let hp = kind.stats().0 * scale;
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            flash: 0.0,
        });
    }

    fn spawn(&mut self, dt: f32) {
        if self.nuke.is_some() {
            return;
        }
        self.spawn_acc += self.spawn_rate() * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            if self.enemies.len() > 300 {
                continue;
            }
            let roll = self.rng.f32();
            let brute_p = 0.05 + 0.04 * self.cycle as f32;
            let kind = if roll < brute_p {
                EnemyKind::Brute
            } else if roll < brute_p + 0.3 {
                EnemyKind::Raider
            } else {
                EnemyKind::Grunt
            };
            let pos = self.edge_point();
            self.add_enemy(kind, pos);
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
        p.fire_cd -= dt;
        if p.fire_cd > 0.0 {
            return;
        }
        let (pos, range) = (p.pos, p.range);
        let Some(t) = self.nearest_enemy(pos, range) else {
            return;
        };
        let p = &mut self.player;
        p.fire_cd = 1.0 / p.fire_rate;
        let base = (self.enemies[t].pos - pos).norm();
        let n = p.multishot;
        for i in 0..n {
            let off = (i as f32 - (n - 1) as f32 / 2.0) * 0.14;
            let a = base.y.atan2(base.x) + off;
            self.bullets.push(Bullet {
                pos,
                vel: V2::from_angle(a) * 620.0,
                dmg: p.dmg,
                life: range / 620.0 + 0.1,
                turret: false,
            });
        }
        self.events.push(Event::Shot);
    }

    fn tick_buildings(&mut self, dt: f32) {
        for i in 0..self.buildings.len() {
            let b = &mut self.buildings[i];
            match b.kind {
                BuildKind::Turret => {
                    b.cd -= dt;
                    if b.cd > 0.0 {
                        continue;
                    }
                    let pos = b.pos;
                    if let Some(t) = self.nearest_enemy(pos, 260.0) {
                        self.buildings[i].cd = 0.45;
                        let dir = (self.enemies[t].pos - pos).norm();
                        self.bullets.push(Bullet {
                            pos,
                            vel: dir * 520.0,
                            dmg: 9.0 + 2.0 * self.cycle as f32,
                            life: 0.6,
                            turret: true,
                        });
                        self.events.push(Event::TurretShot);
                    }
                }
                BuildKind::AntiNuke => b.charge = (b.charge + dt).min(ANTI_CHARGE),
                BuildKind::Solar => {}
            }
        }
    }

    fn tick_bullets(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.bullets.len() {
            let b = &mut self.bullets[i];
            b.pos += b.vel * dt;
            b.life -= dt;
            let (pos, dmg) = (b.pos, b.dmg);
            let dead = b.life <= 0.0;
            let hit = self
                .enemies
                .iter()
                .position(|e| e.pos.dist(pos) < e.kind.stats().2 + 3.0);
            if let Some(h) = hit {
                let e = &mut self.enemies[h];
                e.hp -= dmg;
                e.flash = 0.08;
                self.events.push(Event::Hit);
                if e.hp <= 0.0 {
                    self.kill(h);
                }
                self.bullets.swap_remove(i);
            } else if dead {
                self.bullets.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn kill(&mut self, i: usize) {
        let e = self.enemies.swap_remove(i);
        let (.., xp) = e.kind.stats();
        self.kills += 1;
        let big = matches!(e.kind, EnemyKind::Brute | EnemyKind::Juggernaut);
        self.events.push(Event::Kill { pos: e.pos, big });
        self.pickups.push(Pickup {
            kind: PickupKind::Xp,
            pos: e.pos,
            amount: xp,
        });
        let drop = if e.kind == EnemyKind::Juggernaut {
            1.0
        } else {
            0.35
        };
        if self.rng.chance(drop) {
            let kind = if self.rng.chance(0.5) {
                PickupKind::Metal
            } else {
                PickupKind::Energy
            };
            let amount = if big { 8 } else { 2 };
            let jitter = v2(self.rng.range(-12.0, 12.0), self.rng.range(-12.0, 12.0));
            self.pickups.push(Pickup {
                kind,
                pos: e.pos + jitter,
                amount,
            });
        }
        if e.kind == EnemyKind::Juggernaut {
            // Killing it buys a fresh countdown.
            self.jugg_spawned = false;
            self.jugg_clock = jugg_timer(self.cycle) * 0.6;
        }
    }

    fn tick_enemies(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let mut silo_dmg = 0.0;
        let mut player_dmg = 0.0;
        for e in &mut self.enemies {
            let (_, speed, r, dps, _) = e.kind.stats();
            e.flash = (e.flash - dt).max(0.0);
            // Raiders hunt the player; everyone else goes for the silo but
            // gets distracted by a nearby player or building.
            let mut target = V2::ZERO;
            let mut tr = SILO_RADIUS;
            if e.kind != EnemyKind::Juggernaut {
                if e.kind == EnemyKind::Raider || e.pos.dist(ppos) < 180.0 {
                    target = ppos;
                    tr = PLAYER_RADIUS;
                } else if let Some(b) = self.buildings.iter().find(|b| b.pos.dist(e.pos) < 120.0) {
                    target = b.pos;
                    tr = b.kind.radius();
                }
            }
            let to = target - e.pos;
            if to.len() > r + tr {
                e.pos += to.norm() * speed * dt;
            }
            if e.pos.len() < r + SILO_RADIUS + 2.0 {
                silo_dmg += dps * dt;
            }
            if e.pos.dist(ppos) < r + PLAYER_RADIUS + 2.0 {
                player_dmg += dps * dt;
            }
            for b in &mut self.buildings {
                if e.pos.dist(b.pos) < r + b.kind.radius() + 2.0 {
                    b.hp -= dps * dt;
                }
            }
        }
        self.silo_hp -= silo_dmg;
        if player_dmg > 0.0 {
            self.player.hp -= player_dmg;
            if self.player.hurt <= 0.0 {
                self.player.hurt = 0.3;
                self.events.push(Event::PlayerHurt);
            }
        }
        let before = self.buildings.len();
        self.buildings.retain(|b| b.hp > 0.0);
        if self.buildings.len() < before {
            self.events.push(Event::BuildingLost);
        }
    }

    fn check_end(&mut self) {
        if self.phase == Phase::Playing && (self.silo_hp <= 0.0 || self.player.hp <= 0.0) {
            self.silo_hp = self.silo_hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    fn fill_silo(g: &mut Game) {
        g.silo_metal = metal_needed(g.cycle);
        g.silo_energy = energy_needed(g.cycle);
    }

    #[test]
    fn requirements_escalate() {
        assert!(metal_needed(2) > metal_needed(1));
        assert!(energy_needed(3) > energy_needed(2));
        assert!(jugg_timer(5) < jugg_timer(1));
        assert!(jugg_timer(50) >= 90.0);
    }

    #[test]
    fn deposit_only_near_silo() {
        let mut g = Game::new(1);
        g.player.metal = 10;
        g.player.energy = 5;
        g.player.pos = v2(500.0, 0.0);
        g.deposit();
        assert_eq!(g.silo_metal, 0);
        assert_eq!(g.events.last(), Some(&Event::Denied));
        g.player.pos = v2(70.0, 0.0);
        g.deposit();
        assert_eq!((g.silo_metal, g.silo_energy), (10, 5));
        assert_eq!(g.cargo(), 0);
    }

    #[test]
    fn launch_requires_armed_silo() {
        let mut g = Game::new(1);
        g.try_launch();
        assert!(g.nuke.is_none());
        fill_silo(&mut g);
        assert!(g.armed());
        g.try_launch();
        assert!(g.nuke.is_some());
        assert_eq!(g.silo_metal, 0);
    }

    #[test]
    fn detonation_clears_map_and_advances_cycle() {
        let mut g = Game::new(2);
        for _ in 0..20 {
            g.add_enemy(EnemyKind::Grunt, v2(900.0, 900.0));
        }
        fill_silo(&mut g);
        g.try_launch();
        g.player.hp = 1e9;
        g.silo_hp = 1e9;
        for _ in 0..((NUKE_FLIGHT + 0.1) * 60.0) as usize {
            g.update(1.0 / 60.0, &idle());
        }
        assert_eq!(g.launches, 1);
        assert_eq!(g.cycle, 2);
        assert!(g.enemies.len() < 5, "map should be wiped");
        assert!(g.kills >= 20);
    }

    #[test]
    fn winning_after_all_cycles() {
        let mut g = Game::new(3);
        g.launches = CYCLES - 1;
        g.cycle = CYCLES;
        fill_silo(&mut g);
        g.try_launch();
        for _ in 0..200 {
            g.update(1.0 / 60.0, &idle());
        }
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn silo_destroyed_loses() {
        let mut g = Game::new(4);
        g.silo_hp = 0.5;
        g.add_enemy(EnemyKind::Brute, v2(SILO_RADIUS + 10.0, 0.0));
        g.player.pos = v2(-800.0, 0.0);
        for _ in 0..60 {
            g.update(1.0 / 60.0, &idle());
        }
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn juggernaut_spawns_when_clock_expires() {
        let mut g = Game::new(5);
        g.jugg_clock = 0.01;
        g.update(0.02, &idle());
        assert!(g.enemies.iter().any(|e| e.kind == EnemyKind::Juggernaut));
        assert!(g.events.contains(&Event::JuggernautSpawn));
    }

    #[test]
    fn building_costs_carried_metal_and_blocks_overlap() {
        let mut g = Game::new(6);
        g.player.pos = v2(300.0, 0.0);
        g.build(BuildKind::Turret);
        assert!(g.buildings.is_empty());
        g.player.metal = 40;
        g.build(BuildKind::Turret);
        assert_eq!(g.buildings.len(), 1);
        assert_eq!(g.player.metal, 40 - BuildKind::Turret.cost());
        g.build(BuildKind::Solar);
        assert_eq!(g.buildings.len(), 1, "cannot stack on the turret");
    }

    #[test]
    fn solar_feeds_silo_energy() {
        let mut g = Game::new(7);
        g.player.metal = 30;
        g.player.pos = v2(0.0, 300.0);
        g.build(BuildKind::Solar);
        g.player.pos = v2(-600.0, 600.0);
        for _ in 0..600 {
            g.update(1.0 / 60.0, &idle());
            if g.phase == Phase::LevelUp {
                g.update(
                    0.0,
                    &Input {
                        choice: Some(0),
                        ..idle()
                    },
                );
            }
        }
        assert!(g.silo_energy >= 5);
    }

    #[test]
    fn anti_nuke_intercepts_enemy_nuke() {
        let mut g = Game::new(8);
        g.cycle = 2;
        g.player.pos = v2(0.0, 300.0);
        g.player.metal = 40;
        g.build(BuildKind::AntiNuke);
        g.buildings[0].charge = ANTI_CHARGE;
        g.enemy_nuke = Some(EnemyNuke {
            from: v2(2000.0, 0.0),
            to: V2::ZERO,
            t: 0.0,
        });
        g.enemy_nuke_clock = 1000.0;
        let hp = g.silo_hp;
        for _ in 0..(ENEMY_NUKE_FLIGHT * 60.0) as usize + 5 {
            g.player.hp = 100.0;
            g.update(1.0 / 60.0, &idle());
            if g.phase == Phase::LevelUp {
                g.update(
                    0.0,
                    &Input {
                        choice: Some(0),
                        ..idle()
                    },
                );
            }
        }
        assert!(
            g.events
                .iter()
                .any(|e| matches!(e, Event::Intercepted { .. }))
        );
        assert!(g.silo_hp > hp - 200.0);
    }

    #[test]
    fn unblocked_enemy_nuke_damages_silo() {
        let mut g = Game::new(9);
        g.player.pos = v2(900.0, 900.0);
        let hp = g.silo_hp;
        g.enemy_nuke_hit(V2::ZERO);
        assert!(g.silo_hp <= hp - 400.0);
    }

    #[test]
    fn level_up_offers_three_distinct_choices() {
        let mut g = Game::new(10);
        g.gain_xp(xp_needed(1));
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        assert_ne!(g.choices[0], g.choices[1]);
        let dmg = g.player.dmg;
        g.choices[0] = Upgrade::Damage;
        g.update(
            0.0,
            &Input {
                choice: Some(0),
                ..idle()
            },
        );
        assert_eq!(g.phase, Phase::Playing);
        assert!(g.player.dmg > dmg);
    }

    #[test]
    fn mining_fills_cargo_up_to_cap() {
        let mut g = Game::new(11);
        let d = g.deposits[0].pos;
        g.deposits[0].metal = 1000.0;
        g.player.pos = d;
        g.player.hp = 1e9;
        g.silo_hp = 1e9;
        for _ in 0..60 * 20 {
            g.player.pos = d;
            g.update(1.0 / 60.0, &idle());
            if g.phase == Phase::LevelUp {
                g.update(
                    0.0,
                    &Input {
                        choice: Some(0),
                        ..idle()
                    },
                );
            }
        }
        assert!(g.cargo() <= g.player.cargo_max);
        assert!(g.player.metal >= 25);
    }

    #[test]
    fn difficulty_rises_with_cycle() {
        let mut g = Game::new(12);
        let r1 = g.spawn_rate();
        g.cycle = 3;
        g.jugg_clock = jugg_timer(3);
        assert!(g.spawn_rate() > r1);
    }
}
