//! Core simulation: the commander, blueprints, defenses, enemy waves,
//! metal economy and level-up upgrades.

use crate::geom::{V2, v2};
use crate::rng::Rng;

pub const WORLD_W: f32 = 2400.0;
pub const WORLD_H: f32 = 1600.0;
pub const WAVES: u32 = 12;
pub const WAVE_SECS: f32 = 35.0;
pub const MAX_BLUEPRINTS: usize = 6;
const START_METAL: f32 = 180.0;
const BASE_INCOME: f32 = 1.5;
const PICKUP_RADIUS: f32 = 36.0;
const MAGNET_RADIUS: f32 = 140.0;
const AGGRO_RADIUS: f32 = 150.0;
/// Hit points restored per unit of buildpower per second when repairing.
const REPAIR_RATE: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wall,
    Laser,
    Mex,
    Nano,
    Heavy,
}

pub const KINDS: [Kind; 5] = [Kind::Wall, Kind::Laser, Kind::Mex, Kind::Nano, Kind::Heavy];

pub struct Spec {
    pub name: &'static str,
    pub cost: f32,
    /// Buildpower-seconds needed to finish.
    pub work: f32,
    pub hp: f32,
    pub radius: f32,
    pub range: f32,
    pub damage: f32,
    pub reload: f32,
    pub bp: f32,
    pub income: f32,
}

impl Kind {
    pub fn spec(self) -> Spec {
        let base = Spec {
            name: "",
            cost: 0.0,
            work: 0.0,
            hp: 0.0,
            radius: 18.0,
            range: 0.0,
            damage: 0.0,
            reload: 1.0,
            bp: 0.0,
            income: 0.0,
        };
        match self {
            Kind::Wall => Spec {
                name: "Dragon Wall",
                cost: 12.0,
                work: 25.0,
                hp: 520.0,
                radius: 20.0,
                ..base
            },
            Kind::Laser => Spec {
                name: "Laser Tower",
                cost: 45.0,
                work: 80.0,
                hp: 260.0,
                range: 220.0,
                damage: 13.0,
                reload: 0.45,
                ..base
            },
            Kind::Mex => Spec {
                name: "Metal Extractor",
                cost: 35.0,
                work: 70.0,
                hp: 300.0,
                radius: 20.0,
                income: 1.4,
                ..base
            },
            Kind::Nano => Spec {
                name: "Nano Turret",
                cost: 70.0,
                work: 130.0,
                hp: 220.0,
                range: 240.0,
                bp: 16.0,
                ..base
            },
            Kind::Heavy => Spec {
                name: "Heavy Laser",
                cost: 130.0,
                work: 260.0,
                hp: 720.0,
                radius: 24.0,
                range: 310.0,
                damage: 75.0,
                reload: 1.3,
                ..base
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Building {
    pub id: u32,
    pub kind: Kind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Construction progress in [0, 1]; 1 means complete.
    pub progress: f32,
    pub cooldown: f32,
    /// Position currently receiving this building's nano beam, if any.
    pub beam: Option<V2>,
}

impl Building {
    pub fn done(&self) -> bool {
        self.progress >= 1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EKind {
    Grunt,
    Raider,
    Brute,
    Sumo,
    Boss,
}

pub struct ESpec {
    pub hp: f32,
    pub speed: f32,
    pub dps: f32,
    pub radius: f32,
    pub metal: f32,
    pub xp: f32,
}

impl EKind {
    pub fn spec(self) -> ESpec {
        match self {
            EKind::Grunt => ESpec {
                hp: 30.0,
                speed: 62.0,
                dps: 10.0,
                radius: 12.0,
                metal: 3.0,
                xp: 1.0,
            },
            EKind::Raider => ESpec {
                hp: 20.0,
                speed: 115.0,
                dps: 7.0,
                radius: 10.0,
                metal: 2.0,
                xp: 1.0,
            },
            EKind::Brute => ESpec {
                hp: 120.0,
                speed: 46.0,
                dps: 24.0,
                radius: 16.0,
                metal: 8.0,
                xp: 3.0,
            },
            EKind::Sumo => ESpec {
                hp: 420.0,
                speed: 34.0,
                dps: 50.0,
                radius: 22.0,
                metal: 25.0,
                xp: 10.0,
            },
            EKind::Boss => ESpec {
                hp: 6000.0,
                speed: 28.0,
                dps: 160.0,
                radius: 42.0,
                metal: 200.0,
                xp: 50.0,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// True while chewing on something this tick.
    pub biting: bool,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub pos: V2,
    pub metal: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    BuildPower,
    BuildRange,
    TurretDamage,
    Armor,
    Speed,
    NanoPower,
    MexOutput,
    Plating,
}

pub const UPGRADES: [Upgrade; 8] = [
    Upgrade::BuildPower,
    Upgrade::BuildRange,
    Upgrade::TurretDamage,
    Upgrade::Armor,
    Upgrade::Speed,
    Upgrade::NanoPower,
    Upgrade::MexOutput,
    Upgrade::Plating,
];

impl Upgrade {
    pub fn title(self) -> &'static str {
        match self {
            Upgrade::BuildPower => "Nano Lathe",
            Upgrade::BuildRange => "Long Beam",
            Upgrade::TurretDamage => "Focused Lasers",
            Upgrade::Armor => "Commander Armor",
            Upgrade::Speed => "Servo Legs",
            Upgrade::NanoPower => "Nano Swarm",
            Upgrade::MexOutput => "Deep Drills",
            Upgrade::Plating => "Reinforced Plating",
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Upgrade::BuildPower => "+30% commander buildpower",
            Upgrade::BuildRange => "+20% nano beam range",
            Upgrade::TurretDamage => "+20% tower damage",
            Upgrade::Armor => "+120 max HP, full heal",
            Upgrade::Speed => "+12% move speed",
            Upgrade::NanoPower => "+35% nano turret buildpower",
            Upgrade::MexOutput => "+40% metal income",
            Upgrade::Plating => "+25% building HP",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot { from: V2, to: V2, heavy: bool },
    EnemyKilled { pos: V2, big: bool },
    BuildingLost { pos: V2 },
    Placed,
    Denied,
    Completed { pos: V2 },
    Pickup,
    LevelUp,
    WaveStart(u32),
    CommanderHurt,
    GameOver,
    Victory,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub repair: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    Lost,
    Won,
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub bp: f32,
    pub range: f32,
    pub nano_mult: f32,
    pub damage_mult: f32,
    pub income_mult: f32,
    pub hp_mult: f32,
    pub metal: f32,
    pub xp: f32,
    pub level: u32,
    pub choices: Vec<Upgrade>,
    pending_levels: u32,
    pub wave: u32,
    pub wave_time: f32,
    spawn_timer: f32,
    boss_spawned: bool,
    pub buildings: Vec<Building>,
    pub enemies: Vec<Enemy>,
    pub pickups: Vec<Pickup>,
    /// Target of the commander's nano beam this tick.
    pub beam: Option<V2>,
    pub outcome: Outcome,
    pub kills: u32,
    next_id: u32,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Game {
            rng: Rng::new(seed),
            time: 0.0,
            pos: v2(WORLD_W / 2.0, WORLD_H / 2.0),
            hp: 400.0,
            max_hp: 400.0,
            speed: 165.0,
            bp: 22.0,
            range: 170.0,
            nano_mult: 1.0,
            damage_mult: 1.0,
            income_mult: 1.0,
            hp_mult: 1.0,
            metal: START_METAL,
            xp: 0.0,
            level: 1,
            choices: Vec::new(),
            pending_levels: 0,
            wave: 0,
            wave_time: 6.0,
            spawn_timer: 0.0,
            boss_spawned: false,
            buildings: Vec::new(),
            enemies: Vec::new(),
            pickups: Vec::new(),
            beam: None,
            outcome: Outcome::Playing,
            kills: 0,
            next_id: 1,
        }
    }

    pub fn xp_needed(&self) -> f32 {
        8.0 + 7.0 * (self.level - 1) as f32 + 0.6 * ((self.level - 1) as f32).powi(2)
    }

    pub fn income(&self) -> f32 {
        let mex: f32 = self
            .buildings
            .iter()
            .filter(|b| b.done())
            .map(|b| b.kind.spec().income)
            .sum();
        (BASE_INCOME + mex) * self.income_mult
    }

    /// Total buildpower: commander plus every finished nano turret.
    pub fn total_bp(&self) -> f32 {
        let nanos = self
            .buildings
            .iter()
            .filter(|b| b.done() && b.kind == Kind::Nano)
            .count() as f32;
        self.bp + nanos * Kind::Nano.spec().bp * self.nano_mult
    }

    pub fn blueprints(&self) -> usize {
        self.buildings.iter().filter(|b| !b.done()).count()
    }

    /// Whether a building of `kind` may be placed at `at`.
    pub fn can_place(&self, kind: Kind, at: V2) -> bool {
        let r = kind.spec().radius;
        if at.x < r || at.y < r || at.x > WORLD_W - r || at.y > WORLD_H - r {
            return false;
        }
        if self.blueprints() >= MAX_BLUEPRINTS || self.metal < kind.spec().cost * 0.25 {
            return false;
        }
        if at.dist(self.pos) < r + 14.0 {
            return false;
        }
        self.buildings
            .iter()
            .all(|b| b.pos.dist(at) >= b.kind.spec().radius + r)
    }

    /// Place a blueprint. Metal is drained gradually while it is built.
    pub fn place(&mut self, kind: Kind, at: V2) -> Event {
        if self.outcome != Outcome::Playing || !self.can_place(kind, at) {
            return Event::Denied;
        }
        self.buildings.push(Building {
            id: self.next_id,
            kind,
            pos: at,
            hp: 1.0,
            max_hp: kind.spec().hp * self.hp_mult,
            progress: 0.0,
            cooldown: 0.0,
            beam: None,
        });
        self.next_id += 1;
        Event::Placed
    }

    pub fn choose(&mut self, idx: usize) -> bool {
        let Some(&up) = self.choices.get(idx) else {
            return false;
        };
        match up {
            Upgrade::BuildPower => self.bp *= 1.3,
            Upgrade::BuildRange => self.range *= 1.2,
            Upgrade::TurretDamage => self.damage_mult *= 1.2,
            Upgrade::Armor => {
                self.max_hp += 120.0;
                self.hp = self.max_hp;
            }
            Upgrade::Speed => self.speed *= 1.12,
            Upgrade::NanoPower => self.nano_mult *= 1.35,
            Upgrade::MexOutput => self.income_mult *= 1.4,
            Upgrade::Plating => {
                self.hp_mult *= 1.25;
                for b in &mut self.buildings {
                    b.max_hp *= 1.25;
                    b.hp *= 1.25;
                }
            }
        }
        self.choices.clear();
        if self.pending_levels > 0 {
            self.pending_levels -= 1;
            self.roll_choices();
        }
        true
    }

    fn roll_choices(&mut self) {
        let mut pool = UPGRADES.to_vec();
        self.choices.clear();
        for _ in 0..3 {
            let i = self.rng.index(pool.len());
            self.choices.push(pool.swap_remove(i));
        }
    }

    /// Advance the simulation. Paused while a level-up choice is pending.
    pub fn update(&mut self, dt: f32, input: Input, ev: &mut Vec<Event>) {
        if self.outcome != Outcome::Playing || !self.choices.is_empty() {
            return;
        }
        self.time += dt;
        self.move_commander(dt, input);
        self.metal += self.income() * dt;
        self.waves(dt, ev);
        self.construct(dt, input.repair, ev);
        self.towers(dt, ev);
        self.enemies_act(dt, ev);
        self.collect(dt, ev);
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.outcome = Outcome::Lost;
            ev.push(Event::GameOver);
        } else if self.wave >= WAVES && self.boss_spawned && self.enemies.is_empty() {
            self.outcome = Outcome::Won;
            ev.push(Event::Victory);
        }
    }

    fn move_commander(&mut self, dt: f32, input: Input) {
        let mut p = self.pos + input.dir.norm() * (self.speed * dt);
        p.x = p.x.clamp(16.0, WORLD_W - 16.0);
        p.y = p.y.clamp(16.0, WORLD_H - 16.0);
        // Slide out of finished buildings.
        for b in self.buildings.iter().filter(|b| b.done()) {
            let min = b.kind.spec().radius + 14.0;
            let d = p.dist(b.pos);
            if d < min {
                p = b.pos + (p - b.pos).norm() * min;
            }
        }
        self.pos = p;
    }

    /// Pick the target for a builder at `at` with reach `range`. Blueprints
    /// come first unless `repair` is set; returns a building index.
    fn build_target(&self, at: V2, range: f32, repair: bool, skip: Option<u32>) -> Option<usize> {
        let in_range = |b: &Building| b.pos.dist(at) <= range + b.kind.spec().radius;
        let blueprint = self
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.done() && in_range(b))
            .min_by(|a, b| {
                // Prefer the most advanced blueprint so things finish.
                b.1.progress.total_cmp(&a.1.progress)
            })
            .map(|(i, _)| i);
        let damaged = self
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.done() && b.hp < b.max_hp && Some(b.id) != skip && in_range(b))
            .min_by(|a, b| (a.1.hp / a.1.max_hp).total_cmp(&(b.1.hp / b.1.max_hp)))
            .map(|(i, _)| i);
        if repair {
            damaged.or(blueprint)
        } else {
            blueprint.or(damaged)
        }
    }

    fn apply_work(&mut self, i: usize, bp: f32, dt: f32, ev: &mut Vec<Event>) {
        let b = &mut self.buildings[i];
        let spec = b.kind.spec();
        if b.done() {
            b.hp = (b.hp + bp * REPAIR_RATE * dt).min(b.max_hp);
            return;
        }
        let mut dp = (bp * dt / spec.work).min(1.0 - b.progress);
        let cost = dp * spec.cost;
        if cost > self.metal {
            dp *= self.metal / cost;
        }
        self.metal -= dp * spec.cost;
        b.progress += dp;
        b.hp = (b.hp + dp * b.max_hp).min(b.max_hp);
        if b.progress >= 1.0 - 1e-5 {
            b.progress = 1.0;
            ev.push(Event::Completed { pos: b.pos });
        }
    }

    fn construct(&mut self, dt: f32, repair: bool, ev: &mut Vec<Event>) {
        self.beam = None;
        if let Some(i) = self.build_target(self.pos, self.range, repair, None) {
            self.beam = Some(self.buildings[i].pos);
            self.apply_work(i, self.bp, dt, ev);
        }
        let nano = Kind::Nano.spec();
        for n in 0..self.buildings.len() {
            self.buildings[n].beam = None;
            if !(self.buildings[n].done() && self.buildings[n].kind == Kind::Nano) {
                continue;
            }
            let at = self.buildings[n].pos;
            let id = self.buildings[n].id;
            let bp = nano.bp * self.nano_mult;
            if let Some(i) = self.build_target(at, nano.range, false, Some(id)) {
                self.buildings[n].beam = Some(self.buildings[i].pos);
                self.apply_work(i, bp, dt, ev);
            } else if self.pos.dist(at) <= nano.range && self.hp < self.max_hp {
                // Idle nano turrets patch up the commander.
                self.buildings[n].beam = Some(self.pos);
                self.hp = (self.hp + bp * dt).min(self.max_hp);
            }
        }
    }

    fn towers(&mut self, dt: f32, ev: &mut Vec<Event>) {
        for b in &mut self.buildings {
            if !b.done() {
                continue;
            }
            let spec = b.kind.spec();
            if spec.damage <= 0.0 {
                continue;
            }
            b.cooldown -= dt;
            if b.cooldown > 0.0 {
                continue;
            }
            let target = self
                .enemies
                .iter_mut()
                .filter(|e| e.hp > 0.0 && e.pos.dist(b.pos) <= spec.range + e.kind.spec().radius)
                .min_by(|a, c| a.pos.dist(b.pos).total_cmp(&c.pos.dist(b.pos)));
            if let Some(e) = target {
                e.hp -= spec.damage * self.damage_mult;
                e.flash = 0.1;
                b.cooldown = spec.reload;
                ev.push(Event::Shot {
                    from: b.pos,
                    to: e.pos,
                    heavy: b.kind == Kind::Heavy,
                });
            }
        }
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let s = e.kind.spec();
                self.kills += 1;
                self.pickups.push(Pickup {
                    pos: e.pos,
                    metal: s.metal,
                });
                self.gain_xp(s.xp, ev);
                ev.push(Event::EnemyKilled {
                    pos: e.pos,
                    big: matches!(e.kind, EKind::Sumo | EKind::Boss),
                });
            } else {
                i += 1;
            }
        }
    }

    fn gain_xp(&mut self, xp: f32, ev: &mut Vec<Event>) {
        self.xp += xp;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            ev.push(Event::LevelUp);
            if self.choices.is_empty() {
                self.roll_choices();
            } else {
                self.pending_levels += 1;
            }
        }
    }

    fn enemies_act(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let mut hurt = false;
        for k in 0..self.enemies.len() {
            let (pos, spec) = {
                let e = &self.enemies[k];
                (e.pos, e.kind.spec())
            };
            self.enemies[k].flash = (self.enemies[k].flash - dt).max(0.0);
            // Chew whatever building is touching us.
            let touching = self
                .buildings
                .iter()
                .position(|b| b.pos.dist(pos) <= b.kind.spec().radius + spec.radius + 2.0);
            if let Some(bi) = touching {
                self.buildings[bi].hp -= spec.dps * dt;
                self.enemies[k].biting = true;
                continue;
            }
            if self.pos.dist(pos) <= spec.radius + 16.0 {
                self.hp -= spec.dps * dt;
                self.enemies[k].biting = true;
                hurt = true;
                continue;
            }
            self.enemies[k].biting = false;
            let target = self
                .buildings
                .iter()
                .map(|b| b.pos)
                .filter(|p| p.dist(pos) < AGGRO_RADIUS)
                .min_by(|a, b| a.dist(pos).total_cmp(&b.dist(pos)))
                .unwrap_or(self.pos);
            let mut step = (target - pos).norm() * (spec.speed * dt);
            // Light separation so swarms spread around obstacles.
            for (j, o) in self.enemies.iter().enumerate() {
                if j == k {
                    continue;
                }
                let d = pos.dist(o.pos);
                let min = spec.radius + o.kind.spec().radius;
                if d < min && d > 0.01 {
                    step += (pos - o.pos).norm() * ((min - d) * 0.5);
                }
            }
            self.enemies[k].pos = pos + step;
        }
        if hurt {
            ev.push(Event::CommanderHurt);
        }
        let mut i = 0;
        while i < self.buildings.len() {
            if self.buildings[i].hp <= 0.0 {
                let b = self.buildings.swap_remove(i);
                ev.push(Event::BuildingLost { pos: b.pos });
            } else {
                i += 1;
            }
        }
    }

    fn collect(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let me = self.pos;
        let mut gained = 0.0;
        self.pickups.retain_mut(|p| {
            let d = p.pos.dist(me);
            if d < PICKUP_RADIUS {
                gained += p.metal;
                return false;
            }
            if d < MAGNET_RADIUS {
                p.pos += (me - p.pos).norm() * (320.0 * dt);
            }
            true
        });
        if gained > 0.0 {
            self.metal += gained;
            ev.push(Event::Pickup);
        }
    }

    fn waves(&mut self, dt: f32, ev: &mut Vec<Event>) {
        self.wave_time -= dt;
        if self.wave_time <= 0.0 && self.wave < WAVES {
            self.wave += 1;
            self.wave_time = WAVE_SECS;
            ev.push(Event::WaveStart(self.wave));
            if self.wave == WAVES && !self.boss_spawned {
                self.boss_spawned = true;
                self.spawn(EKind::Boss);
            }
        }
        if self.wave == 0 {
            return;
        }
        // During the final wave the timer stops at zero but spawns continue
        // until the boss is dead.
        self.wave_time = self.wave_time.max(0.0);
        if self.boss_spawned && !self.enemies.iter().any(|e| e.kind == EKind::Boss) {
            return;
        }
        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 {
            return;
        }
        let w = self.wave as f32;
        self.spawn_timer = (1.7 - 0.11 * w).max(0.35);
        let group = 1 + self.wave / 3;
        for _ in 0..group {
            let roll = self.rng.f32();
            let kind = if self.wave >= 5 && roll < 0.04 + 0.01 * w {
                EKind::Sumo
            } else if self.wave >= 3 && roll < 0.2 + 0.015 * w {
                EKind::Brute
            } else if self.wave >= 2 && roll < 0.5 {
                EKind::Raider
            } else {
                EKind::Grunt
            };
            self.spawn(kind);
        }
    }

    fn spawn(&mut self, kind: EKind) {
        let s = kind.spec();
        let t = self.rng.f32();
        let pos = match self.rng.index(4) {
            0 => v2(t * WORLD_W, -s.radius),
            1 => v2(t * WORLD_W, WORLD_H + s.radius),
            2 => v2(-s.radius, t * WORLD_H),
            _ => v2(WORLD_W + s.radius, t * WORLD_H),
        };
        let hp = s.hp * (1.0 + 0.09 * self.wave as f32);
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            biting: false,
            flash: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(g: &mut Game, secs: f32, input: Input) -> Vec<Event> {
        let mut ev = Vec::new();
        let mut t = 0.0;
        while t < secs {
            g.update(1.0 / 60.0, input, &mut ev);
            t += 1.0 / 60.0;
        }
        ev
    }

    fn near(g: &Game) -> V2 {
        g.pos + v2(60.0, 0.0)
    }

    #[test]
    fn commander_builds_blueprint_in_range_and_pays_metal() {
        let mut g = Game::new(1);
        let at = near(&g);
        assert_eq!(g.place(Kind::Wall, at), Event::Placed);
        let metal = g.metal;
        let ev = step(&mut g, 2.0, Input::default());
        assert!(g.buildings[0].done());
        assert!(ev.iter().any(|e| matches!(e, Event::Completed { .. })));
        // Paid the cost minus income earned in the meantime.
        assert!(g.metal < metal + 2.0 * BASE_INCOME);
        assert!((g.buildings[0].hp - g.buildings[0].max_hp).abs() < 1.0);
    }

    #[test]
    fn out_of_range_blueprint_does_not_progress() {
        let mut g = Game::new(1);
        let at = g.pos + v2(500.0, 0.0);
        g.place(Kind::Laser, at);
        step(&mut g, 3.0, Input::default());
        assert_eq!(g.buildings[0].progress, 0.0);
    }

    #[test]
    fn construction_stalls_without_metal() {
        let mut g = Game::new(1);
        g.place(Kind::Heavy, near(&g));
        g.metal = 0.0;
        g.income_mult = 0.0;
        step(&mut g, 2.0, Input::default());
        assert_eq!(g.buildings[0].progress, 0.0);
        assert!(g.metal >= 0.0);
    }

    #[test]
    fn nano_turret_adds_buildpower() {
        let mut g = Game::new(1);
        g.metal = 1000.0;
        let solo = {
            let mut h = Game::new(1);
            h.metal = 1000.0;
            h.place(Kind::Heavy, near(&h));
            step(&mut h, 1.0, Input::default());
            h.buildings[0].progress
        };
        g.place(Kind::Nano, g.pos + v2(-60.0, 0.0));
        g.buildings[0].progress = 1.0;
        g.place(Kind::Heavy, near(&g));
        assert!(g.total_bp() > g.bp);
        step(&mut g, 1.0, Input::default());
        assert!(g.buildings[1].progress > solo * 1.5);
    }

    #[test]
    fn placement_rules() {
        let mut g = Game::new(1);
        assert!(!g.can_place(Kind::Wall, g.pos));
        assert!(!g.can_place(Kind::Wall, v2(-5.0, 10.0)));
        let at = near(&g);
        g.place(Kind::Wall, at);
        assert!(!g.can_place(Kind::Wall, at + v2(5.0, 0.0)));
        g.metal = 0.0;
        assert_eq!(g.place(Kind::Wall, at + v2(0.0, 100.0)), Event::Denied);
    }

    #[test]
    fn repair_mode_heals_damaged_building() {
        let mut g = Game::new(1);
        g.place(Kind::Wall, near(&g));
        g.buildings[0].progress = 1.0;
        g.buildings[0].hp = 100.0;
        let input = Input {
            repair: true,
            ..Input::default()
        };
        step(&mut g, 1.0, input);
        assert!(g.buildings[0].hp > 150.0);
    }

    #[test]
    fn tower_kills_enemy_and_drops_metal_and_xp() {
        let mut g = Game::new(1);
        g.place(Kind::Laser, near(&g));
        g.buildings[0].progress = 1.0;
        g.enemies.push(Enemy {
            kind: EKind::Grunt,
            pos: g.buildings[0].pos + v2(150.0, 0.0),
            hp: 20.0,
            max_hp: 20.0,
            biting: false,
            flash: 0.0,
        });
        let ev = step(&mut g, 1.5, Input::default());
        assert!(ev.iter().any(|e| matches!(e, Event::Shot { .. })));
        assert!(ev.iter().any(|e| matches!(e, Event::EnemyKilled { .. })));
        assert_eq!(g.kills, 1);
        assert!(g.xp > 0.0);
    }

    #[test]
    fn enemies_chew_walls_and_destroy_them() {
        let mut g = Game::new(1);
        let at = g.pos + v2(600.0, 0.0);
        g.place(Kind::Wall, at);
        g.buildings[0].progress = 1.0;
        g.buildings[0].hp = 5.0;
        g.enemies.push(Enemy {
            kind: EKind::Brute,
            pos: at + v2(30.0, 0.0),
            hp: 100.0,
            max_hp: 100.0,
            biting: false,
            flash: 0.0,
        });
        let ev = step(&mut g, 1.0, Input::default());
        assert!(ev.iter().any(|e| matches!(e, Event::BuildingLost { .. })));
        assert!(g.buildings.is_empty());
    }

    #[test]
    fn level_up_pauses_and_choice_applies() {
        let mut g = Game::new(3);
        let mut ev = Vec::new();
        g.gain_xp(g.xp_needed(), &mut ev);
        assert_eq!(g.level, 2);
        assert_eq!(g.choices.len(), 3);
        let t = g.time;
        g.update(0.5, Input::default(), &mut ev);
        assert_eq!(g.time, t, "paused while choosing");
        let bp = g.bp;
        g.choices[0] = Upgrade::BuildPower;
        assert!(g.choose(0));
        assert!(g.bp > bp);
        assert!(g.choices.is_empty());
    }

    #[test]
    fn commander_dies_when_swarmed() {
        let mut g = Game::new(1);
        g.hp = 5.0;
        g.enemies.push(Enemy {
            kind: EKind::Sumo,
            pos: g.pos + v2(10.0, 0.0),
            hp: 400.0,
            max_hp: 400.0,
            biting: false,
            flash: 0.0,
        });
        let ev = step(&mut g, 1.0, Input::default());
        assert_eq!(g.outcome, Outcome::Lost);
        assert!(ev.contains(&Event::GameOver));
    }

    #[test]
    fn waves_escalate_and_boss_wave_wins_when_cleared() {
        let mut g = Game::new(9);
        g.hp = f32::MAX;
        g.max_hp = f32::MAX;
        let ev = step(&mut g, 6.0 + WAVE_SECS * 2.0, Input::default());
        assert!(ev.contains(&Event::WaveStart(1)));
        assert!(g.wave >= 2);
        assert!(!g.enemies.is_empty());
        g.wave = WAVES - 1;
        g.wave_time = 0.01;
        let mut ev = Vec::new();
        g.update(0.02, Input::default(), &mut ev);
        assert!(g.enemies.iter().any(|e| e.kind == EKind::Boss));
        g.enemies.clear();
        g.update(0.01, Input::default(), &mut ev);
        assert_eq!(g.outcome, Outcome::Won);
        assert!(ev.contains(&Event::Victory));
    }

    #[test]
    fn pickups_are_magnetized_and_collected() {
        let mut g = Game::new(1);
        g.pickups.push(Pickup {
            pos: g.pos + v2(100.0, 0.0),
            metal: 10.0,
        });
        let m = g.metal;
        let ev = step(&mut g, 1.0, Input::default());
        assert!(g.pickups.is_empty());
        assert!(g.metal >= m + 10.0);
        assert!(ev.contains(&Event::Pickup));
    }
}
