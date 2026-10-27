//! Shield Bubble game rules.
//!
//! The base lives inside a shield dome formed by the union of circles around
//! the HQ and every generator. Outside the dome the commander suffers
//! artillery fog; inside, the shield absorbs fire and the commander heals.

use crate::math::{V2, v2};
use crate::rng::Rng;

pub const WORLD: f32 = 2200.0;
pub const HQ_DOME: f32 = 260.0;
pub const GEN_DOME: f32 = 190.0;
pub const GEN_SPACING: f32 = 110.0;
pub const TURRET_COST: u32 = 30;
pub const TURRET_RANGE: f32 = 280.0;
pub const PLAYER_RADIUS: f32 = 14.0;
pub const SHELL_RADIUS: f32 = 55.0;
pub const WAVE_TIME: f32 = 30.0;
pub const WAVES: u32 = 20;
pub const DURATION: f32 = WAVE_TIME * WAVES as f32;
pub const BEACON_CHANNEL: f32 = 1.2;
pub const BEACON_COOLDOWN: f32 = 20.0;
pub const FOG_DPS: f32 = 2.5;
pub const HQ_HP: f32 = 600.0;
pub const GEN_HP: f32 = 250.0;
pub const TURRET_HP: f32 = 150.0;

pub fn gen_cost(built: usize) -> u32 {
    40 + 25 * built as u32
}

pub fn xp_needed(level: u32) -> u32 {
    5 + level * 4
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    Hq,
    Generator,
    Turret,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildingKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cooldown: f32,
}

impl Building {
    pub fn dome_radius(&self) -> f32 {
        match self.kind {
            BuildingKind::Hq => HQ_DOME,
            BuildingKind::Generator => GEN_DOME,
            BuildingKind::Turret => 0.0,
        }
    }

    pub fn radius(&self) -> f32 {
        match self.kind {
            BuildingKind::Hq => 34.0,
            BuildingKind::Generator => 20.0,
            BuildingKind::Turret => 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Raider,
    Artillery,
    Tank,
    Boss,
}

impl EnemyKind {
    /// (hp, speed, radius, contact drain per second, xp, metal drop)
    pub fn stats(self) -> (f32, f32, f32, f32, u32, u32) {
        match self {
            EnemyKind::Raider => (22.0, 95.0, 12.0, 12.0, 1, 2),
            EnemyKind::Artillery => (40.0, 60.0, 14.0, 6.0, 2, 4),
            EnemyKind::Tank => (170.0, 45.0, 22.0, 35.0, 5, 10),
            EnemyKind::Boss => (2200.0, 35.0, 42.0, 80.0, 40, 80),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub reload: f32,
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

/// An artillery shell in flight. Lands at `to` when `t` reaches `flight`.
#[derive(Clone, Debug)]
pub struct Shell {
    pub from: V2,
    pub to: V2,
    pub t: f32,
    pub flight: f32,
    pub dmg: f32,
    /// Fog shells are the ambient bombardment that hunts the commander.
    pub fog: bool,
}

impl Shell {
    pub fn pos(&self) -> V2 {
        let k = (self.t / self.flight).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * k
    }
}

#[derive(Clone, Debug)]
pub struct Deposit {
    pub pos: V2,
    pub metal: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Range,
    Speed,
    Armor,
    Cargo,
    ShieldRegen,
    Multishot,
}

pub const UPGRADES: [Upgrade; 8] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::Range,
    Upgrade::Speed,
    Upgrade::Armor,
    Upgrade::Cargo,
    Upgrade::ShieldRegen,
    Upgrade::Multishot,
];

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavy Rounds: +30% damage",
            Upgrade::FireRate => "Autoloader: +25% fire rate",
            Upgrade::Range => "Rangefinder: +20% range",
            Upgrade::Speed => "Servos: +12% move speed",
            Upgrade::Armor => "Armor Plating: +30 max HP, heal",
            Upgrade::Cargo => "Cargo Pods: +40 metal capacity",
            Upgrade::ShieldRegen => "Capacitors: +40% shield regen",
            Upgrade::Multishot => "Twin Barrels: +1 projectile",
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot,
    TurretShot,
    Hit,
    Kill { pos: V2, big: bool },
    Mined,
    Unloaded(u32),
    ShellLaunch,
    ShellLand { pos: V2, shielded: bool },
    ShieldDown,
    ShieldUp,
    PlayerHurt,
    LevelUp,
    Built(BuildingKind),
    Denied,
    BeaconStart,
    Teleport,
    Wave(u32),
    BossSpawn,
    BuildingLost,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub build_gen: bool,
    pub build_turret: bool,
    pub beacon: bool,
    pub choice: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub fire_rate: f32,
    pub range: f32,
    pub shots: u32,
    pub reload: f32,
    pub cargo: f32,
    pub cargo_max: f32,
    pub beacon_cd: f32,
    /// Remaining channel time while the return beacon charges.
    pub channel: Option<f32>,
    pub hurt: f32,
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub phase: Phase,
    pub player: Player,
    pub buildings: Vec<Building>,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub shells: Vec<Shell>,
    pub deposits: Vec<Deposit>,
    pub shield: f32,
    pub shield_regen_mult: f32,
    pub metal: u32,
    pub xp: u32,
    pub level: u32,
    pub choices: Vec<Upgrade>,
    pub wave: u32,
    pub kills: u32,
    pub spawn_acc: f32,
    pub fog_timer: f32,
    pub events: Vec<Event>,
    shield_was_up: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut deposits = Vec::new();
        for i in 0..40 {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = 380.0 + (i as f32 / 40.0) * 1500.0 + rng.range(0.0, 200.0);
            deposits.push(Deposit {
                pos: V2::from_angle(a) * d.min(WORLD - 60.0),
                metal: 30.0 + d * 0.03,
            });
        }
        Game {
            rng,
            time: 0.0,
            phase: Phase::Playing,
            player: Player {
                pos: v2(0.0, 80.0),
                hp: 100.0,
                max_hp: 100.0,
                speed: 220.0,
                dmg: 10.0,
                fire_rate: 3.0,
                range: 300.0,
                shots: 1,
                reload: 0.0,
                cargo: 0.0,
                cargo_max: 60.0,
                beacon_cd: 0.0,
                channel: None,
                hurt: 0.0,
            },
            buildings: vec![Building {
                kind: BuildingKind::Hq,
                pos: V2::ZERO,
                hp: HQ_HP,
                max_hp: HQ_HP,
                cooldown: 0.0,
            }],
            enemies: Vec::new(),
            bullets: Vec::new(),
            shells: Vec::new(),
            deposits,
            shield: 200.0,
            shield_regen_mult: 1.0,
            metal: 40,
            xp: 0,
            level: 1,
            choices: Vec::new(),
            wave: 0,
            kills: 0,
            spawn_acc: 0.0,
            fog_timer: 3.0,
            events: Vec::new(),
            shield_was_up: true,
        }
    }

    pub fn generators(&self) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Generator)
            .count()
    }

    pub fn shield_max(&self) -> f32 {
        200.0 + 90.0 * self.generators() as f32
    }

    pub fn shield_regen(&self) -> f32 {
        (5.0 + 3.0 * self.generators() as f32) * self.shield_regen_mult
    }

    /// Signed distance to the dome edge: negative inside.
    pub fn dome_sd(&self, p: V2) -> f32 {
        self.buildings
            .iter()
            .filter(|b| b.dome_radius() > 0.0)
            .map(|b| p.dist(b.pos) - b.dome_radius())
            .fold(f32::INFINITY, f32::min)
    }

    pub fn in_dome(&self, p: V2) -> bool {
        self.dome_sd(p) < 0.0
    }

    /// Pushes `p` (with radius `r`) out of the dome along the nearest edge.
    pub fn push_out(&self, mut p: V2, r: f32) -> V2 {
        for _ in 0..4 {
            let mut moved = false;
            for b in self.buildings.iter().filter(|b| b.dome_radius() > 0.0) {
                let lim = b.dome_radius() + r;
                let d = p.dist(b.pos);
                if d < lim {
                    let n = if d < 1e-3 {
                        v2(1.0, 0.0)
                    } else {
                        (p - b.pos).norm()
                    };
                    p = b.pos + n * lim;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        p
    }

    /// Whether a new building of `kind` could go at `p`.
    pub fn can_place(&self, p: V2) -> bool {
        self.in_dome(p)
            && self
                .buildings
                .iter()
                .all(|b| b.pos.dist(p) >= GEN_SPACING.min(b.radius() + 60.0))
    }

    fn try_build(&mut self, kind: BuildingKind) {
        let p = self.player.pos;
        let cost = match kind {
            BuildingKind::Generator => gen_cost(self.generators()),
            _ => TURRET_COST,
        };
        if !self.can_place(p) || self.metal < cost {
            self.events.push(Event::Denied);
            return;
        }
        self.metal -= cost;
        let hp = if kind == BuildingKind::Generator {
            GEN_HP
        } else {
            TURRET_HP
        };
        self.buildings.push(Building {
            kind,
            pos: p,
            hp,
            max_hp: hp,
            cooldown: 0.0,
        });
        if kind == BuildingKind::Generator {
            self.shield = (self.shield + 60.0).min(self.shield_max());
        }
        self.events.push(Event::Built(kind));
    }

    fn roll_choices(&mut self) {
        let mut pool = UPGRADES.to_vec();
        self.choices.clear();
        for _ in 0..3 {
            let i = self.rng.below(pool.len());
            self.choices.push(pool.swap_remove(i));
        }
    }

    pub fn apply_upgrade(&mut self, u: Upgrade) {
        let p = &mut self.player;
        match u {
            Upgrade::Damage => p.dmg *= 1.3,
            Upgrade::FireRate => p.fire_rate *= 1.25,
            Upgrade::Range => p.range *= 1.2,
            Upgrade::Speed => p.speed *= 1.12,
            Upgrade::Armor => {
                p.max_hp += 30.0;
                p.hp = p.max_hp;
            }
            Upgrade::Cargo => p.cargo_max += 40.0,
            Upgrade::ShieldRegen => self.shield_regen_mult *= 1.4,
            Upgrade::Multishot => p.shots += 1,
        }
    }

    pub fn gain_xp(&mut self, n: u32) {
        self.xp += n;
        if self.xp >= xp_needed(self.level) && self.phase == Phase::Playing {
            self.xp -= xp_needed(self.level);
            self.level += 1;
            self.phase = Phase::LevelUp;
            self.roll_choices();
            self.events.push(Event::LevelUp);
        }
    }

    fn spawn_enemy(&mut self, kind: EnemyKind) {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let d = WORLD * 0.75 + self.rng.range(0.0, 200.0);
        let (hp, ..) = kind.stats();
        let hp = hp * (1.0 + self.wave as f32 * 0.08);
        self.enemies.push(Enemy {
            kind,
            pos: V2::from_angle(a) * d,
            hp,
            max_hp: hp,
            reload: self.rng.range(1.0, 3.0),
            flash: 0.0,
        });
    }

    fn spawn_rate(&self) -> f32 {
        0.6 + self.wave as f32 * 0.35
    }

    fn hurt_player(&mut self, dmg: f32) {
        self.player.hp -= dmg;
        self.player.hurt = 0.2;
        self.events.push(Event::PlayerHurt);
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        self.events.clear();
        match self.phase {
            Phase::LevelUp => {
                if let Some(i) = input.choice
                    && let Some(&u) = self.choices.get(i)
                {
                    self.apply_upgrade(u);
                    self.choices.clear();
                    self.phase = Phase::Playing;
                    // Chain level-ups if enough XP remains.
                    self.gain_xp(0);
                }
                return;
            }
            Phase::Won | Phase::Lost => return,
            Phase::Playing => {}
        }
        self.time += dt;

        // Waves.
        let wave = ((self.time / WAVE_TIME) as u32 + 1).min(WAVES);
        if wave != self.wave {
            self.wave = wave;
            self.events.push(Event::Wave(wave));
            if wave.is_multiple_of(5) {
                let n = wave / 5;
                for _ in 0..n {
                    self.spawn_enemy(if wave >= 10 {
                        EnemyKind::Boss
                    } else {
                        EnemyKind::Tank
                    });
                }
                if wave >= 10 {
                    self.events.push(Event::BossSpawn);
                }
            }
        }
        if self.time < DURATION {
            self.spawn_acc += dt * self.spawn_rate();
            while self.spawn_acc >= 1.0 {
                self.spawn_acc -= 1.0;
                let r = self.rng.f32();
                let w = self.wave as f32;
                let kind = if r < 0.04 + w * 0.012 {
                    EnemyKind::Tank
                } else if r < 0.2 + w * 0.02 {
                    EnemyKind::Artillery
                } else {
                    EnemyKind::Raider
                };
                self.spawn_enemy(kind);
            }
        } else if !self.enemies.iter().any(|e| e.kind == EnemyKind::Boss) {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }

        self.update_player(dt, input);
        self.update_buildings(dt);
        self.update_enemies(dt);
        self.update_bullets(dt);
        self.update_shells(dt);

        // Shield regen.
        let max = self.shield_max();
        self.shield = (self.shield + self.shield_regen() * dt).min(max);
        let up = self.shield > 0.0;
        if up != self.shield_was_up {
            self.events.push(if up {
                Event::ShieldUp
            } else {
                Event::ShieldDown
            });
            self.shield_was_up = up;
        }

        // Destroyed buildings.
        let before = self.buildings.len();
        let hq_alive = self.buildings[0].hp > 0.0;
        self.buildings
            .retain(|b| b.hp > 0.0 || b.kind == BuildingKind::Hq);
        if self.buildings.len() != before {
            self.events.push(Event::BuildingLost);
            self.shield = self.shield.min(self.shield_max());
        }
        if !hq_alive || self.player.hp <= 0.0 {
            self.player.hp = self.player.hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        }
    }

    fn update_player(&mut self, dt: f32, input: &Input) {
        let inside = self.in_dome(self.player.pos);
        let p = &mut self.player;
        p.hurt = (p.hurt - dt).max(0.0);
        p.beacon_cd = (p.beacon_cd - dt).max(0.0);
        if let Some(c) = p.channel {
            let c = c - dt;
            if c <= 0.0 {
                p.channel = None;
                p.pos = v2(0.0, 60.0);
                p.beacon_cd = BEACON_COOLDOWN;
                self.events.push(Event::Teleport);
            } else {
                p.channel = Some(c);
            }
        } else {
            let mv = input.dir.norm() * p.speed * dt;
            p.pos += mv;
            p.pos.x = p.pos.x.clamp(-WORLD, WORLD);
            p.pos.y = p.pos.y.clamp(-WORLD, WORLD);
            if input.beacon {
                if p.beacon_cd <= 0.0 {
                    p.channel = Some(BEACON_CHANNEL);
                    self.events.push(Event::BeaconStart);
                } else {
                    self.events.push(Event::Denied);
                }
            }
        }
        if inside {
            p.hp = (p.hp + 12.0 * dt).min(p.max_hp);
            if p.cargo >= 1.0 {
                let n = p.cargo as u32;
                p.cargo -= n as f32;
                self.metal += n;
                self.events.push(Event::Unloaded(n));
            }
        } else {
            let fog = FOG_DPS * (1.0 + self.wave as f32 * 0.1);
            p.hp -= fog * dt;
            // Ambient bombardment hunts the commander outside the dome.
            self.fog_timer -= dt;
            if self.fog_timer <= 0.0 {
                self.fog_timer = (2.2 - self.wave as f32 * 0.06).max(0.8);
                let off = V2::from_angle(self.rng.range(0.0, 6.3)) * self.rng.range(0.0, 70.0);
                let to = self.player.pos + off;
                self.shells.push(Shell {
                    from: to + v2(-300.0, -500.0),
                    to,
                    t: 0.0,
                    flight: 1.3,
                    dmg: 20.0,
                    fog: true,
                });
            }
        }
        if input.build_gen {
            self.try_build(BuildingKind::Generator);
        }
        if input.build_turret {
            self.try_build(BuildingKind::Turret);
        }

        // Mining.
        let pp = self.player.pos;
        let room = self.player.cargo_max - self.player.cargo;
        if room > 0.0
            && let Some(d) = self
                .deposits
                .iter_mut()
                .find(|d| d.metal > 0.0 && d.pos.dist(pp) < 40.0)
        {
            let take = (25.0 * dt).min(d.metal).min(room);
            d.metal -= take;
            self.player.cargo += take;
            self.events.push(Event::Mined);
        }
        self.deposits.retain(|d| d.metal > 0.0);

        // Auto-fire.
        let p = &mut self.player;
        p.reload -= dt;
        if p.reload <= 0.0 && p.channel.is_none() {
            let range = p.range;
            let target = self
                .enemies
                .iter()
                .filter(|e| e.pos.dist(p.pos) < range)
                .min_by(|a, b| a.pos.dist2(p.pos).total_cmp(&b.pos.dist2(p.pos)));
            if let Some(t) = target {
                p.reload = 1.0 / p.fire_rate;
                let base = (t.pos - p.pos).norm();
                let ang = base.y.atan2(base.x);
                for i in 0..p.shots {
                    let off = (i as f32 - (p.shots - 1) as f32 / 2.0) * 0.12;
                    self.bullets.push(Bullet {
                        pos: p.pos,
                        vel: V2::from_angle(ang + off) * 650.0,
                        dmg: p.dmg,
                        life: range / 650.0 + 0.1,
                        turret: false,
                    });
                }
                self.events.push(Event::Shot);
            }
        }
    }

    fn update_buildings(&mut self, dt: f32) {
        for i in 0..self.buildings.len() {
            let b = &mut self.buildings[i];
            if b.kind != BuildingKind::Turret {
                continue;
            }
            b.cooldown -= dt;
            if b.cooldown > 0.0 {
                continue;
            }
            let bp = b.pos;
            let target = self
                .enemies
                .iter()
                .filter(|e| e.pos.dist(bp) < TURRET_RANGE)
                .min_by(|a, b| a.pos.dist2(bp).total_cmp(&b.pos.dist2(bp)));
            if let Some(t) = target {
                let dir = (t.pos - bp).norm();
                self.buildings[i].cooldown = 0.6;
                self.bullets.push(Bullet {
                    pos: bp,
                    vel: dir * 700.0,
                    dmg: 14.0 + self.wave as f32,
                    life: TURRET_RANGE / 700.0 + 0.1,
                    turret: true,
                });
                self.events.push(Event::TurretShot);
            }
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let shield_up = self.shield > 0.0;
        let pp = self.player.pos;
        let player_out = !self.in_dome(pp);
        let mut drain = 0.0;
        let mut player_dmg = 0.0;
        let mut new_shells = Vec::new();
        let mut building_dmg: Vec<(usize, f32)> = Vec::new();
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            let (_, speed, r, contact, ..) = e.kind.stats();
            let chase_player = player_out && e.pos.dist(pp) < 450.0;
            // Nearest building as the siege target.
            let (bi, bpos) = self
                .buildings
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.pos.dist2(e.pos).total_cmp(&b.1.pos.dist2(e.pos)))
                .map(|(i, b)| (i, b.pos))
                .unwrap_or((0, V2::ZERO));
            let target = if chase_player { pp } else { bpos };
            let ranged = matches!(e.kind, EnemyKind::Artillery | EnemyKind::Boss);
            let edge = self.dome_sd(e.pos);
            let mut pos = e.pos;
            let hold = ranged && !chase_player && edge < 320.0 && edge > 60.0;
            if !hold {
                pos += (target - pos).norm() * speed * dt;
            }
            if shield_up {
                pos = self.push_out(pos, r);
            }
            let touching = self.dome_sd(pos) < r + 2.0;
            let e = &mut self.enemies[i];
            e.pos = pos;
            e.flash = (e.flash - dt).max(0.0);
            if touching {
                if shield_up {
                    drain += contact * dt;
                } else if pos.dist(bpos) < r + 40.0 {
                    building_dmg.push((bi, contact * dt));
                }
            }
            if pos.dist(pp) < r + PLAYER_RADIUS {
                player_dmg += contact * dt;
            }
            if ranged {
                e.reload -= dt;
                if e.reload <= 0.0 && pos.dist(target) < 700.0 {
                    let boss = e.kind == EnemyKind::Boss;
                    e.reload = if boss { 1.2 } else { 3.2 };
                    let n = if boss { 3 } else { 1 };
                    for k in 0..n {
                        let spread = V2::from_angle(k as f32 * 2.1) * (k as f32 * 60.0);
                        new_shells.push(Shell {
                            from: pos,
                            to: target + spread,
                            t: 0.0,
                            flight: 1.5,
                            dmg: if boss { 45.0 } else { 25.0 },
                            fog: false,
                        });
                    }
                }
            }
        }
        if !new_shells.is_empty() {
            self.events.push(Event::ShellLaunch);
            self.shells.extend(new_shells);
        }
        self.shield = (self.shield - drain).max(0.0);
        for (i, d) in building_dmg {
            self.buildings[i].hp -= d;
        }
        if player_dmg > 0.0 {
            self.hurt_player(player_dmg);
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let mut kills = Vec::new();
        for b in &mut self.bullets {
            b.pos += b.vel * dt;
            b.life -= dt;
            for (i, e) in self.enemies.iter_mut().enumerate() {
                if e.hp > 0.0 && e.pos.dist(b.pos) < e.kind.stats().2 + 4.0 {
                    e.hp -= b.dmg;
                    e.flash = 0.08;
                    b.life = 0.0;
                    self.events.push(Event::Hit);
                    if e.hp <= 0.0 {
                        kills.push(i);
                    }
                    break;
                }
            }
        }
        self.bullets.retain(|b| b.life > 0.0);
        self.collect_kills();
    }

    fn collect_kills(&mut self) {
        let mut xp = 0;
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let (.., exp, metal) = e.kind.stats();
                xp += exp;
                self.kills += 1;
                self.deposits.push(Deposit {
                    pos: e.pos,
                    metal: metal as f32,
                });
                self.events.push(Event::Kill {
                    pos: e.pos,
                    big: matches!(e.kind, EnemyKind::Tank | EnemyKind::Boss),
                });
            } else {
                i += 1;
            }
        }
        if xp > 0 {
            self.gain_xp(xp);
        }
    }

    fn update_shells(&mut self, dt: f32) {
        let mut landed = Vec::new();
        for s in &mut self.shells {
            s.t += dt;
            if s.t >= s.flight {
                landed.push(s.clone());
            }
        }
        self.shells.retain(|s| s.t < s.flight);
        for s in landed {
            let p = s.to;
            let shielded = self.in_dome(p) && self.shield > 0.0;
            if shielded {
                self.shield = (self.shield - s.dmg).max(0.0);
            } else {
                if self.player.pos.dist(p) < SHELL_RADIUS + PLAYER_RADIUS {
                    self.hurt_player(s.dmg);
                }
                if !s.fog {
                    for b in &mut self.buildings {
                        if b.pos.dist(p) < SHELL_RADIUS + b.radius() {
                            b.hp -= s.dmg;
                        }
                    }
                }
                // Splash also hurts enemies caught in the blast.
                for e in &mut self.enemies {
                    if e.pos.dist(p) < SHELL_RADIUS {
                        e.hp -= s.dmg * 0.5;
                    }
                }
            }
            self.events.push(Event::ShellLand { pos: p, shielded });
        }
        self.collect_kills();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn starts_inside_dome_and_dome_grows_with_generators() {
        let mut g = Game::new(1);
        assert!(g.in_dome(g.player.pos));
        let far = v2(HQ_DOME + 100.0, 0.0);
        assert!(!g.in_dome(far));
        g.player.pos = v2(HQ_DOME - 20.0, 0.0);
        g.metal = 100;
        g.update(
            0.0,
            &Input {
                build_gen: true,
                ..idle()
            },
        );
        assert_eq!(g.generators(), 1);
        assert_eq!(g.metal, 100 - gen_cost(0));
        assert!(g.in_dome(far));
        assert!(g.shield_max() > 200.0);
    }

    #[test]
    fn cannot_build_outside_dome_or_without_metal() {
        let mut g = Game::new(2);
        g.player.pos = v2(HQ_DOME + 50.0, 0.0);
        g.metal = 1000;
        g.update(
            0.0,
            &Input {
                build_gen: true,
                ..idle()
            },
        );
        assert_eq!(g.generators(), 0);
        assert!(g.events.contains(&Event::Denied));
        g.player.pos = v2(150.0, 0.0);
        g.metal = 5;
        g.update(
            0.0,
            &Input {
                build_turret: true,
                ..idle()
            },
        );
        assert_eq!(g.buildings.len(), 1);
    }

    #[test]
    fn fog_hurts_outside_and_dome_heals() {
        let mut g = Game::new(3);
        g.player.pos = v2(900.0, 0.0);
        g.update(1.0, &idle());
        assert!(g.player.hp < 100.0);
        g.player.pos = v2(0.0, 50.0);
        let hp = g.player.hp;
        g.update(1.0, &idle());
        assert!(g.player.hp > hp);
    }

    #[test]
    fn cargo_unloads_inside_dome() {
        let mut g = Game::new(4);
        g.player.cargo = 25.0;
        let m = g.metal;
        g.update(0.01, &idle());
        assert_eq!(g.metal, m + 25);
        assert!(g.player.cargo < 1.0);
    }

    #[test]
    fn mining_fills_cargo_up_to_capacity() {
        let mut g = Game::new(5);
        g.deposits = vec![Deposit {
            pos: v2(1000.0, 0.0),
            metal: 500.0,
        }];
        g.player.pos = v2(1000.0, 0.0);
        g.player.hp = 1e6;
        for _ in 0..400 {
            g.update(0.05, &idle());
            g.player.pos = v2(1000.0, 0.0);
        }
        assert!((g.player.cargo - g.player.cargo_max).abs() < 0.01);
    }

    #[test]
    fn enemies_cannot_cross_shield_and_drain_it() {
        let mut g = Game::new(6);
        g.time = 1.0;
        g.wave = 1;
        g.spawn_acc = -1e9;
        g.enemies.push(Enemy {
            kind: EnemyKind::Tank,
            pos: v2(HQ_DOME + 30.0, 0.0),
            hp: 1e6,
            max_hp: 1e6,
            reload: 9.0,
            flash: 0.0,
        });
        g.shield_regen_mult = 0.0;
        let s = g.shield;
        for _ in 0..60 {
            g.update(1.0 / 60.0, &idle());
        }
        assert!(g.shield < s);
        assert!(!g.in_dome(g.enemies[0].pos));
    }

    #[test]
    fn shell_on_shield_drains_shield_not_buildings() {
        let mut g = Game::new(7);
        g.spawn_acc = -1e9;
        g.shells.push(Shell {
            from: v2(800.0, 0.0),
            to: V2::ZERO,
            t: 0.0,
            flight: 0.1,
            dmg: 30.0,
            fog: false,
        });
        g.shield_regen_mult = 0.0;
        let s = g.shield;
        g.update(0.2, &idle());
        assert!((g.shield - (s - 30.0)).abs() < 0.01);
        assert_eq!(g.buildings[0].hp, HQ_HP);
    }

    #[test]
    fn level_up_pauses_and_applies_choice() {
        let mut g = Game::new(8);
        g.gain_xp(xp_needed(1));
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        let shots = g.player.shots;
        let dmg = g.player.dmg;
        g.update(
            0.1,
            &Input {
                choice: Some(0),
                ..idle()
            },
        );
        assert_eq!(g.phase, Phase::Playing);
        assert!(g.player.shots > shots || g.player.dmg >= dmg);
    }

    #[test]
    fn beacon_returns_to_base() {
        let mut g = Game::new(9);
        g.player.pos = v2(1500.0, 0.0);
        g.player.hp = 1e6;
        g.update(
            0.01,
            &Input {
                beacon: true,
                ..idle()
            },
        );
        assert!(g.player.channel.is_some());
        for _ in 0..20 {
            g.update(0.1, &idle());
        }
        assert!(g.in_dome(g.player.pos));
        assert!(g.player.beacon_cd > 0.0);
    }

    #[test]
    fn hq_destroyed_loses_and_surviving_wins() {
        let mut g = Game::new(10);
        g.buildings[0].hp = 0.0;
        g.update(0.01, &idle());
        assert_eq!(g.phase, Phase::Lost);

        let mut g = Game::new(11);
        g.time = DURATION + 1.0;
        g.wave = WAVES;
        g.enemies.clear();
        g.update(0.01, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn waves_escalate() {
        let mut g = Game::new(12);
        g.player.hp = 1e9;
        g.buildings[0].hp = 1e9;
        g.update(0.1, &idle());
        assert_eq!(g.wave, 1);
        let r1 = g.spawn_rate();
        g.time = WAVE_TIME * 6.0;
        g.update(0.1, &idle());
        assert!(g.wave >= 7);
        assert!(g.spawn_rate() > r1 * 2.0);
    }
}
