//! The route finder: A* over tiles, like the autorail tool of OpenTTD.
//!
//! The player picks the two ends and optional waypoints. The finder picks
//! the tiles. Tiles that already have track are cheap, so new lines join
//! the network. The result is a list of track pieces that trains follow.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::math::V2;
use crate::track::{Dir, Piece, Tile, TileMap, step};

/// Search weight of a tile without track.
pub const NEW_WEIGHT: f32 = 1.0;
/// Search weight of a tile that has track.
pub const OLD_WEIGHT: f32 = 0.35;
/// Extra weight for a turn, so routes stay straight when they can.
pub const TURN_WEIGHT: f32 = 0.25;

/// A path of track pieces from one station to another.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub pieces: Vec<Piece>,
    /// Distance from the start of the route to the start of each piece.
    starts: Vec<f32>,
    pub len: f32,
    /// Tiles on the route that had no track when the route was found.
    pub new_tiles: u32,
}

impl Route {
    /// Turns a path of adjacent tiles into pieces. The end pieces run
    /// straight through the end tiles.
    pub fn from_tiles(map: &TileMap, tiles: &[Tile]) -> Option<Route> {
        if tiles.len() < 2 {
            return None;
        }
        let n = tiles.len();
        let mut pieces = Vec::with_capacity(n);
        for i in 0..n {
            let exit = if i + 1 < n {
                dir_to(tiles[i], tiles[i + 1])?
            } else {
                dir_to(tiles[i - 1], tiles[i])?
            };
            let entry = if i > 0 {
                dir_to(tiles[i], tiles[i - 1])?
            } else {
                exit.opposite()
            };
            if entry == exit {
                return None;
            }
            pieces.push(Piece::new(tiles[i], entry, exit));
        }
        let mut starts = Vec::with_capacity(n);
        let mut len = 0.0;
        for p in &pieces {
            starts.push(len);
            len += p.len();
        }
        let mut seen: Vec<Tile> = Vec::new();
        let mut new_tiles = 0;
        for &t in tiles {
            if map.track(t.0, t.1) == 0 && !seen.contains(&t) {
                new_tiles += 1;
                seen.push(t);
            }
        }
        Some(Route {
            pieces,
            starts,
            len,
            new_tiles,
        })
    }

    /// Adds the pieces of the route to the map.
    pub fn lay(&self, map: &mut TileMap) {
        for p in &self.pieces {
            map.add_piece(p.tile, p.entry, p.exit);
        }
    }

    pub fn first(&self) -> Tile {
        self.pieces[0].tile
    }

    pub fn last(&self) -> Tile {
        self.pieces[self.pieces.len() - 1].tile
    }

    /// Index of the piece at distance `d` from the start.
    pub fn index_at(&self, d: f32) -> usize {
        let d = d.clamp(0.0, self.len);
        self.starts.partition_point(|&s| s <= d).saturating_sub(1)
    }

    /// Distance from the start of the route to the start of piece `i`.
    pub fn start_of(&self, i: usize) -> f32 {
        self.starts[i]
    }

    /// Position and unit heading (toward the end of the route) at distance `d`.
    pub fn point(&self, d: f32) -> (V2, V2) {
        let d = d.clamp(0.0, self.len);
        let i = self.index_at(d);
        let p = &self.pieces[i];
        p.point((d - self.starts[i]).min(p.len()))
    }

    /// The tiles of the route in order.
    pub fn tiles(&self) -> impl Iterator<Item = Tile> + '_ {
        self.pieces.iter().map(|p| p.tile)
    }
}

/// The direction from tile `a` to the adjacent tile `b`.
pub fn dir_to(a: Tile, b: Tile) -> Option<Dir> {
    Dir::ALL.into_iter().find(|&d| step(a, d) == b)
}

/// Finds a route from any tile in `from` to any tile in `to` over the
/// waypoints `via`, in order. Tiles in `blocked` are not passable, unless
/// they are an end or a waypoint. Returns `None` if there is no way.
pub fn find(
    map: &TileMap,
    from: &[Tile],
    to: &[Tile],
    via: &[Tile],
    blocked: &[Tile],
) -> Option<Route> {
    let mut path: Vec<Tile> = Vec::new();
    let mut starts: Vec<Tile> = from.to_vec();
    let mut arrive: Option<Dir> = None;
    let goals = via.iter().map(std::slice::from_ref).chain([to]);
    for goal in goals {
        let seg = search(map, &starts, goal, arrive, blocked)?;
        if seg.len() >= 2 {
            arrive = dir_to(seg[seg.len() - 2], seg[seg.len() - 1]);
        }
        if path.last() == seg.first() {
            path.extend_from_slice(&seg[1..]);
        } else {
            path.extend_from_slice(&seg);
        }
        starts = vec![*path.last()?];
    }
    Route::from_tiles(map, &path)
}

/// Search state: tile index times 5 plus the direction of arrival (4 = none).
fn state(map: &TileMap, t: Tile, d: Option<Dir>) -> usize {
    let i = (t.1 * map.width() + t.0) as usize;
    i * 5 + d.map_or(4, |d| d as usize)
}

fn unstate(map: &TileMap, s: usize) -> (Tile, Option<Dir>) {
    let i = (s / 5) as i32;
    (
        (i % map.width(), i / map.width()),
        Dir::ALL.get(s % 5).copied(),
    )
}

/// One A* search. `arrive` is how the path arrived at the start tile: the
/// path cannot turn back, and a turn costs extra.
fn search(
    map: &TileMap,
    from: &[Tile],
    to: &[Tile],
    arrive: Option<Dir>,
    blocked: &[Tile],
) -> Option<Vec<Tile>> {
    let open = |t: Tile| {
        map.passable(t.0, t.1) && (!blocked.contains(&t) || to.contains(&t) || from.contains(&t))
    };
    if from.is_empty() || to.is_empty() || !to.iter().any(|&t| open(t)) {
        return None;
    }
    let h = |t: Tile| {
        to.iter()
            .map(|g| ((g.0 - t.0).abs() + (g.1 - t.1).abs()) as f32)
            .fold(f32::MAX, f32::min)
            * OLD_WEIGHT
    };
    let n = (map.width() * map.height()) as usize * 5;
    let mut cost = vec![f32::MAX; n];
    let mut prev: Vec<Option<(Tile, Option<Dir>)>> = vec![None; n];
    let mut heap = BinaryHeap::new();
    // Floats are positive, so their bits sort in the same order.
    let key = |f: f32| Reverse(f.to_bits());
    for &t in from {
        if !open(t) {
            continue;
        }
        let s = state(map, t, arrive);
        cost[s] = 0.0;
        heap.push((key(h(t)), s));
    }
    while let Some((_, s)) = heap.pop() {
        let (t, d) = unstate(map, s);
        let c = cost[s];
        if to.contains(&t) {
            let mut path = vec![t];
            let mut at = (t, d);
            while let Some(p) = prev[state(map, at.0, at.1)] {
                path.push(p.0);
                at = p;
            }
            path.reverse();
            return Some(path);
        }
        for nd in Dir::ALL {
            if d == Some(nd.opposite()) {
                continue;
            }
            let nt = step(t, nd);
            if !open(nt) {
                continue;
            }
            let base = if map.track(nt.0, nt.1) != 0 {
                OLD_WEIGHT
            } else {
                NEW_WEIGHT
            };
            let turn = if d.is_some_and(|d| d != nd) {
                TURN_WEIGHT
            } else {
                0.0
            };
            let nc = c + base + turn;
            let ns = state(map, nt, Some(nd));
            if nc < cost[ns] {
                cost[ns] = nc;
                prev[ns] = Some((t, d));
                heap.push((key(nc + h(nt)), ns));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::Terrain;

    fn tiles(r: &Route) -> Vec<Tile> {
        r.tiles().collect()
    }

    fn assert_connected(r: &Route) {
        for w in r.pieces.windows(2) {
            assert_eq!(step(w[0].tile, w[0].exit), w[1].tile);
            assert_eq!(w[0].exit.opposite(), w[1].entry);
        }
        let (a, _) = r.point(0.0);
        let (b, _) = r.point(r.len);
        assert!(a.dist(b) > 0.5);
    }

    #[test]
    fn straight_route_on_open_grass() {
        let m = TileMap::new(16, 8);
        let r = find(&m, &[(1, 3)], &[(9, 3)], &[], &[]).unwrap();
        assert_eq!(tiles(&r).len(), 9);
        assert_eq!(r.new_tiles, 9);
        assert!((r.len - 9.0).abs() < 1e-4);
        assert!(r.pieces.iter().all(|p| p.is_straight()));
        assert_connected(&r);
    }

    #[test]
    fn avoids_water() {
        let mut m = TileMap::new(16, 8);
        for y in 0..6 {
            m.set_terrain(5, y, Terrain::Water);
        }
        let r = find(&m, &[(1, 2)], &[(9, 2)], &[], &[]).unwrap();
        assert!(tiles(&r).iter().all(|&(x, y)| m.passable(x, y)));
        assert!(tiles(&r).contains(&(5, 6)) || tiles(&r).contains(&(5, 7)));
        assert_connected(&r);
    }

    #[test]
    fn none_when_blocked() {
        let mut m = TileMap::new(16, 8);
        for y in 0..8 {
            m.set_terrain(5, y, Terrain::Water);
        }
        assert!(find(&m, &[(1, 2)], &[(9, 2)], &[], &[]).is_none());
        let open = TileMap::new(16, 8);
        let wall: Vec<Tile> = (0..8).map(|y| (5, y)).collect();
        assert!(find(&open, &[(1, 2)], &[(9, 2)], &[], &wall).is_none());
    }

    #[test]
    fn prefers_existing_track_and_counts_only_new_tiles() {
        let mut m = TileMap::new(20, 12);
        // An old line that bends north around the direct way.
        let old = find(&m, &[(2, 5)], &[(14, 5)], &[(8, 1)], &[]).unwrap();
        old.lay(&mut m);
        let r = find(&m, &[(2, 5)], &[(14, 5)], &[], &[]).unwrap();
        assert_eq!(r.new_tiles, 0, "the old line is cheaper than new track");
        let tiles_new = find(&m, &[(2, 5)], &[(14, 9)], &[], &[]).unwrap();
        assert!(tiles_new.new_tiles > 0 && tiles_new.new_tiles < 10);
    }

    #[test]
    fn honours_waypoints_in_order() {
        let m = TileMap::new(20, 12);
        let r = find(&m, &[(1, 5)], &[(18, 5)], &[(6, 1), (12, 10)], &[]).unwrap();
        let t = tiles(&r);
        let a = t.iter().position(|&x| x == (6, 1)).unwrap();
        let b = t.iter().position(|&x| x == (12, 10)).unwrap();
        assert!(a < b);
        assert_connected(&r);
    }

    #[test]
    fn waypoint_does_not_turn_back() {
        let m = TileMap::new(20, 12);
        // The waypoint is past the goal, so the route must loop around.
        let r = find(&m, &[(2, 5)], &[(6, 5)], &[(10, 5)], &[]).unwrap();
        assert_connected(&r);
        assert!(r.pieces.iter().all(|p| p.entry != p.exit));
    }

    #[test]
    fn many_goals_take_the_nearest() {
        let m = TileMap::new(20, 12);
        let r = find(&m, &[(2, 5)], &[(18, 5), (6, 5)], &[], &[]).unwrap();
        assert_eq!(r.last(), (6, 5));
    }

    #[test]
    fn point_runs_along_the_pieces() {
        let m = TileMap::new(12, 12);
        let r = find(&m, &[(1, 1)], &[(5, 5)], &[], &[]).unwrap();
        let mut last = r.point(0.0).0;
        let mut d = 0.05;
        while d <= r.len {
            let (p, h) = r.point(d);
            assert!(p.dist(last) < 0.06);
            assert!((h.len() - 1.0).abs() < 1e-3);
            last = p;
            d += 0.05;
        }
        assert_eq!(r.index_at(0.0), 0);
        assert_eq!(r.index_at(r.len), r.pieces.len() - 1);
    }
}
