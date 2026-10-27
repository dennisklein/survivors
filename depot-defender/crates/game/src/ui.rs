//! HUD, menus, network map and full-screen overlays.

use crate::fx::Fx;
use macroquad::prelude::*;
use rules::data::{Cargo, EnemyKind, TOWNS, Upgrade};
use rules::game::{
    DEPOT_MAX_HP, FINAL_WAVE, Game, MAX_LINE_LEVEL, MAX_WEAPON_LEVEL, Phase, TrainState,
};
use touch::{is_key_pressed, is_mouse_button_pressed, mouse_position};

pub const BG: Color = Color::new(0.09, 0.1, 0.09, 1.0);
pub const GROUND: Color = Color::new(0.2, 0.24, 0.17, 1.0);
pub const FENCE: Color = Color::new(0.45, 0.38, 0.25, 1.0);
pub const SLEEPER: Color = Color::new(0.33, 0.24, 0.16, 1.0);
pub const RAIL: Color = Color::new(0.7, 0.7, 0.72, 1.0);
pub const PLATFORM: Color = Color::new(0.5, 0.5, 0.47, 1.0);
const PANEL: Color = Color::new(0.06, 0.07, 0.08, 0.88);
const GOLD: Color = Color::new(1.0, 0.82, 0.3, 1.0);

/// Rasterize every glyph size used by the UI up front, so the font atlas does
/// not grow (and replace its texture) in the middle of a frame.
pub fn warm_glyph_cache() {
    let chars: String = (32u8..127).map(char::from).collect();
    for size in [
        16, 17, 18, 19, 20, 22, 24, 26, 28, 30, 32, 36, 40, 54, 60, 80,
    ] {
        measure_text(&chars, None, size, 1.0);
    }
}

pub fn cargo_color(c: Cargo) -> Color {
    match c {
        Cargo::Steel => Color::new(0.62, 0.72, 0.9, 1.0),
        Cargo::Coal => Color::new(1.0, 0.5, 0.15, 1.0),
        Cargo::Goods => Color::new(0.85, 0.72, 0.4, 1.0),
    }
}

pub fn enemy_color(k: EnemyKind) -> Color {
    match k {
        EnemyKind::Rustbug => Color::new(0.6, 0.35, 0.2, 1.0),
        EnemyKind::Sprinter => Color::new(0.8, 0.22, 0.25, 1.0),
        EnemyKind::Brute => Color::new(0.45, 0.4, 0.55, 1.0),
        EnemyKind::Boss => Color::new(0.35, 0.15, 0.4, 1.0),
    }
}

pub fn depot_hp_color(frac: f32) -> Color {
    if frac > 0.5 {
        GREEN
    } else if frac > 0.25 {
        YELLOW
    } else {
        RED
    }
}

pub fn text_centered(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    let w = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, cx - w / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color, label: &str) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
    draw_rectangle_lines(x, y, w, h, 1.5, Color::new(1.0, 1.0, 1.0, 0.3));
    draw_text(label, x + 6.0, y + h - 4.0, 18.0, WHITE);
}

fn mouse_in(r: Rect) -> bool {
    let (mx, my) = mouse_position();
    r.contains(vec2(mx, my))
}

/// Draws a button and returns whether it was clicked.
fn button(r: Rect, label: &str, enabled: bool, accent: Color) -> bool {
    let hover = enabled && mouse_in(r);
    let bg = if !enabled {
        Color::new(0.2, 0.2, 0.2, 0.9)
    } else if hover {
        Color::new(accent.r * 0.6, accent.g * 0.6, accent.b * 0.6, 1.0)
    } else {
        Color::new(0.14, 0.15, 0.17, 0.95)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    let border = if enabled { accent } else { GRAY };
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);
    let tc = if enabled { WHITE } else { GRAY };
    text_centered(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, tc);
    hover && is_mouse_button_pressed(MouseButton::Left)
}

pub fn hud(game: &Game, fx: &Fx, muted: bool) {
    let sw = screen_width();
    let p = &game.player;
    // XP bar across the top.
    bar(
        0.0,
        0.0,
        sw,
        14.0,
        p.xp as f32 / p.xp_next as f32,
        Color::new(0.3, 0.8, 1.0, 1.0),
        "",
    );
    text_centered(&format!("LV {}", p.level), sw / 2.0, 12.0, 16.0, WHITE);

    bar(
        12.0,
        22.0,
        220.0,
        20.0,
        p.hp / p.max_hp,
        Color::new(0.85, 0.2, 0.25, 1.0),
        &format!("Health {:.0}/{:.0}", p.hp.max(0.0), p.max_hp),
    );
    let dfrac = game.depot_hp / DEPOT_MAX_HP;
    bar(
        12.0,
        46.0,
        220.0,
        20.0,
        dfrac,
        depot_hp_color(dfrac),
        &format!("Depot {:.0}/{:.0}", game.depot_hp.max(0.0), DEPOT_MAX_HP),
    );

    // Wave and money.
    let wave = if game.boss_alive() && game.wave_time <= 0.0 {
        format!("Wave {}/{}  BOSS", game.wave, FINAL_WAVE)
    } else {
        format!(
            "Wave {}/{}  {:>2.0}s",
            game.wave,
            FINAL_WAVE,
            game.wave_time.ceil()
        )
    };
    let ww = measure_text(&wave, None, 28, 1.0).width;
    draw_text(&wave, sw - ww - 14.0, 42.0, 28.0, WHITE);
    let cash = format!("${}   Kills {}", game.cash, game.stats.kills);
    let cw = measure_text(&cash, None, 22, 1.0).width;
    draw_text(&cash, sw - cw - 14.0, 66.0, 22.0, GOLD);

    // Weapons.
    let mut y = 92.0;
    for c in Cargo::ALL {
        let lvl = game.weapons[c.index()];
        let (txt, col) = if lvl == 0 {
            (
                format!("{} - locked (collect {})", c.weapon(), c.name()),
                GRAY,
            )
        } else {
            (format!("{} Lv {}", c.weapon(), lvl), cargo_color(c))
        };
        draw_rectangle(12.0, y - 12.0, 12.0, 12.0, col);
        draw_text(&txt, 30.0, y, 18.0, col);
        y += 20.0;
    }

    // Timetable.
    let sh = screen_height();
    let mut y = sh - 14.0 - 20.0 * (game.lines.len() as f32 - 1.0);
    draw_text("TIMETABLE", 12.0, y - 22.0, 18.0, GOLD);
    for line in &game.lines {
        let status = match line.state {
            TrainState::Away(t) => format!("due in {:.0}s", t.ceil() + 3.0),
            TrainState::Arriving(_) => "arriving".to_owned(),
            TrainState::Docked(t) => format!("DOCKED - unload! {:.0}s", t.ceil()),
            TrainState::Leaving(_) => "departing".to_owned(),
        };
        let col = if matches!(line.state, TrainState::Docked(_)) {
            GREEN
        } else {
            WHITE
        };
        draw_rectangle(12.0, y - 12.0, 12.0, 12.0, cargo_color(line.cargo));
        draw_text(
            format!("{:<11} {}", TOWNS[line.town].name, status),
            30.0,
            y,
            18.0,
            col,
        );
        y += 20.0;
    }

    let hint = format!(
        "WASD move  E unload  Tab map  P pause  M sound: {}",
        if muted { "OFF" } else { "on" }
    );
    let hw = measure_text(&hint, None, 16, 1.0).width;
    draw_text(
        &hint,
        sw - hw - 12.0,
        sh - 10.0,
        16.0,
        Color::new(1.0, 1.0, 1.0, 0.5),
    );

    if let Some((text, t)) = &fx.banner {
        let a = t.min(1.0);
        text_centered(
            text,
            sw / 2.0 + 2.0,
            112.0,
            36.0,
            Color::new(0.0, 0.0, 0.0, a * 0.7),
        );
        text_centered(text, sw / 2.0, 110.0, 36.0, Color::new(1.0, 0.9, 0.5, a));
    }
}

fn dim() {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
}

/// Shows the three upgrade cards. Returns the picked index.
pub fn level_up(choices: &[Upgrade; 3], game: &Game) -> Option<usize> {
    dim();
    let (sw, sh) = (screen_width(), screen_height());
    text_centered(
        "PROMOTION! Choose an upgrade",
        sw / 2.0,
        sh / 2.0 - 120.0,
        40.0,
        GOLD,
    );
    let cw = 270.0f32.min((sw - 60.0) / 3.0);
    let ch = 160.0;
    let total = cw * 3.0 + 40.0;
    let mut picked = None;
    for (i, u) in choices.iter().enumerate() {
        let r = Rect::new(
            sw / 2.0 - total / 2.0 + i as f32 * (cw + 20.0),
            sh / 2.0 - 80.0,
            cw,
            ch,
        );
        let accent = match u {
            Upgrade::Weapon(c) => cargo_color(*c),
            _ => Color::new(0.6, 0.85, 0.6, 1.0),
        };
        let hover = mouse_in(r);
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            if hover {
                Color::new(0.2, 0.22, 0.25, 0.97)
            } else {
                PANEL
            },
        );
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if hover { 4.0 } else { 2.0 }, accent);
        text_centered(
            &format!("[{}]", i + 1),
            r.x + r.w / 2.0,
            r.y + 28.0,
            22.0,
            GRAY,
        );
        text_centered(&u.title(), r.x + r.w / 2.0, r.y + 62.0, 26.0, accent);
        let sub = match u {
            Upgrade::Weapon(c) => {
                let lvl = game.weapons[c.index()];
                if lvl == 0 {
                    "NEW WEAPON".to_owned()
                } else {
                    format!("Level {} -> {} (max {})", lvl, lvl + 1, MAX_WEAPON_LEVEL)
                }
            }
            _ => String::new(),
        };
        text_centered(&sub, r.x + r.w / 2.0, r.y + 90.0, 18.0, WHITE);
        text_centered(u.blurb(), r.x + r.w / 2.0, r.y + 120.0, 16.0, LIGHTGRAY);
        if hover && is_mouse_button_pressed(MouseButton::Left) {
            picked = Some(i);
        }
    }
    for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .iter()
        .enumerate()
    {
        if is_key_pressed(*k) {
            picked = Some(i);
        }
    }
    picked
}

struct MapLayout {
    center: Vec2,
    radius: f32,
}

fn map_layout() -> MapLayout {
    let (sw, sh) = (screen_width(), screen_height());
    MapLayout {
        center: vec2(sw * 0.36, sh * 0.53),
        radius: (sh * 0.36).min(sw * 0.28),
    }
}

fn town_pos(m: &MapLayout, town: usize) -> Vec2 {
    let t = TOWNS[town];
    m.center + vec2(t.angle.cos(), t.angle.sin()) * (t.dist * m.radius)
}

/// Draws the network map; returns the hovered town.
fn network_map(game: &Game) -> Option<usize> {
    let m = map_layout();
    draw_circle(
        m.center.x,
        m.center.y,
        m.radius + 30.0,
        Color::new(0.14, 0.18, 0.13, 0.95),
    );
    draw_circle_lines(m.center.x, m.center.y, m.radius + 30.0, 2.0, FENCE);
    let mut hovered = None;
    for (i, t) in TOWNS.iter().enumerate() {
        let p = town_pos(&m, i);
        let owned = game.owns_town(i);
        if owned {
            draw_line(m.center.x, m.center.y, p.x, p.y, 5.0, cargo_color(t.cargo));
            let line = game.lines.iter().find(|l| l.town == i);
            if let Some(TrainState::Arriving(prog)) = line.map(|l| l.state) {
                let tp = p.lerp(m.center, prog);
                draw_circle(tp.x, tp.y, 6.0, WHITE);
            }
        } else {
            let mut k = 0.0;
            while k < 1.0 {
                let a = m.center.lerp(p, k);
                let b = m.center.lerp(p, (k + 0.04).min(1.0));
                draw_line(a.x, a.y, b.x, b.y, 2.0, Color::new(1.0, 1.0, 1.0, 0.25));
                k += 0.08;
            }
        }
        let hover = p.distance(vec2(mouse_position().0, mouse_position().1)) < 24.0;
        if hover {
            hovered = Some(i);
        }
        let r = if hover { 17.0 } else { 14.0 };
        draw_circle(p.x, p.y, r + 3.0, if owned { WHITE } else { DARKGRAY });
        draw_circle(p.x, p.y, r, cargo_color(t.cargo));
        text_centered(t.name, p.x, p.y - 24.0, 20.0, WHITE);
        let sub = if owned {
            let lvl = game
                .lines
                .iter()
                .find(|l| l.town == i)
                .map_or(0, |l| l.level);
            format!("{} line Lv {}", t.cargo.name(), lvl + 1)
        } else {
            format!("{}  ${}", t.cargo.name(), t.cost)
        };
        text_centered(
            &sub,
            p.x,
            p.y + 34.0,
            17.0,
            if owned {
                cargo_color(t.cargo)
            } else {
                LIGHTGRAY
            },
        );
    }
    draw_poly(
        m.center.x,
        m.center.y,
        8,
        20.0,
        22.5,
        Color::new(0.55, 0.32, 0.22, 1.0),
    );
    text_centered("DEPOT", m.center.x, m.center.y + 6.0, 16.0, WHITE);
    hovered
}

/// Read-only map shown with Tab during a wave (the game is paused).
pub fn line_map(game: &Game) {
    dim();
    network_map(game);
    let sw = screen_width();
    text_centered(
        "NETWORK MAP (paused) - Tab to close",
        sw / 2.0,
        70.0,
        30.0,
        GOLD,
    );
    text_centered(
        "New lines can be leased between waves",
        sw / 2.0,
        98.0,
        20.0,
        LIGHTGRAY,
    );
}

/// Between-wave shop: lease lines to towns and extend existing ones.
pub fn shop(game: &mut Game) {
    dim();
    let (sw, sh) = (screen_width(), screen_height());
    text_centered(
        &format!("Wave {} cleared! Plan the timetable", game.wave),
        sw / 2.0,
        48.0,
        36.0,
        GOLD,
    );
    text_centered(
        &format!("Cash: ${}   (click a town to lease a line)", game.cash),
        sw / 2.0,
        78.0,
        22.0,
        WHITE,
    );

    if let Some(t) = network_map(game) {
        let town = TOWNS[t];
        if !game.owns_town(t) {
            let (mx, my) = mouse_position();
            let tip = format!(
                "Lease {} line: ${} - unlocks {}",
                town.cargo.name(),
                town.cost,
                town.cargo.weapon()
            );
            let w = measure_text(&tip, None, 18, 1.0).width + 16.0;
            draw_rectangle(mx + 12.0, my + 12.0, w, 26.0, PANEL);
            draw_text(
                &tip,
                mx + 20.0,
                my + 30.0,
                18.0,
                if game.cash >= town.cost { GREEN } else { RED },
            );
            if is_mouse_button_pressed(MouseButton::Left) {
                game.buy_line(t);
            }
        }
    }

    // Right panel: owned lines.
    let px = sw * 0.66;
    let pw = (sw - px - 20.0).max(260.0);
    let mut y = 120.0;
    draw_text("YOUR LINES", px, y, 24.0, GOLD);
    y += 14.0;
    for i in 0..game.lines.len() {
        let line = &game.lines[i];
        let name = TOWNS[line.town].name;
        draw_rectangle(px, y + 4.0, 10.0, 34.0, cargo_color(line.cargo));
        draw_text(
            format!("{} ({})", name, line.cargo.name()),
            px + 16.0,
            y + 18.0,
            19.0,
            WHITE,
        );
        draw_text(
            format!(
                "{} crates every {:.0}s",
                line.capacity(),
                line.interval() + 16.0
            ),
            px + 16.0,
            y + 36.0,
            16.0,
            LIGHTGRAY,
        );
        let r = Rect::new(px + pw - 120.0, y + 4.0, 115.0, 34.0);
        if line.level >= MAX_LINE_LEVEL {
            button(r, "MAX", false, GRAY);
        } else {
            let cost = line.extend_cost();
            if button(r, &format!("Extend ${cost}"), game.cash >= cost, GOLD) {
                game.extend_line(i);
            }
        }
        y += 44.0;
    }
    let r = Rect::new(px, sh - 90.0, pw - 5.0, 54.0);
    if button(
        r,
        &format!("DEPART: Wave {} [Space]", game.wave + 1),
        true,
        GREEN,
    ) {
        game.depart();
    }
}

pub fn paused() {
    dim();
    let (sw, sh) = (screen_width(), screen_height());
    text_centered("PAUSED", sw / 2.0, sh / 2.0, 60.0, WHITE);
    text_centered("P to resume", sw / 2.0, sh / 2.0 + 40.0, 24.0, LIGHTGRAY);
}

pub fn game_over(game: &Game) {
    dim();
    let (sw, sh) = (screen_width(), screen_height());
    let won = game.phase == Phase::Won;
    let (title, col) = if won {
        ("TIMETABLE KEPT - VICTORY!", GOLD)
    } else if game.depot_hp <= 0.0 {
        ("THE DEPOT HAS FALLEN", RED)
    } else {
        ("THE DEPOT MASTER IS DOWN", RED)
    };
    text_centered(title, sw / 2.0, sh / 2.0 - 90.0, 54.0, col);
    let s = &game.stats;
    let lines = [
        format!("Reached wave {} of {}", game.wave, FINAL_WAVE),
        format!("Enemies scrapped: {}", s.kills),
        format!(
            "Trains unloaded: {}   missed: {}",
            s.trains_unloaded, s.trains_missed
        ),
        format!("Crates handled: {}   Lines: {}", s.crates, game.lines.len()),
        format!(
            "Time on shift: {}:{:02}",
            (s.time / 60.0) as u32,
            s.time as u32 % 60
        ),
    ];
    for (i, l) in lines.iter().enumerate() {
        text_centered(l, sw / 2.0, sh / 2.0 - 30.0 + 30.0 * i as f32, 24.0, WHITE);
    }
    text_centered(
        "Press Enter to return to the title",
        sw / 2.0,
        sh / 2.0 + 150.0,
        26.0,
        LIGHTGRAY,
    );
}

pub fn title_screen(t: f32, muted: bool) {
    clear_background(BG);
    let (sw, sh) = (screen_width(), screen_height());
    // A train rolling along the bottom of the title screen.
    let ty = sh - 70.0;
    draw_rectangle(0.0, ty + 14.0, sw, 4.0, RAIL);
    let mut x = 0.0;
    while x < sw {
        draw_rectangle(x, ty + 10.0, 8.0, 14.0, SLEEPER);
        x += 24.0;
    }
    let tx = (t * 140.0) % (sw + 400.0) - 300.0;
    draw_rectangle(
        tx + 200.0,
        ty - 20.0,
        70.0,
        32.0,
        Color::new(0.55, 0.12, 0.1, 1.0),
    );
    draw_rectangle(
        tx + 245.0,
        ty - 34.0,
        25.0,
        46.0,
        Color::new(0.15, 0.15, 0.17, 1.0),
    );
    draw_rectangle(tx + 210.0, ty - 32.0, 10.0, 14.0, DARKGRAY);
    for (k, c) in Cargo::ALL.iter().enumerate() {
        draw_rectangle(
            tx + 140.0 - 66.0 * k as f32,
            ty - 14.0,
            58.0,
            26.0,
            cargo_color(*c),
        );
    }
    for k in 0..4 {
        let puff = (t * 2.0 + k as f32 * 0.5) % 2.0;
        draw_circle(
            tx + 215.0 - puff * 40.0,
            ty - 40.0 - puff * 30.0,
            6.0 + puff * 8.0,
            Color::new(0.9, 0.9, 0.9, 0.5 - puff * 0.25),
        );
    }

    text_centered("DEPOT DEFENDER", sw / 2.0, sh * 0.2, 80.0, GOLD);
    text_centered(
        "Keep the depot alive and the timetable on time.",
        sw / 2.0,
        sh * 0.2 + 44.0,
        24.0,
        LIGHTGRAY,
    );
    let help = [
        "WASD / Arrows - walk (weapons fire automatically)",
        "E - unload a docked train when standing at its platform",
        "Tab - network map     P - pause     M - mute",
        "Mouse / 1-3 - pick upgrades and lease lines between waves",
        "",
        "Steel powers the Rivet Gun, Coal the Firebox Mortar, Goods the Parcel Orbit.",
        "Survive 10 waves and derail the boss trains to win.",
    ];
    for (i, h) in help.iter().enumerate() {
        text_centered(h, sw / 2.0, sh * 0.38 + 28.0 * i as f32, 22.0, WHITE);
    }
    let a = 0.6 + 0.4 * (t * 3.0).sin();
    text_centered(
        "Press any key to start your shift",
        sw / 2.0,
        sh * 0.38 + 230.0,
        32.0,
        Color::new(1.0, 1.0, 1.0, a),
    );
    if muted {
        text_centered(
            "(sound muted - M to unmute)",
            sw / 2.0,
            sh * 0.38 + 262.0,
            18.0,
            GRAY,
        );
    }
}
