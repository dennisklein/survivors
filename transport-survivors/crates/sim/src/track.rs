//! Tile map and track pieces.
//!
//! Each tile stores its track as 6 bits, one bit per piece, like the
//! `TrackBits` of OpenTTD. A piece connects the midpoints of two tile edges.
//! A tile with more than one bit is a junction or a crossing.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use crate::math::V2;
use crate::rng::Pcg32;

/// Compass direction. `y` points down (south), as on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];

    pub fn opposite(self) -> Dir {
        match self {
            Dir::N => Dir::S,
            Dir::E => Dir::W,
            Dir::S => Dir::N,
            Dir::W => Dir::E,
        }
    }

    /// The direction 90 degrees to the left of a train that heads this way.
    pub fn left(self) -> Dir {
        match self {
            Dir::N => Dir::W,
            Dir::E => Dir::N,
            Dir::S => Dir::E,
            Dir::W => Dir::S,
        }
    }

    pub fn right(self) -> Dir {
        self.left().opposite()
    }

    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }

    pub fn vec(self) -> V2 {
        let (x, y) = self.delta();
        V2::new(x as f32, y as f32)
    }
}

pub type Tile = (i32, i32);

pub fn step(tile: Tile, d: Dir) -> Tile {
    let (dx, dy) = d.delta();
    (tile.0 + dx, tile.1 + dy)
}

pub const NS: u8 = 1;
pub const EW: u8 = 2;
pub const NE: u8 = 4;
pub const ES: u8 = 8;
pub const SW: u8 = 16;
pub const WN: u8 = 32;
pub const ALL_PIECES: [u8; 6] = [NS, EW, NE, ES, SW, WN];

/// The bit of the piece that connects edges `a` and `b`. Zero if `a == b`.
pub fn piece_bit(a: Dir, b: Dir) -> u8 {
    use Dir::*;
    match (a, b) {
        (N, S) | (S, N) => NS,
        (E, W) | (W, E) => EW,
        (N, E) | (E, N) => NE,
        (E, S) | (S, E) => ES,
        (S, W) | (W, S) => SW,
        (W, N) | (N, W) => WN,
        _ => 0,
    }
}

/// The two edges that a single piece bit connects.
pub fn bit_edges(bit: u8) -> (Dir, Dir) {
    use Dir::*;
    match bit {
        NS => (N, S),
        EW => (E, W),
        NE => (N, E),
        ES => (E, S),
        SW => (S, W),
        WN => (W, N),
        _ => panic!("not a single piece bit: {bit}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Grass,
    Water,
}

#[derive(Clone, Debug)]
pub struct TileMap {
    w: i32,
    h: i32,
    terrain: Vec<Terrain>,
    track: Vec<u8>,
}

impl TileMap {
    pub fn new(w: i32, h: i32) -> Self {
        let n = (w * h) as usize;
        Self {
            w,
            h,
            terrain: vec![Terrain::Grass; n],
            track: vec![0; n],
        }
    }

    /// Grass with random lakes. The area within `clear` tiles of the centre
    /// stays dry, so the train has room to start.
    pub fn generate(w: i32, h: i32, clear: f32, rng: &mut Pcg32) -> Self {
        let mut map = Self::new(w, h);
        let centre = V2::new(w as f32 / 2.0, h as f32 / 2.0);
        let lakes = 7 + rng.below(4);
        for _ in 0..lakes {
            let c = V2::new(rng.range(0.0, w as f32), rng.range(0.0, h as f32));
            let r = rng.range(1.5, 4.5);
            let wobble = [rng.range(0.6, 1.4), rng.range(0.6, 1.4)];
            for y in 0..h {
                for x in 0..w {
                    let p = V2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let d = p - c;
                    let e = V2::new(d.x / wobble[0], d.y / wobble[1]);
                    if e.len() < r && p.dist(centre) > clear {
                        map.set_terrain(x, y, Terrain::Water);
                    }
                }
            }
        }
        map
    }

    pub fn width(&self) -> i32 {
        self.w
    }

    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    /// Terrain at a tile. Outside the map counts as water.
    pub fn terrain(&self, x: i32, y: i32) -> Terrain {
        if self.in_bounds(x, y) {
            self.terrain[self.idx(x, y)]
        } else {
            Terrain::Water
        }
    }

    pub fn set_terrain(&mut self, x: i32, y: i32, t: Terrain) {
        if self.in_bounds(x, y) {
            let i = self.idx(x, y);
            self.terrain[i] = t;
        }
    }

    /// True if a train can drive on this tile.
    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.terrain(x, y) == Terrain::Grass
    }

    pub fn track(&self, x: i32, y: i32) -> u8 {
        if self.in_bounds(x, y) {
            self.track[self.idx(x, y)]
        } else {
            0
        }
    }

    pub fn has_piece(&self, tile: Tile, a: Dir, b: Dir) -> bool {
        self.track(tile.0, tile.1) & piece_bit(a, b) != 0
    }

    /// Adds a piece. Returns true if the piece is new.
    pub fn add_piece(&mut self, tile: Tile, a: Dir, b: Dir) -> bool {
        if !self.passable(tile.0, tile.1) || self.has_piece(tile, a, b) {
            return false;
        }
        let i = self.idx(tile.0, tile.1);
        self.track[i] |= piece_bit(a, b);
        true
    }

    pub fn clear_track(&mut self, tile: Tile) {
        if self.in_bounds(tile.0, tile.1) {
            let i = self.idx(tile.0, tile.1);
            self.track[i] = 0;
        }
    }

    /// Exit edges that existing track connects to the `entry` edge.
    pub fn exits(&self, tile: Tile, entry: Dir) -> impl Iterator<Item = Dir> + '_ {
        Dir::ALL
            .into_iter()
            .filter(move |&d| d != entry && self.has_piece(tile, entry, d))
    }
}

/// One tile of a path: the train enters through `entry` and leaves through `exit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub tile: Tile,
    pub entry: Dir,
    pub exit: Dir,
}

/// Length of a curve: a quarter circle with radius 1/2.
pub const CURVE_LEN: f32 = FRAC_PI_4;

impl Piece {
    pub fn new(tile: Tile, entry: Dir, exit: Dir) -> Self {
        debug_assert!(entry != exit);
        Self { tile, entry, exit }
    }

    pub fn is_straight(&self) -> bool {
        self.entry.opposite() == self.exit
    }

    pub fn len(&self) -> f32 {
        if self.is_straight() { 1.0 } else { CURVE_LEN }
    }

    pub fn reversed(&self) -> Piece {
        Piece::new(self.tile, self.exit, self.entry)
    }

    /// Position and unit heading at distance `t` from the entry edge.
    pub fn point(&self, t: f32) -> (V2, V2) {
        let a = edge_mid(self.tile, self.entry);
        let b = edge_mid(self.tile, self.exit);
        if self.is_straight() {
            let h = self.exit.vec();
            return (a + h * t, h);
        }
        // The arc centre is the tile corner between the two edges.
        let centre = tile_centre(self.tile);
        let corner = a + b - centre;
        let a0 = (a - corner).angle();
        let mut sweep = (b - corner).angle() - a0;
        if sweep > PI {
            sweep -= 2.0 * PI;
        } else if sweep < -PI {
            sweep += 2.0 * PI;
        }
        let ang = a0 + sweep * (t / CURVE_LEN);
        let pos = corner + V2::from_angle(ang) * 0.5;
        let heading = V2::from_angle(ang + FRAC_PI_2 * sweep.signum());
        (pos, heading)
    }
}

pub fn tile_centre(tile: Tile) -> V2 {
    V2::new(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5)
}

/// Midpoint of a tile edge.
pub fn edge_mid(tile: Tile, d: Dir) -> V2 {
    tile_centre(tile) + d.vec() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: V2, b: V2) -> bool {
        a.dist(b) < 1e-4
    }

    #[test]
    fn piece_bits_are_unique_and_symmetric() {
        let mut seen = 0u8;
        for a in Dir::ALL {
            for b in Dir::ALL {
                let bit = piece_bit(a, b);
                assert_eq!(bit, piece_bit(b, a));
                if a == b {
                    assert_eq!(bit, 0);
                } else {
                    assert_eq!(bit.count_ones(), 1);
                    let (x, y) = bit_edges(bit);
                    assert!((x, y) == (a, b) || (x, y) == (b, a));
                    seen |= bit;
                }
            }
        }
        assert_eq!(seen, 0b11_1111);
    }

    #[test]
    fn turns_are_relative() {
        assert_eq!(Dir::N.left(), Dir::W);
        assert_eq!(Dir::N.right(), Dir::E);
        assert_eq!(Dir::E.right(), Dir::S);
        for d in Dir::ALL {
            assert_eq!(d.left().right(), d);
            assert_eq!(d.opposite().opposite(), d);
        }
    }

    #[test]
    fn connect_straight_piece() {
        let mut m = TileMap::new(4, 4);
        assert!(m.add_piece((1, 1), Dir::S, Dir::N));
        assert!(!m.add_piece((1, 1), Dir::N, Dir::S), "already there");
        assert_eq!(m.track(1, 1), NS);
        assert_eq!(m.exits((1, 1), Dir::S).collect::<Vec<_>>(), vec![Dir::N]);
        assert_eq!(m.exits((1, 1), Dir::E).count(), 0);
    }

    #[test]
    fn crossing_keeps_both_lines_apart() {
        let mut m = TileMap::new(4, 4);
        m.add_piece((1, 1), Dir::N, Dir::S);
        m.add_piece((1, 1), Dir::E, Dir::W);
        assert_eq!(m.track(1, 1), NS | EW);
        assert_eq!(m.exits((1, 1), Dir::S).collect::<Vec<_>>(), vec![Dir::N]);
        assert_eq!(m.exits((1, 1), Dir::W).collect::<Vec<_>>(), vec![Dir::E]);
    }

    #[test]
    fn junction_offers_two_exits() {
        let mut m = TileMap::new(4, 4);
        m.add_piece((2, 2), Dir::S, Dir::N);
        m.add_piece((2, 2), Dir::S, Dir::E);
        let mut exits: Vec<_> = m.exits((2, 2), Dir::S).collect();
        exits.sort_by_key(|d| *d as u8);
        assert_eq!(exits, vec![Dir::N, Dir::E]);
    }

    #[test]
    fn no_track_on_water_or_outside() {
        let mut m = TileMap::new(4, 4);
        m.set_terrain(0, 0, Terrain::Water);
        assert!(!m.add_piece((0, 0), Dir::N, Dir::S));
        assert!(!m.add_piece((9, 9), Dir::N, Dir::S));
        assert!(!m.passable(-1, 0));
    }

    #[test]
    fn straight_piece_geometry() {
        let p = Piece::new((3, 5), Dir::S, Dir::N);
        assert_eq!(p.len(), 1.0);
        let (a, h) = p.point(0.0);
        assert!(close(a, V2::new(3.5, 6.0)));
        assert!(close(h, V2::new(0.0, -1.0)));
        assert!(close(p.point(1.0).0, V2::new(3.5, 5.0)));
    }

    #[test]
    fn curve_piece_geometry() {
        // Drive north into the tile, turn right to the east edge.
        let p = Piece::new((0, 0), Dir::S, Dir::E);
        assert!((p.len() - CURVE_LEN).abs() < 1e-6);
        let (start, h0) = p.point(0.0);
        let (end, h1) = p.point(p.len());
        assert!(close(start, V2::new(0.5, 1.0)));
        assert!(close(end, V2::new(1.0, 0.5)));
        assert!(close(h0, V2::new(0.0, -1.0)));
        assert!(close(h1, V2::new(1.0, 0.0)));
        // Every point is 1/2 from the corner (1, 1).
        for i in 0..=10 {
            let (q, _) = p.point(p.len() * i as f32 / 10.0);
            assert!((q.dist(V2::new(1.0, 1.0)) - 0.5).abs() < 1e-4);
        }
        // The reversed piece runs the same arc backwards.
        let r = p.reversed();
        assert!(close(r.point(0.0).0, end));
        assert!(close(r.point(0.0).1, V2::new(-1.0, 0.0)));
    }

    #[test]
    fn generated_map_keeps_centre_dry() {
        let m = TileMap::generate(64, 64, 6.0, &mut Pcg32::new(3));
        for y in 28..36 {
            for x in 28..36 {
                assert!(m.passable(x, y));
            }
        }
        let water = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(x, y)| !m.passable(x, y))
            .count();
        assert!(water > 0);
    }
}
