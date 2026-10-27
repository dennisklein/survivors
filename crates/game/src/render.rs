//! Draws the world with original programmer art. One world unit is one tile.

use macroquad::prelude::*;
use sim::enemy::{BUS_CHARGE_SPEED, BUS_CHARGE_TIME, Enemy, EnemyKind, Phase};
use sim::math::V2;
use sim::track::{self, Piece, Terrain};
use sim::weapon::{BLAST_TTL, WagonKind};
use sim::world::World;

/// Tiles visible across the screen width.
pub const VIEW_TILES: f32 = 24.0;

const BACKDROP: Color = Color::new(0.11, 0.17, 0.10, 1.0);
const GRASS: [Color; 3] = [
    Color::new(0.30, 0.49, 0.24, 1.0),
    Color::new(0.31, 0.50, 0.245, 1.0),
    Color::new(0.295, 0.48, 0.235, 1.0),
];
const WATER: Color = Color::new(0.22, 0.43, 0.65, 1.0);
const SHORE: Color = Color::new(0.62, 0.58, 0.40, 1.0);
const SLEEPER: Color = Color::new(0.42, 0.29, 0.17, 1.0);
const RAIL: Color = Color::new(0.80, 0.80, 0.82, 1.0);
const GHOST: Color = Color::new(0.35, 0.65, 1.0, 0.65);
const LOCO: Color = Color::new(0.69, 0.23, 0.18, 1.0);
const LOCO_CAB: Color = Color::new(0.45, 0.13, 0.10, 1.0);
const ARMOURED: Color = Color::new(0.33, 0.40, 0.30, 1.0);
const TURRET: Color = Color::new(0.20, 0.25, 0.18, 1.0);
const HOPPER: Color = Color::new(0.36, 0.27, 0.20, 1.0);
const COAL: Color = Color::new(0.10, 0.10, 0.10, 1.0);
const STEAM: Color = Color::new(0.34, 0.45, 0.62, 1.0);
const DOME: Color = Color::new(0.85, 0.88, 0.92, 1.0);
const SHOT: Color = Color::new(1.0, 0.92, 0.45, 1.0);
const FIRE: Color = Color::new(1.0, 0.45, 0.10, 1.0);
const EMBER: Color = Color::new(1.0, 0.80, 0.25, 1.0);
const RESIDENT: Color = Color::new(0.80, 0.58, 0.42, 1.0);
const RESIDENT_HAIR: Color = Color::new(0.30, 0.20, 0.14, 1.0);
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

/// A camera that follows `centre` and shows `VIEW_TILES` tiles across.
pub fn camera(centre: V2) -> Camera2D {
    let w = VIEW_TILES;
    let h = w * screen_height() / screen_width();
    Camera2D {
        target: v(centre),
        zoom: vec2(2.0 / w, 2.0 / h),
        ..Default::default()
    }
}

/// The visible tile range, with one tile of margin.
fn visible(cam: &Camera2D) -> (i32, i32, i32, i32) {
    let a = cam.screen_to_world(vec2(0.0, 0.0));
    let b = cam.screen_to_world(vec2(screen_width(), screen_height()));
    (
        a.x.min(b.x).floor() as i32 - 1,
        a.y.min(b.y).floor() as i32 - 1,
        a.x.max(b.x).ceil() as i32 + 1,
        a.y.max(b.y).ceil() as i32 + 1,
    )
}

pub fn draw_world(world: &World, cam: &Camera2D) {
    clear_background(BACKDROP);
    let view = visible(cam);
    draw_terrain(world, view);
    draw_track(world, view);
    draw_fires(world);
    draw_ghost(world);
    draw_train(world);
    draw_enemies(world);
    draw_shots(world);
}

fn draw_terrain(world: &World, (x0, y0, x1, y1): (i32, i32, i32, i32)) {
    let map = &world.map;
    for y in y0.max(0)..y1.min(map.height()) {
        for x in x0.max(0)..x1.min(map.width()) {
            let (fx, fy) = (x as f32, y as f32);
            match map.terrain(x, y) {
                Terrain::Grass => {
                    let shade = GRASS[(hash(x, y) % 3) as usize];
                    draw_rectangle(fx, fy, 1.0, 1.0, shade);
                }
                Terrain::Water => {
                    draw_rectangle(fx, fy, 1.0, 1.0, WATER);
                    // Sand where water meets grass.
                    for d in track::Dir::ALL {
                        let (dx, dy) = d.delta();
                        if map.terrain(x + dx, y + dy) == Terrain::Grass
                            && map.in_bounds(x + dx, y + dy)
                        {
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
    draw_rectangle_lines(
        -0.1,
        -0.1,
        w + 0.2,
        h + 0.2,
        0.2,
        Color::new(0.05, 0.08, 0.05, 1.0),
    );
}

fn visible_pieces(
    world: &World,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
) -> impl Iterator<Item = Piece> + '_ {
    (y0..y1).flat_map(move |y| {
        (x0..x1).flat_map(move |x| {
            let bits = world.map.track(x, y);
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

fn draw_track(world: &World, view: (i32, i32, i32, i32)) {
    for p in visible_pieces(world, view) {
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
    for p in visible_pieces(world, view) {
        for off in [-0.17, 0.17] {
            draw_path(&p, off, 0.05, RAIL);
        }
    }
}

/// Draws a line along a piece, shifted sideways by `off`.
fn draw_path(p: &Piece, off: f32, thick: f32, color: Color) {
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

fn draw_ghost(world: &World) {
    let Some(p) = world.train.preview(&world.map) else {
        return;
    };
    draw_path(&p, 0.0, 0.12, GHOST);
    let (end, h) = p.point(p.len());
    let (tip, hv, side) = (v(end + h * 0.15), v(h), v(h.perp()));
    let base = tip - hv * 0.3;
    draw_triangle(tip, base + side * 0.2, base - side * 0.2, GHOST);
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

fn draw_train(world: &World) {
    let train = &world.train;
    for (i, w) in world.wagons.iter().enumerate() {
        let car = train.slot(i + 1);
        let (p, h) = (car.pos, car.heading);
        match w.kind {
            WagonKind::Armoured => {
                draw_box(p, h, 0.8, 0.52, ARMOURED);
                let tip = v(p + w.aim * 0.32);
                draw_line(p.x, p.y, tip.x, tip.y, 0.08, TURRET);
                draw_circle(p.x, p.y, 0.15, TURRET);
            }
            WagonKind::Hopper => {
                draw_box(p, h, 0.8, 0.52, HOPPER);
                draw_box(p, h, 0.6, 0.36, COAL);
            }
            WagonKind::Steam => {
                draw_box(p, h, 0.8, 0.52, STEAM);
                draw_circle(p.x, p.y, 0.16, DOME);
            }
        }
    }
    let loco = train.slot(0);
    // The locomotive always points to the front of the train.
    let fwd = if train.is_flipped() {
        -loco.heading
    } else {
        loco.heading
    };
    draw_box(loco.pos, fwd, 0.84, 0.54, LOCO);
    draw_box(loco.pos - fwd * 0.24, fwd, 0.3, 0.5, LOCO_CAB);
    let chimney = v(loco.pos + fwd * 0.26);
    draw_circle(chimney.x, chimney.y, 0.09, BLACK);
}

fn draw_enemies(world: &World) {
    for e in &world.enemies {
        if e.kind == EnemyKind::Bus && e.phase == Phase::Aim {
            let end = v(e.pos + e.dir * (BUS_CHARGE_SPEED * BUS_CHARGE_TIME));
            draw_line(e.pos.x, e.pos.y, end.x, end.y, 0.12, AIM);
        }
    }
    for e in &world.enemies {
        draw_enemy(e);
    }
}

fn tint(e: &Enemy, c: Color) -> Color {
    if e.flash > 0.0 { FLASH } else { c }
}

fn draw_enemy(e: &Enemy) {
    let p = e.pos;
    match e.kind {
        EnemyKind::Resident => {
            draw_circle(p.x, p.y, e.radius(), tint(e, RESIDENT));
            let hair = v(p - e.dir * 0.08);
            draw_circle(hair.x, hair.y, e.radius() * 0.6, tint(e, RESIDENT_HAIR));
        }
        EnemyKind::Bus => {
            draw_box(p, e.dir, 0.95, 0.48, tint(e, BUS));
            draw_box(p + e.dir * 0.05, e.dir, 0.7, 0.3, tint(e, BUS_WINDOW));
            draw_box(p + e.dir * 0.4, e.dir, 0.1, 0.44, tint(e, BUS));
        }
    }
}

fn draw_fires(world: &World) {
    let t = get_time() as f32;
    for ((x, y), ttl) in world.fires.iter() {
        let flicker = 0.5 + 0.5 * (t * 13.0 + (x * 3 + y * 5) as f32).sin();
        let a = ttl.min(1.0) * (0.35 + 0.15 * flicker);
        draw_rectangle(
            x as f32 + 0.06,
            y as f32 + 0.06,
            0.88,
            0.88,
            Color { a, ..FIRE },
        );
        let e = (hash(x, y) % 4) as f32 * 0.2 + 0.2;
        draw_circle(
            x as f32 + e,
            y as f32 + 0.8 - e * 0.5,
            0.07,
            Color {
                a: a * 2.0,
                ..EMBER
            },
        );
    }
}

fn draw_shots(world: &World) {
    for s in &world.shots {
        let tail = v(s.pos - s.vel.norm() * 0.25);
        draw_line(tail.x, tail.y, s.pos.x, s.pos.y, 0.08, SHOT);
    }
    for b in &world.blasts {
        let k = 1.0 - b.ttl / BLAST_TTL;
        let a = 0.7 * (1.0 - k);
        draw_circle(
            b.pos.x,
            b.pos.y,
            b.radius * k,
            Color::new(0.9, 0.95, 1.0, a * 0.35),
        );
        draw_circle_lines(
            b.pos.x,
            b.pos.y,
            b.radius * k,
            0.08,
            Color::new(1.0, 1.0, 1.0, a),
        );
    }
}
