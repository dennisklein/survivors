//! Transient visual effects: particles, floating text, banners and shake.

use macroquad::prelude::*;
use rules::game::{Event, Game};
use rules::geom::V2;

pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
    pub max: f32,
    pub size: f32,
    pub color: Color,
}

pub struct Floater {
    pub pos: V2,
    pub text: String,
    pub color: Color,
    pub life: f32,
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub floaters: Vec<Floater>,
    pub banner: Option<(String, f32)>,
    pub shake: f32,
    seed: u32,
}

impl Fx {
    fn rnd(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn burst(&mut self, pos: V2, n: usize, speed: f32, color: Color, size: f32) {
        for _ in 0..n {
            let a = self.rnd() * std::f32::consts::TAU;
            let v = V2::from_angle(a) * (speed * (0.3 + self.rnd()));
            let life = 0.3 + self.rnd() * 0.5;
            let size = size * (0.5 + self.rnd());
            self.particles.push(Particle {
                pos,
                vel: v,
                life,
                max: life,
                size,
                color,
            });
        }
    }

    pub fn smoke(&mut self, pos: V2) {
        let v = V2::new(self.rnd() * 20.0 - 10.0, -25.0 - self.rnd() * 15.0);
        let size = 4.0 + self.rnd() * 3.0;
        self.particles.push(Particle {
            pos,
            vel: v,
            life: 1.0,
            max: 1.0,
            size,
            color: Color::new(0.85, 0.85, 0.85, 0.5),
        });
    }

    pub fn float(&mut self, pos: V2, text: String, color: Color) {
        self.floaters.push(Floater {
            pos,
            text,
            color,
            life: 1.2,
        });
    }

    pub fn on_event(&mut self, e: &Event, g: &Game) {
        match e {
            Event::Killed { pos, big } => {
                if *big {
                    self.burst(*pos, 40, 220.0, ORANGE, 5.0);
                    self.shake = self.shake.max(8.0);
                } else {
                    self.burst(*pos, 10, 120.0, Color::new(0.8, 0.2, 0.2, 1.0), 3.0);
                }
            }
            Event::Served { town, cash } => {
                let p = g.towns[*town].pos + V2::new(0.0, -30.0);
                self.float(p, format!("+${cash}"), GOLD);
            }
            Event::WaveStart { wave, town } => {
                let boss = if wave % 5 == 0 { "  -  BOSS!" } else { "" };
                self.banner = Some((
                    format!("Wave {wave}: the tide hits {}{boss}", g.towns[*town].name),
                    3.0,
                ));
            }
            Event::RescueArrived(t) => {
                let p = g.towns[*t].pos;
                self.burst(p, 30, 160.0, GOLD, 4.0);
                self.float(p + V2::new(0.0, -40.0), "Defenders rallied!".into(), GOLD);
            }
            Event::TownLost(t) => {
                let p = g.towns[*t].pos;
                self.burst(p, 60, 260.0, Color::new(1.0, 0.4, 0.1, 1.0), 6.0);
                self.shake = 14.0;
                self.banner = Some((format!("{} has fallen!", g.towns[*t].name), 3.0));
            }
            Event::PlayerHurt => self.shake = self.shake.max(4.0),
            Event::LevelUp => {
                let p = g.loco_pos();
                self.burst(p, 30, 200.0, SKYBLUE, 4.0);
            }
            Event::Built | Event::Upgraded => {}
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.pos += p.vel * dt;
            p.vel = p.vel * (1.0 - 2.0 * dt).max(0.0);
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for f in &mut self.floaters {
            f.pos.y -= 30.0 * dt;
            f.life -= dt;
        }
        self.floaters.retain(|f| f.life > 0.0);
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
        self.shake = (self.shake - 30.0 * dt).max(0.0);
    }
}
