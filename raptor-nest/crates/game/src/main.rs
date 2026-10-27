//! Raptor Nest: macroquad front end. Rendering, input and audio only; all
//! rules live in the `sim` crate.

mod audio;

use audio::{Audio, Sfx};
use macroquad::prelude::*;
use sim::game::{
    BUILD_TIME, CORE_POS, Event, Game, Input, Phase, RaptorKind, TowerKind, WORLD, build_radius,
};
use sim::math::V2;

const BG: Color = Color::new(0.07, 0.10, 0.07, 1.0);
const PANEL: Color = Color::new(0.0, 0.0, 0.0, 0.65);
const BLUE: Color = Color::new(0.35, 0.65, 1.0, 1.0);
const ACID: Color = Color::new(0.6, 1.0, 0.2, 1.0);
const NEST: Color = Color::new(1.0, 0.45, 0.1, 1.0);

fn conf() -> Conf {
    Conf {
        window_title: "Raptor Nest".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: false,
        ..Default::default()
    }
}

enum Fx {
    Beam {
        a: V2,
        b: V2,
        heavy: bool,
        t: f32,
    },
    Blast {
        p: V2,
        r: f32,
        t: f32,
        max: f32,
        color: Color,
    },
    Spark {
        p: V2,
        t: f32,
    },
    Text {
        p: V2,
        s: String,
        t: f32,
        color: Color,
    },
}

enum Screen {
    Title,
    Play,
}

struct App {
    game: Game,
    audio: Audio,
    screen: Screen,
    fx: Vec<Fx>,
    selected: TowerKind,
    message: Option<(String, f32)>,
    shake: f32,
    seed: u64,
    paused: bool,
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

impl App {
    fn cam(&self) -> V2 {
        let (w, h) = (screen_width(), screen_height());
        let c = self.game.cmd.pos;
        let mut x = c.x - w / 2.0;
        let mut y = c.y - h / 2.0;
        x = x.clamp(-100.0, (WORLD - w + 100.0).max(-100.0));
        y = y.clamp(-100.0, (WORLD - h + 100.0).max(-100.0));
        if self.shake > 0.0 {
            x += rand::gen_range(-1.0, 1.0) * self.shake * 10.0;
            y += rand::gen_range(-1.0, 1.0) * self.shake * 10.0;
        }
        V2::new(x, y)
    }

    fn restart(&mut self) {
        self.seed += 1;
        self.game = Game::new(self.seed);
        self.fx.clear();
        self.message = Some(("Build phase: place towers, Enter when ready".into(), 4.0));
    }

    fn handle_events(&mut self) {
        for e in std::mem::take(&mut self.game.events) {
            self.audio.on_event(&e);
            match e {
                Event::Beam { from, to, heavy } => self.fx.push(Fx::Beam {
                    a: from,
                    b: to,
                    heavy,
                    t: if heavy { 0.18 } else { 0.08 },
                }),
                Event::Hit(p) => {
                    if self.fx.len() < 400 {
                        self.fx.push(Fx::Spark { p, t: 0.12 })
                    }
                }
                Event::Kill { pos, big } => self.fx.push(Fx::Blast {
                    p: pos,
                    r: 0.0,
                    t: 0.0,
                    max: if big { 50.0 } else { 22.0 },
                    color: ACID,
                }),
                Event::Explosion { pos, radius } => self.fx.push(Fx::Blast {
                    p: pos,
                    r: 0.0,
                    t: 0.0,
                    max: radius,
                    color: ORANGE,
                }),
                Event::StructureLost(p) => {
                    self.shake = 0.4;
                    self.fx.push(Fx::Blast {
                        p,
                        r: 0.0,
                        t: 0.0,
                        max: 70.0,
                        color: ORANGE,
                    })
                }
                Event::NestDestroyed(p) => {
                    self.shake = 0.8;
                    self.fx.push(Fx::Blast {
                        p,
                        r: 0.0,
                        t: 0.0,
                        max: 160.0,
                        color: NEST,
                    });
                    self.fx.push(Fx::Text {
                        p,
                        s: "NEST DESTROYED".into(),
                        t: 2.0,
                        color: NEST,
                    });
                }
                Event::WaveStart(n) => {
                    let boss = if n.is_multiple_of(4) {
                        " - a Matriarch stirs!"
                    } else {
                        ""
                    };
                    self.message = Some((format!("Wave {n} incoming{boss}"), 3.0));
                }
                Event::WaveCleared(n) => {
                    self.message = Some((
                        format!("Wave {n} cleared! Loot collected. Build phase."),
                        3.5,
                    ))
                }
                Event::CommanderHurt => self.shake = self.shake.max(0.15),
                Event::LevelUp => {
                    let p = self.game.cmd.pos;
                    self.fx.push(Fx::Text {
                        p,
                        s: "LEVEL UP".into(),
                        t: 1.2,
                        color: GOLD,
                    });
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::M) {
            self.audio.toggle_mute();
        }
        match self.screen {
            Screen::Title => {
                if get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) {
                    self.audio.start_music();
                    self.audio.play(Sfx::Click);
                    self.screen = Screen::Play;
                    self.restart();
                }
                return;
            }
            Screen::Play => {}
        }
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
        self.shake = (self.shake - dt).max(0.0);
        for f in &mut self.fx {
            match f {
                Fx::Beam { t, .. } | Fx::Spark { t, .. } => *t -= dt,
                Fx::Text { p, t, .. } => {
                    *t -= dt;
                    p.y -= 30.0 * dt;
                }
                Fx::Blast { r, t, max, .. } => {
                    *t += dt;
                    *r = *max * (*t / 0.35).min(1.0);
                }
            }
        }
        self.fx.retain(|f| match f {
            Fx::Beam { t, .. } | Fx::Spark { t, .. } | Fx::Text { t, .. } => *t > 0.0,
            Fx::Blast { t, .. } => *t < 0.45,
        });

        match self.game.phase {
            Phase::Won | Phase::Lost => {
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::R) {
                    self.restart();
                }
                return;
            }
            Phase::LevelUp => {
                for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                    .into_iter()
                    .enumerate()
                {
                    if is_key_pressed(k) {
                        self.game.choose_upgrade(i);
                        self.audio.play(Sfx::Click);
                    }
                }
                if is_mouse_button_pressed(MouseButton::Left)
                    && let Some(i) = self.card_at(mouse_position().into())
                {
                    self.game.choose_upgrade(i);
                    self.audio.play(Sfx::Click);
                }
                self.handle_events();
                return;
            }
            _ => {}
        }
        if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
            self.paused = !self.paused;
        }
        if self.paused {
            return;
        }

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
        let mut ready = false;
        if let Phase::Build { .. } = self.game.phase {
            let keys = [
                KeyCode::Key1,
                KeyCode::Key2,
                KeyCode::Key3,
                KeyCode::Key4,
                KeyCode::Key5,
            ];
            for (k, kind) in keys.into_iter().zip(TowerKind::ALL) {
                if is_key_pressed(k) {
                    self.selected = kind;
                    self.audio.play(Sfx::Click);
                }
            }
            let (mx, my) = mouse_position();
            if is_mouse_button_pressed(MouseButton::Left) {
                if let Some(kind) = self.palette_at(vec2(mx, my)) {
                    self.selected = kind;
                    self.audio.play(Sfx::Click);
                } else {
                    let p = self.cam() + V2::new(mx, my);
                    if let Some(err) = self.game.placement_error(self.selected, p) {
                        self.message = Some((err.into(), 1.5));
                    }
                    self.game.build(self.selected, p);
                }
            }
            ready = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space);
        }
        self.game.update(dt.min(0.05), Input { mv, ready });
        self.handle_events();
    }

    fn palette_rect(i: usize) -> Rect {
        let w = 150.0;
        let x0 = screen_width() / 2.0 - w * 2.5;
        Rect::new(x0 + i as f32 * w, screen_height() - 70.0, w - 8.0, 60.0)
    }

    fn palette_at(&self, p: Vec2) -> Option<TowerKind> {
        (0..5)
            .find(|&i| Self::palette_rect(i).contains(p))
            .map(|i| TowerKind::ALL[i])
    }

    fn card_rect(i: usize) -> Rect {
        let w = 300.0;
        let x0 = screen_width() / 2.0 - w * 1.5;
        Rect::new(
            x0 + i as f32 * w + 10.0,
            screen_height() / 2.0 - 60.0,
            w - 20.0,
            130.0,
        )
    }

    fn card_at(&self, p: Vec2) -> Option<usize> {
        (0..self.game.choices.len()).find(|&i| Self::card_rect(i).contains(p))
    }

    fn draw(&self) {
        clear_background(BG);
        match self.screen {
            Screen::Title => self.draw_title(),
            Screen::Play => {
                self.draw_world();
                self.draw_hud();
            }
        }
    }

    fn draw_title(&self) {
        let (w, h) = (screen_width(), screen_height());
        let t = get_time() as f32;
        for i in 0..6 {
            let a = t * 0.2 + i as f32;
            let p = vec2(w / 2.0 + a.cos() * 380.0, h / 2.0 + a.sin() * 260.0);
            draw_nest(p, 40.0, t + i as f32, 1.0);
        }
        center_text("RAPTOR NEST", h * 0.28, 84.0, NEST);
        center_text(
            "Build towers in the calm. Fight in person when the raptors come.",
            h * 0.28 + 50.0,
            24.0,
            WHITE,
        );
        let lines = [
            "BUILD PHASE: 1-5 pick tower, mouse click to place, Enter = ready",
            "WAVE PHASE: WASD to move, the Commander fires automatically",
            "Towers extend your build range - push out and burn every nest",
            "Protect the Core. M = mute, P = pause",
        ];
        for (i, l) in lines.iter().enumerate() {
            center_text(l, h * 0.5 + i as f32 * 30.0, 22.0, LIGHTGRAY);
        }
        let blink = if (t * 2.0) as i32 % 2 == 0 {
            GOLD
        } else {
            YELLOW
        };
        center_text("PRESS ANY KEY TO START", h * 0.78, 34.0, blink);
        center_text(
            "Homage to Beyond All Reason - procedural art and sound",
            h - 24.0,
            16.0,
            GRAY,
        );
    }

    fn draw_world(&self) {
        let g = &self.game;
        let cam = self.cam();
        let o = |p: V2| v(p - cam);
        let (w, h) = (screen_width(), screen_height());
        let t = get_time() as f32;

        // Ground grid.
        let step = 80.0;
        let gx0 = (cam.x / step).floor() * step;
        let gy0 = (cam.y / step).floor() * step;
        let grid = Color::new(0.12, 0.17, 0.11, 1.0);
        let mut x = gx0;
        while x < cam.x + w + step {
            draw_line(x - cam.x, 0.0, x - cam.x, h, 1.0, grid);
            x += step;
        }
        let mut y = gy0;
        while y < cam.y + h + step {
            draw_line(0.0, y - cam.y, w, y - cam.y, 1.0, grid);
            y += step;
        }
        draw_rectangle_lines(-cam.x, -cam.y, WORLD, WORLD, 4.0, DARKGRAY);

        let building = matches!(g.phase, Phase::Build { .. });
        // Build range zones.
        if building {
            let zone = Color::new(0.3, 0.6, 1.0, 0.06);
            let c = o(CORE_POS);
            draw_circle(c.x, c.y, 320.0, zone);
            for tw in &g.towers {
                let p = o(tw.pos);
                draw_circle(p.x, p.y, build_radius(tw.kind), zone);
            }
        }

        // Nests.
        for (i, n) in g.nests.iter().enumerate() {
            let p = o(n.pos);
            if n.alive {
                draw_nest(p, 46.0, t + i as f32, 1.0);
                bar(p + vec2(-40.0, -62.0), 80.0, n.hp / g.nest_max_hp(), NEST);
            } else {
                draw_circle(p.x, p.y, 40.0, Color::new(0.15, 0.12, 0.1, 1.0));
                draw_circle_lines(p.x, p.y, 40.0, 2.0, Color::new(0.3, 0.2, 0.1, 1.0));
            }
        }

        // Core.
        let c = o(CORE_POS);
        draw_poly(
            c.x,
            c.y,
            6,
            44.0,
            t * 10.0,
            Color::new(0.15, 0.25, 0.4, 1.0),
        );
        draw_poly_lines(c.x, c.y, 6, 44.0, t * 10.0, 3.0, BLUE);
        let pulse = 14.0 + (t * 3.0).sin() * 3.0;
        draw_circle(c.x, c.y, pulse, Color::new(0.5, 0.8, 1.0, 0.9));
        bar(
            c + vec2(-45.0, -60.0),
            90.0,
            g.core_hp / g.core_max_hp(),
            BLUE,
        );

        // Towers.
        for tw in &g.towers {
            let p = o(tw.pos);
            draw_tower(tw.kind, p, tw.fired, t);
            if tw.hp < tw.kind.max_hp() {
                bar(
                    p + vec2(-18.0, -30.0),
                    36.0,
                    tw.hp / tw.kind.max_hp(),
                    GREEN,
                );
            }
        }

        // Pickups.
        for pk in &g.pickups {
            let p = o(pk.pos);
            let s = 4.0 + (pk.value as f32).sqrt();
            draw_poly(p.x, p.y, 4, s, t * 90.0, Color::new(0.75, 0.8, 0.9, 1.0));
        }

        // Raptors.
        for r in &g.raptors {
            draw_raptor(r.kind, o(r.pos), r.radius, r.facing, r.flash > 0.0, t);
            if r.kind == RaptorKind::Matriarch || r.kind == RaptorKind::Brute {
                let p = o(r.pos);
                bar(
                    p + vec2(-r.radius, -r.radius - 10.0),
                    r.radius * 2.0,
                    r.hp / r.max_hp,
                    ACID,
                );
            }
        }

        // Shots.
        for s in &g.shots {
            let p = o(s.pos);
            if s.hostile {
                draw_circle(p.x, p.y, 5.0, ACID);
            } else if s.splash > 0.0 {
                let tail = p - v(s.vel.norm()) * 14.0;
                draw_line(p.x, p.y, tail.x, tail.y, 4.0, ORANGE);
                draw_circle(p.x, p.y, 3.5, WHITE);
            } else {
                let tail = p - v(s.vel.norm()) * 10.0;
                draw_line(
                    p.x,
                    p.y,
                    tail.x,
                    tail.y,
                    3.0,
                    Color::new(0.6, 0.9, 1.0, 1.0),
                );
            }
        }

        // Commander.
        let cp = o(g.cmd.pos);
        let hurt = g.cmd.hurt > 0.15;
        let body = if hurt { RED } else { BLUE };
        draw_circle(cp.x, cp.y, 16.0, Color::new(0.1, 0.15, 0.25, 1.0));
        draw_circle_lines(cp.x, cp.y, 16.0, 3.0, body);
        let d = vec2(g.cmd.facing.cos(), g.cmd.facing.sin());
        draw_line(cp.x, cp.y, cp.x + d.x * 24.0, cp.y + d.y * 24.0, 5.0, body);
        draw_circle(cp.x, cp.y, 6.0, WHITE);
        bar(
            cp + vec2(-20.0, -28.0),
            40.0,
            g.cmd.hp / g.cmd.max_hp,
            GREEN,
        );

        // Effects.
        for f in &self.fx {
            match f {
                Fx::Beam { a, b, heavy, t } => {
                    let (a, b) = (o(*a), o(*b));
                    let (wd, col) = if *heavy {
                        (5.0, Color::new(1.0, 0.4, 0.3, t * 5.0))
                    } else {
                        (2.0, Color::new(1.0, 0.9, 0.3, t * 10.0))
                    };
                    draw_line(a.x, a.y, b.x, b.y, wd, col);
                }
                Fx::Blast { p, r, t, color, .. } => {
                    let p = o(*p);
                    let alpha = (1.0 - t / 0.45).max(0.0);
                    let mut c = *color;
                    c.a = alpha * 0.4;
                    draw_circle(p.x, p.y, *r, c);
                    c.a = alpha;
                    draw_circle_lines(p.x, p.y, *r, 3.0, c);
                }
                Fx::Spark { p, t } => {
                    let p = o(*p);
                    draw_circle(p.x, p.y, 3.0 + t * 20.0, Color::new(1.0, 1.0, 0.7, t * 6.0));
                }
                Fx::Text { p, s, t, color } => {
                    let p = o(*p);
                    let mut c = *color;
                    c.a = t.min(1.0);
                    let dim = measure_text(s, None, 26, 1.0);
                    draw_text(s, p.x - dim.width / 2.0, p.y - 40.0, 26.0, c);
                }
            }
        }

        // Placement preview.
        if building {
            let (mx, my) = mouse_position();
            if self.palette_at(vec2(mx, my)).is_none() {
                let wp = cam + V2::new(mx, my);
                let ok = g.placement_error(self.selected, wp).is_none();
                let col = if ok {
                    Color::new(0.3, 1.0, 0.4, 0.7)
                } else {
                    Color::new(1.0, 0.3, 0.3, 0.7)
                };
                let r = self.selected.range();
                if r > 0.0 {
                    draw_circle_lines(mx, my, r, 1.5, col);
                }
                draw_circle(mx, my, self.selected.radius(), col);
            }
        }
    }

    fn draw_hud(&self) {
        let g = &self.game;
        let (w, h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, w, 44.0, PANEL);
        let phase = match g.phase {
            Phase::Build { timer } => format!("BUILD {:>2.0}s  (Enter = ready)", timer.ceil()),
            Phase::Wave => format!(
                "WAVE {}  raptors {}",
                g.wave,
                g.raptors.len() + g.to_spawn as usize
            ),
            Phase::LevelUp => "LEVEL UP".into(),
            Phase::Won => "VICTORY".into(),
            Phase::Lost => "DEFEAT".into(),
        };
        draw_text(&phase, 12.0, 28.0, 26.0, WHITE);
        draw_text(format!("Metal {}", g.metal), 380.0, 28.0, 26.0, LIGHTGRAY);
        draw_text(
            format!("Nests {}/{}", g.nests_alive(), g.nests.len()),
            520.0,
            28.0,
            26.0,
            NEST,
        );
        draw_text("Core", 680.0, 20.0, 18.0, BLUE);
        bar(vec2(680.0, 26.0), 140.0, g.core_hp / g.core_max_hp(), BLUE);
        draw_text("Commander", 840.0, 20.0, 18.0, GREEN);
        bar(vec2(840.0, 26.0), 140.0, g.cmd.hp / g.cmd.max_hp, GREEN);
        draw_text(format!("Lv {}", g.level), 1000.0, 20.0, 18.0, GOLD);
        bar(
            vec2(1000.0, 26.0),
            120.0,
            g.xp as f32 / g.xp_needed() as f32,
            GOLD,
        );
        let mute = if self.audio.muted {
            "M: unmute"
        } else {
            "M: mute"
        };
        draw_text(mute, w - 90.0, 28.0, 18.0, GRAY);

        // Phase timer bar.
        if let Phase::Build { timer } = g.phase {
            draw_rectangle(0.0, 44.0, w * timer / BUILD_TIME, 4.0, BLUE);
        }

        self.draw_minimap();

        if let Phase::Build { .. } = g.phase {
            for (i, kind) in TowerKind::ALL.into_iter().enumerate() {
                let r = Self::palette_rect(i);
                let sel = kind == self.selected;
                let afford = g.metal >= kind.cost();
                draw_rectangle(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    if sel {
                        Color::new(0.2, 0.35, 0.6, 0.9)
                    } else {
                        PANEL
                    },
                );
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if sel { WHITE } else { DARKGRAY });
                draw_tower(kind, vec2(r.x + 24.0, r.y + 30.0), 10.0, 0.0);
                let col = if afford { WHITE } else { GRAY };
                draw_text(
                    format!("{} {}", i + 1, kind.name()),
                    r.x + 46.0,
                    r.y + 24.0,
                    16.0,
                    col,
                );
                draw_text(
                    format!("{} metal", kind.cost()),
                    r.x + 46.0,
                    r.y + 44.0,
                    16.0,
                    if afford { GOLD } else { RED },
                );
            }
        }

        if let Some((m, t)) = &self.message {
            let mut c = WHITE;
            c.a = t.min(1.0);
            center_text(m, 90.0, 28.0, c);
        }

        if self.paused {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
            center_text("PAUSED - press P", h / 2.0, 40.0, WHITE);
        }

        match g.phase {
            Phase::LevelUp => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
                center_text(
                    "LEVEL UP - choose an upgrade (1-3 or click)",
                    h / 2.0 - 100.0,
                    32.0,
                    GOLD,
                );
                for (i, u) in g.choices.iter().enumerate() {
                    let r = Self::card_rect(i);
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.12, 0.2, 0.95));
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
                    let label = u.label();
                    let (title, rest) = label.split_once(": ").unwrap_or((label, ""));
                    draw_text(
                        format!("{}. {title}", i + 1),
                        r.x + 12.0,
                        r.y + 40.0,
                        26.0,
                        WHITE,
                    );
                    draw_text(rest, r.x + 12.0, r.y + 80.0, 18.0, LIGHTGRAY);
                }
            }
            Phase::Won | Phase::Lost => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
                let (title, col) = if g.phase == Phase::Won {
                    ("ALL NESTS DESTROYED - VICTORY", GOLD)
                } else if g.core_hp <= 0.0 {
                    ("THE CORE HAS FALLEN", RED)
                } else {
                    ("COMMANDER DOWN", RED)
                };
                center_text(title, h / 2.0 - 60.0, 54.0, col);
                center_text(
                    &format!(
                        "Waves survived {}   Raptors slain {}   Level {}   Nests burned {}",
                        g.wave,
                        g.kills,
                        g.level,
                        g.nests.len() - g.nests_alive()
                    ),
                    h / 2.0,
                    24.0,
                    WHITE,
                );
                center_text("Press Enter to play again", h / 2.0 + 50.0, 28.0, LIGHTGRAY);
            }
            _ => {}
        }
    }

    fn draw_minimap(&self) {
        let g = &self.game;
        let size = 170.0;
        let x0 = screen_width() - size - 10.0;
        let y0 = 54.0;
        let k = size / WORLD;
        let m = |p: V2| vec2(x0 + p.x * k, y0 + p.y * k);
        draw_rectangle(x0, y0, size, size, PANEL);
        draw_rectangle_lines(x0, y0, size, size, 1.0, DARKGRAY);
        for n in g.nests.iter().filter(|n| n.alive) {
            let p = m(n.pos);
            draw_circle(p.x, p.y, 5.0, NEST);
        }
        for tw in &g.towers {
            let p = m(tw.pos);
            draw_rectangle(p.x - 1.5, p.y - 1.5, 3.0, 3.0, LIGHTGRAY);
        }
        for r in &g.raptors {
            let p = m(r.pos);
            draw_rectangle(p.x - 1.0, p.y - 1.0, 2.0, 2.0, ACID);
        }
        let c = m(CORE_POS);
        draw_circle(c.x, c.y, 4.0, BLUE);
        let p = m(g.cmd.pos);
        draw_circle(p.x, p.y, 3.0, WHITE);
    }
}

fn center_text(s: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (screen_width() - d.width) / 2.0, y, size, color);
}

fn bar(p: Vec2, w: f32, frac: f32, color: Color) {
    draw_rectangle(p.x, p.y, w, 5.0, Color::new(0.0, 0.0, 0.0, 0.7));
    draw_rectangle(p.x, p.y, w * frac.clamp(0.0, 1.0), 5.0, color);
}

fn draw_nest(p: Vec2, r: f32, t: f32, alpha: f32) {
    let glow = 0.5 + 0.5 * (t * 2.5).sin();
    draw_circle(
        p.x,
        p.y,
        r * (1.3 + glow * 0.15),
        Color::new(1.0, 0.4, 0.1, 0.12 * alpha),
    );
    for i in 0..9 {
        let a = i as f32 / 9.0 * std::f32::consts::TAU + t * 0.1;
        let tip = p + vec2(a.cos(), a.sin()) * r * 1.15;
        let l = p + vec2((a - 0.3).cos(), (a - 0.3).sin()) * r * 0.7;
        let rr = p + vec2((a + 0.3).cos(), (a + 0.3).sin()) * r * 0.7;
        draw_triangle(tip, l, rr, Color::new(0.35, 0.18, 0.1, alpha));
    }
    draw_circle(p.x, p.y, r * 0.75, Color::new(0.3, 0.12, 0.08, alpha));
    draw_circle(
        p.x,
        p.y,
        r * (0.35 + glow * 0.1),
        Color::new(1.0, 0.5 + glow * 0.3, 0.1, alpha),
    );
}

fn draw_raptor(kind: RaptorKind, p: Vec2, r: f32, facing: f32, flash: bool, t: f32) {
    let col = if flash {
        WHITE
    } else {
        match kind {
            RaptorKind::Swarmer => Color::new(0.75, 0.55, 0.25, 1.0),
            RaptorKind::Brute => Color::new(0.55, 0.3, 0.2, 1.0),
            RaptorKind::Spitter => Color::new(0.45, 0.75, 0.25, 1.0),
            RaptorKind::Matriarch => Color::new(0.7, 0.15, 0.35, 1.0),
        }
    };
    let d = vec2(facing.cos(), facing.sin());
    let n = vec2(-d.y, d.x);
    let wob = (t * 14.0 + p.x * 0.05).sin() * 0.25;
    // Tail.
    let tail = p - d * r * 2.0 + n * r * wob;
    draw_triangle(p + n * r * 0.5, p - n * r * 0.5, tail, col);
    // Body.
    draw_circle(p.x, p.y, r, col);
    // Head.
    let head = p + d * r * 1.7;
    draw_triangle(
        head,
        p + d * r * 0.6 + n * r * 0.6,
        p + d * r * 0.6 - n * r * 0.6,
        col,
    );
    let eye = p + d * r * 0.9 + n * r * 0.25;
    draw_circle(eye.x, eye.y, (r * 0.18).max(1.5), YELLOW);
}

fn draw_tower(kind: TowerKind, p: Vec2, fired: f32, t: f32) {
    let r = kind.radius();
    let base = Color::new(0.3, 0.33, 0.38, 1.0);
    match kind {
        TowerKind::Wall => {
            draw_poly(p.x, p.y, 4, r, 45.0, Color::new(0.4, 0.4, 0.42, 1.0));
            draw_poly_lines(p.x, p.y, 4, r, 45.0, 2.0, DARKGRAY);
        }
        TowerKind::Nano => {
            draw_poly(p.x, p.y, 8, r, 0.0, base);
            let a = t * 2.0;
            for k in 0..3 {
                let aa = a + k as f32 * 2.094;
                let q = p + vec2(aa.cos(), aa.sin()) * r * 0.6;
                draw_circle(q.x, q.y, 3.5, GREEN);
            }
        }
        _ => {
            draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, base);
            draw_rectangle_lines(p.x - r, p.y - r, r * 2.0, r * 2.0, 2.0, DARKGRAY);
            let col = match kind {
                TowerKind::Llt => YELLOW,
                TowerKind::Hlt => RED,
                _ => ORANGE,
            };
            draw_circle(p.x, p.y, r * 0.55, col);
            if fired < 0.1 {
                draw_circle(p.x, p.y, r * 0.8, Color::new(1.0, 1.0, 0.8, 0.6));
            }
            if kind == TowerKind::Rocket {
                for k in [-1.0, 1.0] {
                    draw_rectangle(p.x + k * r * 0.5 - 2.0, p.y - r * 0.9, 4.0, r * 0.8, WHITE);
                }
            }
        }
    }
}

#[macroquad::main(conf)]
async fn main() {
    let audio = Audio::new().await;
    let mut app = App {
        game: Game::new(1),
        audio,
        screen: Screen::Title,
        fx: Vec::new(),
        selected: TowerKind::Llt,
        message: None,
        shake: 0.0,
        seed: (macroquad::miniquad::date::now() * 1000.0) as u64,
        paused: false,
    };
    loop {
        app.update(get_frame_time());
        app.draw();
        next_frame().await;
    }
}
