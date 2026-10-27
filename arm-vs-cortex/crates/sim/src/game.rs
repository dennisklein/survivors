//! Game rules: three-way war between the player's commander, the opposing
//! faction and scavengers, with capturable enemy factories.

use crate::math::{V2, v2};
use crate::rng::Rng;

pub const WORLD: f32 = 2400.0;
pub const PLAYER_RADIUS: f32 = 16.0;
pub const CAPTURE_RANGE: f32 = 110.0;
pub const PICKUP_RANGE: f32 = 20.0;
pub const WAVE_TIME: f32 = 25.0;
pub const ENEMY_FACTORIES: usize = 4;
const MAX_UNITS: usize = 240;
const UNIT_SHOT_SPEED: f32 = 320.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Arm,
    Cortex,
}

impl Faction {
    pub fn other(self) -> Faction {
        match self {
            Faction::Arm => Faction::Cortex,
            Faction::Cortex => Faction::Arm,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Faction::Arm => "Armada",
            Faction::Cortex => "Cortex",
        }
    }
}

/// Who an entity fights for. Captured factories and their units are `Player`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
    Scav,
}

pub fn hostile(a: Team, b: Team) -> bool {
    a != b
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotStyle {
    Laser,
    Plasma,
    Bolt,
}

/// A weapon profile.
#[derive(Clone, Copy, Debug)]
pub struct Weapon {
    pub reload: f32,
    pub dmg: f32,
    pub speed: f32,
    pub range: f32,
    pub splash: f32,
    pub style: ShotStyle,
}

/// The commander's main gun for a faction: Arm fires fast light lasers,
/// Cortex lobs slow plasma with splash damage.
pub fn commander_weapon(f: Faction) -> Weapon {
    match f {
        Faction::Arm => Weapon {
            reload: 0.16,
            dmg: 7.0,
            speed: 760.0,
            range: 370.0,
            splash: 0.0,
            style: ShotStyle::Laser,
        },
        Faction::Cortex => Weapon {
            reload: 0.5,
            dmg: 19.0,
            speed: 470.0,
            range: 340.0,
            splash: 55.0,
            style: ShotStyle::Plasma,
        },
    }
}

/// Defensive tower each faction can build: Arm light laser, Cortex heavy cannon.
pub fn turret_weapon(f: Faction) -> Weapon {
    match f {
        Faction::Arm => Weapon {
            reload: 0.45,
            dmg: 10.0,
            speed: 800.0,
            range: 300.0,
            splash: 0.0,
            style: ShotStyle::Laser,
        },
        Faction::Cortex => Weapon {
            reload: 1.1,
            dmg: 28.0,
            speed: 500.0,
            range: 320.0,
            splash: 35.0,
            style: ShotStyle::Plasma,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Raider,
    Tank,
    Artillery,
    Boss,
}

#[derive(Clone, Copy, Debug)]
pub struct UnitStats {
    pub hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub range: f32,
    pub reload: f32,
    pub radius: f32,
    pub splash: f32,
}

/// Unit stats by kind, faction (`None` = scavenger) and wave number.
pub fn unit_stats(kind: UnitKind, faction: Option<Faction>, wave: u32) -> UnitStats {
    let mut s = match kind {
        UnitKind::Raider => UnitStats {
            hp: 28.0,
            speed: 95.0,
            dmg: 5.0,
            range: 150.0,
            reload: 0.9,
            radius: 10.0,
            splash: 0.0,
        },
        UnitKind::Tank => UnitStats {
            hp: 85.0,
            speed: 58.0,
            dmg: 11.0,
            range: 200.0,
            reload: 1.4,
            radius: 14.0,
            splash: 0.0,
        },
        UnitKind::Artillery => UnitStats {
            hp: 45.0,
            speed: 48.0,
            dmg: 14.0,
            range: 320.0,
            reload: 2.6,
            radius: 12.0,
            splash: 40.0,
        },
        UnitKind::Boss => UnitStats {
            hp: 900.0,
            speed: 42.0,
            dmg: 22.0,
            range: 270.0,
            reload: 0.9,
            radius: 24.0,
            splash: 30.0,
        },
    };
    match faction {
        Some(Faction::Arm) => {
            s.speed *= 1.15;
            s.hp *= 0.9;
        }
        Some(Faction::Cortex) => {
            s.speed *= 0.9;
            s.hp *= 1.2;
        }
        None => {
            s.hp *= 0.8;
            s.speed *= 1.05;
        }
    }
    let k = 1.0 + 0.13 * wave.saturating_sub(1) as f32;
    s.hp *= k;
    s.dmg *= 1.0 + 0.05 * wave.saturating_sub(1) as f32;
    s
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub team: Team,
    pub faction: Option<Faction>,
    pub kind: UnitKind,
    pub pos: V2,
    pub facing: f32,
    pub hp: f32,
    pub stats: UnitStats,
    pub cooldown: f32,
    pub think: f32,
    pub aim: Option<V2>,
    pub flash: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    Factory,
    Turret,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildingKind,
    /// Faction whose tech this building uses.
    pub faction: Faction,
    pub team: Team,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub timer: f32,
    /// Capture progress, `0..1`.
    pub capture: f32,
    /// The player's starting factory: destroyed rather than reverted.
    pub home: bool,
    pub flash: f32,
}

impl Building {
    pub fn radius(&self) -> f32 {
        match self.kind {
            BuildingKind::Factory => 40.0,
            BuildingKind::Turret => 16.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub team: Team,
    pub dmg: f32,
    pub splash: f32,
    pub ttl: f32,
    pub style: ShotStyle,
    pub faction: Option<Faction>,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub pos: V2,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Multishot,
    Speed,
    Armor,
    Magnet,
    Capture,
    Turrets,
    Regen,
    Allies,
}

pub const ALL_UPGRADES: [Upgrade; 10] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::Multishot,
    Upgrade::Speed,
    Upgrade::Armor,
    Upgrade::Magnet,
    Upgrade::Capture,
    Upgrade::Turrets,
    Upgrade::Regen,
    Upgrade::Allies,
];

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Overcharged Rounds",
            Upgrade::FireRate => "Rapid Loader",
            Upgrade::Multishot => "Twin Barrels",
            Upgrade::Speed => "Servo Legs",
            Upgrade::Armor => "Armor Plating",
            Upgrade::Magnet => "Reclaim Field",
            Upgrade::Capture => "Nano Hacker",
            Upgrade::Turrets => "Tower Tech",
            Upgrade::Regen => "Field Repair",
            Upgrade::Allies => "Veteran Crews",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+25% weapon damage",
            Upgrade::FireRate => "+20% fire rate",
            Upgrade::Multishot => "+1 projectile per volley",
            Upgrade::Speed => "+12% move speed",
            Upgrade::Armor => "+30 max HP, full repair",
            Upgrade::Magnet => "+60% metal pickup radius",
            Upgrade::Capture => "Capture factories 40% faster",
            Upgrade::Turrets => "Towers +35% damage, cheaper",
            Upgrade::Regen => "+1.5 HP/s regeneration",
            Upgrade::Allies => "Allied units +30% damage, +HP",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub faction: Faction,
    pub pos: V2,
    pub facing: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub metal: u32,
    pub xp: u32,
    pub level: u32,
    pub dmg_mult: f32,
    pub rate_mult: f32,
    pub multishot: u32,
    pub magnet: f32,
    pub capture_mult: f32,
    pub turret_mult: f32,
    pub regen: f32,
    pub ally_mult: f32,
    pub cooldown: f32,
    pub tech_cooldown: f32,
    pub hurt: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub dir: V2,
    pub capture: bool,
    pub build: bool,
    pub choice: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp([Upgrade; 3]),
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot { player: bool, style: ShotStyle },
    Hit { pos: V2 },
    PlayerHurt,
    Kill { pos: V2, big: bool },
    Impact { pos: V2, splash: f32 },
    Pickup,
    LevelUp,
    Built,
    Denied,
    Capturing,
    Captured,
    FactoryLost,
    Wave(u32),
    BossSpawn,
    Won,
    Lost,
}

pub struct Game {
    pub rng: Rng,
    pub phase: Phase,
    pub time: f32,
    pub wave: u32,
    pub wave_timer: f32,
    pub player: Player,
    pub units: Vec<Unit>,
    pub buildings: Vec<Building>,
    pub shots: Vec<Shot>,
    pub pickups: Vec<Pickup>,
    pub events: Vec<Event>,
    pub kills: u32,
    capture_sfx: f32,
}

pub fn xp_needed(level: u32) -> u32 {
    10 + level * 7 + level * level
}

/// Metal cost of the next tower, given how many the player already owns.
pub fn turret_cost(owned: usize, turret_mult: f32) -> u32 {
    let base = 30.0 + 15.0 * owned as f32;
    (base / turret_mult.sqrt()).round() as u32
}

impl Game {
    pub fn new(faction: Faction, seed: u64) -> Game {
        let c = v2(WORLD / 2.0, WORLD / 2.0);
        let enemy = faction.other();
        let mut buildings = vec![Building {
            kind: BuildingKind::Factory,
            faction,
            team: Team::Player,
            pos: c + v2(0.0, -90.0),
            hp: 600.0,
            max_hp: 600.0,
            timer: 4.0,
            capture: 0.0,
            home: true,
            flash: 0.0,
        }];
        for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            buildings.push(Building {
                kind: BuildingKind::Factory,
                faction: enemy,
                team: Team::Enemy,
                pos: c + v2(dx * 820.0, dy * 820.0),
                hp: 800.0,
                max_hp: 800.0,
                timer: 6.0,
                capture: 0.0,
                home: false,
                flash: 0.0,
            });
        }
        Game {
            rng: Rng::new(seed),
            phase: Phase::Playing,
            time: 0.0,
            wave: 0,
            wave_timer: 6.0,
            player: Player {
                faction,
                pos: c,
                facing: 0.0,
                hp: 120.0,
                max_hp: 120.0,
                speed: 175.0,
                metal: 40,
                xp: 0,
                level: 1,
                dmg_mult: 1.0,
                rate_mult: 1.0,
                multishot: 0,
                magnet: 90.0,
                capture_mult: 1.0,
                turret_mult: 1.0,
                regen: 0.5,
                ally_mult: 1.0,
                cooldown: 0.0,
                tech_cooldown: 0.0,
                hurt: 0.0,
            },
            units: Vec::new(),
            buildings,
            shots: Vec::new(),
            pickups: Vec::new(),
            events: Vec::new(),
            kills: 0,
            capture_sfx: 0.0,
        }
    }

    /// Enemy-faction factories currently held by the player.
    pub fn captured(&self) -> usize {
        self.buildings
            .iter()
            .filter(|b| {
                b.kind == BuildingKind::Factory
                    && b.team == Team::Player
                    && b.faction != self.player.faction
            })
            .count()
    }

    pub fn turrets_owned(&self) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Turret && b.team == Team::Player)
            .count()
    }

    pub fn next_turret_cost(&self) -> u32 {
        turret_cost(self.turrets_owned(), self.player.turret_mult)
    }

    /// The building the commander would capture right now, if any.
    pub fn capture_target(&self) -> Option<usize> {
        self.buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.team != Team::Player && b.kind == BuildingKind::Factory)
            .map(|(i, b)| (i, b.pos.dist(self.player.pos) - b.radius()))
            .filter(|&(_, d)| d < CAPTURE_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    pub fn step(&mut self, dt: f32, input: &Input) {
        self.events.clear();
        match self.phase.clone() {
            Phase::Playing => {}
            Phase::LevelUp(choices) => {
                if let Some(c) = input.choice.filter(|&c| c < 3) {
                    self.apply_upgrade(choices[c]);
                    self.phase = Phase::Playing;
                }
                return;
            }
            Phase::Won | Phase::Lost => return,
        }
        self.time += dt;
        self.update_waves(dt);
        self.update_player(dt, input);
        self.update_buildings(dt);
        self.update_units(dt);
        self.update_shots(dt);
        self.reap();
        self.update_pickups(dt);
        self.check_end();
    }

    fn update_waves(&mut self, dt: f32) {
        self.wave_timer -= dt;
        if self.wave_timer > 0.0 {
            return;
        }
        self.wave += 1;
        self.wave_timer = WAVE_TIME;
        let w = self.wave;
        self.events.push(Event::Wave(w));
        let enemy = self.player.faction.other();
        let scavs = 3 + 2 * w as usize;
        let enemies = 2 + w as usize;
        for _ in 0..scavs {
            let p = self.edge_point();
            let k = self.random_kind();
            self.spawn(Team::Scav, None, k, p);
        }
        for _ in 0..enemies {
            let p = self.edge_point();
            let k = self.random_kind();
            self.spawn(Team::Enemy, Some(enemy), k, p);
        }
        if w.is_multiple_of(4) {
            let p = self.edge_point();
            self.spawn(Team::Enemy, Some(enemy), UnitKind::Boss, p);
            if w >= 8 {
                let p = self.edge_point();
                self.spawn(Team::Scav, None, UnitKind::Boss, p);
            }
            self.events.push(Event::BossSpawn);
        }
    }

    fn edge_point(&mut self) -> V2 {
        let t = self.rng.range(0.0, WORLD);
        match self.rng.below(4) {
            0 => v2(t, 20.0),
            1 => v2(t, WORLD - 20.0),
            2 => v2(20.0, t),
            _ => v2(WORLD - 20.0, t),
        }
    }

    fn random_kind(&mut self) -> UnitKind {
        let r = self.rng.f32();
        if r < 0.5 {
            UnitKind::Raider
        } else if r < 0.8 {
            UnitKind::Tank
        } else {
            UnitKind::Artillery
        }
    }

    pub fn spawn(&mut self, team: Team, faction: Option<Faction>, kind: UnitKind, pos: V2) {
        if self.units.len() >= MAX_UNITS && kind != UnitKind::Boss {
            return;
        }
        let mut stats = unit_stats(kind, faction, self.wave.max(1));
        if team == Team::Player {
            stats.dmg *= self.player.ally_mult;
            stats.hp *= 1.0 + (self.player.ally_mult - 1.0) * 0.5;
        }
        self.units.push(Unit {
            team,
            faction,
            kind,
            pos,
            facing: 0.0,
            hp: stats.hp,
            stats,
            cooldown: self.rng.range(0.0, stats.reload),
            think: 0.0,
            aim: None,
            flash: 0.0,
        });
    }

    /// Nearest entity hostile to `team` within `range` of `pos`.
    pub fn nearest_hostile(&self, pos: V2, team: Team, range: f32) -> Option<V2> {
        let mut best: Option<(f32, V2)> = None;
        let mut consider = |p: V2, r: f32| {
            let d = p.dist(pos) - r;
            if d <= range && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
        };
        if hostile(team, Team::Player) {
            consider(self.player.pos, PLAYER_RADIUS);
        }
        for u in &self.units {
            if hostile(team, u.team) {
                consider(u.pos, u.stats.radius);
            }
        }
        for b in &self.buildings {
            if hostile(team, b.team) {
                consider(b.pos, b.radius());
            }
        }
        best.map(|(_, p)| p)
    }

    fn fire(&mut self, from: V2, at: V2, w: Weapon, team: Team, faction: Option<Faction>) {
        let dir = (at - from).norm();
        self.shots.push(Shot {
            pos: from + dir * 14.0,
            vel: dir * w.speed,
            team,
            dmg: w.dmg,
            splash: w.splash,
            ttl: w.range / w.speed * 1.25,
            style: w.style,
            faction,
        });
    }

    fn update_player(&mut self, dt: f32, input: &Input) {
        let p = &mut self.player;
        let dir = input.dir.norm();
        p.pos += dir * p.speed * dt;
        p.pos.x = p.pos.x.clamp(20.0, WORLD - 20.0);
        p.pos.y = p.pos.y.clamp(20.0, WORLD - 20.0);
        p.hp = (p.hp + p.regen * dt).min(p.max_hp);
        p.hurt = (p.hurt - dt).max(0.0);
        p.cooldown -= dt;
        p.tech_cooldown -= dt;
        let faction = p.faction;
        let pos = p.pos;

        let main = commander_weapon(faction);
        if self.player.cooldown <= 0.0
            && let Some(t) = self.nearest_hostile(pos, Team::Player, main.range)
        {
            let mut w = main;
            w.dmg *= self.player.dmg_mult;
            let n = 1 + self.player.multishot;
            let base = (t - pos).y.atan2((t - pos).x);
            self.player.facing = base;
            for i in 0..n {
                let off = (i as f32 - (n - 1) as f32 / 2.0) * 0.14;
                let aim = pos + V2::from_angle(base + off) * 100.0;
                self.fire(pos, aim, w, Team::Player, Some(faction));
            }
            self.player.cooldown = main.reload / self.player.rate_mult;
            self.events.push(Event::Shot {
                player: true,
                style: main.style,
            });
        }

        // Captured enemy tech: a secondary weapon of the other faction that
        // grows stronger with every enemy factory held.
        let tech = self.captured();
        if tech > 0 && self.player.tech_cooldown <= 0.0 {
            let mut w = commander_weapon(faction.other());
            if let Some(t) = self.nearest_hostile(pos, Team::Player, w.range) {
                w.dmg *= self.player.dmg_mult * (0.6 + 0.2 * tech as f32);
                self.fire(pos, t, w, Team::Player, Some(faction.other()));
                self.player.tech_cooldown =
                    w.reload * 1.5 / (self.player.rate_mult * (1.0 + 0.25 * (tech - 1) as f32));
                self.events.push(Event::Shot {
                    player: true,
                    style: w.style,
                });
            }
        }

        if input.build {
            let cost = self.next_turret_cost();
            let blocked = self
                .buildings
                .iter()
                .any(|b| b.pos.dist(pos) < b.radius() + 30.0);
            if self.player.metal >= cost && !blocked {
                self.player.metal -= cost;
                self.buildings.push(Building {
                    kind: BuildingKind::Turret,
                    faction,
                    team: Team::Player,
                    pos,
                    hp: 220.0,
                    max_hp: 220.0,
                    timer: 0.0,
                    capture: 0.0,
                    home: false,
                    flash: 0.0,
                });
                self.events.push(Event::Built);
            } else {
                self.events.push(Event::Denied);
            }
        }

        self.capture_sfx -= dt;
        let target = if input.capture {
            self.capture_target()
        } else {
            None
        };
        for (i, b) in self.buildings.iter_mut().enumerate() {
            if Some(i) == target {
                let damaged = if b.hp < b.max_hp * 0.5 { 2.0 } else { 1.0 };
                b.capture += 0.16 * self.player.capture_mult * damaged * dt;
                if self.capture_sfx <= 0.0 {
                    self.capture_sfx = 0.35;
                    self.events.push(Event::Capturing);
                }
                if b.capture >= 1.0 {
                    b.capture = 0.0;
                    b.team = Team::Player;
                    b.hp = b.max_hp;
                    b.timer = 2.0;
                    self.events.push(Event::Captured);
                }
            } else {
                b.capture = (b.capture - 0.05 * dt).max(0.0);
            }
        }
    }

    fn update_buildings(&mut self, dt: f32) {
        let allies = self.units.iter().filter(|u| u.team == Team::Player).count();
        let ally_cap = 8 + 4 * self.captured();
        for i in 0..self.buildings.len() {
            let b = &mut self.buildings[i];
            b.flash = (b.flash - dt).max(0.0);
            b.timer -= dt;
            if b.timer > 0.0 {
                continue;
            }
            let (kind, team, faction, pos) = (b.kind, b.team, b.faction, b.pos);
            match kind {
                BuildingKind::Factory => {
                    let interval = match team {
                        Team::Player => 7.0,
                        _ => (10.0 - 0.35 * self.wave as f32).max(4.0),
                    };
                    self.buildings[i].timer = interval;
                    if team == Team::Player && allies >= ally_cap {
                        continue;
                    }
                    let k = self.random_kind();
                    let off = V2::from_angle(self.rng.range(0.0, std::f32::consts::TAU)) * 55.0;
                    self.spawn(team, Some(faction), k, pos + off);
                }
                BuildingKind::Turret => {
                    let mut w = turret_weapon(faction);
                    if let Some(t) = self.nearest_hostile(pos, team, w.range) {
                        w.dmg *= self.player.turret_mult;
                        self.fire(pos, t, w, team, Some(faction));
                        self.buildings[i].timer = w.reload;
                    }
                }
            }
        }
    }

    fn update_units(&mut self, dt: f32) {
        let ppos = self.player.pos;
        for i in 0..self.units.len() {
            let u = &mut self.units[i];
            u.flash = (u.flash - dt).max(0.0);
            u.cooldown -= dt;
            u.think -= dt;
            if u.think <= 0.0 {
                u.think = 0.25;
                let (pos, team) = (u.pos, u.team);
                let aim = self.nearest_hostile(pos, team, 520.0);
                self.units[i].aim = aim;
            }
            let u = &self.units[i];
            let (pos, team, st) = (u.pos, u.team, u.stats);
            let goal = match u.aim {
                Some(t) if t.dist(pos) > st.range * 0.8 => Some(t),
                Some(_) => None,
                None if team == Team::Player => {
                    if ppos.dist(pos) > 120.0 {
                        Some(ppos)
                    } else {
                        None
                    }
                }
                None => Some(ppos),
            };
            let u = &mut self.units[i];
            if let Some(g) = goal {
                let d = (g - pos).norm();
                u.pos += d * st.speed * dt;
                u.facing = d.y.atan2(d.x);
            }
            if let Some(t) = u.aim
                && u.cooldown <= 0.0
                && t.dist(pos) <= st.range + 20.0
            {
                u.cooldown = st.reload;
                let d = t - pos;
                u.facing = d.y.atan2(d.x);
                let f = u.faction;
                let w = Weapon {
                    reload: st.reload,
                    dmg: st.dmg,
                    speed: UNIT_SHOT_SPEED,
                    range: st.range + 40.0,
                    splash: st.splash,
                    style: ShotStyle::Bolt,
                };
                self.fire(pos, t, w, team, f);
                if team != Team::Player && t.dist(ppos) < 40.0 {
                    self.events.push(Event::Shot {
                        player: false,
                        style: ShotStyle::Bolt,
                    });
                }
            }
        }
        // Cheap separation so crowds spread out.
        let n = self.units.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.units[i].pos, self.units[j].pos);
                let min = self.units[i].stats.radius + self.units[j].stats.radius;
                let d2 = a.dist2(b);
                if d2 < min * min && d2 > 1e-4 {
                    let d = d2.sqrt();
                    let push = (b - a) * ((min - d) / d * 0.5);
                    self.units[i].pos -= push;
                    self.units[j].pos += push;
                }
            }
        }
        for u in &mut self.units {
            u.pos.x = u.pos.x.clamp(5.0, WORLD - 5.0);
            u.pos.y = u.pos.y.clamp(5.0, WORLD - 5.0);
        }
    }

    /// Applies `dmg` to everything hostile to `team` around `pos` (splash) or
    /// to the single entity hit.
    fn damage_area(&mut self, pos: V2, team: Team, dmg: f32, splash: f32) {
        if hostile(team, Team::Player) && self.player.pos.dist(pos) <= splash + PLAYER_RADIUS {
            self.hurt_player(dmg);
        }
        for u in &mut self.units {
            if hostile(team, u.team) && u.pos.dist(pos) <= splash + u.stats.radius {
                u.hp -= dmg;
                u.flash = 0.1;
            }
        }
        for b in &mut self.buildings {
            if hostile(team, b.team) && b.pos.dist(pos) <= splash + b.radius() {
                b.hp -= dmg;
                b.flash = 0.1;
            }
        }
    }

    fn hurt_player(&mut self, dmg: f32) {
        self.player.hp -= dmg;
        if self.player.hurt <= 0.0 {
            self.events.push(Event::PlayerHurt);
        }
        self.player.hurt = 0.15;
    }

    fn update_shots(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.shots.len() {
            let s = &mut self.shots[i];
            s.pos += s.vel * dt;
            s.ttl -= dt;
            let s = self.shots[i].clone();
            let hit = (hostile(s.team, Team::Player)
                && s.pos.dist(self.player.pos) < PLAYER_RADIUS)
                || self
                    .units
                    .iter()
                    .any(|u| hostile(s.team, u.team) && s.pos.dist(u.pos) < u.stats.radius)
                || self
                    .buildings
                    .iter()
                    .any(|b| hostile(s.team, b.team) && s.pos.dist(b.pos) < b.radius());
            let expired = s.ttl <= 0.0;
            if hit || (expired && s.splash > 0.0) {
                self.damage_area(s.pos, s.team, s.dmg, s.splash.max(1.0));
                if s.splash > 0.0 {
                    self.events.push(Event::Impact {
                        pos: s.pos,
                        splash: s.splash,
                    });
                } else {
                    self.events.push(Event::Hit { pos: s.pos });
                }
            }
            if hit || expired {
                self.shots.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.units.len() {
            if self.units[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let u = self.units.swap_remove(i);
            let big = u.kind == UnitKind::Boss;
            self.events.push(Event::Kill { pos: u.pos, big });
            if u.team != Team::Player {
                self.kills += 1;
                let value = match u.kind {
                    UnitKind::Raider => 2,
                    UnitKind::Tank | UnitKind::Artillery => 4,
                    UnitKind::Boss => 40,
                };
                self.pickups.push(Pickup { pos: u.pos, value });
            }
        }
        let mut i = 0;
        while i < self.buildings.len() {
            let b = &mut self.buildings[i];
            if b.hp > 0.0 {
                i += 1;
                continue;
            }
            let pos = b.pos;
            match (b.kind, b.team) {
                // Enemy factories cannot be destroyed, only captured.
                (BuildingKind::Factory, Team::Enemy | Team::Scav) => {
                    b.hp = 1.0;
                    i += 1;
                }
                (BuildingKind::Factory, Team::Player) if !b.home => {
                    b.team = Team::Enemy;
                    b.hp = b.max_hp * 0.5;
                    b.timer = 5.0;
                    self.events.push(Event::FactoryLost);
                    self.events.push(Event::Kill { pos, big: true });
                    i += 1;
                }
                _ => {
                    self.buildings.swap_remove(i);
                    self.events.push(Event::Kill { pos, big: true });
                }
            }
        }
    }

    fn update_pickups(&mut self, dt: f32) {
        let pp = self.player.pos;
        let magnet = self.player.magnet;
        let mut gained = 0;
        self.pickups.retain_mut(|k| {
            let d = k.pos.dist(pp);
            if d < PICKUP_RANGE {
                gained += k.value;
                return false;
            }
            if d < magnet {
                k.pos += (pp - k.pos).norm() * 420.0 * dt;
            }
            true
        });
        if gained > 0 {
            self.events.push(Event::Pickup);
            self.player.metal += gained;
            self.player.xp += gained;
        }
        if self.player.xp >= xp_needed(self.player.level) {
            self.player.xp -= xp_needed(self.player.level);
            self.player.level += 1;
            let choices = self.roll_upgrades();
            self.phase = Phase::LevelUp(choices);
            self.events.push(Event::LevelUp);
        }
    }

    pub fn roll_upgrades(&mut self) -> [Upgrade; 3] {
        let mut pool = ALL_UPGRADES.to_vec();
        let mut out = [Upgrade::Damage; 3];
        for slot in &mut out {
            let k = self.rng.below(pool.len());
            *slot = pool.swap_remove(k);
        }
        out
    }

    pub fn apply_upgrade(&mut self, u: Upgrade) {
        let p = &mut self.player;
        match u {
            Upgrade::Damage => p.dmg_mult *= 1.25,
            Upgrade::FireRate => p.rate_mult *= 1.2,
            Upgrade::Multishot => p.multishot += 1,
            Upgrade::Speed => p.speed *= 1.12,
            Upgrade::Armor => {
                p.max_hp += 30.0;
                p.hp = p.max_hp;
            }
            Upgrade::Magnet => p.magnet *= 1.6,
            Upgrade::Capture => p.capture_mult *= 1.4,
            Upgrade::Turrets => p.turret_mult *= 1.35,
            Upgrade::Regen => p.regen += 1.5,
            Upgrade::Allies => p.ally_mult *= 1.3,
        }
    }

    fn check_end(&mut self) {
        if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        } else if self.captured() >= ENEMY_FACTORIES {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
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
    fn setup_has_home_and_four_enemy_factories() {
        let g = Game::new(Faction::Arm, 1);
        assert_eq!(g.buildings.len(), 5);
        let enemy: Vec<_> = g
            .buildings
            .iter()
            .filter(|b| b.team == Team::Enemy)
            .collect();
        assert_eq!(enemy.len(), ENEMY_FACTORIES);
        assert!(enemy.iter().all(|b| b.faction == Faction::Cortex));
        assert_eq!(g.captured(), 0);
    }

    #[test]
    fn factions_have_distinct_weapons() {
        let a = commander_weapon(Faction::Arm);
        let c = commander_weapon(Faction::Cortex);
        assert!(a.reload < c.reload);
        assert!(c.splash > 0.0 && a.splash == 0.0);
        assert_eq!(Faction::Arm.other(), Faction::Cortex);
    }

    #[test]
    fn waves_escalate() {
        let mut g = Game::new(Faction::Cortex, 2);
        g.wave_timer = 0.0;
        g.step(0.01, &idle());
        let first = g.units.len();
        assert_eq!(g.wave, 1);
        g.units.clear();
        g.wave_timer = 0.0;
        g.step(0.01, &idle());
        assert!(g.units.len() > first);
        assert!(unit_stats(UnitKind::Tank, None, 10).hp > unit_stats(UnitKind::Tank, None, 1).hp);
    }

    #[test]
    fn capture_flips_factory_and_grants_tech() {
        let mut g = Game::new(Faction::Arm, 3);
        g.wave_timer = 1e9;
        let target = g.buildings[1].pos;
        g.player.pos = target + v2(50.0, 0.0);
        g.buildings[1].timer = 1e9;
        let input = Input {
            capture: true,
            ..Input::default()
        };
        for _ in 0..1000 {
            g.player.hp = g.player.max_hp;
            g.step(0.02, &input);
            if g.buildings[1].team == Team::Player {
                break;
            }
        }
        assert_eq!(g.buildings[1].team, Team::Player);
        assert_eq!(g.captured(), 1);
        g.step(0.02, &idle());
        // Tech weapon fires the other faction's plasma once anything is near.
        g.spawn(
            Team::Scav,
            None,
            UnitKind::Tank,
            g.player.pos + v2(100.0, 0.0),
        );
        g.player.tech_cooldown = 0.0;
        g.step(0.02, &idle());
        assert!(g.shots.iter().any(|s| s.style == ShotStyle::Plasma));
    }

    #[test]
    fn capturing_all_factories_wins() {
        let mut g = Game::new(Faction::Arm, 4);
        for b in &mut g.buildings {
            b.team = Team::Player;
        }
        g.step(0.01, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn enemy_factories_cannot_be_destroyed() {
        let mut g = Game::new(Faction::Arm, 5);
        g.buildings[1].hp = -50.0;
        g.step(0.01, &idle());
        assert_eq!(g.buildings.len(), 5);
        assert!(g.buildings[1].hp > 0.0);
    }

    #[test]
    fn lost_factory_reverts_to_enemy() {
        let mut g = Game::new(Faction::Arm, 6);
        g.buildings[2].team = Team::Player;
        g.buildings[2].hp = -1.0;
        g.step(0.01, &idle());
        assert_eq!(g.buildings[2].team, Team::Enemy);
    }

    #[test]
    fn building_costs_metal_and_needs_enough() {
        let mut g = Game::new(Faction::Cortex, 7);
        g.player.pos = v2(300.0, 1200.0);
        g.player.metal = 0;
        let b = Input {
            build: true,
            ..Input::default()
        };
        g.step(0.01, &b);
        assert!(g.events.contains(&Event::Denied));
        g.player.metal = 1000;
        g.step(0.01, &b);
        assert_eq!(g.turrets_owned(), 1);
        assert!(g.player.metal < 1000);
        assert!(g.next_turret_cost() > turret_cost(0, 1.0));
    }

    #[test]
    fn kills_drop_metal_and_level_up_pauses_for_choice() {
        let mut g = Game::new(Faction::Arm, 8);
        g.wave_timer = 1e9;
        g.pickups.push(Pickup {
            pos: g.player.pos,
            value: 100,
        });
        g.step(0.01, &idle());
        assert!(matches!(g.phase, Phase::LevelUp(_)));
        let before = g.player.level;
        let t = g.time;
        g.step(0.5, &idle());
        assert_eq!(g.time, t, "paused while choosing");
        g.step(
            0.01,
            &Input {
                choice: Some(0),
                ..Input::default()
            },
        );
        assert_eq!(g.player.level, before);
        assert!(g.player.metal >= 100);
    }

    #[test]
    fn upgrade_rolls_are_distinct() {
        let mut g = Game::new(Faction::Arm, 9);
        for _ in 0..50 {
            let [a, b, c] = g.roll_upgrades();
            assert!(a != b && b != c && a != c);
        }
    }

    #[test]
    fn scavs_and_enemies_fight_each_other() {
        assert!(hostile(Team::Scav, Team::Enemy));
        assert!(hostile(Team::Enemy, Team::Player));
        assert!(!hostile(Team::Player, Team::Player));
        let mut g = Game::new(Faction::Arm, 10);
        g.spawn(
            Team::Enemy,
            Some(Faction::Cortex),
            UnitKind::Tank,
            v2(100.0, 100.0),
        );
        let t = g.nearest_hostile(v2(150.0, 100.0), Team::Scav, 200.0);
        assert_eq!(t, Some(v2(100.0, 100.0)));
    }

    #[test]
    fn player_dies_and_game_is_lost() {
        let mut g = Game::new(Faction::Cortex, 11);
        g.player.hp = 0.0;
        g.player.regen = 0.0;
        g.step(0.01, &idle());
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.events.contains(&Event::Lost));
    }

    #[test]
    fn long_simulation_stays_sane() {
        let mut g = Game::new(Faction::Arm, 12);
        let mut r = Rng::new(1);
        for _ in 0..6000 {
            let input = Input {
                dir: v2(r.range(-1.0, 1.0), r.range(-1.0, 1.0)),
                capture: r.chance(0.3),
                build: r.chance(0.01),
                choice: Some(r.below(3)),
            };
            g.player.hp = g.player.max_hp;
            g.step(1.0 / 60.0, &input);
        }
        assert!(g.wave >= 3);
        assert!(g.units.len() <= MAX_UNITS + 4);
        assert!(g.units.iter().all(|u| u.pos.x.is_finite()));
    }
}
