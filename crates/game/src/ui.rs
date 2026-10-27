//! Screen-space UI: level-up cards.

use macroquad::prelude::*;
use sim::world::World;

pub const PANEL: Color = Color::new(0.10, 0.12, 0.10, 0.92);
pub const PANEL_HOVER: Color = Color::new(0.18, 0.22, 0.17, 0.95);
pub const BORDER: Color = Color::new(0.85, 0.70, 0.35, 1.0);
pub const TEXT: Color = Color::new(0.95, 0.93, 0.88, 1.0);
pub const MUTED: Color = Color::new(0.70, 0.70, 0.65, 1.0);
pub const DIM: Color = Color::new(0.0, 0.0, 0.0, 0.55);

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
