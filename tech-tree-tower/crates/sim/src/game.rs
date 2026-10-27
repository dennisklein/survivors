//! Game rules: Commander, enemy waves, the building grid with adjacency
//! synergies, level-up cards and the win/lose conditions.

use crate::math::V2;
use crate::rng::Rng;

/// Side length of the square world in pixels.
pub const WORLD: f32 = 2400.0;
/// Building grid size in cells (square).
pub const GRID: i32 = 13;
/// Size of a grid cell in world pixels.
pub const CELL: f32 = 48.0;
/// Seconds per wave.
pub const WAVE_LEN: f32 = 30.0;
/// Number of waves; surviving all of them wins.
pub const WAVES: u32 = 20;
/// Total run length in seconds.
pub const RUN_TIME: f32 = WAVE_LEN * WAVES as f32;

const COMMANDER_HP: f32 = 320.0;
const COMMANDER_SPEED: f32 = 200.0;
const COMMANDER_RANGE: f32 = 300.0;
const PICKUP_RADIUS: f32 = 22.0;

/// Top-left corner of the building grid in world space.
pub fn grid_origin() -> V2 {
    let half = GRID as f32 * CELL / 2.0;
    V2::new(WORLD / 2.0 - half, WORLD / 2.0 - half)
}

/// World-space centre of a grid cell.
pub fn cell_center(c: (i32, i32)) -> V2 {
    let o = grid_origin();
    V2::new(
        o.x + (c.0 as f32 + 0.5) * CELL,
        o.y + (c.1 as f32 + 0.5) * CELL,
    )
}

/// Grid cell containing a world position (may be outside the grid).
pub fn cell_at(p: V2) -> (i32, i32) {
    let o = grid_origin();
    (
        ((p.x - o.x) / CELL).floor() as i32,
        ((p.y - o.y) / CELL).floor() as i32,
    )
}

pub fn in_grid(c: (i32, i32)) -> bool {
    c.0 >= 0 && c.1 >= 0 && c.0 < GRID && c.1 < GRID
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Core,
    Mex,
    Solar,
    Wind,
    Fusion,
    Storage,
    Llt,
    Hlt,
    Rocket,
    Nano,
    Radar,
    Wall,
}

/// Every building that can show up on a level-up card.
pub const CARD_KINDS: [Kind; 11] = [
    Kind::Mex,
    Kind::Solar,
    Kind::Wind,
    Kind::Fusion,
    Kind::Storage,
    Kind::Llt,
    Kind::Hlt,
    Kind::Rocket,
    Kind::Nano,
    Kind::Radar,
    Kind::Wall,
];

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Core => "Command Core",
            Kind::Mex => "Metal Extractor",
            Kind::Solar => "Solar Collector",
            Kind::Wind => "Wind Turbine",
            Kind::Fusion => "Fusion Reactor",
            Kind::Storage => "Metal Storage",
            Kind::Llt => "Light Laser Tower",
            Kind::Hlt => "Heavy Laser Tower",
            Kind::Rocket => "Rocket Battery",
            Kind::Nano => "Nano Turret",
            Kind::Radar => "Radar Tower",
            Kind::Wall => "Dragon's Teeth",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Kind::Core => "CORE",
            Kind::Mex => "MEX",
            Kind::Solar => "SOL",
            Kind::Wind => "WIN",
            Kind::Fusion => "FUS",
            Kind::Storage => "STO",
            Kind::Llt => "LLT",
            Kind::Hlt => "HLT",
            Kind::Rocket => "RKT",
            Kind::Nano => "NANO",
            Kind::Radar => "RAD",
            Kind::Wall => "WALL",
        }
    }

    /// Card text: what the building does and what it combos with.
    pub fn blurb(self) -> &'static str {
        match self {
            Kind::Core => "Your base. Lose it and you lose.",
            Kind::Mex => "+XP/s. x1.5 per adjacent energy, x2 per Storage.",
            Kind::Solar => "Energy 1. Turrets next to it fire 30% faster.",
            Kind::Wind => "Energy 1, cheap. Also boosts adjacent Mex.",
            Kind::Fusion => "Energy 3 (2 cells). Explodes when destroyed.",
            Kind::Storage => "Doubles adjacent Mex income. Sturdy.",
            Kind::Llt => "Fast laser. Loves energy, Nano, Radar.",
            Kind::Hlt => "Heavy laser, long range, big hits.",
            Kind::Rocket => "Splash rockets, longest range.",
            Kind::Nano => "Repairs nearby. Adjacent turrets +40% dmg.",
            Kind::Radar => "Adjacent turrets +30% range.",
            Kind::Wall => "3-cell barrier. Adjacent turrets take half damage.",
        }
    }

    /// Footprint offsets at rotation 0.
    pub fn footprint(self) -> &'static [(i32, i32)] {
        match self {
            Kind::Core => &[(0, 0), (1, 0), (0, 1), (1, 1)],
            Kind::Fusion => &[(0, 0), (1, 0)],
            Kind::Wall => &[(-1, 0), (0, 0), (1, 0)],
            _ => &[(0, 0)],
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            Kind::Core => 2500.0,
            Kind::Wall => 600.0,
            Kind::Storage => 400.0,
            Kind::Fusion => 350.0,
            Kind::Hlt => 320.0,
            Kind::Llt | Kind::Rocket => 220.0,
            Kind::Nano | Kind::Mex => 200.0,
            Kind::Radar | Kind::Solar => 160.0,
            Kind::Wind => 110.0,
        }
    }

    pub fn energy(self) -> f32 {
        match self {
            Kind::Solar | Kind::Wind | Kind::Core => 1.0,
            Kind::Fusion => 3.0,
            _ => 0.0,
        }
    }

    pub fn is_turret(self) -> bool {
        matches!(self, Kind::Llt | Kind::Hlt | Kind::Rocket)
    }

    /// (range, damage, cooldown) before synergies.
    pub fn weapon(self) -> Option<(f32, f32, f32)> {
        match self {
            Kind::Core => Some((210.0, 9.0, 0.6)),
            Kind::Llt => Some((230.0, 11.0, 0.45)),
            Kind::Hlt => Some((290.0, 46.0, 1.3)),
            Kind::Rocket => Some((380.0, 32.0, 1.9)),
            _ => None,
        }
    }
}

/// Rotates a footprint offset by `rot` quarter turns.
pub fn rotate(o: (i32, i32), rot: u8) -> (i32, i32) {
    let mut p = o;
    for _ in 0..rot % 4 {
        p = (-p.1, p.0);
    }
    p
}

/// Cells covered by `kind` anchored at `at` with rotation `rot`.
pub fn cells_of(kind: Kind, at: (i32, i32), rot: u8) -> Vec<(i32, i32)> {
    kind.footprint()
        .iter()
        .map(|&o| {
            let r = rotate(o, rot);
            (at.0 + r.0, at.1 + r.1)
        })
        .collect()
}

/// Synergy-derived stats of a building.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub rate: f32,
    pub dmg: f32,
    pub range: f32,
    pub armor: f32,
    /// XP per second (Mex).
    pub income: f32,
    /// HP per second (Nano).
    pub heal: f32,
    /// Human-readable synergy list.
    pub notes: Vec<String>,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            rate: 1.0,
            dmg: 1.0,
            range: 1.0,
            armor: 1.0,
            income: 0.0,
            heal: 0.0,
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Building {
    pub kind: Kind,
    pub cells: Vec<(i32, i32)>,
    pub hp: f32,
    pub cooldown: f32,
    pub stats: Stats,
    pub flash: f32,
}

impl Building {
    pub fn center(&self) -> V2 {
        let mut s = V2::ZERO;
        for &c in &self.cells {
            s += cell_center(c);
        }
        s * (1.0 / self.cells.len() as f32)
    }
}

/// Indices of buildings 4-adjacent to building `i`.
pub fn neighbors(buildings: &[Building], i: usize) -> Vec<usize> {
    let mine = &buildings[i].cells;
    let mut out = Vec::new();
    for (j, b) in buildings.iter().enumerate() {
        if j == i {
            continue;
        }
        let touch = b.cells.iter().any(|&(x, y)| {
            mine.iter()
                .any(|&(a, c)| (a - x).abs() + (c - y).abs() == 1)
        });
        if touch {
            out.push(j);
        }
    }
    out
}

/// Computes the stats a building of `kind` would get with `adj` neighbours.
pub fn synergy(kind: Kind, adj: &[Kind]) -> Stats {
    let mut s = Stats::default();
    let energy: f32 = adj.iter().map(|k| k.energy()).sum();
    let count = |k: Kind| adj.iter().filter(|&&a| a == k).count() as f32;
    let nanos = count(Kind::Nano);
    let radars = count(Kind::Radar);
    let walls = count(Kind::Wall);
    let stores = count(Kind::Storage);
    let winds = count(Kind::Wind);
    if kind.is_turret() {
        s.rate = 1.0 + 0.3 * energy;
        s.dmg = 1.0 + 0.4 * nanos;
        s.range = 1.0 + 0.3 * radars;
        s.armor = if walls > 0.0 { 0.5 } else { 1.0 };
        if energy > 0.0 {
            s.notes
                .push(format!("+{:.0}% fire rate", (s.rate - 1.0) * 100.0));
        }
        if nanos > 0.0 {
            s.notes
                .push(format!("+{:.0}% damage", (s.dmg - 1.0) * 100.0));
        }
        if radars > 0.0 {
            s.notes
                .push(format!("+{:.0}% range", (s.range - 1.0) * 100.0));
        }
        if walls > 0.0 {
            s.notes.push("half damage taken".into());
        }
    }
    match kind {
        Kind::Mex => {
            let mult = 1.5f32.powf(energy + winds) * 2f32.powf(stores);
            s.income = 0.6 * mult;
            if mult > 1.0 {
                s.notes.push(format!("x{mult:.1} income"));
            }
        }
        Kind::Nano => {
            s.heal = 6.0 * (1.0 + 0.5 * energy);
            if energy > 0.0 {
                s.notes.push(format!("+{:.0}% repair", energy * 50.0));
            }
        }
        Kind::Core => {
            s.income = 0.2;
        }
        _ => {}
    }
    s
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    OutOfGrid,
    Occupied,
    Disconnected,
}

/// Checks whether `cells` can be built: inside the grid, free and touching
/// the existing base.
pub fn can_place(buildings: &[Building], cells: &[(i32, i32)]) -> Result<(), PlaceError> {
    if !cells.iter().all(|&c| in_grid(c)) {
        return Err(PlaceError::OutOfGrid);
    }
    let occupied = |c: (i32, i32)| buildings.iter().any(|b| b.cells.contains(&c));
    if cells.iter().any(|&c| occupied(c)) {
        return Err(PlaceError::Occupied);
    }
    let touches = cells.iter().any(|&(x, y)| {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|&(dx, dy)| occupied((x + dx, y + dy)))
    });
    if touches {
        Ok(())
    } else {
        Err(PlaceError::Disconnected)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Pawn,
    Grunt,
    Raider,
    Tank,
    Boss,
}

impl EnemyKind {
    /// (hp, speed, contact dps, radius, xp)
    pub fn stats(self) -> (f32, f32, f32, f32, f32) {
        match self {
            EnemyKind::Pawn => (22.0, 85.0, 14.0, 10.0, 1.0),
            EnemyKind::Grunt => (60.0, 65.0, 22.0, 13.0, 2.0),
            EnemyKind::Raider => (35.0, 150.0, 16.0, 11.0, 2.0),
            EnemyKind::Tank => (200.0, 45.0, 24.0, 18.0, 6.0),
            EnemyKind::Boss => (1500.0, 40.0, 60.0, 34.0, 40.0),
        }
    }

    fn cost(self) -> f32 {
        match self {
            EnemyKind::Pawn => 1.0,
            EnemyKind::Grunt => 2.5,
            EnemyKind::Raider => 2.0,
            EnemyKind::Tank => 7.0,
            EnemyKind::Boss => 0.0,
        }
    }

    /// Tanks go for the base; everyone else hunts the Commander.
    pub fn sieges(self) -> bool {
        matches!(self, EnemyKind::Tank | EnemyKind::Boss)
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub radius: f32,
    pub flash: f32,
    pub shot_cd: f32,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub splash: f32,
    pub life: f32,
    pub hostile: bool,
    pub rocket: bool,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub pos: V2,
    pub xp: f32,
    pub heal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Armor,
    Repair,
    Magnet,
}

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Commander: Plasma Overcharge",
            Upgrade::FireRate => "Commander: Autoloader",
            Upgrade::Armor => "Commander: Armor Plating",
            Upgrade::Repair => "Field Repair",
            Upgrade::Magnet => "Wreck Magnet",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Upgrade::Damage => "+25% Commander damage.",
            Upgrade::FireRate => "+20% Commander fire rate.",
            Upgrade::Armor => "+80 max HP and full heal.",
            Upgrade::Repair => "Fully repair every building.",
            Upgrade::Magnet => "+60% pickup range.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Build(Kind),
    Up(Upgrade),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    Choosing([Card; 3]),
    Placing { kind: Kind, rot: u8 },
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot,
    Beam { a: V2, b: V2, heavy: bool },
    RocketLaunch,
    Hit(V2),
    Kill { pos: V2, big: bool },
    BuildingLost(V2),
    Explosion { pos: V2, r: f32 },
    Pickup,
    Heal,
    LevelUp,
    Built,
    Denied,
    WaveStart(u32),
    BossSpawn,
    CommanderHurt,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub move_dir: V2,
}

pub struct Commander {
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub dmg: f32,
    pub rate: f32,
    pub cooldown: f32,
    pub hurt_cd: f32,
    pub magnet: f32,
}

pub struct Game {
    pub rng: Rng,
    pub time: f32,
    pub phase: Phase,
    pub commander: Commander,
    pub buildings: Vec<Building>,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub pickups: Vec<Pickup>,
    pub level: u32,
    pub xp: f32,
    pub pending_levels: u32,
    pub wave: u32,
    pub kills: u32,
    spawn_budget: f32,
    pub events: Vec<Event>,
}

/// XP needed to go from `level` to the next one.
pub fn xp_needed(level: u32) -> f32 {
    5.0 + level as f32 * 5.0 + (level as f32).powf(1.6)
}

/// Spawn points per second for a wave.
pub fn wave_rate(wave: u32) -> f32 {
    0.8 + wave as f32 * 0.3 + (wave as f32 / 7.0).powi(2)
}

/// Enemy HP multiplier for a wave.
pub fn wave_hp(wave: u32) -> f32 {
    1.0 + (wave.saturating_sub(1)) as f32 * 0.1
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Game {
            rng: Rng::new(seed),
            time: 0.0,
            phase: Phase::Playing,
            commander: Commander {
                pos: V2::new(WORLD / 2.0, WORLD / 2.0 + CELL * 2.5),
                hp: COMMANDER_HP,
                max_hp: COMMANDER_HP,
                dmg: 18.0,
                rate: 1.0,
                cooldown: 0.0,
                hurt_cd: 0.0,
                magnet: 110.0,
            },
            buildings: Vec::new(),
            enemies: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            level: 1,
            xp: 0.0,
            pending_levels: 0,
            wave: 0,
            kills: 0,
            spawn_budget: 0.0,
            events: Vec::new(),
        };
        let c = GRID / 2 - 1;
        g.add_building(Kind::Core, cells_of(Kind::Core, (c, c), 0));
        g
    }

    fn add_building(&mut self, kind: Kind, cells: Vec<(i32, i32)>) {
        self.buildings.push(Building {
            kind,
            cells,
            hp: kind.max_hp(),
            cooldown: 0.0,
            stats: Stats::default(),
            flash: 0.0,
        });
        self.recompute();
    }

    /// Re-derives every building's synergy stats from its neighbours.
    pub fn recompute(&mut self) {
        for i in 0..self.buildings.len() {
            let adj: Vec<Kind> = neighbors(&self.buildings, i)
                .into_iter()
                .map(|j| self.buildings[j].kind)
                .collect();
            self.buildings[i].stats = synergy(self.buildings[i].kind, &adj);
        }
    }

    /// Stats a building would get if placed at `cells` now.
    pub fn preview(&self, kind: Kind, cells: &[(i32, i32)]) -> Stats {
        let mut tmp = self.buildings.clone();
        tmp.push(Building {
            kind,
            cells: cells.to_vec(),
            hp: 1.0,
            cooldown: 0.0,
            stats: Stats::default(),
            flash: 0.0,
        });
        let i = tmp.len() - 1;
        let adj: Vec<Kind> = neighbors(&tmp, i)
            .into_iter()
            .map(|j| tmp[j].kind)
            .collect();
        synergy(kind, &adj)
    }

    pub fn xp_needed(&self) -> f32 {
        xp_needed(self.level)
    }

    pub fn core_alive(&self) -> bool {
        self.buildings.iter().any(|b| b.kind == Kind::Core)
    }

    fn roll_cards(&mut self) -> [Card; 3] {
        let mut cards = [Card::Up(Upgrade::Damage); 3];
        let mut used: Vec<Card> = Vec::new();
        for (slot, card) in cards.iter_mut().enumerate() {
            loop {
                let c = if slot < 2 || self.rng.chance(0.6) {
                    let pool: Vec<Kind> = CARD_KINDS
                        .iter()
                        .copied()
                        .filter(|k| *k != Kind::Fusion || self.level >= 6)
                        .collect();
                    Card::Build(pool[self.rng.below(pool.len())])
                } else {
                    let ups = [
                        Upgrade::Damage,
                        Upgrade::FireRate,
                        Upgrade::Armor,
                        Upgrade::Repair,
                        Upgrade::Magnet,
                    ];
                    Card::Up(ups[self.rng.below(ups.len())])
                };
                if !used.contains(&c) {
                    used.push(c);
                    *card = c;
                    break;
                }
            }
        }
        cards
    }

    /// Picks level-up card `i` (0..3).
    pub fn choose(&mut self, i: usize) {
        let Phase::Choosing(cards) = self.phase.clone() else {
            return;
        };
        let Some(card) = cards.get(i).copied() else {
            return;
        };
        match card {
            Card::Build(kind) => {
                self.phase = Phase::Placing { kind, rot: 0 };
                return;
            }
            Card::Up(u) => {
                let c = &mut self.commander;
                match u {
                    Upgrade::Damage => c.dmg *= 1.25,
                    Upgrade::FireRate => c.rate *= 1.2,
                    Upgrade::Armor => {
                        c.max_hp += 80.0;
                        c.hp = c.max_hp;
                    }
                    Upgrade::Repair => {
                        for b in &mut self.buildings {
                            b.hp = b.kind.max_hp();
                        }
                    }
                    Upgrade::Magnet => c.magnet *= 1.6,
                }
                self.events.push(Event::Heal);
            }
        }
        self.after_pick();
    }

    fn after_pick(&mut self) {
        if self.pending_levels > 0 {
            self.pending_levels -= 1;
            let cards = self.roll_cards();
            self.phase = Phase::Choosing(cards);
            self.events.push(Event::LevelUp);
        } else {
            self.phase = Phase::Playing;
        }
    }

    pub fn rotate_ghost(&mut self) {
        if let Phase::Placing { rot, .. } = &mut self.phase {
            *rot = (*rot + 1) % 4;
        }
    }

    /// Tries to place the pending building at `at`. Returns whether it was
    /// built.
    pub fn place(&mut self, at: (i32, i32)) -> bool {
        let Phase::Placing { kind, rot } = self.phase else {
            return false;
        };
        let cells = cells_of(kind, at, rot);
        match can_place(&self.buildings, &cells) {
            Ok(()) => {
                self.add_building(kind, cells);
                self.events.push(Event::Built);
                self.after_pick();
                true
            }
            Err(_) => {
                self.events.push(Event::Denied);
                false
            }
        }
    }

    fn gain_xp(&mut self, xp: f32) {
        self.xp += xp;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.pending_levels += 1;
        }
    }

    fn pick_enemy(&mut self) -> EnemyKind {
        let w = self.wave;
        let mut pool = vec![EnemyKind::Pawn, EnemyKind::Pawn];
        if w >= 2 {
            pool.push(EnemyKind::Grunt);
        }
        if w >= 4 {
            pool.push(EnemyKind::Raider);
        }
        if w >= 6 {
            pool.push(EnemyKind::Tank);
            pool.push(EnemyKind::Grunt);
        }
        if w >= 12 {
            pool.push(EnemyKind::Tank);
            pool.push(EnemyKind::Raider);
        }
        pool[self.rng.below(pool.len())]
    }

    fn spawn(&mut self, kind: EnemyKind) {
        let (hp, _, _, radius, _) = kind.stats();
        let mult = wave_hp(self.wave)
            * if kind == EnemyKind::Boss {
                0.4 + 0.6 * (self.wave as f32 / 5.0).max(1.0)
            } else {
                1.0
            };
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let d = self.rng.range(720.0, 820.0);
        let c = self.commander.pos;
        let pos = V2::new(
            (c.x + a.cos() * d).clamp(20.0, WORLD - 20.0),
            (c.y + a.sin() * d).clamp(20.0, WORLD - 20.0),
        );
        self.enemies.push(Enemy {
            kind,
            pos,
            hp: hp * mult,
            max_hp: hp * mult,
            radius,
            flash: 0.0,
            shot_cd: 2.0,
        });
    }

    /// Advances the simulation by `dt` seconds. Only runs while playing.
    pub fn step(&mut self, dt: f32, input: Input) {
        if self.phase != Phase::Playing {
            return;
        }
        if self.pending_levels > 0 {
            self.pending_levels -= 1;
            let cards = self.roll_cards();
            self.phase = Phase::Choosing(cards);
            self.events.push(Event::LevelUp);
            return;
        }
        self.time += dt;
        self.update_waves(dt);
        self.update_commander(dt, input);
        self.update_buildings(dt);
        self.update_enemies(dt);
        self.update_projectiles(dt);
        self.update_pickups(dt);
        self.check_end();
    }

    fn update_waves(&mut self, dt: f32) {
        let wave = ((self.time / WAVE_LEN) as u32 + 1).min(WAVES);
        if wave != self.wave {
            self.wave = wave;
            self.events.push(Event::WaveStart(wave));
            if wave.is_multiple_of(5) {
                self.spawn(EnemyKind::Boss);
                self.events.push(Event::BossSpawn);
            }
        }
        if self.enemies.len() < 320 {
            self.spawn_budget += wave_rate(self.wave) * dt;
        }
        while self.spawn_budget > 0.0 {
            let k = self.pick_enemy();
            self.spawn_budget -= k.cost();
            self.spawn(k);
        }
    }

    fn update_commander(&mut self, dt: f32, input: Input) {
        let c = &mut self.commander;
        c.pos += input.move_dir.norm() * (COMMANDER_SPEED * dt);
        c.pos.x = c.pos.x.clamp(16.0, WORLD - 16.0);
        c.pos.y = c.pos.y.clamp(16.0, WORLD - 16.0);
        c.hurt_cd -= dt;
        c.cooldown -= dt;
        if c.cooldown <= 0.0 {
            let pos = c.pos;
            if let Some(t) = self.nearest_enemy(pos, COMMANDER_RANGE) {
                let tp = self.enemies[t].pos;
                let c = &mut self.commander;
                c.cooldown = 0.42 / c.rate;
                self.projectiles.push(Projectile {
                    pos,
                    vel: (tp - pos).norm() * 620.0,
                    dmg: c.dmg,
                    splash: 0.0,
                    life: 0.7,
                    hostile: false,
                    rocket: false,
                });
                self.events.push(Event::Shot);
            }
        }
    }

    fn nearest_enemy(&self, p: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range * range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist2(p);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn damage_enemy(&mut self, i: usize, dmg: f32) {
        let e = &mut self.enemies[i];
        e.hp -= dmg;
        e.flash = 0.1;
    }

    fn update_buildings(&mut self, dt: f32) {
        let mut income = 0.0;
        for i in 0..self.buildings.len() {
            let b = &mut self.buildings[i];
            b.flash = (b.flash - dt).max(0.0);
            if b.kind == Kind::Core {
                b.hp = (b.hp + 4.0 * dt).min(b.kind.max_hp());
            }
            income += b.stats.income;
            let (heal, center) = (b.stats.heal * dt, b.center());
            if heal > 0.0 {
                for o in &mut self.buildings {
                    if o.center().dist(center) < CELL * 3.5 {
                        o.hp = (o.hp + heal).min(o.kind.max_hp());
                    }
                }
                let c = &mut self.commander;
                if c.pos.dist(center) < CELL * 3.5 {
                    c.hp = (c.hp + heal).min(c.max_hp);
                }
            }
            let b = &mut self.buildings[i];
            let Some((range, dmg, cd)) = b.kind.weapon() else {
                continue;
            };
            b.cooldown -= dt;
            if b.cooldown > 0.0 {
                continue;
            }
            let center = b.center();
            let range = range * b.stats.range;
            let dmg = dmg * b.stats.dmg;
            let rate = b.stats.rate;
            let kind = b.kind;
            let Some(t) = self.nearest_enemy(center, range) else {
                continue;
            };
            self.buildings[i].cooldown = cd / rate;
            let tp = self.enemies[t].pos;
            match kind {
                Kind::Rocket => {
                    self.projectiles.push(Projectile {
                        pos: center,
                        vel: (tp - center).norm() * 380.0,
                        dmg,
                        splash: 70.0,
                        life: range / 380.0 + 0.2,
                        hostile: false,
                        rocket: true,
                    });
                    self.events.push(Event::RocketLaunch);
                }
                _ => {
                    self.damage_enemy(t, dmg);
                    self.events.push(Event::Beam {
                        a: center,
                        b: tp,
                        heavy: kind == Kind::Hlt,
                    });
                }
            }
        }
        self.gain_xp(income * dt);
        self.reap_enemies();
    }

    fn building_hit(&self, p: V2, r: f32) -> Option<usize> {
        let c = cell_at(p);
        let reach = (r / CELL).ceil() as i32 + 1;
        for (i, b) in self.buildings.iter().enumerate() {
            for &cell in &b.cells {
                if (cell.0 - c.0).abs() > reach || (cell.1 - c.1).abs() > reach {
                    continue;
                }
                let cc = cell_center(cell);
                let h = CELL / 2.0;
                let nx = p.x.clamp(cc.x - h, cc.x + h);
                let ny = p.y.clamp(cc.y - h, cc.y + h);
                if V2::new(nx, ny).dist2(p) < r * r {
                    return Some(i);
                }
            }
        }
        None
    }

    fn hurt_building(&mut self, i: usize, dmg: f32) {
        let b = &mut self.buildings[i];
        b.hp -= dmg * b.stats.armor;
        b.flash = 0.1;
    }

    fn update_enemies(&mut self, dt: f32) {
        let cpos = self.commander.pos;
        let core = self
            .buildings
            .iter()
            .find(|b| b.kind == Kind::Core)
            .map(|b| b.center())
            .unwrap_or(cpos);
        let n = self.enemies.len();
        // Separation push, bucketed coarsely to keep it cheap.
        let mut push = vec![V2::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.enemies[i].pos - self.enemies[j].pos;
                let rr = self.enemies[i].radius + self.enemies[j].radius;
                let l2 = d.x * d.x + d.y * d.y;
                if l2 < rr * rr && l2 > 1e-4 {
                    let l = l2.sqrt();
                    let f = d * ((rr - l) / l * 0.5);
                    push[i] += f;
                    push[j] = push[j] - f;
                }
            }
        }
        let mut commander_dmg = 0.0;
        let mut hits: Vec<(usize, f32)> = Vec::new();
        let mut shots = Vec::new();
        for (i, e) in self.enemies.iter_mut().enumerate() {
            let (_, speed, dps, _, _) = e.kind.stats();
            e.flash = (e.flash - dt).max(0.0);
            let target = if e.kind.sieges() && e.pos.dist(core) < e.pos.dist(cpos) + 200.0 {
                core
            } else {
                cpos
            };
            e.pos += (target - e.pos).norm() * (speed * dt) + push[i];
            if e.pos.dist(cpos) < e.radius + 14.0 {
                commander_dmg += dps * dt;
            }
            if e.kind == EnemyKind::Boss {
                e.shot_cd -= dt;
                if e.shot_cd <= 0.0 && e.pos.dist(cpos) < 500.0 {
                    e.shot_cd = 1.6;
                    shots.push((e.pos, target));
                }
            }
        }
        for (p, t) in shots {
            for k in -1..=1 {
                let a = (t - p).angle() + k as f32 * 0.25;
                self.projectiles.push(Projectile {
                    pos: p,
                    vel: V2::from_angle(a) * 260.0,
                    dmg: 22.0,
                    splash: 0.0,
                    life: 2.5,
                    hostile: true,
                    rocket: false,
                });
            }
        }
        // Buildings block enemies, and get chewed on.
        for i in 0..self.enemies.len() {
            let (p, r) = (self.enemies[i].pos, self.enemies[i].radius);
            if let Some(bi) = self.building_hit(p, r) {
                let b = &self.buildings[bi];
                let mut best = b.center();
                let mut bd = f32::MAX;
                for &cell in &b.cells {
                    let d = cell_center(cell).dist2(p);
                    if d < bd {
                        bd = d;
                        best = cell_center(cell);
                    }
                }
                let away = (p - best).norm();
                let away = if away == V2::ZERO {
                    V2::new(0.0, 1.0)
                } else {
                    away
                };
                self.enemies[i].pos += away * (60.0 * dt);
                let k = self.enemies[i].kind;
                let bite = if k.sieges() { 1.0 } else { 0.3 };
                hits.push((bi, k.stats().2 * bite * dt));
            }
        }
        for (bi, d) in hits {
            self.hurt_building(bi, d);
        }
        if commander_dmg > 0.0 {
            self.commander.hp -= commander_dmg;
            if self.commander.hurt_cd <= 0.0 {
                self.commander.hurt_cd = 0.4;
                self.events.push(Event::CommanderHurt);
            }
        }
        self.reap_buildings();
    }

    fn reap_buildings(&mut self) {
        let mut changed = false;
        let mut i = 0;
        while i < self.buildings.len() {
            if self.buildings[i].hp <= 0.0 {
                let b = self.buildings.remove(i);
                let c = b.center();
                self.events.push(Event::BuildingLost(c));
                if b.kind == Kind::Fusion {
                    self.explode(c, 160.0, 400.0);
                }
                changed = true;
            } else {
                i += 1;
            }
        }
        if changed {
            self.recompute();
        }
    }

    fn explode(&mut self, p: V2, r: f32, dmg: f32) {
        self.events.push(Event::Explosion { pos: p, r });
        for i in 0..self.enemies.len() {
            if self.enemies[i].pos.dist(p) < r + self.enemies[i].radius {
                self.damage_enemy(i, dmg);
            }
        }
    }

    fn update_projectiles(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.projectiles.len() {
            let pr = self.projectiles[i].clone();
            let pos = pr.pos + pr.vel * dt;
            let mut dead = pr.life - dt <= 0.0;
            if pr.hostile {
                if pos.dist(self.commander.pos) < 16.0 {
                    self.commander.hp -= pr.dmg;
                    self.events.push(Event::CommanderHurt);
                    dead = true;
                } else if let Some(b) = self.building_hit(pos, 4.0) {
                    self.hurt_building(b, pr.dmg);
                    dead = true;
                }
            } else if let Some(t) = self
                .enemies
                .iter()
                .position(|e| e.pos.dist2(pos) < (e.radius + 4.0).powi(2))
            {
                if pr.splash > 0.0 {
                    self.explode(pos, pr.splash, pr.dmg);
                } else {
                    self.damage_enemy(t, pr.dmg);
                    self.events.push(Event::Hit(pos));
                }
                dead = true;
            } else if dead && pr.rocket {
                self.explode(pos, pr.splash, pr.dmg);
            }
            if dead {
                self.projectiles.swap_remove(i);
            } else {
                let p = &mut self.projectiles[i];
                p.pos = pos;
                p.life -= dt;
                i += 1;
            }
        }
        self.reap_enemies();
        self.reap_buildings();
    }

    fn reap_enemies(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let (_, _, _, _, xp) = e.kind.stats();
                let big = matches!(e.kind, EnemyKind::Tank | EnemyKind::Boss);
                self.events.push(Event::Kill { pos: e.pos, big });
                self.kills += 1;
                self.pickups.push(Pickup {
                    pos: e.pos,
                    xp,
                    heal: false,
                });
                if self.rng.chance(if big { 0.5 } else { 0.012 }) {
                    self.pickups.push(Pickup {
                        pos: e.pos + V2::new(10.0, 0.0),
                        xp: 0.0,
                        heal: true,
                    });
                }
            } else {
                i += 1;
            }
        }
    }

    fn update_pickups(&mut self, dt: f32) {
        let c = self.commander.pos;
        let magnet = self.commander.magnet;
        let mut gained = 0.0;
        let mut healed = false;
        self.pickups.retain_mut(|p| {
            let d = p.pos.dist(c);
            if d < PICKUP_RADIUS {
                if p.heal {
                    healed = true;
                } else {
                    gained += p.xp;
                }
                return false;
            }
            if d < magnet {
                p.pos += (c - p.pos).norm() * (420.0 * dt);
            }
            true
        });
        if healed {
            let cm = &mut self.commander;
            cm.hp = (cm.hp + cm.max_hp * 0.35).min(cm.max_hp);
            self.events.push(Event::Heal);
        }
        if gained > 0.0 {
            self.events.push(Event::Pickup);
            self.gain_xp(gained);
        }
        // Keep the wreck count bounded by merging the oldest ones.
        if self.pickups.len() > 400 {
            let extra: f32 = self.pickups.drain(0..100).map(|p| p.xp).sum();
            self.gain_xp(extra * 0.5);
        }
    }

    fn check_end(&mut self) {
        if self.commander.hp <= 0.0 || !self.core_alive() {
            self.commander.hp = self.commander.hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        } else if self.time >= RUN_TIME {
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

    fn core_cell() -> (i32, i32) {
        (GRID / 2 - 1, GRID / 2 - 1)
    }

    #[test]
    fn rotation_cycles_back() {
        assert_eq!(rotate((1, 0), 1), (0, 1));
        assert_eq!(rotate((1, 0), 4), (1, 0));
        let wall = cells_of(Kind::Wall, (5, 5), 1);
        assert!(wall.contains(&(5, 4)) && wall.contains(&(5, 6)));
    }

    #[test]
    fn placement_requires_free_connected_cells() {
        let g = Game::new(1);
        let (cx, cy) = core_cell();
        assert_eq!(
            can_place(&g.buildings, &[(cx, cy)]),
            Err(PlaceError::Occupied)
        );
        assert_eq!(
            can_place(&g.buildings, &[(0, 0)]),
            Err(PlaceError::Disconnected)
        );
        assert_eq!(
            can_place(&g.buildings, &[(-1, 0)]),
            Err(PlaceError::OutOfGrid)
        );
        assert_eq!(can_place(&g.buildings, &[(cx - 1, cy)]), Ok(()));
    }

    #[test]
    fn energy_speeds_up_adjacent_turrets() {
        let alone = synergy(Kind::Llt, &[]);
        let powered = synergy(Kind::Llt, &[Kind::Solar, Kind::Fusion]);
        assert_eq!(alone.rate, 1.0);
        assert!((powered.rate - 2.2).abs() < 1e-5);
        assert!(!powered.notes.is_empty());
    }

    #[test]
    fn nano_radar_wall_synergies() {
        let s = synergy(Kind::Hlt, &[Kind::Nano, Kind::Radar, Kind::Wall]);
        assert!((s.dmg - 1.4).abs() < 1e-5);
        assert!((s.range - 1.3).abs() < 1e-5);
        assert_eq!(s.armor, 0.5);
    }

    #[test]
    fn mex_income_multiplies() {
        let base = synergy(Kind::Mex, &[]).income;
        let combo = synergy(Kind::Mex, &[Kind::Solar, Kind::Storage]).income;
        assert!((combo / base - 3.0).abs() < 1e-4);
    }

    #[test]
    fn level_up_offers_cards_and_placing_builds() {
        let mut g = Game::new(7);
        g.gain_xp(g.xp_needed() + 0.1);
        g.step(0.016, Input::default());
        let Phase::Choosing(cards) = g.phase.clone() else {
            panic!("expected cards, got {:?}", g.phase);
        };
        let i = cards
            .iter()
            .position(|c| matches!(c, Card::Build(_)))
            .expect("at least two building cards");
        g.choose(i);
        assert!(matches!(g.phase, Phase::Placing { .. }));
        let (cx, cy) = core_cell();
        assert!(!g.place((0, 0)));
        // Left of the core is free; rotate so walls run vertically.
        g.rotate_ghost();
        assert!(g.place((cx - 2, cy)) || g.place((cx - 1, cy)));
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.buildings.len(), 2);
    }

    #[test]
    fn synergies_update_when_neighbours_die() {
        let mut g = Game::new(3);
        let (cx, cy) = core_cell();
        g.add_building(Kind::Llt, vec![(cx - 2, cy)]);
        g.add_building(Kind::Solar, vec![(cx - 3, cy)]);
        let llt = g
            .buildings
            .iter()
            .position(|b| b.kind == Kind::Llt)
            .unwrap();
        assert!(g.buildings[llt].stats.rate > 1.0);
        let sol = g
            .buildings
            .iter()
            .position(|b| b.kind == Kind::Solar)
            .unwrap();
        g.buildings[sol].hp = -1.0;
        g.reap_buildings();
        let llt = g
            .buildings
            .iter()
            .position(|b| b.kind == Kind::Llt)
            .unwrap();
        assert_eq!(g.buildings[llt].stats.rate, 1.0);
    }

    #[test]
    fn idle_commander_eventually_dies() {
        let mut g = Game::new(11);
        for _ in 0..(60 * 400) {
            match g.phase {
                Phase::Choosing(_) => g.choose(2),
                Phase::Placing { .. } => {
                    g.phase = Phase::Playing;
                }
                Phase::Lost => break,
                _ => {}
            }
            g.step(1.0 / 60.0, Input::default());
            g.drain_events();
        }
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn waves_escalate_and_bosses_arrive() {
        assert!(wave_rate(10) > wave_rate(1) * 3.0);
        assert!(wave_hp(20) > 2.0);
        let mut g = Game::new(5);
        g.commander.hp = 1e9;
        g.commander.max_hp = 1e9;
        g.buildings[0].hp = 1e9;
        let mut boss = false;
        for _ in 0..(60 * 151) {
            if let Phase::Choosing(_) = g.phase {
                g.phase = Phase::Playing;
                g.pending_levels = 0;
            }
            g.step(1.0 / 60.0, Input::default());
            boss |= g.drain_events().contains(&Event::BossSpawn);
        }
        assert_eq!(g.wave, 6, "{:?} t={} lvl={}", g.phase, g.time, g.level);
        assert!(boss);
    }

    #[test]
    fn surviving_all_waves_wins() {
        let mut g = Game::new(9);
        g.time = RUN_TIME - 0.001;
        g.wave = WAVES;
        g.step(0.01, Input::default());
        assert_eq!(g.phase, Phase::Won);
    }
}
