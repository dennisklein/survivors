//! Nano Fortress: macroquad front end (input, rendering, audio, screens).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use rules::game::{
    Building, EKind, Enemy, Event, Game, Input, KINDS, Kind, MAX_BLUEPRINTS, Outcome, WAVES,
    WORLD_H, WORLD_W,
};
use rules::geom::V2;
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position};

const BLUE: Color = Color::new(0.35, 0.65, 1.0, 1.0);
const NANO: Color = Color::new(0.35, 1.0, 0.55, 1.0);
const METAL: Color = Color::new(0.95, 0.85, 0.35, 1.0);
const PANEL: Color = Color::new(0.05, 0.07, 0.1, 0.85);

#[derive(PartialEq)]
enum Screen {
    Title,
    Playing,
    Paused,
    Over,
}

struct Fx {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
}

struct Beam {
    from: Vec2,
    to: Vec2,
    life: f32,
    heavy: bool,
}

fn p(v: V2) -> Vec2 {
    vec2(v.x, v.y)
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Nano Fortress".to_owned(),
        window_width: 1280,
        window_height: 760,
        high_dpi: false,
        ..Default::default()
    }
}

fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Wall => Color::new(0.6, 0.62, 0.66, 1.0),
        Kind::Laser => Color::new(1.0, 0.45, 0.35, 1.0),
        Kind::Mex => METAL,
        Kind::Nano => NANO,
        Kind::Heavy => Color::new(1.0, 0.3, 0.8, 1.0),
    }
}

fn draw_building_shape(k: Kind, c: Vec2, r: f32, col: Color, fill: bool, t: f32) {
    let shape = |sides: u8, rot: f32| {
        if fill {
            draw_poly(c.x, c.y, sides, r, rot, Color::new(0.12, 0.16, 0.22, 1.0));
        }
        draw_poly_lines(c.x, c.y, sides, r, rot, 3.0, col);
    };
    match k {
        Kind::Wall => {
            shape(4, 45.0);
            draw_line(c.x - r * 0.6, c.y, c.x + r * 0.6, c.y, 2.0, col);
            draw_line(c.x, c.y - r * 0.6, c.x, c.y + r * 0.6, 2.0, col);
        }
        Kind::Laser => {
            shape(6, 0.0);
            draw_circle(c.x, c.y, r * 0.35, col);
        }
        Kind::Mex => {
            shape(8, 22.5);
            let a = t * 2.0;
            for i in 0..3 {
                let ang = a + i as f32 * std::f32::consts::TAU / 3.0;
                let d = vec2(ang.cos(), ang.sin()) * r * 0.7;
                draw_line(c.x, c.y, c.x + d.x, c.y + d.y, 3.0, col);
            }
        }
        Kind::Nano => {
            shape(5, -90.0);
            let pulse = 0.25 + 0.1 * (t * 6.0).sin();
            draw_circle(c.x, c.y, r * pulse + 2.0, col);
        }
        Kind::Heavy => {
            shape(6, 30.0);
            draw_poly_lines(c.x, c.y, 3, r * 0.55, -90.0, 3.0, col);
            draw_circle(c.x, c.y, r * 0.2, col);
        }
    }
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, col: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, col);
}

fn draw_building(b: &Building, t: f32) {
    let s = b.kind.spec();
    let c = p(b.pos);
    if b.done() {
        draw_building_shape(b.kind, c, s.radius, kind_color(b.kind), true, t);
        if b.hp < b.max_hp {
            bar(
                c.x - s.radius,
                c.y + s.radius + 4.0,
                s.radius * 2.0,
                4.0,
                b.hp / b.max_hp,
                GREEN,
            );
        }
    } else {
        let mut col = NANO;
        col.a = 0.35 + 0.5 * b.progress;
        draw_circle(c.x, c.y, s.radius, Color::new(0.2, 1.0, 0.5, 0.08));
        draw_building_shape(b.kind, c, s.radius, col, false, 0.0);
        bar(
            c.x - s.radius,
            c.y + s.radius + 4.0,
            s.radius * 2.0,
            4.0,
            b.progress,
            NANO,
        );
    }
}

fn enemy_color(k: EKind) -> Color {
    match k {
        EKind::Grunt => Color::new(0.95, 0.3, 0.25, 1.0),
        EKind::Raider => Color::new(1.0, 0.6, 0.2, 1.0),
        EKind::Brute => Color::new(0.8, 0.15, 0.15, 1.0),
        EKind::Sumo => Color::new(0.6, 0.1, 0.3, 1.0),
        EKind::Boss => Color::new(0.9, 0.05, 0.1, 1.0),
    }
}

fn draw_enemy(e: &Enemy, me: Vec2, t: f32) {
    let s = e.kind.spec();
    let c = p(e.pos);
    let col = if e.flash > 0.0 {
        WHITE
    } else {
        enemy_color(e.kind)
    };
    let face = (me - c).normalize_or_zero();
    let rot = face.y.atan2(face.x).to_degrees();
    let r = s.radius
        * if e.biting {
            1.0 + 0.12 * (t * 25.0).sin()
        } else {
            1.0
        };
    match e.kind {
        EKind::Raider => draw_poly(c.x, c.y, 3, r, rot, col),
        EKind::Grunt => draw_poly(c.x, c.y, 4, r, rot + 45.0, col),
        _ => {
            draw_poly(c.x, c.y, 6, r, rot, col);
            draw_poly_lines(c.x, c.y, 6, r, rot, 2.0, BLACK);
        }
    }
    let eye = c + face * r * 0.55;
    draw_circle(eye.x, eye.y, r * 0.22, Color::new(1.0, 0.9, 0.3, 1.0));
    if e.hp < e.max_hp && matches!(e.kind, EKind::Brute | EKind::Sumo | EKind::Boss) {
        bar(c.x - r, c.y - r - 8.0, r * 2.0, 4.0, e.hp / e.max_hp, RED);
    }
}

fn draw_commander(c: Vec2, aim: Vec2, t: f32, hurt: f32) {
    let col = if hurt > 0.0 {
        Color::new(1.0, 0.5, 0.5, 1.0)
    } else {
        BLUE
    };
    draw_circle(c.x, c.y, 16.0, Color::new(0.1, 0.15, 0.3, 1.0));
    draw_circle_lines(c.x, c.y, 16.0, 3.0, col);
    let d = (aim - c).normalize_or_zero();
    draw_line(c.x, c.y, c.x + d.x * 22.0, c.y + d.y * 22.0, 5.0, col);
    draw_circle(c.x, c.y, 6.0 + (t * 4.0).sin(), NANO);
}

fn nano_beam(from: Vec2, to: Vec2, t: f32, fx: &mut Vec<Fx>) {
    let mut col = NANO;
    col.a = 0.6;
    draw_line(from.x, from.y, to.x, to.y, 2.0, col);
    let n = 6;
    for i in 0..n {
        let k = ((t * 1.8 + i as f32 / n as f32) % 1.0).abs();
        let q = from.lerp(to, k);
        draw_circle(q.x, q.y, 2.5, NANO);
    }
    if rand::gen_range(0.0, 1.0) < 0.3 {
        fx.push(Fx {
            pos: to,
            vel: vec2(rand::gen_range(-40.0, 40.0), rand::gen_range(-40.0, 40.0)),
            life: 0.4,
            max: 0.4,
            size: 2.0,
            color: NANO,
        });
    }
}

fn burst(fx: &mut Vec<Fx>, at: Vec2, n: usize, speed: f32, size: f32, color: Color) {
    for _ in 0..n {
        let a = rand::gen_range(0.0, std::f32::consts::TAU);
        let v = rand::gen_range(0.3, 1.0) * speed;
        let life = rand::gen_range(0.3, 0.8);
        fx.push(Fx {
            pos: at,
            vel: vec2(a.cos(), a.sin()) * v,
            life,
            max: life,
            size,
            color,
        });
    }
}

fn center_text(s: &str, y: f32, size: f32, col: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (screen_width() - d.width) / 2.0, y, size, col);
}

struct Ui {
    selected: usize,
    beams: Vec<Beam>,
    fx: Vec<Fx>,
    shake: f32,
    hurt: f32,
    banner: (String, f32),
}

fn draw_world(g: &Game, ui: &mut Ui, mouse: Vec2, t: f32, preview: bool) {
    // Ground grid.
    draw_rectangle(0.0, 0.0, WORLD_W, WORLD_H, Color::new(0.08, 0.1, 0.12, 1.0));
    let grid = Color::new(0.14, 0.17, 0.2, 1.0);
    let mut x = 0.0;
    while x <= WORLD_W {
        draw_line(x, 0.0, x, WORLD_H, 1.0, grid);
        x += 80.0;
    }
    let mut y = 0.0;
    while y <= WORLD_H {
        draw_line(0.0, y, WORLD_W, y, 1.0, grid);
        y += 80.0;
    }
    draw_rectangle_lines(
        0.0,
        0.0,
        WORLD_W,
        WORLD_H,
        4.0,
        Color::new(0.4, 0.2, 0.2, 1.0),
    );

    let me = p(g.pos);
    draw_circle_lines(me.x, me.y, g.range, 1.5, Color::new(0.3, 1.0, 0.5, 0.25));
    for b in &g.buildings {
        if b.done()
            && b.kind.spec().damage > 0.0
            && b.pos.dist(V2 {
                x: mouse.x,
                y: mouse.y,
            }) < 30.0
        {
            let c = p(b.pos);
            draw_circle_lines(
                c.x,
                c.y,
                b.kind.spec().range,
                1.0,
                Color::new(1.0, 0.5, 0.5, 0.4),
            );
        }
        draw_building(b, t);
    }
    for pk in &g.pickups {
        let c = p(pk.pos);
        draw_poly(c.x, c.y, 4, 4.0 + pk.metal.sqrt(), t * 120.0, METAL);
    }
    for e in &g.enemies {
        draw_enemy(e, me, t);
    }
    draw_commander(me, mouse, t, ui.hurt);
    if let Some(to) = g.beam {
        nano_beam(me, p(to), t, &mut ui.fx);
    }
    for b in &g.buildings {
        if let Some(to) = b.beam {
            nano_beam(p(b.pos), p(to), t, &mut ui.fx);
        }
    }
    for bm in &ui.beams {
        let (w, col) = if bm.heavy {
            (5.0 * bm.life / 0.15, Color::new(1.0, 0.4, 0.9, 1.0))
        } else {
            (2.5, Color::new(1.0, 0.35, 0.25, bm.life / 0.1))
        };
        draw_line(bm.from.x, bm.from.y, bm.to.x, bm.to.y, w, col);
    }
    for f in &ui.fx {
        let mut c = f.color;
        c.a = f.life / f.max;
        draw_circle(f.pos.x, f.pos.y, f.size * (0.5 + c.a), c);
    }
    if preview {
        let k = KINDS[ui.selected];
        let ok = g.can_place(
            k,
            V2 {
                x: mouse.x,
                y: mouse.y,
            },
        );
        let mut col = if ok { NANO } else { RED };
        col.a = 0.6;
        draw_building_shape(k, mouse, k.spec().radius, col, false, t);
        if k.spec().range > 0.0 {
            draw_circle_lines(
                mouse.x,
                mouse.y,
                k.spec().range,
                1.0,
                Color { a: 0.3, ..col },
            );
        }
    }
}

fn draw_hud(g: &Game, ui: &Ui, muted: bool) {
    let w = screen_width();
    draw_rectangle(0.0, 0.0, w, 44.0, PANEL);
    draw_text(format!("METAL {:.0}", g.metal), 14.0, 29.0, 28.0, METAL);
    draw_text(format!("+{:.1}/s", g.income()), 150.0, 29.0, 20.0, METAL);
    draw_text(
        format!("BUILDPOWER {:.0}", g.total_bp()),
        240.0,
        29.0,
        26.0,
        NANO,
    );
    let wave = if g.wave == 0 {
        format!("FIRST WAVE IN {:.0}s", g.wave_time.ceil())
    } else if g.wave >= WAVES {
        "FINAL WAVE: DESTROY THE KORGOTH".to_owned()
    } else {
        format!("WAVE {}/{}  next {:.0}s", g.wave, WAVES, g.wave_time.ceil())
    };
    draw_text(&wave, w * 0.5 - 60.0, 29.0, 26.0, WHITE);
    draw_text(
        format!("LV {}   KILLS {}", g.level, g.kills),
        w - 260.0,
        29.0,
        24.0,
        WHITE,
    );
    bar(
        0.0,
        44.0,
        w,
        5.0,
        g.xp / g.xp_needed(),
        Color::new(0.4, 0.7, 1.0, 1.0),
    );

    // Commander health.
    bar(
        14.0,
        58.0,
        240.0,
        14.0,
        g.hp / g.max_hp,
        Color::new(0.3, 0.9, 0.4, 1.0),
    );
    draw_text(
        format!("COMMANDER {:.0}/{:.0}", g.hp, g.max_hp),
        18.0,
        70.0,
        18.0,
        WHITE,
    );
    if muted {
        draw_text("MUTED (M)", w - 120.0, 70.0, 20.0, GRAY);
    }

    // Build bar.
    let h = screen_height();
    let slot = 150.0;
    let x0 = (w - slot * KINDS.len() as f32) / 2.0;
    for (i, k) in KINDS.iter().enumerate() {
        let x = x0 + i as f32 * slot;
        let y = h - 78.0;
        let sel = i == ui.selected;
        draw_rectangle(x + 4.0, y, slot - 8.0, 70.0, PANEL);
        draw_rectangle_lines(
            x + 4.0,
            y,
            slot - 8.0,
            70.0,
            if sel { 3.0 } else { 1.0 },
            if sel { NANO } else { GRAY },
        );
        draw_building_shape(
            *k,
            vec2(x + 30.0, y + 35.0),
            16.0,
            kind_color(*k),
            true,
            0.0,
        );
        let s = k.spec();
        draw_text(format!("{}", i + 1), x + 10.0, y + 16.0, 16.0, GRAY);
        draw_text(s.name, x + 52.0, y + 26.0, 16.0, WHITE);
        let col = if g.metal >= s.cost { METAL } else { RED };
        draw_text(
            format!("{:.0} metal", s.cost),
            x + 52.0,
            y + 46.0,
            16.0,
            col,
        );
    }
    draw_text(
        format!(
            "Q/E cycle  Click place ({}/{} blueprints)  Shift repair  M mute  P pause",
            g.blueprints(),
            MAX_BLUEPRINTS
        ),
        x0 + 4.0,
        h - 84.0,
        18.0,
        LIGHTGRAY,
    );

    if ui.banner.1 > 0.0 {
        let mut c = Color::new(1.0, 0.8, 0.3, 1.0);
        c.a = ui.banner.1.min(1.0);
        center_text(&ui.banner.0, screen_height() * 0.28, 48.0, c);
    }
}

/// Draw the level-up cards; returns the card rects for mouse picking.
fn draw_levelup(g: &Game) -> Vec<Rect> {
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    center_text(
        &format!("LEVEL {}! CHOOSE AN UPGRADE", g.level),
        h * 0.25,
        44.0,
        NANO,
    );
    let cw = 260.0;
    let x0 = (w - cw * 3.0 - 40.0) / 2.0;
    let mut rects = Vec::new();
    let m = mouse_position();
    for (i, up) in g.choices.iter().enumerate() {
        let r = Rect::new(x0 + i as f32 * (cw + 20.0), h * 0.35, cw, 160.0);
        let hover = r.contains(vec2(m.0, m.1));
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if hover { 4.0 } else { 2.0 }, NANO);
        draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 30.0, 24.0, GRAY);
        draw_text(up.title(), r.x + 12.0, r.y + 70.0, 30.0, WHITE);
        draw_text(up.text(), r.x + 12.0, r.y + 110.0, 20.0, LIGHTGRAY);
        rects.push(r);
    }
    rects
}

fn draw_title(t: f32) {
    let (w, h) = (screen_width(), screen_height());
    clear_background(Color::new(0.04, 0.05, 0.07, 1.0));
    for i in 0..40 {
        let x = (i as f32 * 97.0 + t * 30.0) % w;
        let y = (i as f32 * 53.0) % h;
        draw_circle(x, y, 2.0, Color::new(0.3, 1.0, 0.5, 0.3));
    }
    center_text("NANO FORTRESS", h * 0.26, 84.0, NANO);
    center_text(
        "You cannot shoot. You can only build.",
        h * 0.36,
        28.0,
        WHITE,
    );
    let lines = [
        "Click to drop a blueprint, then stand near it: your nano beam builds it.",
        "Towers fight for you, Nano Turrets add buildpower, Extractors add metal.",
        "Hold SHIFT to prioritise repairs while the swarm chews your walls.",
        "Survive 12 waves and destroy the Korgoth. Do not let your Commander fall.",
        "",
        "WASD move   Click place   Q/E or 1-5 select   Shift repair   M mute   P pause",
    ];
    for (i, l) in lines.iter().enumerate() {
        center_text(l, h * 0.46 + i as f32 * 30.0, 22.0, LIGHTGRAY);
    }
    if (t * 2.0) as i32 % 2 == 0 {
        center_text("PRESS ANY KEY OR CLICK TO START", h * 0.82, 32.0, METAL);
    }
}

fn draw_over(g: &Game) {
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
    let (title, col) = if g.outcome == Outcome::Won {
        ("FORTRESS HOLDS: VICTORY", NANO)
    } else {
        ("COMMANDER DESTROYED", RED)
    };
    center_text(title, h * 0.35, 64.0, col);
    center_text(
        &format!(
            "Wave {}/{}   Level {}   Kills {}   Time {}:{:02}",
            g.wave,
            WAVES,
            g.level,
            g.kills,
            g.time as u32 / 60,
            g.time as u32 % 60
        ),
        h * 0.47,
        28.0,
        WHITE,
    );
    center_text("Press ENTER or R to play again", h * 0.58, 28.0, METAL);
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Bld <", KeyCode::Q),
        touch::key("Bld >", KeyCode::E),
        touch::key("Repair", KeyCode::LeftShift),
        touch::small("Pause", KeyCode::P),
        touch::small("Mute", KeyCode::M),
        touch::small("Retry", KeyCode::R),
    ]);
    let mut audio = Audio::new().await;
    let mut seed = 1u64;
    let mut g = Game::new(seed);
    let mut screen = Screen::Title;
    let mut ui = Ui {
        selected: 1,
        beams: Vec::new(),
        fx: Vec::new(),
        shake: 0.0,
        hurt: 0.0,
        banner: (String::new(), 0.0),
    };
    let mut events = Vec::new();

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        let now = get_time();
        let t = now as f32;
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }

        match screen {
            Screen::Title => {
                draw_title(t);
                if any_input() {
                    audio.start_music();
                    seed = seed.wrapping_add((now * 1000.0) as u64);
                    g = Game::new(seed);
                    screen = Screen::Playing;
                }
                touch::next_frame().await;
                continue;
            }
            Screen::Playing => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Paused;
                }
            }
            Screen::Paused => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Playing;
                }
            }
            Screen::Over => {
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::R) {
                    seed = seed.wrapping_add(7);
                    g = Game::new(seed);
                    ui.beams.clear();
                    ui.fx.clear();
                    screen = Screen::Playing;
                }
            }
        }

        // Camera follows the commander, clamped to the world.
        let (sw, sh) = (screen_width(), screen_height());
        let mut cam_c = p(g.pos);
        cam_c.x = cam_c.x.clamp(sw / 2.0, (WORLD_W - sw / 2.0).max(sw / 2.0));
        cam_c.y = cam_c
            .y
            .clamp(sh / 2.0 - 50.0, (WORLD_H - sh / 2.0 + 90.0).max(sh / 2.0));
        if ui.shake > 0.0 {
            cam_c += vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * ui.shake * 8.0;
        }
        let cam =
            Camera2D::from_display_rect(Rect::new(cam_c.x - sw / 2.0, cam_c.y - sh / 2.0, sw, sh));
        let mouse = cam.screen_to_world(mouse_position().into());

        let leveling = !g.choices.is_empty();
        let active = screen == Screen::Playing && g.outcome == Outcome::Playing;
        let mut card_click = None;
        if active && !leveling {
            for (i, key) in [
                KeyCode::Key1,
                KeyCode::Key2,
                KeyCode::Key3,
                KeyCode::Key4,
                KeyCode::Key5,
            ]
            .iter()
            .enumerate()
            {
                if is_key_pressed(*key) {
                    ui.selected = i;
                }
            }
            if is_key_pressed(KeyCode::Q) {
                ui.selected = (ui.selected + KINDS.len() - 1) % KINDS.len();
            }
            if is_key_pressed(KeyCode::E) {
                ui.selected = (ui.selected + 1) % KINDS.len();
            }
            if is_mouse_button_pressed(MouseButton::Left) && mouse_position().1 < sh - 90.0 {
                let e = g.place(
                    KINDS[ui.selected],
                    V2 {
                        x: mouse.x,
                        y: mouse.y,
                    },
                );
                events.push(e);
            }
            let mut dir = V2::default();
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
            let repair = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            let hp_before = g.hp;
            g.update(dt, Input { dir, repair }, &mut events);
            if g.hp < hp_before {
                ui.hurt = 0.15;
            }
            if g.enemies.iter().any(|e| e.biting) {
                audio.chew(now);
            }
        } else if active && leveling {
            for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                .iter()
                .enumerate()
            {
                if is_key_pressed(*key) {
                    card_click = Some(i);
                }
            }
        }

        // Events -> sound + effects.
        for e in events.drain(..) {
            audio.on_event(&e, now);
            match e {
                Event::Shot { from, to, heavy } => ui.beams.push(Beam {
                    from: p(from),
                    to: p(to),
                    life: if heavy { 0.15 } else { 0.1 },
                    heavy,
                }),
                Event::EnemyKilled { pos, big } => {
                    let n = if big { 40 } else { 10 };
                    burst(
                        &mut ui.fx,
                        p(pos),
                        n,
                        if big { 260.0 } else { 140.0 },
                        3.0,
                        ORANGE,
                    );
                    if big {
                        ui.shake = ui.shake.max(0.6);
                    }
                }
                Event::BuildingLost { pos } => {
                    burst(&mut ui.fx, p(pos), 30, 200.0, 4.0, GRAY);
                    ui.shake = ui.shake.max(0.4);
                }
                Event::Completed { pos } => burst(&mut ui.fx, p(pos), 16, 90.0, 2.5, NANO),
                Event::WaveStart(n) => {
                    ui.banner = if n == WAVES {
                        ("FINAL WAVE: THE KORGOTH APPROACHES".to_owned(), 3.0)
                    } else {
                        (format!("WAVE {n}"), 2.0)
                    }
                }
                Event::GameOver | Event::Victory => screen = Screen::Over,
                _ => {}
            }
        }
        if screen == Screen::Playing {
            let fdt = if leveling { 0.0 } else { dt };
            for b in &mut ui.beams {
                b.life -= fdt;
            }
            ui.beams.retain(|b| b.life > 0.0);
            for f in &mut ui.fx {
                f.pos += f.vel * fdt;
                f.vel *= 1.0 - 2.0 * fdt;
                f.life -= fdt;
            }
            ui.fx.retain(|f| f.life > 0.0);
            ui.shake = (ui.shake - dt * 2.0).max(0.0);
            ui.hurt = (ui.hurt - dt).max(0.0);
            ui.banner.1 -= dt;
        }
        let beaming = active
            && !leveling
            && (g.beam.is_some() || g.buildings.iter().any(|b| b.beam.is_some()));
        audio.set_hum(beaming);

        // Render.
        clear_background(BLACK);
        set_camera(&cam);
        draw_world(&g, &mut ui, mouse, t, active && !leveling);
        set_default_camera();
        draw_hud(&g, &ui, audio.muted);
        if active && leveling {
            let rects = draw_levelup(&g);
            if is_mouse_button_pressed(MouseButton::Left) {
                let m = mouse_position();
                card_click = rects.iter().position(|r| r.contains(vec2(m.0, m.1)));
            }
            if let Some(i) = card_click {
                g.choose(i);
            }
        }
        match screen {
            Screen::Paused => {
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
                center_text("PAUSED", sh * 0.45, 64.0, WHITE);
                center_text("P to resume", sh * 0.53, 28.0, LIGHTGRAY);
            }
            Screen::Over => draw_over(&g),
            _ => {}
        }
        touch::next_frame().await;
    }
}
