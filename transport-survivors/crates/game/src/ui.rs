//! Screen-space UI: HUD, level-up cards and screens.

use macroquad::prelude::*;
use sim::cards::Upgrade;
use sim::world::{BOSS_TIME, World};
use touch::{is_key_pressed, is_mouse_button_pressed, mouse_position};

pub const PANEL: Color = Color::new(0.10, 0.12, 0.10, 0.92);
pub const PANEL_HOVER: Color = Color::new(0.18, 0.22, 0.17, 0.95);
pub const BORDER: Color = Color::new(0.85, 0.70, 0.35, 1.0);
pub const TEXT: Color = Color::new(0.95, 0.93, 0.88, 1.0);
pub const MUTED: Color = Color::new(0.70, 0.70, 0.65, 1.0);
pub const DIM: Color = Color::new(0.0, 0.0, 0.0, 0.55);
const BAR_BACK: Color = Color::new(0.0, 0.0, 0.0, 0.55);
const HP: Color = Color::new(0.85, 0.22, 0.20, 1.0);
const XP: Color = Color::new(0.35, 0.70, 1.0, 1.0);
const BOSS: Color = Color::new(0.65, 0.35, 0.95, 1.0);
const GOLD: Color = Color::new(0.95, 0.80, 0.35, 1.0);

/// UI scale: 1.0 at 720 pixels screen height.
pub fn scale() -> f32 {
    screen_height() / 720.0
}

pub fn text_centred(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    let m = measure_text(text, None, size as u16, 1.0);
    draw_text(text, cx - m.width / 2.0, y, size, color);
}

/// Draws text in lines no wider than `width`. Returns the y after the last line.
pub fn text_wrapped(text: &str, x: f32, mut y: f32, width: f32, size: f32, color: Color) -> f32 {
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if measure_text(&next, None, size as u16, 1.0).width > width && !line.is_empty() {
            draw_text(&line, x, y, size, color);
            y += size * 1.2;
            line = word.to_owned();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        draw_text(&line, x, y, size, color);
        y += size * 1.2;
    }
    y
}

fn card_rects() -> [Rect; 3] {
    let k = scale();
    let (w, h, gap) = (280.0 * k, 180.0 * k, 24.0 * k);
    let x0 = (screen_width() - 3.0 * w - 2.0 * gap) / 2.0;
    let y = (screen_height() - h) / 2.0;
    std::array::from_fn(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h))
}

/// Draws the level-up cards. Returns the card that the player picked.
pub fn level_up(world: &World, cards: &[sim::cards::Card; 3]) -> Option<usize> {
    let k = scale();
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), DIM);
    let rects = card_rects();
    text_centred(
        &format!("Level {}: pick an upgrade", world.level),
        screen_width() / 2.0,
        rects[0].y - 28.0 * k,
        36.0 * k,
        TEXT,
    );
    let mouse: Vec2 = mouse_position().into();
    let mut picked = None;
    for (i, (card, r)) in cards.iter().zip(rects).enumerate() {
        let hover = r.contains(mouse);
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { PANEL_HOVER } else { PANEL });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * k, BORDER);
        let (title, text) = card.describe(&world.wagons, &world.upgrades);
        let pad = 16.0 * k;
        draw_text(
            format!("{}", i + 1),
            r.x + pad,
            r.y + 34.0 * k,
            30.0 * k,
            BORDER,
        );
        let y = text_wrapped(
            &title,
            r.x + pad + 28.0 * k,
            r.y + 34.0 * k,
            r.w - 2.0 * pad - 28.0 * k,
            26.0 * k,
            TEXT,
        );
        text_wrapped(
            text,
            r.x + pad,
            y + 14.0 * k,
            r.w - 2.0 * pad,
            22.0 * k,
            MUTED,
        );
        if hover && is_mouse_button_pressed(MouseButton::Left) {
            picked = Some(i);
        }
    }
    for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .into_iter()
        .enumerate()
    {
        if is_key_pressed(key) {
            picked = Some(i);
        }
    }
    picked
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, BAR_BACK);
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
}

fn clock(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{:02}:{:02}", t / 60, t % 60)
}

/// The run HUD: XP bar, HP bar, timer, kills, boss bar and the train.
pub fn hud(world: &World) {
    let k = scale();
    let sw = screen_width();
    // XP along the top edge.
    let xp = world.xp as f32 / world.xp_needed() as f32;
    bar(0.0, 0.0, sw, 10.0 * k, xp, XP);
    draw_text(
        format!("Lv {}", world.level),
        12.0 * k,
        36.0 * k,
        26.0 * k,
        TEXT,
    );
    // HP.
    let (hx, hy, hw, hh) = (12.0 * k, 46.0 * k, 220.0 * k, 18.0 * k);
    bar(hx, hy, hw, hh, world.hp / world.max_hp, HP);
    draw_text(
        format!("{:.0} / {:.0}", world.hp, world.max_hp),
        hx + 8.0 * k,
        hy + hh - 4.0 * k,
        18.0 * k,
        TEXT,
    );
    // Clock: counts up to the boss.
    let left = BOSS_TIME - world.time;
    let label = if world.boss_sent {
        "BOSS".to_owned()
    } else {
        clock(world.time)
    };
    text_centred(
        &label,
        sw / 2.0,
        44.0 * k,
        36.0 * k,
        if left < 30.0 { BOSS } else { TEXT },
    );
    let kills = format!("{} kills", world.kills);
    let m = measure_text(&kills, None, (24.0 * k) as u16, 1.0);
    draw_text(&kills, sw - m.width - 14.0 * k, 36.0 * k, 24.0 * k, TEXT);
    if let Some(b) = world.boss() {
        let w = sw * 0.5;
        bar((sw - w) / 2.0, 56.0 * k, w, 14.0 * k, b.hp / b.max_hp, BOSS);
        text_centred("Large UFO", sw / 2.0, 90.0 * k, 22.0 * k, BOSS);
    }
    // The train: wagons and engine upgrades.
    let mut y = screen_height() - 14.0 * k;
    let size = 20.0 * k;
    for u in Upgrade::ALL.iter().rev() {
        let lv = world.upgrades.level(*u);
        if lv > 0 {
            draw_text(
                format!("{} {}", u.name(), stars(lv)),
                12.0 * k,
                y,
                size,
                MUTED,
            );
            y -= size * 1.1;
        }
    }
    for w in world.wagons.iter().rev() {
        draw_text(
            format!("{} {}", w.kind.name(), stars(w.level)),
            12.0 * k,
            y,
            size,
            TEXT,
        );
        y -= size * 1.1;
    }
    let hint = if world.train.is_building() {
        "building"
    } else {
        "on track"
    };
    let speed = format!("{:.1} tiles/s  {hint}", world.train.speed());
    let m = measure_text(&speed, None, size as u16, 1.0);
    draw_text(
        &speed,
        sw - m.width - 14.0 * k,
        screen_height() - 14.0 * k,
        size,
        MUTED,
    );
}

fn stars(level: u8) -> String {
    (0..5).map(|i| if i < level { '*' } else { '.' }).collect()
}

/// A red edge when the train takes damage. `a` fades from 1 to 0.
pub fn hurt(a: f32) {
    let (w, h, t) = (screen_width(), screen_height(), 18.0 * scale());
    let c = Color::new(0.9, 0.1, 0.1, 0.45 * a);
    draw_rectangle(0.0, 0.0, w, t, c);
    draw_rectangle(0.0, h - t, w, t, c);
    draw_rectangle(0.0, t, t, h - 2.0 * t, c);
    draw_rectangle(w - t, t, t, h - 2.0 * t, c);
}

fn panel(lines: &[(&str, f32, Color)]) {
    let k = scale();
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), DIM);
    let total: f32 = lines.iter().map(|(_, s, _)| s * 1.5 * k).sum();
    let width = lines
        .iter()
        .map(|(t, s, _)| measure_text(t, None, (s * k) as u16, 1.0).width)
        .fold(0.0, f32::max);
    let pad = 32.0 * k;
    let mut y = (screen_height() - total) / 2.0;
    let r = Rect::new(
        (screen_width() - width) / 2.0 - pad,
        y - pad / 2.0,
        width + 2.0 * pad,
        total + 1.5 * pad,
    );
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * k, BORDER);
    for (text, size, color) in lines {
        y += size * 1.5 * k;
        text_centred(text, screen_width() / 2.0, y, size * k, *color);
    }
}

pub fn title() {
    panel(&[
        ("TRANSPORT SURVIVORS", 64.0, GOLD),
        (
            "Your train lays its own track. Wagons are weapons.",
            24.0,
            TEXT,
        ),
        ("Survive 10 minutes, then beat the Large UFO.", 24.0, TEXT),
        ("", 16.0, TEXT),
        (
            "A / D or arrow keys: turn left or right at the next tile",
            22.0,
            MUTED,
        ),
        (
            "No key: follow the track.   S or down: reverse",
            22.0,
            MUTED,
        ),
        ("1 2 3: pick a card.   Esc or P: pause", 22.0, MUTED),
        ("", 16.0, TEXT),
        ("Press Enter or click to start", 30.0, GOLD),
    ]);
}

pub fn paused() {
    panel(&[
        ("PAUSED", 56.0, GOLD),
        ("Esc or P: resume", 24.0, TEXT),
        ("R: new run", 24.0, TEXT),
    ]);
}

pub fn game_over(world: &World, won: bool) {
    let stats = format!(
        "Time {}   Level {}   Kills {}",
        clock(world.time),
        world.level,
        world.kills
    );
    let (head, sub) = if won {
        ("YOU WIN", "The Large UFO is down.")
    } else {
        ("TRAIN CRASHED", "The train has no HP left.")
    };
    panel(&[
        (head, 64.0, if won { GOLD } else { HP }),
        (sub, 26.0, TEXT),
        (&stats, 26.0, TEXT),
        ("", 16.0, TEXT),
        ("Press Enter to play again", 30.0, GOLD),
    ]);
}
