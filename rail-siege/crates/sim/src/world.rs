//! The world: map, stations, lines with their trains, and the fixed-step
//! simulation.

use crate::rng::Pcg32;
use crate::route::{self, Route};
use crate::station::{self, Station};
use crate::track::{Tile, TileMap};
use crate::train::{self, Phase, Stop, Train};

/// One simulation step: 60 steps per second.
pub const DT: f32 = 1.0 / 60.0;
pub const MAX_LINES: usize = 12;

/// A player command. The game sends commands, the world applies them.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Build a line between two stations over waypoints, with one train.
    Build { a: usize, b: usize, via: Vec<Tile> },
    /// Remove a line and its train.
    Remove(usize),
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
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::SameStation => "Pick a different station",
            Refusal::NeedsTown => "A line must end at the town",
            Refusal::NoRoute => "No route",
            Refusal::TooManyLines => "Line limit reached",
            Refusal::NoSuchLine => "No such line",
        }
    }
}

/// Something the front end can show or play a sound for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Built,
    Removed,
    /// A train unloaded cargo.
    Delivered {
        units: u32,
    },
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
}

pub struct World {
    pub map: TileMap,
    /// Station 0 is the town.
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
    pub rng: Pcg32,
    pub time: f32,
    pub events: Vec<Event>,
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
        };
        // The free start line: town to the nearest coal mine.
        let coal = w.nearest_coal();
        if let Ok(r) = w.plan(coal, 0, &[]) {
            w.add_line(coal, 0, Vec::new(), r);
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

    /// Applies a command now. The game also calls this while paused.
    pub fn apply(&mut self, cmd: &Command) -> Result<(), Refusal> {
        match cmd {
            Command::Build { a, b, via } => {
                if self.lines.len() >= MAX_LINES {
                    return Err(Refusal::TooManyLines);
                }
                let r = self.plan(*a, *b, via)?;
                let (ind, town) = if self.stations[*a].is_town() {
                    (*b, *a)
                } else {
                    (*a, *b)
                };
                self.add_line(ind, town, via.clone(), r);
                Ok(())
            }
            Command::Remove(i) => {
                if *i >= self.lines.len() {
                    return Err(Refusal::NoSuchLine);
                }
                self.lines.remove(*i);
                self.relay_track();
                self.events.push(Event::Removed);
                Ok(())
            }
        }
    }

    fn add_line(&mut self, a: usize, b: usize, via: Vec<Tile>, route: Route) {
        route.lay(&mut self.map);
        let train = Train::new(&route, 0, train::TRAIN_HP);
        self.lines.push(Line {
            a,
            b,
            via,
            route,
            train,
            delivered: 0,
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
        self.time += DT;
        for s in &mut self.stations {
            s.produce(DT);
        }
        self.run_trains();
    }

    fn run_trains(&mut self) {
        for li in 0..self.lines.len() {
            let line = &mut self.lines[li];
            let t = &mut line.train;
            match t.phase {
                Phase::Loading(waited) => {
                    let cap = train::CARGO_WAGONS as u32 * train::WAGON_CAPACITY;
                    t.cargo += self.stations[line.a].take(cap - t.cargo);
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
                    match t.drive(&line.route, &self.map, 1.0, DT) {
                        Some(Stop::Town) => {
                            let units = std::mem::take(&mut t.cargo);
                            line.delivered += units;
                            t.phase = Phase::Unloading(train::UNLOAD_TIME);
                            self.events.push(Event::Delivered { units });
                        }
                        Some(Stop::Industry) => {
                            t.reverse();
                            t.phase = Phase::Loading(0.0);
                        }
                        Some(Stop::Break) => {
                            t.reverse();
                            t.phase = if t.dir > 0.0 {
                                Phase::ToTown
                            } else {
                                Phase::ToIndustry
                            };
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
        for _ in 0..60 * 90 {
            w.step(&[]);
            for e in w.events.drain(..) {
                if let Event::Delivered { units } = e {
                    delivered += units;
                }
            }
        }
        assert!(delivered >= 40, "delivered {delivered}");
        assert_eq!(w.lines[0].delivered, delivered);
    }

    #[test]
    fn two_trains_share_the_stock() {
        let mut a = World::new(7);
        let mut b = World::new(7);
        let coal = b.lines[0].a;
        b.apply(&Command::Build {
            a: coal,
            b: 0,
            via: vec![],
        })
        .unwrap();
        for _ in 0..60 * 180 {
            a.step(&[]);
            b.step(&[]);
        }
        let one = a.lines[0].delivered;
        let two: u32 = b.lines.iter().map(|l| l.delivered).sum();
        let made = Industry::Coal.output() * 180.0;
        assert!(two as f32 <= made + 1.0, "{two} > {made}");
        assert!(two >= one);
    }
}
