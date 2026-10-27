//! Pure game rules for Cargo Cascade: economy, routes, trains, monsters,
//! defences and the day/night cycle. No rendering or audio lives here; the
//! front end feeds input through the methods on [`Game`] and reacts to the
//! [`Event`]s returned by [`Game::update`].

use macroquad::math::{Vec2, vec2};

pub const WORLD_W: f32 = 1600.0;
pub const WORLD_H: f32 = 1000.0;
pub const LAST_NIGHT: u32 = 20;
pub const STOCK_CAP: f32 = 60.0;
pub const SUPPLY_WINDOW: f32 = 45.0;
pub const MAX_TRAINS: usize = 4;
pub const TRACK_BUILD_SPEED: f32 = 160.0;
pub const TURRET_REACH: f32 = 60.0;
pub const START_MONEY: f32 = 800.0;
/// Goods a town must receive before it counts as connected.
pub const TOWN_GOAL: f32 = 100.0;

/// Small deterministic xorshift RNG so the rules stay reproducible in tests.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    pub fn f(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn pick(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n.max(1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cargo {
    Coal,
    Power,
    Goods,
}

impl Cargo {
    /// Base pay per unit delivered.
    pub fn pay(self) -> f32 {
        match self {
            Cargo::Coal => 6.0,
            Cargo::Power => 9.0,
            Cargo::Goods => 14.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Mine,
    Plant,
    Factory,
    Town,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Mine => "Coal Mine",
            Kind::Plant => "Power Plant",
            Kind::Factory => "Factory",
            Kind::Town => "Town",
        }
    }
    pub fn output(self) -> Option<Cargo> {
        match self {
            Kind::Mine => Some(Cargo::Coal),
            Kind::Plant => Some(Cargo::Power),
            Kind::Factory => Some(Cargo::Goods),
            Kind::Town => None,
        }
    }
    pub fn input(self) -> Option<Cargo> {
        match self {
            Kind::Mine => None,
            Kind::Plant => Some(Cargo::Coal),
            Kind::Factory => Some(Cargo::Power),
            Kind::Town => Some(Cargo::Goods),
        }
    }
    fn max_hp(self) -> f32 {
        if self == Kind::Town { 500.0 } else { 120.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Industry {
    pub kind: Kind,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub input: f32,
    pub output: f32,
    /// Seconds left of "recently supplied" status.
    pub supplied: f32,
    /// Total cargo received so far.
    pub received: f32,
    /// Index among industries of the same kind (for labels).
    pub number: u32,
}

impl Industry {
    pub fn wrecked(&self) -> bool {
        self.hp <= 0.0
    }
    /// A town is connected once it has received its goal and is still supplied.
    pub fn connected(&self) -> bool {
        self.kind == Kind::Town && self.received >= TOWN_GOAL && self.supplied > 0.0
    }
    pub fn label(&self) -> String {
        format!("{} {}", self.kind.name(), self.number)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    Kettle,
    Mogul,
    Express,
}

impl Engine {
    pub const ALL: [Engine; 3] = [Engine::Kettle, Engine::Mogul, Engine::Express];
    pub fn name(self) -> &'static str {
        match self {
            Engine::Kettle => "Kettle",
            Engine::Mogul => "Mogul",
            Engine::Express => "Express",
        }
    }
    pub fn cost(self) -> f32 {
        match self {
            Engine::Kettle => 120.0,
            Engine::Mogul => 300.0,
            Engine::Express => 600.0,
        }
    }
    pub fn speed(self) -> f32 {
        match self {
            Engine::Kettle => 70.0,
            Engine::Mogul => 110.0,
            Engine::Express => 170.0,
        }
    }
    pub fn capacity(self) -> f32 {
        match self {
            Engine::Kettle => 8.0,
            Engine::Mogul => 14.0,
            Engine::Express => 22.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TrainState {
    Loading(f32),
    ToB,
    Unloading(f32),
    ToA,
}

#[derive(Clone, Debug)]
pub struct Train {
    pub engine: Engine,
    pub dist: f32,
    pub load: f32,
    pub state: TrainState,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub a: usize,
    pub b: usize,
    pub pa: Vec2,
    pub pb: Vec2,
    pub len: f32,
    pub built: f32,
    pub trains: Vec<Train>,
    pub cargo: Cargo,
    pub earned: f32,
}

impl Route {
    pub fn is_built(&self) -> bool {
        self.built >= self.len
    }
    pub fn point(&self, d: f32) -> Vec2 {
        self.pa + (self.pb - self.pa) * (d / self.len).clamp(0.0, 1.0)
    }
    pub fn dir(&self) -> Vec2 {
        (self.pb - self.pa).normalize_or_zero()
    }
    /// Distance from `p` to the built part of the track.
    pub fn dist_to_built(&self, p: Vec2) -> f32 {
        if self.built <= 0.0 {
            return f32::INFINITY;
        }
        let end = self.point(self.built);
        seg_dist(p, self.pa, end)
    }
}

pub fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[derive(Clone, Debug)]
pub struct Turret {
    pub pos: Vec2,
    pub cooldown: f32,
    pub aim: Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Gremlin,
    Saboteur,
    Brute,
    Behemoth,
}

impl EnemyKind {
    /// (hp, speed, dps, xp, scrap value, radius)
    pub fn stats(self) -> (f32, f32, f32, u32, f32, f32) {
        match self {
            EnemyKind::Gremlin => (18.0, 55.0, 5.0, 1, 2.0, 7.0),
            EnemyKind::Saboteur => (34.0, 40.0, 11.0, 2, 4.0, 9.0),
            EnemyKind::Brute => (120.0, 24.0, 18.0, 5, 9.0, 13.0),
            EnemyKind::Behemoth => (650.0, 17.0, 16.0, 40, 150.0, 24.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub radius: f32,
    pub speed: f32,
    pub dps: f32,
    pub target: Option<usize>,
    pub attacking: bool,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub pos: Vec2,
    pub vel: Vec2,
    pub dmg: f32,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Coin {
    pub pos: Vec2,
    pub value: f32,
    pub t: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Perk {
    AuraPower,
    AuraReach,
    SignalDamage,
    SignalRate,
    SignalRange,
    Freight,
    Steam,
    Fortify,
}

impl Perk {
    pub const ALL: [Perk; 8] = [
        Perk::AuraPower,
        Perk::AuraReach,
        Perk::SignalDamage,
        Perk::SignalRate,
        Perk::SignalRange,
        Perk::Freight,
        Perk::Steam,
        Perk::Fortify,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Perk::AuraPower => "Brighter Lamps",
            Perk::AuraReach => "Wider Glow",
            Perk::SignalDamage => "Heavy Shot",
            Perk::SignalRate => "Quick Levers",
            Perk::SignalRange => "Tall Masts",
            Perk::Freight => "Freight Contracts",
            Perk::Steam => "Superheated Steam",
            Perk::Fortify => "Fortify & Repair",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Perk::AuraPower => "Town aura damage +30%",
            Perk::AuraReach => "Town aura radius +25",
            Perk::SignalDamage => "Signal box damage +35%",
            Perk::SignalRate => "Signal box fire rate +25%",
            Perk::SignalRange => "Signal box range +25",
            Perk::Freight => "Delivery income +20%",
            Perk::Steam => "All trains +15% speed",
            Perk::Fortify => "Industry HP +25%, heal all",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Perks {
    pub aura_mult: f32,
    pub aura_reach: f32,
    pub signal_dmg: f32,
    pub signal_rate: f32,
    pub signal_range: f32,
    pub income: f32,
    pub train_speed: f32,
    pub hp_mult: f32,
}

impl Default for Perks {
    fn default() -> Self {
        Self {
            aura_mult: 1.0,
            aura_reach: 0.0,
            signal_dmg: 1.0,
            signal_rate: 1.0,
            signal_range: 0.0,
            income: 1.0,
            train_speed: 1.0,
            hp_mult: 1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Day,
    Night,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Victory,
    TownLost(usize),
    Deadline,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    Shoot(Vec2),
    Hit(Vec2),
    Kill(Vec2, EnemyKind),
    Pickup(Vec2, f32),
    Delivered(Vec2, f32),
    TrackDone(usize),
    Wrecked(usize),
    LevelUp,
    NightFall(u32),
    Dawn(u32),
    TownConnected(usize),
    GameOver,
    Victory,
}

pub struct Game {
    pub industries: Vec<Industry>,
    pub routes: Vec<Route>,
    pub turrets: Vec<Turret>,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub coins: Vec<Coin>,
    pub money: f32,
    pub phase: Phase,
    pub phase_t: f32,
    pub phase_len: f32,
    /// Number of the current (or most recent) night; 0 before the first night.
    pub night: u32,
    pub xp: u32,
    pub level: u32,
    pub pending_levels: u32,
    pub perk_choices: Option<[Perk; 3]>,
    pub perks: Perks,
    pub outcome: Option<Outcome>,
    pub kills: u32,
    pub earned: f32,
    pub time: f32,
    rng: Rng,
    spawn_acc: f32,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let industries = generate_map(&mut rng);
        Self {
            industries,
            routes: Vec::new(),
            turrets: Vec::new(),
            enemies: Vec::new(),
            projectiles: Vec::new(),
            coins: Vec::new(),
            money: START_MONEY,
            phase: Phase::Day,
            phase_t: 55.0,
            phase_len: 55.0,
            night: 0,
            xp: 0,
            level: 1,
            pending_levels: 0,
            perk_choices: None,
            perks: Perks::default(),
            outcome: None,
            kills: 0,
            earned: 0.0,
            time: 0.0,
            rng,
            spawn_acc: 0.0,
        }
    }

    pub fn towns(&self) -> impl Iterator<Item = (usize, &Industry)> {
        self.industries
            .iter()
            .enumerate()
            .filter(|(_, i)| i.kind == Kind::Town)
    }

    pub fn connected_towns(&self) -> (usize, usize) {
        let total = self.towns().count();
        let done = self.towns().filter(|(_, t)| t.connected()).count();
        (done, total)
    }

    pub fn xp_needed(&self) -> u32 {
        8 + self.level * 7
    }

    pub fn industry_at(&self, p: Vec2, radius: f32) -> Option<usize> {
        self.industries
            .iter()
            .enumerate()
            .filter(|(_, i)| i.pos.distance(p) <= radius)
            .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
            .map(|(k, _)| k)
    }

    pub fn route_cost(&self, a: usize, b: usize) -> f32 {
        let len = self.industries[a].pos.distance(self.industries[b].pos);
        (40.0 + len * 0.45).round()
    }

    /// Validate a route from producer `a` to consumer `b`.
    pub fn check_route(&self, a: usize, b: usize) -> Result<(), &'static str> {
        let (ia, ib) = (&self.industries[a], &self.industries[b]);
        let Some(out) = ia.kind.output() else {
            return Err("Towns only receive cargo");
        };
        if ib.kind.input() != Some(out) {
            return Err(match out {
                Cargo::Coal => "Coal goes to a Power Plant",
                Cargo::Power => "Power goes to a Factory",
                Cargo::Goods => "Goods go to a Town",
            });
        }
        if self.routes.iter().any(|r| r.a == a && r.b == b) {
            return Err("Route already exists");
        }
        if self.money < self.route_cost(a, b) {
            return Err("Not enough money for track");
        }
        Ok(())
    }

    pub fn create_route(&mut self, a: usize, b: usize) -> Result<usize, &'static str> {
        self.check_route(a, b)?;
        let cost = self.route_cost(a, b);
        self.money -= cost;
        let (pa, pb) = (self.industries[a].pos, self.industries[b].pos);
        let cargo = self.industries[a].kind.output().expect("checked");
        self.routes.push(Route {
            a,
            b,
            pa,
            pb,
            len: pa.distance(pb).max(1.0),
            built: 0.0,
            trains: Vec::new(),
            cargo,
            earned: 0.0,
        });
        Ok(self.routes.len() - 1)
    }

    pub fn remove_route(&mut self, r: usize) {
        if r < self.routes.len() {
            let refund: f32 = self.routes[r]
                .trains
                .iter()
                .map(|t| t.engine.cost() * 0.5)
                .sum();
            self.money += refund;
            self.routes.remove(r);
        }
    }

    pub fn add_train(&mut self, r: usize, engine: Engine) -> Result<(), &'static str> {
        let Some(route) = self.routes.get(r) else {
            return Err("No such route");
        };
        if route.trains.len() >= MAX_TRAINS {
            return Err("Route is full (4 trains)");
        }
        if self.money < engine.cost() {
            return Err("Not enough money for engine");
        }
        self.money -= engine.cost();
        let wait = 0.5 + 0.7 * self.routes[r].trains.len() as f32;
        self.routes[r].trains.push(Train {
            engine,
            dist: 0.0,
            load: 0.0,
            state: TrainState::Loading(wait),
        });
        Ok(())
    }

    /// Replace the slowest train on the route with `engine` (half refund).
    pub fn upgrade_train(&mut self, r: usize, engine: Engine) -> Result<(), &'static str> {
        let Some(route) = self.routes.get(r) else {
            return Err("No such route");
        };
        let Some((k, old)) = route
            .trains
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.engine.speed().total_cmp(&b.1.engine.speed()))
            .map(|(k, t)| (k, t.engine))
        else {
            return Err("No train to upgrade");
        };
        if old.speed() >= engine.speed() {
            return Err("Pick a faster engine (2/3)");
        }
        let price = engine.cost() - old.cost() * 0.5;
        if self.money < price {
            return Err("Not enough money for engine");
        }
        self.money -= price;
        self.routes[r].trains[k].engine = engine;
        Ok(())
    }

    pub fn turret_cost(&self) -> f32 {
        150.0 + 30.0 * self.turrets.len() as f32
    }

    pub fn turret_range(&self) -> f32 {
        135.0 + self.perks.signal_range
    }

    pub fn check_turret(&self, p: Vec2) -> Result<(), &'static str> {
        let near_track = self
            .routes
            .iter()
            .any(|r| r.dist_to_built(p) <= TURRET_REACH);
        let near_ind = self
            .industries
            .iter()
            .any(|i| i.pos.distance(p) <= TURRET_REACH + 20.0);
        if !near_track && !near_ind {
            return Err("Signal boxes go beside track or industry");
        }
        if self.turrets.iter().any(|t| t.pos.distance(p) < 26.0) {
            return Err("Too close to another signal box");
        }
        if self.industries.iter().any(|i| i.pos.distance(p) < 26.0) {
            return Err("Too close to an industry");
        }
        if self.money < self.turret_cost() {
            return Err("Not enough money for signal box");
        }
        Ok(())
    }

    pub fn place_turret(&mut self, p: Vec2) -> Result<(), &'static str> {
        self.check_turret(p)?;
        self.money -= self.turret_cost();
        self.turrets.push(Turret {
            pos: p,
            cooldown: 0.0,
            aim: vec2(0.0, -1.0),
        });
        Ok(())
    }

    pub fn repair_cost(&self, i: usize) -> f32 {
        let ind = &self.industries[i];
        let missing = (ind.max_hp - ind.hp.max(0.0)).max(0.0);
        let wreck_fee = if ind.wrecked() { 60.0 } else { 0.0 };
        (missing * 0.8 + wreck_fee).round()
    }

    pub fn repair(&mut self, i: usize) -> Result<(), &'static str> {
        let cost = self.repair_cost(i);
        if cost <= 0.0 {
            return Err("Nothing to repair");
        }
        if self.money < cost {
            return Err("Not enough money to repair");
        }
        self.money -= cost;
        let ind = &mut self.industries[i];
        ind.hp = ind.max_hp;
        Ok(())
    }

    pub fn choose_perk(&mut self, k: usize) -> Option<Perk> {
        let choices = self.perk_choices?;
        let perk = *choices.get(k)?;
        let p = &mut self.perks;
        match perk {
            Perk::AuraPower => p.aura_mult *= 1.3,
            Perk::AuraReach => p.aura_reach += 25.0,
            Perk::SignalDamage => p.signal_dmg *= 1.35,
            Perk::SignalRate => p.signal_rate *= 1.25,
            Perk::SignalRange => p.signal_range += 25.0,
            Perk::Freight => p.income *= 1.2,
            Perk::Steam => p.train_speed *= 1.15,
            Perk::Fortify => {
                p.hp_mult *= 1.25;
                let m = p.hp_mult;
                for ind in &mut self.industries {
                    ind.max_hp = ind.kind.max_hp() * m;
                    ind.hp = ind.max_hp;
                }
            }
        }
        self.perk_choices = None;
        self.pending_levels = self.pending_levels.saturating_sub(1);
        Some(perk)
    }

    /// Radius and damage per second of a town's defensive aura.
    pub fn aura(&self, town: usize) -> (f32, f32) {
        let t = &self.industries[town];
        if t.kind != Kind::Town || t.wrecked() {
            return (0.0, 0.0);
        }
        if t.supplied <= 0.0 {
            return (95.0, 11.0);
        }
        let feeders = self
            .routes
            .iter()
            .filter(|r| {
                r.b == town && r.is_built() && {
                    let f = &self.industries[r.a];
                    !f.wrecked() && f.supplied > 0.0
                }
            })
            .count()
            .max(1) as f32;
        (
            140.0 + 12.0 * (feeders - 1.0) + self.perks.aura_reach,
            (22.0 + 9.0 * (feeders - 1.0)) * self.perks.aura_mult,
        )
    }

    fn add_xp(&mut self, xp: u32) {
        self.xp += xp;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.pending_levels += 1;
        }
    }

    fn roll_perks(&mut self) -> [Perk; 3] {
        let mut pool = Perk::ALL.to_vec();
        let mut out = [Perk::AuraPower; 3];
        for slot in &mut out {
            let k = self.rng.pick(pool.len());
            *slot = pool.swap_remove(k);
        }
        out
    }

    fn hp_scale(&self) -> f32 {
        let n = self.night.saturating_sub(1) as f32;
        1.0 + 0.12 * n + 0.012 * n * n
    }

    fn spawn(&mut self, kind: EnemyKind) {
        let (hp, speed, dps, _, _, radius) = kind.stats();
        let hp = hp * self.hp_scale();
        let side = self.rng.pick(4);
        let pos = match side {
            0 => vec2(self.rng.range(0.0, WORLD_W), -20.0),
            1 => vec2(self.rng.range(0.0, WORLD_W), WORLD_H + 20.0),
            2 => vec2(-20.0, self.rng.range(0.0, WORLD_H)),
            _ => vec2(WORLD_W + 20.0, self.rng.range(0.0, WORLD_H)),
        };
        let speed = speed * self.rng.range(0.85, 1.15) * (1.0 + 0.01 * self.night as f32);
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            radius,
            speed,
            dps: dps * (1.0 + 0.05 * self.night as f32),
            target: None,
            attacking: false,
            flash: 0.0,
        });
    }

    fn roll_enemy(&mut self) -> EnemyKind {
        let n = self.night as f32;
        let r = self.rng.f();
        if r < (0.03 * (n - 3.0)).clamp(0.0, 0.22) {
            EnemyKind::Brute
        } else if r < (0.12 + 0.03 * n).min(0.55) {
            EnemyKind::Saboteur
        } else {
            EnemyKind::Gremlin
        }
    }

    fn spawn_rate(&self) -> f32 {
        let n = self.night as f32;
        match self.phase {
            Phase::Night => 0.25 + 0.3 * n + 0.012 * n * n,
            Phase::Day if self.night >= 2 => 0.04 * n,
            Phase::Day => 0.0,
        }
    }

    fn choose_target(&self, e: &Enemy) -> Option<usize> {
        let towns_only = matches!(e.kind, EnemyKind::Brute | EnemyKind::Behemoth);
        let avoid_towns = e.kind == EnemyKind::Saboteur;
        let best = |filter: &dyn Fn(&Industry) -> bool| {
            self.industries
                .iter()
                .enumerate()
                .filter(|(_, i)| !i.wrecked() && filter(i))
                .min_by(|a, b| {
                    a.1.pos
                        .distance_squared(e.pos)
                        .total_cmp(&b.1.pos.distance_squared(e.pos))
                })
                .map(|(k, _)| k)
        };
        if towns_only {
            best(&|i| i.kind == Kind::Town)
        } else if avoid_towns {
            best(&|i| i.kind != Kind::Town).or_else(|| best(&|_| true))
        } else {
            best(&|_| true)
        }
    }

    /// Advance the simulation by `dt` seconds.
    pub fn update(&mut self, dt: f32) -> Vec<Event> {
        let mut ev = Vec::new();
        if self.outcome.is_some() {
            return ev;
        }
        if self.perk_choices.is_none() && self.pending_levels > 0 {
            self.perk_choices = Some(self.roll_perks());
            ev.push(Event::LevelUp);
        }
        if self.perk_choices.is_some() {
            return ev;
        }
        self.time += dt;
        self.step_phase(dt, &mut ev);
        if self.outcome.is_some() {
            return ev;
        }
        self.step_industries(dt);
        self.step_routes(dt, &mut ev);
        self.step_spawns(dt);
        self.step_enemies(dt, &mut ev);
        self.step_defence(dt, &mut ev);
        self.step_coins(dt, &mut ev);
        self.check_outcome(&mut ev);
        ev
    }

    fn step_phase(&mut self, dt: f32, ev: &mut Vec<Event>) {
        self.phase_t -= dt;
        if self.phase_t > 0.0 {
            return;
        }
        match self.phase {
            Phase::Day => {
                self.night += 1;
                self.phase = Phase::Night;
                self.phase_len = 30.0 + 1.5 * self.night as f32;
                self.phase_t = self.phase_len;
                if self.night.is_multiple_of(5) {
                    for _ in 0..self.night / 5 {
                        self.spawn(EnemyKind::Behemoth);
                    }
                }
                ev.push(Event::NightFall(self.night));
            }
            Phase::Night => {
                if self.night >= LAST_NIGHT {
                    self.outcome = Some(Outcome::Deadline);
                    ev.push(Event::GameOver);
                    return;
                }
                self.phase = Phase::Day;
                self.phase_len = 40.0;
                self.phase_t = self.phase_len;
                let bonus = 100.0 + 25.0 * self.night as f32;
                self.money += bonus;
                ev.push(Event::Dawn(self.night));
            }
        }
    }

    fn step_industries(&mut self, dt: f32) {
        for ind in &mut self.industries {
            ind.supplied = (ind.supplied - dt).max(0.0);
            if ind.wrecked() {
                ind.input = 0.0;
                ind.output = 0.0;
                continue;
            }
            match ind.kind {
                Kind::Mine => ind.output = (ind.output + 1.3 * dt).min(STOCK_CAP),
                Kind::Plant | Kind::Factory => {
                    let c = ind.input.min(1.6 * dt).min(STOCK_CAP - ind.output);
                    ind.input -= c;
                    ind.output += c;
                }
                Kind::Town => {
                    ind.input = (ind.input - 0.5 * dt).max(0.0);
                    // Towns slowly heal, faster when supplied.
                    let heal = if ind.supplied > 0.0 { 3.0 } else { 1.0 };
                    ind.hp = (ind.hp + heal * dt).min(ind.max_hp);
                }
            }
        }
    }

    fn step_routes(&mut self, dt: f32, ev: &mut Vec<Event>) {
        let day = self.phase == Phase::Day;
        let speed_mult = self.perks.train_speed;
        let income = self.perks.income;
        for (ri, route) in self.routes.iter_mut().enumerate() {
            if !route.is_built() {
                if day {
                    route.built = (route.built + TRACK_BUILD_SPEED * dt).min(route.len);
                    if route.is_built() {
                        ev.push(Event::TrackDone(ri));
                    }
                }
                continue;
            }
            for train in &mut route.trains {
                let v = train.engine.speed() * speed_mult;
                match train.state {
                    TrainState::Loading(t) => {
                        let t = t - dt;
                        if t <= 0.0 {
                            let src = &mut self.industries[route.a];
                            let amt = if src.wrecked() {
                                0.0
                            } else {
                                src.output.min(train.engine.capacity())
                            };
                            src.output -= amt;
                            train.load = amt;
                            train.state = TrainState::ToB;
                        } else {
                            train.state = TrainState::Loading(t);
                        }
                    }
                    TrainState::ToB => {
                        train.dist += v * dt;
                        if train.dist >= route.len {
                            train.dist = route.len;
                            let was_connected = self.industries[route.b].connected();
                            let dst = &mut self.industries[route.b];
                            if !dst.wrecked() && train.load > 0.0 {
                                let space = if dst.kind == Kind::Town {
                                    f32::INFINITY
                                } else {
                                    (STOCK_CAP - dst.input).max(0.0)
                                };
                                let amt = train.load.min(space);
                                dst.input += amt;
                                if amt > 0.0 {
                                    dst.supplied = SUPPLY_WINDOW;
                                    dst.received += amt;
                                    let pay = amt
                                        * route.cargo.pay()
                                        * (1.0 + route.len / 450.0)
                                        * income;
                                    self.money += pay;
                                    self.earned += pay;
                                    route.earned += pay;
                                    ev.push(Event::Delivered(dst.pos, pay));
                                    if dst.kind == Kind::Town && !was_connected && dst.connected() {
                                        ev.push(Event::TownConnected(route.b));
                                    }
                                }
                            }
                            train.load = 0.0;
                            train.state = TrainState::Unloading(1.0);
                        }
                    }
                    TrainState::Unloading(t) => {
                        let t = t - dt;
                        train.state = if t <= 0.0 {
                            TrainState::ToA
                        } else {
                            TrainState::Unloading(t)
                        };
                    }
                    TrainState::ToA => {
                        train.dist -= v * dt;
                        if train.dist <= 0.0 {
                            train.dist = 0.0;
                            train.state = TrainState::Loading(1.0);
                        }
                    }
                }
            }
        }
    }

    fn step_spawns(&mut self, dt: f32) {
        self.spawn_acc += self.spawn_rate() * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let k = self.roll_enemy();
            self.spawn(k);
        }
    }

    fn step_enemies(&mut self, dt: f32, ev: &mut Vec<Event>) {
        for k in 0..self.enemies.len() {
            let needs = match self.enemies[k].target {
                None => true,
                Some(t) => self.industries[t].wrecked(),
            };
            if needs {
                let t = self.choose_target(&self.enemies[k]);
                self.enemies[k].target = t;
            }
            let e = &mut self.enemies[k];
            e.flash = (e.flash - dt).max(0.0);
            e.attacking = false;
            let Some(t) = e.target else { continue };
            let ind = &mut self.industries[t];
            let to = ind.pos - e.pos;
            let d = to.length();
            if d > 20.0 + e.radius * 0.5 {
                e.pos += to / d * e.speed * dt;
            } else {
                e.attacking = true;
                let before = ind.hp;
                ind.hp -= e.dps * dt;
                if before > 0.0 && ind.hp <= 0.0 {
                    ind.hp = 0.0;
                    ev.push(Event::Wrecked(t));
                }
            }
        }
    }

    fn step_defence(&mut self, dt: f32, ev: &mut Vec<Event>) {
        // Town auras.
        let auras: Vec<(Vec2, f32, f32)> = self
            .towns()
            .map(|(i, t)| (i, t.pos))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(i, p)| {
                let (r, d) = self.aura(i);
                (p, r, d)
            })
            .collect();
        for e in &mut self.enemies {
            for &(p, r, d) in &auras {
                if d > 0.0 && e.pos.distance(p) <= r + e.radius {
                    e.hp -= d * dt;
                }
            }
        }
        // Signal box turrets.
        let range = self.turret_range();
        let dmg = 12.0 * self.perks.signal_dmg;
        let rate = 1.6 * self.perks.signal_rate;
        for tur in &mut self.turrets {
            tur.cooldown -= dt;
            if tur.cooldown > 0.0 {
                continue;
            }
            let target = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(tur.pos) <= range + e.radius)
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(tur.pos)
                        .total_cmp(&b.pos.distance_squared(tur.pos))
                });
            if let Some(e) = target {
                let dir = (e.pos - tur.pos).normalize_or_zero();
                tur.aim = dir;
                tur.cooldown = 1.0 / rate;
                self.projectiles.push(Projectile {
                    pos: tur.pos + dir * 8.0,
                    vel: dir * 480.0,
                    dmg,
                    life: (range + 40.0) / 480.0,
                });
                ev.push(Event::Shoot(tur.pos));
            } else {
                tur.cooldown = 0.0;
            }
        }
        for p in &mut self.projectiles {
            let from = p.pos;
            p.pos += p.vel * dt;
            p.life -= dt;
            if let Some(e) = self
                .enemies
                .iter_mut()
                .find(|e| e.hp > 0.0 && seg_dist(e.pos, from, p.pos) <= e.radius + 5.0)
            {
                e.hp -= p.dmg;
                e.flash = 0.1;
                p.life = 0.0;
                ev.push(Event::Hit(p.pos));
            }
        }
        self.projectiles.retain(|p| p.life > 0.0);
        // Deaths.
        let mut xp = 0;
        let mut k = 0;
        while k < self.enemies.len() {
            if self.enemies[k].hp <= 0.0 {
                let e = self.enemies.swap_remove(k);
                let (_, _, _, exp, value, _) = e.kind.stats();
                xp += exp;
                self.kills += 1;
                self.coins.push(Coin {
                    pos: e.pos,
                    value,
                    t: 0.0,
                });
                ev.push(Event::Kill(e.pos, e.kind));
            } else {
                k += 1;
            }
        }
        if xp > 0 {
            self.add_xp(xp);
        }
    }

    fn step_coins(&mut self, dt: f32, ev: &mut Vec<Event>) {
        for c in &mut self.coins {
            c.t += dt;
            c.pos.y -= 30.0 * dt;
        }
        let mut gained = 0.0;
        self.coins.retain(|c| {
            if c.t >= 0.8 {
                gained += c.value;
                ev.push(Event::Pickup(c.pos, c.value));
                false
            } else {
                true
            }
        });
        self.money += gained;
    }

    fn check_outcome(&mut self, ev: &mut Vec<Event>) {
        let lost = self.towns().find(|(_, t)| t.wrecked()).map(|(i, _)| i);
        if let Some(i) = lost {
            self.outcome = Some(Outcome::TownLost(i));
            ev.push(Event::GameOver);
            return;
        }
        let (done, total) = self.connected_towns();
        if total > 0 && done == total {
            self.outcome = Some(Outcome::Victory);
            ev.push(Event::Victory);
        }
    }
}

/// Lay out towns, factories, plants and mines with a minimum spacing.
pub fn generate_map(rng: &mut Rng) -> Vec<Industry> {
    let plan = [
        (Kind::Town, 4),
        (Kind::Factory, 3),
        (Kind::Plant, 3),
        (Kind::Mine, 5),
    ];
    let mut out: Vec<Industry> = Vec::new();
    for (kind, n) in plan {
        for number in 1..=n {
            let mut min_d = 170.0;
            let pos = loop {
                let mut found = None;
                for _ in 0..400 {
                    // Towns sit a bit inward so monsters need time to reach them.
                    let m = if kind == Kind::Town { 220.0 } else { 90.0 };
                    let p = vec2(
                        rng.range(m, WORLD_W - m),
                        rng.range(m * 0.7, WORLD_H - m * 0.7),
                    );
                    if out.iter().all(|i| i.pos.distance(p) >= min_d) {
                        found = Some(p);
                        break;
                    }
                }
                if let Some(p) = found {
                    break p;
                }
                min_d *= 0.85;
            };
            let hp = kind.max_hp();
            out.push(Industry {
                kind,
                pos,
                hp,
                max_hp: hp,
                input: 0.0,
                output: if kind == Kind::Mine { 10.0 } else { 0.0 },
                supplied: 0.0,
                received: 0.0,
                number,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(g: &Game, k: Kind) -> usize {
        g.industries.iter().position(|i| i.kind == k).unwrap()
    }

    fn run(g: &mut Game, secs: f32) -> Vec<Event> {
        let mut all = Vec::new();
        let steps = (secs / 0.05) as usize;
        for _ in 0..steps {
            all.extend(g.update(0.05));
            while g.perk_choices.is_some() {
                g.choose_perk(0);
            }
        }
        all
    }

    #[test]
    fn map_has_all_industries_spaced() {
        let g = Game::new(7);
        assert_eq!(g.towns().count(), 4);
        assert_eq!(g.industries.len(), 15);
        for (i, a) in g.industries.iter().enumerate() {
            assert!(a.pos.x > 0.0 && a.pos.x < WORLD_W && a.pos.y > 0.0 && a.pos.y < WORLD_H);
            for b in &g.industries[i + 1..] {
                assert!(a.pos.distance(b.pos) > 60.0);
            }
        }
    }

    #[test]
    fn route_validation_follows_chain() {
        let mut g = Game::new(1);
        g.money = 1e6;
        let (m, p, f, t) = (
            first(&g, Kind::Mine),
            first(&g, Kind::Plant),
            first(&g, Kind::Factory),
            first(&g, Kind::Town),
        );
        assert!(g.check_route(m, f).is_err());
        assert!(g.check_route(t, m).is_err());
        assert!(g.check_route(p, m).is_err());
        assert!(g.create_route(m, p).is_ok());
        assert_eq!(g.create_route(m, p), Err("Route already exists"));
        assert!(g.create_route(p, f).is_ok());
        assert!(g.create_route(f, t).is_ok());
    }

    #[test]
    fn route_and_train_cost_money() {
        let mut g = Game::new(2);
        let (m, p) = (first(&g, Kind::Mine), first(&g, Kind::Plant));
        let cost = g.route_cost(m, p);
        let r = g.create_route(m, p).unwrap();
        assert_eq!(g.money, START_MONEY - cost);
        g.add_train(r, Engine::Kettle).unwrap();
        assert_eq!(g.money, START_MONEY - cost - Engine::Kettle.cost());
        g.money = 0.0;
        assert!(g.add_train(r, Engine::Kettle).is_err());
    }

    #[test]
    fn full_chain_connects_town_and_wins() {
        let mut g = Game::new(3);
        g.money = 1e6;
        let (m, p, f) = (
            first(&g, Kind::Mine),
            first(&g, Kind::Plant),
            first(&g, Kind::Factory),
        );
        for r in [g.create_route(m, p), g.create_route(p, f)] {
            let r = r.unwrap();
            g.add_train(r, Engine::Express).unwrap();
        }
        let towns: Vec<usize> = g.towns().map(|(i, _)| i).collect();
        for t in towns {
            let r = g.create_route(f, t).unwrap();
            g.add_train(r, Engine::Express).unwrap();
        }
        let ev = run(&mut g, 54.0);
        assert!(ev.iter().any(|e| matches!(e, Event::TrackDone(_))));
        assert!(ev.iter().any(|e| matches!(e, Event::Delivered(..))));
        assert!(g.industries[p].supplied > 0.0);
        assert!(g.earned > 0.0);
        // Keep running until victory (one factory feeds every town).
        for _ in 0..1500 {
            if g.outcome.is_some() {
                break;
            }
            // Keep the test about economy: clear monsters.
            g.enemies.clear();
            run(&mut g, 1.0);
        }
        assert_eq!(g.outcome, Some(Outcome::Victory));
    }

    #[test]
    fn tracks_only_build_in_daylight() {
        let mut g = Game::new(4);
        let (m, p) = (first(&g, Kind::Mine), first(&g, Kind::Plant));
        let r = g.create_route(m, p).unwrap();
        g.phase = Phase::Night;
        g.phase_t = 100.0;
        g.update(0.5);
        assert_eq!(g.routes[r].built, 0.0);
        g.phase = Phase::Day;
        g.update(0.5);
        assert!(g.routes[r].built > 0.0);
    }

    #[test]
    fn day_turns_to_night_and_spawns_waves() {
        let mut g = Game::new(5);
        let ev = run(&mut g, 56.0);
        assert!(ev.contains(&Event::NightFall(1)));
        assert_eq!(g.phase, Phase::Night);
        run(&mut g, 5.0);
        assert!(!g.enemies.is_empty());
    }

    #[test]
    fn waves_escalate() {
        let mut g = Game::new(6);
        g.phase = Phase::Night;
        g.night = 1;
        let early = g.spawn_rate();
        g.night = 10;
        let late = g.spawn_rate();
        assert!(late > early * 3.0);
        assert!(g.hp_scale() > 2.0);
    }

    #[test]
    fn deadline_loses_after_last_night() {
        let mut g = Game::new(8);
        g.night = LAST_NIGHT;
        g.phase = Phase::Night;
        g.phase_t = 0.01;
        let ev = g.update(0.05);
        assert!(ev.contains(&Event::GameOver));
        assert_eq!(g.outcome, Some(Outcome::Deadline));
    }

    #[test]
    fn town_destruction_loses() {
        let mut g = Game::new(9);
        let t = first(&g, Kind::Town);
        g.industries[t].hp = 0.0;
        let ev = g.update(0.05);
        assert!(ev.contains(&Event::GameOver));
        assert_eq!(g.outcome, Some(Outcome::TownLost(t)));
    }

    #[test]
    fn enemies_wreck_industries_and_repair_restores() {
        let mut g = Game::new(10);
        let m = first(&g, Kind::Mine);
        g.industries[m].hp = 1.0;
        g.spawn(EnemyKind::Saboteur);
        let pos = g.industries[m].pos;
        g.enemies[0].pos = pos;
        g.enemies[0].target = Some(m);
        let ev = g.update(0.5);
        assert!(ev.contains(&Event::Wrecked(m)));
        assert!(g.industries[m].wrecked());
        g.enemies.clear();
        g.money = 1000.0;
        let cost = g.repair_cost(m);
        g.repair(m).unwrap();
        assert_eq!(g.money, 1000.0 - cost);
        assert!(!g.industries[m].wrecked());
    }

    #[test]
    fn turrets_need_track_and_shoot() {
        let mut g = Game::new(11);
        g.money = 1e6;
        assert!(g.place_turret(vec2(-500.0, -500.0)).is_err());
        let m = first(&g, Kind::Mine);
        let p = g.industries[m].pos + vec2(40.0, 0.0);
        g.place_turret(p).unwrap();
        g.spawn(EnemyKind::Gremlin);
        g.enemies[0].pos = p + vec2(60.0, 0.0);
        g.enemies[0].target = Some(m);
        let ev = run(&mut g, 2.0);
        assert!(ev.iter().any(|e| matches!(e, Event::Shoot(_))));
        assert!(ev.iter().any(|e| matches!(e, Event::Kill(..))));
        assert!(g.kills >= 1);
    }

    #[test]
    fn kills_give_xp_and_level_ups_offer_perks() {
        let mut g = Game::new(12);
        g.add_xp(1000);
        assert!(g.pending_levels > 0);
        let ev = g.update(0.05);
        assert!(ev.contains(&Event::LevelUp));
        let before = g.perks.clone();
        let perk = g.choose_perk(1).unwrap();
        assert!(g.perk_choices.is_none());
        let changed = format!("{:?}", before) != format!("{:?}", g.perks);
        assert!(changed, "{perk:?} should change perks");
    }

    #[test]
    fn coins_are_collected_as_money() {
        let mut g = Game::new(13);
        g.coins.push(Coin {
            pos: vec2(10.0, 10.0),
            value: 50.0,
            t: 0.0,
        });
        let m0 = g.money;
        let ev = run(&mut g, 1.0);
        assert!(ev.iter().any(|e| matches!(e, Event::Pickup(..))));
        assert!(g.money >= m0 + 50.0);
    }

    #[test]
    fn supplied_town_has_stronger_aura() {
        let mut g = Game::new(14);
        let t = first(&g, Kind::Town);
        let (r0, d0) = g.aura(t);
        g.industries[t].supplied = 10.0;
        let (r1, d1) = g.aura(t);
        assert!(r1 > r0 && d1 > d0);
    }

    #[test]
    fn upgrade_replaces_slowest_engine() {
        let mut g = Game::new(15);
        g.money = 1e6;
        let (m, p) = (first(&g, Kind::Mine), first(&g, Kind::Plant));
        let r = g.create_route(m, p).unwrap();
        assert!(g.upgrade_train(r, Engine::Mogul).is_err());
        g.add_train(r, Engine::Kettle).unwrap();
        g.upgrade_train(r, Engine::Express).unwrap();
        assert_eq!(g.routes[r].trains[0].engine, Engine::Express);
        assert!(g.upgrade_train(r, Engine::Mogul).is_err());
    }
}
