//! The town, the industries and their stations, and the map layout.
//!
//! The town is in the centre of the map. Industries stand around it: far
//! means rich. Each industry has one station tile and a small building
//! behind it. The town has four platforms, one on each side.

use std::collections::VecDeque;
use std::f32::consts::TAU;

use crate::math::V2;
use crate::rng::Pcg32;
use crate::track::{Dir, Terrain, Tile, TileMap, step, tile_centre};

pub const MAP_W: i32 = 48;
pub const MAP_H: i32 = 32;
/// Centre tile of the 3 x 3 town block.
pub const TOWN: Tile = (MAP_W / 2, MAP_H / 2);
/// Maximum units an industry keeps in stock.
pub const STOCK_MAX: f32 = 40.0;
/// Industries keep at least this many tiles apart.
const MIN_GAP: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Industry {
    Coal,
    Forest,
    Farm,
    Iron,
    Oil,
}

impl Industry {
    /// The industries on every map, nearest first.
    pub const LAYOUT: [Industry; 6] = [
        Industry::Coal,
        Industry::Coal,
        Industry::Farm,
        Industry::Forest,
        Industry::Iron,
        Industry::Oil,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Industry::Coal => "Coal mine",
            Industry::Forest => "Forest",
            Industry::Farm => "Farm",
            Industry::Iron => "Iron mine",
            Industry::Oil => "Oil wells",
        }
    }

    pub fn cargo(self) -> &'static str {
        match self {
            Industry::Coal => "coal",
            Industry::Forest => "wood",
            Industry::Farm => "grain",
            Industry::Iron => "iron ore",
            Industry::Oil => "oil",
        }
    }

    /// Distance to the town in tiles: minimum and maximum.
    pub fn distance(self) -> (f32, f32) {
        match self {
            Industry::Coal => (8.0, 12.0),
            Industry::Forest => (10.0, 14.0),
            Industry::Farm => (8.0, 14.0),
            Industry::Iron => (14.0, 18.0),
            Industry::Oil => (18.0, 22.0),
        }
    }

    /// Units per second.
    pub fn output(self) -> f32 {
        match self {
            Industry::Coal | Industry::Farm => 1.0,
            Industry::Forest => 1.2,
            Industry::Iron => 1.5,
            Industry::Oil => 2.0,
        }
    }

    /// Pay per unit per tile of route.
    pub fn rate(self) -> f32 {
        match self {
            Industry::Coal => 0.12,
            Industry::Forest | Industry::Farm => 0.10,
            Industry::Iron => 0.14,
            Industry::Oil => 0.18,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationKind {
    Town,
    Industry(Industry),
}

#[derive(Clone, Debug)]
pub struct Station {
    pub kind: StationKind,
    /// The station tile. For the town: the centre of the town block.
    pub tile: Tile,
    /// Tiles of the building. Clicks on them select the station.
    pub building: Vec<Tile>,
    pub stock: f32,
    /// Too many enemies near the station: the industry stops.
    pub overrun: bool,
}

impl Station {
    pub fn town() -> Self {
        let mut building = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                building.push((TOWN.0 + dx, TOWN.1 + dy));
            }
        }
        Self {
            kind: StationKind::Town,
            tile: TOWN,
            building,
            stock: 0.0,
            overrun: false,
        }
    }

    pub fn industry(&self) -> Option<Industry> {
        match self.kind {
            StationKind::Industry(i) => Some(i),
            StationKind::Town => None,
        }
    }

    pub fn is_town(&self) -> bool {
        self.kind == StationKind::Town
    }

    pub fn name(&self) -> &'static str {
        self.industry().map_or("Town", Industry::name)
    }

    /// Tiles where a line can end.
    pub fn platforms(&self) -> Vec<Tile> {
        match self.kind {
            StationKind::Town => Dir::ALL
                .into_iter()
                .map(|d| {
                    let (dx, dy) = d.delta();
                    (TOWN.0 + 2 * dx, TOWN.1 + 2 * dy)
                })
                .collect(),
            StationKind::Industry(_) => vec![self.tile],
        }
    }

    pub fn centre(&self) -> V2 {
        tile_centre(self.tile)
    }

    /// True if a click on `tile` selects this station.
    pub fn covers(&self, tile: Tile) -> bool {
        self.tile == tile || self.building.contains(&tile) || self.platforms().contains(&tile)
    }

    /// Adds `dt` seconds of output to the stock. Overrun industries stop.
    pub fn produce(&mut self, dt: f32) {
        if let Some(i) = self.industry()
            && !self.overrun
        {
            self.stock = (self.stock + i.output() * dt).min(STOCK_MAX);
        }
    }

    /// Takes up to `want` whole units from the stock.
    pub fn take(&mut self, want: u32) -> u32 {
        let got = (self.stock.floor() as u32).min(want);
        self.stock -= got as f32;
        got
    }
}

/// Makes the map and the stations for a seed. Station 0 is the town.
/// Every station can reach the town over grass.
pub fn generate(rng: &mut Pcg32) -> (TileMap, Vec<Station>) {
    loop {
        if let Some(found) = try_generate(rng) {
            return found;
        }
    }
}

fn try_generate(rng: &mut Pcg32) -> Option<(TileMap, Vec<Station>)> {
    let mut map = TileMap::new(MAP_W, MAP_H);
    let mut stations = vec![Station::town()];
    for kind in Industry::LAYOUT {
        stations.push(place(kind, &stations, rng)?);
    }
    // Keep a ring of grass around every building and platform.
    let keep: Vec<Tile> = stations
        .iter()
        .flat_map(|s| s.building.iter().copied().chain(s.platforms()))
        .collect();
    let lakes = 6 + rng.below(4);
    map.add_lakes(lakes, rng, |(x, y)| {
        keep.iter()
            .any(|&(kx, ky)| (kx - x).abs() <= 1 && (ky - y).abs() <= 1)
    });
    for s in &stations {
        for &(x, y) in &s.building {
            map.set_terrain(x, y, Terrain::Building);
        }
    }
    let reach = reachable(&map, &stations[0].platforms());
    let ok = stations[1..]
        .iter()
        .all(|s| reach[(s.tile.1 * MAP_W + s.tile.0) as usize]);
    ok.then_some((map, stations))
}

/// Puts an industry at a random place in its distance band.
fn place(kind: Industry, others: &[Station], rng: &mut Pcg32) -> Option<Station> {
    let (lo, hi) = kind.distance();
    let centre = tile_centre(TOWN);
    for _ in 0..200 {
        let p = centre + V2::from_angle(rng.range(0.0, TAU)) * rng.range(lo, hi);
        let tile = (p.x.floor() as i32, p.y.floor() as i32);
        // The building stands behind the station, away from the town.
        let away = tile_centre(tile) - centre;
        let back = if away.x.abs() > away.y.abs() {
            if away.x > 0.0 { Dir::E } else { Dir::W }
        } else if away.y > 0.0 {
            Dir::S
        } else {
            Dir::N
        };
        let b0 = step(tile, back);
        let b1 = step(b0, back.right());
        let building = vec![b0, b1, step(b0, back), step(b1, back)];
        let inside = |&(x, y): &Tile| x >= 1 && y >= 1 && x < MAP_W - 1 && y < MAP_H - 1;
        let d = tile_centre(tile).dist(centre);
        if d < lo
            || d > hi
            || !inside(&tile)
            || !building.iter().all(inside)
            || others
                .iter()
                .any(|o| o.centre().dist(tile_centre(tile)) < MIN_GAP)
        {
            continue;
        }
        return Some(Station {
            kind: StationKind::Industry(kind),
            tile,
            building,
            stock: 0.0,
            overrun: false,
        });
    }
    None
}

/// Flood fill over passable tiles from `from`.
fn reachable(map: &TileMap, from: &[Tile]) -> Vec<bool> {
    let mut seen = vec![false; (MAP_W * MAP_H) as usize];
    let mut queue: VecDeque<Tile> = from.iter().copied().collect();
    for &(x, y) in from {
        seen[(y * MAP_W + x) as usize] = true;
    }
    while let Some(t) = queue.pop_front() {
        for d in Dir::ALL {
            let n = step(t, d);
            if map.passable(n.0, n.1) && !seen[(n.1 * MAP_W + n.0) as usize] {
                seen[(n.1 * MAP_W + n.0) as usize] = true;
                queue.push_back(n);
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_follows_the_distance_bands() {
        for seed in 0..20 {
            let (map, stations) = generate(&mut Pcg32::new(seed));
            assert_eq!(stations.len(), 7);
            assert!(stations[0].is_town());
            for s in &stations[1..] {
                let kind = s.industry().unwrap();
                let (lo, hi) = kind.distance();
                let d = s.centre().dist(tile_centre(TOWN));
                assert!(d >= lo && d <= hi, "{kind:?} at {d}");
                assert!(map.passable(s.tile.0, s.tile.1));
                for &(x, y) in &s.building {
                    assert_eq!(map.terrain(x, y), Terrain::Building);
                }
            }
            for p in stations[0].platforms() {
                assert!(map.passable(p.0, p.1));
            }
        }
    }

    #[test]
    fn same_seed_same_map() {
        let (_, a) = generate(&mut Pcg32::new(5));
        let (_, b) = generate(&mut Pcg32::new(5));
        let tiles = |s: &[Station]| s.iter().map(|s| s.tile).collect::<Vec<_>>();
        assert_eq!(tiles(&a), tiles(&b));
    }

    #[test]
    fn industries_produce_into_a_capped_stock() {
        let (_, mut stations) = generate(&mut Pcg32::new(1));
        let oil = stations
            .iter_mut()
            .find(|s| s.industry() == Some(Industry::Oil))
            .unwrap();
        oil.produce(3.0);
        assert!((oil.stock - 6.0).abs() < 1e-4);
        assert_eq!(oil.take(4), 4);
        assert_eq!(oil.take(10), 2, "only whole units");
        oil.produce(100.0);
        assert_eq!(oil.stock, STOCK_MAX);
        oil.overrun = true;
        oil.take(40);
        oil.produce(5.0);
        assert_eq!(oil.stock, 0.0, "overrun industries stop");
    }

    #[test]
    fn town_has_four_platforms_and_no_output() {
        let mut town = Station::town();
        assert_eq!(town.platforms().len(), 4);
        assert!(town.covers(TOWN) && town.covers((TOWN.0 + 2, TOWN.1)));
        town.produce(10.0);
        assert_eq!(town.stock, 0.0);
    }
}
