//! A push-pull train that shuttles on the route of its line.
//!
//! The route runs from the industry (distance 0) to the town (distance
//! `len`). The locomotive is always at the town end of the train: it pulls
//! to the town and pushes back to the industry, so there is no turning
//! loop. `s` is the route distance of the locomotive. Car `k` is `k *
//! CAR_GAP` behind it, so every car stays on the rails, also in curves.

use crate::math::V2;
use crate::route::Route;
use crate::track::TileMap;

pub const CAR_GAP: f32 = 0.9;
/// Top speed in tiles per second, before wagon drag.
pub const TOP_SPEED: f32 = 4.0;
/// Speed loss per wagon.
pub const WAGON_DRAG: f32 = 0.03;
pub const ACCEL: f32 = 2.0;
pub const BRAKE: f32 = 3.0;
/// Trains slow to this part of the top speed in curves.
pub const CURVE_FACTOR: f32 = 0.75;
pub const CARGO_WAGONS: usize = 2;
pub const WAGON_CAPACITY: u32 = 10;
/// A train waits at most this long for cargo.
pub const LOAD_WAIT: f32 = 2.0;
pub const UNLOAD_TIME: f32 = 1.0;
pub const TRAIN_HP: f32 = 60.0;
/// A wrecked train is off the line for this long.
pub const WRECK_TIME: f32 = 10.0;
/// Trains stop this far before a broken tile.
const BREAK_MARGIN: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// At the industry. Takes cargo for at most `LOAD_WAIT` seconds.
    Loading(f32),
    ToTown,
    /// At the town, for `UNLOAD_TIME` seconds.
    Unloading(f32),
    ToIndustry,
    /// Off the line. Comes back at the industry when the time runs out.
    Wrecked(f32),
}

/// Why a moving train stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Town,
    Industry,
    /// Broken track ahead.
    Break,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Car {
    pub pos: V2,
    /// Unit heading toward the town end of the route.
    pub heading: V2,
}

#[derive(Clone, Debug)]
pub struct Train {
    /// Route distance of the locomotive.
    pub s: f32,
    /// +1: to the town, -1: to the industry.
    pub dir: f32,
    pub v: f32,
    pub phase: Phase,
    pub cargo: u32,
    pub hp: f32,
    /// Car 0 is the locomotive, then the cargo wagons, then the weapon wagons.
    cars: Vec<Car>,
}

impl Train {
    /// A new train at the industry end of `route`, ready to load.
    pub fn new(route: &Route, weapons: usize, hp: f32) -> Self {
        let mut t = Self {
            s: 0.0,
            dir: 1.0,
            v: 0.0,
            phase: Phase::Loading(0.0),
            cargo: 0,
            hp,
            cars: vec![Car::default(); 1 + CARGO_WAGONS + weapons],
        };
        t.respawn(route, hp);
        t
    }

    /// Puts the train back at the industry, empty and repaired.
    pub fn respawn(&mut self, route: &Route, hp: f32) {
        self.s = (0.5 + self.length()).min(route.len);
        self.dir = 1.0;
        self.v = 0.0;
        self.phase = Phase::Loading(0.0);
        self.cargo = 0;
        self.hp = hp;
        self.place(route);
    }

    /// Distance from the locomotive to the last car.
    pub fn length(&self) -> f32 {
        (self.cars.len() - 1) as f32 * CAR_GAP
    }

    pub fn wagons(&self) -> usize {
        self.cars.len() - 1
    }

    pub fn set_weapons(&mut self, n: usize, route: &Route) {
        self.cars.resize(1 + CARGO_WAGONS + n, Car::default());
        self.s = self.s.max((0.5 + self.length()).min(route.len));
        self.place(route);
    }

    pub fn cars(&self) -> &[Car] {
        &self.cars
    }

    /// The car at the front in the direction of travel.
    pub fn lead(&self) -> &Car {
        if self.dir > 0.0 {
            &self.cars[0]
        } else {
            &self.cars[self.cars.len() - 1]
        }
    }

    pub fn is_wrecked(&self) -> bool {
        matches!(self.phase, Phase::Wrecked(_))
    }

    pub fn is_moving(&self) -> bool {
        matches!(self.phase, Phase::ToTown | Phase::ToIndustry)
    }

    /// Route distance of the lead car.
    fn lead_s(&self) -> f32 {
        if self.dir > 0.0 {
            self.s
        } else {
            self.s - self.length()
        }
    }

    /// Where the lead car must stop: the end of the route or the edge of
    /// the first broken tile ahead.
    fn target(&self, route: &Route, map: &TileMap) -> (f32, Stop) {
        let lead = self.lead_s();
        let i = route.index_at(lead);
        if self.dir > 0.0 {
            for j in i + 1..route.pieces.len() {
                let t = route.pieces[j].tile;
                if map.is_broken(t) {
                    return (route.start_of(j) - BREAK_MARGIN, Stop::Break);
                }
            }
            (route.len - 0.5, Stop::Town)
        } else {
            for j in (0..i).rev() {
                let t = route.pieces[j].tile;
                if map.is_broken(t) {
                    let end = route.start_of(j) + route.pieces[j].len();
                    return (end + BREAK_MARGIN, Stop::Break);
                }
            }
            (0.5, Stop::Industry)
        }
    }

    /// Top speed with wagon drag and a speed factor from upgrades.
    pub fn top_speed(&self, bonus: f32) -> f32 {
        TOP_SPEED * (1.0 - WAGON_DRAG * self.wagons() as f32) * bonus
    }

    /// Drives one step. Returns why the train stopped, if it stopped.
    pub fn drive(&mut self, route: &Route, map: &TileMap, bonus: f32, dt: f32) -> Option<Stop> {
        let (target, why) = self.target(route, map);
        let lead = self.lead_s();
        let rem = (target - lead) * self.dir;
        if rem <= 1e-3 {
            self.v = 0.0;
            return Some(why);
        }
        let mut top = self.top_speed(bonus);
        if !route.pieces[route.index_at(lead)].is_straight() {
            top *= CURVE_FACTOR;
        }
        let allowed = top.min((2.0 * BRAKE * rem).sqrt());
        self.v = (self.v + ACCEL * dt).min(allowed);
        let mv = (self.v * dt).min(rem);
        self.s += mv * self.dir;
        self.place(route);
        if mv >= rem - 1e-4 {
            self.v = 0.0;
            return Some(why);
        }
        None
    }

    /// Swaps the direction of travel.
    pub fn reverse(&mut self) {
        self.dir = -self.dir;
        self.v = 0.0;
    }

    /// Places every car on the route.
    pub fn place(&mut self, route: &Route) {
        for (k, car) in self.cars.iter_mut().enumerate() {
            let (pos, heading) = route.point(self.s - k as f32 * CAR_GAP);
            *car = Car { pos, heading };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::find;
    use crate::track::Tile;

    const DT: f32 = 1.0 / 60.0;

    fn setup(to: Tile) -> (TileMap, Route) {
        let mut map = TileMap::new(32, 16);
        let r = find(&map, &[(2, 4)], &[to], &[], &[]).unwrap();
        r.lay(&mut map);
        (map, r)
    }

    fn run(t: &mut Train, r: &Route, m: &TileMap, max: usize) -> (Option<Stop>, usize) {
        for i in 0..max {
            if let Some(s) = t.drive(r, m, 1.0, DT) {
                return (Some(s), i);
            }
        }
        (None, max)
    }

    #[test]
    fn starts_loading_with_all_cars_on_the_route() {
        let (_, r) = setup((20, 4));
        let t = Train::new(&r, 1, TRAIN_HP);
        assert_eq!(t.cars().len(), 4);
        assert_eq!(t.phase, Phase::Loading(0.0));
        let last = t.cars()[3].pos;
        assert!((last.x - 2.5).abs() < 1e-4, "last car at the industry");
        for w in t.cars().windows(2) {
            assert!((w[0].pos.dist(w[1].pos) - CAR_GAP).abs() < 1e-3);
        }
    }

    #[test]
    fn drives_to_the_town_and_stops_at_the_end() {
        let (map, r) = setup((20, 4));
        let mut t = Train::new(&r, 0, TRAIN_HP);
        t.phase = Phase::ToTown;
        let (stop, steps) = run(&mut t, &r, &map, 2000);
        assert_eq!(stop, Some(Stop::Town));
        assert!((t.lead().pos.x - 20.5).abs() < 1e-3);
        // 18 tiles minus the train length at about 4 tiles/s.
        let secs = steps as f32 * DT;
        assert!(secs > 4.0 && secs < 7.0, "took {secs} s");
    }

    #[test]
    fn wagons_and_curves_slow_the_train() {
        let (map, r) = setup((20, 4));
        let mut t = Train::new(&r, 3, TRAIN_HP);
        assert!((t.top_speed(1.0) - TOP_SPEED * (1.0 - 5.0 * WAGON_DRAG)).abs() < 1e-4);
        t.phase = Phase::ToTown;
        run(&mut t, &r, &map, 120);
        assert!(t.v <= t.top_speed(1.0) + 1e-4);
        let (cmap, curvy) = setup((20, 12));
        let mut c = Train::new(&curvy, 0, TRAIN_HP);
        let mut slow = false;
        for _ in 0..600 {
            c.drive(&curvy, &cmap, 1.0, DT);
            let i = curvy.index_at(c.s);
            if !curvy.pieces[i].is_straight() {
                slow |= c.v <= c.top_speed(1.0) * CURVE_FACTOR + 1e-4;
            }
        }
        assert!(slow);
    }

    #[test]
    fn push_pull_reverse_keeps_the_locomotive_at_the_town_end() {
        let (map, r) = setup((20, 4));
        let mut t = Train::new(&r, 1, TRAIN_HP);
        run(&mut t, &r, &map, 2000);
        let loco = t.cars()[0].pos;
        t.reverse();
        assert_eq!(t.cars()[0].pos, loco);
        assert_eq!(t.lead().pos, t.cars()[3].pos, "the last car leads now");
        let (stop, _) = run(&mut t, &r, &map, 2000);
        assert_eq!(stop, Some(Stop::Industry));
        assert!((t.lead().pos.x - 2.5).abs() < 1e-3);
    }

    #[test]
    fn stops_before_broken_track() {
        let (mut map, r) = setup((20, 4));
        map.set_broken((12, 4), true);
        let mut t = Train::new(&r, 0, TRAIN_HP);
        let (stop, _) = run(&mut t, &r, &map, 2000);
        assert_eq!(stop, Some(Stop::Break));
        assert!(t.lead().pos.x < 12.0 && t.lead().pos.x > 11.5);
        // Back to the industry, then the break stops the trip to the town again.
        t.reverse();
        assert_eq!(run(&mut t, &r, &map, 2000).0, Some(Stop::Industry));
        t.reverse();
        assert_eq!(run(&mut t, &r, &map, 2000).0, Some(Stop::Break));
        map.set_broken((12, 4), false);
        assert_eq!(run(&mut t, &r, &map, 2000).0, Some(Stop::Town));
    }

    #[test]
    fn respawn_resets_the_train() {
        let (map, r) = setup((20, 4));
        let mut t = Train::new(&r, 2, TRAIN_HP);
        run(&mut t, &r, &map, 200);
        t.cargo = 7;
        t.phase = Phase::Wrecked(0.0);
        t.respawn(&r, 80.0);
        assert_eq!(t.cargo, 0);
        assert_eq!(t.hp, 80.0);
        assert_eq!(t.phase, Phase::Loading(0.0));
    }
}
