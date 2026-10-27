//! Core simulation: rail network, the HQ locomotive, towns and their militia,
//! enemy waves, the rescue train, upgrades and the economy.

use crate::geom::V2;
use crate::rng::Rng;

pub const WORLD_W: f32 = 1600.0;
pub const WORLD_H: f32 = 900.0;
pub const WAVES: u32 = 25;
pub const WAVE_TIME: f32 = 30.0;
pub const TOWN_RADIUS: f32 = 26.0;
pub const MAX_LINK: f32 = 520.0;
pub const RESCUE_COST: f32 = 60.0;
pub const RESCUE_COOLDOWN: f32 = 18.0;
pub const RESCUE_SPEED: f32 = 320.0;
pub const RESCUE_BOOST: f32 = 15.0;
pub const MILITIA_RANGE: f32 = 190.0;
const BASE_ACCEL: f32 = 150.0;
const BRAKE: f32 = 260.0;
const BULLET_SPEED: f32 = 560.0;

const NAMES: [&str; 14] = [
    "Ashby",
    "Bramford",
    "Coalton",
    "Dunmere",
    "Elmstead",
    "Fernhill",
    "Gorsedge",
    "Harrowgate",
    "Ironbridge",
    "Juniper",
    "Kettleby",
    "Larkspur",
    "Millbrook",
    "Northwick",
];

/// X coordinate of the river's centre line at height `y`.
pub fn river_x(y: f32) -> f32 {
    WORLD_W * 0.5 + 70.0 * (y / 140.0).sin()
}

/// Whether a straight link between `a` and `b` needs a bridge.
pub fn crosses_river(a: V2, b: V2) -> bool {
    (a.x - river_x(a.y)).signum() != (b.x - river_x(b.y)).signum()
}

/// Price of laying a straight track between two points.
pub fn track_cost(a: V2, b: V2) -> u32 {
    let base = 40.0 + a.dist(b) * 0.45;
    let k = if crosses_river(a, b) { 2.2 } else { 1.0 };
    (base * k).round() as u32
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Steer {
    #[default]
    Straight,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub steer: Steer,
    /// -1 brake, 0 cruise, +1 accelerate.
    pub throttle: f32,
    /// Flip direction when (almost) stopped.
    pub reverse: bool,
    /// Dispatch the rescue train to the assaulted town.
    pub rescue: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot,
    MilitiaShot,
    Hit,
    Killed { pos: V2, big: bool },
    Coin,
    LevelUp,
    Served { town: usize, cash: u32 },
    Built,
    Upgraded,
    Denied,
    WaveStart { wave: u32, town: usize },
    BossSpawned,
    RescueDispatched,
    RescueArrived(usize),
    PlayerHurt,
    TownHurt(usize),
    TownLost(usize),
    Junction,
    GameOver,
    Victory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    SameNode,
    Exists,
    TooLong,
    NotConnected,
    NoCash,
    MaxLevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Multishot,
    Range,
    Speed,
    Armor,
    Militia,
    Magnet,
    Dividends,
    Pierce,
}

impl Upgrade {
    pub const ALL: [Upgrade; 10] = [
        Upgrade::Damage,
        Upgrade::FireRate,
        Upgrade::Multishot,
        Upgrade::Range,
        Upgrade::Speed,
        Upgrade::Armor,
        Upgrade::Militia,
        Upgrade::Magnet,
        Upgrade::Dividends,
        Upgrade::Pierce,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavy Shells",
            Upgrade::FireRate => "Steam Turret",
            Upgrade::Multishot => "Extra Gunner",
            Upgrade::Range => "Rangefinder",
            Upgrade::Speed => "Superheater",
            Upgrade::Armor => "Armour Plate",
            Upgrade::Militia => "Militia Drill",
            Upgrade::Magnet => "Coin Magnet",
            Upgrade::Dividends => "Dividends",
            Upgrade::Pierce => "Piercing Rounds",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+30% shot damage",
            Upgrade::FireRate => "+22% fire rate",
            Upgrade::Multishot => "+1 shell per volley",
            Upgrade::Range => "+18% gun range",
            Upgrade::Speed => "+18% top speed",
            Upgrade::Armor => "+30 max HP, full repair",
            Upgrade::Militia => "Militia +35% damage",
            Upgrade::Magnet => "+50% coin pickup radius",
            Upgrade::Dividends => "+25% station income",
            Upgrade::Pierce => "Shells pierce +1 enemy",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub damage: f32,
    pub fire_cd: f32,
    pub multishot: u32,
    pub range: f32,
    pub top_speed: f32,
    pub max_hp: f32,
    pub militia_mult: f32,
    pub magnet: f32,
    pub income_mult: f32,
    pub pierce: u32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            damage: 10.0,
            fire_cd: 0.55,
            multishot: 1,
            range: 250.0,
            top_speed: 150.0,
            max_hp: 100.0,
            militia_mult: 1.0,
            magnet: 90.0,
            income_mult: 1.0,
            pierce: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Town {
    pub name: &'static str,
    pub pos: V2,
    pub pop: f32,
    pub hp: f32,
    /// 0 = unconnected site, 1..=3 station level.
    pub station: u8,
    /// Service rating in [0, 1]; drives growth.
    pub rating: f32,
    pub since_served: f32,
    pub alive: bool,
    /// Seconds of rescue-train boost left.
    pub boost: f32,
    pub militia_cd: f32,
    pub hurt_cd: f32,
}

impl Town {
    pub fn max_hp(&self) -> f32 {
        60.0 + 40.0 * self.station as f32
    }

    pub fn max_pop(&self) -> f32 {
        50.0 + 50.0 * self.station as f32
    }

    pub fn active(&self) -> bool {
        self.alive && self.station > 0
    }

    /// Number of militia defenders this town fields.
    pub fn militia(&self) -> usize {
        if self.active() {
            ((self.pop / 18.0) as usize).min(10)
        } else {
            0
        }
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub len: f32,
    pub bridge: bool,
}

impl Edge {
    pub fn other(&self, n: usize) -> usize {
        if self.a == n { self.b } else { self.a }
    }
}

#[derive(Clone, Debug)]
pub struct Loco {
    pub edge: usize,
    /// Distance from `edges[edge].a`.
    pub s: f32,
    /// Travelling a -> b.
    pub forward: bool,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Grunt,
    Runner,
    Brute,
    Boss,
}

impl Kind {
    pub fn speed(self) -> f32 {
        match self {
            Kind::Grunt => 42.0,
            Kind::Runner => 78.0,
            Kind::Brute => 28.0,
            Kind::Boss => 22.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            Kind::Grunt => 9.0,
            Kind::Runner => 7.0,
            Kind::Brute => 15.0,
            Kind::Boss => 30.0,
        }
    }

    fn base_hp(self) -> f32 {
        match self {
            Kind::Grunt => 14.0,
            Kind::Runner => 9.0,
            Kind::Brute => 60.0,
            Kind::Boss => 900.0,
        }
    }

    fn dps(self) -> f32 {
        match self {
            Kind::Grunt => 5.0,
            Kind::Runner => 4.0,
            Kind::Brute => 12.0,
            Kind::Boss => 30.0,
        }
    }

    fn xp(self) -> f32 {
        match self {
            Kind::Grunt => 1.0,
            Kind::Runner => 1.0,
            Kind::Brute => 4.0,
            Kind::Boss => 30.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub kind: Kind,
    /// Town index under assault, or `None` when hunting the locomotive.
    pub target: Option<usize>,
    pub flash: f32,
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: u32,
    pub militia: bool,
    hits: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Coin {
    pub pos: V2,
    pub value: f32,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Rescue {
    pub pts: Vec<V2>,
    pub dist: f32,
    pub target: usize,
}

impl Rescue {
    pub fn total(&self) -> f32 {
        self.pts.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    /// Position and heading along the route.
    pub fn pose(&self) -> (V2, V2) {
        let mut d = self.dist;
        for w in self.pts.windows(2) {
            let l = w[0].dist(w[1]);
            if d <= l {
                return (w[0].lerp(w[1], d / l.max(1e-3)), (w[1] - w[0]).norm());
            }
            d -= l;
        }
        let n = self.pts.len();
        let dir = if n >= 2 {
            (self.pts[n - 1] - self.pts[n - 2]).norm()
        } else {
            V2::new(1.0, 0.0)
        };
        (self.pts[n - 1], dir)
    }
}

pub struct Game {
    pub rng: Rng,
    pub towns: Vec<Town>,
    pub edges: Vec<Edge>,
    pub loco: Loco,
    pub hp: f32,
    pub stats: Stats,
    pub enemies: Vec<Enemy>,
    pub shots: Vec<Shot>,
    pub coins: Vec<Coin>,
    pub rescue: Option<Rescue>,
    pub rescue_cd: f32,
    pub cash: f32,
    pub track_value: f32,
    pub xp: f32,
    pub level: u32,
    pub choice: Option<[Upgrade; 3]>,
    pub wave: u32,
    pub wave_time: f32,
    /// Town under assault this wave.
    pub target: usize,
    pub phase: Phase,
    pub time: f32,
    pub kills: u32,
    pub events: Vec<Event>,
    spawn_acc: f32,
    fire_cd: f32,
    hurt_cd: f32,
    next_id: u32,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut positions = vec![V2::new(260.0, WORLD_H * 0.5)];
        let mut tries = 0;
        while positions.len() < NAMES.len() && tries < 20_000 {
            tries += 1;
            let p = V2::new(
                rng.range(90.0, WORLD_W - 90.0),
                rng.range(110.0, WORLD_H - 80.0),
            );
            let clear_of_river = (p.x - river_x(p.y)).abs() > 70.0;
            if clear_of_river && positions.iter().all(|q| q.dist(p) > 185.0) {
                positions.push(p);
            }
        }
        let towns = positions
            .iter()
            .zip(NAMES)
            .map(|(&pos, name)| Town {
                name,
                pos,
                pop: 20.0,
                hp: 60.0,
                station: 0,
                rating: 0.5,
                since_served: 0.0,
                alive: true,
                boost: 0.0,
                militia_cd: 0.0,
                hurt_cd: 0.0,
            })
            .collect();
        let mut g = Game {
            rng,
            towns,
            edges: Vec::new(),
            loco: Loco {
                edge: 0,
                s: 0.0,
                forward: true,
                speed: 60.0,
            },
            hp: 100.0,
            stats: Stats::default(),
            enemies: Vec::new(),
            shots: Vec::new(),
            coins: Vec::new(),
            rescue: None,
            rescue_cd: 0.0,
            cash: 220.0,
            track_value: 0.0,
            xp: 0.0,
            level: 1,
            choice: None,
            wave: 0,
            wave_time: 0.0,
            target: 0,
            phase: Phase::Playing,
            time: 0.0,
            kills: 0,
            events: Vec::new(),
            spawn_acc: 0.0,
            fire_cd: 0.0,
            hurt_cd: 0.0,
            next_id: 0,
        };
        g.seed_network();
        g.start_wave();
        g
    }

    /// HQ plus its three nearest same-bank neighbours, joined by a spanning tree
    /// and one extra loop so there is a junction choice from the start.
    fn seed_network(&mut self) {
        let hq = self.towns[0].pos;
        let mut idx: Vec<usize> = (1..self.towns.len())
            .filter(|&i| !crosses_river(hq, self.towns[i].pos))
            .collect();
        idx.sort_by(|&a, &b| {
            hq.dist(self.towns[a].pos)
                .total_cmp(&hq.dist(self.towns[b].pos))
        });
        let mut set = vec![0];
        set.extend(idx.into_iter().take(3));
        let mut in_tree = vec![set[0]];
        let mut spare = Vec::new();
        while in_tree.len() < set.len() {
            let mut best = (f32::INFINITY, 0, 0);
            for &a in &in_tree {
                for &b in set.iter().filter(|b| !in_tree.contains(b)) {
                    let d = self.towns[a].pos.dist(self.towns[b].pos);
                    if d < best.0 {
                        best = (d, a, b);
                    }
                }
            }
            self.add_edge(best.1, best.2);
            in_tree.push(best.2);
        }
        for (i, &a) in set.iter().enumerate() {
            for &b in &set[i + 1..] {
                if self.edge_between(a, b).is_none() {
                    spare.push((self.towns[a].pos.dist(self.towns[b].pos), a, b));
                }
            }
        }
        spare.sort_by(|x, y| x.0.total_cmp(&y.0));
        if let Some(&(_, a, b)) = spare.first() {
            self.add_edge(a, b);
        }
        for &i in &set {
            let t = &mut self.towns[i];
            t.station = 1;
            t.hp = t.max_hp();
        }
        self.towns[0].pop = 35.0;
        self.track_value = 0.0;
        self.loco.edge = self.incident(0)[0];
        self.loco.forward = self.edges[self.loco.edge].a == 0;
        self.loco.s = if self.loco.forward {
            0.0
        } else {
            self.edges[self.loco.edge].len
        };
    }

    fn add_edge(&mut self, a: usize, b: usize) {
        let (pa, pb) = (self.towns[a].pos, self.towns[b].pos);
        self.edges.push(Edge {
            a,
            b,
            len: pa.dist(pb),
            bridge: crosses_river(pa, pb),
        });
    }

    pub fn edge_between(&self, a: usize, b: usize) -> Option<usize> {
        self.edges
            .iter()
            .position(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a))
    }

    pub fn incident(&self, n: usize) -> Vec<usize> {
        (0..self.edges.len())
            .filter(|&i| self.edges[i].a == n || self.edges[i].b == n)
            .collect()
    }

    pub fn connected(&self, n: usize) -> bool {
        self.edges.iter().any(|e| e.a == n || e.b == n)
    }

    pub fn loco_pos(&self) -> V2 {
        let e = &self.edges[self.loco.edge];
        let (a, b) = (self.towns[e.a].pos, self.towns[e.b].pos);
        a.lerp(b, self.loco.s / e.len.max(1e-3))
    }

    /// Direction of travel.
    pub fn loco_dir(&self) -> V2 {
        let e = &self.edges[self.loco.edge];
        let d = (self.towns[e.b].pos - self.towns[e.a].pos).norm();
        if self.loco.forward { d } else { d * -1.0 }
    }

    /// Node the locomotive is heading towards.
    pub fn next_node(&self) -> usize {
        let e = &self.edges[self.loco.edge];
        if self.loco.forward { e.b } else { e.a }
    }

    /// Edge chosen at `node` when arriving along `from_edge` with `heading`.
    pub fn pick_next(&self, node: usize, from_edge: usize, heading: V2, steer: Steer) -> usize {
        let pos = self.towns[node].pos;
        let cands: Vec<(usize, f32)> = self
            .incident(node)
            .into_iter()
            .filter(|&e| e != from_edge)
            .map(|e| {
                let d = (self.towns[self.edges[e].other(node)].pos - pos).norm();
                (e, heading.cross(d).atan2(heading.dot(d)))
            })
            .collect();
        let pick = match steer {
            Steer::Straight => cands.iter().min_by(|a, b| a.1.abs().total_cmp(&b.1.abs())),
            Steer::Left => cands.iter().min_by(|a, b| a.1.total_cmp(&b.1)),
            Steer::Right => cands.iter().max_by(|a, b| a.1.total_cmp(&b.1)),
        };
        pick.map_or(from_edge, |p| p.0)
    }

    /// Edge the locomotive will take at its next junction for `steer`.
    pub fn preview_next(&self, steer: Steer) -> usize {
        self.pick_next(self.next_node(), self.loco.edge, self.loco_dir(), steer)
    }

    /// Shortest path between two nodes as a list of node indices.
    pub fn path(&self, from: usize, to: usize) -> Option<Vec<usize>> {
        let n = self.towns.len();
        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        let mut done = vec![false; n];
        dist[from] = 0.0;
        while let Some(u) = (0..n)
            .filter(|&i| !done[i] && dist[i].is_finite())
            .min_by(|&a, &b| dist[a].total_cmp(&dist[b]))
        {
            done[u] = true;
            if u == to {
                break;
            }
            for e in self.incident(u) {
                let v = self.edges[e].other(u);
                let nd = dist[u] + self.edges[e].len;
                if nd < dist[v] {
                    dist[v] = nd;
                    prev[v] = u;
                }
            }
        }
        if !dist[to].is_finite() {
            return None;
        }
        let mut p = vec![to];
        while *p.last().unwrap() != from {
            p.push(prev[*p.last().unwrap()]);
        }
        p.reverse();
        Some(p)
    }

    pub fn xp_needed(&self) -> f32 {
        8.0 + 7.0 * self.level as f32
    }

    pub fn company_value(&self) -> u32 {
        let pop: f32 = self
            .towns
            .iter()
            .filter(|t| t.active())
            .map(|t| t.pop)
            .sum();
        let stations: u32 = self
            .towns
            .iter()
            .filter(|t| t.alive)
            .map(|t| t.station as u32)
            .sum();
        (self.cash + self.track_value + pop * 8.0 + stations as f32 * 100.0 + self.kills as f32)
            .max(0.0) as u32
    }

    pub fn link_cost(&self, a: usize, b: usize) -> u32 {
        track_cost(self.towns[a].pos, self.towns[b].pos)
    }

    pub fn can_link(&self, a: usize, b: usize) -> Result<u32, BuildError> {
        if a == b {
            return Err(BuildError::SameNode);
        }
        if self.edge_between(a, b).is_some() {
            return Err(BuildError::Exists);
        }
        if self.towns[a].pos.dist(self.towns[b].pos) > MAX_LINK {
            return Err(BuildError::TooLong);
        }
        if !self.connected(a) && !self.connected(b) {
            return Err(BuildError::NotConnected);
        }
        let cost = self.link_cost(a, b);
        if self.cash < cost as f32 {
            return Err(BuildError::NoCash);
        }
        Ok(cost)
    }

    /// Lay track between two towns; a new town gets a level 1 station.
    pub fn build_link(&mut self, a: usize, b: usize) -> Result<(), BuildError> {
        let cost = match self.can_link(a, b) {
            Ok(c) => c,
            Err(e) => {
                self.events.push(Event::Denied);
                return Err(e);
            }
        };
        self.cash -= cost as f32;
        self.track_value += cost as f32 * 0.6;
        self.add_edge(a, b);
        for n in [a, b] {
            let t = &mut self.towns[n];
            if t.station == 0 {
                t.station = 1;
                t.alive = true;
                t.hp = t.max_hp();
                t.rating = 0.5;
                t.since_served = 0.0;
            }
        }
        self.events.push(Event::Built);
        Ok(())
    }

    /// Cost to upgrade (or rebuild, if destroyed) the station at `n`.
    pub fn station_cost(&self, n: usize) -> Result<u32, BuildError> {
        let t = &self.towns[n];
        if t.station == 0 {
            Err(BuildError::NotConnected)
        } else if !t.alive {
            Ok(150)
        } else if t.station >= 3 {
            Err(BuildError::MaxLevel)
        } else {
            Ok(140 * t.station as u32)
        }
    }

    pub fn upgrade_station(&mut self, n: usize) -> Result<(), BuildError> {
        let cost = match self.station_cost(n) {
            Ok(c) if self.cash >= c as f32 => c,
            Ok(_) => {
                self.events.push(Event::Denied);
                return Err(BuildError::NoCash);
            }
            Err(e) => {
                self.events.push(Event::Denied);
                return Err(e);
            }
        };
        self.cash -= cost as f32;
        let t = &mut self.towns[n];
        if t.alive {
            t.station += 1;
        } else {
            t.alive = true;
            t.pop = 15.0;
            t.rating = 0.5;
        }
        t.hp = t.max_hp();
        self.events.push(Event::Upgraded);
        Ok(())
    }

    pub fn choose(&mut self, i: usize) {
        let Some(c) = self.choice.take() else { return };
        let u = c[i.min(2)];
        let s = &mut self.stats;
        match u {
            Upgrade::Damage => s.damage *= 1.3,
            Upgrade::FireRate => s.fire_cd /= 1.22,
            Upgrade::Multishot => s.multishot += 1,
            Upgrade::Range => s.range *= 1.18,
            Upgrade::Speed => s.top_speed *= 1.18,
            Upgrade::Armor => {
                s.max_hp += 30.0;
                self.hp = s.max_hp;
            }
            Upgrade::Militia => s.militia_mult *= 1.35,
            Upgrade::Magnet => s.magnet *= 1.5,
            Upgrade::Dividends => s.income_mult *= 1.25,
            Upgrade::Pierce => s.pierce += 1,
        }
        self.check_level();
    }

    fn check_level(&mut self) {
        if self.choice.is_none() && self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            let mut pool = Upgrade::ALL.to_vec();
            let mut c = [Upgrade::Damage; 3];
            for slot in &mut c {
                *slot = pool.remove(self.rng.below(pool.len()));
            }
            self.choice = Some(c);
            self.events.push(Event::LevelUp);
        }
    }

    fn start_wave(&mut self) {
        self.wave += 1;
        self.wave_time = 0.0;
        self.spawn_acc = 0.0;
        let alive: Vec<usize> = (0..self.towns.len())
            .filter(|&i| self.towns[i].active())
            .collect();
        if !alive.is_empty() {
            self.target = alive[self.rng.below(alive.len())];
        }
        self.events.push(Event::WaveStart {
            wave: self.wave,
            town: self.target,
        });
        if self.wave.is_multiple_of(5) {
            let p = self.border_point_near(self.towns[self.target].pos);
            let hp = Kind::Boss.base_hp() * (self.wave as f32 / 5.0).powf(1.3);
            self.spawn(Kind::Boss, p, hp, Some(self.target));
            self.events.push(Event::BossSpawned);
        }
    }

    pub fn wave_enemy_count(wave: u32) -> u32 {
        10 + 5 * wave + wave * wave / 3
    }

    pub fn hp_scale(wave: u32) -> f32 {
        let w = wave.saturating_sub(1) as f32;
        1.0 + 0.12 * w + 0.004 * w * w
    }

    fn border_point(&mut self) -> V2 {
        let t = self.rng.f32();
        match self.rng.below(4) {
            0 => V2::new(t * WORLD_W, -20.0),
            1 => V2::new(t * WORLD_W, WORLD_H + 20.0),
            2 => V2::new(-20.0, t * WORLD_H),
            _ => V2::new(WORLD_W + 20.0, t * WORLD_H),
        }
    }

    fn border_point_near(&mut self, to: V2) -> V2 {
        let mut best = self.border_point();
        for _ in 0..2 {
            let p = self.border_point();
            if p.dist(to) < best.dist(to) {
                best = p;
            }
        }
        best
    }

    fn spawn(&mut self, kind: Kind, pos: V2, hp: f32, target: Option<usize>) {
        self.next_id += 1;
        self.enemies.push(Enemy {
            id: self.next_id,
            pos,
            hp,
            max_hp: hp,
            kind,
            target,
            flash: 0.0,
        });
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing || self.choice.is_some() {
            return;
        }
        self.time += dt;
        self.hurt_cd -= dt;
        self.move_loco(dt, input);
        if input.rescue {
            self.dispatch_rescue();
        }
        self.update_towns(dt);
        self.update_waves(dt);
        self.player_fire(dt);
        self.militia_fire(dt);
        self.update_shots(dt);
        self.update_enemies(dt);
        self.update_coins(dt);
        self.update_rescue(dt);
        self.check_level();
        if self.phase == Phase::Playing
            && (self.hp <= 0.0 || !self.towns.iter().any(|t| t.active()))
        {
            self.hp = self.hp.max(0.0);
            self.phase = Phase::Lost;
            self.events.push(Event::GameOver);
        }
    }

    fn move_loco(&mut self, dt: f32, input: &Input) {
        let top = self.stats.top_speed;
        if input.reverse && self.loco.speed < 8.0 {
            self.loco.forward = !self.loco.forward;
            self.loco.speed = 0.0;
        }
        if input.throttle > 0.0 {
            self.loco.speed += BASE_ACCEL * dt;
        } else if input.throttle < 0.0 {
            self.loco.speed -= BRAKE * dt;
        }
        self.loco.speed = self.loco.speed.clamp(0.0, top);
        let mut d = self.loco.speed * dt;
        for _ in 0..8 {
            let e = &self.edges[self.loco.edge];
            let (len, a, b) = (e.len, e.a, e.b);
            let node = if self.loco.forward {
                if self.loco.s + d < len {
                    self.loco.s += d;
                    return;
                }
                d -= len - self.loco.s;
                b
            } else {
                if self.loco.s - d > 0.0 {
                    self.loco.s -= d;
                    return;
                }
                d -= self.loco.s;
                a
            };
            let heading = self.loco_dir();
            self.serve(node);
            let next = self.pick_next(node, self.loco.edge, heading, input.steer);
            self.loco.edge = next;
            self.loco.forward = self.edges[next].a == node;
            self.loco.s = if self.loco.forward {
                0.0
            } else {
                self.edges[next].len
            };
            self.events.push(Event::Junction);
        }
    }

    fn serve(&mut self, n: usize) {
        let mult = self.stats.income_mult;
        let t = &mut self.towns[n];
        if !t.active() || t.since_served < 2.0 {
            return;
        }
        let freshness = 0.4 + 0.6 * (t.since_served / 20.0).min(1.0);
        let cash = (8.0 + t.pop * 0.4) * freshness * mult;
        t.since_served = 0.0;
        t.rating = (t.rating + 0.3).min(1.0);
        self.cash += cash;
        // Station crews patch up the HQ locomotive.
        self.hp = (self.hp + 4.0).min(self.stats.max_hp);
        self.events.push(Event::Served {
            town: n,
            cash: cash.round() as u32,
        });
    }

    fn update_towns(&mut self, dt: f32) {
        for t in &mut self.towns {
            t.hurt_cd -= dt;
            if !t.active() {
                continue;
            }
            t.since_served += dt;
            let decay = if t.since_served > 25.0 { 0.04 } else { 0.015 };
            t.rating = (t.rating - decay * dt).max(0.0);
            t.pop += dt * 0.5 * (t.rating - 0.3) * (1.0 + t.pop / 60.0);
            t.pop = t.pop.clamp(5.0, t.max_pop());
            t.boost = (t.boost - dt).max(0.0);
            t.hp = (t.hp + 0.6 * dt).min(t.max_hp());
        }
    }

    fn update_waves(&mut self, dt: f32) {
        self.wave_time += dt;
        if self.wave_time >= WAVE_TIME {
            if self.wave >= WAVES {
                self.phase = Phase::Won;
                self.events.push(Event::Victory);
                return;
            }
            self.start_wave();
        }
        if !self.towns[self.target].active() {
            return;
        }
        let n = Self::wave_enemy_count(self.wave) as f32;
        let window = WAVE_TIME * 0.85;
        if self.wave_time > window {
            return;
        }
        self.spawn_acc += dt * n / window;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let w = self.wave;
            let r = self.rng.f32();
            let kind = if w >= 6 && r < 0.08 + 0.006 * w as f32 {
                Kind::Brute
            } else if w >= 3 && r < 0.35 {
                Kind::Runner
            } else {
                Kind::Grunt
            };
            let hp = kind.base_hp() * Self::hp_scale(w);
            let (target, pos) = if self.rng.chance(0.65) {
                let tp = self.towns[self.target].pos;
                (Some(self.target), self.border_point_near(tp))
            } else {
                (None, self.border_point())
            };
            self.spawn(kind, pos, hp, target);
        }
    }

    fn nearest_enemy(&self, p: V2, range: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = range;
        for (i, e) in self.enemies.iter().enumerate() {
            let d = e.pos.dist(p) - e.kind.radius();
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    fn player_fire(&mut self, dt: f32) {
        self.fire_cd -= dt;
        if self.fire_cd > 0.0 {
            return;
        }
        let p = self.loco_pos();
        let Some(i) = self.nearest_enemy(p, self.stats.range) else {
            return;
        };
        self.fire_cd = self.stats.fire_cd;
        let dir = (self.enemies[i].pos - p).norm();
        let n = self.stats.multishot;
        for k in 0..n {
            let off = (k as f32 - (n - 1) as f32 * 0.5) * 0.14;
            self.shots.push(Shot {
                pos: p,
                vel: dir.rotate(off) * BULLET_SPEED,
                dmg: self.stats.damage,
                life: self.stats.range / BULLET_SPEED + 0.15,
                pierce: self.stats.pierce,
                militia: false,
                hits: Vec::new(),
            });
        }
        self.events.push(Event::Shot);
    }

    fn militia_fire(&mut self, dt: f32) {
        let mut fired = false;
        for ti in 0..self.towns.len() {
            let m = self.towns[ti].militia();
            if m == 0 {
                continue;
            }
            self.towns[ti].militia_cd -= dt;
            if self.towns[ti].militia_cd > 0.0 {
                continue;
            }
            let tp = self.towns[ti].pos;
            let Some(i) = self.nearest_enemy(tp, MILITIA_RANGE) else {
                continue;
            };
            let boost = if self.towns[ti].boost > 0.0 { 2.5 } else { 1.0 };
            self.towns[ti].militia_cd = 1.5 / (m as f32).sqrt() / boost;
            let dir = (self.enemies[i].pos - tp).norm();
            let from = tp + dir * (TOWN_RADIUS * 0.6);
            self.shots.push(Shot {
                pos: from,
                vel: dir * 480.0,
                dmg: 6.0 * self.stats.militia_mult * (1.0 + 0.04 * m as f32),
                life: MILITIA_RANGE / 480.0 + 0.2,
                pierce: 0,
                militia: true,
                hits: Vec::new(),
            });
            fired = true;
        }
        if fired {
            self.events.push(Event::MilitiaShot);
        }
    }

    fn update_shots(&mut self, dt: f32) {
        let mut hit_any = false;
        for s in &mut self.shots {
            s.pos += s.vel * dt;
            s.life -= dt;
            for e in &mut self.enemies {
                if s.life <= 0.0 || e.hp <= 0.0 || s.hits.contains(&e.id) {
                    continue;
                }
                if s.pos.dist(e.pos) < e.kind.radius() + 4.0 {
                    e.hp -= s.dmg;
                    e.flash = 0.1;
                    s.hits.push(e.id);
                    hit_any = true;
                    if s.pierce == 0 {
                        s.life = 0.0;
                    } else {
                        s.pierce -= 1;
                    }
                }
            }
        }
        self.shots.retain(|s| s.life > 0.0);
        if hit_any {
            self.events.push(Event::Hit);
        }
        self.reap();
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let e = self.enemies.swap_remove(i);
            self.kills += 1;
            self.xp += e.kind.xp();
            let big = matches!(e.kind, Kind::Brute | Kind::Boss);
            self.events.push(Event::Killed { pos: e.pos, big });
            let drops = match e.kind {
                Kind::Boss => 12,
                Kind::Brute => 2,
                _ => usize::from(self.rng.chance(0.3)),
            };
            for _ in 0..drops {
                let off = V2::from_angle(self.rng.range(0.0, std::f32::consts::TAU))
                    * self.rng.range(0.0, 20.0);
                self.coins.push(Coin {
                    pos: e.pos + off,
                    value: 3.0 + self.wave as f32 * 0.3,
                    life: 14.0,
                });
            }
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let lp = self.loco_pos();
        let mut player_dmg = 0.0;
        for e in &mut self.enemies {
            e.flash -= dt;
            if let Some(t) = e.target
                && !self.towns[t].active()
            {
                e.target = None;
            }
            let goal = e.target.map_or(lp, |t| self.towns[t].pos);
            let reach = e.kind.radius()
                + if e.target.is_some() {
                    TOWN_RADIUS
                } else {
                    12.0
                };
            let d = goal.dist(e.pos);
            if d > reach {
                e.pos += (goal - e.pos).norm() * (e.kind.speed() * dt).min(d - reach);
            } else if let Some(t) = e.target {
                let town = &mut self.towns[t];
                town.hp -= e.kind.dps() * dt;
                if town.hurt_cd <= 0.0 {
                    town.hurt_cd = 1.0;
                    self.events.push(Event::TownHurt(t));
                }
                if town.hp <= 0.0 && town.alive {
                    town.alive = false;
                    town.pop = 0.0;
                    town.boost = 0.0;
                    self.events.push(Event::TownLost(t));
                }
            } else {
                player_dmg += e.kind.dps() * dt;
            }
            // Enemies grazing the locomotive also take ram damage.
            if e.pos.dist(lp) < e.kind.radius() + 14.0 && e.kind != Kind::Boss {
                e.hp -= 6.0 * dt * (1.0 + self.loco.speed / 60.0);
            }
        }
        // Light separation so crowds spread out.
        let n = self.enemies.len();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (self.enemies[i].pos, self.enemies[j].pos);
                let min = self.enemies[i].kind.radius() + self.enemies[j].kind.radius();
                let d = a.dist(b);
                if d < min && d > 1e-3 {
                    let push = (a - b).norm() * ((min - d) * 0.25);
                    self.enemies[i].pos += push;
                    self.enemies[j].pos += push * -1.0;
                }
            }
        }
        if player_dmg > 0.0 {
            self.hp -= player_dmg;
            if self.hurt_cd <= 0.0 {
                self.hurt_cd = 0.6;
                self.events.push(Event::PlayerHurt);
            }
        }
        self.reap();
    }

    fn update_coins(&mut self, dt: f32) {
        let lp = self.loco_pos();
        let magnet = self.stats.magnet;
        let mut got = 0.0;
        self.coins.retain_mut(|c| {
            c.life -= dt;
            let d = c.pos.dist(lp);
            if d < 18.0 {
                got += c.value;
                return false;
            }
            if d < magnet {
                c.pos += (lp - c.pos).norm() * (420.0 * dt).min(d);
            }
            c.life > 0.0
        });
        if got > 0.0 {
            self.cash += got;
            self.events.push(Event::Coin);
        }
    }

    pub fn rescue_ready(&self) -> bool {
        self.rescue.is_none() && self.rescue_cd <= 0.0 && self.cash >= RESCUE_COST
    }

    fn dispatch_rescue(&mut self) {
        if !self.rescue_ready() || !self.towns[self.target].active() {
            self.events.push(Event::Denied);
            return;
        }
        let from = self.nearest_node(self.loco_pos());
        let Some(path) = self.path(from, self.target) else {
            self.events.push(Event::Denied);
            return;
        };
        let mut pts = vec![self.loco_pos()];
        pts.extend(path.iter().map(|&n| self.towns[n].pos));
        self.cash -= RESCUE_COST;
        self.rescue = Some(Rescue {
            pts,
            dist: 0.0,
            target: self.target,
        });
        self.events.push(Event::RescueDispatched);
    }

    pub fn nearest_node(&self, p: V2) -> usize {
        (0..self.towns.len())
            .filter(|&i| self.connected(i))
            .min_by(|&a, &b| {
                self.towns[a]
                    .pos
                    .dist(p)
                    .total_cmp(&self.towns[b].pos.dist(p))
            })
            .unwrap_or(0)
    }

    fn update_rescue(&mut self, dt: f32) {
        self.rescue_cd = (self.rescue_cd - dt).max(0.0);
        let Some(r) = &mut self.rescue else { return };
        r.dist += RESCUE_SPEED * dt;
        let (pos, _) = r.pose();
        let done = r.dist >= r.total();
        let target = r.target;
        for e in &mut self.enemies {
            if e.pos.dist(pos) < e.kind.radius() + 16.0 {
                e.hp -= 80.0 * dt;
                e.flash = 0.1;
            }
        }
        if done {
            self.rescue = None;
            self.rescue_cd = RESCUE_COOLDOWN;
            let t = &mut self.towns[target];
            if t.active() {
                t.boost = RESCUE_BOOST;
                t.hp = (t.hp + t.max_hp() * 0.4).min(t.max_hp());
                t.pop = (t.pop + 10.0).min(t.max_pop());
            }
            self.events.push(Event::RescueArrived(target));
        }
        self.reap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    /// Hand-built cross junction: centre node 0 with arms east(1), north(2), south(3), west(4).
    fn cross() -> Game {
        let mut g = Game::new(1);
        g.enemies.clear();
        g.target = 0;
        let c = V2::new(800.0, 450.0);
        let pts = [
            c,
            c + V2::new(200.0, 0.0),
            c + V2::new(0.0, -200.0),
            c + V2::new(0.0, 200.0),
            c + V2::new(-200.0, 0.0),
        ];
        g.towns.truncate(5);
        for (t, p) in g.towns.iter_mut().zip(pts) {
            t.pos = p;
            t.station = 1;
            t.alive = true;
        }
        g.edges.clear();
        for i in 1..5 {
            g.add_edge(0, i);
        }
        g
    }

    #[test]
    fn new_game_has_a_loop_network() {
        let g = Game::new(42);
        assert_eq!(g.towns.len(), NAMES.len());
        assert_eq!(g.edges.len(), 4, "3 tree edges plus one loop");
        assert_eq!(g.towns.iter().filter(|t| t.active()).count(), 4);
        assert_eq!(g.wave, 1);
        assert!(g.towns.iter().all(|t| t.pos.x > 0.0 && t.pos.x < WORLD_W));
    }

    #[test]
    fn steering_picks_relative_turns() {
        let g = cross();
        // Arriving at centre from the west, heading east (edge 3 is 0-4).
        let h = V2::new(1.0, 0.0);
        assert_eq!(g.pick_next(0, 3, h, Steer::Straight), 0);
        assert_eq!(
            g.pick_next(0, 3, h, Steer::Left),
            1,
            "left is north on screen"
        );
        assert_eq!(g.pick_next(0, 3, h, Steer::Right), 2, "right is south");
    }

    #[test]
    fn dead_end_reverses() {
        let g = cross();
        assert_eq!(g.pick_next(1, 0, V2::new(1.0, 0.0), Steer::Straight), 0);
    }

    #[test]
    fn loco_moves_through_junction_and_serves() {
        let mut g = cross();
        g.loco = Loco {
            edge: 3,
            s: 190.0,
            forward: false,
            speed: 100.0,
        };
        // Edge 3 is 0 -> 4; travelling backwards heads into centre node 0.
        g.towns[0].since_served = 10.0;
        let cash = g.cash;
        let input = Input {
            steer: Steer::Left,
            ..idle()
        };
        for _ in 0..120 {
            g.move_loco(1.0 / 60.0, &input);
        }
        assert!(g.cash > cash);
        assert!(g.events.contains(&Event::Junction));
    }

    #[test]
    fn reverse_only_when_slow() {
        let mut g = cross();
        g.loco.speed = 100.0;
        let f = g.loco.forward;
        g.move_loco(
            0.01,
            &Input {
                reverse: true,
                ..idle()
            },
        );
        assert_eq!(g.loco.forward, f);
        g.loco.speed = 0.0;
        g.move_loco(
            0.01,
            &Input {
                reverse: true,
                ..idle()
            },
        );
        assert_ne!(g.loco.forward, f);
    }

    #[test]
    fn bridges_cost_more() {
        let a = V2::new(600.0, 450.0);
        let b = V2::new(1000.0, 450.0);
        let c = V2::new(200.0, 450.0);
        assert!(crosses_river(a, b));
        assert!(!crosses_river(a, c));
        assert!(track_cost(a, b) > track_cost(a, c) * 2);
    }

    #[test]
    fn build_rules() {
        let mut g = cross();
        g.towns.push(g.towns[1].clone());
        g.towns[5].pos = V2::new(1000.0, 700.0);
        g.towns[5].station = 0;
        g.cash = 10_000.0;
        assert_eq!(g.build_link(1, 1), Err(BuildError::SameNode));
        assert_eq!(g.build_link(0, 1), Err(BuildError::Exists));
        assert!(g.build_link(1, 5).is_ok());
        assert_eq!(g.towns[5].station, 1);
        assert!(g.upgrade_station(5).is_ok());
        assert!(g.upgrade_station(5).is_ok());
        assert_eq!(g.upgrade_station(5), Err(BuildError::MaxLevel));
        g.cash = 0.0;
        assert_eq!(g.build_link(2, 5), Err(BuildError::NoCash));
    }

    #[test]
    fn link_needs_network_endpoint() {
        let mut g = Game::new(3);
        g.cash = 10_000.0;
        let lonely: Vec<usize> = (0..g.towns.len()).filter(|&i| !g.connected(i)).collect();
        let pair = lonely
            .iter()
            .flat_map(|&a| lonely.iter().map(move |&b| (a, b)))
            .find(|&(a, b)| a != b && g.towns[a].pos.dist(g.towns[b].pos) < MAX_LINK);
        if let Some((a, b)) = pair {
            assert_eq!(g.can_link(a, b), Err(BuildError::NotConnected));
        }
    }

    #[test]
    fn well_served_towns_grow_and_field_militia() {
        let mut g = cross();
        let t = &mut g.towns[1];
        t.pop = 20.0;
        t.rating = 1.0;
        let before = t.militia();
        for _ in 0..600 {
            g.towns[1].rating = 1.0;
            g.update_towns(0.1);
        }
        assert!(g.towns[1].pop > 40.0);
        assert!(g.towns[1].militia() > before);
        // Neglected towns shrink.
        g.towns[2].pop = 60.0;
        g.towns[2].rating = 0.0;
        g.update_towns(10.0);
        assert!(g.towns[2].pop < 60.0);
    }

    #[test]
    fn waves_escalate() {
        assert!(Game::wave_enemy_count(10) > Game::wave_enemy_count(1) * 3);
        assert!(Game::hp_scale(25) > Game::hp_scale(5) * 2.0);
        assert_eq!(Game::hp_scale(1), 1.0);
    }

    #[test]
    fn boss_every_fifth_wave() {
        let mut g = Game::new(5);
        while g.wave < 5 {
            g.start_wave();
        }
        assert!(g.enemies.iter().any(|e| e.kind == Kind::Boss));
    }

    #[test]
    fn level_up_offers_three_distinct_choices() {
        let mut g = Game::new(9);
        g.xp = 100.0;
        g.check_level();
        let c = g.choice.expect("level up");
        assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        let lvl = g.level;
        g.choose(0);
        assert!(g.level > lvl);
        // Paused while a choice is pending.
        g.choice = Some(c);
        let t = g.time;
        g.update(0.1, &idle());
        assert_eq!(g.time, t);
    }

    #[test]
    fn upgrades_apply() {
        let mut g = Game::new(9);
        g.choice = Some([Upgrade::Multishot, Upgrade::Armor, Upgrade::Speed]);
        g.choose(0);
        assert_eq!(g.stats.multishot, 2);
        g.choice = Some([Upgrade::Multishot, Upgrade::Armor, Upgrade::Speed]);
        g.hp = 10.0;
        g.choose(1);
        assert_eq!(g.hp, 130.0);
    }

    #[test]
    fn player_shoots_and_kills() {
        let mut g = Game::new(2);
        g.enemies.clear();
        let p = g.loco_pos() + V2::new(80.0, 0.0);
        g.spawn(Kind::Grunt, p, 5.0, Some(0));
        for _ in 0..60 {
            g.update(1.0 / 60.0, &idle());
        }
        assert!(g.kills >= 1);
        assert!(g.events.contains(&Event::Shot));
    }

    #[test]
    fn town_falls_and_enemies_retarget() {
        let mut g = cross();
        g.towns[1].hp = 1.0;
        g.spawn(Kind::Brute, g.towns[1].pos, 1e6, Some(1));
        g.update_enemies(0.5);
        assert!(!g.towns[1].alive);
        assert!(g.events.contains(&Event::TownLost(1)));
        g.update_enemies(0.1);
        assert_eq!(g.enemies[0].target, None);
        // Rebuilding a ruined station.
        g.cash = 500.0;
        assert!(g.upgrade_station(1).is_ok());
        assert!(g.towns[1].alive);
    }

    #[test]
    fn rescue_train_boosts_target() {
        let mut g = cross();
        g.target = 1;
        g.cash = 500.0;
        g.loco = Loco {
            edge: 3,
            s: 100.0,
            forward: true,
            speed: 0.0,
        };
        g.update(
            0.01,
            &Input {
                rescue: true,
                ..idle()
            },
        );
        assert!(g.rescue.is_some());
        for _ in 0..200 {
            g.update(1.0 / 60.0, &idle());
        }
        assert!(g.rescue.is_none());
        assert!(g.towns[1].boost > 0.0);
        assert!(g.rescue_cd > 0.0);
    }

    #[test]
    fn path_finds_route() {
        let g = cross();
        assert_eq!(g.path(1, 2), Some(vec![1, 0, 2]));
    }

    #[test]
    fn coins_are_magnetised() {
        let mut g = Game::new(4);
        let p = g.loco_pos();
        g.coins.push(Coin {
            pos: p + V2::new(50.0, 0.0),
            value: 10.0,
            life: 5.0,
        });
        let cash = g.cash;
        for _ in 0..30 {
            g.update_coins(1.0 / 60.0);
        }
        assert!(g.cash >= cash + 10.0);
    }

    #[test]
    fn losing_hq_hp_ends_game() {
        let mut g = Game::new(8);
        g.hp = 0.0;
        g.update(0.01, &idle());
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.events.contains(&Event::GameOver));
    }

    #[test]
    fn surviving_all_waves_wins() {
        let mut g = Game::new(8);
        g.wave = WAVES;
        g.wave_time = WAVE_TIME - 0.001;
        g.enemies.clear();
        g.update(0.01, &idle());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn autopilot_survives_early_waves() {
        // Smoke test: cruising with full throttle and picking upgrades never
        // panics and survives at least the first two waves.
        let mut g = Game::new(11);
        let input = Input {
            throttle: 1.0,
            ..idle()
        };
        for _ in 0..(60 * 60) {
            if g.choice.is_some() {
                g.choose(0);
            }
            g.update(1.0 / 60.0, &input);
            g.events.clear();
        }
        assert_eq!(g.phase, Phase::Playing);
        assert!(g.wave >= 2);
        assert!(g.kills > 0);
    }
}
