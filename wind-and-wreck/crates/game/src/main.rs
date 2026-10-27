//! Wind and Wreck: macroquad front end (input, camera, drawing, audio).

mod audio;

use audio::{Audio, Sfx};
use macroquad::prelude::*;
use sim::game::{
    ARENA, BUILD_RANGE, Building, EnemyKind, Event, Game, Input, Kind, PLAYER_R, Phase,
};
use sim::math::V2;
use touch::{
    is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_position,
};

const BG: Color = Color::new(0.06, 0.09, 0.1, 1.0);
const ENERGY: Color = Color::new(1.0, 0.85, 0.2, 1.0);
const SHIELD: Color = Color::new(0.4, 0.8, 1.0, 1.0);
const EMP: Color = Color::new(0.55, 0.45, 1.0, 1.0);

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
}

struct Fx {
    pos: Vec2,
    t: f32,
    big: bool,
}

struct App {
    game: Game,
    screen: Screen,
    audio: Audio,
    build_menu: bool,
    selected: Kind,
    paused: bool,
    fx: Vec<Fx>,
    blade: f32,
    banner: (String, f32),
    seed: u64,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Wind and Wreck".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Shield", KeyCode::Space),
        touch::key("Build", KeyCode::B),
        touch::key("Wind", KeyCode::Key1),
        touch::key("Solar", KeyCode::Key2),
        touch::key("Store", KeyCode::Key3),
        touch::key("Laser", KeyCode::Key4),
        touch::small("Pause", KeyCode::P),
        touch::small("Mute", KeyCode::M),
        touch::small("Retry", KeyCode::R),
    ]);
    clear_background(BG);
    center(
        "Charging capacitors...",
        screen_height() * 0.5,
        32.0,
        ENERGY,
    );
    touch::next_frame().await;
    let audio = Audio::new().await;
    let mut app = App {
        game: Game::new(1),
        screen: Screen::Title,
        audio,
        build_menu: false,
        selected: Kind::Wind,
        paused: false,
        fx: Vec::new(),
        blade: 0.0,
        banner: (String::new(), 0.0),
        seed: 1,
    };
    loop {
        let dt = get_frame_time();
        if is_key_pressed(KeyCode::M) {
            app.audio.toggle_mute();
        }
        match app.screen {
            Screen::Title => {
                draw_title(&app);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    app.audio.start_music();
                    app.audio.play(Sfx::Build);
                    app.seed = (get_time() * 1000.0) as u64 ^ 0x5eed;
                    app.game = Game::new(app.seed);
                    app.screen = Screen::Play;
                    app.banner = ("WAVE 1 - KEEP THE LIGHTS ON".into(), 3.0);
                }
            }
            Screen::Play => {
                app.audio.start_music();
                step(&mut app, dt);
                draw_world(&app);
                draw_hud(&app);
            }
        }
        touch::next_frame().await;
    }
}

fn camera_for(game: &Game) -> Camera2D {
    let zoom = 1.0;
    Camera2D {
        target: v(game.player),
        zoom: vec2(2.0 * zoom / screen_width(), 2.0 * zoom / screen_height()),
        ..Default::default()
    }
}

fn mouse_world(game: &Game) -> V2 {
    let w = camera_for(game).screen_to_world(mouse_position().into());
    V2::new(w.x, w.y)
}

fn step(app: &mut App, dt: f32) {
    let g = &mut app.game;
    match g.phase {
        Phase::Won | Phase::Lost => {
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::R) {
                app.seed = app.seed.wrapping_add(1);
                app.game = Game::new(app.seed);
                app.fx.clear();
                app.build_menu = false;
                app.banner = ("WAVE 1 - KEEP THE LIGHTS ON".into(), 3.0);
            }
            return;
        }
        Phase::LevelUp => {
            let mut choose = None;
            for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                .into_iter()
                .enumerate()
            {
                if is_key_pressed(k) {
                    choose = Some(i);
                }
            }
            if is_mouse_button_pressed(MouseButton::Left)
                && let Some(i) = card_at(mouse_position().into())
            {
                choose = Some(i);
            }
            let input = Input {
                choose,
                ..Input::default()
            };
            g.update(dt, &input);
            handle_events(app);
            return;
        }
        Phase::Playing => {}
    }
    if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
        app.paused = !app.paused;
    }
    if app.paused {
        return;
    }
    if is_key_pressed(KeyCode::B) {
        app.build_menu = !app.build_menu;
    }
    for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
        .into_iter()
        .enumerate()
    {
        if is_key_pressed(k) {
            app.selected = Kind::ALL[i];
            app.build_menu = true;
        }
    }
    let mut mv = V2::ZERO;
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        mv.y -= 1.0;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        mv.y += 1.0;
    }
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        mv.x -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        mv.x += 1.0;
    }
    let aim = mouse_world(g);
    let mut input = Input {
        movement: mv,
        aim,
        shield: is_mouse_button_down(MouseButton::Right) || is_key_down(KeyCode::Space),
        ..Input::default()
    };
    if app.build_menu {
        if is_mouse_button_pressed(MouseButton::Left) {
            input.build = Some((app.selected, aim));
        }
    } else {
        input.fire = is_mouse_button_down(MouseButton::Left);
    }
    g.update(dt, &input);
    app.blade += dt * 9.0 * g.gust();
    handle_events(app);
    for f in &mut app.fx {
        f.t += dt;
    }
    app.fx.retain(|f| f.t < 0.6);
    app.banner.1 -= dt;
}

fn handle_events(app: &mut App) {
    let events = app.game.events.clone();
    for e in &events {
        app.audio.on_event(e);
        match *e {
            Event::Kill { big, pos } => app.fx.push(Fx {
                pos: v(pos),
                t: 0.0,
                big,
            }),
            Event::BuildingLost { pos } => {
                app.fx.push(Fx {
                    pos: v(pos),
                    t: 0.0,
                    big: true,
                });
                app.banner = ("GENERATOR LOST!".into(), 1.5);
            }
            Event::WaveStart(w) => app.banner = (format!("WAVE {w}"), 2.5),
            Event::BossSpawn => app.banner = ("OVERLOAD CORE INBOUND - DESTROY IT".into(), 4.0),
            Event::Brownout => app.banner = ("BROWNOUT - BUILD MORE POWER".into(), 2.0),
            _ => {}
        }
    }
}

fn draw_title(app: &App) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    let t = get_time() as f32;
    // Spinning turbine backdrop.
    draw_turbine(vec2(w * 0.5, h * 0.3), 3.0, t * 2.0, 1.0);
    center("WIND AND WRECK", h * 0.52, 72.0, ENERGY);
    center(
        "Energy is everything. Keep the lights on.",
        h * 0.59,
        26.0,
        WHITE,
    );
    let lines = [
        "WASD move  |  Mouse aim  |  LMB laser (costs energy)  |  RMB / Space shield (drains energy)",
        "B build menu, 1-4 pick: Wind (gusty)  Solar (steady, fragile)  Storage  Laser Tower",
        "EMP bots drain your grid. Survive 12 waves and destroy the Overload Core.",
        "M mute  |  P pause",
    ];
    for (i, l) in lines.iter().enumerate() {
        center(l, h * 0.67 + i as f32 * 28.0, 20.0, LIGHTGRAY);
    }
    if (t * 2.0).sin() > -0.3 {
        center("Press any key to start", h * 0.86, 32.0, SHIELD);
    }
    if app.audio.muted {
        draw_text("MUTED", 12.0, h - 12.0, 20.0, GRAY);
    }
}

fn center(s: &str, y: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (screen_width() - d.width) * 0.5, y, size, c);
}

fn draw_turbine(p: Vec2, scale: f32, angle: f32, alpha: f32) {
    let pole = Color::new(0.75, 0.78, 0.8, alpha);
    draw_circle(p.x, p.y, 7.0 * scale, Color::new(0.3, 0.35, 0.38, alpha));
    for i in 0..3 {
        let a = angle + i as f32 * std::f32::consts::TAU / 3.0;
        let tip = p + Vec2::from_angle(a) * 20.0 * scale;
        draw_line(p.x, p.y, tip.x, tip.y, 3.5 * scale, pole);
    }
    draw_circle(p.x, p.y, 3.0 * scale, WHITE);
}

fn draw_building(b: &Building, blade: f32, game: &Game) {
    let p = v(b.pos);
    let r = b.kind.radius();
    let flash = b.flash > 0.0;
    match b.kind {
        Kind::Wind => draw_turbine(p, 0.9, blade + p.x * 0.01, 1.0),
        Kind::Solar => {
            draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, DARKGRAY);
            for i in 0..2 {
                for j in 0..2 {
                    draw_rectangle(
                        p.x - r + 2.0 + i as f32 * r,
                        p.y - r + 2.0 + j as f32 * r,
                        r - 4.0,
                        r - 4.0,
                        Color::new(0.15, 0.35, 0.75, 1.0),
                    );
                }
            }
        }
        Kind::Storage => {
            let fill = (game.energy / game.capacity()).clamp(0.0, 1.0);
            draw_circle(p.x, p.y, r, Color::new(0.25, 0.27, 0.22, 1.0));
            draw_circle(p.x, p.y, r * 0.7 * fill.max(0.1), ENERGY);
            draw_circle_lines(p.x, p.y, r, 2.0, GRAY);
        }
        Kind::Turret => {
            draw_poly(p.x, p.y, 6, r, 0.0, Color::new(0.35, 0.4, 0.3, 1.0));
            draw_circle(p.x, p.y, r * 0.45, Color::new(1.0, 0.4, 0.3, 1.0));
        }
    }
    if flash {
        draw_circle_lines(p.x, p.y, r + 3.0, 2.0, RED);
    }
    let max = if b.kind == Kind::Solar {
        b.kind.max_hp() * game.stats.solar_hp_mult
    } else {
        b.kind.max_hp()
    };
    if b.hp < max {
        bar(p.x - r, p.y + r + 4.0, r * 2.0, 3.0, b.hp / max, GREEN);
    }
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, c: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, c);
}

fn draw_world(app: &App) {
    let g = &app.game;
    clear_background(BG);
    set_camera(&camera_for(g));
    // Grid floor.
    let step = 80.0;
    let line = Color::new(1.0, 1.0, 1.0, 0.04);
    let mut x = -ARENA;
    while x <= ARENA {
        draw_line(x, -ARENA, x, ARENA, 1.0, line);
        draw_line(-ARENA, x, ARENA, x, 1.0, line);
        x += step;
    }
    draw_rectangle_lines(-ARENA, -ARENA, ARENA * 2.0, ARENA * 2.0, 4.0, ORANGE);

    // Power lines from each generator to the commander.
    let pl = v(g.player);
    for b in &g.buildings {
        if matches!(b.kind, Kind::Wind | Kind::Solar | Kind::Storage)
            && b.pos.dist(g.player) < 500.0
        {
            let a = if g.energy > 1.0 { 0.25 } else { 0.06 };
            draw_line(
                b.pos.x,
                b.pos.y,
                pl.x,
                pl.y,
                1.5,
                Color::new(1.0, 0.85, 0.2, a),
            );
        }
    }
    for b in &g.buildings {
        draw_building(b, app.blade, g);
    }

    // Orbs.
    for o in &g.orbs {
        let r = 3.0 + o.value.min(10) as f32 * 0.8;
        draw_circle(o.pos.x, o.pos.y, r, Color::new(0.5, 1.0, 0.6, 1.0));
    }

    // Enemies.
    let t = get_time() as f32;
    for e in &g.enemies {
        let p = v(e.pos);
        if e.kind.drain_range() > 100.0 {
            draw_circle_lines(
                p.x,
                p.y,
                e.kind.drain_range(),
                1.0,
                Color::new(0.55, 0.45, 1.0, 0.25),
            );
        }
        let body = match e.kind {
            EnemyKind::Spark => Color::new(0.85, 0.3, 0.3, 1.0),
            EnemyKind::Leech => Color::new(0.6, 0.3, 0.8, 1.0),
            EnemyKind::Brute => Color::new(0.7, 0.45, 0.2, 1.0),
            EnemyKind::Overload => Color::new(0.4, 0.15, 0.6, 1.0),
        };
        let c = if e.flash > 0.0 { WHITE } else { body };
        draw_poly(p.x, p.y, 6, e.radius, t * 60.0, c);
        draw_circle(p.x, p.y, e.radius * 0.4, EMP);
        // EMP crackle.
        let a = t * 9.0 + p.x;
        let q = p + Vec2::from_angle(a) * e.radius * 1.2;
        draw_line(p.x, p.y, q.x, q.y, 1.5, EMP);
        if e.hp < e.max_hp && e.kind != EnemyKind::Overload {
            bar(
                p.x - e.radius,
                p.y - e.radius - 6.0,
                e.radius * 2.0,
                3.0,
                e.hp / e.max_hp,
                RED,
            );
        }
    }

    // Bolts.
    for b in &g.bolts {
        let tail = b.pos - b.vel * 0.02;
        let c = if b.from_turret {
            Color::new(1.0, 0.45, 0.3, 1.0)
        } else {
            Color::new(0.5, 1.0, 1.0, 1.0)
        };
        draw_line(b.pos.x, b.pos.y, tail.x, tail.y, 3.0, c);
    }

    // Commander.
    let aim = mouse_world(g);
    let dir = Vec2::new(aim.x - g.player.x, aim.y - g.player.y).normalize_or_zero();
    let body = if g.hurt_flash > 0.0 {
        RED
    } else {
        Color::new(0.3, 0.55, 0.9, 1.0)
    };
    draw_circle(pl.x, pl.y, PLAYER_R, body);
    draw_circle_lines(pl.x, pl.y, PLAYER_R, 2.0, WHITE);
    let gun = pl + dir * (PLAYER_R + 10.0);
    draw_line(pl.x, pl.y, gun.x, gun.y, 5.0, LIGHTGRAY);
    if g.shield_on {
        let wob = 4.0 * (t * 12.0).sin();
        draw_circle(pl.x, pl.y, 34.0 + wob, Color::new(0.4, 0.8, 1.0, 0.18));
        draw_circle_lines(pl.x, pl.y, 34.0 + wob, 2.0, SHIELD);
    }

    // Build preview.
    if app.build_menu && g.phase == Phase::Playing {
        draw_circle_lines(pl.x, pl.y, BUILD_RANGE, 1.0, Color::new(1.0, 1.0, 1.0, 0.2));
        let ok = g.can_build(app.selected, aim);
        let c = if ok {
            Color::new(0.3, 1.0, 0.4, 0.5)
        } else {
            Color::new(1.0, 0.3, 0.3, 0.5)
        };
        draw_circle(aim.x, aim.y, app.selected.radius(), c);
    }

    // Explosions.
    for f in &app.fx {
        let k = f.t / 0.6;
        let r = if f.big { 60.0 } else { 26.0 } * k;
        draw_circle(
            f.pos.x,
            f.pos.y,
            r,
            Color::new(1.0, 0.6 + 0.4 * k, 0.2, 0.6 * (1.0 - k)),
        );
        draw_circle_lines(
            f.pos.x,
            f.pos.y,
            r * 1.2,
            2.0,
            Color::new(0.6, 0.5, 1.0, 1.0 - k),
        );
    }
    set_default_camera();
}

fn draw_hud(app: &App) {
    let g = &app.game;
    let (w, h) = (screen_width(), screen_height());
    // Top-left: HP and energy.
    draw_rectangle(8.0, 8.0, 330.0, 112.0, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_text("HULL", 16.0, 30.0, 20.0, WHITE);
    bar(70.0, 16.0, 260.0, 16.0, g.hp / g.stats.max_hp, RED);
    draw_text(
        format!("{:.0}/{:.0}", g.hp, g.stats.max_hp),
        76.0,
        29.0,
        18.0,
        WHITE,
    );
    draw_text("POWER", 16.0, 56.0, 20.0, WHITE);
    let low = g.energy < g.stats.laser_cost * 3.0;
    let ec = if low && (get_time() * 6.0).sin() > 0.0 {
        RED
    } else {
        ENERGY
    };
    bar(70.0, 42.0, 260.0, 16.0, g.energy / g.capacity(), ec);
    draw_text(
        format!("{:.0}/{:.0}", g.energy, g.capacity()),
        76.0,
        55.0,
        18.0,
        BLACK,
    );
    let net = g.income - g.spend / get_frame_time().max(1e-3);
    draw_text(
        format!("+{:.1}/s gen   net {:+.1}/s", g.income, net),
        16.0,
        80.0,
        18.0,
        if net >= 0.0 { GREEN } else { ORANGE },
    );
    draw_text(
        format!(
            "Laser {:.1} E  |  Shield {:.0} E/s",
            g.stats.laser_cost, g.stats.shield_drain
        ),
        16.0,
        104.0,
        18.0,
        LIGHTGRAY,
    );

    // Wind gauge.
    let gx = 350.0;
    draw_rectangle(gx, 8.0, 170.0, 70.0, Color::new(0.0, 0.0, 0.0, 0.55));
    let gust = g.gust();
    draw_text("WIND", gx + 8.0, 26.0, 18.0, WHITE);
    let label = if gust > 1.4 {
        "GUST"
    } else if gust < 0.5 {
        "LULL"
    } else {
        "breeze"
    };
    draw_text(
        format!("{:.0}% {label}", gust * 100.0),
        gx + 60.0,
        26.0,
        18.0,
        SKYBLUE,
    );
    bar(gx + 8.0, 34.0, 154.0, 10.0, gust / 2.0, SKYBLUE);
    // Forecast sparkline for the next 20 s.
    for i in 0..30 {
        let f = g.gust_at(g.time + i as f32 * 0.7) / 2.0;
        draw_rectangle(
            gx + 8.0 + i as f32 * 5.1,
            74.0 - f * 24.0,
            4.0,
            f * 24.0,
            Color::new(0.5, 0.8, 1.0, 0.5),
        );
    }

    // Top-right: wave, level, scrap.
    let rx = w - 250.0;
    draw_rectangle(rx, 8.0, 242.0, 96.0, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_text(
        format!(
            "WAVE {}/{}  {:.0}s",
            g.wave,
            sim::game::WAVES,
            (sim::game::WAVE_SECS - g.wave_time).max(0.0)
        ),
        rx + 8.0,
        30.0,
        22.0,
        WHITE,
    );
    draw_text(
        format!("LV {}   kills {}", g.level, g.kills),
        rx + 8.0,
        54.0,
        20.0,
        WHITE,
    );
    bar(
        rx + 8.0,
        62.0,
        226.0,
        8.0,
        g.xp as f32 / g.xp_to_next() as f32,
        GREEN,
    );
    draw_text(
        format!("SCRAP {}", g.scrap),
        rx + 8.0,
        94.0,
        22.0,
        Color::new(0.5, 1.0, 0.6, 1.0),
    );

    // Boss bar.
    if let Some(b) = g.boss() {
        let bw = w * 0.5;
        draw_text("OVERLOAD CORE", (w - bw) * 0.5, h - 54.0, 22.0, EMP);
        bar((w - bw) * 0.5, h - 46.0, bw, 14.0, b.hp / b.max_hp, EMP);
    }

    // Build menu.
    let by = h - 30.0;
    if app.build_menu {
        draw_rectangle(8.0, h - 150.0, 330.0, 142.0, Color::new(0.0, 0.0, 0.0, 0.7));
        draw_text("BUILD (B close, LMB place)", 16.0, h - 130.0, 18.0, ENERGY);
        for (i, k) in Kind::ALL.iter().enumerate() {
            let y = h - 106.0 + i as f32 * 26.0;
            let sel = *k == app.selected;
            let afford = g.scrap >= k.cost();
            let c = if !afford {
                GRAY
            } else if sel {
                YELLOW
            } else {
                WHITE
            };
            draw_text(
                format!(
                    "{}{} {} [{}] {}",
                    if sel { ">" } else { " " },
                    i + 1,
                    k.name(),
                    k.cost(),
                    k.blurb()
                ),
                16.0,
                y,
                17.0,
                c,
            );
        }
    } else {
        draw_text("B: build menu", 16.0, by, 20.0, LIGHTGRAY);
    }
    if app.audio.muted {
        draw_text("MUTED (M)", w - 110.0, h - 12.0, 20.0, GRAY);
    }

    // Banner.
    if app.banner.1 > 0.0 {
        let a = app.banner.1.min(1.0);
        center(&app.banner.0, h * 0.25, 40.0, Color::new(1.0, 0.85, 0.3, a));
    }

    if app.paused {
        overlay();
        center("PAUSED", h * 0.45, 60.0, WHITE);
        center("P to resume", h * 0.52, 26.0, LIGHTGRAY);
    }

    match g.phase {
        Phase::LevelUp => draw_level_up(g),
        Phase::Won => end_screen(g, true),
        Phase::Lost => end_screen(g, false),
        Phase::Playing => {}
    }
}

fn overlay() {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
}

fn card_rect(i: usize) -> Rect {
    let (w, h) = (screen_width(), screen_height());
    let cw = 260.0;
    let gap = 24.0;
    let total = cw * 3.0 + gap * 2.0;
    Rect::new(
        (w - total) * 0.5 + i as f32 * (cw + gap),
        h * 0.38,
        cw,
        140.0,
    )
}

fn card_at(p: Vec2) -> Option<usize> {
    (0..3).find(|&i| card_rect(i).contains(p))
}

fn draw_level_up(g: &Game) {
    overlay();
    let h = screen_height();
    center(&format!("LEVEL {}!", g.level), h * 0.3, 54.0, ENERGY);
    center("Pick an upgrade (1/2/3 or click)", h * 0.34, 22.0, WHITE);
    let hover = card_at(mouse_position().into());
    for (i, u) in g.choices.iter().enumerate() {
        let r = card_rect(i);
        let c = if hover == Some(i) {
            Color::new(0.25, 0.3, 0.2, 1.0)
        } else {
            Color::new(0.12, 0.15, 0.17, 1.0)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, c);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, ENERGY);
        draw_text(format!("{}", i + 1), r.x + 10.0, r.y + 28.0, 28.0, ENERGY);
        draw_text(u.name(), r.x + 36.0, r.y + 28.0, 22.0, WHITE);
        draw_text(u.desc(), r.x + 10.0, r.y + 80.0, 17.0, LIGHTGRAY);
    }
}

fn end_screen(g: &Game, won: bool) {
    overlay();
    let h = screen_height();
    if won {
        center("GRID SECURED", h * 0.4, 72.0, ENERGY);
        center(
            "The Overload Core is scrap. The lights stay on.",
            h * 0.47,
            26.0,
            WHITE,
        );
    } else {
        center("BLACKOUT", h * 0.4, 72.0, RED);
        center("Your commander has been wrecked.", h * 0.47, 26.0, WHITE);
    }
    center(
        &format!(
            "Wave {}  |  Level {}  |  Kills {}  |  Time {:.0}s",
            g.wave, g.level, g.kills, g.time
        ),
        h * 0.54,
        24.0,
        LIGHTGRAY,
    );
    center("Press Enter or R to play again", h * 0.62, 28.0, SHIELD);
}
