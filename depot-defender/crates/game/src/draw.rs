//! World rendering with procedural vector shapes.

use crate::fx::Fx;
use crate::ui::{self, cargo_color, enemy_color};
use macroquad::prelude::*;
use rules::data::EnemyKind;
use rules::game::{
    ARENA, DEPOT_MAX_HP, DEPOT_RADIUS, Game, Line, PLAYER_RADIUS, PickupKind, ShotKind, TrainState,
    UNLOAD_RANGE,
};
use rules::math::V2;

/// Converts world coordinates to screen coordinates for one frame.
#[derive(Clone, Copy)]
pub struct View {
    center: V2,
    half: Vec2,
}

impl View {
    pub fn new(fx: &Fx) -> Self {
        Self {
            center: fx.view_center(),
            half: vec2(screen_width() / 2.0, screen_height() / 2.0),
        }
    }

    pub fn s(&self, p: V2) -> Vec2 {
        vec2(p.x - self.center.x, p.y - self.center.y) + self.half
    }

    pub fn on_screen(&self, p: V2, pad: f32) -> bool {
        let s = self.s(p);
        s.x > -pad && s.y > -pad && s.x < screen_width() + pad && s.y < screen_height() + pad
    }
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

/// Filled rectangle centred at `c`, long side along `dir`.
pub fn rect_rot(c: Vec2, dir: Vec2, len: f32, wid: f32, color: Color) {
    let f = dir * (len / 2.0);
    let s = vec2(-dir.y, dir.x) * (wid / 2.0);
    let (a, b, d, e) = (c + f + s, c + f - s, c - f - s, c - f + s);
    draw_triangle(a, b, d, color);
    draw_triangle(a, d, e, color);
}

pub fn world(game: &Game, fx: &Fx) {
    clear_background(ui::BG);
    let view = View::new(fx);
    ground(&view);
    for line in &game.lines {
        track(&view, line);
    }
    depot(&view, game);
    for (i, line) in game.lines.iter().enumerate() {
        train(&view, line, game.unloadable_line() == Some(i));
    }
    pickups(&view, game);
    enemies(&view, game);
    projectiles(&view, game, fx);
    player(&view, game);
    for p in &fx.particles {
        let mut c = p.color;
        c.a *= (p.life / p.max_life).clamp(0.0, 1.0);
        let s = view.s(p.pos);
        draw_rectangle(s.x - p.size / 2.0, s.y - p.size / 2.0, p.size, p.size, c);
    }
    for t in &fx.texts {
        let s = view.s(t.pos);
        let mut c = t.color;
        c.a = t.life.min(1.0);
        let w = measure_text(&t.text, None, 20, 1.0).width;
        draw_text(&t.text, s.x - w / 2.0, s.y, 20.0, c);
    }
    offscreen_markers(&view, game);
}

fn ground(view: &View) {
    let tl = view.s(V2::new(-ARENA.x, -ARENA.y));
    let br = view.s(ARENA);
    draw_rectangle(tl.x, tl.y, br.x - tl.x, br.y - tl.y, ui::GROUND);
    let grid = Color::new(1.0, 1.0, 1.0, 0.035);
    let mut x = -ARENA.x;
    while x <= ARENA.x {
        let a = view.s(V2::new(x, -ARENA.y));
        draw_line(a.x, tl.y, a.x, br.y, 1.0, grid);
        x += 100.0;
    }
    let mut y = -ARENA.y;
    while y <= ARENA.y {
        let a = view.s(V2::new(-ARENA.x, y));
        draw_line(tl.x, a.y, br.x, a.y, 1.0, grid);
        y += 100.0;
    }
    draw_rectangle_lines(tl.x, tl.y, br.x - tl.x, br.y - tl.y, 4.0, ui::FENCE);
}

fn track(view: &View, line: &Line) {
    let dir = v(line.dir());
    let side = vec2(-dir.y, dir.x);
    let start = DEPOT_RADIUS + 4.0;
    let end = rules::game::TRACK_LEN;
    // Sleepers.
    let mut d = start;
    while d < end {
        let p = line.dir() * d;
        if view.on_screen(p, 30.0) {
            rect_rot(view.s(p), side, 26.0, 7.0, ui::SLEEPER);
        }
        d += 22.0;
    }
    let a = view.s(line.dir() * start);
    let b = view.s(line.dir() * end);
    for o in [-7.0, 7.0] {
        let off = side * o;
        draw_line(
            a.x + off.x,
            a.y + off.y,
            b.x + off.x,
            b.y + off.y,
            2.5,
            ui::RAIL,
        );
    }
    // Platforms on both sides of the track.
    let c = line.platform() + line.dir() * 40.0;
    for o in [-34.0, 34.0] {
        rect_rot(view.s(c) + side * o, dir, 130.0, 22.0, ui::PLATFORM);
    }
    let color = cargo_color(line.cargo);
    rect_rot(view.s(c) + side * 48.0, dir, 130.0, 3.0, color);
    rect_rot(view.s(c) - side * 48.0, dir, 130.0, 3.0, color);
}

fn train(view: &View, line: &Line, can_unload: bool) {
    let Some(front) = line.train_pos() else {
        return;
    };
    let back = v(line.dir());
    let f = view.s(front);
    // Locomotive.
    let loco_len = 50.0;
    let lc = f + back * (loco_len / 2.0);
    rect_rot(lc, back, loco_len, 26.0, Color::new(0.55, 0.12, 0.1, 1.0));
    rect_rot(
        lc + back * 14.0,
        back,
        18.0,
        22.0,
        Color::new(0.15, 0.15, 0.17, 1.0),
    );
    draw_circle(f.x + back.x * 8.0, f.y + back.y * 8.0, 6.0, DARKGRAY);
    draw_circle(f.x, f.y, 4.0, YELLOW);
    // Wagons.
    let color = cargo_color(line.cargo);
    let mut at = loco_len + 6.0;
    for _ in 0..line.wagons() {
        let wc = f + back * (at + 18.0);
        rect_rot(wc, back, 36.0, 24.0, Color::new(0.2, 0.2, 0.22, 1.0));
        rect_rot(wc, back, 30.0, 18.0, color);
        at += 42.0;
    }
    if let TrainState::Docked(t) = line.state {
        let p = view.s(line.platform());
        let pulse = 0.5 + 0.5 * (get_time() as f32 * 6.0).sin();
        let ring = if can_unload {
            GREEN
        } else {
            Color::new(1.0, 1.0, 1.0, 0.3 + 0.3 * pulse)
        };
        draw_circle_lines(p.x, p.y, UNLOAD_RANGE, 2.0, ring);
        let frac = t / rules::game::DOCK_TIME;
        let bar_w = 70.0;
        draw_rectangle(
            p.x - bar_w / 2.0,
            p.y - 40.0,
            bar_w,
            6.0,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        let bc = if frac < 0.3 { RED } else { YELLOW };
        draw_rectangle(p.x - bar_w / 2.0, p.y - 40.0, bar_w * frac, 6.0, bc);
        let label = if can_unload { "[E] UNLOAD" } else { "DOCKED" };
        let w = measure_text(label, None, 20, 1.0).width;
        draw_text(label, p.x - w / 2.0, p.y - 46.0, 20.0, WHITE);
    }
}

fn depot(view: &View, game: &Game) {
    let c = view.s(V2::ZERO);
    draw_poly(
        c.x,
        c.y,
        8,
        DEPOT_RADIUS + 6.0,
        22.5,
        Color::new(0.2, 0.18, 0.16, 1.0),
    );
    draw_poly(
        c.x,
        c.y,
        8,
        DEPOT_RADIUS,
        22.5,
        Color::new(0.55, 0.32, 0.22, 1.0),
    );
    draw_poly(
        c.x,
        c.y,
        8,
        DEPOT_RADIUS * 0.7,
        22.5,
        Color::new(0.42, 0.24, 0.17, 1.0),
    );
    draw_poly(
        c.x,
        c.y,
        8,
        DEPOT_RADIUS * 0.25,
        22.5,
        Color::new(0.85, 0.75, 0.4, 1.0),
    );
    // Turntable spokes.
    let t = get_time() as f32 * 0.3;
    for k in 0..4 {
        let a = t + k as f32 * std::f32::consts::FRAC_PI_4 * 2.0;
        let d = vec2(a.cos(), a.sin()) * DEPOT_RADIUS * 0.68;
        draw_line(
            c.x - d.x,
            c.y - d.y,
            c.x + d.x,
            c.y + d.y,
            3.0,
            Color::new(0.3, 0.17, 0.12, 1.0),
        );
    }
    let frac = (game.depot_hp / DEPOT_MAX_HP).clamp(0.0, 1.0);
    let w = 110.0;
    draw_rectangle(
        c.x - w / 2.0,
        c.y + DEPOT_RADIUS + 12.0,
        w,
        8.0,
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    draw_rectangle(
        c.x - w / 2.0,
        c.y + DEPOT_RADIUS + 12.0,
        w * frac,
        8.0,
        ui::depot_hp_color(frac),
    );
}

fn pickups(view: &View, game: &Game) {
    for p in &game.pickups {
        if !view.on_screen(p.pos, 20.0) {
            continue;
        }
        let s = view.s(p.pos);
        match p.kind {
            PickupKind::Gem(xp) => {
                let r = 4.0 + xp.min(6) as f32;
                let c = Color::new(0.35, 0.9, 1.0, 1.0);
                draw_poly(s.x, s.y, 4, r, 0.0, c);
            }
            PickupKind::Crate(cargo) => {
                let bob = (p.age * 5.0).sin() * 2.0;
                draw_rectangle(
                    s.x - 9.0,
                    s.y - 9.0 + bob,
                    18.0,
                    18.0,
                    Color::new(0.25, 0.17, 0.1, 1.0),
                );
                draw_rectangle(s.x - 7.0, s.y - 7.0 + bob, 14.0, 14.0, cargo_color(cargo));
                draw_line(
                    s.x - 7.0,
                    s.y - 7.0 + bob,
                    s.x + 7.0,
                    s.y + 7.0 + bob,
                    1.5,
                    Color::new(0.0, 0.0, 0.0, 0.4),
                );
            }
        }
    }
}

fn enemies(view: &View, game: &Game) {
    for e in &game.enemies {
        if !view.on_screen(e.pos, e.radius + 10.0) {
            continue;
        }
        let s = view.s(e.pos);
        let col = if e.flash > 0.0 {
            WHITE
        } else {
            enemy_color(e.kind)
        };
        let to = (game.player.pos - e.pos).norm();
        let rot = to.angle().to_degrees();
        match e.kind {
            EnemyKind::Rustbug => {
                draw_circle(s.x, s.y, e.radius, col);
                for k in [-1.0, 1.0] {
                    let eye = s + vec2(to.x, to.y) * 5.0 + vec2(-to.y, to.x) * (4.0 * k);
                    draw_circle(eye.x, eye.y, 2.5, ORANGE);
                }
            }
            EnemyKind::Sprinter => {
                draw_poly(s.x, s.y, 3, e.radius + 3.0, rot, col);
            }
            EnemyKind::Brute => {
                draw_poly(
                    s.x,
                    s.y,
                    4,
                    e.radius + 3.0,
                    rot + 45.0,
                    Color::new(0.15, 0.12, 0.18, 1.0),
                );
                draw_poly(s.x, s.y, 4, e.radius, rot + 45.0, col);
                draw_circle(s.x + to.x * 8.0, s.y + to.y * 8.0, 4.0, RED);
            }
            EnemyKind::Boss => {
                draw_poly(
                    s.x,
                    s.y,
                    8,
                    e.radius + 5.0,
                    rot,
                    Color::new(0.1, 0.08, 0.12, 1.0),
                );
                draw_poly(s.x, s.y, 8, e.radius, rot, col);
                let pulse = 8.0 + 3.0 * (get_time() as f32 * 8.0).sin();
                draw_circle(s.x, s.y, pulse, RED);
                let frac = (e.hp / e.max_hp).clamp(0.0, 1.0);
                draw_rectangle(s.x - 50.0, s.y - e.radius - 18.0, 100.0, 7.0, BLACK);
                draw_rectangle(s.x - 50.0, s.y - e.radius - 18.0, 100.0 * frac, 7.0, RED);
            }
        }
    }
}

fn projectiles(view: &View, game: &Game, fx: &Fx) {
    for p in &game.projectiles {
        let s = view.s(p.pos);
        match p.kind {
            ShotKind::Rivet { .. } => {
                let d = vec2(p.vel.x, p.vel.y).normalize_or_zero() * 7.0;
                draw_line(
                    s.x - d.x,
                    s.y - d.y,
                    s.x + d.x,
                    s.y + d.y,
                    3.0,
                    Color::new(0.85, 0.9, 1.0, 1.0),
                );
            }
            ShotKind::Shell { .. } => {
                // Fake arc: lift the shell towards mid-flight.
                let lift = (p.life * 4.0).sin().abs() * 14.0;
                draw_circle(s.x, s.y, 3.0, Color::new(0.0, 0.0, 0.0, 0.35));
                draw_circle(s.x, s.y - lift, 7.0, Color::new(1.0, 0.45, 0.1, 1.0));
                draw_circle(s.x, s.y - lift, 3.5, YELLOW);
            }
        }
    }
    for &(pos, r, life) in &fx.rings {
        let s = view.s(pos);
        let t = 1.0 - life / 0.35;
        draw_circle(
            s.x,
            s.y,
            r * (0.4 + 0.6 * t),
            Color::new(1.0, 0.5, 0.1, 0.35 * (1.0 - t)),
        );
        draw_circle_lines(
            s.x,
            s.y,
            r * (0.4 + 0.6 * t),
            3.0,
            Color::new(1.0, 0.8, 0.3, 1.0 - t),
        );
    }
    for c in game.orbit_positions() {
        let s = view.s(c);
        draw_rectangle(
            s.x - 10.0,
            s.y - 10.0,
            20.0,
            20.0,
            Color::new(0.3, 0.2, 0.1, 1.0),
        );
        draw_rectangle(
            s.x - 8.0,
            s.y - 8.0,
            16.0,
            16.0,
            cargo_color(rules::data::Cargo::Goods),
        );
    }
}

fn player(view: &View, game: &Game) {
    let p = &game.player;
    let s = view.s(p.pos);
    if p.invuln > 0.0 && (get_time() * 20.0) as i32 % 2 == 0 {
        return;
    }
    let mag = Color::new(0.35, 0.9, 1.0, 0.06);
    draw_circle(s.x, s.y, p.magnet, mag);
    draw_circle(
        s.x,
        s.y + 3.0,
        PLAYER_RADIUS,
        Color::new(0.0, 0.0, 0.0, 0.3),
    );
    draw_circle(s.x, s.y, PLAYER_RADIUS, Color::new(0.2, 0.35, 0.7, 1.0));
    draw_circle(
        s.x,
        s.y,
        PLAYER_RADIUS - 4.0,
        Color::new(0.95, 0.8, 0.65, 1.0),
    );
    // Peaked cap pointing the way the depot master faces.
    let f = vec2(p.facing.x, p.facing.y);
    draw_circle(
        s.x - f.x * 2.0,
        s.y - f.y * 2.0,
        7.0,
        Color::new(0.1, 0.15, 0.35, 1.0),
    );
    draw_line(
        s.x,
        s.y,
        s.x + f.x * 12.0,
        s.y + f.y * 12.0,
        4.0,
        Color::new(0.1, 0.15, 0.35, 1.0),
    );
}

/// Arrows on the screen edge pointing at docked trains that are out of view.
fn offscreen_markers(view: &View, game: &Game) {
    let (w, h) = (screen_width(), screen_height());
    for line in &game.lines {
        if !matches!(line.state, TrainState::Docked(_)) || view.on_screen(line.platform(), -30.0) {
            continue;
        }
        let s = view.s(line.platform());
        let c = vec2(w / 2.0, h / 2.0);
        let d = (s - c).normalize_or_zero();
        let k =
            ((w / 2.0 - 30.0) / d.x.abs().max(1e-3)).min((h / 2.0 - 30.0) / d.y.abs().max(1e-3));
        let tip = c + d * k;
        let side = vec2(-d.y, d.x);
        draw_triangle(
            tip,
            tip - d * 22.0 + side * 12.0,
            tip - d * 22.0 - side * 12.0,
            cargo_color(line.cargo),
        );
    }
}
