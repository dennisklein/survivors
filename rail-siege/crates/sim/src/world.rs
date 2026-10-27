//! The world: map, stations, lines with their trains, and the fixed-step
//! simulation.

use crate::cards::{self, Card, Item, Levels};
use crate::director::{self, Director};
use crate::enemy::{self, Enemy, EnemyKind};
use crate::grid::Grid;
use crate::math::V2;
use crate::rng::Pcg32;
use crate::route::{self, Route};
use crate::station::{self, MAP_H, MAP_W, Station};
use crate::track::{Tile, TileMap, tile_centre};
use crate::train::{self, Phase, Stop, Train};

/// One simulation step: 60 steps per second.
pub const DT: f32 = 1.0 / 60.0;
pub const MAX_LINES: usize = 12;
pub const START_MONEY: f32 = 100.0;
/// Cost of a tile of new track.
pub const TILE_COST: f32 = 5.0;
pub const TRAIN_COST: f32 = 40.0;
/// Part of the build cost that removing a line pays back.
pub const REFUND: f32 = 0.5;
pub const REPAIR_COST: f32 = 10.0;
pub const TOWN_HP: f32 = 100.0;
pub const WALLS_HP: f32 = 25.0;
/// Ground enemies this close to the town centre hit the town.
pub const TOWN_RADIUS: f32 = 1.9;
/// A saboteur breaks its track tile after this long on it.
pub const SABOTAGE_TIME: f32 = 2.0;
/// Residents go for a train car this close.
pub const RESIDENT_AGGRO: f32 = 2.0;
/// This many enemies near a station stop the industry.
pub const OVERRUN_COUNT: usize = 6;
pub const OVERRUN_RADIUS: f32 = 2.0;
pub const CAR_RADIUS: f32 = 0.4;
/// Ram damage is this factor times the train speed.
pub const RAM_FACTOR: f32 = 10.0;
pub const RAM_COOLDOWN: f32 = 0.5;

/// A player command. The game sends commands, the world applies them.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Build a line between two stations over waypoints, with one train.
    Build { a: usize, b: usize, via: Vec<Tile> },
    /// Remove a line and its train.
    Remove(usize),
    /// Pick card `i` of the level-up choice.
    Pick(usize),
    /// Repair broken track on a tile.
    Repair(Tile),
}

/// Why a command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    SameStation,
    /// A line must end at the town.
    NeedsTown,
    NoRoute,
    TooManyLines,
    NoSuchLine,
    NoMoney,
    NoCard,
    NotBroken,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::SameStation => "Pick a different station",
            Refusal::NeedsTown => "A line must end at the town",
            Refusal::NoRoute => "No route",
            Refusal::TooManyLines => "Line limit reached",
            Refusal::NoSuchLine => "No such line",
            Refusal::NoMoney => "Not enough money",
            Refusal::NoCard => "No card to pick",
            Refusal::NotBroken => "Nothing to repair",
        }
    }
}

/// Something the front end can show or play a sound for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Built,
    Removed,
    /// A train unloaded cargo and earned money.
    Delivered {
        units: u32,
        pay: f32,
    },
    LevelUp,
    Picked,
    Killed {
        at: V2,
    },
    TownHit,
    TrackBroken(Tile),
    Repaired,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Running,
    /// Paused: the player picks one of three cards.
    LevelUp([Card; 3]),
    /// The town has no HP left.
    Lost,
}

/// A line from an industry to the town with its train.
#[derive(Clone, Debug)]
pub struct Line {
    /// The industry station.
    pub a: usize,
    /// The town station.
    pub b: usize,
    pub via: Vec<Tile>,
    pub route: Route,
    pub train: Train,
    /// Units delivered on this line.
    pub delivered: u32,
    /// Money earned on this line.
    pub earned: f32,
    /// What the line cost to build.
    pub cost: f32,
    /// Time when the line was built.
    pub built: f32,
}

pub struct World {
    pub map: TileMap,
    /// Station 0 is the town.
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
    pub rng: Pcg32,
    pub time: f32,
    pub events: Vec<Event>,
    pub state: State,
    pub money: f32,
    pub xp: u32,
    pub level: u32,
    pub levels: Levels,
    /// Level-ups that still wait for a card.
    pending: u32,
    pub enemies: Vec<Enemy>,
    pub town_hp: f32,
    pub town_max: f32,
    pub kills: u32,
    /// Broken track tiles and how long they are broken.
    pub breaks: Vec<(Tile, f32)>,
    director: Director,
    grid: Grid,
    next_id: u32,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let (map, stations) = station::generate(&mut rng);
        let mut w = Self {
            map,
            stations,
            lines: Vec::new(),
            rng,
            time: 0.0,
            events: Vec::new(),
            state: State::Running,
            money: START_MONEY,
            xp: 0,
            level: 1,
            levels: Levels::default(),
            pending: 0,
            enemies: Vec::new(),
            town_hp: TOWN_HP,
            town_max: TOWN_HP,
            kills: 0,
            breaks: Vec::new(),
            director: Director::default(),
            grid: Grid::new(MAP_W, MAP_H),
            next_id: 0,
        };
        // The free start line: town to the nearest coal mine.
        let coal = w.nearest_coal();
        if let Ok(r) = w.plan(coal, 0, &[]) {
            w.add_line(coal, 0, Vec::new(), r, 0.0);
        }
        w.events.clear();
        w
    }

    fn nearest_coal(&self) -> usize {
        let town = self.stations[0].centre();
        (1..self.stations.len())
            .filter(|&i| self.stations[i].industry() == Some(station::Industry::Coal))
            .min_by(|&a, &b| {
                let da = self.stations[a].centre().dist(town);
                let db = self.stations[b].centre().dist(town);
                da.total_cmp(&db)
            })
            .unwrap_or(1)
    }

    /// The station that a click on `tile` selects.
    pub fn station_at(&self, tile: Tile) -> Option<usize> {
        self.stations.iter().position(|s| s.covers(tile))
    }

    /// Finds the route for a line from `a` to `b` over `via`. The route
    /// always runs from the industry to the town.
    pub fn plan(&self, a: usize, b: usize, via: &[Tile]) -> Result<Route, Refusal> {
        if a == b {
            return Err(Refusal::SameStation);
        }
        let (ind, town, via): (usize, usize, Vec<Tile>) =
            match (self.stations[a].is_town(), self.stations[b].is_town()) {
                (false, true) => (a, b, via.to_vec()),
                (true, false) => (b, a, via.iter().rev().copied().collect()),
                _ => return Err(Refusal::NeedsTown),
            };
        let from = self.stations[ind].platforms();
        let to = self.stations[town].platforms();
        let blocked: Vec<Tile> = self
            .stations
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != ind && i != town)
            .map(|(_, s)| s.tile)
            .collect();
        route::find(&self.map, &from, &to, &via, &blocked).ok_or(Refusal::NoRoute)
    }

    /// A route from station `from` over `via` to any tile, for the ghost
    /// route that follows the mouse before the second station is picked.
    pub fn sketch(&self, from: usize, via: &[Tile], end: Tile) -> Option<Route> {
        let blocked: Vec<Tile> = self
            .stations
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != from)
            .map(|(_, s)| s.tile)
            .collect();
        let start = self.stations[from].platforms();
        route::find(&self.map, &start, &[end], via, &blocked)
    }

    /// Money to build a line on `route`, with its train.
    pub fn cost(&self, route: &Route) -> f32 {
        (route.new_tiles as f32 * TILE_COST * self.levels.build_factor()).ceil() + TRAIN_COST
    }

    /// XP needed for the next level.
    pub fn xp_needed(&self) -> u32 {
        cards::xp_needed(self.level)
    }

    /// Units a train carries.
    pub fn capacity(&self) -> u32 {
        let base = (train::CARGO_WAGONS as u32 * train::WAGON_CAPACITY) as f32;
        (base * self.levels.capacity_bonus()).round() as u32
    }

    /// Applies a command now. The game also calls this while paused.
    pub fn apply(&mut self, cmd: &Command) -> Result<(), Refusal> {
        match cmd {
            Command::Build { a, b, via } => {
                if self.lines.len() >= MAX_LINES {
                    return Err(Refusal::TooManyLines);
                }
                let r = self.plan(*a, *b, via)?;
                let cost = self.cost(&r);
                if cost > self.money {
                    return Err(Refusal::NoMoney);
                }
                self.money -= cost;
                let (ind, town) = if self.stations[*a].is_town() {
                    (*b, *a)
                } else {
                    (*a, *b)
                };
                self.add_line(ind, town, via.clone(), r, cost);
                Ok(())
            }
            Command::Remove(i) => {
                if *i >= self.lines.len() {
                    return Err(Refusal::NoSuchLine);
                }
                let line = self.lines.remove(*i);
                self.money += (line.cost * REFUND).floor();
                self.relay_track();
                self.events.push(Event::Removed);
                Ok(())
            }
            Command::Pick(i) => self.pick_card(*i),
            Command::Repair(t) => {
                if !self.map.is_broken(*t) {
                    return Err(Refusal::NotBroken);
                }
                let cost = self.repair_cost();
                if cost > self.money {
                    return Err(Refusal::NoMoney);
                }
                self.money -= cost;
                self.repair(*t);
                Ok(())
            }
        }
    }

    pub fn repair_cost(&self) -> f32 {
        (REPAIR_COST * self.levels.build_factor()).ceil()
    }

    fn repair(&mut self, t: Tile) {
        self.map.set_broken(t, false);
        self.breaks.retain(|b| b.0 != t);
        self.events.push(Event::Repaired);
    }

    fn break_track(&mut self, t: Tile) {
        if self.map.set_broken(t, true) {
            self.breaks.push((t, 0.0));
            self.events.push(Event::TrackBroken(t));
        }
    }

    fn pick_card(&mut self, i: usize) -> Result<(), Refusal> {
        let State::LevelUp(cards) = self.state else {
            return Err(Refusal::NoCard);
        };
        let card = *cards.get(i).ok_or(Refusal::NoCard)?;
        match card {
            Card::Item(item) => {
                self.levels.raise(item);
                if item == Item::TownWalls {
                    self.town_max += WALLS_HP;
                    self.town_hp = (self.town_hp + WALLS_HP).min(self.town_max);
                }
            }
            Card::Subsidy => self.money += cards::SUBSIDY,
        }
        self.pending -= 1;
        self.state = State::Running;
        self.events.push(Event::Picked);
        self.offer_cards();
        Ok(())
    }

    fn offer_cards(&mut self) {
        if self.pending > 0 && self.state == State::Running {
            self.state = State::LevelUp(cards::roll(&self.levels, &mut self.rng));
            self.events.push(Event::LevelUp);
        }
    }

    /// Pays for `units` carried `dist` tiles and adds the XP.
    fn pay(&mut self, line: usize, units: u32, dist: f32) {
        let Some(kind) = self.stations[self.lines[line].a].industry() else {
            return;
        };
        let pay = units as f32 * kind.rate() * dist;
        self.money += pay;
        let l = &mut self.lines[line];
        l.delivered += units;
        l.earned += pay;
        self.xp += units;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            self.pending += 1;
        }
        self.events.push(Event::Delivered { units, pay });
        self.offer_cards();
    }

    fn add_line(&mut self, a: usize, b: usize, via: Vec<Tile>, route: Route, cost: f32) {
        route.lay(&mut self.map);
        let train = Train::new(&route, 0, train::TRAIN_HP);
        self.lines.push(Line {
            a,
            b,
            via,
            route,
            train,
            delivered: 0,
            earned: 0.0,
            cost,
            built: self.time,
        });
        self.events.push(Event::Built);
    }

    /// Lays the track of all lines again, after a line was removed.
    fn relay_track(&mut self) {
        self.map.clear_all_track();
        for l in &self.lines {
            l.route.lay(&mut self.map);
        }
        self.map.drop_stale_breaks();
        let map = &self.map;
        self.breaks.retain(|b| map.is_broken(b.0));
    }

    pub fn step(&mut self, cmds: &[Command]) {
        for c in cmds {
            let _ = self.apply(c);
        }
        if self.state != State::Running {
            return;
        }
        self.time += DT;
        self.check_overrun();
        for s in &mut self.stations {
            s.produce(DT);
        }
        self.run_trains();
        self.spawn();
        self.move_enemies();
        self.ram();
        self.hit_town();
        self.remove_dead();
        self.repair_crew();
    }

    pub fn damage_town(&mut self, amount: f32) {
        self.town_hp -= amount;
        self.events.push(Event::TownHit);
        if self.town_hp <= 0.0 {
            self.town_hp = 0.0;
            if self.state != State::Lost {
                self.state = State::Lost;
                self.events.push(Event::Lost);
            }
        }
    }

    pub fn spawn_enemy(&mut self, kind: EnemyKind, at: V2) {
        let e = Enemy::new(self.next_id, kind, at, director::hp_scale(self.time));
        self.next_id += 1;
        self.enemies.push(e);
    }

    fn size(&self) -> V2 {
        V2::new(self.map.width() as f32, self.map.height() as f32)
    }

    fn spawn(&mut self) {
        let live = self.enemies.len();
        let size = self.size();
        let Some(g) = self.director.tick(DT, self.time, live, size, &mut self.rng) else {
            return;
        };
        for _ in 0..g.count {
            let jitter = V2::new(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
            let p = g.at + jitter;
            let p = V2::new(p.x.clamp(0.1, size.x - 0.1), p.y.clamp(0.1, size.y - 0.1));
            self.spawn_enemy(g.kind, p);
        }
    }

    /// Positions of all train cars on the map.
    fn car_points(&self) -> Vec<V2> {
        self.lines
            .iter()
            .filter(|l| !l.train.is_wrecked())
            .flat_map(|l| l.train.cars().iter().map(|c| c.pos))
            .collect()
    }

    fn move_enemies(&mut self) {
        let cars = self.car_points();
        let town = tile_centre(station::TOWN);
        let size = self.size();
        // Unbroken track tiles, for the saboteurs.
        let mut track = Vec::new();
        for y in 0..self.map.height() {
            for x in 0..self.map.width() {
                if self.map.track(x, y) != 0 && !self.map.is_broken((x, y)) {
                    track.push((x, y));
                }
            }
        }
        let nearest = |p: V2, pts: &[V2]| {
            pts.iter()
                .copied()
                .min_by(|a, b| a.dist_sq(p).total_cmp(&b.dist_sq(p)))
        };
        let mut broken = Vec::new();
        for e in &mut self.enemies {
            let target = match e.kind {
                EnemyKind::Resident => nearest(e.pos, &cars)
                    .filter(|c| c.dist(e.pos) < RESIDENT_AGGRO)
                    .unwrap_or(town),
                EnemyKind::Bus => nearest(e.pos, &cars).unwrap_or(town),
                EnemyKind::Saboteur => {
                    if e.goal.is_none_or(|g| !track.contains(&g)) {
                        e.goal = track.iter().copied().min_by(|a, b| {
                            let da = tile_centre(*a).dist_sq(e.pos);
                            let db = tile_centre(*b).dist_sq(e.pos);
                            da.total_cmp(&db)
                        });
                        e.work = 0.0;
                    }
                    match e.goal {
                        Some(g) => {
                            let c = tile_centre(g);
                            if c.dist(e.pos) < 0.35 {
                                e.work += DT;
                                if e.work >= SABOTAGE_TIME {
                                    broken.push(g);
                                    e.goal = None;
                                }
                            }
                            c
                        }
                        None => town,
                    }
                }
            };
            e.think(target, DT);
            e.pos = V2::new(e.pos.x.clamp(0.0, size.x), e.pos.y.clamp(0.0, size.y));
        }
        for t in broken {
            self.break_track(t);
        }
        self.rebuild_grid();
        enemy::separate(&mut self.enemies, &self.grid);
    }

    fn rebuild_grid(&mut self) {
        let pts: Vec<V2> = self.enemies.iter().map(|e| e.pos).collect();
        self.grid.build(&pts);
    }

    /// The lead car of a moving train hits enemies in its way.
    fn ram(&mut self) {
        for l in &self.lines {
            let t = &l.train;
            if !t.is_moving() || t.v < 0.5 {
                continue;
            }
            let car = *t.lead();
            let enemies = &mut self.enemies;
            self.grid.query(car.pos, CAR_RADIUS + 1.5, |i| {
                let e = &mut enemies[i];
                let reach = CAR_RADIUS + e.radius();
                let d = e.pos - car.pos;
                if d.len_sq() >= reach * reach || e.ram_cd > 0.0 {
                    return;
                }
                e.hit(RAM_FACTOR * t.v);
                e.ram_cd = RAM_COOLDOWN;
                e.push = d.norm() * 6.0;
            });
        }
    }

    /// Ground enemies that reach the town hit it and are gone.
    fn hit_town(&mut self) {
        let town = tile_centre(station::TOWN);
        let mut dmg = 0.0;
        self.enemies.retain(|e| {
            if !e.kind.flies() && e.pos.dist(town) < TOWN_RADIUS {
                dmg += e.stats().touch;
                false
            } else {
                true
            }
        });
        if dmg > 0.0 {
            self.damage_town(dmg);
        }
    }

    fn remove_dead(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                self.kills += 1;
                self.events.push(Event::Killed { at: e.pos });
            } else {
                i += 1;
            }
        }
    }

    fn check_overrun(&mut self) {
        self.rebuild_grid();
        for s in &mut self.stations {
            if s.is_town() {
                continue;
            }
            let c = s.centre();
            let mut n = 0;
            let enemies = &self.enemies;
            self.grid.query(c, OVERRUN_RADIUS, |i| {
                let e = &enemies[i];
                if e.pos.dist_sq(c) < OVERRUN_RADIUS * OVERRUN_RADIUS {
                    n += 1;
                }
            });
            s.overrun = n >= OVERRUN_COUNT;
        }
    }

    fn repair_crew(&mut self) {
        for b in &mut self.breaks {
            b.1 += DT;
        }
        if let Some(limit) = self.levels.repair_time() {
            let due: Vec<Tile> = self
                .breaks
                .iter()
                .filter(|b| b.1 >= limit)
                .map(|b| b.0)
                .collect();
            for t in due {
                self.repair(t);
            }
        }
    }

    fn run_trains(&mut self) {
        let cap = self.capacity();
        let bonus = self.levels.speed_bonus();
        for li in 0..self.lines.len() {
            let line = &mut self.lines[li];
            let t = &mut line.train;
            match t.phase {
                Phase::Loading(waited) => {
                    let want = cap.saturating_sub(t.cargo);
                    t.cargo += self.stations[line.a].take(want);
                    let waited = waited + DT;
                    t.phase = if t.cargo >= cap || waited >= train::LOAD_WAIT {
                        Phase::ToTown
                    } else {
                        Phase::Loading(waited)
                    };
                }
                Phase::Unloading(left) => {
                    t.phase = if left > DT {
                        Phase::Unloading(left - DT)
                    } else {
                        t.reverse();
                        Phase::ToIndustry
                    };
                }
                Phase::Wrecked(left) => {
                    if left > DT {
                        t.phase = Phase::Wrecked(left - DT);
                    } else {
                        t.respawn(&line.route, train::TRAIN_HP);
                    }
                }
                Phase::ToTown | Phase::ToIndustry => {
                    match t.drive(&line.route, &self.map, bonus, DT) {
                        Some(Stop::Town) => {
                            let units = std::mem::take(&mut t.cargo);
                            let dist = line.route.len;
                            t.phase = Phase::Unloading(train::UNLOAD_TIME);
                            self.pay(li, units, dist);
                        }
                        Some(Stop::Industry) => {
                            t.reverse();
                            t.phase = Phase::Loading(0.0);
                        }
                        Some(Stop::Break) => {
                            // Broken track makes the line shorter: unload
                            // here, for the distance driven.
                            let units = std::mem::take(&mut t.cargo);
                            let dist = t.lead_s();
                            t.reverse();
                            t.phase = if t.dir > 0.0 {
                                Phase::ToTown
                            } else {
                                Phase::ToIndustry
                            };
                            if units > 0 {
                                self.pay(li, units, dist);
                            }
                        }
                        None => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::station::Industry;

    /// Steps the world and picks the first card at every level-up.
    /// The town cannot fall.
    fn run(w: &mut World, steps: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..steps {
            let pick = matches!(w.state, State::LevelUp(_));
            w.town_hp = TOWN_HP;
            w.step(if pick { &[Command::Pick(0)] } else { &[] });
            events.append(&mut w.events);
        }
        events
    }

    fn industry(w: &World, kind: Industry) -> usize {
        w.stations
            .iter()
            .position(|s| s.industry() == Some(kind))
            .unwrap()
    }

    #[test]
    fn starts_with_a_free_coal_line() {
        let w = World::new(3);
        assert_eq!(w.lines.len(), 1);
        let l = &w.lines[0];
        assert_eq!(w.stations[l.a].industry(), Some(Industry::Coal));
        assert!(w.stations[l.b].is_town());
        assert_eq!(l.route.first(), w.stations[l.a].tile);
        assert!(w.stations[0].platforms().contains(&l.route.last()));
        assert!(w.map.track(l.route.first().0, l.route.first().1) != 0);
    }

    #[test]
    fn build_and_remove_lines() {
        let mut w = World::new(4);
        w.money = 1000.0;
        let oil = industry(&w, Industry::Oil);
        assert_eq!(
            w.apply(&Command::Build {
                a: oil,
                b: oil,
                via: vec![]
            }),
            Err(Refusal::SameStation)
        );
        let iron = industry(&w, Industry::Iron);
        assert_eq!(
            w.apply(&Command::Build {
                a: oil,
                b: iron,
                via: vec![]
            }),
            Err(Refusal::NeedsTown)
        );
        // Town first: the route still runs from the industry.
        w.apply(&Command::Build {
            a: 0,
            b: oil,
            via: vec![],
        })
        .unwrap();
        assert_eq!(w.lines[1].a, oil);
        assert_eq!(w.lines[1].route.first(), w.stations[oil].tile);
        let tile = w.stations[oil].tile;
        w.apply(&Command::Remove(1)).unwrap();
        assert_eq!(w.lines.len(), 1);
        assert_eq!(w.map.track(tile.0, tile.1), 0, "track of the line is gone");
        assert!(w.apply(&Command::Remove(5)).is_err());
    }

    #[test]
    fn line_limit() {
        let mut w = World::new(5);
        w.money = 1000.0;
        let coal = w.lines[0].a;
        for _ in 1..MAX_LINES {
            w.apply(&Command::Build {
                a: coal,
                b: 0,
                via: vec![],
            })
            .unwrap();
        }
        assert_eq!(
            w.apply(&Command::Build {
                a: coal,
                b: 0,
                via: vec![]
            }),
            Err(Refusal::TooManyLines)
        );
    }

    #[test]
    fn trains_shuttle_and_deliver() {
        let mut w = World::new(6);
        let mut delivered = 0;
        for e in run(&mut w, 60 * 90) {
            if let Event::Delivered { units, .. } = e {
                delivered += units;
            }
        }
        assert!(delivered >= 40, "delivered {delivered}");
        assert_eq!(w.lines[0].delivered, delivered);
        assert!(w.money > START_MONEY);
    }

    #[test]
    fn pay_formula_and_xp() {
        let mut w = World::new(8);
        let len = w.lines[0].route.len;
        let before = w.money;
        w.pay(0, 20, len);
        let want = 20.0 * Industry::Coal.rate() * len;
        assert!((w.money - before - want).abs() < 1e-3);
        assert_eq!(w.xp, 20);
        assert_eq!(w.state, State::Running);
        w.pay(0, 10, len);
        assert_eq!(w.level, 2);
        assert_eq!(w.xp, 5);
        assert!(matches!(w.state, State::LevelUp(_)));
        // The world stops while the cards show.
        let t = w.time;
        w.step(&[]);
        assert_eq!(w.time, t);
        w.step(&[Command::Pick(0)]);
        assert_eq!(w.state, State::Running);
        assert_eq!(w.apply(&Command::Pick(0)), Err(Refusal::NoCard));
    }

    #[test]
    fn building_costs_money_and_removing_refunds_half() {
        let mut w = World::new(9);
        let oil = industry(&w, Industry::Oil);
        let r = w.plan(oil, 0, &[]).unwrap();
        let cost = w.cost(&r);
        assert_eq!(cost, r.new_tiles as f32 * TILE_COST + TRAIN_COST);
        let build = Command::Build {
            a: oil,
            b: 0,
            via: vec![],
        };
        w.money = cost - 1.0;
        assert_eq!(w.apply(&build), Err(Refusal::NoMoney));
        w.money = cost;
        w.apply(&build).unwrap();
        assert_eq!(w.money, 0.0);
        // The same line again only needs a train.
        let again = w.plan(oil, 0, &[]).unwrap();
        assert_eq!(w.cost(&again), TRAIN_COST);
        w.apply(&Command::Remove(1)).unwrap();
        assert_eq!(w.money, (cost * REFUND).floor());
    }

    #[test]
    fn break_on_the_way_pays_less() {
        let mut w = World::new(10);
        let len = w.lines[0].route.len;
        let mid = w.lines[0].route.pieces[w.lines[0].route.pieces.len() / 2].tile;
        w.map.set_broken(mid, true);
        let mut paid = Vec::new();
        for e in run(&mut w, 60 * 40) {
            if let Event::Delivered { units, pay } = e {
                paid.push(pay / units as f32);
            }
        }
        let full = Industry::Coal.rate() * len;
        assert!(!paid.is_empty());
        assert!(paid.iter().all(|&p| p < full * 0.8), "{paid:?} vs {full}");
    }

    #[test]
    fn two_trains_share_the_stock() {
        let mut a = World::new(7);
        let mut b = World::new(7);
        let coal = b.lines[0].a;
        b.money = 1000.0;
        b.apply(&Command::Build {
            a: coal,
            b: 0,
            via: vec![],
        })
        .unwrap();
        run(&mut a, 60 * 180);
        run(&mut b, 60 * 180);
        let one = a.lines[0].delivered;
        let two: u32 = b.lines.iter().map(|l| l.delivered).sum();
        let made = Industry::Coal.output() * 180.0;
        assert!(two as f32 <= made + 1.0, "{two} > {made}");
        assert!(two >= one);
    }

    #[test]
    fn saboteur_breaks_track_and_repair_fixes_it() {
        let mut w = World::new(11);
        let tile = w.lines[0].route.pieces[3].tile;
        w.spawn_enemy(EnemyKind::Saboteur, tile_centre(tile) + V2::new(0.2, 0.0));
        let mut broke = None;
        for _ in 0..60 * 4 {
            w.step(&[]);
            w.enemies.retain(|e| e.kind == EnemyKind::Saboteur);
            for e in w.events.drain(..) {
                if let Event::TrackBroken(t) = e {
                    broke = Some(t);
                }
            }
        }
        let t = broke.expect("track broke");
        assert!(w.map.is_broken(t));
        w.money = 5.0;
        assert_eq!(w.apply(&Command::Repair(t)), Err(Refusal::NoMoney));
        w.money = 100.0;
        w.apply(&Command::Repair(t)).unwrap();
        assert!(!w.map.is_broken(t));
        assert_eq!(w.money, 100.0 - REPAIR_COST);
        assert_eq!(w.apply(&Command::Repair(t)), Err(Refusal::NotBroken));
    }

    #[test]
    fn repair_crew_fixes_breaks_by_itself() {
        let mut w = World::new(12);
        w.levels.raise(Item::RepairCrew);
        let tile = w.lines[0].route.pieces[2].tile;
        w.break_track(tile);
        for _ in 0..60 * 7 {
            w.step(&[]);
        }
        assert!(w.map.is_broken(tile));
        for _ in 0..60 * 2 {
            w.step(&[]);
        }
        assert!(!w.map.is_broken(tile));
    }

    #[test]
    fn crowd_overruns_an_industry() {
        let mut w = World::new(13);
        let s = industry(&w, Industry::Oil);
        let c = w.stations[s].centre();
        for k in 0..OVERRUN_COUNT {
            w.spawn_enemy(EnemyKind::Resident, c + V2::new(0.1 * k as f32, 0.0));
        }
        w.step(&[]);
        assert!(w.stations[s].overrun);
        w.enemies.clear();
        w.step(&[]);
        assert!(!w.stations[s].overrun);
    }

    #[test]
    fn residents_hit_the_town_and_the_town_falls() {
        let mut w = World::new(14);
        let town = tile_centre(station::TOWN);
        w.spawn_enemy(EnemyKind::Resident, town + V2::new(0.5, 0.0));
        w.step(&[]);
        assert_eq!(w.town_hp, TOWN_HP - EnemyKind::Resident.stats().touch);
        w.town_hp = 1.0;
        w.spawn_enemy(EnemyKind::Resident, town);
        w.step(&[]);
        assert_eq!(w.state, State::Lost);
        let t = w.time;
        w.step(&[]);
        assert_eq!(w.time, t, "a lost run stops");
    }

    #[test]
    fn town_walls_raise_and_heal_town_hp() {
        let mut w = World::new(15);
        w.town_hp = 50.0;
        w.state = State::LevelUp([Card::Item(Item::TownWalls); 3]);
        w.pending = 1;
        w.apply(&Command::Pick(1)).unwrap();
        assert_eq!(w.town_max, TOWN_HP + WALLS_HP);
        assert_eq!(w.town_hp, 75.0);
    }

    #[test]
    fn ram_kills_residents_on_the_track() {
        let mut w = World::new(16);
        let mut killed = 0;
        for _ in 0..60 * 30 {
            if w.time > 3.0 && w.lines[0].train.is_moving() && w.enemies.len() < 3 {
                let t = &w.lines[0].train;
                let ahead = t.lead().pos + t.lead().heading * (t.dir * 0.6);
                w.spawn_enemy(EnemyKind::Resident, ahead);
            }
            w.town_hp = TOWN_HP;
            w.step(&[]);
            killed += w
                .events
                .drain(..)
                .filter(|e| matches!(e, Event::Killed { .. }))
                .count();
        }
        assert!(killed > 0);
    }

    #[test]
    fn same_seed_and_commands_give_the_same_run() {
        let play = |seed| {
            let mut w = World::new(seed);
            w.money = 1000.0;
            let oil = industry(&w, Industry::Oil);
            for i in 0..60 * 240 {
                let mut cmds = Vec::new();
                if i == 300 {
                    cmds.push(Command::Build {
                        a: oil,
                        b: 0,
                        via: vec![],
                    });
                }
                if matches!(w.state, State::LevelUp(_)) {
                    cmds.push(Command::Pick(i % 3));
                }
                w.town_hp = TOWN_HP;
                w.step(&cmds);
            }
            (
                w.money,
                w.kills,
                w.enemies.iter().map(|e| e.pos).collect::<Vec<_>>(),
            )
        };
        let (a, b) = (play(21), play(21));
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
        assert_eq!(a.2, b.2);
        assert!(!a.2.is_empty());
    }
}
