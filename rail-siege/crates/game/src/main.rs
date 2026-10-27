mod render;
mod ui;

use macroquad::prelude::*;
use sim::math::V2;
use sim::route::Route;
use sim::station::{MAP_H, MAP_W};
use sim::track::Tile;
use sim::world::{Command, DT, World};

/// At most this many simulation steps per frame, so a slow frame does not freeze the game.
const MAX_STEPS: u32 = 5;
/// Game speed while a line is started.
const BUILD_SPEED: f32 = 0.25;
const PAN_SPEED: f32 = 18.0;
const MAX_WAYPOINTS: usize = 8;

fn window_conf() -> Conf {
    Conf {
        window_title: "Rail Siege".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

/// Camera with two zoom levels. Far shows the whole map.
struct View {
    centre: V2,
    near: bool,
}

impl View {
    fn new() -> Self {
        Self {
            centre: V2::new(MAP_W as f32 / 2.0, MAP_H as f32 / 2.0),
            near: false,
        }
    }

    /// Tiles across the screen.
    fn tiles_across(&self) -> f32 {
        let aspect = screen_width() / screen_height();
        // Room for the top bar above the map.
        let fit = (MAP_W as f32 + 1.0).max((MAP_H as f32 + 5.0) * aspect);
        if self.near { fit / 2.0 } else { fit }
    }

    fn camera(&self) -> Camera2D {
        let w = self.tiles_across();
        let h = w * screen_height() / screen_width();
        // In the far view the map sits a little lower, under the top bar.
        let shift = if self.near { 0.0 } else { -0.6 };
        Camera2D {
            target: vec2(self.centre.x, self.centre.y + shift),
            zoom: vec2(2.0 / w, 2.0 / h),
            ..Default::default()
        }
    }

    fn update(&mut self, dt: f32) {
        let mut d = V2::ZERO;
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            d.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            d.x += 1.0;
        }
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            d.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            d.y += 1.0;
        }
        let wheel = mouse_wheel().1;
        if wheel > 0.0 || is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
            if !self.near {
                // Zoom in toward the mouse.
                let m = self.camera().screen_to_world(mouse_position().into());
                self.centre = V2::new(m.x, m.y);
            }
            self.near = true;
        }
        if wheel < 0.0 || is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
            self.near = false;
        }
        if self.near {
            self.centre += d * (PAN_SPEED * dt);
            let w = self.tiles_across() / 2.0;
            let h = w * screen_height() / screen_width();
            let clamp = |c: f32, half: f32, size: f32| {
                if half * 2.0 >= size {
                    size / 2.0
                } else {
                    c.clamp(half, size - half)
                }
            };
            self.centre = V2::new(
                clamp(self.centre.x, w, MAP_W as f32),
                clamp(self.centre.y, h, MAP_H as f32),
            );
        } else {
            self.centre = V2::new(MAP_W as f32 / 2.0, MAP_H as f32 / 2.0);
        }
    }
}

/// A started line: the first station and the waypoints so far.
struct Draft {
    from: usize,
    via: Vec<Tile>,
    /// Cached ghost route and the end it was found for.
    ghost: Option<Route>,
    ghost_end: Option<Tile>,
}

struct Game {
    world: World,
    view: View,
    draft: Option<Draft>,
    paused: bool,
    acc: f32,
    toast: Option<(String, f32)>,
}

impl Game {
    fn new() -> Self {
        Self {
            world: World::new((macroquad::miniquad::date::now() * 1000.0) as u64),
            view: View::new(),
            draft: None,
            paused: false,
            acc: 0.0,
            toast: None,
        }
    }

    fn say(&mut self, text: &str) {
        self.toast = Some((text.to_owned(), 2.0));
    }

    fn speed(&self) -> f32 {
        if self.paused {
            0.0
        } else if self.draft.is_some() {
            BUILD_SPEED
        } else {
            1.0
        }
    }

    fn mouse_tile(&self) -> Option<Tile> {
        let m = self.view.camera().screen_to_world(mouse_position().into());
        let t = (m.x.floor() as i32, m.y.floor() as i32);
        self.world.map.in_bounds(t.0, t.1).then_some(t)
    }

    fn click(&mut self, tile: Tile) {
        let station = self.world.station_at(tile);
        match (&mut self.draft, station) {
            (None, Some(s)) => {
                self.draft = Some(Draft {
                    from: s,
                    via: Vec::new(),
                    ghost: None,
                    ghost_end: None,
                });
            }
            (None, None) => {}
            (Some(d), Some(s)) if s == d.from => self.draft = None,
            (Some(d), Some(s)) => {
                let cmd = Command::Build {
                    a: d.from,
                    b: s,
                    via: d.via.clone(),
                };
                match self.world.apply(&cmd) {
                    Ok(()) => self.draft = None,
                    Err(e) => self.say(e.text()),
                }
            }
            (Some(d), None) => {
                if !self.world.map.passable(tile.0, tile.1) {
                    self.say("Track cannot go there");
                } else if d.via.len() >= MAX_WAYPOINTS {
                    self.say("Too many waypoints");
                } else if !d.via.contains(&tile) {
                    d.via.push(tile);
                    d.ghost_end = None;
                }
            }
        }
    }

    /// Finds the ghost route to the tile under the mouse, if it moved.
    fn update_ghost(&mut self, hover: Option<Tile>) {
        let Some(d) = &mut self.draft else { return };
        if hover == d.ghost_end {
            return;
        }
        d.ghost_end = hover;
        d.ghost = hover.and_then(|t| match self.world.station_at(t) {
            Some(s) if s != d.from => self.world.plan(d.from, s, &d.via).ok(),
            Some(_) => None,
            None => self.world.sketch(d.from, &d.via, t),
        });
    }

    fn update(&mut self) {
        let dt = get_frame_time();
        self.view.update(dt);
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::P) {
            self.paused = !self.paused;
        }
        if is_key_pressed(KeyCode::Escape) || is_mouse_button_pressed(MouseButton::Right) {
            self.draft = None;
        }
        let hover = self.mouse_tile();
        if is_mouse_button_pressed(MouseButton::Left)
            && let Some(t) = hover
        {
            self.click(t);
        }
        self.update_ghost(hover);
        if let Some((_, left)) = &mut self.toast {
            *left -= dt;
            if *left <= 0.0 {
                self.toast = None;
            }
        }
        self.acc += dt * self.speed();
        let mut steps = 0;
        while self.acc >= DT && steps < MAX_STEPS {
            self.world.step(&[]);
            self.acc -= DT;
            steps += 1;
        }
        if steps == MAX_STEPS {
            self.acc = 0.0;
        }
        self.world.events.clear();
    }

    fn draw(&self) {
        let hover = self.mouse_tile();
        let overlay = render::Overlay {
            ghost: self.draft.as_ref().and_then(|d| d.ghost.as_ref()),
            ghost_ok: true,
            waypoints: self.draft.as_ref().map_or(&[], |d| &d.via),
            from: self.draft.as_ref().map(|d| d.from),
            hover_station: hover.and_then(|t| self.world.station_at(t)),
            hover_tile: hover,
        };
        set_camera(&self.view.camera());
        render::draw_world(&self.world, &overlay);
        set_default_camera();
        let hint = match &self.draft {
            None => "Click a station to start a line.  WASD: move  Wheel: zoom  Space: pause",
            Some(_) => {
                "Click the second station to build.  Click grass: waypoint.  Esc or right click: cancel"
            }
        };
        ui::hint(hint);
        if let Some(d) = &self.draft {
            let text = match &d.ghost {
                Some(r) => format!("{:.0} tiles, {} new", r.len, r.new_tiles),
                None => "No route".to_owned(),
            };
            ui::cursor_label(&text);
        }
        if self.paused {
            ui::banner("PAUSED");
        } else if self.draft.is_some() {
            ui::banner("x0.25");
        }
        if let Some((text, _)) = &self.toast {
            ui::toast(text);
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new();
    loop {
        game.update();
        game.draw();
        next_frame().await;
    }
}
