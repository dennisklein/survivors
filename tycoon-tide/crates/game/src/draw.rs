//! World rendering: terrain, rails, towns, trains, enemies and effects.

use crate::fx::Fx;
use macroquad::prelude::*;
use rules::game::{Game, Kind, TOWN_RADIUS, WORLD_H, WORLD_W, river_x};
use rules::geom::V2;

/// Maps world coordinates onto the screen, letterboxed.
#[derive(Clone, Copy)]
pub struct View {
    pub s: f32,
    pub ox: f32,
    pub oy: f32,
}

impl View {
    pub fn fit(shake: f32, t: f32) -> Self {
        let s = (screen_width() / WORLD_W).min(screen_height() / WORLD_H);
        let jitter = V2::new((t * 91.0).sin(), (t * 77.0).cos()) * shake;
        Self {
            s,
            ox: (screen_width() - WORLD_W * s) * 0.5 + jitter.x,
            oy: (screen_height() - WORLD_H * s) * 0.5 + jitter.y,
        }
    }

    pub fn p(&self, v: V2) -> Vec2 {
        vec2(self.ox + v.x * self.s, self.oy + v.y * self.s)
    }

    pub fn world(&self, m: Vec2) -> V2 {
        V2::new((m.x - self.ox) / self.s, (m.y - self.oy) / self.s)
    }

    pub fn l(&self, x: f32) -> f32 {
        x * self.s
    }
}

pub const GRASS: Color = Color::new(0.33, 0.52, 0.29, 1.0);
const WATER: Color = Color::new(0.2, 0.42, 0.66, 1.0);
const WATER_HI: Color = Color::new(0.35, 0.58, 0.8, 1.0);

fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    (x & 0xFFFF) as f32 / 65535.0
}

pub fn terrain(v: &View, t: f32) {
    clear_background(Color::new(0.11, 0.17, 0.12, 1.0));
    let tl = v.p(V2::new(0.0, 0.0));
    draw_rectangle(tl.x, tl.y, v.l(WORLD_W), v.l(WORLD_H), GRASS);
    for i in 0..220 {
        let p = V2::new(hash(i) * WORLD_W, hash(i + 999) * WORLD_H);
        let c = if i % 3 == 0 {
            Color::new(0.27, 0.45, 0.24, 1.0)
        } else {
            Color::new(0.38, 0.58, 0.32, 1.0)
        };
        let q = v.p(p);
        if i % 7 == 0 {
            // Little tree.
            draw_circle(q.x, q.y, v.l(9.0), Color::new(0.2, 0.38, 0.2, 1.0));
            draw_circle(
                q.x - v.l(2.0),
                q.y - v.l(3.0),
                v.l(5.0),
                Color::new(0.28, 0.5, 0.26, 1.0),
            );
        } else {
            draw_circle(q.x, q.y, v.l(5.0 + hash(i + 7) * 14.0), c);
        }
    }
    let mut y = -20.0;
    while y < WORLD_H + 20.0 {
        let a = v.p(V2::new(river_x(y), y));
        let b = v.p(V2::new(river_x(y + 12.0), y + 12.0));
        draw_line(a.x, a.y, b.x, b.y, v.l(48.0), WATER);
        draw_circle(a.x, a.y, v.l(24.0), WATER);
        let w = (t * 2.0 + y * 0.05).sin() * 6.0;
        let c = v.p(V2::new(river_x(y) + w, y));
        draw_circle(c.x, c.y, v.l(2.0), WATER_HI);
        y += 12.0;
    }
}

fn rail_segment(v: &View, a: V2, b: V2, rail: Color, sleeper: Color, width: f32) {
    let d = (b - a).norm();
    let n = V2::new(-d.y, d.x);
    let len = a.dist(b);
    let mut s = 6.0;
    while s < len {
        let c = a + d * s;
        let p = v.p(c + n * 6.0);
        let q = v.p(c - n * 6.0);
        draw_line(p.x, p.y, q.x, q.y, v.l(3.0), sleeper);
        s += 12.0;
    }
    for k in [-3.5f32, 3.5] {
        let p = v.p(a + n * k);
        let q = v.p(b + n * k);
        draw_line(p.x, p.y, q.x, q.y, v.l(width), rail);
    }
}

pub fn network(v: &View, g: &Game, highlight: Option<usize>) {
    for (i, e) in g.edges.iter().enumerate() {
        let (a, b) = (g.towns[e.a].pos, g.towns[e.b].pos);
        if e.bridge {
            // Bridge deck around the river crossing.
            let mut lo = 0.0f32;
            let mut hi = 1.0f32;
            let side = |t: f32| {
                let p = a.lerp(b, t);
                (p.x - river_x(p.y)).signum()
            };
            let s0 = side(0.0);
            for _ in 0..20 {
                let m = (lo + hi) * 0.5;
                if side(m) == s0 { lo = m } else { hi = m }
            }
            let c = a.lerp(b, lo);
            let d = (b - a).norm();
            let p = v.p(c - d * 45.0);
            let q = v.p(c + d * 45.0);
            draw_line(
                p.x,
                p.y,
                q.x,
                q.y,
                v.l(20.0),
                Color::new(0.45, 0.32, 0.2, 1.0),
            );
            draw_line(
                p.x,
                p.y,
                q.x,
                q.y,
                v.l(16.0),
                Color::new(0.6, 0.44, 0.28, 1.0),
            );
        }
        if highlight == Some(i) {
            let p = v.p(a);
            let q = v.p(b);
            draw_line(
                p.x,
                p.y,
                q.x,
                q.y,
                v.l(16.0),
                Color::new(1.0, 0.9, 0.3, 0.25),
            );
        }
        rail_segment(
            v,
            a,
            b,
            Color::new(0.75, 0.75, 0.78, 1.0),
            Color::new(0.4, 0.28, 0.18, 1.0),
            1.6,
        );
    }
}

pub fn towns(v: &View, g: &Game, t: f32) {
    for (i, town) in g.towns.iter().enumerate() {
        let c = v.p(town.pos);
        let r = v.l(TOWN_RADIUS);
        if !town.alive {
            draw_circle(c.x, c.y, r, Color::new(0.25, 0.22, 0.2, 1.0));
            for k in 0..6 {
                let o = V2::from_angle(k as f32 * 1.1) * 12.0;
                let q = v.p(town.pos + o);
                draw_rectangle(
                    q.x - v.l(4.0),
                    q.y - v.l(3.0),
                    v.l(8.0),
                    v.l(6.0),
                    Color::new(0.15, 0.13, 0.12, 1.0),
                );
            }
        } else {
            let faded = town.station == 0;
            let base = if faded {
                Color::new(0.45, 0.5, 0.4, 0.6)
            } else {
                Color::new(0.62, 0.58, 0.48, 1.0)
            };
            draw_circle(c.x, c.y, r, base);
            let houses = 3 + (town.pop / 12.0) as usize;
            for k in 0..houses.min(16) {
                let ring = if k < 6 { 13.0 } else { 21.0 };
                let a = k as f32 * 2.399 + i as f32;
                let q = v.p(town.pos + V2::from_angle(a) * ring);
                let roof = if faded {
                    Color::new(0.5, 0.5, 0.5, 0.7)
                } else if k % 3 == 0 {
                    Color::new(0.7, 0.25, 0.2, 1.0)
                } else if k % 3 == 1 {
                    Color::new(0.55, 0.3, 0.25, 1.0)
                } else {
                    Color::new(0.35, 0.35, 0.45, 1.0)
                };
                draw_rectangle(
                    q.x - v.l(4.5),
                    q.y - v.l(3.5),
                    v.l(9.0),
                    v.l(7.0),
                    Color::new(0.9, 0.86, 0.75, 1.0),
                );
                draw_triangle(
                    vec2(q.x - v.l(5.5), q.y - v.l(3.0)),
                    vec2(q.x + v.l(5.5), q.y - v.l(3.0)),
                    vec2(q.x, q.y - v.l(8.0)),
                    roof,
                );
            }
            if town.station > 0 {
                // Station platform with level pips.
                draw_rectangle(
                    c.x - v.l(10.0),
                    c.y - v.l(4.0),
                    v.l(20.0),
                    v.l(8.0),
                    Color::new(0.2, 0.2, 0.25, 1.0),
                );
                for k in 0..town.station {
                    draw_circle(c.x - v.l(6.0) + v.l(6.0 * k as f32), c.y, v.l(2.0), GOLD);
                }
            }
        }
        if i == g.target && town.active() {
            let pulse = 0.5 + 0.5 * (t * 6.0).sin();
            draw_circle_lines(
                c.x,
                c.y,
                r + v.l(8.0 + pulse * 4.0),
                v.l(2.5),
                Color::new(1.0, 0.2, 0.15, 0.8),
            );
        }
        if town.boost > 0.0 {
            draw_circle_lines(
                c.x,
                c.y,
                r + v.l(4.0),
                v.l(3.0),
                Color::new(1.0, 0.85, 0.2, 0.9),
            );
        }
        // Militia defenders orbit their town.
        let m = town.militia();
        for k in 0..m {
            let a = t * 0.8 + k as f32 * std::f32::consts::TAU / m as f32;
            let q = v.p(town.pos + V2::from_angle(a) * (TOWN_RADIUS + 10.0));
            draw_circle(q.x, q.y, v.l(3.5), Color::new(0.2, 0.35, 0.85, 1.0));
            draw_circle(
                q.x,
                q.y - v.l(2.0),
                v.l(1.8),
                Color::new(0.95, 0.8, 0.6, 1.0),
            );
        }
        let label = if town.station == 0 {
            town.name.to_string()
        } else {
            format!("{} {}", town.name, town.pop as u32)
        };
        let fs = v.l(20.0).max(11.0);
        let dim = measure_text(&label, None, fs as u16, 1.0);
        let col = if town.station == 0 {
            Color::new(1.0, 1.0, 1.0, 0.45)
        } else {
            WHITE
        };
        draw_text(
            &label,
            c.x - dim.width * 0.5 + 1.0,
            c.y + r + fs + 1.0,
            fs,
            BLACK,
        );
        draw_text(&label, c.x - dim.width * 0.5, c.y + r + fs, fs, col);
        if town.active() && town.hp < town.max_hp() - 0.5 {
            bar(
                c.x - r,
                c.y - r - v.l(10.0),
                r * 2.0,
                v.l(4.0),
                town.hp / town.max_hp(),
                RED,
            );
        }
        if town.active() {
            // Service rating meter.
            bar(
                c.x - r,
                c.y + r + v.l(2.0),
                r * 2.0,
                v.l(3.0),
                town.rating,
                Color::new(0.3, 0.9, 0.4, 1.0),
            );
        }
    }
}

pub fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, c: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, c);
}

fn train_car(v: &View, pos: V2, dir: V2, len: f32, body: Color, trim: Color) {
    let c = v.p(pos);
    let rot = dir.y.atan2(dir.x);
    draw_rectangle_ex(
        c.x,
        c.y,
        v.l(len),
        v.l(15.0),
        DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation: rot,
            color: body,
        },
    );
    draw_rectangle_ex(
        c.x,
        c.y,
        v.l(len * 0.35),
        v.l(15.0),
        DrawRectangleParams {
            offset: vec2(1.2, 0.5),
            rotation: rot,
            color: trim,
        },
    );
}

pub fn loco(v: &View, g: &Game, fx: &mut Fx, t: f32) {
    let pos = g.loco_pos();
    let dir = g.loco_dir();
    // Gun range ring.
    let c = v.p(pos);
    draw_circle_lines(
        c.x,
        c.y,
        v.l(g.stats.range),
        1.0,
        Color::new(1.0, 1.0, 1.0, 0.12),
    );
    let tender = pos - dir * 26.0;
    train_car(
        v,
        tender,
        dir,
        20.0,
        Color::new(0.15, 0.3, 0.2, 1.0),
        Color::new(0.1, 0.1, 0.1, 1.0),
    );
    train_car(
        v,
        pos,
        dir,
        34.0,
        Color::new(0.1, 0.45, 0.28, 1.0),
        Color::new(0.85, 0.7, 0.2, 1.0),
    );
    let chimney = v.p(pos + dir * 11.0);
    draw_circle(
        chimney.x,
        chimney.y,
        v.l(4.5),
        Color::new(0.12, 0.12, 0.12, 1.0),
    );
    let cab = v.p(pos - dir * 10.0);
    draw_circle(cab.x, cab.y, v.l(3.0), Color::new(0.95, 0.85, 0.4, 1.0));
    if g.loco.speed > 5.0 && (t * 12.0).fract() < 0.25 {
        fx.smoke(pos + dir * 11.0);
    }
    if let Some(r) = &g.rescue {
        let (p, d) = r.pose();
        train_car(
            v,
            p - d * 22.0,
            d,
            18.0,
            Color::new(0.7, 0.1, 0.1, 1.0),
            WHITE,
        );
        train_car(
            v,
            p,
            d,
            28.0,
            Color::new(0.85, 0.15, 0.15, 1.0),
            Color::new(1.0, 0.85, 0.2, 1.0),
        );
        if (t * 20.0).fract() < 0.5 {
            fx.smoke(p + d * 9.0);
        }
    }
}

pub fn actors(v: &View, g: &Game, t: f32) {
    for c in &g.coins {
        let q = v.p(c.pos);
        let blink = c.life > 3.0 || (t * 10.0).fract() < 0.6;
        if blink {
            draw_circle(q.x, q.y, v.l(5.0), Color::new(0.85, 0.65, 0.1, 1.0));
            draw_circle(
                q.x - v.l(1.0),
                q.y - v.l(1.0),
                v.l(2.5),
                Color::new(1.0, 0.92, 0.4, 1.0),
            );
        }
    }
    for e in &g.enemies {
        let q = v.p(e.pos);
        let r = v.l(e.kind.radius());
        let flash = e.flash > 0.0;
        let col = |c: Color| if flash { WHITE } else { c };
        let wob = (t * 10.0 + e.id as f32).sin() * 0.15;
        match e.kind {
            Kind::Grunt => {
                draw_circle(q.x, q.y, r, col(Color::new(0.75, 0.15, 0.2, 1.0)));
                draw_circle(q.x, q.y, r * 0.55, col(Color::new(0.5, 0.05, 0.1, 1.0)));
            }
            Kind::Runner => {
                let a = wob * 3.0;
                let p = |k: f32| {
                    let d = V2::from_angle(a + k * 2.094);
                    vec2(q.x + d.x * r * 1.3, q.y + d.y * r * 1.3)
                };
                draw_triangle(p(0.0), p(1.0), p(2.0), col(Color::new(0.95, 0.5, 0.1, 1.0)));
            }
            Kind::Brute => {
                draw_rectangle_ex(
                    q.x,
                    q.y,
                    r * 2.0,
                    r * 2.0,
                    DrawRectangleParams {
                        offset: vec2(0.5, 0.5),
                        rotation: wob,
                        color: col(Color::new(0.45, 0.2, 0.55, 1.0)),
                    },
                );
                draw_circle(q.x, q.y, r * 0.4, col(Color::new(0.9, 0.8, 0.2, 1.0)));
            }
            Kind::Boss => {
                draw_circle(q.x, q.y, r * 1.1, Color::new(0.0, 0.0, 0.0, 0.35));
                draw_poly(
                    q.x,
                    q.y,
                    8,
                    r,
                    t * 40.0,
                    col(Color::new(0.25, 0.08, 0.3, 1.0)),
                );
                draw_poly_lines(
                    q.x,
                    q.y,
                    8,
                    r,
                    t * 40.0,
                    v.l(3.0),
                    Color::new(0.9, 0.2, 0.3, 1.0),
                );
                draw_circle(q.x, q.y, r * 0.35, col(Color::new(1.0, 0.3, 0.2, 1.0)));
                bar(
                    q.x - r,
                    q.y - r - v.l(10.0),
                    r * 2.0,
                    v.l(5.0),
                    e.hp / e.max_hp,
                    RED,
                );
            }
        }
        if e.kind != Kind::Boss {
            // Eyes facing their goal.
            draw_circle(q.x - r * 0.3, q.y - r * 0.2, r * 0.18, WHITE);
            draw_circle(q.x + r * 0.3, q.y - r * 0.2, r * 0.18, WHITE);
        }
    }
    for s in &g.shots {
        let q = v.p(s.pos);
        let c = if s.militia {
            Color::new(0.5, 0.75, 1.0, 1.0)
        } else {
            Color::new(1.0, 0.9, 0.3, 1.0)
        };
        let tail = v.p(s.pos - s.vel * 0.02);
        draw_line(q.x, q.y, tail.x, tail.y, v.l(3.0), c);
    }
}

pub fn effects(v: &View, fx: &Fx) {
    for p in &fx.particles {
        let q = v.p(p.pos);
        let mut c = p.color;
        c.a *= (p.life / p.max).clamp(0.0, 1.0);
        draw_circle(q.x, q.y, v.l(p.size), c);
    }
    for f in &fx.floaters {
        let q = v.p(f.pos);
        let mut c = f.color;
        c.a = f.life.min(1.0);
        let fs = v.l(22.0).max(12.0);
        let d = measure_text(&f.text, None, fs as u16, 1.0);
        draw_text(
            &f.text,
            q.x - d.width * 0.5 + 1.0,
            q.y + 1.0,
            fs,
            Color::new(0.0, 0.0, 0.0, c.a),
        );
        draw_text(&f.text, q.x - d.width * 0.5, q.y, fs, c);
    }
}
