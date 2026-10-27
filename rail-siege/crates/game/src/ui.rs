//! Screen-space UI: hints, labels and messages.

use macroquad::prelude::*;

pub const PANEL: Color = Color::new(0.10, 0.12, 0.14, 0.92);
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

/// Large text in the middle of the top of the map.
pub fn banner(text: &str) {
    let k = scale();
    text_centred(text, screen_width() / 2.0, 110.0 * k, 40.0 * k, TEXT);
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
