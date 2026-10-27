//! Pure game rules for Freight Siege. No rendering, input or audio here: the
//! front end feeds commands in and reads state plus [`Event`]s back out.

use std::collections::HashMap;

pub const GRID_W: i32 = 40;
pub const GRID_H: i32 = 22;
pub const TILE: f32 = 32.0;
pub const WORLD_W: f32 = GRID_W as f32 * TILE;
pub const WORLD_H: f32 = GRID_H as f32 * TILE;

pub const YEARS_TO_WIN: u32 = 10;
pub const YEAR_SECS: f32 = 60.0;
pub const STATION_COST: f32 = 120.0;
pub const TRACK_COST: f32 = 6.0;
pub const TRACK_HP: f32 = 40.0;
pub const STATION_HP: f32 = 300.0;
pub const MAX_WAGONS: usize = 8;
pub const START_CASH: f32 = 260.0;

pub type Tile = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn dist(self, o: V2) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        V2::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

pub fn tile_center(t: Tile) -> V2 {
    V2::new((t.0 as f32 + 0.5) * TILE, (t.1 as f32 + 0.5) * TILE)
}

pub fn world_to_tile(p: V2) -> Option<Tile> {
    let t = ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32);
    in_bounds(t).then_some(t)
}

pub fn in_bounds(t: Tile) -> bool {
    t.0 >= 0 && t.1 >= 0 && t.0 < GRID_W && t.1 < GRID_H
}

fn adjacent(a: Tile, b: Tile) -> bool {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() == 1
}

/// Small deterministic xorshift RNG so rules stay reproducible in tests.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cargo {
    /// Pays the most cash.
    Coal,
    /// Pays less, but each delivery overclocks the train's turrets.
    Goods,
    /// Pays least, but each delivery grants experience.
    Mail,
}

impl Cargo {
    pub fn next(self) -> Self {
        match self {
            Cargo::Coal => Cargo::Goods,
            Cargo::Goods => Cargo::Mail,
            Cargo::Mail => Cargo::Coal,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Cargo::Coal => "Coal",
            Cargo::Goods => "Goods",
            Cargo::Mail => "Mail",
        }
    }
    fn pay_mult(self) -> f32 {
        match self {
            Cargo::Coal => 1.5,
            Cargo::Goods => 1.0,
            Cargo::Mail => 0.7,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Station {
    pub tile: Tile,
    pub cargo: Cargo,
    pub hp: f32,
    pub earned: f32,
}

impl Station {
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
    pub fn pos(&self) -> V2 {
        tile_center(self.tile)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WagonKind {
    Gatling,
    Cannon,
    Tesla,
    Mortar,
}

pub const WAGON_KINDS: [WagonKind; 4] = [
    WagonKind::Gatling,
    WagonKind::Cannon,
    WagonKind::Tesla,
    WagonKind::Mortar,
];

pub struct WagonSpec {
    pub cost: f32,
    pub damage: f32,
    pub cooldown: f32,
    pub range: f32,
    pub splash: f32,
    pub chains: u32,
}

impl WagonKind {
    pub fn spec(self) -> WagonSpec {
        match self {
            WagonKind::Gatling => WagonSpec {
                cost: 60.0,
                damage: 7.0,
                cooldown: 0.22,
                range: 150.0,
                splash: 0.0,
                chains: 0,
            },
            WagonKind::Cannon => WagonSpec {
                cost: 110.0,
                damage: 26.0,
                cooldown: 1.1,
                range: 200.0,
                splash: 55.0,
                chains: 0,
            },
            WagonKind::Tesla => WagonSpec {
                cost: 140.0,
                damage: 12.0,
                cooldown: 0.7,
                range: 140.0,
                splash: 0.0,
                chains: 4,
            },
            WagonKind::Mortar => WagonSpec {
                cost: 190.0,
                damage: 45.0,
                cooldown: 2.4,
                range: 320.0,
                splash: 85.0,
                chains: 0,
            },
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            WagonKind::Gatling => "Gatling",
            WagonKind::Cannon => "Cannon",
            WagonKind::Tesla => "Tesla",
            WagonKind::Mortar => "Mortar",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Wagon {
    pub kind: WagonKind,
    pub cooldown: f32,
}

#[derive(Clone, Debug)]
pub struct Train {
    /// Fractional index along the line's path.
    pub pos: f32,
    /// +1 towards the path end, -1 towards the start.
    pub dir: f32,
    pub wait: f32,
    pub wagons: Vec<Wagon>,
    pub blocked: bool,
    /// Seconds of turret overclock left (from Goods deliveries).
    pub overclock: f32,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub a: usize,
    pub b: usize,
    pub path: Vec<Tile>,
    pub train: Train,
}

impl Line {
    /// World position at a fractional path index (clamped).
    pub fn point(&self, f: f32) -> V2 {
        let max = (self.path.len() - 1) as f32;
        let f = f.clamp(0.0, max);
        let i = f.floor() as usize;
        let j = (i + 1).min(self.path.len() - 1);
        tile_center(self.path[i]).lerp(tile_center(self.path[j]), f - i as f32)
    }
    /// Path index of wagon `k` (0 = locomotive).
    pub fn car_index(&self, k: usize) -> f32 {
        let max = (self.path.len() - 1) as f32;
        (self.train.pos - self.train.dir * 0.85 * k as f32).clamp(0.0, max)
    }
    pub fn car_pos(&self, k: usize) -> V2 {
        self.point(self.car_index(k))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Crawler,
    Runner,
    Brute,
    Behemoth,
}

impl EnemyKind {
    /// (hp, speed px/s, bite dps, radius, scrap value)
    pub fn stats(self) -> (f32, f32, f32, f32, u32) {
        match self {
            EnemyKind::Crawler => (22.0, 38.0, 6.0, 8.0, 1),
            EnemyKind::Runner => (13.0, 72.0, 4.0, 6.0, 1),
            EnemyKind::Brute => (110.0, 24.0, 12.0, 13.0, 4),
            EnemyKind::Behemoth => (1600.0, 17.0, 45.0, 26.0, 30),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub target: usize,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Scrap {
    pub pos: V2,
    pub value: u32,
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Tracer {
    pub from: V2,
    pub to: V2,
    pub kind: WagonKind,
    pub ttl: f32,
}

#[derive(Clone, Debug)]
pub struct Blast {
    pub pos: V2,
    pub radius: f32,
    pub ttl: f32,
}

#[derive(Clone, Debug)]
pub struct Floater {
    pub pos: V2,
    pub text: String,
    pub ttl: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Range,
    Speed,
    Income,
    Armor,
    Magnet,
    Repair,
}

pub const UPGRADES: [Upgrade; 8] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::Range,
    Upgrade::Speed,
    Upgrade::Income,
    Upgrade::Armor,
    Upgrade::Magnet,
    Upgrade::Repair,
];

impl Upgrade {
    pub fn title(self) -> &'static str {
        match self {
            Upgrade::Damage => "Hardened Shells",
            Upgrade::FireRate => "Greased Breeches",
            Upgrade::Range => "Signal Spotters",
            Upgrade::Speed => "Express Boilers",
            Upgrade::Income => "Freight Contracts",
            Upgrade::Armor => "Steel Sleepers",
            Upgrade::Magnet => "Scrap Magnets",
            Upgrade::Repair => "Field Crews",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+20% turret damage",
            Upgrade::FireRate => "+15% fire rate",
            Upgrade::Range => "+15% turret range",
            Upgrade::Speed => "+15% train speed",
            Upgrade::Income => "+20% delivery income",
            Upgrade::Armor => "Track & stations take 25% less damage",
            Upgrade::Magnet => "+40% scrap pickup radius",
            Upgrade::Repair => "Fully repair the network now",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub damage: f32,
    pub fire_rate: f32,
    pub range: f32,
    pub speed: f32,
    pub income: f32,
    pub armor: f32,
    pub magnet: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            damage: 1.0,
            fire_rate: 1.0,
            range: 1.0,
            speed: 1.0,
            income: 1.0,
            armor: 1.0,
            magnet: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp([Upgrade; 3]),
    Won,
    Lost(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shoot(WagonKind),
    Hit,
    Kill { big: bool },
    Pickup,
    LevelUp,
    Build,
    Delivered,
    StationLost,
    YearStart(u32),
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
    /// Track tile -> hp.
    pub track: HashMap<Tile, f32>,
    pub enemies: Vec<Enemy>,
    pub scraps: Vec<Scrap>,
    pub tracers: Vec<Tracer>,
    pub blasts: Vec<Blast>,
    pub floaters: Vec<Floater>,
    pub cash: f32,
    pub xp: u32,
    pub level: u32,
    pub year: u32,
    pub year_time: f32,
    /// Planning mode: the world is frozen and building is allowed.
    pub paused: bool,
    pub phase: Phase,
    pub stats: Stats,
    pub kills: u32,
    pub delivered: u32,
    pub selected_line: usize,
    pub events: Vec<Event>,
    spawn_acc: f32,
    rng: Rng,
}

impl Game {
    /// A fresh game with a starter network: two stations, one line, one gun.
    pub fn new(seed: u64) -> Self {
        let mut g = Self {
            stations: Vec::new(),
            lines: Vec::new(),
            track: HashMap::new(),
            enemies: Vec::new(),
            scraps: Vec::new(),
            tracers: Vec::new(),
            blasts: Vec::new(),
            floaters: Vec::new(),
            cash: 0.0,
            xp: 0,
            level: 1,
            year: 1,
            year_time: 0.0,
            paused: true,
            phase: Phase::Playing,
            stats: Stats::default(),
            kills: 0,
            delivered: 0,
            selected_line: 0,
            events: Vec::new(),
            spawn_acc: 0.0,
            rng: Rng::new(seed),
        };
        g.cash = 10_000.0;
        g.place_station((14, 11)).expect("starter station");
        g.place_station((25, 11)).expect("starter station");
        let path: Vec<Tile> = (14..=25).map(|x| (x, 11)).collect();
        g.build_line(&path).expect("starter line");
        g.lines[0].train.wagons.push(Wagon {
            kind: WagonKind::Gatling,
            cooldown: 0.0,
        });
        g.cash = START_CASH;
        g.events.clear();
        g
    }

    pub fn xp_needed(&self) -> u32 {
        5 + self.level * 5
    }

    pub fn station_at(&self, t: Tile) -> Option<usize> {
        self.stations.iter().position(|s| s.tile == t)
    }

    pub fn wagon_count(&self) -> usize {
        self.lines.iter().map(|l| l.train.wagons.len()).sum()
    }

    /// Cash charged at the end of the current year.
    pub fn upkeep(&self) -> f32 {
        self.track.len() as f32 * 1.0
            + self.stations.len() as f32 * 15.0
            + self.wagon_count() as f32 * 6.0
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Won | Phase::Lost(_))
    }

    fn can_build(&self) -> Result<(), &'static str> {
        if !self.paused {
            return Err("Pause (Space) to build");
        }
        if self.phase != Phase::Playing {
            return Err("Not now");
        }
        Ok(())
    }

    pub fn place_station(&mut self, t: Tile) -> Result<(), &'static str> {
        self.can_build()?;
        if !in_bounds(t) || t.0 == 0 || t.1 == 0 || t.0 == GRID_W - 1 || t.1 == GRID_H - 1 {
            return Err("Too close to the wilds");
        }
        if self.station_at(t).is_some() || self.track.contains_key(&t) {
            return Err("Tile occupied");
        }
        if self.cash < STATION_COST {
            return Err("Not enough cash");
        }
        self.cash -= STATION_COST;
        let cargo = [Cargo::Coal, Cargo::Goods, Cargo::Mail][self.stations.len() % 3];
        self.stations.push(Station {
            tile: t,
            cargo,
            hp: STATION_HP,
            earned: 0.0,
        });
        self.events.push(Event::Build);
        Ok(())
    }

    pub fn cycle_cargo(&mut self, i: usize) {
        if let Some(s) = self.stations.get_mut(i) {
            s.cargo = s.cargo.next();
            self.events.push(Event::Build);
        }
    }

    /// Cost of laying `path` (existing track tiles and stations are free).
    pub fn path_cost(&self, path: &[Tile]) -> f32 {
        path.iter()
            .filter(|t| !self.track.contains_key(t) && self.station_at(**t).is_none())
            .count() as f32
            * TRACK_COST
    }

    /// Build a new line along `path`, which must run station to station.
    pub fn build_line(&mut self, path: &[Tile]) -> Result<(), &'static str> {
        self.can_build()?;
        let (Some(&first), Some(&last)) = (path.first(), path.last()) else {
            return Err("Empty route");
        };
        let a = self.station_at(first).ok_or("Routes start at a station")?;
        let b = self
            .station_at(last)
            .ok_or("Routes must end at a station")?;
        if a == b {
            return Err("Route needs two different stations");
        }
        if !path.windows(2).all(|w| adjacent(w[0], w[1])) || !path.iter().all(|&t| in_bounds(t)) {
            return Err("Broken route");
        }
        if path[1..path.len() - 1]
            .iter()
            .any(|&t| self.station_at(t).is_some())
        {
            return Err("Route passes through a station");
        }
        let cost = self.path_cost(path);
        if self.cash < cost {
            return Err("Not enough cash");
        }
        self.cash -= cost;
        for &t in path {
            if self.station_at(t).is_none() {
                self.track.entry(t).or_insert(TRACK_HP);
            }
        }
        self.lines.push(Line {
            a,
            b,
            path: path.to_vec(),
            train: Train {
                pos: 0.0,
                dir: 1.0,
                wait: 0.5,
                wagons: Vec::new(),
                blocked: false,
                overclock: 0.0,
            },
        });
        self.selected_line = self.lines.len() - 1;
        self.events.push(Event::Build);
        Ok(())
    }

    /// Buy a wagon for the selected line (or the least-armed one).
    pub fn buy_wagon(&mut self, kind: WagonKind) -> Result<(), &'static str> {
        if self.is_over() {
            return Err("Game over");
        }
        if self.lines.is_empty() {
            return Err("Build a line first");
        }
        let cost = kind.spec().cost;
        if self.cash < cost {
            return Err("Not enough cash");
        }
        let mut idx = self.selected_line.min(self.lines.len() - 1);
        if self.lines[idx].train.wagons.len() >= MAX_WAGONS {
            idx = (0..self.lines.len())
                .min_by_key(|&i| self.lines[i].train.wagons.len())
                .unwrap_or(0);
            if self.lines[idx].train.wagons.len() >= MAX_WAGONS {
                return Err("All trains are full");
            }
        }
        self.cash -= cost;
        self.lines[idx].train.wagons.push(Wagon {
            kind,
            cooldown: 0.0,
        });
        self.events.push(Event::Build);
        Ok(())
    }

    pub fn repair_cost(&self) -> f32 {
        let track: f32 = self.track.values().map(|hp| TRACK_HP - hp).sum();
        let st: f32 = self.stations.iter().map(|s| STATION_HP - s.hp).sum();
        (track * 0.3 + st * 0.5).ceil()
    }

    pub fn repair_all(&mut self) -> Result<(), &'static str> {
        self.can_build()?;
        let cost = self.repair_cost();
        if cost <= 0.0 {
            return Err("Nothing to repair");
        }
        if self.cash < cost {
            return Err("Not enough cash");
        }
        self.cash -= cost;
        self.heal_everything();
        self.events.push(Event::Build);
        Ok(())
    }

    fn heal_everything(&mut self) {
        for hp in self.track.values_mut() {
            *hp = TRACK_HP;
        }
        for s in &mut self.stations {
            s.hp = STATION_HP;
        }
    }

    pub fn choose_upgrade(&mut self, i: usize) {
        let Phase::LevelUp(choices) = self.phase.clone() else {
            return;
        };
        let Some(&u) = choices.get(i) else {
            return;
        };
        match u {
            Upgrade::Damage => self.stats.damage += 0.2,
            Upgrade::FireRate => self.stats.fire_rate += 0.15,
            Upgrade::Range => self.stats.range += 0.15,
            Upgrade::Speed => self.stats.speed += 0.15,
            Upgrade::Income => self.stats.income += 0.2,
            Upgrade::Armor => self.stats.armor *= 0.75,
            Upgrade::Magnet => self.stats.magnet += 0.4,
            Upgrade::Repair => self.heal_everything(),
        }
        self.phase = Phase::Playing;
        self.check_level_up();
    }

    fn check_level_up(&mut self) {
        if self.phase == Phase::Playing && self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            let mut pool = UPGRADES.to_vec();
            let mut pick = [Upgrade::Damage; 3];
            for p in &mut pick {
                *p = pool.remove(self.rng.below(pool.len()));
            }
            self.phase = Phase::LevelUp(pick);
            self.events.push(Event::LevelUp);
        }
    }

    /// The station rust-beasts hunt: the most profitable one still standing.
    pub fn target_station(&self) -> Option<usize> {
        self.stations
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive())
            .max_by(|a, b| a.1.earned.total_cmp(&b.1.earned))
            .map(|(i, _)| i)
    }

    pub fn spawn_enemy(&mut self, kind: EnemyKind) {
        let Some(target) = self.target_station() else {
            return;
        };
        let pos = match self.rng.below(4) {
            0 => V2::new(self.rng.range(0.0, WORLD_W), -20.0),
            1 => V2::new(self.rng.range(0.0, WORLD_W), WORLD_H + 20.0),
            2 => V2::new(-20.0, self.rng.range(0.0, WORLD_H)),
            _ => V2::new(WORLD_W + 20.0, self.rng.range(0.0, WORLD_H)),
        };
        let scale = 1.0 + 0.22 * (self.year - 1) as f32;
        let hp = kind.stats().0 * scale;
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            target,
            flash: 0.0,
        });
    }

    fn start_year_wave(&mut self) {
        let n = 4 + self.year * 4;
        for _ in 0..n {
            self.spawn_enemy(EnemyKind::Crawler);
        }
        if self.year >= 3 {
            for _ in 0..self.year {
                self.spawn_enemy(EnemyKind::Brute);
            }
        }
        if self.year.is_multiple_of(5) || self.year == 8 {
            self.spawn_enemy(EnemyKind::Behemoth);
        }
        self.events.push(Event::YearStart(self.year));
    }

    /// Advance the simulation; does nothing while paused or not playing.
    pub fn update(&mut self, dt: f32) {
        self.age_effects(dt);
        if self.paused || self.phase != Phase::Playing {
            return;
        }
        if self.year_time == 0.0 {
            self.start_year_wave();
        }
        self.year_time += dt;
        self.spawn(dt);
        self.move_trains(dt);
        self.fire(dt);
        self.move_enemies(dt);
        self.collect_scrap(dt);
        if self.stations.iter().all(|s| !s.alive()) {
            self.lose("Every station fell to the rust");
            return;
        }
        if self.year_time >= YEAR_SECS {
            self.end_year();
        }
        self.check_level_up();
    }

    fn lose(&mut self, why: &'static str) {
        self.phase = Phase::Lost(why);
        self.events.push(Event::Lost);
    }

    fn end_year(&mut self) {
        self.cash -= self.upkeep();
        if self.cash < 0.0 {
            self.lose("Bankrupt: upkeep exceeded your cash");
            return;
        }
        if self.year >= YEARS_TO_WIN {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }
        self.year += 1;
        self.year_time = 0.0;
        // Rust-beasts regroup: the world pauses so the player can plan.
        self.paused = true;
    }

    fn age_effects(&mut self, dt: f32) {
        for t in &mut self.tracers {
            t.ttl -= dt;
        }
        self.tracers.retain(|t| t.ttl > 0.0);
        for b in &mut self.blasts {
            b.ttl -= dt;
        }
        self.blasts.retain(|b| b.ttl > 0.0);
        for f in &mut self.floaters {
            f.ttl -= dt;
            f.pos.y -= 20.0 * dt;
        }
        self.floaters.retain(|f| f.ttl > 0.0);
    }

    fn spawn(&mut self, dt: f32) {
        let y = (self.year - 1) as f32;
        let rate = 0.5 + 0.45 * y + 0.015 * self.year_time;
        self.spawn_acc += rate * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let r = self.rng.f32();
            let kind = if self.year >= 4 && r < 0.06 + 0.01 * y {
                EnemyKind::Brute
            } else if self.year >= 2 && r < 0.35 {
                EnemyKind::Runner
            } else {
                EnemyKind::Crawler
            };
            self.spawn_enemy(kind);
        }
    }

    fn move_trains(&mut self, dt: f32) {
        let speed = 3.2 * self.stats.speed;
        for li in 0..self.lines.len() {
            let line = &mut self.lines[li];
            let t = &mut line.train;
            t.overclock = (t.overclock - dt).max(0.0);
            if t.wait > 0.0 {
                t.wait -= dt;
                continue;
            }
            let max = (line.path.len() - 1) as f32;
            let next = (t.pos + t.dir * speed * dt).clamp(0.0, max);
            // Look one tile ahead for broken track.
            let ahead = (next + t.dir * 0.5).round().clamp(0.0, max) as usize;
            let tile = line.path[ahead];
            t.blocked = self.track.get(&tile).is_some_and(|hp| *hp <= 0.0);
            if t.blocked {
                continue;
            }
            t.pos = next;
            let arrived = if t.dir > 0.0 {
                t.pos >= max
            } else {
                t.pos <= 0.0
            };
            if arrived {
                t.dir = -t.dir;
                t.wait = 0.8;
                let (from, to) = if t.dir < 0.0 {
                    (line.a, line.b)
                } else {
                    (line.b, line.a)
                };
                let len = line.path.len() as f32;
                self.deliver(li, from, to, len);
            }
        }
    }

    fn deliver(&mut self, li: usize, from: usize, to: usize, len: f32) {
        if !self.stations[from].alive() || !self.stations[to].alive() {
            return;
        }
        let cargo = self.stations[from].cargo;
        let pay = ((8.0 + 1.3 * len) * cargo.pay_mult() * self.stats.income).round();
        self.cash += pay;
        self.stations[to].earned += pay;
        self.stations[from].earned += pay * 0.5;
        self.delivered += 1;
        match cargo {
            Cargo::Goods => self.lines[li].train.overclock = 12.0,
            Cargo::Mail => self.xp += 2,
            Cargo::Coal => {}
        }
        self.floaters.push(Floater {
            pos: self.stations[to].pos(),
            text: format!("+${pay}"),
            ttl: 1.2,
        });
        self.events.push(Event::Delivered);
    }

    fn fire(&mut self, dt: f32) {
        for li in 0..self.lines.len() {
            for wi in 0..self.lines[li].train.wagons.len() {
                let overclock = if self.lines[li].train.overclock > 0.0 {
                    1.5
                } else {
                    1.0
                };
                let fire_rate = self.stats.fire_rate;
                let w = &mut self.lines[li].train.wagons[wi];
                w.cooldown -= dt * fire_rate * overclock;
                if w.cooldown > 0.0 {
                    continue;
                }
                let kind = w.kind;
                let spec = kind.spec();
                let from = self.lines[li].car_pos(wi + 1);
                let range = spec.range * self.stats.range;
                let Some(target) = self.nearest_enemy(from, range, &[]) else {
                    self.lines[li].train.wagons[wi].cooldown = 0.0;
                    continue;
                };
                self.lines[li].train.wagons[wi].cooldown = spec.cooldown;
                self.shoot(kind, from, target);
            }
            // Cowcatcher: the locomotive rams beasts on the rails.
            let loco = self.lines[li].car_pos(0);
            for e in &mut self.enemies {
                if e.pos.dist(loco) < 16.0 + e.kind.stats().3 {
                    e.hp -= 40.0 * dt;
                }
            }
        }
        self.reap();
    }

    fn nearest_enemy(&self, from: V2, range: f32, skip: &[usize]) -> Option<usize> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(i, e)| e.hp > 0.0 && !skip.contains(i) && e.pos.dist(from) <= range)
            .min_by(|a, b| a.1.pos.dist(from).total_cmp(&b.1.pos.dist(from)))
            .map(|(i, _)| i)
    }

    fn shoot(&mut self, kind: WagonKind, from: V2, target: usize) {
        let spec = kind.spec();
        let dmg = spec.damage * self.stats.damage;
        let to = self.enemies[target].pos;
        self.events.push(Event::Shoot(kind));
        self.tracers.push(Tracer {
            from,
            to,
            kind,
            ttl: 0.12,
        });
        if spec.splash > 0.0 {
            for e in &mut self.enemies {
                if e.pos.dist(to) <= spec.splash {
                    e.hp -= dmg;
                    e.flash = 0.1;
                }
            }
            self.blasts.push(Blast {
                pos: to,
                radius: spec.splash,
                ttl: 0.35,
            });
        } else {
            self.enemies[target].hp -= dmg;
            self.enemies[target].flash = 0.1;
            let mut hit = vec![target];
            let mut last = to;
            for _ in 0..spec.chains {
                let Some(n) = self.nearest_enemy(last, 90.0, &hit) else {
                    break;
                };
                let p = self.enemies[n].pos;
                self.enemies[n].hp -= dmg;
                self.enemies[n].flash = 0.1;
                self.tracers.push(Tracer {
                    from: last,
                    to: p,
                    kind,
                    ttl: 0.12,
                });
                hit.push(n);
                last = p;
            }
        }
        self.events.push(Event::Hit);
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let big = matches!(e.kind, EnemyKind::Brute | EnemyKind::Behemoth);
                self.kills += 1;
                self.scraps.push(Scrap {
                    pos: e.pos,
                    value: e.kind.stats().4,
                    age: 0.0,
                });
                if big {
                    self.blasts.push(Blast {
                        pos: e.pos,
                        radius: e.kind.stats().3 * 3.0,
                        ttl: 0.5,
                    });
                }
                self.events.push(Event::Kill { big });
            } else {
                i += 1;
            }
        }
    }

    fn move_enemies(&mut self, dt: f32) {
        let fallback = self.target_station();
        let armor = self.stats.armor;
        for e in &mut self.enemies {
            e.flash = (e.flash - dt).max(0.0);
            let (_, speed, bite, radius, _) = e.kind.stats();
            if !self.stations[e.target].alive() {
                match fallback {
                    Some(t) => e.target = t,
                    None => continue,
                }
            }
            let st = &mut self.stations[e.target];
            let goal = st.pos();
            let d = e.pos.dist(goal);
            if d <= radius + 12.0 {
                let before = st.alive();
                st.hp -= bite * armor * dt;
                if before && !st.alive() {
                    st.hp = 0.0;
                    self.events.push(Event::StationLost);
                }
                continue;
            }
            // Chew any track underfoot (slowing down while doing so).
            let mut step = speed * dt;
            if let Some(tile) = world_to_tile(e.pos)
                && let Some(hp) = self.track.get_mut(&tile)
                && *hp > 0.0
            {
                *hp = (*hp - bite * 0.6 * armor * dt).max(0.0);
                step *= 0.6;
            }
            e.pos.x += (goal.x - e.pos.x) / d * step;
            e.pos.y += (goal.y - e.pos.y) / d * step;
        }
    }

    fn collect_scrap(&mut self, dt: f32) {
        let mut cars = Vec::new();
        for l in &self.lines {
            for k in 0..=l.train.wagons.len() {
                cars.push(l.car_pos(k));
            }
        }
        let radius = 60.0 * self.stats.magnet;
        let mut gained = 0;
        self.scraps.retain_mut(|s| {
            s.age += dt;
            let Some(c) = cars
                .iter()
                .copied()
                .min_by(|a, b| a.dist(s.pos).total_cmp(&b.dist(s.pos)))
            else {
                return s.age < 25.0;
            };
            let d = c.dist(s.pos);
            if d < 12.0 {
                gained += s.value;
                return false;
            }
            if d < radius {
                s.pos = s.pos.lerp(c, (dt * 8.0).min(1.0));
            }
            s.age < 25.0
        });
        if gained > 0 {
            self.xp += gained;
            self.cash += gained as f32 * 2.0;
            self.events.push(Event::Pickup);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running() -> Game {
        let mut g = Game::new(42);
        g.paused = false;
        g
    }

    #[test]
    fn starter_network() {
        let g = Game::new(1);
        assert_eq!(g.stations.len(), 2);
        assert_eq!(g.lines.len(), 1);
        assert_eq!(g.cash, START_CASH);
        assert!(g.paused);
    }

    #[test]
    fn building_requires_pause_and_cash() {
        let mut g = running();
        assert!(g.place_station((5, 5)).is_err());
        g.paused = true;
        assert!(g.place_station((5, 5)).is_ok());
        assert_eq!(g.cash, START_CASH - STATION_COST);
        assert_eq!(g.place_station((5, 5)), Err("Tile occupied"));
        g.cash = 0.0;
        assert_eq!(g.place_station((6, 6)), Err("Not enough cash"));
    }

    #[test]
    fn line_must_connect_stations() {
        let mut g = Game::new(1);
        g.place_station((14, 5)).unwrap();
        let path: Vec<Tile> = (5..=11).map(|y| (14, 12 - y + 5)).collect();
        // Runs (14,12)->(14,6): neither end is the new station.
        assert!(g.build_line(&path).is_err());
        let path: Vec<Tile> = (5..=11).rev().map(|y| (14, y)).collect();
        let before = g.cash;
        g.build_line(&path).unwrap();
        assert_eq!(g.cash, before - 5.0 * TRACK_COST);
        assert_eq!(g.lines.len(), 2);
        assert_eq!(g.selected_line, 1);
    }

    #[test]
    fn gapped_route_rejected() {
        let mut g = Game::new(1);
        let path = vec![(14, 11), (16, 11), (25, 11)];
        assert_eq!(g.build_line(&path), Err("Broken route"));
    }

    #[test]
    fn train_delivers_income() {
        let mut g = running();
        g.year_time = 1.0; // skip the opening wave
        let cash = g.cash;
        for _ in 0..400 {
            g.update(0.02);
            g.enemies.clear();
        }
        assert!(g.delivered >= 1);
        assert!(g.cash > cash);
        assert!(g.stations.iter().any(|s| s.earned > 0.0));
    }

    #[test]
    fn broken_track_blocks_train() {
        let mut g = running();
        g.year_time = 1.0;
        *g.track.get_mut(&(16, 11)).unwrap() = 0.0;
        for _ in 0..300 {
            g.update(0.02);
            g.enemies.clear();
        }
        assert!(g.lines[0].train.blocked);
        assert!(g.lines[0].train.pos < 2.0);
        g.paused = true;
        g.repair_all().unwrap();
        assert_eq!(g.track[&(16, 11)], TRACK_HP);
    }

    #[test]
    fn enemies_target_most_profitable_station() {
        let mut g = Game::new(3);
        g.stations[1].earned = 50.0;
        assert_eq!(g.target_station(), Some(1));
        g.spawn_enemy(EnemyKind::Crawler);
        assert_eq!(g.enemies[0].target, 1);
    }

    #[test]
    fn turret_kills_enemy_and_drops_scrap() {
        let mut g = running();
        g.year_time = 1.0;
        g.spawn_enemy(EnemyKind::Crawler);
        g.enemies[0].pos = g.lines[0].car_pos(1);
        g.enemies[0].pos.y += 40.0;
        for _ in 0..100 {
            g.update(0.02);
        }
        assert!(g.kills >= 1);
        assert!(g.events.contains(&Event::Shoot(WagonKind::Gatling)));
    }

    #[test]
    fn wagons_cost_cash_and_cap() {
        let mut g = Game::new(1);
        g.cash = 10_000.0;
        for _ in 0..(MAX_WAGONS - 1) {
            g.buy_wagon(WagonKind::Cannon).unwrap();
        }
        assert_eq!(g.buy_wagon(WagonKind::Tesla), Err("All trains are full"));
        assert_eq!(g.wagon_count(), MAX_WAGONS);
    }

    #[test]
    fn level_up_offers_three_distinct_upgrades() {
        let mut g = running();
        g.xp = g.xp_needed();
        g.check_level_up();
        let Phase::LevelUp(c) = g.phase.clone() else {
            panic!("expected level up");
        };
        assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        assert_eq!(g.level, 2);
        g.choose_upgrade(0);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn year_end_charges_upkeep_and_pauses() {
        let mut g = running();
        g.year_time = YEAR_SECS - 0.01;
        g.cash = 1000.0;
        let upkeep = g.upkeep();
        g.update(0.05);
        assert_eq!(g.year, 2);
        assert!(g.paused);
        assert!(g.cash <= 1000.0 - upkeep + 50.0);
    }

    #[test]
    fn bankrupt_loses() {
        let mut g = running();
        g.year_time = YEAR_SECS - 0.01;
        g.cash = 0.0;
        g.update(0.05);
        assert!(matches!(g.phase, Phase::Lost(_)));
    }

    #[test]
    fn surviving_ten_years_wins() {
        let mut g = running();
        g.year = YEARS_TO_WIN;
        g.year_time = YEAR_SECS - 0.01;
        g.cash = 1000.0;
        g.update(0.05);
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn losing_all_stations_loses() {
        let mut g = running();
        for s in &mut g.stations {
            s.hp = 0.0;
        }
        g.update(0.01);
        assert!(matches!(g.phase, Phase::Lost(_)));
    }

    #[test]
    fn waves_escalate() {
        let mut a = running();
        a.update(0.01);
        let mut b = running();
        b.year = 6;
        b.update(0.01);
        assert!(b.enemies.len() > a.enemies.len());
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(9);
        let mut b = Rng::new(9);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert!((0.0..1.0).contains(&a.f32()));
    }
}
