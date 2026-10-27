//! Wall Weaver: macroquad front end. Rules live in the `sim` crate.

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    CORE, CORE_RADIUS, Cell, EnemyKind, Event, Game, H, Input, MAX_TURRET_LEVEL, Phase, TILE, W,
    WAVES, center, gates, tile_of,
};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position};

const HUD: f32 = 44.0;
const WORLD_W: f32 = W as f32 * TILE;
const WORLD_H: f32 = H as f32 * TILE;

const BG: Color = Color::new(0.05, 0.06, 0.08, 1.0);
const GRID: Color = Color::new(0.12, 0.14, 0.18, 1.0);
const ACCENT: Color = Color::new(0.35, 0.85, 1.0, 1.0);
const GOLD: Color = Color::new(1.0, 0.8, 0.3, 1.0);
const DANGER: Color = Color::new(1.0, 0.35, 0.3, 1.0);

fn window_conf() -> Conf {
    Conf {
        window_title: "Wall Weaver".to_owned(),
        window_width: 1100,
        window_height: 760,
        high_dpi: true,
        ..Default::default()
    }
}

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
}

struct Fx {
    x: f32,
    y: f32,
    t: f32,
    big: bool,
}

struct View {
    scale: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn compute() -> Self {
        let scale = (screen_width() / WORLD_W).min((screen_height() - HUD) / WORLD_H);
        let ox = (screen_width() - WORLD_W * scale) * 0.5;
        let oy = HUD + (screen_height() - HUD - WORLD_H * scale) * 0.5;
        View { scale, ox, oy }
    }
    fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.scale, self.oy + y * self.scale)
    }
    fn to_world(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.ox) / self.scale, (y - self.oy) / self.scale)
    }
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) * 0.5, y, size, color);
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::click("Demolish", MouseButton::Right),
        touch::small("Back", KeyCode::Escape),
        touch::small("Mute", KeyCode::M),
        touch::small("Start", KeyCode::Enter),
    ]);
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut game = Game::new(macroquad::miniquad::date::now() as u64);
    let mut fx: Vec<Fx> = Vec::new();
    let mut banner: (String, f32) = (String::new(), 0.0);
    let mut shake = 0.0f32;

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        clear_background(BG);
        let view = View::compute();

        match screen {
            Screen::Title => {
                draw_title(&audio);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    game = Game::new(macroquad::miniquad::date::now() as u64);
                    fx.clear();
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
                let input = read_input(&game, &view);
                let events = game.update(dt, &input);
                for e in &events {
                    audio.on_event(e);
                    match *e {
                        Event::Kill { x, y, big } => fx.push(Fx { x, y, t: 0.0, big }),
                        Event::WaveStart(n) => {
                            let tag = if n.is_multiple_of(5) {
                                "  -  BOSS INBOUND"
                            } else {
                                ""
                            };
                            banner = (format!("WAVE {n}/{WAVES}{tag}"), 2.5);
                        }
                        Event::CoreHit => shake = 6.0,
                        Event::PlayerHurt => shake = shake.max(3.0),
                        _ => {}
                    }
                }
                fx.retain_mut(|f| {
                    f.t += dt;
                    f.t < if f.big { 0.6 } else { 0.35 }
                });
                banner.1 -= dt;
                shake = (shake - dt * 30.0).max(0.0);
                let jitter = View {
                    ox: view.ox + rand::gen_range(-shake, shake),
                    oy: view.oy + rand::gen_range(-shake, shake),
                    scale: view.scale,
                };
                draw_world(&game, &jitter, &fx);
                draw_cursor(&game, &view);
                draw_hud(&game, &audio);
                if banner.1 > 0.0 {
                    let a = banner.1.min(1.0);
                    centered(&banner.0, HUD + 60.0, 40.0, Color::new(1.0, 0.8, 0.3, a));
                }
                match game.phase {
                    Phase::LevelUp => draw_level_up(&game),
                    Phase::Won | Phase::Lost => {
                        draw_end(&game);
                        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                            game = Game::new(macroquad::miniquad::date::now() as u64);
                            fx.clear();
                        } else if is_key_pressed(KeyCode::Escape) {
                            screen = Screen::Title;
                        }
                    }
                    Phase::Playing => {}
                }
            }
        }
        touch::next_frame().await;
    }
}

fn mouse_tile(view: &View) -> Option<(usize, usize)> {
    let (mx, my) = mouse_position();
    let (wx, wy) = view.to_world(mx, my);
    tile_of(wx, wy)
}

fn read_input(game: &Game, view: &View) -> Input {
    let mut input = Input::default();
    let k = |a: KeyCode, b: KeyCode| is_key_down(a) || is_key_down(b);
    if k(KeyCode::A, KeyCode::Left) {
        input.mx -= 1.0;
    }
    if k(KeyCode::D, KeyCode::Right) {
        input.mx += 1.0;
    }
    if k(KeyCode::W, KeyCode::Up) {
        input.my -= 1.0;
    }
    if k(KeyCode::S, KeyCode::Down) {
        input.my += 1.0;
    }
    input.lay_wall = k(KeyCode::LeftShift, KeyCode::RightShift);
    if game.phase == Phase::LevelUp {
        for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
            .into_iter()
            .enumerate()
        {
            if is_key_pressed(key) {
                input.pick = Some(i);
            }
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            for i in 0..3 {
                let r = card_rect(i);
                if r.contains(vec2(mx, my)) {
                    input.pick = Some(i);
                }
            }
        }
        return input;
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        input.build = mouse_tile(view);
    }
    if is_key_pressed(KeyCode::X) || is_mouse_button_pressed(MouseButton::Right) {
        input.demolish = mouse_tile(view);
    }
    input
}

fn draw_title(audio: &Audio) {
    let h = screen_height();
    let t = get_time() as f32;
    // Animated dragon's teeth backdrop.
    for i in 0..30 {
        let x = (i as f32 * 47.0 + t * 20.0) % (screen_width() + 40.0) - 20.0;
        let y = h * 0.62 + (i as f32 * 1.7).sin() * 30.0;
        draw_tooth(x, y, 16.0, Color::new(0.4, 0.42, 0.48, 0.5));
    }
    centered("WALL WEAVER", h * 0.28, 84.0, ACCENT);
    centered(
        "Weave dragon's teeth into a maze. Funnel the swarm into your guns.",
        h * 0.28 + 44.0,
        24.0,
        WHITE,
    );
    let lines = [
        "WASD move   |   hold SHIFT to lay a wall trail (1 scrap per tile)",
        "LEFT CLICK build turret / upgrade turret   |   X or RIGHT CLICK demolish",
        "Protect the core for 20 waves. Walls can never fully seal the path.",
        "M mute",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(l, h * 0.42 + i as f32 * 28.0, 22.0, LIGHTGRAY);
    }
    if (t * 2.0).sin() > -0.3 {
        centered("PRESS ANY KEY TO START", h * 0.8, 34.0, GOLD);
    }
    if audio.muted {
        centered("(muted)", h * 0.8 + 32.0, 20.0, GRAY);
    }
}

fn draw_tooth(x: f32, y: f32, s: f32, c: Color) {
    draw_triangle(
        vec2(x - s, y + s * 0.7),
        vec2(x + s, y + s * 0.7),
        vec2(x, y - s),
        c,
    );
    draw_triangle(
        vec2(x, y - s),
        vec2(x + s, y + s * 0.7),
        vec2(x + s * 0.3, y + s * 0.2),
        Color::new(c.r * 0.6, c.g * 0.6, c.b * 0.6, c.a),
    );
}

fn draw_world(g: &Game, v: &View, fx: &[Fx]) {
    let s = v.scale;
    let ts = TILE * s;
    let (x0, y0) = v.to_screen(0.0, 0.0);
    draw_rectangle(
        x0,
        y0,
        WORLD_W * s,
        WORLD_H * s,
        Color::new(0.08, 0.09, 0.11, 1.0),
    );
    for x in 0..=W {
        let sx = x0 + x as f32 * ts;
        draw_line(sx, y0, sx, y0 + WORLD_H * s, 1.0, GRID);
    }
    for y in 0..=H {
        let sy = y0 + y as f32 * ts;
        draw_line(x0, sy, x0 + WORLD_W * s, sy, 1.0, GRID);
    }
    // Gates.
    for (gx, gy) in gates() {
        let (sx, sy) = v.to_screen(gx as f32 * TILE, gy as f32 * TILE);
        draw_rectangle(sx, sy, ts, ts, Color::new(0.5, 0.1, 0.1, 0.45));
    }
    let t = get_time() as f32;
    for ty in 0..H {
        for tx in 0..W {
            let (cx, cy) = center(tx, ty);
            let (sx, sy) = v.to_screen(cx, cy);
            match g.cell(tx, ty) {
                Cell::Wall => draw_tooth(sx, sy, ts * 0.48, Color::new(0.62, 0.64, 0.7, 1.0)),
                Cell::Pending => {
                    let a = 0.35 + 0.25 * (t * 12.0).sin();
                    draw_tooth(sx, sy, ts * 0.4, Color::new(0.35, 0.85, 1.0, a));
                }
                _ => {}
            }
        }
    }
    // Core.
    let (cx, cy) = center(CORE.0, CORE.1);
    let (sx, sy) = v.to_screen(cx, cy);
    let pulse = 1.0 + 0.06 * (t * 3.0).sin();
    draw_poly(
        sx,
        sy,
        6,
        CORE_RADIUS * s * pulse,
        t * 20.0,
        Color::new(0.2, 0.6, 1.0, 1.0),
    );
    draw_poly(
        sx,
        sy,
        6,
        CORE_RADIUS * s * 0.55,
        -t * 40.0,
        Color::new(0.7, 0.95, 1.0, 1.0),
    );
    // Turrets.
    for tu in &g.turrets {
        let (cx, cy) = center(tu.tx, tu.ty);
        let (sx, sy) = v.to_screen(cx, cy);
        let col = match tu.level {
            1 => Color::new(0.4, 0.8, 0.4, 1.0),
            2 => Color::new(0.4, 0.7, 1.0, 1.0),
            _ => Color::new(1.0, 0.6, 0.9, 1.0),
        };
        draw_rectangle(
            sx - ts * 0.45,
            sy - ts * 0.45,
            ts * 0.9,
            ts * 0.9,
            Color::new(0.2, 0.22, 0.26, 1.0),
        );
        draw_circle(sx, sy, ts * 0.32, col);
        let (dx, dy) = (tu.aim.cos(), tu.aim.sin());
        draw_line(
            sx,
            sy,
            sx + dx * ts * 0.5,
            sy + dy * ts * 0.5,
            4.0 * s,
            DARKGRAY,
        );
        for k in 0..tu.level {
            draw_circle(
                sx - ts * 0.3 + k as f32 * ts * 0.3,
                sy + ts * 0.36,
                2.0 * s,
                GOLD,
            );
        }
    }
    // Orbs.
    for o in &g.orbs {
        let (sx, sy) = v.to_screen(o.x, o.y);
        let big = o.value / 1000 >= 5;
        draw_circle(
            sx,
            sy,
            if big { 5.0 } else { 3.0 } * s,
            if big { GOLD } else { GREEN },
        );
    }
    // Enemies.
    for e in &g.enemies {
        let (sx, sy) = v.to_screen(e.x, e.y);
        let r = e.kind.stats().2 * s;
        let mut col = match e.kind {
            EnemyKind::Raider => Color::new(1.0, 0.55, 0.2, 1.0),
            EnemyKind::Grunt => Color::new(0.9, 0.25, 0.25, 1.0),
            EnemyKind::Tank => Color::new(0.6, 0.2, 0.6, 1.0),
            EnemyKind::Boss => Color::new(0.95, 0.1, 0.35, 1.0),
        };
        if e.hurt_flash > 0.0 {
            col = WHITE;
        }
        match e.kind {
            EnemyKind::Raider => draw_poly(sx, sy, 3, r * 1.2, t * 200.0, col),
            EnemyKind::Grunt => draw_circle(sx, sy, r, col),
            EnemyKind::Tank => draw_poly(sx, sy, 4, r * 1.2, 45.0, col),
            EnemyKind::Boss => {
                draw_poly(sx, sy, 8, r, t * 30.0, col);
                draw_circle(sx, sy, r * 0.45, BLACK);
            }
        }
        if e.hp < e.max_hp {
            let w = r * 2.0;
            draw_rectangle(sx - r, sy - r - 6.0, w, 3.0, Color::new(0.2, 0.0, 0.0, 0.8));
            draw_rectangle(
                sx - r,
                sy - r - 6.0,
                w * (e.hp / e.max_hp).max(0.0),
                3.0,
                DANGER,
            );
        }
    }
    // Bullets.
    for b in &g.bullets {
        let (sx, sy) = v.to_screen(b.x, b.y);
        let c = if b.from_turret { GOLD } else { ACCENT };
        draw_line(
            sx,
            sy,
            sx - b.vx * 0.02 * s,
            sy - b.vy * 0.02 * s,
            2.0 * s,
            c,
        );
    }
    // Commander.
    let (sx, sy) = v.to_screen(g.px, g.py);
    draw_circle(sx, sy, 11.0 * s, Color::new(0.1, 0.2, 0.35, 1.0));
    draw_circle(sx, sy, 8.0 * s, ACCENT);
    draw_circle_lines(
        sx,
        sy,
        60.0 * g.stats.magnet * s,
        1.0,
        Color::new(0.35, 0.85, 1.0, 0.12),
    );
    if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
        draw_circle_lines(
            sx,
            sy,
            13.0 * s,
            2.0,
            Color::new(0.35, 0.85, 1.0, 0.6 + 0.4 * (t * 10.0).sin()),
        );
    }
    // Explosions.
    for f in fx {
        let (sx, sy) = v.to_screen(f.x, f.y);
        let life = if f.big { 0.6 } else { 0.35 };
        let k = f.t / life;
        let r = (if f.big { 34.0 } else { 14.0 }) * s * (0.4 + k);
        draw_circle(
            sx,
            sy,
            r,
            Color::new(1.0, 0.7 - k * 0.5, 0.2, 0.8 * (1.0 - k)),
        );
    }
}

fn draw_cursor(g: &Game, v: &View) {
    if g.phase != Phase::Playing {
        return;
    }
    let Some((tx, ty)) = mouse_tile(v) else {
        return;
    };
    let (sx, sy) = v.to_screen(tx as f32 * TILE, ty as f32 * TILE);
    let ts = TILE * v.scale;
    let (label, ok) = match g.cell(tx, ty) {
        Cell::Turret => {
            let t = g.turrets.iter().find(|t| (t.tx, t.ty) == (tx, ty));
            let lvl = t.map_or(1, |t| t.level);
            let (cx, cy) = center(tx, ty);
            let (px, py) = v.to_screen(cx, cy);
            draw_circle_lines(
                px,
                py,
                g.turret_range(lvl) * v.scale,
                1.5,
                Color::new(1.0, 0.8, 0.3, 0.5),
            );
            if lvl < MAX_TURRET_LEVEL {
                let c = Game::upgrade_cost(lvl);
                (format!("upgrade {c}"), g.scrap >= c)
            } else {
                ("max level".to_owned(), false)
            }
        }
        Cell::Core => (String::new(), false),
        _ => {
            let c = g.turret_cost();
            let (cx, cy) = center(tx, ty);
            let (px, py) = v.to_screen(cx, cy);
            draw_circle_lines(
                px,
                py,
                g.turret_range(1) * v.scale,
                1.0,
                Color::new(1.0, 1.0, 1.0, 0.25),
            );
            (format!("turret {c}"), g.scrap >= c)
        }
    };
    let col = if ok {
        Color::new(0.4, 1.0, 0.5, 0.8)
    } else {
        Color::new(1.0, 0.3, 0.3, 0.8)
    };
    draw_rectangle_lines(sx, sy, ts, ts, 2.0, col);
    if !label.is_empty() {
        draw_text(&label, sx + ts + 4.0, sy + ts * 0.7, 18.0, col);
    }
}

fn bar(x: f32, y: f32, w: f32, frac: f32, col: Color, label: &str) {
    draw_rectangle(x, y, w, 14.0, Color::new(0.15, 0.15, 0.18, 1.0));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), 14.0, col);
    draw_text(label, x + 4.0, y + 12.0, 16.0, WHITE);
}

fn draw_hud(g: &Game, audio: &Audio) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        HUD,
        Color::new(0.03, 0.035, 0.05, 1.0),
    );
    bar(
        10.0,
        6.0,
        160.0,
        g.hp / g.max_hp,
        Color::new(0.2, 0.7, 0.9, 1.0),
        &format!("HP {:.0}", g.hp),
    );
    bar(
        10.0,
        24.0,
        160.0,
        g.core_hp / g.core_max,
        Color::new(0.3, 0.5, 1.0, 1.0),
        &format!("CORE {:.0}", g.core_hp),
    );
    bar(
        185.0,
        24.0,
        160.0,
        g.xp as f32 / sim::game::xp_for_level(g.level) as f32,
        Color::new(0.3, 0.8, 0.3, 1.0),
        &format!("LV {}", g.level),
    );
    draw_text(format!("SCRAP {}", g.scrap), 185.0, 18.0, 22.0, GOLD);
    let wave = match g.next_wave_in() {
        Some(t) if g.wave < WAVES => format!("WAVE {}/{}  next in {:.0}s", g.wave, WAVES, t.ceil()),
        _ => format!(
            "WAVE {}/{}  enemies {}",
            g.wave,
            WAVES,
            g.enemies.len() + g.to_spawn.len()
        ),
    };
    draw_text(&wave, 365.0, 28.0, 24.0, WHITE);
    let tail = format!(
        "kills {}  {}",
        g.kills,
        if audio.muted {
            "[M] muted"
        } else {
            "[M] sound"
        }
    );
    let d = measure_text(&tail, None, 18, 1.0);
    draw_text(
        &tail,
        screen_width() - d.width - 12.0,
        28.0,
        18.0,
        LIGHTGRAY,
    );
}

fn card_rect(i: usize) -> Rect {
    let w = 230.0;
    let gap = 24.0;
    let total = w * 3.0 + gap * 2.0;
    let x = (screen_width() - total) * 0.5 + i as f32 * (w + gap);
    Rect::new(x, screen_height() * 0.38, w, 150.0)
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
        "LEVEL UP - choose an upgrade",
        screen_height() * 0.3,
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
            if hover {
                Color::new(0.15, 0.25, 0.35, 1.0)
            } else {
                Color::new(0.1, 0.12, 0.16, 1.0)
            },
        );
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, ACCENT);
        draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 30.0, 24.0, GOLD);
        draw_text(u.name(), r.x + 12.0, r.y + 66.0, 24.0, WHITE);
        draw_text(u.describe(), r.x + 12.0, r.y + 100.0, 17.0, LIGHTGRAY);
    }
}

fn draw_end(g: &Game) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.7),
    );
    let h = screen_height();
    if g.phase == Phase::Won {
        centered("THE MAZE HELD", h * 0.36, 64.0, GOLD);
    } else {
        let why = if g.core_hp <= 0.0 {
            "CORE DESTROYED"
        } else {
            "COMMANDER DOWN"
        };
        centered(why, h * 0.36, 64.0, DANGER);
    }
    centered(
        &format!(
            "wave {}/{}   level {}   kills {}   turrets {}",
            g.wave,
            WAVES,
            g.level,
            g.kills,
            g.turrets.len()
        ),
        h * 0.46,
        26.0,
        WHITE,
    );
    centered(
        "ENTER to play again   |   ESC for title",
        h * 0.56,
        26.0,
        LIGHTGRAY,
    );
}
