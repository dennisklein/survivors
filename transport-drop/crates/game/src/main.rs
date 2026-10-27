//! Transport Drop: macroquad front end (input, rendering, HUD, audio).

mod audio;

use audio::{Audio, Sfx};
use macroquad::prelude::*;
use sim::game::{
    CRATE_FALL, EnemyKind, Event, Flight, Game, Input, KINDS, Kind, Phase, WAVES, WORLD, xp_needed,
};
use sim::math::V2;

fn window_conf() -> Conf {
    Conf {
        window_title: "Transport Drop".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
}

struct Ring {
    pos: Vec2,
    t: f32,
    max_r: f32,
    color: Color,
}

struct Beam {
    from: Vec2,
    to: Vec2,
    t: f32,
}

#[derive(Default)]
struct Fx {
    parts: Vec<Particle>,
    rings: Vec<Ring>,
    beams: Vec<Beam>,
    shake: f32,
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

impl Fx {
    fn burst(&mut self, at: Vec2, n: usize, speed: f32, size: f32, color: Color) {
        for _ in 0..n {
            let a = rand::gen_range(0.0, std::f32::consts::TAU);
            let s = rand::gen_range(0.2, 1.0) * speed;
            let life = rand::gen_range(0.25, 0.7);
            self.parts.push(Particle {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * s,
                life,
                max: life,
                size,
                color,
            });
        }
    }

    fn on_event(&mut self, e: &Event) {
        if self.parts.len() > 1500 {
            return;
        }
        match *e {
            Event::Hit(p) => self.burst(v(p), 3, 90.0, 2.0, ORANGE),
            Event::Kill { pos, big } => {
                let n = if big { 40 } else { 12 };
                self.burst(v(pos), n, if big { 220.0 } else { 140.0 }, 3.0, ORANGE);
                self.burst(v(pos), n / 2, 60.0, 4.0, DARKGRAY);
                if big {
                    self.shake = self.shake.max(8.0);
                }
            }
            Event::TurretShot { from, to } => self.beams.push(Beam {
                from: v(from),
                to: v(to),
                t: 0.1,
            }),
            Event::CrateLanded(p) => {
                self.rings.push(Ring {
                    pos: v(p),
                    t: 0.0,
                    max_r: 60.0,
                    color: Color::new(1.0, 0.8, 0.3, 1.0),
                });
                self.burst(v(p), 24, 160.0, 3.0, Color::new(0.7, 0.6, 0.4, 1.0));
                self.shake = self.shake.max(6.0);
            }
            Event::Touchdown(p) => {
                self.rings.push(Ring {
                    pos: v(p),
                    t: 0.0,
                    max_r: 140.0,
                    color: Color::new(0.4, 0.9, 1.0, 1.0),
                });
                self.burst(v(p), 30, 200.0, 3.0, Color::new(0.6, 0.6, 0.5, 1.0));
                self.shake = self.shake.max(7.0);
            }
            Event::StructureLost(p) => {
                self.burst(v(p), 30, 180.0, 3.5, RED);
                self.shake = self.shake.max(5.0);
            }
            Event::Hurt => self.shake = self.shake.max(3.0),
            _ => {}
        }
    }

    fn update(&mut self, dt: f32) {
        self.parts.retain_mut(|p| {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.vel *= 1.0 - 3.0 * dt;
            p.life > 0.0
        });
        self.rings.retain_mut(|r| {
            r.t += dt / 0.35;
            r.t < 1.0
        });
        self.beams.retain_mut(|b| {
            b.t -= dt;
            b.t > 0.0
        });
        self.shake = (self.shake - dt * 30.0).max(0.0);
    }

    fn draw(&self) {
        for r in &self.rings {
            let mut c = r.color;
            c.a = 1.0 - r.t;
            draw_circle_lines(r.pos.x, r.pos.y, r.max_r * r.t, 4.0, c);
        }
        for b in &self.beams {
            let c = Color::new(1.0, 0.3, 0.3, b.t * 10.0);
            draw_line(b.from.x, b.from.y, b.to.x, b.to.y, 2.5, c);
        }
        for p in &self.parts {
            let mut c = p.color;
            c.a = p.life / p.max;
            draw_rectangle(
                p.pos.x - p.size / 2.0,
                p.pos.y - p.size / 2.0,
                p.size,
                p.size,
                c,
            );
        }
    }
}

const TEAM: Color = Color::new(0.25, 0.55, 1.0, 1.0);
const ENEMY: Color = Color::new(0.85, 0.25, 0.2, 1.0);

fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Turret => Color::new(0.9, 0.35, 0.3, 1.0),
        Kind::Wall => Color::new(0.55, 0.55, 0.5, 1.0),
        Kind::Mex => Color::new(0.95, 0.8, 0.3, 1.0),
        Kind::Repair => Color::new(0.35, 0.9, 0.5, 1.0),
    }
}

fn draw_structure(k: Kind, p: Vec2, alpha: f32) {
    let mut c = kind_color(k);
    c.a = alpha;
    let dark = Color::new(0.12, 0.14, 0.16, alpha);
    match k {
        Kind::Turret => {
            draw_rectangle(p.x - 16.0, p.y - 16.0, 32.0, 32.0, dark);
            draw_circle(p.x, p.y, 11.0, c);
            draw_rectangle(p.x - 2.0, p.y - 22.0, 4.0, 14.0, c);
        }
        Kind::Wall => {
            for (dx, dy) in [(-11.0, -11.0), (11.0, -11.0), (-11.0, 11.0), (11.0, 11.0)] {
                draw_poly(p.x + dx, p.y + dy, 4, 10.0, 45.0, c);
            }
        }
        Kind::Mex => {
            draw_poly(p.x, p.y, 8, 18.0, 22.5, dark);
            draw_poly_lines(p.x, p.y, 8, 18.0, 22.5, 2.0, c);
            let a = get_time() as f32 * 3.0;
            for i in 0..3 {
                let a = a + i as f32 * 2.094;
                draw_line(p.x, p.y, p.x + a.cos() * 13.0, p.y + a.sin() * 13.0, 3.0, c);
            }
        }
        Kind::Repair => {
            draw_rectangle(p.x - 18.0, p.y - 18.0, 36.0, 36.0, dark);
            draw_rectangle(p.x - 4.0, p.y - 13.0, 8.0, 26.0, c);
            draw_rectangle(p.x - 13.0, p.y - 4.0, 26.0, 8.0, c);
        }
    }
}

fn draw_enemy(kind: EnemyKind, p: Vec2, flash: bool, face: f32) {
    let r = kind.stats().3;
    let c = if flash { WHITE } else { ENEMY };
    let dark = Color::new(0.3, 0.08, 0.06, 1.0);
    match kind {
        EnemyKind::Raider => {
            draw_poly(p.x, p.y, 3, r, face.to_degrees(), c);
        }
        EnemyKind::Grunt => {
            draw_circle(p.x, p.y, r, dark);
            draw_circle(p.x, p.y, r - 3.0, c);
            let d = vec2(face.cos(), face.sin()) * r;
            draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 3.0, dark);
        }
        EnemyKind::Tank => {
            draw_poly(p.x, p.y, 6, r, face.to_degrees(), dark);
            draw_poly(p.x, p.y, 6, r - 4.0, face.to_degrees(), c);
            let d = vec2(face.cos(), face.sin()) * (r + 8.0);
            draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 5.0, dark);
        }
        EnemyKind::Boss => {
            draw_poly(p.x, p.y, 8, r, get_time() as f32 * 20.0, dark);
            draw_poly(p.x, p.y, 8, r - 6.0, get_time() as f32 * 20.0, c);
            draw_circle(p.x, p.y, 12.0, YELLOW);
        }
    }
}

fn draw_commander(p: Vec2, face: f32) {
    draw_circle(p.x, p.y, 16.0, Color::new(0.1, 0.18, 0.35, 1.0));
    draw_circle(p.x, p.y, 13.0, TEAM);
    let d = vec2(face.cos(), face.sin());
    draw_line(p.x, p.y, p.x + d.x * 22.0, p.y + d.y * 22.0, 5.0, LIGHTGRAY);
    draw_circle(p.x, p.y, 5.0, WHITE);
}

fn draw_transport(p: Vec2, alt: f32, carrying: bool) {
    let lift = alt * 46.0;
    let scale = 1.0 + alt * 0.25;
    // Shadow on the ground.
    draw_ellipse(
        p.x + alt * 18.0,
        p.y + alt * 18.0,
        40.0 * scale,
        16.0 * scale,
        0.0,
        Color::new(0.0, 0.0, 0.0, 0.25 + alt * 0.1),
    );
    let q = vec2(p.x, p.y - lift);
    let body = Color::new(0.3, 0.42, 0.6, 1.0);
    let s = scale;
    draw_rectangle(q.x - 42.0 * s, q.y - 4.0 * s, 84.0 * s, 8.0 * s, body);
    draw_ellipse(
        q.x,
        q.y,
        16.0 * s,
        30.0 * s,
        0.0,
        Color::new(0.4, 0.55, 0.8, 1.0),
    );
    draw_rectangle(q.x - 10.0 * s, q.y + 22.0 * s, 20.0 * s, 6.0 * s, body);
    if carrying {
        draw_circle(q.x, q.y, 9.0 * s, TEAM);
    }
    // Rotors.
    let a = get_time() as f32 * 40.0;
    for side in [-1.0f32, 1.0] {
        let c = vec2(q.x + side * 38.0 * s, q.y);
        draw_circle_lines(c.x, c.y, 14.0 * s, 1.0, Color::new(0.8, 0.9, 1.0, 0.4));
        for k in 0..2 {
            let r = a * side + k as f32 * std::f32::consts::PI;
            let d = vec2(r.cos(), r.sin()) * 14.0 * s;
            draw_line(c.x - d.x, c.y - d.y, c.x + d.x, c.y + d.y, 2.0, LIGHTGRAY);
        }
    }
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fg: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fg);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn draw_ground(cam: Vec2) {
    clear_background(Color::new(0.16, 0.2, 0.14, 1.0));
    let (w, h) = (screen_width(), screen_height());
    let step = 80.0;
    let x0 = ((cam.x - w / 2.0) / step).floor() * step;
    let y0 = ((cam.y - h / 2.0) / step).floor() * step;
    let line = Color::new(0.22, 0.27, 0.19, 1.0);
    let mut x = x0;
    while x < cam.x + w / 2.0 + step {
        draw_line(x, cam.y - h, x, cam.y + h, 1.0, line);
        x += step;
    }
    let mut y = y0;
    while y < cam.y + h / 2.0 + step {
        draw_line(cam.x - w, y, cam.x + w, y, 1.0, line);
        y += step;
    }
    // Deterministic rocks/craters so the map has landmarks.
    let cell = 240.0;
    let cx0 = ((cam.x - w / 2.0) / cell).floor() as i32 - 1;
    let cy0 = ((cam.y - h / 2.0) / cell).floor() as i32 - 1;
    for cx in cx0..cx0 + (w / cell) as i32 + 3 {
        for cy in cy0..cy0 + (h / cell) as i32 + 3 {
            let hsh = (cx.wrapping_mul(73_856_093) ^ cy.wrapping_mul(19_349_663)) as u32;
            if !hsh.is_multiple_of(3) {
                continue;
            }
            let px = cx as f32 * cell + (hsh % 200) as f32;
            let py = cy as f32 * cell + ((hsh / 200) % 200) as f32;
            let r = 6.0 + (hsh % 17) as f32;
            draw_circle(px, py, r, Color::new(0.13, 0.16, 0.11, 1.0));
            draw_circle(
                px - 2.0,
                py - 2.0,
                r * 0.6,
                Color::new(0.2, 0.24, 0.17, 1.0),
            );
        }
    }
    draw_rectangle_lines(0.0, 0.0, WORLD, WORLD, 6.0, Color::new(0.8, 0.3, 0.2, 0.7));
}

struct App {
    game: Game,
    fx: Fx,
    face: f32,
    best_wave: u32,
}

impl App {
    fn new() -> Self {
        App {
            game: Game::new(macroquad::miniquad::date::now() as u64),
            fx: Fx::default(),
            face: 0.0,
            best_wave: 0,
        }
    }
}

fn draw_world(app: &App, cam: Vec2, mouse_world: Vec2) {
    let g = &app.game;
    set_camera(&Camera2D {
        target: cam,
        zoom: vec2(2.0 / screen_width(), 2.0 / screen_height()),
        ..Default::default()
    });
    draw_ground(cam);
    for w in &g.wrecks {
        draw_rectangle(
            w.pos.x - 4.0,
            w.pos.y - 4.0,
            8.0,
            8.0,
            Color::new(0.4, 0.4, 0.38, 1.0),
        );
        draw_rectangle(
            w.pos.x - 2.0,
            w.pos.y - 2.0,
            4.0,
            4.0,
            Color::new(0.85, 0.9, 1.0, 1.0),
        );
    }
    for s in &g.structures {
        let p = v(s.pos);
        if s.kind == Kind::Repair && !g.airborne() && s.pos.dist(g.pos) < 90.0 {
            draw_circle_lines(p.x, p.y, 90.0, 2.0, Color::new(0.3, 1.0, 0.5, 0.5));
        }
        draw_structure(s.kind, p, 1.0);
        if s.hp < s.kind.max_hp() {
            bar(
                p.x - 18.0,
                p.y + 22.0,
                36.0,
                4.0,
                s.hp / s.kind.max_hp(),
                GREEN,
            );
        }
    }
    for c in &g.crates {
        let p = v(c.pos);
        let k = 1.0 - c.t / CRATE_FALL;
        let r = 30.0 + 10.0 * (1.0 - k);
        draw_circle_lines(p.x, p.y, r, 2.0, Color::new(1.0, 0.85, 0.2, 0.8));
        draw_circle(p.x, p.y, 3.0, YELLOW);
        // Falling crate with parachute, shadow growing as it nears.
        let h = c.t / CRATE_FALL * 400.0;
        draw_circle(p.x, p.y, 10.0 * k, Color::new(0.0, 0.0, 0.0, 0.3));
        let q = vec2(p.x, p.y - h);
        draw_rectangle(
            q.x - 12.0,
            q.y - 12.0,
            24.0,
            24.0,
            Color::new(0.55, 0.42, 0.25, 1.0),
        );
        draw_rectangle_lines(q.x - 12.0, q.y - 12.0, 24.0, 24.0, 2.0, kind_color(c.kind));
    }
    for e in &g.enemies {
        let face = (g.pos - e.pos).angle();
        draw_enemy(e.kind, v(e.pos), e.flash > 0.0, face);
        if e.kind != EnemyKind::Raider && e.hp < e.max_hp {
            let r = e.kind.stats().3;
            bar(
                e.pos.x - r,
                e.pos.y - r - 8.0,
                r * 2.0,
                3.0,
                e.hp / e.max_hp,
                RED,
            );
        }
    }
    for b in &g.bullets {
        if b.hostile {
            draw_circle(b.pos.x, b.pos.y, 5.0, Color::new(1.0, 0.5, 0.1, 1.0));
        } else {
            let t = b.pos - b.vel * 0.02;
            draw_line(
                t.x,
                t.y,
                b.pos.x,
                b.pos.y,
                3.0,
                Color::new(0.5, 0.9, 1.0, 1.0),
            );
        }
    }
    let p = v(g.pos);
    let alt = g.altitude();
    if alt < 1.0 {
        draw_commander(p, app.face);
    }
    if alt > 0.0 {
        draw_transport(p, alt, alt >= 1.0);
    }
    if g.flight == Flight::Ground {
        draw_circle_lines(
            p.x,
            p.y,
            g.stats.magnet,
            1.0,
            Color::new(0.6, 0.8, 1.0, 0.15),
        );
    } else {
        // Preview of the touchdown blast.
        draw_circle_lines(
            p.x,
            p.y,
            g.stats.shock_radius,
            1.5,
            Color::new(0.4, 0.9, 1.0, 0.35),
        );
    }
    app.fx.draw();
    // Drop cursor.
    let can = g.metal >= g.cost(g.selected);
    let mut c = kind_color(g.selected);
    c.a = if can { 0.5 } else { 0.2 };
    draw_structure(g.selected, mouse_world, c.a);
    draw_circle_lines(
        mouse_world.x,
        mouse_world.y,
        g.selected.radius() + 34.0,
        1.0,
        if can {
            Color::new(1.0, 0.85, 0.2, 0.6)
        } else {
            Color::new(1.0, 0.2, 0.2, 0.5)
        },
    );
    set_default_camera();
}

fn draw_hud(app: &App, muted: bool) {
    let g = &app.game;
    let w = screen_width();
    // Top-left: health, fuel, metal.
    draw_rectangle(10.0, 10.0, 290.0, 104.0, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_text("HP", 20.0, 32.0, 22.0, WHITE);
    bar(
        60.0,
        18.0,
        230.0,
        16.0,
        g.hp / g.stats.max_hp,
        Color::new(0.3, 0.9, 0.4, 1.0),
    );
    draw_text(
        format!("{:.0}/{:.0}", g.hp, g.stats.max_hp),
        66.0,
        31.0,
        18.0,
        WHITE,
    );
    draw_text("FUEL", 20.0, 56.0, 18.0, WHITE);
    let fuel_c = if g.fuel < 2.0 && g.airborne() {
        if (get_time() * 6.0) as i32 % 2 == 0 {
            RED
        } else {
            ORANGE
        }
    } else {
        Color::new(0.3, 0.7, 1.0, 1.0)
    };
    bar(60.0, 44.0, 230.0, 14.0, g.fuel / g.stats.max_fuel, fuel_c);
    draw_text(
        format!("METAL {}", g.metal),
        20.0,
        84.0,
        24.0,
        Color::new(0.85, 0.9, 1.0, 1.0),
    );
    let state = match g.flight {
        Flight::Air => "AIRBORNE - safe, can't reclaim",
        Flight::Ground => "GROUNDED - fighting",
        Flight::Landing(_) => "LANDING...",
        Flight::Lifting(_) => "LIFTING OFF...",
    };
    draw_text(
        state,
        20.0,
        106.0,
        18.0,
        if g.airborne() { SKYBLUE } else { YELLOW },
    );

    // Top-center: wave + XP.
    let wave_txt = if g.boss_spawned {
        "FINAL WAVE - DESTROY THE BOSS".to_string()
    } else {
        format!("WAVE {}/{}", g.wave, WAVES)
    };
    centered(&wave_txt, 32.0, 30.0, WHITE);
    if !g.boss_spawned {
        bar(w / 2.0 - 120.0, 40.0, 240.0, 6.0, g.wave_progress(), ORANGE);
    }
    bar(
        w / 2.0 - 200.0,
        54.0,
        400.0,
        10.0,
        g.xp as f32 / xp_needed(g.level) as f32,
        Color::new(0.8, 0.5, 1.0, 1.0),
    );
    centered(
        &format!("LV {}", g.level),
        82.0,
        20.0,
        Color::new(0.85, 0.7, 1.0, 1.0),
    );
    if let Some(boss) = g.enemies.iter().find(|e| e.kind == EnemyKind::Boss) {
        bar(
            w / 2.0 - 250.0,
            92.0,
            500.0,
            14.0,
            boss.hp / boss.max_hp,
            RED,
        );
    }

    // Top-right.
    let t = format!(
        "Kills {}  Time {:02}:{:02}",
        g.kills,
        g.time as u32 / 60,
        g.time as u32 % 60
    );
    draw_text(&t, w - 270.0, 32.0, 22.0, WHITE);
    draw_text(
        if muted {
            "[M] sound OFF"
        } else {
            "[M] sound on"
        },
        w - 270.0,
        56.0,
        18.0,
        GRAY,
    );

    // Bottom: drop palette.
    let h = screen_height();
    let bw = 170.0;
    let x0 = w / 2.0 - bw * 2.0;
    for (i, k) in KINDS.iter().enumerate() {
        let x = x0 + i as f32 * bw;
        let sel = *k == g.selected;
        let can = g.metal >= g.cost(*k);
        draw_rectangle(
            x + 4.0,
            h - 74.0,
            bw - 8.0,
            64.0,
            if sel {
                Color::new(0.25, 0.3, 0.4, 0.85)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.5)
            },
        );
        if sel {
            draw_rectangle_lines(x + 4.0, h - 74.0, bw - 8.0, 64.0, 2.0, YELLOW);
        }
        draw_structure(*k, vec2(x + 30.0, h - 42.0), 1.0);
        let tc = if can { WHITE } else { GRAY };
        draw_text(
            format!("{} {}", i + 1, k.name()),
            x + 52.0,
            h - 50.0,
            16.0,
            tc,
        );
        draw_text(
            format!("{} metal", g.cost(*k)),
            x + 52.0,
            h - 28.0,
            18.0,
            tc,
        );
    }
    draw_text(
        "WASD move  L land/lift  1-4 pick crate  Click: air-drop  M mute",
        14.0,
        h - 86.0,
        18.0,
        Color::new(1.0, 1.0, 1.0, 0.5),
    );
}

fn draw_title(t: f32) {
    clear_background(Color::new(0.07, 0.09, 0.08, 1.0));
    let h = screen_height();
    let wob = (t * 2.0).sin() * 6.0;
    draw_transport(vec2(screen_width() / 2.0, h * 0.3 + wob), 1.0, true);
    centered("TRANSPORT DROP", h * 0.45, 72.0, WHITE);
    centered(
        "Fly safe. Land to fight. Crush them with air-dropped crates.",
        h * 0.45 + 40.0,
        24.0,
        LIGHTGRAY,
    );
    let lines = [
        "WASD - move (fly fast in the air, fight on the ground)",
        "L - land / lift off (touchdown blasts nearby enemies)",
        "In the air you are invulnerable but burn fuel and can't reclaim",
        "On the ground you auto-fire and reclaim metal from wrecks",
        "1-4 - choose crate,  Mouse click - air-drop it anywhere (crushes enemies)",
        "M - mute.  Survive 10 waves and destroy the final boss.",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(
            l,
            h * 0.58 + i as f32 * 28.0,
            22.0,
            Color::new(0.75, 0.85, 1.0, 1.0),
        );
    }
    if (t * 2.0) as i32 % 2 == 0 {
        centered("PRESS ANY KEY OR CLICK TO START", h * 0.9, 30.0, YELLOW);
    }
}

fn draw_level_up(g: &Game) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    centered(
        "LEVEL UP!",
        screen_height() * 0.28,
        56.0,
        Color::new(0.85, 0.7, 1.0, 1.0),
    );
    centered(
        "Choose an upgrade (1-3 or click)",
        screen_height() * 0.28 + 36.0,
        22.0,
        LIGHTGRAY,
    );
    for (i, r) in choice_rects(g.choices.len()).iter().enumerate() {
        let hover = r.contains(mouse_position().into());
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            if hover {
                Color::new(0.3, 0.25, 0.45, 0.95)
            } else {
                Color::new(0.15, 0.13, 0.22, 0.95)
            },
        );
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.8, 0.6, 1.0, 1.0));
        let up = g.choices[i];
        draw_text(format!("{}", i + 1), r.x + 12.0, r.y + 30.0, 28.0, YELLOW);
        draw_text(up.title(), r.x + 16.0, r.y + 70.0, 28.0, WHITE);
        draw_text(up.desc(), r.x + 16.0, r.y + 104.0, 18.0, LIGHTGRAY);
    }
}

fn choice_rects(n: usize) -> Vec<Rect> {
    let (w, h) = (300.0, 140.0);
    let gap = 24.0;
    let total = n as f32 * w + (n as f32 - 1.0) * gap;
    let x0 = (screen_width() - total) / 2.0;
    (0..n)
        .map(|i| Rect::new(x0 + i as f32 * (w + gap), screen_height() * 0.4, w, h))
        .collect()
}

fn draw_end(app: &App) {
    let g = &app.game;
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.7),
    );
    let h = screen_height();
    if g.phase == Phase::Won {
        centered("VICTORY", h * 0.35, 80.0, GOLD);
        centered(
            "The boss is scrap. The sector is yours.",
            h * 0.35 + 44.0,
            26.0,
            WHITE,
        );
    } else {
        centered("COMMANDER LOST", h * 0.35, 80.0, RED);
        centered(
            &format!("Fell on wave {}/{}", g.wave, WAVES),
            h * 0.35 + 44.0,
            26.0,
            WHITE,
        );
    }
    centered(
        &format!(
            "Kills {}   Level {}   Time {:02}:{:02}   Best wave {}",
            g.kills,
            g.level,
            g.time as u32 / 60,
            g.time as u32 % 60,
            app.best_wave
        ),
        h * 0.5,
        24.0,
        LIGHTGRAY,
    );
    centered("Press ENTER or click to fly again", h * 0.62, 30.0, YELLOW);
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || !touches().is_empty()
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut app = App::new();
    let mut title_t = 0.0f32;

    loop {
        let dt = get_frame_time();
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match screen {
            Screen::Title => {
                title_t += dt;
                draw_title(title_t);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    audio.play(Sfx::Click);
                    app = App::new();
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
                let g = &mut app.game;
                let cam = v(g.pos)
                    + vec2(
                        rand::gen_range(-1.0, 1.0) * app.fx.shake,
                        rand::gen_range(-1.0, 1.0) * app.fx.shake,
                    );
                let (mx, my) = mouse_position();
                let mouse_world = cam + vec2(mx - screen_width() / 2.0, my - screen_height() / 2.0);
                match g.phase {
                    Phase::Playing => {
                        for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
                            .iter()
                            .enumerate()
                        {
                            if is_key_pressed(*key) {
                                g.selected = KINDS[i];
                                audio.play(Sfx::Click);
                            }
                        }
                        let mut dir = V2::ZERO;
                        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
                            dir.y -= 1.0;
                        }
                        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
                            dir.y += 1.0;
                        }
                        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
                            dir.x -= 1.0;
                        }
                        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
                            dir.x += 1.0;
                        }
                        let input = Input {
                            dir,
                            toggle_flight: is_key_pressed(KeyCode::L)
                                || is_key_pressed(KeyCode::Space),
                            drop_at: is_mouse_button_pressed(MouseButton::Left)
                                .then(|| V2::new(mouse_world.x, mouse_world.y)),
                        };
                        g.update(dt, &input);
                        if let Some(i) =
                            g.enemies.iter().enumerate().min_by(|a, b| {
                                a.1.pos.dist2(g.pos).total_cmp(&b.1.pos.dist2(g.pos))
                            })
                        {
                            app.face = (i.1.pos - g.pos).angle();
                        } else if dir != V2::ZERO {
                            app.face = dir.angle();
                        }
                    }
                    Phase::LevelUp => {
                        let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3];
                        let mut pick = keys.iter().position(|k| is_key_pressed(*k));
                        if is_mouse_button_pressed(MouseButton::Left) {
                            let m: Vec2 = mouse_position().into();
                            pick = choice_rects(g.choices.len())
                                .iter()
                                .position(|r| r.contains(m))
                                .or(pick);
                        }
                        if let Some(i) = pick {
                            g.choose(i);
                            audio.play(Sfx::Click);
                        }
                    }
                    Phase::Won | Phase::Lost => {
                        app.best_wave = app.best_wave.max(g.wave);
                        if is_key_pressed(KeyCode::Enter)
                            || is_mouse_button_pressed(MouseButton::Left)
                        {
                            let best = app.best_wave;
                            app = App::new();
                            app.best_wave = best;
                            audio.play(Sfx::Click);
                        }
                    }
                }
                for e in std::mem::take(&mut app.game.events) {
                    audio.on_event(&e);
                    app.fx.on_event(&e);
                }
                app.fx.update(dt);
                draw_world(&app, cam, mouse_world);
                draw_hud(&app, audio.muted);
                match app.game.phase {
                    Phase::LevelUp => draw_level_up(&app.game),
                    Phase::Won | Phase::Lost => draw_end(&app),
                    Phase::Playing => {}
                }
            }
        }
        next_frame().await;
    }
}
