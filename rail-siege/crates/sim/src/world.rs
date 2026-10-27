//! The world: map, stations, lines with their trains, and the fixed-step
//! simulation.

use crate::cards::{self, Card, Levels};
use crate::rng::Pcg32;
use crate::route::{self, Route};
use crate::station::{self, Station};
use crate::track::{Tile, TileMap};
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

/// A player command. The game sends commands, the world applies them.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Build a line between two stations over waypoints, with one train.
    Build { a: usize, b: usize, via: Vec<Tile> },
    /// Remove a line and its train.
    Remove(usize),
    /// Pick card `i` of the level-up choice.
    Pick(usize),
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Running,
    /// Paused: the player picks one of three cards.
    LevelUp([Card; 3]),
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
        }
    }

    fn pick_card(&mut self, i: usize) -> Result<(), Refusal> {
        let State::LevelUp(cards) = self.state else {
            return Err(Refusal::NoCard);
        };
        let card = *cards.get(i).ok_or(Refusal::NoCard)?;
        match card {
            Card::Item(item) => self.levels.raise(item),
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
    }

    pub fn step(&mut self, cmds: &[Command]) {
        for c in cmds {
            let _ = self.apply(c);
        }
        if self.state != State::Running {
            return;
        }
        self.time += DT;
        for s in &mut self.stations {
            s.produce(DT);
        }
        self.run_trains();
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
    fn run(w: &mut World, steps: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..steps {
            let pick = matches!(w.state, State::LevelUp(_));
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
}
