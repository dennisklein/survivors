//! Scavenger Salvage: macroquad front end. Rules live in the `sim` crate.

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    EnemyKind, Event, GRAB_RANGE, Game, Input, Phase, RECYCLER_RADIUS, StructKind, V2, WAVE_TIME,
    WORLD, center, quota,
};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position};

const BG: Color = Color::new(0.09, 0.08, 0.07, 1.0);
const GRID: Color = Color::new(0.14, 0.13, 0.11, 1.0);
const ACCENT: Color = Color::new(0.35, 0.85, 1.0, 1.0);
const GOLD: Color = Color::new(1.0, 0.78, 0.3, 1.0);
const DANGER: Color = Color::new(1.0, 0.35, 0.3, 1.0);
const OK: Color = Color::new(0.45, 0.95, 0.5, 1.0);

fn window_conf() -> Conf {
    Conf {
        window_title: "Scavenger Salvage".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

/// World-to-screen transform.
#[derive(Clone, Copy)]
struct View {
    ox: f32,
    oy: f32,
    scale: f32,
}

impl View {
    fn centered_on(p: V2, scale: f32) -> View {
        View {
            ox: screen_width() / 2.0 - p.x * scale,
            oy: screen_height() / 2.0 - p.y * scale,
            scale,
        }
    }
    fn s(&self, p: V2) -> Vec2 {
        vec2(self.ox + p.x * self.scale, self.oy + p.y * self.scale)
    }
    fn l(&self, len: f32) -> f32 {
        len * self.scale
    }
}

struct Fx {
    pos: V2,
    t: f32,
    big: bool,
}

struct Floater {
    pos: V2,
    text: String,
    t: f32,
    color: Color,
}

enum Screen {
    Title,
    Playing,
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || !touches().is_empty()
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Grab", KeyCode::E),
        touch::small("Map", KeyCode::Tab),
        touch::small("Pause", KeyCode::P),
        touch::small("Mute", KeyCode::M),
        touch::small("Start", KeyCode::Enter),
    ]);
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut game = Game::new(1);
    let mut fx: Vec<Fx> = Vec::new();
    let mut floaters: Vec<Floater> = Vec::new();
    let mut banner = (String::new(), 0.0f32);
    let mut shake = 0.0f32;
    let mut overview = false;
    let mut paused = false;
    let mut zoom = 1.0f32;

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        clear_background(BG);
        match screen {
            Screen::Title => {
                draw_title(&audio);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    game = Game::new(macroquad::miniquad::date::now() as u64);
                    fx.clear();
                    floaters.clear();
                    banner = ("Wave 1: haul wrecks to the Recycler!".into(), 3.0);
                    screen = Screen::Playing;
                }
            }
            Screen::Playing => {
                if is_key_pressed(KeyCode::Tab) {
                    overview = !overview;
                }
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    paused = !paused;
                }
                if game.phase == Phase::Playing && !paused {
                    let input = read_input();
                    game.step(dt, &input);
                }
                if game.phase == Phase::LevelUp
                    && let Some(i) = level_up_pick()
                {
                    game.choose(i);
                }
                for e in game.drain_events() {
                    audio.on_event(&e);
                    match &e {
                        Event::Kill { pos, big } => {
                            fx.push(Fx {
                                pos: *pos,
                                t: 0.0,
                                big: *big,
                            });
                            if *big {
                                shake = shake.max(8.0);
                            }
                        }
                        Event::Deposit(v) => floaters.push(Floater {
                            pos: center(),
                            text: format!("+{v} salvage"),
                            t: 0.0,
                            color: GOLD,
                        }),
                        Event::BuildStart(k) => {
                            banner = (format!("Quota met! Building {}", k.name()), 2.5)
                        }
                        Event::Built(k) => {
                            banner = (format!("{} online: {}", k.name(), k.blurb()), 3.0);
                            fx.push(Fx {
                                pos: k.slot(),
                                t: 0.0,
                                big: true,
                            });
                        }
                        Event::WaveStart(w) => {
                            banner = if w % 5 == 0 {
                                (format!("Wave {w}: SCAVENGER MOTHERSHIP INBOUND"), 3.0)
                            } else {
                                (format!("Wave {w}"), 2.0)
                            }
                        }
                        Event::PlayerHurt => shake = shake.max(3.0),
                        Event::Lost => shake = 12.0,
                        _ => {}
                    }
                }
                fx.retain_mut(|f| {
                    f.t += dt;
                    f.t < if f.big { 0.7 } else { 0.35 }
                });
                floaters.retain_mut(|f| {
                    f.t += dt;
                    f.t < 1.4
                });
                banner.1 -= dt;
                shake = (shake - dt * 30.0).max(0.0);

                let base_scale = (screen_height() / 900.0).clamp(0.6, 1.6);
                let (target_zoom, focus) = if overview {
                    (screen_height() / 1500.0, center())
                } else {
                    (base_scale, game.player.pos)
                };
                zoom += (target_zoom - zoom) * (1.0 - (-dt * 8.0).exp());
                let mut view = View::centered_on(focus, zoom);
                if overview {
                    view = View::centered_on(center(), zoom);
                }
                view.ox += rand::gen_range(-shake, shake);
                view.oy += rand::gen_range(-shake, shake);

                draw_world(&game, &view, &fx, &floaters);
                draw_pointers(&game, &view);
                draw_hud(&game, &audio, overview);
                if banner.1 > 0.0 {
                    let a = banner.1.min(1.0);
                    centered(&banner.0, 110.0, 34.0, Color::new(1.0, 0.8, 0.3, a));
                }
                if paused && game.phase == Phase::Playing {
                    dim();
                    centered("PAUSED", screen_height() / 2.0, 60.0, WHITE);
                    centered(
                        "P / Esc to resume",
                        screen_height() / 2.0 + 40.0,
                        24.0,
                        LIGHTGRAY,
                    );
                }
                match game.phase {
                    Phase::LevelUp => draw_level_up(&game),
                    Phase::Won | Phase::Lost => {
                        draw_end(&game);
                        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                            game = Game::new(macroquad::miniquad::date::now() as u64);
                            fx.clear();
                            floaters.clear();
                            overview = false;
                        }
                    }
                    Phase::Playing => {}
                }
            }
        }
        touch::next_frame().await;
    }
}

fn read_input() -> Input {
    let mut d = V2::default();
    let k = |a: KeyCode, b: KeyCode| is_key_down(a) || is_key_down(b);
    if k(KeyCode::A, KeyCode::Left) {
        d.x -= 1.0;
    }
    if k(KeyCode::D, KeyCode::Right) {
        d.x += 1.0;
    }
    if k(KeyCode::W, KeyCode::Up) {
        d.y -= 1.0;
    }
    if k(KeyCode::S, KeyCode::Down) {
        d.y += 1.0;
    }
    Input {
        dir: d,
        grab: is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Space),
    }
}

fn card_rect(i: usize) -> Rect {
    let w = 300.0f32.min(screen_width() / 3.0 - 20.0);
    let h = 170.0;
    let total = w * 3.0 + 40.0;
    let x = (screen_width() - total) / 2.0 + i as f32 * (w + 20.0);
    Rect::new(x, screen_height() / 2.0 - h / 2.0, w, h)
}

fn level_up_pick() -> Option<usize> {
    for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .iter()
        .enumerate()
    {
        if is_key_pressed(*k) {
            return Some(i);
        }
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        let (mx, my) = mouse_position();
        return (0..3).find(|&i| card_rect(i).contains(vec2(mx, my)));
    }
    None
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn dim() {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
}

fn draw_title(audio: &Audio) {
    let t = get_time() as f32;
    let h = screen_height();
    // Drifting wreck silhouettes.
    for i in 0..14 {
        let x = ((i as f32 * 211.0 + t * 20.0) % (screen_width() + 200.0)) - 100.0;
        let y = (i as f32 * 137.0) % h;
        draw_wreck_shape(vec2(x, y), 18.0 + (i % 4) as f32 * 8.0, i as f32, 0.25);
    }
    centered("SCAVENGER SALVAGE", h * 0.28, 72.0, GOLD);
    centered(
        "Kill scavenger drones. Tow their wrecks home. Feed the Recycler.",
        h * 0.28 + 50.0,
        26.0,
        WHITE,
    );
    let lines = [
        "WASD / arrows  move        (your gun fires automatically)",
        "E / Space      grab or release a wreck (towing slows you!)",
        "Tab            base overview      P / Esc  pause      M  mute",
        "Each salvage quota auto-builds a base structure.",
        "Build the Evac Gate to win. Lose the Commander or Recycler and it's over.",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(l, h * 0.5 + i as f32 * 30.0, 22.0, LIGHTGRAY);
    }
    let blink = (t * 2.5).sin() > -0.3;
    if blink {
        centered("Press any key to start", h * 0.82, 34.0, ACCENT);
    }
    centered(
        if audio.muted {
            "sound: muted (M)"
        } else {
            "sound: on (M to mute)"
        },
        h * 0.82 + 34.0,
        18.0,
        GRAY,
    );
    centered(
        "A fan homage to Beyond All Reason",
        h - 20.0,
        16.0,
        DARKGRAY,
    );
}

/// Jagged polygon for a wreck, stable per seed.
fn draw_wreck_shape(c: Vec2, r: f32, seed: f32, alpha: f32) {
    let n = 7;
    let pts: Vec<Vec2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + seed;
            let k = 0.65 + 0.35 * ((seed * 7.3 + i as f32 * 2.1).sin() * 0.5 + 0.5);
            c + vec2(a.cos(), a.sin()) * r * k
        })
        .collect();
    let fill = Color::new(0.42, 0.36, 0.3, alpha);
    let edge = Color::new(0.75, 0.55, 0.3, alpha);
    for i in 0..n {
        draw_triangle(c, pts[i], pts[(i + 1) % n], fill);
    }
    for i in 0..n {
        draw_line(
            pts[i].x,
            pts[i].y,
            pts[(i + 1) % n].x,
            pts[(i + 1) % n].y,
            2.0,
            edge,
        );
    }
}

fn draw_world(g: &Game, v: &View, fx: &[Fx], floaters: &[Floater]) {
    let t = get_time() as f32;
    // Grid.
    let step = 100.0;
    let mut x = 0.0;
    while x <= WORLD {
        let a = v.s(V2::new(x, 0.0));
        let b = v.s(V2::new(x, WORLD));
        draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
        let a = v.s(V2::new(0.0, x));
        let b = v.s(V2::new(WORLD, x));
        draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
        x += step;
    }
    let tl = v.s(V2::new(0.0, 0.0));
    draw_rectangle_lines(tl.x, tl.y, v.l(WORLD), v.l(WORLD), 4.0, DANGER);

    // Base zone.
    let c = v.s(center());
    draw_circle(c.x, c.y, v.l(260.0), Color::new(0.2, 0.5, 0.7, 0.06));
    draw_circle_lines(c.x, c.y, v.l(260.0), 2.0, Color::new(0.3, 0.7, 1.0, 0.25));

    // Unbuilt slots: ghost outlines for the next structure.
    if let Some(k) = g.next_structure() {
        let p = v.s(k.slot());
        draw_circle_lines(p.x, p.y, v.l(26.0), 1.5, Color::new(1.0, 1.0, 1.0, 0.25));
    }

    // Recycler.
    draw_circle(
        c.x,
        c.y,
        v.l(RECYCLER_RADIUS),
        Color::new(0.16, 0.2, 0.24, 1.0),
    );
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU + t * 0.8;
        let p = c + vec2(a.cos(), a.sin()) * v.l(RECYCLER_RADIUS - 14.0);
        draw_circle(p.x, p.y, v.l(9.0), Color::new(0.4, 0.45, 0.5, 1.0));
    }
    draw_circle(c.x, c.y, v.l(28.0), GOLD);
    draw_circle(c.x, c.y, v.l(16.0), Color::new(0.2, 0.15, 0.05, 1.0));
    draw_circle_lines(c.x, c.y, v.l(RECYCLER_RADIUS), 3.0, ACCENT);
    bar(
        c.x - v.l(60.0),
        c.y + v.l(RECYCLER_RADIUS + 10.0),
        v.l(120.0),
        5.0,
        g.recycler_hp / g.recycler_max,
        OK,
    );

    for s in &g.structures {
        draw_structure(s, v, t);
    }

    // Wrecks and cables.
    for w in &g.wrecks {
        let p = v.s(w.pos);
        draw_wreck_shape(p, v.l(w.radius), w.id as f32 * 1.37, 1.0);
        if !w.towed && w.pos.dist(g.player.pos) < GRAB_RANGE + w.radius {
            draw_circle_lines(p.x, p.y, v.l(w.radius + 6.0), 2.0, ACCENT);
        }
    }
    let mut prev = g.player.pos;
    for id in &g.towing {
        if let Some(w) = g.wrecks.iter().find(|w| w.id == *id) {
            let a = v.s(prev);
            let b = v.s(w.pos);
            draw_line(a.x, a.y, b.x, b.y, 3.0, Color::new(0.9, 0.85, 0.4, 0.9));
            prev = w.pos;
        }
    }

    // Enemies.
    for e in &g.enemies {
        let (_, _, r, ..) = e.kind.stats();
        let p = v.s(e.pos);
        let r = v.l(r);
        let to = g.player.pos - e.pos;
        let a = to.y.atan2(to.x);
        let col = if e.flash > 0.0 {
            WHITE
        } else {
            match e.kind {
                EnemyKind::Drone => Color::new(0.85, 0.3, 0.25, 1.0),
                EnemyKind::Raider => Color::new(0.95, 0.55, 0.15, 1.0),
                EnemyKind::Brute => Color::new(0.6, 0.2, 0.55, 1.0),
                EnemyKind::Boss => Color::new(0.75, 0.1, 0.15, 1.0),
            }
        };
        let f = vec2(a.cos(), a.sin());
        let side = vec2(-f.y, f.x);
        match e.kind {
            EnemyKind::Drone | EnemyKind::Raider => {
                draw_triangle(p + f * r, p - f * r + side * r, p - f * r - side * r, col);
            }
            EnemyKind::Brute | EnemyKind::Boss => {
                draw_poly(p.x, p.y, 6, r, a.to_degrees(), col);
                draw_poly_lines(p.x, p.y, 6, r, a.to_degrees(), 2.0, BLACK);
                draw_circle(p.x, p.y, r * 0.35, Color::new(1.0, 0.9, 0.3, 1.0));
            }
        }
        if matches!(e.kind, EnemyKind::Brute | EnemyKind::Boss) || e.hp < e.max_hp {
            bar(
                p.x - r,
                p.y - r - 8.0,
                r * 2.0,
                3.0,
                e.hp / e.max_hp,
                DANGER,
            );
        }
    }

    // Bullets.
    for b in &g.bullets {
        let p = v.s(b.pos);
        let tail = v.s(b.pos - b.vel * 0.025);
        let col = if b.from_turret { ACCENT } else { GOLD };
        draw_line(p.x, p.y, tail.x, tail.y, 3.0, col);
    }

    // Player commander.
    let pl = &g.player;
    let p = v.s(pl.pos);
    let hurt = pl.hurt > 0.0;
    draw_circle(p.x, p.y, v.l(20.0), Color::new(0.1, 0.25, 0.45, 1.0));
    draw_circle(
        p.x,
        p.y,
        v.l(15.0),
        if hurt {
            DANGER
        } else {
            Color::new(0.3, 0.6, 1.0, 1.0)
        },
    );
    let gun = p + vec2(pl.aim.cos(), pl.aim.sin()) * v.l(26.0);
    draw_line(
        p.x,
        p.y,
        gun.x,
        gun.y,
        v.l(7.0),
        Color::new(0.8, 0.85, 0.9, 1.0),
    );
    draw_circle(p.x, p.y, v.l(6.0), GOLD);
    draw_circle_lines(
        p.x,
        p.y,
        v.l(GRAB_RANGE),
        1.0,
        Color::new(0.35, 0.85, 1.0, 0.15),
    );

    // Explosions.
    for f in fx {
        let life = if f.big { 0.7 } else { 0.35 };
        let k = f.t / life;
        let p = v.s(f.pos);
        let r = v.l(if f.big { 70.0 } else { 26.0 }) * (0.3 + k);
        draw_circle(p.x, p.y, r, Color::new(1.0, 0.6, 0.2, 0.5 * (1.0 - k)));
        draw_circle(
            p.x,
            p.y,
            r * 0.5,
            Color::new(1.0, 0.95, 0.6, 0.7 * (1.0 - k)),
        );
    }
    for f in floaters {
        let p = v.s(f.pos);
        let mut c = f.color;
        c.a = 1.0 - f.t / 1.4;
        let d = measure_text(&f.text, None, 26, 1.0);
        draw_text(
            &f.text,
            p.x - d.width / 2.0,
            p.y - 90.0 - f.t * 40.0,
            26.0,
            c,
        );
    }
}

fn draw_structure(s: &sim::game::Structure, v: &View, t: f32) {
    let p = v.s(s.pos);
    let r = v.l(24.0);
    let a = if s.built() { 1.0 } else { 0.35 };
    let body = Color::new(0.3, 0.38, 0.45, a);
    let trim = match s.kind {
        StructKind::Llt => Color::new(0.4, 1.0, 0.5, a),
        StructKind::Hlt => Color::new(0.2, 0.9, 1.0, a),
        StructKind::Flak => Color::new(1.0, 0.7, 0.3, a),
        StructKind::Annihilator => Color::new(0.8, 0.4, 1.0, a),
        StructKind::RepairPad => Color::new(0.4, 1.0, 0.6, a),
        StructKind::NanoTower => Color::new(0.3, 1.0, 0.3, a),
        StructKind::Radar => Color::new(0.9, 0.9, 0.9, a),
        StructKind::EvacGate => Color::new(1.0, 0.85, 0.3, a),
    };
    match s.kind {
        StructKind::RepairPad => {
            draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, body);
            draw_rectangle(p.x - r * 0.2, p.y - r * 0.7, r * 0.4, r * 1.4, trim);
            draw_rectangle(p.x - r * 0.7, p.y - r * 0.2, r * 1.4, r * 0.4, trim);
        }
        StructKind::Radar => {
            draw_circle(p.x, p.y, r * 0.8, body);
            let a2 = t * 2.0;
            let e = p + vec2(a2.cos(), a2.sin()) * r * 1.2;
            draw_line(p.x, p.y, e.x, e.y, 3.0, trim);
        }
        StructKind::NanoTower => {
            draw_poly(p.x, p.y, 4, r, 45.0, body);
            draw_circle_lines(p.x, p.y, r * (0.5 + (t * 3.0).sin().abs() * 0.4), 2.0, trim);
        }
        StructKind::EvacGate => {
            draw_rectangle(p.x - r * 1.6, p.y - r, r * 3.2, r * 2.0, body);
            draw_circle(p.x, p.y, r * 0.8, Color::new(0.5, 0.8, 1.0, a * 0.8));
        }
        _ => {
            draw_poly(p.x, p.y, 8, r, 0.0, body);
            let e = p + vec2(s.aim.cos(), s.aim.sin()) * r * 1.3;
            let w = if s.kind == StructKind::Annihilator {
                9.0
            } else {
                5.0
            };
            draw_line(p.x, p.y, e.x, e.y, v.l(w), trim);
            draw_circle(p.x, p.y, r * 0.4, trim);
        }
    }
    draw_poly_lines(p.x, p.y, 8, r + 2.0, 0.0, 1.5, trim);
    if !s.built() {
        bar(p.x - r, p.y + r + 6.0, r * 2.0, 4.0, s.progress, GOLD);
    }
}

/// Edge-of-screen arrows: always to the base, and to wrecks once a Radar
/// is online.
fn draw_pointers(g: &Game, v: &View) {
    let sw = screen_width();
    let sh = screen_height();
    let arrow = |p: V2, col: Color, size: f32| {
        let s = v.s(p);
        if s.x > 0.0 && s.x < sw && s.y > 60.0 && s.y < sh - 40.0 {
            return;
        }
        let c = vec2(sw / 2.0, sh / 2.0);
        let d = (s - c).normalize_or_zero();
        let m = 40.0;
        let kx = if d.x.abs() > 1e-4 {
            (sw / 2.0 - m) / d.x.abs()
        } else {
            f32::MAX
        };
        let ky = if d.y.abs() > 1e-4 {
            (sh / 2.0 - m - 30.0) / d.y.abs()
        } else {
            f32::MAX
        };
        let e = c + d * kx.min(ky);
        let side = vec2(-d.y, d.x);
        draw_triangle(
            e + d * size,
            e - d * size * 0.6 + side * size * 0.7,
            e - d * size * 0.6 - side * size * 0.7,
            col,
        );
    };
    if g.has(StructKind::Radar) {
        for w in g.wrecks.iter().filter(|w| !w.towed) {
            arrow(w.pos, Color::new(0.85, 0.65, 0.35, 0.8), 9.0);
        }
    }
    arrow(center(), ACCENT, 16.0);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, col: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, col);
}

fn draw_hud(g: &Game, audio: &Audio, overview: bool) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, 64.0, Color::new(0.0, 0.0, 0.0, 0.55));
    let p = &g.player;
    draw_text("HP", 12.0, 24.0, 22.0, WHITE);
    bar(44.0, 10.0, 200.0, 16.0, p.hp / p.max_hp, OK);
    draw_text(
        format!("{:.0}/{:.0}", p.hp.max(0.0), p.max_hp),
        250.0,
        24.0,
        20.0,
        WHITE,
    );
    draw_text(format!("LV {}", g.level), 12.0, 52.0, 22.0, GOLD);
    bar(
        70.0,
        40.0,
        174.0,
        10.0,
        g.xp as f32 / sim::game::xp_for(g.level) as f32,
        GOLD,
    );

    // Salvage and quota.
    let mid = sw / 2.0 - 180.0;
    match g.next_structure() {
        Some(k) => {
            let q = quota(g.structures_built());
            draw_text(
                format!("Salvage {}/{}  next: {}", g.salvage, q, k.name()),
                mid,
                24.0,
                22.0,
                GOLD,
            );
            bar(mid, 32.0, 360.0, 10.0, g.salvage as f32 / q as f32, GOLD);
        }
        None => {
            draw_text("All structures built", mid, 24.0, 22.0, GOLD);
        }
    }
    draw_text(
        format!(
            "Base {}/{}   Recycler",
            g.structures_built(),
            StructKind::ORDER.len()
        ),
        mid,
        60.0,
        18.0,
        LIGHTGRAY,
    );
    bar(
        mid + 175.0,
        50.0,
        185.0,
        8.0,
        g.recycler_hp / g.recycler_max,
        if g.recycler_hp < g.recycler_max * 0.3 {
            DANGER
        } else {
            ACCENT
        },
    );

    let right = sw - 250.0;
    draw_text(
        format!(
            "Wave {}  next in {:.0}s",
            g.wave,
            g.wave_timer.min(WAVE_TIME)
        ),
        right,
        24.0,
        22.0,
        WHITE,
    );
    draw_text(
        format!(
            "Kills {}  {}",
            g.kills,
            if audio.muted { "[muted]" } else { "" }
        ),
        right,
        50.0,
        20.0,
        LIGHTGRAY,
    );

    // Tow status at the bottom.
    let load = g.tow_load();
    let msg = if g.towing.is_empty() {
        format!(
            "Cables 0/{}  -  E near a wreck to grab   Tab overview",
            p.cables
        )
    } else {
        format!(
            "Towing {} ({} salvage)  speed {:.0}%  -  drag into the Recycler, E to release",
            g.towing.len(),
            load,
            g.tow_factor() * 100.0
        )
    };
    let col = if g.towing.is_empty() { LIGHTGRAY } else { GOLD };
    centered(&msg, sh - 14.0, 22.0, col);
    if overview {
        centered("BASE OVERVIEW (Tab to return)", 90.0, 24.0, ACCENT);
        let mut y = 130.0;
        for (i, k) in StructKind::ORDER.iter().enumerate() {
            let built = i < g.structures_built();
            let col = if built { OK } else { GRAY };
            draw_text(
                format!(
                    "{} {:<18} {:>4}  {}",
                    if built { "[x]" } else { "[ ]" },
                    k.name(),
                    quota(i),
                    k.blurb()
                ),
                16.0,
                y,
                20.0,
                col,
            );
            y += 24.0;
        }
    }
}

fn draw_level_up(g: &Game) {
    dim();
    centered(
        &format!("LEVEL {}! Choose an upgrade", g.level),
        screen_height() / 2.0 - 130.0,
        40.0,
        GOLD,
    );
    let (mx, my) = mouse_position();
    for (i, u) in g.choices.iter().enumerate() {
        let r = card_rect(i);
        let hover = r.contains(vec2(mx, my));
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            Color::new(0.12, 0.14, 0.18, if hover { 1.0 } else { 0.9 }),
        );
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            if hover { 4.0 } else { 2.0 },
            if hover { GOLD } else { ACCENT },
        );
        draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 32.0, 28.0, GOLD);
        draw_text(u.name(), r.x + 12.0, r.y + 75.0, 28.0, WHITE);
        draw_text(u.desc(), r.x + 12.0, r.y + 115.0, 19.0, LIGHTGRAY);
    }
}

fn draw_end(g: &Game) {
    dim();
    let h = screen_height();
    let (title, col) = if g.phase == Phase::Won {
        ("EVACUATED! YOU WIN", OK)
    } else if g.recycler_hp <= 0.0 {
        ("RECYCLER DESTROYED", DANGER)
    } else {
        ("COMMANDER DESTROYED", DANGER)
    };
    centered(title, h * 0.38, 64.0, col);
    let m = (g.time / 60.0) as u32;
    let s = g.time as u32 % 60;
    centered(
        &format!(
            "Wave {}   Level {}   Kills {}   Salvage hauled {}   Time {m}:{s:02}",
            g.wave, g.level, g.kills, g.total_salvage
        ),
        h * 0.38 + 50.0,
        24.0,
        WHITE,
    );
    centered(
        &format!(
            "Structures built: {}/{}",
            g.structures_built(),
            StructKind::ORDER.len()
        ),
        h * 0.38 + 82.0,
        24.0,
        LIGHTGRAY,
    );
    centered(
        "Enter / Space to play again",
        h * 0.38 + 140.0,
        30.0,
        ACCENT,
    );
}
