//! Purely cosmetic effects: camera, particles, floating texts and banners.

use crate::ui;
use macroquad::prelude::*;
use rules::data::{EnemyKind, TOWNS};
use rules::game::{ARENA, Event, Game};
use rules::math::V2;

pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: Color,
}

pub struct FloatText {
    pub pos: V2,
    pub text: String,
    pub life: f32,
    pub color: Color,
}

pub struct Fx {
    pub cam: V2,
    pub shake: f32,
    pub particles: Vec<Particle>,
    pub texts: Vec<FloatText>,
    pub banner: Option<(String, f32)>,
    pub rings: Vec<(V2, f32, f32)>,
    seed: u32,
}

impl Fx {
    pub fn new(cam: V2) -> Self {
        Self {
            cam,
            shake: 0.0,
            particles: Vec::new(),
            texts: Vec::new(),
            banner: None,
            rings: Vec::new(),
            seed: 1,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    /// World point at the centre of the screen, with shake applied.
    pub fn view_center(&self) -> V2 {
        let t = macroquad::time::get_time() as f32;
        let s = self.shake * 8.0;
        self.cam + V2::new((t * 61.0).sin() * s, (t * 47.0).cos() * s)
    }

    pub fn update(&mut self, dt: f32, game: &Game) {
        let target = game.player.pos;
        self.cam = self.cam.lerp(target, (dt * 6.0).min(1.0));
        let half = V2::new(screen_width() / 2.0, screen_height() / 2.0);
        let margin = 120.0;
        let clamp_axis = |c: f32, arena: f32, h: f32| {
            let lim = (arena + margin - h).max(0.0);
            c.clamp(-lim, lim)
        };
        self.cam.x = clamp_axis(self.cam.x, ARENA.x, half.x);
        self.cam.y = clamp_axis(self.cam.y, ARENA.y, half.y);
        self.shake = (self.shake - dt * 2.5).max(0.0);
        self.particles.retain_mut(|p| {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.vel = p.vel * (1.0 - 3.0 * dt).max(0.0);
            p.life > 0.0
        });
        self.texts.retain_mut(|t| {
            t.life -= dt;
            t.pos.y -= 30.0 * dt;
            t.life > 0.0
        });
        self.rings.retain_mut(|r| {
            r.2 -= dt;
            r.2 > 0.0
        });
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
    }

    fn burst(&mut self, pos: V2, n: usize, speed: f32, color: Color, size: f32) {
        if self.particles.len() > 1500 {
            return;
        }
        for _ in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let v = speed * (0.3 + self.rand());
            let life = 0.3 + 0.4 * self.rand();
            self.particles.push(Particle {
                pos,
                vel: V2::from_angle(a) * v,
                life,
                max_life: life,
                size,
                color,
            });
        }
    }

    fn text(&mut self, pos: V2, text: impl Into<String>, color: Color) {
        self.texts.push(FloatText {
            pos,
            text: text.into(),
            life: 1.2,
            color,
        });
    }

    fn banner(&mut self, text: impl Into<String>) {
        self.banner = Some((text.into(), 2.5));
    }

    pub fn on_event(&mut self, e: &Event, game: &Game) {
        match *e {
            Event::Killed(pos, kind) => {
                let (n, size) = match kind {
                    EnemyKind::Boss => (60, 6.0),
                    EnemyKind::Brute => (16, 4.0),
                    _ => (7, 3.0),
                };
                self.burst(pos, n, 160.0, ui::enemy_color(kind), size);
                if kind == EnemyKind::Boss {
                    self.shake = 1.5;
                    self.banner("Boss derailed!");
                }
            }
            Event::Hit(pos) => self.burst(pos, 2, 90.0, WHITE, 2.0),
            Event::Explosion(pos, r) => {
                self.rings.push((pos, r, 0.35));
                self.burst(pos, 14, r * 3.0, ORANGE, 4.0);
                self.shake = self.shake.max(0.25);
            }
            Event::Crate(c) => {
                let p = game.player.pos;
                self.text(
                    p + V2::new(0.0, -26.0),
                    format!("+1 {}", c.name()),
                    ui::cargo_color(c),
                );
            }
            Event::WeaponUnlocked(c) => self.banner(format!("{} unlocked!", c.weapon())),
            Event::TrainArriving(i) => {
                if let Some(l) = game.lines.get(i) {
                    self.banner(format!("Train from {} approaching", TOWNS[l.town].name));
                }
            }
            Event::Unloaded(i) => {
                if let Some(l) = game.lines.get(i) {
                    self.burst(l.platform(), 20, 120.0, Color::new(0.9, 0.9, 0.9, 0.6), 5.0);
                    self.text(l.platform(), "On time! +$10", GREEN);
                }
            }
            Event::TrainMissed(i) => {
                if let Some(l) = game.lines.get(i) {
                    self.text(l.platform(), "Missed train!", RED);
                }
            }
            Event::PlayerHurt => self.shake = self.shake.max(0.4),
            Event::WaveStart(w) => self.banner(format!("Wave {w}")),
            Event::BossSpawned => self.banner("A boss train-wrecker approaches!"),
            Event::LineBought(t) => self.banner(format!("Line to {} leased", TOWNS[t].name)),
            _ => {}
        }
    }
}
