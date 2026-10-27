//! Screen-space UI: hints, labels and messages.

use macroquad::prelude::*;
use sim::cards::Card;
use sim::world::World;

pub const PANEL: Color = Color::new(0.10, 0.12, 0.14, 0.92);
pub const PANEL_HOVER: Color = Color::new(0.16, 0.20, 0.26, 0.95);
pub const BORDER: Color = Color::new(0.45, 0.62, 0.95, 1.0);
pub const DIM: Color = Color::new(0.0, 0.0, 0.0, 0.55);
pub const TEXT: Color = Color::new(0.95, 0.94, 0.90, 1.0);
pub const MUTED: Color = Color::new(0.70, 0.71, 0.70, 1.0);
pub const BAD: Color = Color::new(0.95, 0.35, 0.30, 1.0);

/// UI scale: 1.0 at 800 pixels screen height.
pub fn scale() -> f32 {
    screen_height() / 800.0
}

pub fn text_centred(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    let m = measure_text(text, None, size as u16, 1.0);
    draw_text(text, cx - m.width / 2.0, y, size, color);
}

/// A line of help at the bottom of the screen.
pub fn hint(text: &str) {
    let k = scale();
    let h = 30.0 * k;
    draw_rectangle(0.0, screen_height() - h, screen_width(), h, PANEL);
    text_centred(
        text,
        screen_width() / 2.0,
        screen_height() - 9.0 * k,
        20.0 * k,
        MUTED,
    );
}

/// A small label next to the mouse.
pub fn cursor_label(text: &str) {
    let k = scale();
    let (mx, my) = mouse_position();
    let size = 20.0 * k;
    let m = measure_text(text, None, size as u16, 1.0);
    let (x, y) = (mx + 16.0 * k, my + 28.0 * k);
    draw_rectangle(
        x - 6.0 * k,
        y - size,
        m.width + 12.0 * k,
        size * 1.35,
        PANEL,
    );
    draw_text(text, x, y, size, TEXT);
}

/// A short message above the hint line.
pub fn toast(text: &str) {
    let k = scale();
    let size = 26.0 * k;
    let m = measure_text(text, None, size as u16, 1.0);
    let y = screen_height() - 60.0 * k;
    draw_rectangle(
        (screen_width() - m.width) / 2.0 - 12.0 * k,
        y - size,
        m.width + 24.0 * k,
        size * 1.4,
        PANEL,
    );
    text_centred(text, screen_width() / 2.0, y, size, BAD);
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

const BAR_BACK: Color = Color::new(0.0, 0.0, 0.0, 0.55);
const HP: Color = Color::new(0.85, 0.25, 0.22, 1.0);
const XP: Color = Color::new(0.35, 0.65, 1.0, 1.0);
const GOLD: Color = Color::new(0.95, 0.80, 0.35, 1.0);
const BOSS: Color = Color::new(0.65, 0.35, 0.95, 1.0);

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, BAR_BACK);
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
}

pub fn clock(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{:02}:{:02}", t / 60, t % 60)
}

/// The top bar: money, town HP, level and XP, time and game speed.
pub fn top_bar(world: &World, speed: &str) {
    let k = scale();
    let h = 46.0 * k;
    let sw = screen_width();
    draw_rectangle(0.0, 0.0, sw, h, PANEL);
    let size = 26.0 * k;
    let y = 31.0 * k;
    let mut x = 16.0 * k;
    // Money.
    let money = format!("${:.0}", world.money.floor());
    draw_text(&money, x, y, size, GOLD);
    x += 130.0 * k;
    // Town HP.
    draw_text("Town", x, y, size * 0.8, MUTED);
    x += 52.0 * k;
    let w = 170.0 * k;
    bar(x, 14.0 * k, w, 20.0 * k, world.town_hp / world.town_max, HP);
    draw_text(
        format!("{:.0}/{:.0}", world.town_hp.ceil(), world.town_max),
        x + 8.0 * k,
        y - 2.0 * k,
        size * 0.75,
        TEXT,
    );
    x += w + 26.0 * k;
    // Level and XP.
    draw_text(format!("Lv {}", world.level), x, y, size, TEXT);
    x += 70.0 * k;
    let w = 170.0 * k;
    bar(
        x,
        14.0 * k,
        w,
        20.0 * k,
        world.xp as f32 / world.xp_needed() as f32,
        XP,
    );
    draw_text(
        format!("{}/{}", world.xp, world.xp_needed()),
        x + 8.0 * k,
        y - 2.0 * k,
        size * 0.75,
        TEXT,
    );
    x += w + 26.0 * k;
    draw_text(
        format!("Lines {}/{}", world.lines.len(), sim::world::MAX_LINES),
        x,
        y,
        size * 0.8,
        MUTED,
    );
    // Time and speed on the right.
    let label = if world.boss_sent {
        "BOSS".to_owned()
    } else {
        format!("{} / 15:00", clock(world.time))
    };
    let right = format!("{label}   {speed}");
    let m = measure_text(&right, None, size as u16, 1.0);
    let color = if world.time > sim::world::BOSS_TIME - 30.0 {
        BOSS
    } else {
        TEXT
    };
    draw_text(&right, sw - m.width - 16.0 * k, y, size, color);
    if let Some(b) = world.boss() {
        let w = sw * 0.4;
        let x = (sw - w) / 2.0;
        bar(x, h + 8.0 * k, w, 14.0 * k, b.hp / b.max_hp, BOSS);
        text_centred("Large UFO", sw / 2.0, h + 42.0 * k, 22.0 * k, BOSS);
    }
}

/// The train panel of a line. Returns its rectangle and true if the
/// remove button was clicked.
pub fn train_panel(world: &World, i: usize) -> (Rect, bool) {
    let k = scale();
    let l = &world.lines[i];
    let r = Rect::new(screen_width() - 300.0 * k, 60.0 * k, 284.0 * k, 250.0 * k);
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0 * k, BORDER);
    let pad = 14.0 * k;
    let size = 22.0 * k;
    let mut y = r.y + 30.0 * k;
    let name = world.stations[l.a].name();
    draw_text(
        format!("Line {}: {name}", i + 1),
        r.x + pad,
        y,
        26.0 * k,
        TEXT,
    );
    y += 30.0 * k;
    let cargo = world.stations[l.a]
        .industry()
        .map_or("cargo", |c| c.cargo());
    let t = &l.train;
    let minutes = ((world.time - l.built) / 60.0).max(0.25);
    let status = if t.is_wrecked() {
        "wrecked".to_owned()
    } else {
        format!("{:.0}/{:.0} HP", t.hp, world.train_hp())
    };
    let lines = [
        format!("{} {cargo} on board", t.cargo),
        format!("Train: {status}"),
        format!("Route: {:.0} tiles", l.route.len),
        format!("Delivered: {} units", l.delivered),
        format!("Income: ${:.0}/min", l.earned / minutes),
    ];
    for text in &lines {
        draw_text(text, r.x + pad, y, size, MUTED);
        y += size * 1.25;
    }
    let refund = (l.cost * sim::world::REFUND).floor();
    let b = Rect::new(r.x + pad, r.y + r.h - 46.0 * k, r.w - 2.0 * pad, 34.0 * k);
    let hover = b.contains(mouse_position().into());
    draw_rectangle(b.x, b.y, b.w, b.h, if hover { BAD } else { PANEL_HOVER });
    text_centred(
        &format!("Remove line (+${refund:.0})"),
        b.x + b.w / 2.0,
        b.y + 23.0 * k,
        size,
        TEXT,
    );
    (r, hover && is_mouse_button_pressed(MouseButton::Left))
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
        ("RAIL SIEGE", 64.0, GOLD),
        (
            "Build lines with the mouse. Trains carry cargo to the town",
            24.0,
            TEXT,
        ),
        ("and shoot the horde on the way.", 24.0, TEXT),
        (
            "Keep the town alive for 15 minutes, then beat the Large UFO.",
            24.0,
            TEXT,
        ),
        ("", 14.0, TEXT),
        (
            "Click a station, then a second station: build a line",
            22.0,
            MUTED,
        ),
        (
            "Click grass while building: waypoint.  Esc or right click: cancel",
            22.0,
            MUTED,
        ),
        (
            "Click broken track: repair.  Click a train: train panel",
            22.0,
            MUTED,
        ),
        (
            "WASD or arrows: move.  Wheel or + -: zoom.  Space or P: pause",
            22.0,
            MUTED,
        ),
        ("1 2 3 or click: pick a card", 22.0, MUTED),
        ("", 14.0, TEXT),
        ("Click or press Enter to start", 30.0, GOLD),
    ]);
}

/// A label under the top bar. The map stays visible, so the player can
/// build while paused.
pub fn paused() {
    let k = scale();
    let text = "PAUSED   Space or P: resume   R: new run";
    let size = 26.0 * k;
    let m = measure_text(text, None, size as u16, 1.0);
    let y = 90.0 * k;
    draw_rectangle(
        (screen_width() - m.width) / 2.0 - 14.0 * k,
        y - size,
        m.width + 28.0 * k,
        size * 1.45,
        PANEL,
    );
    text_centred(text, screen_width() / 2.0, y, size, GOLD);
}

pub fn end_screen(world: &World, won: bool) {
    let delivered: u32 = world.lines.iter().map(|l| l.delivered).sum();
    let stats = format!(
        "Time {}   Level {}   Kills {}   Lines {}",
        clock(world.time),
        world.level,
        world.kills,
        world.lines.len()
    );
    let cargo = format!("Cargo on current lines: {delivered} units");
    let (head, sub) = if won {
        ("YOU WIN", "The Large UFO is down. The town stands.")
    } else {
        ("THE TOWN FELL", "The horde broke through.")
    };
    panel(&[
        (head, 64.0, if won { GOLD } else { HP }),
        (sub, 26.0, TEXT),
        (&stats, 24.0, TEXT),
        (&cargo, 24.0, MUTED),
        ("", 14.0, TEXT),
        ("Press Enter to play again", 30.0, GOLD),
    ]);
}

fn card_rects() -> [Rect; 3] {
    let k = scale();
    let (w, h, gap) = (280.0 * k, 180.0 * k, 24.0 * k);
    let x0 = (screen_width() - 3.0 * w - 2.0 * gap) / 2.0;
    let y = (screen_height() - h) / 2.0;
    std::array::from_fn(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h))
}

/// Draws the level-up cards. Returns the card that the player picked.
pub fn cards(world: &World, cards: &[Card; 3]) -> Option<usize> {
    let k = scale();
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), DIM);
    let rects = card_rects();
    text_centred(
        &format!("Level {}: pick a card", world.level),
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
        let (title, text) = card.describe(&world.levels);
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
