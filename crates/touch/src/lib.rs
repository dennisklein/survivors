//! Touch controls for macroquad games.
//!
//! The overlay stays hidden until the first touch. Then:
//! - A touch on the left side of the screen becomes a floating joystick
//!   that holds WASD and the arrow keys.
//! - Buttons on the right side hold their keys, arm a mouse button for
//!   the next tap, or scroll the mouse wheel.
//! - Any other touch acts as the mouse: position, press and release.
//!
//! The overlay shows a control only while the game reads its input. The
//! joystick shows while the game reads a direction key, and a button
//! shows while the game reads its key or mouse button.
//!
//! A game uses the input functions of this crate in place of the
//! macroquad ones, sets its buttons once with [`set_buttons`] and calls
//! [`next_frame`] in place of `macroquad::window::next_frame`.

use macroquad::prelude::{self as mq, Color, KeyCode, MouseButton, TouchPhase, Vec2, vec2};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// What a button does while a finger holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Hold a key.
    Key(KeyCode),
    /// Make the next tap a click of this mouse button.
    Click(MouseButton),
    /// Scroll the mouse wheel by this amount once.
    Wheel(f32),
}

/// One on-screen button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Button {
    pub label: &'static str,
    pub action: Action,
    /// Draw a smaller button, for menu keys such as pause and mute.
    pub small: bool,
}

/// A large button that holds `key`.
pub const fn key(label: &'static str, key: KeyCode) -> Button {
    Button {
        label,
        action: Action::Key(key),
        small: false,
    }
}

/// A small button that holds `key`.
pub const fn small(label: &'static str, key: KeyCode) -> Button {
    Button {
        label,
        action: Action::Key(key),
        small: true,
    }
}

/// A large button that makes the next tap a click of `button`.
pub const fn click(label: &'static str, button: MouseButton) -> Button {
    Button {
        label,
        action: Action::Click(button),
        small: false,
    }
}

/// A small button that scrolls the mouse wheel by `amount`.
pub const fn wheel(label: &'static str, amount: f32) -> Button {
    Button {
        label,
        action: Action::Wheel(amount),
        small: true,
    }
}

/// Keys the joystick holds, with the direction of each.
const STICK_KEYS: [(KeyCode, Vec2); 8] = [
    (KeyCode::W, vec2(0.0, -1.0)),
    (KeyCode::Up, vec2(0.0, -1.0)),
    (KeyCode::S, vec2(0.0, 1.0)),
    (KeyCode::Down, vec2(0.0, 1.0)),
    (KeyCode::A, vec2(-1.0, 0.0)),
    (KeyCode::Left, vec2(-1.0, 0.0)),
    (KeyCode::D, vec2(1.0, 0.0)),
    (KeyCode::Right, vec2(1.0, 0.0)),
];

/// Joystick deflection, as a fraction of its radius, that holds a key.
const DEAD_ZONE: f32 = 0.35;

/// Longest touch, in seconds, and largest drag, in pixels, of a tap on
/// the joystick side.
const TAP_TIME: f64 = 0.5;
const TAP_SLOP: f32 = 24.0;

/// Part of the screen width, from the left, where the joystick starts.
const STICK_ZONE: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Stick,
    Button(usize),
    Pointer,
    Ignored,
}

#[derive(Clone, Copy, Debug)]
struct Stick {
    id: u64,
    start: Vec2,
    origin: Vec2,
    pos: Vec2,
    time: f64,
}

#[derive(Default)]
struct Mouse {
    down: HashSet<MouseButton>,
    pressed: HashSet<MouseButton>,
    released: HashSet<MouseButton>,
}

#[derive(Default)]
struct State {
    buttons: Vec<Button>,
    active: bool,
    started: bool,
    roles: HashMap<u64, Role>,
    stick: Option<Stick>,
    pointer: Option<(u64, MouseButton)>,
    pointer_pos: Vec2,
    tap_release: Option<MouseButton>,
    touch_pos: bool,
    last_mouse: Vec2,
    armed: Option<MouseButton>,
    wheel: f32,
    keys_down: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,
    mouse: Mouse,
    polled_keys: HashSet<KeyCode>,
    polled_mouse: HashSet<MouseButton>,
    polled_wheel: bool,
    stick_enabled: bool,
    layout: Vec<(usize, Vec2, f32)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Sets the buttons of the overlay, from the bottom of the right edge up.
pub fn set_buttons(buttons: &[Button]) {
    with(|s| s.buttons = buttons.to_vec());
}

/// True after a touch, until the next key press on a keyboard.
pub fn is_active() -> bool {
    with(|s| s.active)
}

/// The joystick deflection, with a length of at most 1.
pub fn stick() -> Option<Vec2> {
    with(|s| {
        s.stick
            .map(|st| deflection(st.origin, st.pos, stick_radius()))
    })
}

/// Draws the overlay, ends the frame and reads the touches of the next
/// frame.
pub async fn next_frame() {
    draw();
    mq::next_frame().await;
    update();
}

pub fn is_key_down(key: KeyCode) -> bool {
    with(|s| {
        s.polled_keys.insert(key);
        s.keys_down.contains(&key)
    }) || mq::is_key_down(key)
}

pub fn is_key_pressed(key: KeyCode) -> bool {
    with(|s| {
        s.polled_keys.insert(key);
        s.keys_pressed.contains(&key)
    }) || mq::is_key_pressed(key)
}

pub fn is_key_released(key: KeyCode) -> bool {
    with(|s| {
        s.polled_keys.insert(key);
        s.keys_released.contains(&key)
    }) || mq::is_key_released(key)
}

pub fn is_mouse_button_down(button: MouseButton) -> bool {
    with(|s| {
        s.polled_mouse.insert(button);
        s.mouse.down.contains(&button)
    }) || mq::is_mouse_button_down(button)
}

pub fn is_mouse_button_pressed(button: MouseButton) -> bool {
    with(|s| {
        s.polled_mouse.insert(button);
        s.mouse.pressed.contains(&button)
    }) || mq::is_mouse_button_pressed(button)
}

pub fn is_mouse_button_released(button: MouseButton) -> bool {
    with(|s| {
        s.polled_mouse.insert(button);
        s.mouse.released.contains(&button)
    }) || mq::is_mouse_button_released(button)
}

/// The mouse position, or the position of the last pointer touch.
pub fn mouse_position() -> (f32, f32) {
    with(|s| s.touch_pos.then_some((s.pointer_pos.x, s.pointer_pos.y)))
        .unwrap_or_else(mq::mouse_position)
}

pub fn mouse_wheel() -> (f32, f32) {
    let (x, y) = mq::mouse_wheel();
    let extra = with(|s| {
        s.polled_wheel = true;
        s.wheel
    });
    (x, y + extra)
}

fn stick_radius() -> f32 {
    (mq::screen_width().min(mq::screen_height()) * 0.11).clamp(44.0, 90.0)
}

fn button_radius(small: bool) -> f32 {
    let r = (mq::screen_width().min(mq::screen_height()) * 0.075).clamp(28.0, 50.0);
    if small { r * 0.7 } else { r }
}

/// The deflection of a joystick, with a length of at most 1.
fn deflection(origin: Vec2, pos: Vec2, radius: f32) -> Vec2 {
    ((pos - origin) / radius).clamp_length_max(1.0)
}

/// The keys that a deflection holds.
fn stick_keys(d: Vec2) -> impl Iterator<Item = KeyCode> {
    STICK_KEYS
        .into_iter()
        .filter(move |(_, dir)| d.dot(*dir) > DEAD_ZONE)
        .map(|(k, _)| k)
}

/// Centres and radii of the buttons, from the bottom right corner up.
/// The column wraps to the left when the screen is not high enough.
fn layout(sizes: &[f32], screen: Vec2, margin: f32) -> Vec<(Vec2, f32)> {
    let mut out = Vec::with_capacity(sizes.len());
    let mut x = screen.x - margin;
    let mut y = screen.y - margin;
    let mut col_w: f32 = 0.0;
    for &r in sizes {
        if y - 2.0 * r < margin && col_w > 0.0 {
            x -= col_w + margin;
            y = screen.y - margin;
            col_w = 0.0;
        }
        col_w = col_w.max(2.0 * r);
        out.push((vec2(x - col_w / 2.0, y - r), r));
        y -= 2.0 * r + margin;
    }
    out
}

fn update() {
    with(|s| {
        if !s.started {
            // This crate makes the mouse events from touches itself.
            mq::simulate_mouse_with_touch(false);
            s.started = true;
        }
        s.keys_pressed.clear();
        s.keys_released.clear();
        s.mouse.pressed.clear();
        s.mouse.released.clear();
        s.wheel = 0.0;
        if let Some(b) = s.tap_release.take() {
            s.mouse.down.remove(&b);
            s.mouse.released.insert(b);
        }

        let touches = mq::touches();
        let shown = s.active;
        if !touches.is_empty() {
            s.active = true;
        } else if mq::get_last_key_pressed().is_some() {
            s.active = false;
        }

        let mouse = vec2(mq::mouse_position().0, mq::mouse_position().1);
        if mouse != s.last_mouse && touches.is_empty() {
            s.touch_pos = false;
        }
        s.last_mouse = mouse;

        let mut held_keys = HashSet::new();
        for t in &touches {
            let ended = matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled);
            let role = match s.roles.get(&t.id) {
                Some(r) => *r,
                None => {
                    let r = s.begin(t.id, t.position, shown);
                    s.roles.insert(t.id, r);
                    r
                }
            };
            match role {
                Role::Stick => {
                    if let Some(st) = s.stick.as_mut() {
                        st.pos = t.position;
                        // Drag the base along when the finger goes past
                        // the rim.
                        let r = stick_radius();
                        let off = st.pos - st.origin;
                        if off.length() > r {
                            st.origin = st.pos - off.normalize() * r;
                        }
                    }
                    if ended {
                        // A short touch without a drag is a tap, so that
                        // menus on the left side still take clicks.
                        if let Some(st) = s.stick.take()
                            && st.start.distance(st.pos) < TAP_SLOP
                            && mq::get_time() - st.time < TAP_TIME
                        {
                            s.tap(st.start);
                        }
                    }
                }
                Role::Button(i) => {
                    if let Some(Action::Key(k)) = s.buttons.get(i).map(|b| b.action)
                        && !ended
                    {
                        held_keys.insert(k);
                    }
                }
                Role::Pointer => {
                    s.pointer_pos = t.position;
                    s.touch_pos = true;
                    if ended && let Some((_, b)) = s.pointer.take() {
                        s.mouse.down.remove(&b);
                        s.mouse.released.insert(b);
                    }
                }
                Role::Ignored => {}
            }
            if ended {
                s.roles.remove(&t.id);
            }
        }
        // Forget touches that macroquad no longer reports.
        let live: HashSet<u64> = touches.iter().map(|t| t.id).collect();
        s.roles.retain(|id, _| live.contains(id));
        if s.stick.is_some_and(|st| !live.contains(&st.id)) {
            s.stick = None;
        }
        if let Some((id, b)) = s.pointer
            && !live.contains(&id)
        {
            s.pointer = None;
            s.mouse.down.remove(&b);
            s.mouse.released.insert(b);
        }

        if let Some(st) = s.stick {
            held_keys.extend(stick_keys(deflection(st.origin, st.pos, stick_radius())));
        }
        for k in held_keys.difference(&s.keys_down) {
            s.keys_pressed.insert(*k);
        }
        for k in s.keys_down.difference(&held_keys) {
            s.keys_released.insert(*k);
        }
        s.keys_down = held_keys;

        s.polled_keys.clear();
        s.polled_mouse.clear();
        s.polled_wheel = false;
    });
}

impl State {
    /// Gives a new touch its role and starts its action. While the
    /// overlay is hidden, every touch acts as the mouse.
    fn begin(&mut self, id: u64, pos: Vec2, shown: bool) -> Role {
        if shown
            && let Some(&(i, _, _)) = self
                .layout
                .iter()
                .find(|(_, c, r)| c.distance(pos) <= *r * 1.15)
        {
            match self.buttons[i].action {
                Action::Key(_) => {}
                Action::Click(b) => {
                    self.armed = if self.armed == Some(b) { None } else { Some(b) };
                }
                Action::Wheel(w) => self.wheel += w,
            }
            return Role::Button(i);
        }
        if shown
            && self.stick_enabled
            && self.stick.is_none()
            && pos.x < mq::screen_width() * STICK_ZONE
        {
            self.stick = Some(Stick {
                id,
                start: pos,
                origin: pos,
                pos,
                time: mq::get_time(),
            });
            return Role::Stick;
        }
        if self.pointer.is_none() {
            let b = self.armed.take().unwrap_or(MouseButton::Left);
            self.pointer = Some((id, b));
            self.pointer_pos = pos;
            self.touch_pos = true;
            self.mouse.down.insert(b);
            self.mouse.pressed.insert(b);
            return Role::Pointer;
        }
        Role::Ignored
    }

    /// Clicks at `pos` now and releases the click in the next frame.
    fn tap(&mut self, pos: Vec2) {
        if self.pointer.is_some() {
            return;
        }
        let b = self.armed.take().unwrap_or(MouseButton::Left);
        self.pointer_pos = pos;
        self.touch_pos = true;
        self.mouse.down.insert(b);
        self.mouse.pressed.insert(b);
        self.tap_release = Some(b);
    }

    fn shows(&self, b: &Button) -> bool {
        match b.action {
            Action::Key(k) => self.polled_keys.contains(&k),
            Action::Click(m) => self.polled_mouse.contains(&m),
            Action::Wheel(_) => self.polled_wheel,
        }
    }
}

const FILL: Color = Color::new(1.0, 1.0, 1.0, 0.14);
const FILL_HELD: Color = Color::new(1.0, 1.0, 1.0, 0.32);
const FILL_ARMED: Color = Color::new(0.94, 0.71, 0.16, 0.45);
const RIM: Color = Color::new(1.0, 1.0, 1.0, 0.45);
const FONT_SIZE: u16 = 32;
const TEXT: Color = Color::new(1.0, 1.0, 1.0, 0.85);

fn draw() {
    with(|s| {
        s.stick_enabled = STICK_KEYS.iter().any(|(k, _)| s.polled_keys.contains(k));
        if !s.stick_enabled {
            s.stick = None;
        }

        let shown: Vec<usize> = (0..s.buttons.len())
            .filter(|&i| s.shows(&s.buttons[i]))
            .collect();
        let sizes: Vec<f32> = shown
            .iter()
            .map(|&i| button_radius(s.buttons[i].small))
            .collect();
        let screen = vec2(mq::screen_width(), mq::screen_height());
        let margin = (screen.x.min(screen.y) * 0.03).clamp(10.0, 24.0);
        s.layout = shown
            .iter()
            .zip(layout(&sizes, screen, margin))
            .map(|(&i, (c, r))| (i, c, r))
            .collect();

        if !s.active {
            return;
        }
        mq::push_camera_state();
        mq::set_default_camera();

        let r = stick_radius();
        if let Some(st) = s.stick {
            mq::draw_circle(st.origin.x, st.origin.y, r, FILL);
            mq::draw_circle_lines(st.origin.x, st.origin.y, r, 2.0, RIM);
            mq::draw_circle(st.pos.x, st.pos.y, r * 0.45, FILL_HELD);
        } else if s.stick_enabled {
            let c = vec2(margin + r, screen.y - margin - r);
            mq::draw_circle_lines(c.x, c.y, r, 2.0, Color::new(1.0, 1.0, 1.0, 0.2));
            mq::draw_circle(c.x, c.y, r * 0.45, Color::new(1.0, 1.0, 1.0, 0.1));
        }

        let held: HashSet<usize> = s
            .roles
            .values()
            .filter_map(|r| match r {
                Role::Button(i) => Some(*i),
                _ => None,
            })
            .collect();
        for &(i, c, br) in &s.layout {
            let b = &s.buttons[i];
            let fill = if matches!(b.action, Action::Click(m) if s.armed == Some(m)) {
                FILL_ARMED
            } else if held.contains(&i) {
                FILL_HELD
            } else {
                FILL
            };
            mq::draw_circle(c.x, c.y, br, fill);
            mq::draw_circle_lines(c.x, c.y, br, 2.0, RIM);
            // One font size with a scale, so the glyph cache stays small.
            let dims = mq::measure_text(b.label, None, FONT_SIZE, 1.0);
            let scale = ((br * 0.62).max(14.0) / FONT_SIZE as f32).min(br * 1.8 / dims.width);
            mq::draw_text_ex(
                b.label,
                c.x - dims.width * scale / 2.0,
                c.y + dims.offset_y * scale / 2.0,
                mq::TextParams {
                    font_size: FONT_SIZE,
                    font_scale: scale,
                    color: TEXT,
                    ..Default::default()
                },
            );
        }
        mq::pop_camera_state();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(d: Vec2) -> HashSet<KeyCode> {
        stick_keys(d).collect()
    }

    #[test]
    fn centre_holds_no_key() {
        assert!(keys(vec2(0.1, -0.2)).is_empty());
    }

    #[test]
    fn right_holds_d_and_right() {
        assert_eq!(
            keys(vec2(1.0, 0.0)),
            HashSet::from([KeyCode::D, KeyCode::Right])
        );
    }

    #[test]
    fn diagonal_holds_two_directions() {
        let d = vec2(-1.0, -1.0).normalize();
        assert_eq!(
            keys(d),
            HashSet::from([KeyCode::A, KeyCode::Left, KeyCode::W, KeyCode::Up])
        );
    }

    #[test]
    fn deflection_is_clamped() {
        let d = deflection(Vec2::ZERO, vec2(300.0, 0.0), 50.0);
        assert_eq!(d, vec2(1.0, 0.0));
    }

    #[test]
    fn layout_stacks_up_and_wraps_left() {
        let out = layout(&[40.0; 4], vec2(800.0, 250.0), 10.0);
        // Two buttons fit in the first column.
        assert_eq!(out[0].0, vec2(750.0, 200.0));
        assert_eq!(out[1].0, vec2(750.0, 110.0));
        // The third starts a new column on the left.
        assert_eq!(out[2].0, vec2(660.0, 200.0));
        assert_eq!(out[3].0, vec2(660.0, 110.0));
    }
}
