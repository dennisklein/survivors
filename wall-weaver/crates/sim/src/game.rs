//! Pure game rules for Wall Weaver: a tile grid the commander weaves walls
//! into, a flow field enemies follow toward the core, turrets, waves and
//! level-up upgrades. No rendering or audio lives here.

use crate::rng::Rng;
use std::collections::VecDeque;

pub const W: usize = 40;
pub const H: usize = 28;
pub const TILE: f32 = 24.0;
pub const CORE: (usize, usize) = (W / 2, H / 2);
pub const WAVES: u32 = 20;
pub const UNREACHABLE: u32 = u32::MAX;

pub const WALL_COST: u32 = 1;
pub const TURRET_COST: u32 = 25;
pub const MAX_TURRET_LEVEL: u32 = 3;
pub const WALL_SETTLE: f32 = 0.45;
pub const COMMANDER_RADIUS: f32 = 8.0;
pub const CORE_RADIUS: f32 = 18.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Open,
    /// Wall trail that solidifies once its timer runs out and the tile is clear.
    Pending,
    Wall,
    Turret,
    Core,
}

impl Cell {
    pub fn blocks(self) -> bool {
        matches!(self, Cell::Wall | Cell::Turret | Cell::Core)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Raider,
    Grunt,
    Tank,
    Boss,
}

impl EnemyKind {
    /// (hp, speed px/s, radius, core damage, scrap, xp)
    pub fn stats(self) -> (f32, f32, f32, f32, u32, u32) {
        match self {
            EnemyKind::Raider => (14.0, 70.0, 6.0, 4.0, 1, 1),
            EnemyKind::Grunt => (30.0, 45.0, 8.0, 6.0, 2, 2),
            EnemyKind::Tank => (110.0, 28.0, 11.0, 15.0, 5, 5),
            EnemyKind::Boss => (900.0, 22.0, 16.0, 60.0, 40, 30),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub hurt_flash: f32,
}

#[derive(Clone, Debug)]
pub struct Turret {
    pub tx: usize,
    pub ty: usize,
    pub level: u32,
    pub cooldown: f32,
    pub aim: f32,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub dmg: f32,
    pub life: f32,
    pub from_turret: bool,
}

#[derive(Clone, Debug)]
pub struct Orb {
    pub x: f32,
    pub y: f32,
    pub value: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    TurretDamage,
    TurretRate,
    TurretRange,
    GunDamage,
    GunRate,
    Speed,
    RepairCore,
    MaxHp,
    Magnet,
    Salvage,
    CheapTurrets,
}

pub const ALL_UPGRADES: [Upgrade; 11] = [
    Upgrade::TurretDamage,
    Upgrade::TurretRate,
    Upgrade::TurretRange,
    Upgrade::GunDamage,
    Upgrade::GunRate,
    Upgrade::Speed,
    Upgrade::RepairCore,
    Upgrade::MaxHp,
    Upgrade::Magnet,
    Upgrade::Salvage,
    Upgrade::CheapTurrets,
];

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::TurretDamage => "Heavier Rounds",
            Upgrade::TurretRate => "Autoloaders",
            Upgrade::TurretRange => "Rangefinders",
            Upgrade::GunDamage => "Commander Cannon",
            Upgrade::GunRate => "Rapid Fire",
            Upgrade::Speed => "Servo Legs",
            Upgrade::RepairCore => "Core Repair",
            Upgrade::MaxHp => "Armor Plating",
            Upgrade::Magnet => "Tractor Field",
            Upgrade::Salvage => "Salvage Rigs",
            Upgrade::CheapTurrets => "Prefab Turrets",
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            Upgrade::TurretDamage => "Turret damage +25%",
            Upgrade::TurretRate => "Turret fire rate +20%",
            Upgrade::TurretRange => "Turret range +15%",
            Upgrade::GunDamage => "Commander gun damage +35%",
            Upgrade::GunRate => "Commander fire rate +25%",
            Upgrade::Speed => "Move speed +12%",
            Upgrade::RepairCore => "Repair core +40, heal fully",
            Upgrade::MaxHp => "Max HP +25, heal fully",
            Upgrade::Magnet => "Pickup radius +50%",
            Upgrade::Salvage => "Scrap from kills +30%",
            Upgrade::CheapTurrets => "Turret cost -20%",
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub mx: f32,
    pub my: f32,
    pub lay_wall: bool,
    /// Left click on a tile: build a turret, or upgrade one already there.
    pub build: Option<(usize, usize)>,
    pub demolish: Option<(usize, usize)>,
    pub pick: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    Shot,
    TurretShot,
    Hit,
    Kill { x: f32, y: f32, big: bool },
    Pickup,
    LevelUp,
    WallLaid,
    WallSolid,
    TurretBuilt,
    TurretUpgraded,
    Demolished,
    Denied,
    WaveStart(u32),
    CoreHit,
    PlayerHurt,
    Won,
    Lost,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Playing,
    LevelUp,
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub turret_dmg: f32,
    pub turret_rate: f32,
    pub turret_range: f32,
    pub gun_dmg: f32,
    pub gun_rate: f32,
    pub speed: f32,
    pub magnet: f32,
    pub salvage: f32,
    pub turret_cost_mul: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            turret_dmg: 1.0,
            turret_rate: 1.0,
            turret_range: 1.0,
            gun_dmg: 1.0,
            gun_rate: 1.0,
            speed: 1.0,
            magnet: 1.0,
            salvage: 1.0,
            turret_cost_mul: 1.0,
        }
    }
}

pub struct Game {
    pub cells: Vec<Cell>,
    pub pending: Vec<f32>,
    pub dist: Vec<u32>,
    pub px: f32,
    pub py: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub core_hp: f32,
    pub core_max: f32,
    pub scrap: u32,
    pub xp: u32,
    pub level: u32,
    pub wave: u32,
    pub to_spawn: Vec<EnemyKind>,
    pub spawn_timer: f32,
    pub break_timer: f32,
    pub enemies: Vec<Enemy>,
    pub turrets: Vec<Turret>,
    pub bullets: Vec<Bullet>,
    pub orbs: Vec<Orb>,
    pub stats: Stats,
    pub gun_cd: f32,
    pub hurt_cd: f32,
    pub phase: Phase,
    pub choices: Vec<Upgrade>,
    pub pending_levels: u32,
    pub kills: u32,
    pub time: f32,
    rng: Rng,
}

pub fn idx(tx: usize, ty: usize) -> usize {
    ty * W + tx
}

pub fn tile_of(x: f32, y: f32) -> Option<(usize, usize)> {
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let (tx, ty) = ((x / TILE) as usize, (y / TILE) as usize);
    (tx < W && ty < H).then_some((tx, ty))
}

pub fn center(tx: usize, ty: usize) -> (f32, f32) {
    ((tx as f32 + 0.5) * TILE, (ty as f32 + 0.5) * TILE)
}

/// Spawn gates: three-tile openings in the middle of each map edge.
pub fn gates() -> Vec<(usize, usize)> {
    let mut g = Vec::new();
    for d in 0..3 {
        g.push((W / 2 - 1 + d, 0));
        g.push((W / 2 - 1 + d, H - 1));
        g.push((0, H / 2 - 1 + d));
        g.push((W - 1, H / 2 - 1 + d));
    }
    g
}

pub fn is_gate(tx: usize, ty: usize) -> bool {
    gates().contains(&(tx, ty))
}

/// Breadth-first distance from the core over non-blocking tiles.
pub fn flow_field(cells: &[Cell]) -> Vec<u32> {
    let mut dist = vec![UNREACHABLE; W * H];
    let mut q = VecDeque::new();
    dist[idx(CORE.0, CORE.1)] = 0;
    q.push_back(CORE);
    while let Some((x, y)) = q.pop_front() {
        let d = dist[idx(x, y)];
        for (nx, ny) in neighbours(x, y) {
            let i = idx(nx, ny);
            if dist[i] == UNREACHABLE && !cells[i].blocks() {
                dist[i] = d + 1;
                q.push_back((nx, ny));
            }
        }
    }
    dist
}

fn neighbours(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .filter_map(move |(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            (nx >= 0 && ny >= 0 && (nx as usize) < W && (ny as usize) < H)
                .then_some((nx as usize, ny as usize))
        })
}

/// Enemy roster for a wave: more and tougher enemies as waves climb, with a
/// boss every fifth wave.
pub fn wave_roster(wave: u32) -> Vec<EnemyKind> {
    let mut r = Vec::new();
    let n = 8 + wave * 4 + wave * wave / 3;
    for i in 0..n {
        let k = if wave >= 3 && i % 7 == 6 {
            EnemyKind::Tank
        } else if wave >= 2 && i % 3 == 1 {
            EnemyKind::Raider
        } else {
            EnemyKind::Grunt
        };
        r.push(k);
    }
    if wave.is_multiple_of(5) {
        for _ in 0..wave / 5 {
            r.push(EnemyKind::Boss);
        }
    }
    r
}

/// Enemy hp multiplier per wave.
pub fn hp_scale(wave: u32) -> f32 {
    1.0 + (wave.saturating_sub(1)) as f32 * 0.16
}

pub fn xp_for_level(level: u32) -> u32 {
    6 + level * 5
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut cells = vec![Cell::Open; W * H];
        cells[idx(CORE.0, CORE.1)] = Cell::Core;
        let dist = flow_field(&cells);
        let (cx, cy) = center(CORE.0, CORE.1 + 3);
        Game {
            cells,
            pending: vec![0.0; W * H],
            dist,
            px: cx,
            py: cy,
            hp: 100.0,
            max_hp: 100.0,
            core_hp: 200.0,
            core_max: 200.0,
            scrap: 80,
            xp: 0,
            level: 1,
            wave: 0,
            to_spawn: Vec::new(),
            spawn_timer: 0.0,
            break_timer: 8.0,
            enemies: Vec::new(),
            turrets: Vec::new(),
            bullets: Vec::new(),
            orbs: Vec::new(),
            stats: Stats::default(),
            gun_cd: 0.0,
            hurt_cd: 0.0,
            phase: Phase::Playing,
            choices: Vec::new(),
            pending_levels: 0,
            kills: 0,
            time: 0.0,
            rng: Rng::new(seed),
        }
    }

    pub fn cell(&self, tx: usize, ty: usize) -> Cell {
        self.cells[idx(tx, ty)]
    }

    pub fn turret_cost(&self) -> u32 {
        ((TURRET_COST as f32) * self.stats.turret_cost_mul).round() as u32
    }

    pub fn upgrade_cost(level: u32) -> u32 {
        30 * level
    }

    pub fn turret_range(&self, level: u32) -> f32 {
        (95.0 + level as f32 * 20.0) * self.stats.turret_range
    }

    fn turret_damage(&self, level: u32) -> f32 {
        (7.0 + level as f32 * 5.0) * self.stats.turret_dmg
    }

    fn turret_period(&self, level: u32) -> f32 {
        0.55 / (1.0 + 0.3 * (level - 1) as f32) / self.stats.turret_rate
    }

    /// Whether blocking `(tx, ty)` keeps every gate and every enemy connected
    /// to the core. Walls may shape the maze but never seal it.
    pub fn can_block(&self, tx: usize, ty: usize) -> bool {
        let i = idx(tx, ty);
        if self.cells[i].blocks() || is_gate(tx, ty) {
            return false;
        }
        let mut cells = self.cells.clone();
        cells[i] = Cell::Wall;
        let d = flow_field(&cells);
        if gates().iter().any(|&(x, y)| d[idx(x, y)] == UNREACHABLE) {
            return false;
        }
        for e in &self.enemies {
            if let Some((ex, ey)) = tile_of(e.x, e.y)
                && (ex, ey) != (tx, ty)
                && d[idx(ex, ey)] == UNREACHABLE
            {
                return false;
            }
        }
        true
    }

    fn tile_occupied(&self, tx: usize, ty: usize) -> bool {
        let (cx, cy) = center(tx, ty);
        let half = TILE * 0.5;
        let touches =
            |x: f32, y: f32, r: f32| (x - cx).abs() < half + r && (y - cy).abs() < half + r;
        touches(self.px, self.py, COMMANDER_RADIUS - 2.0)
            || self
                .enemies
                .iter()
                .any(|e| touches(e.x, e.y, e.kind.stats().2 * 0.6))
    }

    fn reflow(&mut self) {
        self.dist = flow_field(&self.cells);
    }

    pub fn update(&mut self, dt: f32, input: &Input) -> Vec<Event> {
        let mut ev = Vec::new();
        match self.phase {
            Phase::Won | Phase::Lost => return ev,
            Phase::LevelUp => {
                if let Some(k) = input.pick
                    && k < self.choices.len()
                {
                    let u = self.choices[k];
                    self.apply(u);
                    self.pending_levels -= 1;
                    if self.pending_levels > 0 {
                        self.roll_choices();
                    } else {
                        self.phase = Phase::Playing;
                    }
                }
                return ev;
            }
            Phase::Playing => {}
        }
        self.time += dt;
        self.move_commander(dt, input);
        self.weave(dt, input, &mut ev);
        self.handle_build(input, &mut ev);
        self.waves(dt, &mut ev);
        self.move_enemies(dt, &mut ev);
        self.shoot(dt, &mut ev);
        self.move_bullets(dt, &mut ev);
        self.collect(dt, &mut ev);

        if self.core_hp <= 0.0 || self.hp <= 0.0 {
            self.core_hp = self.core_hp.max(0.0);
            self.hp = self.hp.max(0.0);
            self.phase = Phase::Lost;
            ev.push(Event::Lost);
        } else if self.wave >= WAVES && self.to_spawn.is_empty() && self.enemies.is_empty() {
            self.phase = Phase::Won;
            ev.push(Event::Won);
        }
        ev
    }

    fn move_commander(&mut self, dt: f32, input: &Input) {
        let len = (input.mx * input.mx + input.my * input.my).sqrt();
        if len < 1e-3 {
            return;
        }
        let sp = 130.0 * self.stats.speed * dt / len;
        let r = COMMANDER_RADIUS;
        let solid = |g: &Game, x: f32, y: f32| {
            for (ox, oy) in [(-r, -r), (r, -r), (-r, r), (r, r)] {
                match tile_of(x + ox, y + oy) {
                    None => return true,
                    Some((tx, ty)) => {
                        if g.cell(tx, ty).blocks() {
                            return true;
                        }
                    }
                }
            }
            false
        };
        let nx = self.px + input.mx * sp;
        if !solid(self, nx, self.py) {
            self.px = nx;
        }
        let ny = self.py + input.my * sp;
        if !solid(self, self.px, ny) {
            self.py = ny;
        }
    }

    fn weave(&mut self, dt: f32, input: &Input, ev: &mut Vec<Event>) {
        if input.lay_wall
            && let Some((tx, ty)) = tile_of(self.px, self.py)
        {
            let i = idx(tx, ty);
            if self.cells[i] == Cell::Open && !is_gate(tx, ty) {
                if self.scrap >= WALL_COST {
                    self.scrap -= WALL_COST;
                    self.cells[i] = Cell::Pending;
                    self.pending[i] = WALL_SETTLE;
                    ev.push(Event::WallLaid);
                } else if self.time.fract() < dt {
                    ev.push(Event::Denied);
                }
            }
        }
        let mut changed = false;
        for ty in 0..H {
            for tx in 0..W {
                let i = idx(tx, ty);
                if self.cells[i] != Cell::Pending {
                    continue;
                }
                self.pending[i] -= dt;
                if self.pending[i] > 0.0 || self.tile_occupied(tx, ty) {
                    continue;
                }
                if self.can_block(tx, ty) {
                    self.cells[i] = Cell::Wall;
                    changed = true;
                    ev.push(Event::WallSolid);
                } else {
                    // Would seal the maze: the trail crumbles and refunds.
                    self.cells[i] = Cell::Open;
                    self.scrap += WALL_COST;
                    ev.push(Event::Denied);
                }
            }
        }
        if changed {
            self.reflow();
        }
    }

    fn handle_build(&mut self, input: &Input, ev: &mut Vec<Event>) {
        if let Some((tx, ty)) = input.build {
            if tx >= W || ty >= H {
                return;
            }
            let i = idx(tx, ty);
            match self.cells[i] {
                Cell::Turret => {
                    let t = self
                        .turrets
                        .iter_mut()
                        .find(|t| (t.tx, t.ty) == (tx, ty))
                        .expect("turret cell has turret");
                    let cost = Game::upgrade_cost(t.level);
                    if t.level < MAX_TURRET_LEVEL && self.scrap >= cost {
                        self.scrap -= cost;
                        t.level += 1;
                        ev.push(Event::TurretUpgraded);
                    } else {
                        ev.push(Event::Denied);
                    }
                }
                Cell::Open | Cell::Wall | Cell::Pending => {
                    let cost = self.turret_cost();
                    let was_wall = self.cells[i] == Cell::Wall;
                    let ok = self.scrap >= cost
                        && (was_wall || (!self.tile_occupied(tx, ty) && self.can_block(tx, ty)));
                    if ok {
                        self.scrap -= cost;
                        if self.cells[i] == Cell::Pending {
                            self.scrap += WALL_COST;
                        }
                        self.cells[i] = Cell::Turret;
                        self.turrets.push(Turret {
                            tx,
                            ty,
                            level: 1,
                            cooldown: 0.0,
                            aim: 0.0,
                        });
                        self.reflow();
                        ev.push(Event::TurretBuilt);
                    } else {
                        ev.push(Event::Denied);
                    }
                }
                Cell::Core => ev.push(Event::Denied),
            }
        }
        if let Some((tx, ty)) = input.demolish {
            if tx >= W || ty >= H {
                return;
            }
            let i = idx(tx, ty);
            let refund = match self.cells[i] {
                Cell::Wall | Cell::Pending => WALL_COST,
                Cell::Turret => {
                    let pos = self
                        .turrets
                        .iter()
                        .position(|t| (t.tx, t.ty) == (tx, ty))
                        .expect("turret cell has turret");
                    let t = self.turrets.swap_remove(pos);
                    let spent: u32 =
                        (1..t.level).map(Game::upgrade_cost).sum::<u32>() + self.turret_cost();
                    spent / 2
                }
                _ => return,
            };
            self.cells[i] = Cell::Open;
            self.scrap += refund;
            self.reflow();
            ev.push(Event::Demolished);
        }
    }

    fn waves(&mut self, dt: f32, ev: &mut Vec<Event>) {
        if self.to_spawn.is_empty() && self.enemies.is_empty() && self.wave < WAVES {
            self.break_timer -= dt;
            if self.break_timer <= 0.0 {
                self.wave += 1;
                self.to_spawn = wave_roster(self.wave);
                self.to_spawn.reverse();
                self.spawn_timer = 0.0;
                self.break_timer = 6.0;
                ev.push(Event::WaveStart(self.wave));
            }
        }
        if self.to_spawn.is_empty() {
            return;
        }
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = (0.55 - self.wave as f32 * 0.02).max(0.12);
            let kind = self.to_spawn.pop().expect("non-empty");
            let g = gates();
            let (gx, gy) = g[self.rng.below(g.len())];
            let (x, y) = center(gx, gy);
            let (hp, ..) = kind.stats();
            let hp = hp * hp_scale(self.wave);
            self.enemies.push(Enemy {
                kind,
                x,
                y,
                hp,
                max_hp: hp,
                hurt_flash: 0.0,
            });
        }
    }

    /// Direction an enemy at `(x, y)` should walk: toward the centre of the
    /// neighbouring tile closest to the core.
    pub fn steer(&self, x: f32, y: f32) -> (f32, f32) {
        let (cx, cy) = center(CORE.0, CORE.1);
        let Some((tx, ty)) = tile_of(x, y) else {
            return norm(cx - x, cy - y);
        };
        let here = self.dist[idx(tx, ty)];
        if here == UNREACHABLE || here <= 1 {
            return norm(cx - x, cy - y);
        }
        let mut best = (tx, ty);
        let mut bd = here;
        for (nx, ny) in neighbours(tx, ty) {
            let d = self.dist[idx(nx, ny)];
            if d < bd {
                bd = d;
                best = (nx, ny);
            }
        }
        let (bx, by) = center(best.0, best.1);
        norm(bx - x, by - y)
    }

    fn move_enemies(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let (cx, cy) = center(CORE.0, CORE.1);
        self.hurt_cd -= dt;
        let mut core_dmg = 0.0;
        let mut player_dmg = 0.0;
        for i in 0..self.enemies.len() {
            let (dx, dy) = self.steer(self.enemies[i].x, self.enemies[i].y);
            let e = &mut self.enemies[i];
            let (_, speed, r, dmg, ..) = e.kind.stats();
            e.hurt_flash -= dt;
            e.x += dx * speed * dt;
            e.y += dy * speed * dt;
            if dist(e.x, e.y, cx, cy) < CORE_RADIUS + r {
                core_dmg += dmg;
                e.hp = -1.0;
            } else if dist(e.x, e.y, self.px, self.py) < COMMANDER_RADIUS + r {
                player_dmg += dmg * 0.5;
            }
        }
        // Light separation so swarms spread along corridors.
        let n = self.enemies.len();
        for i in 0..n {
            for j in i + 1..n.min(i + 12) {
                let (a, b) = (&self.enemies[i], &self.enemies[j]);
                let min = a.kind.stats().2 + b.kind.stats().2;
                let (ddx, ddy) = (b.x - a.x, b.y - a.y);
                let d = (ddx * ddx + ddy * ddy).sqrt();
                if d > 0.01 && d < min {
                    let push = (min - d) * 0.25 / d;
                    let (px, py) = (ddx * push, ddy * push);
                    if self.walkable(a.x - px, a.y - py) {
                        self.enemies[i].x -= px;
                        self.enemies[i].y -= py;
                    }
                    let b = &self.enemies[j];
                    if self.walkable(b.x + px, b.y + py) {
                        self.enemies[j].x += px;
                        self.enemies[j].y += py;
                    }
                }
            }
        }
        if core_dmg > 0.0 {
            self.core_hp -= core_dmg;
            ev.push(Event::CoreHit);
        }
        if player_dmg > 0.0 && self.hurt_cd <= 0.0 {
            self.hp -= player_dmg;
            self.hurt_cd = 0.4;
            ev.push(Event::PlayerHurt);
        }
        self.enemies.retain(|e| e.hp > 0.0);
    }

    fn walkable(&self, x: f32, y: f32) -> bool {
        walkable(&self.cells, x, y)
    }

    fn nearest(&self, x: f32, y: f32, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = dist(x, y, e.x, e.y);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn fire(&mut self, x: f32, y: f32, target: usize, dmg: f32, speed: f32, turret: bool) -> f32 {
        let e = &self.enemies[target];
        let (dx, dy) = norm(e.x - x, e.y - y);
        self.bullets.push(Bullet {
            x,
            y,
            vx: dx * speed,
            vy: dy * speed,
            dmg,
            life: 0.9,
            from_turret: turret,
        });
        dy.atan2(dx)
    }

    fn shoot(&mut self, dt: f32, ev: &mut Vec<Event>) {
        self.gun_cd -= dt;
        if self.gun_cd <= 0.0
            && let Some(t) = self.nearest(self.px, self.py, 150.0)
        {
            self.gun_cd = 0.45 / self.stats.gun_rate;
            let dmg = 8.0 * self.stats.gun_dmg;
            self.fire(self.px, self.py, t, dmg, 420.0, false);
            ev.push(Event::Shot);
        }
        for k in 0..self.turrets.len() {
            self.turrets[k].cooldown -= dt;
            if self.turrets[k].cooldown > 0.0 {
                continue;
            }
            let lvl = self.turrets[k].level;
            let (x, y) = center(self.turrets[k].tx, self.turrets[k].ty);
            if let Some(t) = self.nearest(x, y, self.turret_range(lvl)) {
                let dmg = self.turret_damage(lvl);
                self.turrets[k].cooldown = self.turret_period(lvl);
                self.turrets[k].aim = self.fire(x, y, t, dmg, 520.0, true);
                ev.push(Event::TurretShot);
            }
        }
    }

    fn move_bullets(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let mut bullets = std::mem::take(&mut self.bullets);
        let cells = &self.cells;
        let enemies = &mut self.enemies;
        bullets.retain_mut(|b| {
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.life -= dt;
            if b.life <= 0.0 || !b.from_turret && !walkable(cells, b.x, b.y) {
                return false;
            }
            for e in enemies.iter_mut() {
                if dist(b.x, b.y, e.x, e.y) < e.kind.stats().2 + 3.0 {
                    e.hp -= b.dmg;
                    e.hurt_flash = 0.08;
                    ev.push(Event::Hit);
                    return false;
                }
            }
            true
        });
        self.bullets = bullets;
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let (.., scrap, xp) = e.kind.stats();
                let value = ((scrap as f32) * self.stats.salvage).ceil() as u32;
                self.orbs.push(Orb {
                    x: e.x,
                    y: e.y,
                    value: value.max(1) * 1000 + xp,
                });
                self.kills += 1;
                ev.push(Event::Kill {
                    x: e.x,
                    y: e.y,
                    big: matches!(e.kind, EnemyKind::Tank | EnemyKind::Boss),
                });
            } else {
                i += 1;
            }
        }
    }

    fn collect(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let reach = 60.0 * self.stats.magnet;
        let (px, py) = (self.px, self.py);
        let mut gained_scrap = 0;
        let mut gained_xp = 0;
        self.orbs.retain_mut(|o| {
            let d = dist(o.x, o.y, px, py);
            if d < COMMANDER_RADIUS + 4.0 {
                gained_scrap += o.value / 1000;
                gained_xp += o.value % 1000;
                return false;
            }
            if d < reach {
                let (dx, dy) = norm(px - o.x, py - o.y);
                o.x += dx * 260.0 * dt;
                o.y += dy * 260.0 * dt;
            }
            true
        });
        if gained_scrap + gained_xp > 0 {
            ev.push(Event::Pickup);
            self.scrap += gained_scrap;
            self.gain_xp(gained_xp, ev);
        }
    }

    pub fn gain_xp(&mut self, xp: u32, ev: &mut Vec<Event>) {
        self.xp += xp;
        while self.xp >= xp_for_level(self.level) {
            self.xp -= xp_for_level(self.level);
            self.level += 1;
            self.pending_levels += 1;
            ev.push(Event::LevelUp);
        }
        if self.pending_levels > 0 && self.phase == Phase::Playing {
            self.phase = Phase::LevelUp;
            self.roll_choices();
        }
    }

    fn roll_choices(&mut self) {
        let mut pool = ALL_UPGRADES.to_vec();
        self.choices.clear();
        for _ in 0..3 {
            let k = self.rng.below(pool.len());
            self.choices.push(pool.swap_remove(k));
        }
    }

    pub fn apply(&mut self, u: Upgrade) {
        let s = &mut self.stats;
        match u {
            Upgrade::TurretDamage => s.turret_dmg *= 1.25,
            Upgrade::TurretRate => s.turret_rate *= 1.2,
            Upgrade::TurretRange => s.turret_range *= 1.15,
            Upgrade::GunDamage => s.gun_dmg *= 1.35,
            Upgrade::GunRate => s.gun_rate *= 1.25,
            Upgrade::Speed => s.speed *= 1.12,
            Upgrade::RepairCore => {
                self.core_hp = (self.core_hp + 40.0).min(self.core_max);
                self.hp = self.max_hp;
            }
            Upgrade::MaxHp => {
                self.max_hp += 25.0;
                self.hp = self.max_hp;
            }
            Upgrade::Magnet => s.magnet *= 1.5,
            Upgrade::Salvage => s.salvage *= 1.3,
            Upgrade::CheapTurrets => s.turret_cost_mul *= 0.8,
        }
    }

    /// Seconds until the next wave, if the field is clear.
    pub fn next_wave_in(&self) -> Option<f32> {
        (self.to_spawn.is_empty() && self.enemies.is_empty() && self.wave < WAVES)
            .then_some(self.break_timer.max(0.0))
    }
}

fn walkable(cells: &[Cell], x: f32, y: f32) -> bool {
    tile_of(x, y).is_some_and(|(tx, ty)| !cells[idx(tx, ty)].blocks())
}

pub fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

fn norm(x: f32, y: f32) -> (f32, f32) {
    let l = (x * x + y * y).sqrt();
    if l < 1e-4 { (0.0, 0.0) } else { (x / l, y / l) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_line(g: &mut Game, y: usize, xs: std::ops::Range<usize>) {
        for x in xs {
            g.cells[idx(x, y)] = Cell::Wall;
        }
        g.reflow();
    }

    #[test]
    fn flow_field_reaches_gates_on_empty_map() {
        let g = Game::new(1);
        for (x, y) in gates() {
            assert_ne!(g.dist[idx(x, y)], UNREACHABLE);
        }
        assert_eq!(g.dist[idx(CORE.0, CORE.1)], 0);
    }

    #[test]
    fn walls_lengthen_the_path() {
        let mut g = Game::new(1);
        let before = g.dist[idx(W / 2, 0)];
        wall_line(&mut g, 5, 5..35);
        assert!(g.dist[idx(W / 2, 0)] > before + 10);
    }

    #[test]
    fn cannot_seal_the_core() {
        let mut g = Game::new(1);
        let (cx, cy) = CORE;
        for (x, y) in [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1)] {
            g.cells[idx(x, y)] = Cell::Wall;
        }
        g.reflow();
        assert!(!g.can_block(cx, cy + 1));
        assert!(g.can_block(cx + 3, cy + 3));
    }

    #[test]
    fn gates_cannot_be_blocked() {
        let g = Game::new(1);
        let (x, y) = gates()[0];
        assert!(!g.can_block(x, y));
    }

    #[test]
    fn trail_costs_scrap_and_solidifies() {
        let mut g = Game::new(1);
        let start = g.scrap;
        let (tx, ty) = tile_of(g.px, g.py).unwrap();
        let input = Input {
            lay_wall: true,
            ..Default::default()
        };
        g.update(0.016, &input);
        assert_eq!(g.cell(tx, ty), Cell::Pending);
        assert_eq!(g.scrap, start - WALL_COST);
        // Walk away so the tile is clear, then let it settle.
        g.px += TILE * 2.0;
        for _ in 0..60 {
            g.update(0.016, &Input::default());
        }
        assert_eq!(g.cell(tx, ty), Cell::Wall);
    }

    #[test]
    fn trail_waits_while_commander_stands_on_it() {
        let mut g = Game::new(1);
        let (tx, ty) = tile_of(g.px, g.py).unwrap();
        g.cells[idx(tx, ty)] = Cell::Pending;
        for _ in 0..60 {
            g.update(0.016, &Input::default());
        }
        assert_eq!(g.cell(tx, ty), Cell::Pending);
    }

    #[test]
    fn build_upgrade_and_demolish_turret() {
        let mut g = Game::new(1);
        g.scrap = 200;
        let t = (3, 3);
        let ev = g.update(
            0.0,
            &Input {
                build: Some(t),
                ..Default::default()
            },
        );
        assert!(ev.contains(&Event::TurretBuilt));
        assert_eq!(g.cell(3, 3), Cell::Turret);
        assert_eq!(g.scrap, 200 - TURRET_COST);
        let ev = g.update(
            0.0,
            &Input {
                build: Some(t),
                ..Default::default()
            },
        );
        assert!(ev.contains(&Event::TurretUpgraded));
        assert_eq!(g.turrets[0].level, 2);
        let before = g.scrap;
        g.update(
            0.0,
            &Input {
                demolish: Some(t),
                ..Default::default()
            },
        );
        assert_eq!(g.cell(3, 3), Cell::Open);
        assert_eq!(g.scrap, before + (TURRET_COST + 30) / 2);
    }

    #[test]
    fn building_without_scrap_is_denied() {
        let mut g = Game::new(1);
        g.scrap = 3;
        let ev = g.update(
            0.0,
            &Input {
                build: Some((3, 3)),
                ..Default::default()
            },
        );
        assert!(ev.contains(&Event::Denied));
        assert_eq!(g.cell(3, 3), Cell::Open);
    }

    #[test]
    fn enemies_follow_the_maze() {
        let mut g = Game::new(1);
        // Wall row just below the top gate with an opening at the far left.
        wall_line(&mut g, 2, 2..W);
        let (x, y) = center(W / 2, 1);
        let (dx, _) = g.steer(x, y);
        assert!(dx < -0.5, "should head left toward the opening, got {dx}");
    }

    #[test]
    fn waves_escalate() {
        assert!(wave_roster(10).len() > wave_roster(1).len());
        assert!(wave_roster(5).contains(&EnemyKind::Boss));
        assert!(!wave_roster(4).contains(&EnemyKind::Boss));
        assert!(hp_scale(10) > hp_scale(1));
    }

    #[test]
    fn first_wave_starts_after_break() {
        let mut g = Game::new(1);
        let mut started = false;
        for _ in 0..600 {
            if g.update(0.016, &Input::default())
                .contains(&Event::WaveStart(1))
            {
                started = true;
                break;
            }
        }
        assert!(started);
    }

    #[test]
    fn level_up_pauses_and_applies_choice() {
        let mut g = Game::new(1);
        let mut ev = Vec::new();
        g.gain_xp(xp_for_level(1), &mut ev);
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        g.update(
            0.016,
            &Input {
                pick: Some(0),
                ..Default::default()
            },
        );
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.level, 2);
    }

    #[test]
    fn core_destroyed_loses() {
        let mut g = Game::new(1);
        g.core_hp = 0.0;
        let ev = g.update(0.016, &Input::default());
        assert!(ev.contains(&Event::Lost));
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn surviving_all_waves_wins() {
        let mut g = Game::new(1);
        g.wave = WAVES;
        let ev = g.update(0.016, &Input::default());
        assert!(ev.contains(&Event::Won));
    }

    #[test]
    fn kills_drop_orbs_that_pay_out() {
        let mut g = Game::new(1);
        g.enemies.push(Enemy {
            kind: EnemyKind::Grunt,
            x: g.px + 40.0,
            y: g.py,
            hp: 0.5,
            max_hp: 30.0,
            hurt_flash: 0.0,
        });
        let start = g.scrap;
        for _ in 0..120 {
            g.update(0.016, &Input::default());
        }
        assert!(g.kills >= 1);
        assert!(g.scrap > start);
    }
}
