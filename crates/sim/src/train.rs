//! The train: a locomotive and wagons that follow one trail of track pieces.
//!
//! The locomotive keeps a trail: the pieces it drove over, newest first.
//! Its position is `s`, the distance travelled on the front piece. Car `i`
//! is at distance `i * CAR_GAP` behind the head on the same trail, so all
//! cars stay on the rails, also on curves and junctions.

use std::collections::VecDeque;

use crate::math::V2;
use crate::track::{self, Dir, Piece, Tile, TileMap};

pub const CAR_GAP: f32 = 0.9;
pub const MAX_CARS: usize = 6;
pub const SPEED_TRACK: f32 = 5.0;
pub const SPEED_BUILD: f32 = 2.5;
/// Speed loss per wagon.
pub const WAGON_DRAG: f32 = 0.03;
pub const REVERSE_STOP: f32 = 0.5;
pub const SELF_HIT_STOP: f32 = 1.0;
/// After a self-hit, the head can pass its own wagons for this time.
pub const SELF_HIT_GRACE: f32 = 2.5;
const SELF_HIT_DIST: f32 = 0.6;
/// The trail always keeps at least this much track behind the head.
const MIN_TRAIL: f32 = (MAX_CARS as f32 - 1.0) * CAR_GAP + 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
struct TrailPiece {
    piece: Piece,
    /// The train built this piece when it entered the tile.
    built: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Car {
    pub pos: V2,
    pub heading: V2,
}

#[derive(Clone, Debug)]
pub struct Train {
    trail: VecDeque<TrailPiece>,
    s: f32,
    /// Car positions in drive order: `[0]` is the head.
    cars: Vec<Car>,
    /// True when the train drives backwards: the last wagon is the head.
    flipped: bool,
    latch: Option<Turn>,
    stop: f32,
    grace: f32,
    speed: f32,
    /// Speed factor from upgrades.
    pub speed_bonus: f32,
}

impl Train {
    /// Lays a straight line of track that ends at `start` and puts the head there.
    pub fn new(map: &mut TileMap, start: Tile, heading: Dir, cars: usize) -> Self {
        let back = MIN_TRAIL.ceil() as i32 + 1;
        let mut trail = VecDeque::new();
        for i in 0..=back {
            let (dx, dy) = heading.delta();
            let tile = (start.0 - dx * i, start.1 - dy * i);
            map.add_piece(tile, heading.opposite(), heading);
            trail.push_back(TrailPiece {
                piece: Piece::new(tile, heading.opposite(), heading),
                built: false,
            });
        }
        let mut train = Self {
            trail,
            s: 0.5,
            cars: vec![Car::default(); cars.clamp(1, MAX_CARS)],
            flipped: false,
            latch: None,
            stop: 0.0,
            grace: 0.0,
            speed: 0.0,
            speed_bonus: 1.0,
        };
        train.update_cars();
        train
    }

    pub fn car_count(&self) -> usize {
        self.cars.len()
    }

    pub fn set_car_count(&mut self, n: usize) {
        self.cars.resize(n.clamp(1, MAX_CARS), Car::default());
        self.update_cars();
    }

    /// Cars in drive order: `[0]` is the head.
    pub fn cars(&self) -> &[Car] {
        &self.cars
    }

    pub fn head(&self) -> &Car {
        &self.cars[0]
    }

    /// The car in a slot: slot 0 is the locomotive, then the wagons.
    pub fn slot(&self, slot: usize) -> &Car {
        let n = self.cars.len();
        if self.flipped {
            &self.cars[n - 1 - slot]
        } else {
            &self.cars[slot]
        }
    }

    pub fn is_flipped(&self) -> bool {
        self.flipped
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn is_stopped(&self) -> bool {
        self.stop > 0.0
    }

    /// True while the head drives on a piece it built.
    pub fn is_building(&self) -> bool {
        self.trail[0].built
    }

    /// The tile the head is on.
    pub fn tile(&self) -> Tile {
        self.trail[0].piece.tile
    }

    /// Remembers a turn for the next tile.
    pub fn latch(&mut self, turn: Turn) {
        self.latch = Some(turn);
    }

    pub fn latched(&self) -> Option<Turn> {
        self.latch
    }

    /// The piece the head will take on the next tile, or `None` at a dead end.
    pub fn preview(&self, map: &TileMap) -> Option<Piece> {
        let front = self.trail[0].piece;
        let tile = track::step(front.tile, front.exit);
        let entry = front.exit.opposite();
        choose_exit(map, tile, entry, self.latch).map(|(exit, _)| Piece::new(tile, entry, exit))
    }

    /// Stops for a moment and swaps head and tail.
    pub fn reverse(&mut self) {
        let back = (self.cars.len() - 1) as f32 * CAR_GAP;
        let (idx, t, _) = self.locate(back);
        self.trail.truncate(idx + 1);
        self.trail = self
            .trail
            .iter()
            .rev()
            .map(|tp| TrailPiece {
                piece: tp.piece.reversed(),
                built: false,
            })
            .collect();
        self.s = self.trail[0].piece.len() - t;
        self.flipped = !self.flipped;
        self.latch = None;
        self.stop = self.stop.max(REVERSE_STOP);
        self.speed = 0.0;
        self.update_cars();
    }

    /// Advances the train by `dt` seconds. Builds track on new tiles.
    /// Returns true if the head hit one of its own wagons.
    pub fn step(&mut self, map: &mut TileMap, dt: f32) -> bool {
        self.grace = (self.grace - dt).max(0.0);
        if self.stop > 0.0 {
            self.stop -= dt;
            self.speed = 0.0;
            return false;
        }
        let base = if self.is_building() {
            SPEED_BUILD
        } else {
            SPEED_TRACK
        };
        let wagons = (self.cars.len() - 1) as f32;
        self.speed = base * (1.0 - WAGON_DRAG * wagons) * self.speed_bonus;
        self.s += self.speed * dt;
        while self.s >= self.trail[0].piece.len() {
            let front = self.trail[0].piece;
            let tile = track::step(front.tile, front.exit);
            let entry = front.exit.opposite();
            match choose_exit(map, tile, entry, self.latch.take()) {
                Some((exit, built)) => {
                    if built {
                        map.add_piece(tile, entry, exit);
                    }
                    self.s -= front.len();
                    self.trail.push_front(TrailPiece {
                        piece: Piece::new(tile, entry, exit),
                        built,
                    });
                }
                None => {
                    // Dead end: stop, then drive back.
                    self.s = front.len();
                    self.reverse();
                    return false;
                }
            }
        }
        self.trim();
        self.update_cars();
        self.check_self_hit()
    }

    fn check_self_hit(&mut self) -> bool {
        if self.grace > 0.0 {
            return false;
        }
        let head = self.cars[0].pos;
        let hit = self
            .cars
            .iter()
            .skip(2)
            .any(|c| c.pos.dist_sq(head) < SELF_HIT_DIST * SELF_HIT_DIST);
        if hit {
            self.stop = SELF_HIT_STOP;
            self.grace = SELF_HIT_GRACE;
            self.speed = 0.0;
        }
        hit
    }

    /// Drops trail pieces that are far behind the last car.
    fn trim(&mut self) {
        let keep = ((self.cars.len() - 1) as f32 * CAR_GAP).max(MIN_TRAIL);
        let mut acc = self.s;
        let mut i = 1;
        while i < self.trail.len() && acc < keep {
            acc += self.trail[i].piece.len();
            i += 1;
        }
        self.trail.truncate(i);
    }

    /// Finds the trail point `d` behind the head: piece index, distance on that
    /// piece, and how far the point is past the end of the trail.
    fn locate(&self, d: f32) -> (usize, f32, f32) {
        if d <= self.s {
            return (0, self.s - d, 0.0);
        }
        let mut rem = d - self.s;
        for (i, tp) in self.trail.iter().enumerate().skip(1) {
            let l = tp.piece.len();
            if rem <= l {
                return (i, l - rem, 0.0);
            }
            rem -= l;
        }
        (self.trail.len() - 1, 0.0, rem)
    }

    fn point_behind(&self, d: f32) -> Car {
        let (idx, t, past) = self.locate(d);
        let (pos, heading) = self.trail[idx].piece.point(t);
        Car {
            pos: pos - heading * past,
            heading,
        }
    }

    fn update_cars(&mut self) {
        for i in 0..self.cars.len() {
            self.cars[i] = self.point_behind(i as f32 * CAR_GAP);
        }
    }
}

/// Picks the exit edge of a tile that the head enters through `entry`.
/// Returns the exit and true if the piece must be built.
fn choose_exit(map: &TileMap, tile: Tile, entry: Dir, turn: Option<Turn>) -> Option<(Dir, bool)> {
    let h = entry.opposite();
    let open = |d: Dir| {
        let next = track::step(tile, d);
        map.passable(tile.0, tile.1) && map.passable(next.0, next.1)
    };
    let order = match turn {
        Some(Turn::Left) => [h.left(), h, h.right()],
        Some(Turn::Right) => [h.right(), h, h.left()],
        None => {
            // No key: follow existing track, straight first.
            if let Some(d) = [h, h.left(), h.right()]
                .into_iter()
                .find(|&d| map.has_piece(tile, entry, d) && open(d))
            {
                return Some((d, false));
            }
            [h, h.left(), h.right()]
        }
    };
    order
        .into_iter()
        .find(|&d| open(d))
        .map(|d| (d, !map.has_piece(tile, entry, d)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{CURVE_LEN, Terrain};

    const DT: f32 = 1.0 / 60.0;

    fn setup(cars: usize) -> (TileMap, Train) {
        let mut map = TileMap::new(32, 32);
        let train = Train::new(&mut map, (16, 16), Dir::E, cars);
        (map, train)
    }

    #[test]
    fn cars_keep_their_gap_on_a_straight() {
        let (_, t) = setup(4);
        assert_eq!(t.head().pos, V2::new(16.5, 16.5));
        for (i, c) in t.cars().iter().enumerate() {
            assert!((c.pos.x - (16.5 - i as f32 * CAR_GAP)).abs() < 1e-4);
            assert_eq!(c.heading, V2::new(1.0, 0.0));
        }
    }

    #[test]
    fn existing_track_is_faster_than_building() {
        let (mut map, mut t) = setup(1);
        // The start line exists, so the first steps are on old track.
        t.step(&mut map, DT);
        assert!(!t.is_building());
        assert!((t.speed() - SPEED_TRACK).abs() < 1e-4);
        for _ in 0..30 {
            t.step(&mut map, DT);
        }
        assert!(t.is_building());
        assert!((t.speed() - SPEED_BUILD).abs() < 1e-4);
        assert_eq!(map.track(18, 16), track::EW);
    }

    #[test]
    fn wagons_slow_the_train() {
        let (mut map, mut t) = setup(5);
        t.step(&mut map, DT);
        let expect = SPEED_TRACK * (1.0 - 4.0 * WAGON_DRAG);
        assert!((t.speed() - expect).abs() < 1e-4);
    }

    #[test]
    fn turn_builds_a_curve_and_wagons_follow_it() {
        let (mut map, mut t) = setup(3);
        t.latch(Turn::Left);
        let ghost = t.preview(&map).unwrap();
        assert_eq!(ghost, Piece::new((17, 16), Dir::W, Dir::N));
        for _ in 0..120 {
            t.step(&mut map, DT);
        }
        assert_eq!(map.track(17, 16), track::WN);
        assert_eq!(t.latched(), None, "latch is used once");
        // Every car is on a piece of track: at distance 1/2 from a tile
        // corner (curve) or on a tile mid line (straight).
        for c in t.cars() {
            let fx = c.pos.x.fract();
            let fy = c.pos.y.fract();
            let on_straight = (fx - 0.5).abs() < 1e-3 || (fy - 0.5).abs() < 1e-3;
            let corner = V2::new(c.pos.x.round(), c.pos.y.round());
            let on_curve = (c.pos.dist(corner) - 0.5).abs() < 1e-3;
            assert!(on_straight || on_curve, "car off track at {:?}", c.pos);
        }
        assert!((t.head().heading.y + 1.0).abs() < 1e-3, "heads north now");
    }

    #[test]
    fn no_key_follows_existing_track() {
        let (mut map, t) = setup(1);
        map.add_piece((17, 16), Dir::W, Dir::S);
        let ghost = t.preview(&map).unwrap();
        assert_eq!(ghost.exit, Dir::S);
    }

    #[test]
    fn blocked_turn_falls_back_to_straight() {
        let (mut map, mut t) = setup(1);
        map.set_terrain(17, 15, Terrain::Water);
        t.latch(Turn::Left);
        assert_eq!(t.preview(&map).unwrap().exit, Dir::E);
        map.set_terrain(18, 16, Terrain::Water);
        assert_eq!(t.preview(&map).unwrap().exit, Dir::S);
    }

    #[test]
    fn dead_end_reverses() {
        let mut map = TileMap::new(32, 32);
        for (x, y) in [(18, 16), (17, 15), (17, 17)] {
            map.set_terrain(x, y, Terrain::Water);
        }
        let mut t = Train::new(&mut map, (16, 16), Dir::E, 2);
        for _ in 0..60 {
            t.step(&mut map, DT);
        }
        assert!(t.is_flipped());
        assert!(t.head().heading.x < -0.99, "drives west now");
    }

    #[test]
    fn trail_is_trimmed() {
        let (mut map, mut t) = setup(3);
        for _ in 0..600 {
            t.step(&mut map, DT);
        }
        let total: f32 = t.trail.iter().skip(1).map(|p| p.piece.len()).sum();
        assert!(total < MIN_TRAIL + 1.0, "trail too long: {total}");
        assert!(total + t.s >= MIN_TRAIL);
    }

    #[test]
    fn reverse_swaps_head_and_tail() {
        let (mut map, mut t) = setup(4);
        t.latch(Turn::Right);
        for _ in 0..40 {
            t.step(&mut map, DT);
        }
        let before: Vec<V2> = t.cars().iter().map(|c| c.pos).collect();
        let loco = t.slot(0).pos;
        t.reverse();
        assert!(t.is_stopped());
        let after: Vec<V2> = t.cars().iter().map(|c| c.pos).collect();
        for (a, b) in before.iter().rev().zip(&after) {
            assert!(a.dist(*b) < 1e-3, "{a:?} != {b:?}");
        }
        assert!(
            t.slot(0).pos.dist(loco) < 1e-3,
            "slot 0 stays the locomotive"
        );
        // The stop ends, then the train drives the other way.
        for _ in 0..60 {
            t.step(&mut map, DT);
        }
        assert!(!t.is_stopped());
        assert!(t.head().pos.dist(before[3]) > 0.5);
    }

    #[test]
    fn tight_loop_hits_own_wagon() {
        // A 2x2 loop is 4 curves long, shorter than a 5-car train.
        let (mut map, mut t) = setup(5);
        let loop_len = 4.0 * CURVE_LEN;
        assert!(loop_len < 4.0 * CAR_GAP);
        let mut hit = false;
        for _ in 0..300 {
            t.latch(Turn::Right);
            hit |= t.step(&mut map, DT);
        }
        assert!(hit);
    }
}
