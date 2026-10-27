//! Draws the world with original programmer art. One world unit is one tile.

use macroquad::prelude::*;
use sim::enemy::{BUS_CHARGE_SPEED, BUS_CHARGE_TIME, Enemy, EnemyKind, Phase};
use sim::math::V2;
use sim::route::Route;
use sim::station::{Industry, STOCK_MAX, Station, StationKind};
use sim::track::{self, Piece, Terrain, Tile};
use sim::train::{CARGO_WAGONS, Train};
use sim::world::World;

const BACKDROP: Color = Color::new(0.11, 0.17, 0.10, 1.0);
const GRASS: [Color; 3] = [
    Color::new(0.36, 0.55, 0.27, 1.0),
    Color::new(0.37, 0.56, 0.275, 1.0),
    Color::new(0.355, 0.54, 0.265, 1.0),
];
const WATER: Color = Color::new(0.24, 0.45, 0.68, 1.0);
const SHORE: Color = Color::new(0.66, 0.62, 0.43, 1.0);
const BALLAST: Color = Color::new(0.50, 0.46, 0.41, 1.0);
const SLEEPER: Color = Color::new(0.38, 0.27, 0.17, 1.0);
const RAIL: Color = Color::new(0.78, 0.79, 0.82, 1.0);
const BROKEN: Color = Color::new(0.92, 0.22, 0.17, 1.0);
const GHOST: Color = Color::new(0.30, 0.55, 1.0, 0.75);
const GHOST_BAD: Color = Color::new(0.95, 0.30, 0.25, 0.75);
const PLATFORM: Color = Color::new(0.72, 0.70, 0.66, 1.0);
const ROOF: Color = Color::new(0.68, 0.28, 0.22, 1.0);
const WALL: Color = Color::new(0.88, 0.84, 0.74, 1.0);
const LOCO: Color = Color::new(0.12, 0.30, 0.64, 1.0);
const LOCO_CAB: Color = Color::new(0.07, 0.18, 0.40, 1.0);
const WAGON: Color = Color::new(0.45, 0.36, 0.28, 1.0);
const HIGHLIGHT: Color = Color::new(1.0, 1.0, 1.0, 0.85);
const STOCK_BACK: Color = Color::new(0.0, 0.0, 0.0, 0.55);
const STOCK: Color = Color::new(0.95, 0.75, 0.20, 1.0);
const RESIDENT: Color = Color::new(0.85, 0.62, 0.45, 1.0);
const RESIDENT_HAIR: Color = Color::new(0.30, 0.20, 0.14, 1.0);
const SABOTEUR: Color = Color::new(0.22, 0.22, 0.26, 1.0);
const SABOTEUR_HAT: Color = Color::new(0.85, 0.30, 0.15, 1.0);
const BUS: Color = Color::new(0.95, 0.74, 0.16, 1.0);
const BUS_WINDOW: Color = Color::new(0.20, 0.28, 0.34, 1.0);
const AIM: Color = Color::new(1.0, 0.25, 0.2, 0.45);
const FLASH: Color = Color::new(1.0, 1.0, 1.0, 1.0);

/// Cheap stable hash of a tile, for texture variation.
fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

pub fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

/// Colour of an industry building and of its cargo.
pub fn industry_colors(i: Industry) -> (Color, Color) {
    match i {
        Industry::Coal => (
            Color::new(0.30, 0.30, 0.32, 1.0),
            Color::new(0.10, 0.10, 0.10, 1.0),
        ),
        Industry::Forest => (
            Color::new(0.16, 0.36, 0.16, 1.0),
            Color::new(0.62, 0.42, 0.22, 1.0),
        ),
        Industry::Farm => (
            Color::new(0.80, 0.68, 0.30, 1.0),
            Color::new(0.93, 0.82, 0.40, 1.0),
        ),
        Industry::Iron => (
            Color::new(0.48, 0.30, 0.25, 1.0),
            Color::new(0.62, 0.32, 0.22, 1.0),
        ),
        Industry::Oil => (
            Color::new(0.18, 0.18, 0.22, 1.0),
            Color::new(0.08, 0.06, 0.10, 1.0),
        ),
    }
}

/// What the build tool shows on the map.
pub struct Overlay<'a> {
    pub ghost: Option<&'a Route>,
    pub ghost_ok: bool,
    pub waypoints: &'a [Tile],
    /// The station the line starts at.
    pub from: Option<usize>,
    /// The station or tile under the mouse.
    pub hover_station: Option<usize>,
    pub hover_tile: Option<Tile>,
}

pub fn draw_world(world: &World, o: &Overlay) {
    clear_background(BACKDROP);
    draw_terrain(world);
    draw_track(world);
    draw_stations(world, o);
    if let Some(r) = o.ghost {
        draw_route(r, if o.ghost_ok { GHOST } else { GHOST_BAD });
    }
    for &w in o.waypoints {
        let c = track::tile_centre(w);
        draw_circle_lines(c.x, c.y, 0.32, 0.08, GHOST);
    }
    for l in &world.lines {
        draw_train(&l.train);
    }
    draw_enemies(world);
    if let Some((x, y)) = o.hover_tile
        && o.hover_station.is_none()
    {
        draw_rectangle_lines(x as f32, y as f32, 1.0, 1.0, 0.05, HIGHLIGHT);
    }
}

fn draw_terrain(world: &World) {
    let map = &world.map;
    for y in 0..map.height() {
        for x in 0..map.width() {
            let (fx, fy) = (x as f32, y as f32);
            match map.terrain(x, y) {
                Terrain::Grass | Terrain::Building => {
                    let shade = GRASS[(hash(x, y) % 3) as usize];
                    draw_rectangle(fx, fy, 1.0, 1.0, shade);
                }
                Terrain::Water => {
                    draw_rectangle(fx, fy, 1.0, 1.0, WATER);
                    for d in track::Dir::ALL {
                        let (dx, dy) = d.delta();
                        if map.terrain(x + dx, y + dy) != Terrain::Water {
                            let c = vec2(fx + 0.5, fy + 0.5) + vec2(dx as f32, dy as f32) * 0.46;
                            let size = vec2(
                                if dx == 0 { 1.0 } else { 0.08 },
                                if dy == 0 { 1.0 } else { 0.08 },
                            );
                            draw_rectangle(
                                c.x - size.x / 2.0,
                                c.y - size.y / 2.0,
                                size.x,
                                size.y,
                                SHORE,
                            );
                        }
                    }
                }
            }
        }
    }
    let (w, h) = (map.width() as f32, map.height() as f32);
    draw_rectangle_lines(-0.1, -0.1, w + 0.2, h + 0.2, 0.2, BACKDROP);
}

fn pieces(world: &World) -> impl Iterator<Item = Piece> + '_ {
    let map = &world.map;
    (0..map.height()).flat_map(move |y| {
        (0..map.width()).flat_map(move |x| {
            let bits = map.track(x, y);
            track::ALL_PIECES
                .into_iter()
                .filter(move |b| bits & b != 0)
                .map(move |b| {
                    let (a, e) = track::bit_edges(b);
                    Piece::new((x, y), a, e)
                })
        })
    })
}

fn draw_track(world: &World) {
    for p in pieces(world) {
        draw_path(&p, 0.0, 0.62, BALLAST);
    }
    for p in pieces(world) {
        let n = if p.is_straight() { 4 } else { 3 };
        for k in 0..n {
            let (pos, h) = p.point(p.len() * (k as f32 + 0.5) / n as f32);
            let side = v(h.perp()) * 0.3;
            let c = v(pos);
            draw_line(
                c.x - side.x,
                c.y - side.y,
                c.x + side.x,
                c.y + side.y,
                0.09,
                SLEEPER,
            );
        }
    }
    for p in pieces(world) {
        for off in [-0.17, 0.17] {
            draw_path(&p, off, 0.05, RAIL);
        }
    }
    let t = get_time() as f32;
    for y in 0..world.map.height() {
        for x in 0..world.map.width() {
            if world.map.is_broken((x, y)) {
                let a = 0.6 + 0.4 * (t * 6.0).sin();
                let c = Color { a, ..BROKEN };
                let (fx, fy) = (x as f32, y as f32);
                draw_line(fx + 0.2, fy + 0.2, fx + 0.8, fy + 0.8, 0.12, c);
                draw_line(fx + 0.8, fy + 0.2, fx + 0.2, fy + 0.8, 0.12, c);
            }
        }
    }
}

/// Draws a line along a piece, shifted sideways by `off`.
pub fn draw_path(p: &Piece, off: f32, thick: f32, color: Color) {
    let n = if p.is_straight() { 1 } else { 8 };
    let at = |k: usize| {
        let (pos, h) = p.point(p.len() * k as f32 / n as f32);
        v(pos + h.perp() * off)
    };
    for k in 0..n {
        let (a, b) = (at(k), at(k + 1));
        draw_line(a.x, a.y, b.x, b.y, thick, color);
    }
}

fn draw_route(r: &Route, color: Color) {
    for p in &r.pieces {
        draw_path(p, 0.0, 0.22, color);
    }
}

fn draw_stations(world: &World, o: &Overlay) {
    let t = get_time() as f32;
    for (i, s) in world.stations.iter().enumerate() {
        for &(x, y) in &s.platforms() {
            draw_rectangle(x as f32 + 0.08, y as f32 + 0.08, 0.84, 0.84, PLATFORM);
        }
        match s.kind {
            StationKind::Town => draw_town(s),
            StationKind::Industry(kind) => draw_industry(s, kind),
        }
        if s.overrun && (t * 4.0).sin() > 0.0 {
            outline(s, BROKEN);
        }
        if o.from == Some(i) || o.hover_station == Some(i) {
            outline(s, HIGHLIGHT);
        }
    }
}

fn bounds(s: &Station) -> Rect {
    let tiles = s.building.iter().chain(std::iter::once(&s.tile));
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in tiles {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Rect::new(
        x0 as f32,
        y0 as f32,
        (x1 - x0 + 1) as f32,
        (y1 - y0 + 1) as f32,
    )
}

fn outline(s: &Station, color: Color) {
    let r = bounds(s);
    draw_rectangle_lines(r.x - 0.1, r.y - 0.1, r.w + 0.2, r.h + 0.2, 0.1, color);
}

fn draw_town(s: &Station) {
    for &(x, y) in &s.building {
        let (fx, fy) = (x as f32, y as f32);
        draw_rectangle(
            fx + 0.05,
            fy + 0.05,
            0.9,
            0.9,
            Color::new(0.55, 0.55, 0.52, 1.0),
        );
        let h = hash(x, y);
        let n = 1 + (h % 2) as i32;
        for k in 0..n {
            let ox = fx + 0.12 + k as f32 * 0.42;
            let oy = fy + 0.12 + ((h >> 3) % 3) as f32 * 0.1;
            draw_rectangle(ox, oy + 0.18, 0.36, 0.42, WALL);
            draw_triangle(
                vec2(ox - 0.03, oy + 0.2),
                vec2(ox + 0.39, oy + 0.2),
                vec2(ox + 0.18, oy),
                ROOF,
            );
        }
    }
}

fn draw_industry(s: &Station, kind: Industry) {
    let (body, cargo) = industry_colors(kind);
    let r = Rect::new(
        s.building.iter().map(|t| t.0).min().unwrap_or(0) as f32,
        s.building.iter().map(|t| t.1).min().unwrap_or(0) as f32,
        2.0,
        2.0,
    );
    draw_rectangle(r.x + 0.05, r.y + 0.05, r.w - 0.1, r.h - 0.1, body);
    let c = r.center();
    match kind {
        Industry::Coal | Industry::Iron => {
            // Head frame and a heap.
            draw_triangle(
                vec2(c.x - 0.8, c.y + 0.8),
                vec2(c.x + 0.1, c.y + 0.8),
                vec2(c.x - 0.35, c.y + 0.1),
                cargo,
            );
            draw_line(c.x + 0.2, c.y + 0.8, c.x + 0.5, c.y - 0.7, 0.08, WALL);
            draw_line(c.x + 0.8, c.y + 0.8, c.x + 0.5, c.y - 0.7, 0.08, WALL);
            draw_circle(c.x + 0.5, c.y - 0.6, 0.18, WALL);
        }
        Industry::Forest => {
            for (dx, dy) in [(-0.45, -0.4), (0.4, -0.45), (-0.4, 0.45), (0.45, 0.4)] {
                draw_circle(c.x + dx, c.y + dy, 0.38, Color::new(0.10, 0.28, 0.10, 1.0));
            }
            draw_rectangle(c.x - 0.4, c.y - 0.08, 0.8, 0.16, cargo);
        }
        Industry::Farm => {
            for k in 0..4 {
                let y = r.y + 0.2 + k as f32 * 0.42;
                draw_rectangle(r.x + 0.15, y, 1.7, 0.2, cargo);
            }
            draw_rectangle(c.x - 0.3, c.y - 0.3, 0.6, 0.6, ROOF);
        }
        Industry::Oil => {
            draw_circle(
                c.x - 0.4,
                c.y - 0.4,
                0.32,
                Color::new(0.75, 0.75, 0.78, 1.0),
            );
            draw_circle(
                c.x + 0.4,
                c.y + 0.35,
                0.32,
                Color::new(0.75, 0.75, 0.78, 1.0),
            );
            draw_line(c.x - 0.6, c.y + 0.6, c.x + 0.1, c.y - 0.1, 0.1, cargo);
        }
    }
    // Stock bar under the building.
    let frac = s.stock / STOCK_MAX;
    draw_rectangle(r.x + 0.1, r.y + r.h - 0.25, r.w - 0.2, 0.15, STOCK_BACK);
    draw_rectangle(r.x + 0.1, r.y + r.h - 0.25, (r.w - 0.2) * frac, 0.15, STOCK);
}

/// Draws a box of `len` by `wid` tiles, centred on `pos` and turned to `heading`.
pub fn draw_box(pos: V2, heading: V2, len: f32, wid: f32, color: Color) {
    draw_rectangle_ex(
        pos.x,
        pos.y,
        len,
        wid,
        DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation: heading.angle(),
            color,
        },
    );
}

fn draw_train(train: &Train) {
    if train.is_wrecked() {
        return;
    }
    let cars = train.cars();
    let cap = (CARGO_WAGONS as u32 * sim::train::WAGON_CAPACITY).max(1);
    for (k, car) in cars.iter().enumerate().skip(1) {
        let (p, h) = (car.pos, car.heading);
        draw_box(p, h, 0.8, 0.52, WAGON);
        if k <= CARGO_WAGONS {
            // Fill the cargo wagons in order.
            let per = cap as f32 / CARGO_WAGONS as f32;
            let have = (train.cargo as f32 - per * (k - 1) as f32).clamp(0.0, per) / per;
            if have > 0.0 {
                draw_box(p, h, 0.62 * have, 0.36, STOCK);
            }
        }
    }
    let loco = &cars[0];
    // The locomotive faces the town.
    draw_box(loco.pos, loco.heading, 0.84, 0.56, LOCO);
    draw_box(
        loco.pos - loco.heading * 0.24,
        loco.heading,
        0.3,
        0.5,
        LOCO_CAB,
    );
    let lamp = v(loco.pos + loco.heading * 0.36);
    draw_circle(lamp.x, lamp.y, 0.08, Color::new(1.0, 0.95, 0.6, 1.0));
}

fn tint(e: &Enemy, c: Color) -> Color {
    if e.flash > 0.0 { FLASH } else { c }
}

fn draw_enemies(world: &World) {
    for e in &world.enemies {
        if e.kind == EnemyKind::Bus && e.phase == Phase::Aim {
            let end = v(e.pos + e.dir * (BUS_CHARGE_SPEED * BUS_CHARGE_TIME));
            draw_line(e.pos.x, e.pos.y, end.x, end.y, 0.12, AIM);
        }
    }
    for e in &world.enemies {
        let p = e.pos;
        match e.kind {
            EnemyKind::Resident => {
                draw_circle(p.x, p.y, e.radius(), tint(e, RESIDENT));
                let hair = v(p - e.dir * 0.08);
                draw_circle(hair.x, hair.y, e.radius() * 0.6, tint(e, RESIDENT_HAIR));
            }
            EnemyKind::Saboteur => {
                draw_circle(p.x, p.y, e.radius(), tint(e, SABOTEUR));
                let hat = v(p - e.dir * 0.06);
                draw_circle(hat.x, hat.y, e.radius() * 0.55, tint(e, SABOTEUR_HAT));
                if e.work > 0.0 {
                    // A spanner that swings while the saboteur works.
                    let a = (get_time() as f32 * 12.0).sin() * 0.6;
                    let tip = v(p + V2::from_angle(a - 0.8) * 0.45);
                    draw_line(
                        p.x,
                        p.y,
                        tip.x,
                        tip.y,
                        0.08,
                        Color::new(0.8, 0.8, 0.85, 1.0),
                    );
                }
            }
            EnemyKind::Bus => {
                draw_box(p, e.dir, 0.95, 0.48, tint(e, BUS));
                draw_box(p + e.dir * 0.05, e.dir, 0.7, 0.3, tint(e, BUS_WINDOW));
                draw_box(p + e.dir * 0.4, e.dir, 0.1, 0.44, tint(e, BUS));
            }
        }
    }
}
