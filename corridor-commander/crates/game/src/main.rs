//! Corridor Commander: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    BUILD_RANGE, EnemyKind, Event, GATE_SPACING, GATES, Game, Input, PLAYER_R, Phase, TOWER_R, V2,
    VIEW_H, WIDTH, xp_needed,
};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position};

const BG: Color = Color::new(0.07, 0.08, 0.1, 1.0);
const GROUND: Color = Color::new(0.13, 0.15, 0.13, 1.0);
const ALLY: Color = Color::new(0.35, 0.75, 1.0, 1.0);
const ENEMY: Color = Color::new(1.0, 0.35, 0.25, 1.0);
const GOLD: Color = Color::new(1.0, 0.82, 0.3, 1.0);
const HUD_H: f32 = 56.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Corridor Commander".to_owned(),
        window_width: 720,
        window_height: 900,
        high_dpi: true,
        ..Default::default()
    }
}

enum Screen {
    Title,
    Play,
}

struct Particle {
    pos: V2,
    vel: V2,
    life: f32,
    max: f32,
    color: Color,
    size: f32,
}

/// Maps world space to the screen: a centred, letterboxed corridor.
struct View {
    scale: f32,
    ox: f32,
    oy: f32,
    scroll: f32,
}

impl View {
    fn new(scroll: f32) -> Self {
        let avail_h = screen_height() - HUD_H;
        let scale = (screen_width() / WIDTH).min(avail_h / VIEW_H);
        View {
            scale,
            ox: (screen_width() - WIDTH * scale) / 2.0,
            oy: HUD_H + (avail_h - VIEW_H * scale) / 2.0,
            scroll,
        }
    }
    fn to_screen(&self, p: V2) -> Vec2 {
        vec2(
            self.ox + p.x * self.scale,
            self.oy + (VIEW_H - (p.y - self.scroll)) * self.scale,
        )
    }
    fn to_world(&self, s: Vec2) -> V2 {
        V2::new(
            (s.x - self.ox) / self.scale,
            self.scroll + VIEW_H - (s.y - self.oy) / self.scale,
        )
    }
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.2, 0.2, 0.22, 1.0));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
    draw_rectangle_lines(x, y, w, h, 1.0, GRAY);
}

fn read_input(v: &View) -> Input {
    let mut mv = V2::default();
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        mv.y += 1.0;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        mv.y -= 1.0;
    }
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        mv.x -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        mv.x += 1.0;
    }
    let place =
        is_mouse_button_pressed(MouseButton::Left).then(|| v.to_world(mouse_position().into()));
    let choice = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .iter()
        .position(|k| is_key_pressed(*k));
    Input {
        mv,
        dash: is_key_pressed(KeyCode::Space),
        place,
        choice,
    }
}

fn spawn_burst(parts: &mut Vec<Particle>, pos: V2, n: usize, color: Color, speed: f32) {
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU + rand::gen_range(0.0, 0.5);
        let s = rand::gen_range(0.3, 1.0) * speed;
        let life = rand::gen_range(0.3, 0.8);
        parts.push(Particle {
            pos,
            vel: V2::new(a.cos() * s, a.sin() * s),
            life,
            max: life,
            color,
            size: rand::gen_range(2.0, 4.5),
        });
    }
}

fn draw_world(g: &Game, v: &View, parts: &[Particle], shake: f32) {
    let s = v.scale;
    let sh = vec2(
        rand::gen_range(-shake, shake),
        rand::gen_range(-shake, shake),
    );
    let tl = v.to_screen(V2::new(0.0, v.scroll + VIEW_H)) + sh;
    draw_rectangle(tl.x, tl.y, WIDTH * s, VIEW_H * s, GROUND);
    // Scrolling ground plates.
    let step = 80.0;
    let first = (v.scroll / step).floor() * step;
    let mut y = first;
    while y < v.scroll + VIEW_H + step {
        let p = v.to_screen(V2::new(0.0, y)) + sh;
        let c = if ((y / step) as i64) % 2 == 0 {
            Color::new(0.15, 0.17, 0.15, 1.0)
        } else {
            Color::new(0.12, 0.14, 0.12, 1.0)
        };
        draw_rectangle(p.x, p.y - step * s, WIDTH * s, step * s, c);
        y += step;
    }
    // Corridor walls.
    draw_rectangle(tl.x - 6.0, tl.y, 6.0, VIEW_H * s, DARKGRAY);
    draw_rectangle(tl.x + WIDTH * s, tl.y, 6.0, VIEW_H * s, DARKGRAY);

    // Gate line.
    let gy = g.next_gate_y();
    if g.gates_cleared < GATES && gy < v.scroll + VIEW_H + 20.0 {
        let p = v.to_screen(V2::new(0.0, gy)) + sh;
        let pulse = 0.5 + 0.5 * (g.time * 6.0).sin();
        draw_rectangle(
            p.x,
            p.y - 4.0,
            WIDTH * s,
            8.0,
            Color::new(1.0, 0.3, 0.2, 0.5 + 0.4 * pulse),
        );
        let label = format!("GATE {}", g.gates_cleared + 1);
        draw_text(&label, p.x + 8.0, p.y - 10.0, 22.0, ENEMY);
    }

    // Build radius hint.
    let pp = v.to_screen(g.player) + sh;
    draw_circle_lines(
        pp.x,
        pp.y,
        BUILD_RANGE * s,
        1.0,
        Color::new(0.4, 0.8, 1.0, 0.15),
    );

    for gem in &g.gems {
        let p = v.to_screen(gem.pos) + sh;
        let c = if gem.xp >= 4 { GOLD } else { GREEN };
        draw_poly(p.x, p.y, 4, 4.0 * s, g.time * 90.0, c);
    }

    for t in &g.towers {
        let p = v.to_screen(t.pos) + sh;
        let grow = (t.age * 4.0).min(1.0);
        let r = TOWER_R * s * grow;
        draw_poly(p.x, p.y, 6, r, 0.0, Color::new(0.2, 0.35, 0.5, 1.0));
        draw_poly_lines(p.x, p.y, 6, r, 0.0, 2.0, ALLY);
        draw_circle(p.x, p.y, r * 0.35, ALLY);
        let frac = t.hp / g.stats.tower_hp;
        if frac < 1.0 {
            bar(p.x - r, p.y + r + 2.0, r * 2.0, 3.0, frac, GREEN);
        }
    }

    for e in &g.enemies {
        let (_, _, r, ..) = e.kind.stats();
        let p = v.to_screen(e.pos) + sh;
        let r = r * s;
        let c = if e.flash > 0.0 { WHITE } else { ENEMY };
        match e.kind {
            EnemyKind::Raider => {
                draw_triangle(
                    vec2(p.x, p.y + r),
                    vec2(p.x - r, p.y - r),
                    vec2(p.x + r, p.y - r),
                    c,
                );
            }
            EnemyKind::Tank => {
                draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, c);
                draw_rectangle_lines(p.x - r, p.y - r, r * 2.0, r * 2.0, 2.0, MAROON);
            }
            EnemyKind::Gunner => {
                draw_poly(p.x, p.y, 5, r, g.time * 60.0, c);
                draw_circle(p.x, p.y, r * 0.4, ORANGE);
            }
            EnemyKind::Boss => {
                draw_poly(p.x, p.y, 8, r, g.time * 20.0, c);
                draw_poly_lines(p.x, p.y, 8, r * 1.15, -g.time * 30.0, 3.0, ORANGE);
                draw_circle(p.x, p.y, r * 0.45, MAROON);
                bar(
                    p.x - r,
                    p.y - r - 10.0,
                    r * 2.0,
                    5.0,
                    e.hp / e.max_hp,
                    ENEMY,
                );
            }
        }
    }

    for b in &g.bullets {
        let p = v.to_screen(b.pos) + sh;
        let (c, r) = match b.owner {
            sim::game::Owner::Player => (YELLOW, 3.0),
            sim::game::Owner::Tower => (ALLY, 2.5),
            sim::game::Owner::Enemy => (ORANGE, 4.0),
        };
        draw_circle(p.x, p.y, r * s, c);
    }

    // Commander.
    let blink = g.invuln > 0.0 && (g.time * 20.0) as i32 % 2 == 0;
    if !blink {
        let r = PLAYER_R * s;
        let body = if g.dash_t > 0.0 { WHITE } else { ALLY };
        draw_circle(pp.x, pp.y, r, Color::new(0.15, 0.3, 0.5, 1.0));
        draw_circle_lines(pp.x, pp.y, r, 2.5, body);
        draw_rectangle(pp.x - r * 0.25, pp.y - r * 1.4, r * 0.5, r, body);
    }

    for pt in parts {
        let p = v.to_screen(pt.pos) + sh;
        let mut c = pt.color;
        c.a = pt.life / pt.max;
        draw_circle(p.x, p.y, pt.size * s, c);
    }

    // Build cursor preview.
    let m: Vec2 = mouse_position().into();
    let w = v.to_world(m);
    if g.phase == Phase::Playing && w.y > v.scroll && w.y < v.scroll + VIEW_H {
        let ok = g.can_build(w);
        let c = if ok {
            Color::new(0.4, 1.0, 0.5, 0.6)
        } else {
            Color::new(1.0, 0.3, 0.3, 0.5)
        };
        draw_poly_lines(m.x, m.y, 6, TOWER_R * s, 0.0, 2.0, c);
    }
}

fn draw_hud(g: &Game, muted: bool) {
    let w = screen_width();
    draw_rectangle(0.0, 0.0, w, HUD_H, Color::new(0.04, 0.05, 0.07, 1.0));
    draw_text("HP", 10.0, 20.0, 20.0, WHITE);
    bar(40.0, 8.0, 160.0, 14.0, g.hp / g.stats.max_hp, RED);
    draw_text(
        format!("LV {}", g.level),
        10.0,
        44.0,
        20.0,
        Color::new(0.6, 1.0, 0.6, 1.0),
    );
    bar(
        60.0,
        32.0,
        140.0,
        10.0,
        g.xp as f32 / xp_needed(g.level) as f32,
        GREEN,
    );
    draw_text(
        format!("SCRAP {}  (turret {})", g.scrap, g.stats.tower_cost),
        215.0,
        20.0,
        20.0,
        GOLD,
    );
    let dash = if g.dash_cd <= 0.0 {
        "DASH ready".to_owned()
    } else {
        format!("DASH {:.1}s", g.dash_cd)
    };
    draw_text(&dash, 215.0, 44.0, 20.0, SKYBLUE);
    draw_text(format!("KILLS {}", g.kills), w - 240.0, 20.0, 20.0, WHITE);
    draw_text(
        format!("GATES {}/{}", g.gates_cleared, GATES),
        w - 130.0,
        20.0,
        20.0,
        ENEMY,
    );
    bar(w - 240.0, 32.0, 200.0, 10.0, g.progress(), ENEMY);
    if muted {
        draw_text("MUTED", w - 34.0 - 30.0, 52.0, 16.0, GRAY);
    }
    if g.at_gate {
        let a = 0.6 + 0.4 * (g.time * 5.0).sin();
        centered(
            "GATE GUARDIAN - DESTROY IT TO ADVANCE",
            HUD_H + 30.0,
            24.0,
            Color::new(1.0, 0.4, 0.3, a),
        );
    }
}

fn draw_level_up(opts: &[sim::game::Upgrade; 3]) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.65),
    );
    let cy = screen_height() * 0.3;
    centered("LEVEL UP", cy, 48.0, GOLD);
    centered(
        "Choose an upgrade: press 1, 2 or 3 (or click)",
        cy + 36.0,
        20.0,
        WHITE,
    );
    let bw = 420.0f32.min(screen_width() - 32.0);
    let x = (screen_width() - bw) / 2.0;
    for (i, u) in opts.iter().enumerate() {
        let y = cy + 60.0 + i as f32 * 84.0;
        let hover = {
            let (mx, my) = mouse_position();
            mx > x && mx < x + bw && my > y && my < y + 72.0
        };
        let bg = if hover {
            Color::new(0.2, 0.3, 0.4, 1.0)
        } else {
            Color::new(0.12, 0.15, 0.2, 1.0)
        };
        draw_rectangle(x, y, bw, 72.0, bg);
        draw_rectangle_lines(x, y, bw, 72.0, 2.0, ALLY);
        draw_text(
            format!("{}  {}", i + 1, u.name()),
            x + 14.0,
            y + 30.0,
            26.0,
            WHITE,
        );
        draw_text(u.desc(), x + 14.0, y + 56.0, 20.0, LIGHTGRAY);
    }
}

fn level_up_click() -> Option<usize> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let cy = screen_height() * 0.3;
    let bw = 420.0f32.min(screen_width() - 32.0);
    let x = (screen_width() - bw) / 2.0;
    let (mx, my) = mouse_position();
    (0..3).find(|i| {
        let y = cy + 60.0 + *i as f32 * 84.0;
        mx > x && mx < x + bw && my > y && my < y + 72.0
    })
}

fn draw_title(t: f32) {
    clear_background(BG);
    let cy = screen_height() * 0.28;
    centered("CORRIDOR COMMANDER", cy, 52.0, ALLY);
    centered(
        "The front never stops. Build forward, leave defenses behind.",
        cy + 40.0,
        20.0,
        LIGHTGRAY,
    );
    let lines = [
        "WASD / arrows - move (the corridor scrolls on its own)",
        "Your guns fire automatically at the nearest enemy",
        "Left click - build a turret (costs scrap)",
        "Space - dash (brief invulnerability)",
        "M - mute / unmute",
        "",
        "Turrets you leave behind guard your rear flank.",
        "Destroy the guardian at each of 3 gates to win.",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(l, cy + 100.0 + i as f32 * 28.0, 22.0, WHITE);
    }
    let a = 0.5 + 0.5 * (t * 3.0).sin();
    centered(
        "Press any key or click to deploy",
        cy + 360.0,
        28.0,
        Color::new(1.0, 0.82, 0.3, a),
    );
    centered(
        "Theme homage to Beyond All Reason - see README for credits",
        screen_height() - 20.0,
        16.0,
        GRAY,
    );
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Dash", KeyCode::Space),
        touch::small("Mute", KeyCode::M),
        touch::small("Start", KeyCode::Enter),
    ]);
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut game = Game::new(macroquad::miniquad::date::now() as u64);
    let mut parts: Vec<Particle> = Vec::new();
    let mut shake = 0.0f32;
    let mut end_timer = 0.0f32;

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match screen {
            Screen::Title => {
                draw_title(get_time() as f32);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    game = Game::new(macroquad::miniquad::date::now() as u64);
                    parts.clear();
                    end_timer = 0.0;
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
                clear_background(BG);
                let view = View::new(game.scroll);
                let mut input = read_input(&view);
                if let Phase::LevelUp(_) = game.phase {
                    input.place = None;
                    if input.choice.is_none() {
                        input.choice = level_up_click();
                    }
                }
                game.step(&input, dt);
                for e in &game.events {
                    audio.on_event(e);
                    match e {
                        Event::Kill { pos, big } => {
                            let n = if *big { 60 } else { 10 };
                            spawn_burst(
                                &mut parts,
                                *pos,
                                n,
                                ORANGE,
                                if *big { 260.0 } else { 120.0 },
                            );
                            if *big {
                                shake = 10.0;
                            }
                        }
                        Event::PlayerHurt => shake = shake.max(4.0),
                        Event::Built => spawn_burst(
                            &mut parts,
                            game.towers.last().map_or(game.player, |t| t.pos),
                            12,
                            ALLY,
                            80.0,
                        ),
                        Event::Lost => {
                            spawn_burst(&mut parts, game.player, 80, SKYBLUE, 220.0);
                            shake = 14.0;
                        }
                        _ => {}
                    }
                }
                for p in &mut parts {
                    p.pos = p.pos + p.vel * dt;
                    p.vel = p.vel * 0.94;
                    p.life -= dt;
                }
                parts.retain(|p| p.life > 0.0);
                shake = (shake - dt * 30.0).max(0.0);

                let view = View::new(game.scroll);
                draw_world(&game, &view, &parts, shake);
                draw_hud(&game, audio.muted);
                match &game.phase {
                    Phase::LevelUp(opts) => draw_level_up(opts),
                    Phase::Won | Phase::Lost => {
                        end_timer += dt;
                        let won = game.phase == Phase::Won;
                        draw_rectangle(
                            0.0,
                            0.0,
                            screen_width(),
                            screen_height(),
                            Color::new(0.0, 0.0, 0.0, (end_timer * 0.8).min(0.7)),
                        );
                        let cy = screen_height() * 0.38;
                        if won {
                            centered("CORRIDOR SECURED", cy, 52.0, GOLD);
                        } else {
                            centered("COMMANDER LOST", cy, 52.0, ENEMY);
                        }
                        let dist = (game.scroll / (GATE_SPACING * GATES as f32) * 100.0).min(100.0);
                        centered(
                            &format!(
                                "Kills {}   Level {}   Gates {}/{}   Advance {:.0}%   Time {:.0}s",
                                game.kills, game.level, game.gates_cleared, GATES, dist, game.time
                            ),
                            cy + 44.0,
                            22.0,
                            WHITE,
                        );
                        if end_timer > 1.2 {
                            centered(
                                "Press Enter or click to return to title",
                                cy + 90.0,
                                24.0,
                                LIGHTGRAY,
                            );
                            if is_key_pressed(KeyCode::Enter)
                                || is_mouse_button_pressed(MouseButton::Left)
                            {
                                screen = Screen::Title;
                            }
                        }
                    }
                    Phase::Playing => {}
                }
            }
        }
        touch::next_frame().await;
    }
}
