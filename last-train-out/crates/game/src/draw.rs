//! World rendering (vector art only) and transient particle effects.

use macroquad::prelude::*;
use rules::game::{CAR_LEN, CAR_W, CarKind, EnemyKind, Event, Game, STATION_ZONE};
use rules::math::{CIRC, LOOP_R, V2, track_heading, track_pos};

pub const STATION_NAMES: [&str; 3] = ["Ashford Halt", "Cinder Junction", "Greyhaven"];

pub fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

pub fn car_color(kind: CarKind) -> Color {
    match kind {
        CarKind::Engine => Color::from_rgba(70, 74, 84, 255),
        CarKind::Gun => Color::from_rgba(96, 112, 70, 255),
        CarKind::Flak => Color::from_rgba(120, 92, 60, 255),
        CarKind::Workshop => Color::from_rgba(70, 104, 130, 255),
        CarKind::Infirmary => Color::from_rgba(214, 210, 198, 255),
        CarKind::Tender => Color::from_rgba(44, 40, 38, 255),
        CarKind::Coach => Color::from_rgba(140, 50, 44, 255),
    }
}

pub fn car_letter(kind: CarKind) -> &'static str {
    match kind {
        CarKind::Engine => "E",
        CarKind::Gun => "G",
        CarKind::Flak => "F",
        CarKind::Workshop => "W",
        CarKind::Infirmary => "+",
        CarKind::Tender => "T",
        CarKind::Coach => "C",
    }
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
}

struct Floater {
    pos: Vec2,
    text: String,
    life: f32,
    color: Color,
}

#[derive(Default)]
pub struct Fx {
    parts: Vec<Particle>,
    floats: Vec<Floater>,
    pub shake: f32,
    smoke_t: f32,
}

impl Fx {
    fn burst(&mut self, pos: Vec2, n: usize, speed: f32, size: f32, life: f32, color: Color) {
        for _ in 0..n {
            if self.parts.len() > 1500 {
                return;
            }
            let a = rand::gen_range(0.0, std::f32::consts::TAU);
            let s = rand::gen_range(0.2, 1.0) * speed;
            self.parts.push(Particle {
                pos,
                vel: vec2(a.cos(), a.sin()) * s,
                life,
                max: life,
                size: size * rand::gen_range(0.6, 1.3),
                color,
            });
        }
    }

    fn float(&mut self, pos: Vec2, text: String, color: Color) {
        self.floats.push(Floater {
            pos,
            text,
            life: 1.0,
            color,
        });
    }

    pub fn on_event(&mut self, e: &Event) {
        match *e {
            Event::Shot { pos, heavy } => {
                self.burst(v(pos), if heavy { 4 } else { 2 }, 60.0, 2.5, 0.15, YELLOW)
            }
            Event::Hit { pos } => self.burst(
                v(pos),
                3,
                80.0,
                2.0,
                0.2,
                Color::from_rgba(255, 230, 160, 255),
            ),
            Event::Boom { pos, radius } => {
                self.burst(v(pos), 18, radius * 2.5, 5.0, 0.4, ORANGE);
                self.burst(
                    v(pos),
                    8,
                    radius,
                    8.0,
                    0.7,
                    Color::from_rgba(90, 80, 70, 200),
                );
            }
            Event::Kill { pos, big } => {
                let n = if big { 40 } else { 8 };
                self.burst(
                    v(pos),
                    n,
                    if big { 220.0 } else { 90.0 },
                    3.5,
                    0.5,
                    Color::from_rgba(120, 170, 90, 255),
                );
                if big {
                    self.burst(v(pos), 30, 200.0, 7.0, 0.8, ORANGE);
                    self.shake = self.shake.max(8.0);
                }
            }
            Event::CarLost { pos } => {
                self.burst(v(pos), 60, 260.0, 7.0, 1.0, ORANGE);
                self.burst(v(pos), 30, 120.0, 10.0, 1.4, DARKGRAY);
                self.shake = 14.0;
            }
            Event::CarHit { .. } => self.shake = self.shake.max(1.5),
            Event::Coin { pos, value } => {
                self.float(v(pos), format!("+${value}"), GOLD);
            }
            Event::Fare { pos, amount } => {
                self.float(v(pos), format!("+${amount} fare"), GOLD);
            }
            Event::Board { pos } => self.float(v(pos), "+1 aboard".into(), SKYBLUE),
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32, game: &Game) {
        self.shake = (self.shake - dt * 30.0).max(0.0);
        for p in &mut self.parts {
            p.pos += p.vel * dt;
            p.vel *= 1.0 - (dt * 3.0).min(1.0);
            p.life -= dt;
        }
        self.parts.retain(|p| p.life > 0.0);
        for f in &mut self.floats {
            f.pos.y -= 30.0 * dt;
            f.life -= dt;
        }
        self.floats.retain(|f| f.life > 0.0);
        // Smoke from the engine chimney, thicker at speed.
        self.smoke_t -= dt * (1.0 + game.speed / 40.0);
        if self.smoke_t <= 0.0 {
            self.smoke_t = 0.12;
            let p = local(game.car_pos(0), track_heading(game.car_s(0)), 14.0, 0.0);
            self.parts.push(Particle {
                pos: p,
                vel: vec2(rand::gen_range(-10.0, 10.0), rand::gen_range(-25.0, -10.0)),
                life: 1.6,
                max: 1.6,
                size: 6.0,
                color: Color::from_rgba(200, 200, 200, 110),
            });
        }
    }

    pub fn draw(&self) {
        for p in &self.parts {
            let k = p.life / p.max;
            let mut c = p.color;
            c.a *= k;
            draw_circle(p.pos.x, p.pos.y, p.size * (0.5 + 0.5 * k), c);
        }
    }

    /// Floating texts, drawn in screen space so they are not mirrored.
    pub fn draw_floats(&self, cam: &Camera2D) {
        for f in &self.floats {
            let mut c = f.color;
            c.a = f.life.min(1.0);
            let p = cam.world_to_screen(f.pos);
            let w = measure_text(&f.text, None, 18, 1.0).width;
            draw_text(&f.text, p.x - w / 2.0, p.y - 20.0, 18.0, c);
        }
    }
}

/// Transforms a local offset (x along heading, y to the left) into world space.
pub fn local(center: V2, heading: f32, x: f32, y: f32) -> Vec2 {
    let (s, c) = heading.sin_cos();
    vec2(center.x + c * x - s * y, center.y + s * x + c * y)
}

fn rect(center: Vec2, w: f32, h: f32, rot: f32, color: Color) {
    draw_rectangle_ex(
        center.x,
        center.y,
        w,
        h,
        DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation: rot,
            color,
        },
    );
}

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

pub fn draw_ground(view: Rect, time: f32) {
    let cell = 64.0;
    let x0 = (view.x / cell).floor() as i32 - 1;
    let y0 = (view.y / cell).floor() as i32 - 1;
    let x1 = ((view.x + view.w) / cell).ceil() as i32 + 1;
    let y1 = ((view.y + view.h) / cell).ceil() as i32 + 1;
    for gx in x0..x1 {
        for gy in y0..y1 {
            let h = hash(gx, gy);
            let px = gx as f32 * cell + (h % 64) as f32;
            let py = gy as f32 * cell + ((h >> 6) % 64) as f32;
            match h % 7 {
                0 => draw_circle(
                    px,
                    py,
                    3.0 + (h >> 12 & 3) as f32,
                    Color::from_rgba(52, 46, 40, 255),
                ),
                1 | 2 => {
                    let sway = (time * 1.5 + px * 0.01).sin() * 1.5;
                    let c = Color::from_rgba(58, 70, 44, 255);
                    draw_line(px, py, px - 3.0 + sway, py - 7.0, 1.5, c);
                    draw_line(px, py, px + 3.0 + sway, py - 6.0, 1.5, c);
                }
                3 if (h >> 20).is_multiple_of(5) => {
                    // Dead tree.
                    draw_circle(px, py, 12.0, Color::from_rgba(40, 38, 32, 255));
                    draw_circle(px - 4.0, py - 3.0, 7.0, Color::from_rgba(48, 46, 38, 255));
                }
                _ => {}
            }
        }
    }
}

pub fn draw_track(view: Rect) {
    let bed = Color::from_rgba(62, 56, 50, 255);
    draw_poly_lines(0.0, 0.0, 255, LOOP_R, 0.0, 34.0, bed);
    let sleepers = (CIRC / 16.0) as usize;
    let grow = view.w.max(view.h) * 0.1;
    let view = Rect::new(
        view.x - grow,
        view.y - grow,
        view.w + 2.0 * grow,
        view.h + 2.0 * grow,
    );
    for k in 0..sleepers {
        let s = k as f32 * 16.0;
        let p = track_pos(s);
        if !view.contains(v(p)) {
            continue;
        }
        rect(
            v(p),
            5.0,
            26.0,
            track_heading(s),
            Color::from_rgba(88, 66, 46, 255),
        );
    }
    let rail = Color::from_rgba(170, 170, 176, 255);
    draw_poly_lines(0.0, 0.0, 255, LOOP_R - 8.0, 0.0, 2.5, rail);
    draw_poly_lines(0.0, 0.0, 255, LOOP_R + 8.0, 0.0, 2.5, rail);
}

pub fn draw_stations(game: &Game, time: f32) {
    for (k, st) in game.stations.iter().enumerate() {
        let h = track_heading(st.s);
        let center = V2::from_angle(st.s / LOOP_R) * (LOOP_R + 58.0);
        let active = game.at_station == Some(k);
        // Stop zone markers on the track.
        for side in [-1.0, 1.0] {
            let p = track_pos(st.s + side * STATION_ZONE);
            rect(v(p), 6.0, 40.0, h, Color::from_rgba(230, 200, 60, 200));
        }
        let plat = Color::from_rgba(120, 112, 100, 255);
        rect(v(center), 2.0 * STATION_ZONE, 56.0, h, plat);
        let roof = if active {
            Color::from_rgba(80, 150, 90, 255)
        } else {
            Color::from_rgba(110, 60, 50, 255)
        };
        rect(local(center, h, 0.0, -14.0), 140.0, 22.0, h, roof);
        // Waiting refugees.
        for i in 0..st.waiting {
            let col = (i % 7) as f32;
            let row = (i / 7) as f32;
            let bob = ((time * 3.0 + i as f32).sin() * 1.5).abs();
            let p = local(center, h, -60.0 + col * 20.0, 10.0 + row * 12.0 - bob);
            draw_circle(p.x, p.y, 5.0, Color::from_rgba(200, 170, 120, 255));
            draw_circle(p.x, p.y, 2.5, Color::from_rgba(90, 60, 40, 255));
        }
    }
}

/// Station names, drawn in screen space so they are not mirrored.
pub fn station_labels(game: &Game, cam: &Camera2D) {
    for (k, st) in game.stations.iter().enumerate() {
        let center = V2::from_angle(st.s / LOOP_R) * (LOOP_R + 110.0);
        let p = cam.world_to_screen(v(center));
        let name = STATION_NAMES[k];
        let w = measure_text(name, None, 22, 1.0).width;
        draw_text(name, p.x - w / 2.0, p.y, 22.0, WHITE);
    }
}

pub fn draw_train(game: &Game, time: f32) {
    let manned = game.conductor_car();
    for i in 0..game.cars.len() {
        let car = &game.cars[i];
        let s = game.car_s(i);
        let p = track_pos(s);
        let h = track_heading(s);
        let c = v(p);
        // Coupling to the car behind.
        if i + 1 < game.cars.len() {
            let q = v(game.car_pos(i + 1));
            draw_line(c.x, c.y, q.x, q.y, 4.0, Color::from_rgba(30, 30, 30, 255));
        }
        let mut body = car_color(car.kind);
        if car.hurt < 0.12 {
            body = Color::from_rgba(255, 120, 100, 255);
        }
        rect(
            c + vec2(3.0, 4.0),
            CAR_LEN,
            CAR_W,
            h,
            Color::from_rgba(0, 0, 0, 90),
        );
        rect(c, CAR_LEN, CAR_W, h, body);
        rect(c, CAR_LEN - 6.0, CAR_W - 8.0, h, Color { a: 0.25, ..BLACK });
        match car.kind {
            CarKind::Engine => {
                rect(
                    c,
                    CAR_LEN - 10.0,
                    CAR_W - 6.0,
                    h,
                    Color::from_rgba(90, 96, 108, 255),
                );
                let nose = local(p, h, CAR_LEN / 2.0, 0.0);
                let l = local(p, h, CAR_LEN / 2.0 - 8.0, CAR_W / 2.0);
                let r = local(p, h, CAR_LEN / 2.0 - 8.0, -CAR_W / 2.0);
                draw_triangle(
                    nose + (nose - c).normalize() * 10.0,
                    l,
                    r,
                    Color::from_rgba(180, 40, 30, 255),
                );
                let ch = local(p, h, 14.0, 0.0);
                draw_circle(ch.x, ch.y, 6.0, BLACK);
                let cab = local(p, h, -18.0, 0.0);
                rect(cab, 16.0, CAR_W - 4.0, h, Color::from_rgba(50, 52, 60, 255));
                let lamp = local(p, h, CAR_LEN / 2.0 + 4.0, 0.0);
                let glow = 0.5 + 0.5 * (time * 4.0).sin().abs();
                draw_circle(lamp.x, lamp.y, 4.0, Color::new(1.0, 0.95, 0.6, glow));
            }
            CarKind::Gun | CarKind::Flak => {
                let heavy = car.kind == CarKind::Flak;
                let r = if heavy { 11.0 } else { 9.0 };
                draw_circle(c.x, c.y, r + 2.0, Color::from_rgba(30, 30, 30, 255));
                draw_circle(c.x, c.y, r, Color::from_rgba(150, 150, 140, 255));
                let dir = vec2(car.aim.cos(), car.aim.sin());
                let side = vec2(-dir.y, dir.x);
                let offs: &[f32] = if heavy { &[-4.0, 4.0] } else { &[0.0] };
                for o in offs {
                    let a = c + side * *o;
                    let b = a + dir * if heavy { 22.0 } else { 20.0 };
                    draw_line(
                        a.x,
                        a.y,
                        b.x,
                        b.y,
                        if heavy { 4.0 } else { 3.5 },
                        Color::from_rgba(40, 40, 40, 255),
                    );
                }
            }
            CarKind::Workshop => {
                rect(c, 26.0, 6.0, h + 0.8, Color::from_rgba(220, 220, 220, 255));
                rect(c, 26.0, 6.0, h - 0.8, Color::from_rgba(240, 180, 60, 255));
            }
            CarKind::Infirmary => {
                rect(c, 22.0, 7.0, h, Color::from_rgba(200, 40, 40, 255));
                rect(c, 7.0, 22.0, h, Color::from_rgba(200, 40, 40, 255));
            }
            CarKind::Tender => {
                for k in 0..5 {
                    let q = local(
                        p,
                        h,
                        -20.0 + k as f32 * 10.0,
                        ((k * 7) % 3) as f32 * 3.0 - 3.0,
                    );
                    draw_circle(q.x, q.y, 6.0, Color::from_rgba(20, 20, 22, 255));
                }
            }
            CarKind::Coach => {
                let seats = game.seats(i);
                for w in 0..seats.min(8) {
                    let row = if w % 2 == 0 { 1.0 } else { -1.0 };
                    let q = local(p, h, -22.0 + (w / 2) as f32 * 14.0, row * 7.0);
                    let filled = w < car.passengers.len();
                    let col = if filled {
                        Color::from_rgba(255, 220, 140, 255)
                    } else {
                        Color::from_rgba(60, 30, 30, 255)
                    };
                    rect(q, 9.0, 5.0, h, col);
                }
            }
        }
        if car.hp < car.max_hp * 0.999 {
            let k = (car.hp / car.max_hp).clamp(0.0, 1.0);
            let bx = c.x - 22.0;
            let by = c.y - 30.0;
            draw_rectangle(bx, by, 44.0, 5.0, Color::from_rgba(20, 20, 20, 200));
            let col = if k > 0.5 {
                GREEN
            } else if k > 0.25 {
                ORANGE
            } else {
                RED
            };
            draw_rectangle(bx, by, 44.0 * k, 5.0, col);
        }
        if i == manned {
            draw_poly_lines(
                c.x,
                c.y,
                24,
                CAR_LEN * 0.6,
                0.0,
                1.5,
                Color::new(1.0, 1.0, 1.0, 0.25),
            );
        }
    }
    // Conductor.
    let cp = game.conductor_pos();
    let ch = track_heading(game.conductor_s());
    let bob = (time * 10.0).sin() * 1.0;
    let body = local(cp, ch, 0.0, -2.0 + bob);
    draw_circle(body.x, body.y, 7.0, Color::from_rgba(30, 40, 90, 255));
    draw_circle(body.x, body.y, 4.5, Color::from_rgba(240, 200, 160, 255));
    draw_circle(body.x, body.y - 1.0, 3.5, Color::from_rgba(20, 24, 60, 255));
}

pub fn enemy_color(kind: EnemyKind) -> Color {
    match kind {
        EnemyKind::Walker => Color::from_rgba(110, 150, 80, 255),
        EnemyKind::Runner => Color::from_rgba(170, 180, 70, 255),
        EnemyKind::Brute => Color::from_rgba(120, 70, 110, 255),
        EnemyKind::Juggernaut => Color::from_rgba(150, 40, 40, 255),
    }
}

pub fn draw_enemies(game: &Game) {
    let engine = game.car_pos(0);
    for e in &game.enemies {
        let p = v(e.pos);
        let wob = 1.0 + (e.wobble * 8.0).sin() * 0.06;
        let r = e.radius * wob;
        let col = if e.hurt < 0.08 {
            WHITE
        } else {
            enemy_color(e.kind)
        };
        draw_circle(p.x + 2.0, p.y + 3.0, r, Color::from_rgba(0, 0, 0, 80));
        draw_circle(p.x, p.y, r, Color::from_rgba(30, 30, 24, 255));
        draw_circle(p.x, p.y, r - 2.0, col);
        // Eyes look toward the train.
        let to = (v(engine) - p).normalize_or_zero();
        let side = vec2(-to.y, to.x);
        let eye = if e.biting {
            RED
        } else {
            Color::from_rgba(255, 240, 120, 255)
        };
        for s in [-1.0, 1.0] {
            let q = p + to * r * 0.45 + side * s * r * 0.35;
            draw_circle(q.x, q.y, (r * 0.18).max(1.8), eye);
        }
        if e.kind == EnemyKind::Juggernaut {
            let k = (e.hp / e.max_hp).clamp(0.0, 1.0);
            draw_rectangle(
                p.x - 40.0,
                p.y - r - 14.0,
                80.0,
                6.0,
                Color::from_rgba(20, 20, 20, 220),
            );
            draw_rectangle(p.x - 40.0, p.y - r - 14.0, 80.0 * k, 6.0, RED);
            for s in [-1.0, 1.0] {
                let horn = p - to * r * 0.2 + side * s * r * 0.9;
                draw_circle(horn.x, horn.y, 6.0, Color::from_rgba(230, 220, 200, 255));
            }
        }
    }
}

pub fn draw_projectiles(game: &Game, time: f32) {
    for c in &game.coins {
        let bob = (time * 5.0 + c.pos.x).sin() * 2.0;
        let blink = c.age > 25.0 && (time * 10.0).sin() < 0.0;
        if !blink {
            let r = if c.value >= 10 { 9.0 } else { 5.5 };
            draw_circle(
                c.pos.x,
                c.pos.y + bob,
                r,
                Color::from_rgba(150, 110, 20, 255),
            );
            draw_circle(c.pos.x, c.pos.y + bob, r - 1.5, GOLD);
        }
    }
    for b in &game.bullets {
        if b.splash > 0.0 {
            draw_circle(b.pos.x, b.pos.y, 5.0, ORANGE);
            draw_circle(b.pos.x, b.pos.y, 2.5, YELLOW);
        } else {
            let tail = b.pos - b.vel.norm() * 8.0;
            draw_line(
                tail.x,
                tail.y,
                b.pos.x,
                b.pos.y,
                3.0,
                Color::from_rgba(255, 240, 150, 255),
            );
        }
    }
}
