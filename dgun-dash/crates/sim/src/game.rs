//! Game rules: the commander, the swarm, the D-Gun, droppable structures,
//! pickups, levelling and the 10 minute score attack.

use crate::math::{V2, v2};
use crate::rng::Rng;

/// Half the side length of the square arena.
pub const ARENA: f32 = 1400.0;
/// Survive this long (seconds) to win.
pub const DURATION: f32 = 600.0;
/// Seconds without a kill before the combo chain breaks.
pub const COMBO_WINDOW: f32 = 2.0;
/// Combo length beyond which the score multiplier stops growing.
pub const MAX_COMBO_BONUS: u32 = 40;
pub const PLAYER_RADIUS: f32 = 16.0;
pub const MAX_ENEMIES: usize = 360;
pub const DGUN_SPEED: f32 = 900.0;
pub const DGUN_RADIUS: f32 = 26.0;
pub const TURRET_RANGE: f32 = 280.0;
pub const EMP_RADIUS: f32 = 190.0;
pub const PICKUP_LIFE: f32 = 25.0;
pub const BOSS_TIMES: [f32; 3] = [150.0, 330.0, 480.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Basic walker.
    Grunt,
    /// Fast, fragile skirmisher.
    Runner,
    /// Slow armoured tank.
    Tank,
    /// Mini-boss that fires plasma rings.
    Boss,
}

impl EnemyKind {
    /// (hp, speed, radius, contact damage, xp, score)
    pub fn stats(self) -> (f32, f32, f32, f32, u32, u32) {
        match self {
            EnemyKind::Grunt => (14.0, 70.0, 13.0, 8.0, 1, 10),
            EnemyKind::Runner => (8.0, 135.0, 10.0, 6.0, 1, 15),
            EnemyKind::Tank => (70.0, 45.0, 20.0, 16.0, 4, 40),
            EnemyKind::Boss => (2600.0, 55.0, 38.0, 30.0, 40, 2000),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub radius: f32,
    pub speed: f32,
    pub slow: f32,
    pub flash: f32,
    pub fire_cd: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructKind {
    /// Proximity mine.
    Mine,
    /// Pop-up laser turret.
    Turret,
    /// Slowing EMP field.
    Emp,
}

pub const STRUCT_KINDS: [StructKind; 3] = [StructKind::Mine, StructKind::Turret, StructKind::Emp];

impl StructKind {
    pub fn name(self) -> &'static str {
        match self {
            StructKind::Mine => "Mine",
            StructKind::Turret => "Turret",
            StructKind::Emp => "EMP",
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug)]
pub struct Structure {
    pub kind: StructKind,
    pub pos: V2,
    pub life: f32,
    pub max_life: f32,
    pub cd: f32,
    /// Turret aim angle, for drawing.
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: u32,
    pub turret: bool,
}

#[derive(Clone, Debug)]
pub struct EnemyShot {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct DgunShot {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupKind {
    Xp(u32),
    Crate(StructKind),
    Heal,
    Charge,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
    pub pos: V2,
    pub life: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    FireRate,
    Damage,
    Multishot,
    Speed,
    MaxHp,
    Magnet,
    DgunCapacity,
    DgunRecharge,
    TurretTime,
    MineBlast,
    EmpField,
    Supply,
}

pub const ALL_UPGRADES: [Upgrade; 12] = [
    Upgrade::FireRate,
    Upgrade::Damage,
    Upgrade::Multishot,
    Upgrade::Speed,
    Upgrade::MaxHp,
    Upgrade::Magnet,
    Upgrade::DgunCapacity,
    Upgrade::DgunRecharge,
    Upgrade::TurretTime,
    Upgrade::MineBlast,
    Upgrade::EmpField,
    Upgrade::Supply,
];

impl Upgrade {
    pub fn title(self) -> &'static str {
        match self {
            Upgrade::FireRate => "Rapid Laser",
            Upgrade::Damage => "Heavy Laser",
            Upgrade::Multishot => "Split Beam",
            Upgrade::Speed => "Servo Legs",
            Upgrade::MaxHp => "Armour Plating",
            Upgrade::Magnet => "Tractor Field",
            Upgrade::DgunCapacity => "D-Gun Capacitor",
            Upgrade::DgunRecharge => "Kill Converter",
            Upgrade::TurretTime => "Nano Frames",
            Upgrade::MineBlast => "Heavy Mines",
            Upgrade::EmpField => "EMP Overload",
            Upgrade::Supply => "Supply Drop",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::FireRate => "+25% auto-fire rate",
            Upgrade::Damage => "+30% laser damage",
            Upgrade::Multishot => "+1 laser per volley",
            Upgrade::Speed => "+12% move speed",
            Upgrade::MaxHp => "+25 max HP, heal 40",
            Upgrade::Magnet => "+60% pickup radius",
            Upgrade::DgunCapacity => "+1 max D-Gun charge",
            Upgrade::DgunRecharge => "D-Gun recharges with fewer kills",
            Upgrade::TurretTime => "Turrets last +50%, fire faster",
            Upgrade::MineBlast => "Mines: +40% radius and damage",
            Upgrade::EmpField => "EMP lasts longer and burns",
            Upgrade::Supply => "+2 of every structure, +1 capacity",
        }
    }

    fn max_rank(self) -> u32 {
        match self {
            Upgrade::Multishot => 4,
            Upgrade::DgunCapacity => 4,
            Upgrade::DgunRecharge => 4,
            Upgrade::Speed | Upgrade::Magnet => 5,
            _ => 8,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub fire_interval: f32,
    pub damage: f32,
    pub shots: u32,
    pub range: f32,
    pub speed: f32,
    pub magnet: f32,
    pub dgun_max: u32,
    pub kills_per_charge: f32,
    pub turret_time: f32,
    pub turret_interval: f32,
    pub mine_radius: f32,
    pub mine_damage: f32,
    pub emp_time: f32,
    pub emp_dps: f32,
    pub capacity: u32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            fire_interval: 0.42,
            damage: 9.0,
            shots: 1,
            range: 330.0,
            speed: 200.0,
            magnet: 100.0,
            dgun_max: 2,
            kills_per_charge: 25.0,
            turret_time: 10.0,
            turret_interval: 0.3,
            mine_radius: 90.0,
            mine_damage: 60.0,
            emp_time: 6.0,
            emp_dps: 0.0,
            capacity: 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot,
    TurretShot,
    Hit,
    Kill { pos: V2, big: bool },
    Explosion { pos: V2, radius: f32 },
    Dgun,
    DgunEmpty,
    Pickup,
    ChargeReady,
    Built(StructKind),
    NoStock,
    LevelUp,
    PlayerHurt,
    BossSpawn,
    Surge,
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp(Vec<Upgrade>),
    Won,
    Lost,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Desired move direction (need not be normalised).
    pub mv: V2,
    /// Aim point in world coordinates.
    pub aim: V2,
    pub dgun: bool,
    /// Structure slot to drop this frame (0..3).
    pub drop: Option<usize>,
    /// Level-up card picked this frame.
    pub choose: Option<usize>,
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub phase: Phase,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub hurt: f32,
    pub stats: Stats,
    pub ranks: Vec<(Upgrade, u32)>,
    pub fire_cd: f32,
    pub level: u32,
    pub xp: u32,
    pub pending_levels: u32,
    pub score: u64,
    pub kills: u32,
    pub combo: u32,
    pub best_combo: u32,
    pub combo_t: f32,
    pub dgun_charges: u32,
    pub dgun_meter: f32,
    pub inventory: [u32; 3],
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub enemy_shots: Vec<EnemyShot>,
    pub dguns: Vec<DgunShot>,
    pub structures: Vec<Structure>,
    pub pickups: Vec<Pickup>,
    pub events: Vec<Event>,
    spawn_acc: f32,
    next_surge: f32,
    bosses_spawned: usize,
}

pub fn xp_needed(level: u32) -> u32 {
    4 + level * 5 + level * level / 2
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Game {
            rng: Rng::new(seed),
            time: 0.0,
            phase: Phase::Playing,
            pos: V2::ZERO,
            hp: 100.0,
            max_hp: 100.0,
            hurt: 0.0,
            stats: Stats::default(),
            ranks: Vec::new(),
            fire_cd: 0.0,
            level: 1,
            xp: 0,
            pending_levels: 0,
            score: 0,
            kills: 0,
            combo: 0,
            best_combo: 0,
            combo_t: 0.0,
            dgun_charges: 1,
            dgun_meter: 0.0,
            inventory: [3, 2, 1],
            enemies: Vec::new(),
            bullets: Vec::new(),
            enemy_shots: Vec::new(),
            dguns: Vec::new(),
            structures: Vec::new(),
            pickups: Vec::new(),
            events: Vec::new(),
            spawn_acc: 0.0,
            next_surge: 60.0,
            bosses_spawned: 0,
        }
    }

    pub fn rank(&self, u: Upgrade) -> u32 {
        self.ranks
            .iter()
            .find(|(k, _)| *k == u)
            .map_or(0, |(_, r)| *r)
    }

    /// Multiplier applied to kill score by the current combo chain.
    pub fn combo_mult(&self) -> f32 {
        1.0 + self.combo.min(MAX_COMBO_BONUS) as f32 / 10.0
    }

    pub fn alive(&self) -> bool {
        matches!(self.phase, Phase::Playing | Phase::LevelUp(_))
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.min(0.05);
        match &self.phase {
            Phase::LevelUp(choices) => {
                if let Some(i) = input.choose
                    && let Some(&u) = choices.get(i)
                {
                    self.apply_upgrade(u);
                    self.pending_levels = self.pending_levels.saturating_sub(1);
                    self.phase = Phase::Playing;
                    self.maybe_level_up();
                }
                return;
            }
            Phase::Won | Phase::Lost => return,
            Phase::Playing => {}
        }
        self.time += dt;
        self.hurt = (self.hurt - dt).max(0.0);

        // Movement.
        let mv = input.mv.norm();
        self.pos += mv * self.stats.speed * dt;
        let lim = ARENA - PLAYER_RADIUS;
        self.pos.x = self.pos.x.clamp(-lim, lim);
        self.pos.y = self.pos.y.clamp(-lim, lim);

        if input.dgun {
            self.fire_dgun(input.aim);
        }
        if let Some(slot) = input.drop {
            self.drop_structure(slot);
        }

        self.spawn(dt);
        self.auto_fire(dt);
        self.update_structures(dt);
        self.update_bullets(dt);
        self.update_dguns(dt);
        self.update_enemies(dt);
        self.update_enemy_shots(dt);
        self.update_pickups(dt);
        self.reap();

        self.combo_t -= dt;
        if self.combo_t <= 0.0 {
            self.combo = 0;
        }

        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
            return;
        }
        if self.time >= DURATION {
            self.score += (self.hp as u64) * 50;
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }
        self.maybe_level_up();
    }

    fn maybe_level_up(&mut self) {
        if self.phase != Phase::Playing {
            return;
        }
        while self.xp >= xp_needed(self.level) {
            self.xp -= xp_needed(self.level);
            self.level += 1;
            self.pending_levels += 1;
        }
        if self.pending_levels > 0 {
            let choices = self.roll_choices();
            if choices.is_empty() {
                self.pending_levels = 0;
                return;
            }
            self.events.push(Event::LevelUp);
            self.phase = Phase::LevelUp(choices);
        }
    }

    fn roll_choices(&mut self) -> Vec<Upgrade> {
        let mut pool: Vec<Upgrade> = ALL_UPGRADES
            .iter()
            .copied()
            .filter(|u| self.rank(*u) < u.max_rank())
            .collect();
        let mut out = Vec::new();
        while out.len() < 3 && !pool.is_empty() {
            let i = self.rng.below(pool.len());
            out.push(pool.swap_remove(i));
        }
        out
    }

    pub fn apply_upgrade(&mut self, u: Upgrade) {
        let s = &mut self.stats;
        match u {
            Upgrade::FireRate => s.fire_interval /= 1.25,
            Upgrade::Damage => s.damage *= 1.3,
            Upgrade::Multishot => s.shots += 1,
            Upgrade::Speed => s.speed *= 1.12,
            Upgrade::MaxHp => {
                self.max_hp += 25.0;
                self.hp = (self.hp + 40.0).min(self.max_hp);
            }
            Upgrade::Magnet => s.magnet *= 1.6,
            Upgrade::DgunCapacity => {
                s.dgun_max += 1;
                self.dgun_charges += 1;
            }
            Upgrade::DgunRecharge => s.kills_per_charge = (s.kills_per_charge * 0.75).max(6.0),
            Upgrade::TurretTime => {
                s.turret_time *= 1.5;
                s.turret_interval *= 0.85;
            }
            Upgrade::MineBlast => {
                s.mine_radius *= 1.4;
                s.mine_damage *= 1.4;
            }
            Upgrade::EmpField => {
                s.emp_time += 2.0;
                s.emp_dps += 12.0;
            }
            Upgrade::Supply => {
                s.capacity += 1;
                let cap = s.capacity;
                for c in &mut self.inventory {
                    *c = (*c + 2).min(cap);
                }
            }
        }
        if let Some(r) = self.ranks.iter_mut().find(|(k, _)| *k == u) {
            r.1 += 1;
        } else {
            self.ranks.push((u, 1));
        }
    }

    pub fn fire_dgun(&mut self, aim: V2) {
        if self.dgun_charges == 0 {
            self.events.push(Event::DgunEmpty);
            return;
        }
        self.dgun_charges -= 1;
        let mut dir = (aim - self.pos).norm();
        if dir == V2::ZERO {
            dir = v2(1.0, 0.0);
        }
        self.dguns.push(DgunShot {
            pos: self.pos + dir * 20.0,
            vel: dir * DGUN_SPEED,
            life: 1.6,
        });
        self.events.push(Event::Dgun);
    }

    pub fn drop_structure(&mut self, slot: usize) {
        let Some(&kind) = STRUCT_KINDS.get(slot) else {
            return;
        };
        if self.inventory[slot] == 0 {
            self.events.push(Event::NoStock);
            return;
        }
        self.inventory[slot] -= 1;
        let life = match kind {
            StructKind::Mine => 40.0,
            StructKind::Turret => self.stats.turret_time,
            StructKind::Emp => self.stats.emp_time,
        };
        self.structures.push(Structure {
            kind,
            pos: self.pos,
            life,
            max_life: life,
            cd: 0.4,
            angle: 0.0,
        });
        self.events.push(Event::Built(kind));
    }

    /// Spawn rate in enemies per second at time `t`.
    pub fn spawn_rate(t: f32) -> f32 {
        1.2 + t / 28.0 + (t / 120.0).powi(2)
    }

    fn spawn_point(&mut self) -> V2 {
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let d = self.rng.range(620.0, 760.0);
        let p = self.pos + V2::from_angle(a) * d;
        v2(p.x.clamp(-ARENA, ARENA), p.y.clamp(-ARENA, ARENA))
    }

    pub fn spawn_enemy(&mut self, kind: EnemyKind, pos: V2) {
        let (hp, speed, radius, ..) = kind.stats();
        let scale = if kind == EnemyKind::Boss {
            1.0 + self.bosses_spawned as f32 * 0.6
        } else {
            1.0 + self.time / 150.0 + (self.time / 220.0).powi(2)
        };
        let hp = hp * scale;
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            radius,
            speed: speed * self.rng.range(0.9, 1.1) * (1.0 + self.time / 1200.0),
            slow: 0.0,
            flash: 0.0,
            fire_cd: 2.0,
        });
    }

    fn pick_kind(&mut self) -> EnemyKind {
        let t = self.time;
        let r = self.rng.f32();
        if t > 120.0 && r < 0.08 + t / 6000.0 {
            EnemyKind::Tank
        } else if t > 40.0 && r < 0.35 {
            EnemyKind::Runner
        } else {
            EnemyKind::Grunt
        }
    }

    fn spawn(&mut self, dt: f32) {
        if self.bosses_spawned < BOSS_TIMES.len() && self.time >= BOSS_TIMES[self.bosses_spawned] {
            let p = self.spawn_point();
            self.spawn_enemy(EnemyKind::Boss, p);
            self.bosses_spawned += 1;
            self.events.push(Event::BossSpawn);
        }
        if self.time >= self.next_surge {
            self.next_surge += 60.0;
            self.events.push(Event::Surge);
            let n = 18 + (self.time / 20.0) as usize;
            let base = self.rng.range(0.0, std::f32::consts::TAU);
            for i in 0..n {
                if self.enemies.len() >= MAX_ENEMIES {
                    break;
                }
                let a = base + i as f32 / n as f32 * std::f32::consts::TAU;
                let p = self.pos + V2::from_angle(a) * 560.0;
                let k = self.pick_kind();
                self.spawn_enemy(k, v2(p.x.clamp(-ARENA, ARENA), p.y.clamp(-ARENA, ARENA)));
            }
        }
        self.spawn_acc += Self::spawn_rate(self.time) * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            if self.enemies.len() < MAX_ENEMIES {
                let p = self.spawn_point();
                let k = self.pick_kind();
                self.spawn_enemy(k, p);
            }
        }
    }

    fn nearest_enemy(&self, from: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range * range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist2(from);
            if d < bd && e.hp > 0.0 {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn auto_fire(&mut self, dt: f32) {
        self.fire_cd -= dt;
        if self.fire_cd > 0.0 {
            return;
        }
        let Some(t) = self.nearest_enemy(self.pos, self.stats.range) else {
            return;
        };
        self.fire_cd = self.stats.fire_interval;
        let dir = (self.enemies[t].pos - self.pos).norm();
        let base = dir.y.atan2(dir.x);
        let n = self.stats.shots;
        for i in 0..n {
            let off = (i as f32 - (n - 1) as f32 / 2.0) * 0.16;
            self.bullets.push(Bullet {
                pos: self.pos,
                vel: V2::from_angle(base + off) * 620.0,
                dmg: self.stats.damage,
                life: 0.7,
                pierce: 0,
                turret: false,
            });
        }
        self.events.push(Event::Shot);
    }

    fn update_structures(&mut self, dt: f32) {
        let mut blasts = Vec::new();
        let mut shots = Vec::new();
        let mine_r = self.stats.mine_radius;
        let turret_iv = self.stats.turret_interval;
        let emp_dps = self.stats.emp_dps;
        let dmg = self.stats.damage;
        for si in 0..self.structures.len() {
            let s = &mut self.structures[si];
            s.life -= dt;
            s.cd -= dt;
            let pos = s.pos;
            match s.kind {
                StructKind::Mine => {
                    if s.cd <= 0.0
                        && self
                            .enemies
                            .iter()
                            .any(|e| e.pos.dist(pos) < 34.0 + e.radius)
                    {
                        s.life = 0.0;
                        blasts.push(pos);
                    }
                }
                StructKind::Turret => {
                    if s.cd <= 0.0 {
                        let mut best = None;
                        let mut bd = TURRET_RANGE * TURRET_RANGE;
                        for e in &self.enemies {
                            let d = e.pos.dist2(pos);
                            if d < bd {
                                bd = d;
                                best = Some(e.pos);
                            }
                        }
                        if let Some(tp) = best {
                            let s = &mut self.structures[si];
                            s.cd = turret_iv;
                            let dir = (tp - pos).norm();
                            s.angle = dir.y.atan2(dir.x);
                            shots.push(Bullet {
                                pos,
                                vel: dir * 700.0,
                                dmg: dmg * 0.9 + 4.0,
                                life: 0.5,
                                pierce: 1,
                                turret: true,
                            });
                        }
                    }
                }
                StructKind::Emp => {
                    for e in &mut self.enemies {
                        if e.pos.dist(pos) < EMP_RADIUS {
                            e.slow = 0.3;
                            if emp_dps > 0.0 {
                                e.hp -= emp_dps * dt;
                            }
                        }
                    }
                }
            }
        }
        if !shots.is_empty() {
            self.events.push(Event::TurretShot);
            self.bullets.extend(shots);
        }
        for p in blasts {
            self.explode(p, mine_r, self.stats.mine_damage);
        }
        self.structures.retain(|s| s.life > 0.0);
    }

    fn explode(&mut self, pos: V2, radius: f32, dmg: f32) {
        for e in &mut self.enemies {
            if e.pos.dist(pos) < radius + e.radius {
                e.hp -= dmg;
                e.flash = 0.1;
            }
        }
        self.events.push(Event::Explosion { pos, radius });
    }

    fn update_bullets(&mut self, dt: f32) {
        let mut hit = false;
        for b in &mut self.bullets {
            b.pos += b.vel * dt;
            b.life -= dt;
            for e in &mut self.enemies {
                if e.hp > 0.0 && b.life > 0.0 && e.pos.dist2(b.pos) < (e.radius + 4.0).powi(2) {
                    e.hp -= b.dmg;
                    e.flash = 0.08;
                    hit = true;
                    if b.pierce == 0 {
                        b.life = 0.0;
                    } else {
                        b.pierce -= 1;
                    }
                }
            }
        }
        if hit {
            self.events.push(Event::Hit);
        }
        self.bullets.retain(|b| b.life > 0.0);
    }

    fn update_dguns(&mut self, dt: f32) {
        for d in &mut self.dguns {
            let from = d.pos;
            d.pos += d.vel * dt;
            d.life -= dt;
            for e in &mut self.enemies {
                if e.hp > 0.0 && e.pos.seg_dist(from, d.pos) < DGUN_RADIUS + e.radius {
                    if e.kind == EnemyKind::Boss {
                        // Bosses take a big chunk once per shot pass.
                        if e.flash <= 0.0 {
                            e.hp -= 600.0;
                        }
                    } else {
                        e.hp = 0.0;
                    }
                    e.flash = 0.25;
                }
            }
            self.enemy_shots
                .retain(|s| s.pos.seg_dist(from, d.pos) > DGUN_RADIUS);
            if d.pos.x.abs() > ARENA || d.pos.y.abs() > ARENA {
                d.life = 0.0;
            }
        }
        self.dguns.retain(|d| d.life > 0.0);
    }

    fn update_enemies(&mut self, dt: f32) {
        let player = self.pos;
        let mut dmg_taken = 0.0;
        let mut new_shots = Vec::new();
        let n = self.enemies.len();
        for i in 0..n {
            let e = &mut self.enemies[i];
            e.flash = (e.flash - dt).max(0.0);
            let slow = if e.slow > 0.0 { 0.4 } else { 1.0 };
            e.slow = (e.slow - dt).max(0.0);
            let dir = (player - e.pos).norm();
            e.pos += dir * e.speed * slow * dt;
            if e.pos.dist(player) < e.radius + PLAYER_RADIUS {
                dmg_taken += e.kind.stats().3 * dt * 2.0;
            }
            if e.kind == EnemyKind::Boss {
                e.fire_cd -= dt;
                if e.fire_cd <= 0.0 {
                    e.fire_cd = 2.6;
                    let k = 14;
                    let off = self.time;
                    for j in 0..k {
                        let a = off + j as f32 / k as f32 * std::f32::consts::TAU;
                        new_shots.push(EnemyShot {
                            pos: e.pos,
                            vel: V2::from_angle(a) * 190.0,
                            life: 4.0,
                        });
                    }
                }
            }
        }
        // Cheap separation so the swarm spreads out.
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
                let min = self.enemies[i].radius + self.enemies[j].radius;
                let d2 = a.dist2(b);
                if d2 < min * min && d2 > 1e-4 {
                    let d = d2.sqrt();
                    let push = (b - a) * ((min - d) / d * 0.5);
                    let bi = self.enemies[i].kind == EnemyKind::Boss;
                    let bj = self.enemies[j].kind == EnemyKind::Boss;
                    if !bi {
                        self.enemies[i].pos -= push;
                    }
                    if !bj {
                        self.enemies[j].pos += push;
                    }
                }
            }
        }
        self.enemy_shots.extend(new_shots);
        if dmg_taken > 0.0 {
            self.damage_player(dmg_taken);
        }
    }

    fn damage_player(&mut self, dmg: f32) {
        self.hp -= dmg;
        if self.hurt <= 0.0 {
            self.events.push(Event::PlayerHurt);
            self.hurt = 0.25;
        }
    }

    fn update_enemy_shots(&mut self, dt: f32) {
        let mut dmg = 0.0;
        let p = self.pos;
        for s in &mut self.enemy_shots {
            s.pos += s.vel * dt;
            s.life -= dt;
            if s.pos.dist(p) < PLAYER_RADIUS + 6.0 {
                s.life = 0.0;
                dmg += 10.0;
            }
        }
        self.enemy_shots.retain(|s| s.life > 0.0);
        if dmg > 0.0 {
            self.damage_player(dmg);
        }
    }

    fn update_pickups(&mut self, dt: f32) {
        let p = self.pos;
        let magnet = self.stats.magnet;
        let mut got = Vec::new();
        for k in &mut self.pickups {
            k.life -= dt;
            let d = k.pos.dist(p);
            if d < magnet {
                k.pos += (p - k.pos).norm() * (420.0 * dt).min(d);
            }
            if d < PLAYER_RADIUS + 8.0 {
                k.life = 0.0;
                got.push(k.kind);
            }
        }
        self.pickups.retain(|k| k.life > 0.0);
        for kind in got {
            self.collect(kind);
        }
    }

    pub fn collect(&mut self, kind: PickupKind) {
        match kind {
            PickupKind::Xp(v) => self.xp += v,
            PickupKind::Crate(s) => {
                let i = s.index();
                self.inventory[i] = (self.inventory[i] + 1).min(self.stats.capacity);
            }
            PickupKind::Heal => self.hp = (self.hp + 25.0).min(self.max_hp),
            PickupKind::Charge => self.add_charge(),
        }
        self.events.push(Event::Pickup);
    }

    fn add_charge(&mut self) {
        if self.dgun_charges < self.stats.dgun_max {
            self.dgun_charges += 1;
            self.events.push(Event::ChargeReady);
        }
    }

    /// Records one kill: extends the combo chain and feeds the D-Gun meter.
    pub fn register_kill(&mut self, kind: EnemyKind) {
        self.kills += 1;
        self.combo += 1;
        self.best_combo = self.best_combo.max(self.combo);
        self.combo_t = COMBO_WINDOW;
        let score = kind.stats().5 as f32 * self.combo_mult();
        self.score += score as u64;
        // Long chains feed the D-Gun faster.
        let gain = 1.0 + (self.combo / 10) as f32 * 0.5;
        if self.dgun_charges < self.stats.dgun_max {
            self.dgun_meter += gain;
            if self.dgun_meter >= self.stats.kills_per_charge {
                self.dgun_meter -= self.stats.kills_per_charge;
                self.add_charge();
            }
        } else {
            self.dgun_meter = 0.0;
        }
    }

    fn reap(&mut self) {
        let mut dead = Vec::new();
        self.enemies.retain(|e| {
            if e.hp <= 0.0 {
                dead.push((e.kind, e.pos));
                false
            } else {
                true
            }
        });
        for (kind, pos) in dead {
            self.register_kill(kind);
            let big = matches!(kind, EnemyKind::Tank | EnemyKind::Boss);
            self.events.push(Event::Kill { pos, big });
            let xp = kind.stats().4;
            self.pickups.push(Pickup {
                kind: PickupKind::Xp(xp),
                pos,
                life: PICKUP_LIFE,
            });
            let roll = self.rng.f32();
            let drop = if kind == EnemyKind::Boss {
                Some(PickupKind::Charge)
            } else if roll < 0.035 {
                let k = STRUCT_KINDS[self.rng.below(3)];
                Some(PickupKind::Crate(k))
            } else if roll < 0.045 {
                Some(PickupKind::Heal)
            } else if roll < 0.05 {
                Some(PickupKind::Charge)
            } else {
                None
            };
            if let Some(k) = drop {
                let off = v2(self.rng.range(-12.0, 12.0), self.rng.range(-12.0, 12.0));
                self.pickups.push(Pickup {
                    kind: k,
                    pos: pos + off,
                    life: PICKUP_LIFE,
                });
            }
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
    fn player_moves_and_stays_in_arena() {
        let mut g = Game::new(1);
        let inp = Input {
            mv: v2(1.0, 0.0),
            ..idle()
        };
        g.update(0.05, &inp);
        assert!(g.pos.x > 0.0);
        g.pos = v2(ARENA, 0.0);
        g.update(0.05, &inp);
        assert!(g.pos.x <= ARENA);
    }

    #[test]
    fn kills_recharge_dgun_and_combo_multiplies_score() {
        let mut g = Game::new(2);
        g.dgun_charges = 0;
        let kpc = g.stats.kills_per_charge as u32;
        for _ in 0..kpc {
            g.register_kill(EnemyKind::Grunt);
        }
        assert_eq!(g.dgun_charges, 1);
        assert_eq!(g.combo, kpc);
        // Later kills in a chain score more than the base value.
        let before = g.score;
        g.register_kill(EnemyKind::Grunt);
        assert!(g.score - before > 10);
    }

    #[test]
    fn combo_breaks_after_window() {
        let mut g = Game::new(3);
        g.register_kill(EnemyKind::Grunt);
        assert_eq!(g.combo, 1);
        for _ in 0..50 {
            g.update(0.05, &idle());
        }
        assert_eq!(g.combo, 0);
    }

    #[test]
    fn dgun_needs_charge_and_obliterates_line() {
        let mut g = Game::new(4);
        g.dgun_charges = 1;
        g.spawn_enemy(EnemyKind::Tank, v2(200.0, 0.0));
        g.spawn_enemy(EnemyKind::Grunt, v2(300.0, 5.0));
        g.spawn_enemy(EnemyKind::Grunt, v2(0.0, 300.0));
        let fire = Input {
            aim: v2(500.0, 0.0),
            dgun: true,
            ..idle()
        };
        g.update(0.01, &fire);
        assert_eq!(g.dgun_charges, 0);
        for _ in 0..20 {
            g.update(0.02, &idle());
        }
        assert!(g.kills >= 2, "kills {}", g.kills);
        g.events.clear();
        g.fire_dgun(v2(1.0, 0.0));
        assert!(g.events.contains(&Event::DgunEmpty));
    }

    #[test]
    fn structures_consume_inventory_and_expire() {
        let mut g = Game::new(5);
        let n = g.inventory[1];
        g.update(
            0.01,
            &Input {
                drop: Some(1),
                ..idle()
            },
        );
        assert_eq!(g.inventory[1], n - 1);
        assert_eq!(g.structures.len(), 1);
        g.inventory[2] = 0;
        g.drop_structure(2);
        assert!(g.events.contains(&Event::NoStock));
        g.structures[0].life = 0.01;
        g.update(0.05, &idle());
        assert!(g.structures.is_empty());
    }

    #[test]
    fn mine_explodes_on_contact() {
        let mut g = Game::new(6);
        g.drop_structure(0);
        g.structures[0].cd = 0.0;
        g.pos = v2(500.0, 500.0);
        g.spawn_enemy(EnemyKind::Grunt, v2(10.0, 0.0));
        g.update(0.01, &idle());
        assert!(g.structures.is_empty());
        assert!(g.kills >= 1);
    }

    #[test]
    fn crate_pickup_adds_capped_charges() {
        let mut g = Game::new(7);
        g.inventory[0] = g.stats.capacity;
        g.collect(PickupKind::Crate(StructKind::Mine));
        assert_eq!(g.inventory[0], g.stats.capacity);
        g.inventory[2] = 0;
        g.collect(PickupKind::Crate(StructKind::Emp));
        assert_eq!(g.inventory[2], 1);
    }

    #[test]
    fn xp_triggers_level_up_choice_and_pauses() {
        let mut g = Game::new(8);
        g.xp = xp_needed(1);
        g.update(0.01, &idle());
        let Phase::LevelUp(choices) = g.phase.clone() else {
            panic!("expected level up");
        };
        assert_eq!(choices.len(), 3);
        let t = g.time;
        g.update(0.1, &idle());
        assert_eq!(g.time, t, "time frozen while choosing");
        g.update(
            0.01,
            &Input {
                choose: Some(0),
                ..idle()
            },
        );
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.rank(choices[0]), 1);
        assert_eq!(g.level, 2);
    }

    #[test]
    fn upgrades_respect_max_rank() {
        let mut g = Game::new(9);
        for _ in 0..Upgrade::Multishot.max_rank() {
            g.apply_upgrade(Upgrade::Multishot);
        }
        for _ in 0..50 {
            assert!(!g.roll_choices().contains(&Upgrade::Multishot));
        }
    }

    #[test]
    fn surviving_the_clock_wins() {
        let mut g = Game::new(10);
        g.time = DURATION - 0.01;
        g.next_surge = 1e9;
        g.bosses_spawned = BOSS_TIMES.len();
        g.update(0.05, &idle());
        assert_eq!(g.phase, Phase::Won);
        assert!(g.events.contains(&Event::Won));
    }

    #[test]
    fn dying_loses() {
        let mut g = Game::new(11);
        g.hp = 1.0;
        g.spawn_enemy(EnemyKind::Tank, v2(5.0, 0.0));
        for _ in 0..40 {
            g.update(0.05, &idle());
        }
        assert_eq!(g.phase, Phase::Lost);
        assert!(!g.alive());
    }

    #[test]
    fn waves_escalate_and_bosses_arrive() {
        assert!(Game::spawn_rate(300.0) > Game::spawn_rate(0.0) * 4.0);
        let mut g = Game::new(12);
        g.hp = 1e9;
        g.max_hp = 1e9;
        g.time = BOSS_TIMES[0] - 0.01;
        g.update(0.05, &idle());
        assert!(g.enemies.iter().any(|e| e.kind == EnemyKind::Boss));
        assert!(g.events.contains(&Event::BossSpawn));
    }

    #[test]
    fn long_run_is_stable() {
        let mut g = Game::new(13);
        let mut t = 0;
        while g.alive() && t < 20_000 {
            let inp = Input {
                mv: V2::from_angle(t as f32 * 0.01),
                aim: g.pos + v2(1.0, 0.0),
                dgun: t % 300 == 0,
                drop: (t % 200 == 0).then_some((t / 200) % 3),
                choose: Some(0),
            };
            g.update(1.0 / 30.0, &inp);
            g.events.clear();
            t += 1;
            assert!(g.enemies.len() <= MAX_ENEMIES + 40);
        }
        assert!(g.kills > 50);
    }
}
