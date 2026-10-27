//! D-Gun Dash: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    ARENA, DGUN_RADIUS, DURATION, EMP_RADIUS, EnemyKind, Event, Game, Input, PLAYER_RADIUS, Phase,
    PickupKind, STRUCT_KINDS, StructKind, TURRET_RANGE, xp_needed,
};
use sim::math::{V2, v2};

const BG: Color = Color::new(0.05, 0.06, 0.08, 1.0);
const GRID: Color = Color::new(0.12, 0.15, 0.19, 1.0);
const ALLY: Color = Color::new(0.3, 0.75, 1.0, 1.0);
const FOE: Color = Color::new(1.0, 0.33, 0.22, 1.0);
const DGUN: Color = Color::new(1.0, 0.65, 0.15, 1.0);
const XP: Color = Color::new(0.4, 1.0, 0.55, 1.0);
const PANEL: Color = Color::new(0.0, 0.0, 0.0, 0.6);

#[derive(PartialEq)]
enum Screen {
    Title,
    Game,
}

struct Fx {
    pos: V2,
    vel: V2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
    ring: bool,
}

struct Floater {
    pos: V2,
    text: String,
    life: f32,
    color: Color,
}

struct Banner {
    text: String,
    life: f32,
}

struct View {
    cam: V2,
    scale: f32,
    shake: f32,
}

impl View {
    fn w2s(&self, p: V2) -> Vec2 {
        vec2(
            (p.x - self.cam.x) * self.scale + screen_width() / 2.0,
            (p.y - self.cam.y) * self.scale + screen_height() / 2.0,
        )
    }

    fn s2w(&self, s: Vec2) -> V2 {
        v2(
            (s.x - screen_width() / 2.0) / self.scale + self.cam.x,
            (s.y - screen_height() / 2.0) / self.scale + self.cam.y,
        )
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "D-Gun Dash".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: false,
        ..Default::default()
    }
}

fn centered(text: impl AsRef<str>, y: f32, size: f32, color: Color) {
    let text = text.as_ref();
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fg: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.1, 0.1, 0.12, 0.9));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fg);
    draw_rectangle_lines(x, y, w, h, 1.5, Color::new(1.0, 1.0, 1.0, 0.3));
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
        || !touches().is_empty()
}

struct App {
    screen: Screen,
    game: Game,
    audio: Audio,
    view: View,
    fx: Vec<Fx>,
    floaters: Vec<Floater>,
    banner: Option<Banner>,
    paused: bool,
    seed: u64,
    best: u64,
}

impl App {
    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add(0x9E37) ^ (get_time() * 1000.0) as u64;
        self.game = Game::new(self.seed);
        self.fx.clear();
        self.floaters.clear();
        self.banner = Some(Banner {
            text: "Survive 10:00. Chain kills to charge the D-Gun!".into(),
            life: 3.5,
        });
        self.paused = false;
        self.screen = Screen::Game;
    }

    fn burst(&mut self, pos: V2, n: usize, speed: f32, color: Color, size: f32) {
        for _ in 0..n {
            let a = self.game.rng.range(0.0, std::f32::consts::TAU);
            let s = self.game.rng.range(0.2, 1.0) * speed;
            let life = self.game.rng.range(0.25, 0.6);
            self.fx.push(Fx {
                pos,
                vel: V2::from_angle(a) * s,
                life,
                max: life,
                size,
                color,
                ring: false,
            });
        }
    }

    fn ring(&mut self, pos: V2, radius: f32, color: Color, life: f32) {
        self.fx.push(Fx {
            pos,
            vel: V2::ZERO,
            life,
            max: life,
            size: radius,
            color,
            ring: true,
        });
    }

    fn handle_events(&mut self) {
        let events: Vec<Event> = std::mem::take(&mut self.game.events);
        let mut kills = 0;
        for e in &events {
            self.audio.on_event(e);
            match *e {
                Event::Kill { pos, big } => {
                    kills += 1;
                    if self.fx.len() < 900 {
                        let n = if big { 30 } else { 6 };
                        self.burst(pos, n, if big { 260.0 } else { 140.0 }, FOE, 3.0);
                    }
                    if big {
                        self.ring(pos, 70.0, DGUN, 0.4);
                        self.view.shake = self.view.shake.max(6.0);
                    }
                }
                Event::Explosion { pos, radius } => {
                    self.ring(pos, radius, DGUN, 0.35);
                    self.burst(pos, 16, 220.0, DGUN, 3.0);
                    self.view.shake = self.view.shake.max(4.0);
                }
                Event::Dgun => self.view.shake = self.view.shake.max(8.0),
                Event::BossSpawn => {
                    self.banner = Some(Banner {
                        text: "WARNING: Juggernaut inbound! Save a D-Gun!".into(),
                        life: 3.0,
                    })
                }
                Event::Surge => {
                    self.banner = Some(Banner {
                        text: "Swarm surge!".into(),
                        life: 2.0,
                    })
                }
                Event::ChargeReady => {
                    let p = self.game.pos;
                    self.floaters.push(Floater {
                        pos: p + v2(0.0, -30.0),
                        text: "D-GUN +1".into(),
                        life: 1.0,
                        color: DGUN,
                    });
                    self.ring(p, 40.0, DGUN, 0.3);
                }
                Event::Built(k) => {
                    let r = match k {
                        StructKind::Emp => EMP_RADIUS,
                        _ => 30.0,
                    };
                    self.ring(self.game.pos, r, ALLY, 0.3);
                }
                Event::PlayerHurt => self.view.shake = self.view.shake.max(3.0),
                _ => {}
            }
        }
        let c = self.game.combo;
        if kills > 0 && c >= 5 && (c.is_multiple_of(10) || c == 5) {
            self.floaters.push(Floater {
                pos: self.game.pos + v2(0.0, -40.0),
                text: format!("{c} CHAIN!"),
                life: 1.0,
                color: YELLOW,
            });
        }
    }

    fn update_fx(&mut self, dt: f32) {
        for f in &mut self.fx {
            f.pos += f.vel * dt;
            f.vel = f.vel * (1.0 - 3.0 * dt);
            f.life -= dt;
        }
        self.fx.retain(|f| f.life > 0.0);
        for f in &mut self.floaters {
            f.pos.y -= 40.0 * dt;
            f.life -= dt;
        }
        self.floaters.retain(|f| f.life > 0.0);
        if let Some(b) = &mut self.banner {
            b.life -= dt;
            if b.life <= 0.0 {
                self.banner = None;
            }
        }
        self.view.shake = (self.view.shake - 30.0 * dt).max(0.0);
    }

    fn read_input(&self) -> Input {
        let mut mv = V2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            mv.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            mv.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            mv.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            mv.x += 1.0;
        }
        let (mx, my) = mouse_position();
        let aim = self.view.s2w(vec2(mx, my));
        let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3];
        let pressed = keys.iter().position(|k| is_key_pressed(*k));
        let leveling = matches!(self.game.phase, Phase::LevelUp(_));
        let choose = if leveling {
            pressed.or_else(|| {
                is_mouse_button_pressed(MouseButton::Left)
                    .then(|| card_at(vec2(mx, my)))
                    .flatten()
            })
        } else {
            None
        };
        Input {
            mv,
            aim,
            dgun: !leveling
                && (is_key_pressed(KeyCode::Space) || is_mouse_button_pressed(MouseButton::Right)),
            drop: if leveling { None } else { pressed },
            choose,
        }
    }

    fn frame(&mut self) {
        let dt = get_frame_time();
        if is_key_pressed(KeyCode::M) {
            self.audio.toggle_mute();
        }
        match self.screen {
            Screen::Title => {
                if any_input() && !is_key_pressed(KeyCode::M) {
                    self.audio.start_music();
                    self.restart();
                }
                self.view.cam = v2((get_time() as f32 * 0.3).sin() * 80.0, 0.0);
                self.update_fx(dt);
                self.draw_world();
                self.draw_title();
            }
            Screen::Game => {
                self.audio.start_music();
                if self.game.alive()
                    && (is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape))
                {
                    self.paused = !self.paused;
                }
                if !self.game.alive() {
                    if is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Enter) {
                        self.restart();
                    } else if is_key_pressed(KeyCode::T) {
                        self.screen = Screen::Title;
                    }
                }
                if !self.paused {
                    let input = self.read_input();
                    self.game.update(dt, &input);
                    if !self.game.alive() {
                        self.best = self.best.max(self.game.score);
                    }
                    self.handle_events();
                    self.update_fx(dt);
                }
                let target = self.game.pos;
                self.view.cam = self.view.cam + (target - self.view.cam) * (8.0 * dt).min(1.0);
                self.draw_world();
                self.draw_hud();
                self.draw_overlays();
            }
        }
    }

    fn draw_world(&self) {
        clear_background(BG);
        let v = &self.view;
        let shake = v2(
            rand::gen_range(-1.0, 1.0) * v.shake,
            rand::gen_range(-1.0, 1.0) * v.shake,
        );
        let view = View {
            cam: v.cam + shake,
            scale: v.scale,
            shake: 0.0,
        };
        let g = &self.game;
        let s = view.scale;
        // Grid.
        let step = 80.0;
        let tl = view.s2w(vec2(0.0, 0.0));
        let br = view.s2w(vec2(screen_width(), screen_height()));
        let mut x = (tl.x / step).floor() * step;
        while x < br.x {
            if x.abs() <= ARENA {
                let a = view.w2s(v2(x, tl.y.max(-ARENA)));
                let b = view.w2s(v2(x, br.y.min(ARENA)));
                draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            }
            x += step;
        }
        let mut y = (tl.y / step).floor() * step;
        while y < br.y {
            if y.abs() <= ARENA {
                let a = view.w2s(v2(tl.x.max(-ARENA), y));
                let b = view.w2s(v2(br.x.min(ARENA), y));
                draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            }
            y += step;
        }
        let c = view.w2s(v2(-ARENA, -ARENA));
        draw_rectangle_lines(
            c.x,
            c.y,
            ARENA * 2.0 * s,
            ARENA * 2.0 * s,
            4.0,
            Color::new(1.0, 0.3, 0.2, 0.6),
        );

        let t = get_time() as f32;
        // Structures.
        for st in &g.structures {
            let p = view.w2s(st.pos);
            let frac = st.life / st.max_life;
            match st.kind {
                StructKind::Mine => {
                    let blink = if st.cd <= 0.0 && (t * 4.0).fract() < 0.5 {
                        RED
                    } else {
                        Color::new(0.5, 0.15, 0.1, 1.0)
                    };
                    draw_poly(
                        p.x,
                        p.y,
                        6,
                        9.0 * s,
                        t * 30.0,
                        Color::new(0.6, 0.6, 0.5, 1.0),
                    );
                    draw_circle(p.x, p.y, 3.5 * s, blink);
                }
                StructKind::Turret => {
                    draw_circle_lines(
                        p.x,
                        p.y,
                        TURRET_RANGE * s,
                        1.0,
                        Color::new(0.3, 0.75, 1.0, 0.12),
                    );
                    draw_rectangle(p.x - 12.0 * s, p.y - 12.0 * s, 24.0 * s, 24.0 * s, DARKGRAY);
                    draw_circle(p.x, p.y, 8.0 * s, ALLY);
                    let d = V2::from_angle(st.angle) * 18.0 * s;
                    draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 4.0 * s, WHITE);
                    bar(p.x - 12.0 * s, p.y + 15.0 * s, 24.0 * s, 3.0, frac, ALLY);
                }
                StructKind::Emp => {
                    let pulse = 0.08 + 0.05 * (t * 6.0).sin();
                    draw_circle(p.x, p.y, EMP_RADIUS * s, Color::new(0.4, 0.6, 1.0, pulse));
                    draw_circle_lines(
                        p.x,
                        p.y,
                        EMP_RADIUS * s * frac,
                        2.0,
                        Color::new(0.6, 0.8, 1.0, 0.6),
                    );
                    draw_poly(p.x, p.y, 4, 10.0 * s, t * 90.0, SKYBLUE);
                }
            }
        }
        // Pickups.
        for k in &g.pickups {
            let p = view.w2s(k.pos);
            let fade = if k.life < 4.0 && (t * 8.0).fract() < 0.5 {
                0.3
            } else {
                1.0
            };
            match k.kind {
                PickupKind::Xp(v) => {
                    let r = if v > 1 { 5.0 } else { 3.5 };
                    draw_poly(p.x, p.y, 4, r * s, 45.0, Color { a: fade, ..XP });
                }
                PickupKind::Crate(sk) => {
                    draw_rectangle(p.x - 8.0 * s, p.y - 8.0 * s, 16.0 * s, 16.0 * s, BROWN);
                    let l = &sk.name()[..1];
                    draw_text(l, p.x - 4.0 * s, p.y + 5.0 * s, 16.0 * s, WHITE);
                }
                PickupKind::Heal => {
                    draw_rectangle(p.x - 7.0 * s, p.y - 2.5 * s, 14.0 * s, 5.0 * s, GREEN);
                    draw_rectangle(p.x - 2.5 * s, p.y - 7.0 * s, 5.0 * s, 14.0 * s, GREEN);
                }
                PickupKind::Charge => {
                    draw_circle(p.x, p.y, 8.0 * s, Color { a: fade, ..DGUN });
                    draw_circle_lines(p.x, p.y, 11.0 * s + (t * 5.0).sin() * 2.0, 2.0, YELLOW);
                }
            }
        }
        // Enemies.
        for e in &g.enemies {
            let p = view.w2s(e.pos);
            let col = if e.flash > 0.0 {
                WHITE
            } else if e.slow > 0.0 {
                Color::new(0.6, 0.4, 0.9, 1.0)
            } else {
                FOE
            };
            let r = e.radius * s;
            let to = (g.pos - e.pos).norm();
            let ang = to.y.atan2(to.x).to_degrees();
            match e.kind {
                EnemyKind::Grunt => {
                    draw_poly(p.x, p.y, 3, r, ang, col);
                }
                EnemyKind::Runner => {
                    draw_poly(p.x, p.y, 3, r, ang, Color::new(1.0, 0.6, 0.2, 1.0));
                    if e.flash > 0.0 {
                        draw_poly(p.x, p.y, 3, r, ang, WHITE);
                    }
                }
                EnemyKind::Tank => {
                    draw_poly(p.x, p.y, 6, r, ang, Color::new(0.6, 0.15, 0.12, 1.0));
                    draw_poly_lines(p.x, p.y, 6, r, ang, 2.0, col);
                    let d = to * r * 1.2;
                    draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 5.0 * s, col);
                }
                EnemyKind::Boss => {
                    draw_poly(
                        p.x,
                        p.y,
                        8,
                        r,
                        ang + t * 20.0,
                        Color::new(0.45, 0.05, 0.1, 1.0),
                    );
                    draw_poly_lines(p.x, p.y, 8, r, ang + t * 20.0, 4.0, col);
                    draw_circle(p.x, p.y, r * 0.4, col);
                    bar(
                        p.x - 40.0 * s,
                        p.y - r - 14.0,
                        80.0 * s,
                        6.0,
                        e.hp / e.max_hp,
                        FOE,
                    );
                }
            }
        }
        // Projectiles.
        for b in &g.bullets {
            let p = view.w2s(b.pos);
            let tail = view.w2s(b.pos - b.vel * 0.025);
            let c = if b.turret { SKYBLUE } else { ALLY };
            draw_line(p.x, p.y, tail.x, tail.y, 3.0 * s, c);
        }
        for e in &g.enemy_shots {
            let p = view.w2s(e.pos);
            draw_circle(p.x, p.y, 6.0 * s, Color::new(1.0, 0.2, 0.6, 1.0));
            draw_circle(p.x, p.y, 3.0 * s, WHITE);
        }
        for d in &g.dguns {
            let p = view.w2s(d.pos);
            for i in 1..6 {
                let q = view.w2s(d.pos - d.vel * (i as f32 * 0.012));
                draw_circle(
                    q.x,
                    q.y,
                    DGUN_RADIUS * s * (1.0 - i as f32 * 0.15),
                    Color::new(1.0, 0.5, 0.1, 0.35 - i as f32 * 0.05),
                );
            }
            draw_circle(p.x, p.y, DGUN_RADIUS * s, DGUN);
            draw_circle(p.x, p.y, DGUN_RADIUS * s * 0.55, YELLOW);
        }
        // Player (only during play).
        if self.screen == Screen::Game {
            let p = view.w2s(g.pos);
            let (mx, my) = mouse_position();
            let aim = (view.s2w(vec2(mx, my)) - g.pos).norm();
            let r = PLAYER_RADIUS * s;
            let col = if g.hurt > 0.15 { WHITE } else { ALLY };
            draw_circle(p.x, p.y, r * 1.6, Color::new(0.3, 0.75, 1.0, 0.12));
            draw_poly(p.x, p.y, 5, r, t * 40.0, Color::new(0.15, 0.3, 0.5, 1.0));
            draw_circle(p.x, p.y, r * 0.7, col);
            let d = aim * r * 1.5;
            let dcol = if g.dgun_charges > 0 { DGUN } else { GRAY };
            draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 5.0 * s, dcol);
            draw_circle_lines(
                p.x,
                p.y,
                g.stats.magnet * s,
                1.0,
                Color::new(0.4, 1.0, 0.55, 0.08),
            );
            // Aim reticle.
            draw_circle_lines(mx, my, 9.0, 1.5, dcol);
            draw_line(mx - 13.0, my, mx - 5.0, my, 1.5, dcol);
            draw_line(mx + 5.0, my, mx + 13.0, my, 1.5, dcol);
        }
        // Effects.
        for f in &self.fx {
            let p = view.w2s(f.pos);
            let a = f.life / f.max;
            if f.ring {
                let r = f.size * s * (1.2 - a * 0.6);
                draw_circle_lines(p.x, p.y, r, 3.0, Color { a, ..f.color });
            } else {
                draw_circle(p.x, p.y, f.size * s * a.max(0.3), Color { a, ..f.color });
            }
        }
        for f in &self.floaters {
            let p = view.w2s(f.pos);
            let a = f.life.min(1.0);
            let d = measure_text(&f.text, None, 22, 1.0);
            draw_text(
                &f.text,
                p.x - d.width / 2.0,
                p.y,
                22.0,
                Color { a, ..f.color },
            );
        }
    }

    fn draw_hud(&self) {
        let g = &self.game;
        let w = screen_width();
        let h = screen_height();
        // XP bar along the top edge.
        bar(
            0.0,
            0.0,
            w,
            8.0,
            g.xp as f32 / xp_needed(g.level) as f32,
            XP,
        );
        // Top-left: HP and level.
        draw_rectangle(10.0, 16.0, 250.0, 58.0, PANEL);
        draw_text(format!("LV {}", g.level), 18.0, 38.0, 24.0, XP);
        bar(
            80.0,
            24.0,
            170.0,
            16.0,
            g.hp / g.max_hp,
            Color::new(0.9, 0.25, 0.25, 1.0),
        );
        draw_text(
            format!("{:.0}/{:.0}", g.hp.max(0.0), g.max_hp),
            120.0,
            37.0,
            18.0,
            WHITE,
        );
        draw_text(format!("Kills {}", g.kills), 18.0, 64.0, 20.0, LIGHTGRAY);
        // Top-center: countdown.
        let left = (DURATION - g.time).max(0.0);
        let clock = format!("{:02}:{:02}", (left / 60.0) as u32, (left % 60.0) as u32);
        centered(&clock, 46.0, 40.0, WHITE);
        // Top-right: score and combo.
        let sc = format!("SCORE {}", g.score);
        let d = measure_text(&sc, None, 28, 1.0);
        draw_rectangle(w - d.width - 30.0, 16.0, d.width + 20.0, 64.0, PANEL);
        draw_text(&sc, w - d.width - 20.0, 42.0, 28.0, YELLOW);
        if g.combo > 1 {
            let ct = format!("x{:.1}  chain {}", g.combo_mult(), g.combo);
            draw_text(&ct, w - d.width - 20.0, 64.0, 20.0, ORANGE);
            bar(
                w - d.width - 20.0,
                70.0,
                d.width,
                4.0,
                g.combo_t / sim::game::COMBO_WINDOW,
                ORANGE,
            );
        }
        // Bottom-center: D-Gun and structure inventory.
        let pw = 470.0;
        let px = (w - pw) / 2.0;
        let py = h - 86.0;
        draw_rectangle(px, py, pw, 76.0, PANEL);
        draw_text("D-GUN [Space]", px + 10.0, py + 22.0, 20.0, DGUN);
        for i in 0..g.stats.dgun_max {
            let filled = i < g.dgun_charges;
            let cx = px + 22.0 + i as f32 * 26.0;
            let c = if filled { DGUN } else { DARKGRAY };
            draw_circle(cx, py + 42.0, 10.0, c);
            draw_circle_lines(cx, py + 42.0, 10.0, 2.0, YELLOW);
        }
        let meter = if g.dgun_charges >= g.stats.dgun_max {
            1.0
        } else {
            g.dgun_meter / g.stats.kills_per_charge
        };
        bar(px + 10.0, py + 58.0, 140.0, 8.0, meter, DGUN);
        for (i, k) in STRUCT_KINDS.iter().enumerate() {
            let x = px + 170.0 + i as f32 * 98.0;
            let n = g.inventory[i];
            let col = if n > 0 { ALLY } else { DARKGRAY };
            draw_rectangle_lines(x, py + 8.0, 90.0, 60.0, 2.0, col);
            draw_text(
                format!("[{}] {}", i + 1, k.name()),
                x + 6.0,
                py + 28.0,
                20.0,
                col,
            );
            draw_text(format!("x{}", n), x + 6.0, py + 56.0, 28.0, WHITE);
        }
        if self.audio.muted {
            draw_text("MUTED [M]", 18.0, h - 16.0, 20.0, GRAY);
        }
        if let Some(b) = &self.banner {
            let a = b.life.min(1.0);
            centered(&b.text, 120.0, 30.0, Color::new(1.0, 0.85, 0.3, a));
        }
    }

    fn draw_overlays(&self) {
        let g = &self.game;
        let w = screen_width();
        let h = screen_height();
        if let Phase::LevelUp(choices) = &g.phase {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
            centered("LEVEL UP!", h / 2.0 - 150.0, 54.0, XP);
            centered(
                "Pick an upgrade: 1 / 2 / 3 or click",
                h / 2.0 - 112.0,
                22.0,
                LIGHTGRAY,
            );
            let (mx, my) = mouse_position();
            let hover = card_at(vec2(mx, my));
            for (i, u) in choices.iter().enumerate() {
                let r = card_rect(i);
                let hl = hover == Some(i);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.08, 0.12, 0.18, 0.95));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, if hl { 4.0 } else { 2.0 }, ALLY);
                draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 30.0, 24.0, YELLOW);
                draw_text(u.title(), r.x + 12.0, r.y + 70.0, 26.0, WHITE);
                draw_text(u.desc(), r.x + 12.0, r.y + 104.0, 17.0, LIGHTGRAY);
                let rank = g.rank(*u);
                draw_text(
                    format!("rank {} -> {}", rank, rank + 1),
                    r.x + 12.0,
                    r.y + r.h - 16.0,
                    18.0,
                    GRAY,
                );
            }
        }
        if self.paused {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
            centered("PAUSED", h / 2.0, 60.0, WHITE);
            centered("P / Esc to resume", h / 2.0 + 40.0, 24.0, LIGHTGRAY);
        }
        if matches!(g.phase, Phase::Won | Phase::Lost) {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
            let (title, col) = if g.phase == Phase::Won {
                ("COMMANDER SURVIVED!", XP)
            } else {
                ("COMMANDER DESTROYED", FOE)
            };
            centered(title, h / 2.0 - 110.0, 60.0, col);
            let t = g.time;
            centered(format!("Score {}", g.score), h / 2.0 - 50.0, 40.0, YELLOW);
            centered(
                format!(
                    "Time {:02}:{:02}   Kills {}   Best chain {}   Level {}",
                    (t / 60.0) as u32,
                    (t % 60.0) as u32,
                    g.kills,
                    g.best_combo,
                    g.level
                ),
                h / 2.0,
                24.0,
                WHITE,
            );
            centered(
                format!("Best score {}", self.best),
                h / 2.0 + 34.0,
                22.0,
                GRAY,
            );
            centered(
                "R / Enter: play again     T: title",
                h / 2.0 + 90.0,
                26.0,
                LIGHTGRAY,
            );
        }
    }

    fn draw_title(&self) {
        let h = screen_height();
        let w = screen_width();
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.45));
        let pulse = 0.75 + 0.25 * (get_time() as f32 * 3.0).sin();
        centered("D-GUN DASH", h * 0.25, 84.0, DGUN);
        centered(
            "A 10 minute score attack. Kite the swarm, seed temporary defences,",
            h * 0.25 + 50.0,
            22.0,
            LIGHTGRAY,
        );
        centered(
            "chain kills to recharge the Commander's D-Gun and obliterate everything in its path.",
            h * 0.25 + 76.0,
            22.0,
            LIGHTGRAY,
        );
        let lines = [
            "WASD / arrows   move (the laser fires automatically)",
            "Space / right click   fire the D-Gun toward the mouse",
            "1  Mine     2  Pop-up turret     3  EMP slow field",
            "P / Esc pause      M mute",
        ];
        for (i, l) in lines.iter().enumerate() {
            centered(l, h * 0.5 + i as f32 * 32.0, 24.0, WHITE);
        }
        centered(
            "Press any key or click to start",
            h * 0.82,
            34.0,
            Color::new(1.0, 0.85, 0.3, pulse),
        );
        if self.best > 0 {
            centered(
                format!("Best score {}", self.best),
                h * 0.82 + 36.0,
                22.0,
                GRAY,
            );
        }
        centered(
            "Fan homage to Beyond All Reason. Procedural art and sound.",
            h - 16.0,
            16.0,
            GRAY,
        );
    }
}

fn card_rect(i: usize) -> Rect {
    let cw = 280.0;
    let gap = 24.0;
    let total = cw * 3.0 + gap * 2.0;
    let x0 = (screen_width() - total) / 2.0;
    Rect::new(
        x0 + i as f32 * (cw + gap),
        screen_height() / 2.0 - 80.0,
        cw,
        180.0,
    )
}

fn card_at(p: Vec2) -> Option<usize> {
    (0..3).find(|&i| card_rect(i).contains(p))
}

#[macroquad::main(window_conf)]
async fn main() {
    let audio = Audio::new().await;
    let mut app = App {
        screen: Screen::Title,
        game: Game::new(1),
        audio,
        view: View {
            cam: V2::ZERO,
            scale: 1.0,
            shake: 0.0,
        },
        fx: Vec::new(),
        floaters: Vec::new(),
        banner: None,
        paused: false,
        seed: 1,
        best: 0,
    };
    // Populate the title backdrop with a drifting demo swarm.
    for i in 0..40 {
        let a = i as f32 * 0.7;
        let p = V2::from_angle(a) * (200.0 + i as f32 * 12.0);
        app.game.spawn_enemy(EnemyKind::Grunt, p);
    }
    loop {
        app.view.scale = (screen_height() / 820.0).clamp(0.6, 1.6);
        app.frame();
        next_frame().await;
    }
}
