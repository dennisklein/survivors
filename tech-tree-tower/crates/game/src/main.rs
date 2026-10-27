//! Tech Tree Tower: macroquad front end. Rendering, input and audio only;
//! all rules live in the `sim` crate.

mod audio;

use audio::{Audio, Sfx};
use macroquad::prelude::*;
use sim::game::{
    CELL, Card, EnemyKind, Event, GRID, Game, Input, Kind, Phase, RUN_TIME, WAVES, WORLD,
    can_place, cell_at, cell_center, cells_of, grid_origin, neighbors,
};
use sim::math::V2;

const BG: Color = Color::new(0.08, 0.09, 0.08, 1.0);
const PANEL: Color = Color::new(0.0, 0.0, 0.0, 0.7);
const BLUE: Color = Color::new(0.35, 0.65, 1.0, 1.0);
const ENEMY: Color = Color::new(0.95, 0.3, 0.2, 1.0);
const METAL: Color = Color::new(0.55, 0.95, 0.45, 1.0);
const GOLD: Color = Color::new(1.0, 0.82, 0.3, 1.0);

fn conf() -> Conf {
    Conf {
        window_title: "Tech Tree Tower".to_owned(),
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
        t: f32,
        max: f32,
        color: Color,
    },
    Text {
        p: V2,
        s: String,
        t: f32,
        color: Color,
    },
}

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
}

struct App {
    game: Game,
    audio: Audio,
    screen: Screen,
    paused: bool,
    fx: Vec<Fx>,
    cam: Vec2,
    shake: f32,
    banner: Option<(String, f32)>,
    seed: u64,
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Core => Color::new(0.3, 0.55, 0.95, 1.0),
        Kind::Mex => Color::new(0.6, 0.6, 0.62, 1.0),
        Kind::Solar => Color::new(0.25, 0.45, 0.85, 1.0),
        Kind::Wind => Color::new(0.75, 0.85, 0.9, 1.0),
        Kind::Fusion => Color::new(0.3, 0.95, 0.5, 1.0),
        Kind::Storage => Color::new(0.85, 0.7, 0.3, 1.0),
        Kind::Llt => Color::new(0.95, 0.45, 0.35, 1.0),
        Kind::Hlt => Color::new(0.85, 0.25, 0.25, 1.0),
        Kind::Rocket => Color::new(0.95, 0.6, 0.2, 1.0),
        Kind::Nano => Color::new(0.4, 0.9, 0.4, 1.0),
        Kind::Radar => Color::new(0.6, 0.8, 1.0, 1.0),
        Kind::Wall => Color::new(0.45, 0.45, 0.42, 1.0),
    }
}

/// Whether the link a-b carries a synergy worth drawing.
fn synergy_link(a: Kind, b: Kind) -> Option<Color> {
    let one = |x: Kind, y: Kind| -> Option<Color> {
        if y.is_turret() && x.energy() > 0.0 || y == Kind::Mex && x.energy() > 0.0 {
            Some(GOLD)
        } else if x == Kind::Nano && (y.is_turret()) {
            Some(METAL)
        } else if x == Kind::Radar && y.is_turret() {
            Some(BLUE)
        } else if x == Kind::Storage && y == Kind::Mex {
            Some(Color::new(0.9, 0.7, 0.3, 1.0))
        } else if x == Kind::Wall && y.is_turret() {
            Some(GRAY)
        } else {
            None
        }
    };
    one(a, b).or_else(|| one(b, a))
}

impl App {
    async fn new() -> Self {
        App {
            game: Game::new(1),
            audio: Audio::new().await,
            screen: Screen::Title,
            paused: false,
            fx: Vec::new(),
            cam: vec2(WORLD / 2.0, WORLD / 2.0),
            shake: 0.0,
            banner: None,
            seed: 1,
        }
    }

    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add((get_time() * 1000.0) as u64 + 1);
        self.game = Game::new(self.seed);
        self.fx.clear();
        self.paused = false;
        self.banner = Some(("Survive 20 waves. Build synergies!".into(), 3.0));
    }

    fn mouse_world(&self) -> V2 {
        let (mx, my) = mouse_position();
        let o = self.offset();
        V2::new(mx + o.x, my + o.y)
    }

    fn offset(&self) -> Vec2 {
        let s = if self.shake > 0.0 {
            vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * self.shake * 10.0
        } else {
            Vec2::ZERO
        };
        self.cam - vec2(screen_width(), screen_height()) / 2.0 + s
    }

    fn update(&mut self) {
        let dt = get_frame_time().min(1.0 / 20.0);
        let any_input = get_last_key_pressed().is_some()
            || is_mouse_button_pressed(MouseButton::Left)
            || is_mouse_button_pressed(MouseButton::Right);
        if is_key_pressed(KeyCode::M) {
            self.audio.toggle_mute();
        }
        if any_input {
            self.audio.start_music();
        }
        match self.screen {
            Screen::Title => {
                if any_input && !is_key_pressed(KeyCode::M) {
                    self.screen = Screen::Play;
                    self.audio.play(Sfx::Click);
                    self.restart();
                }
            }
            Screen::Play => self.update_play(dt),
        }
        self.shake = (self.shake - dt * 2.5).max(0.0);
        for f in &mut self.fx {
            match f {
                Fx::Beam { t, .. } | Fx::Blast { t, .. } | Fx::Text { t, .. } => *t -= dt,
            }
        }
        self.fx.retain(|f| match f {
            Fx::Beam { t, .. } | Fx::Blast { t, .. } | Fx::Text { t, .. } => *t > 0.0,
        });
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
    }

    fn update_play(&mut self, dt: f32) {
        match self.game.phase.clone() {
            Phase::Won | Phase::Lost => {
                if is_key_pressed(KeyCode::Enter)
                    || is_key_pressed(KeyCode::Space)
                    || is_mouse_button_pressed(MouseButton::Left)
                {
                    self.audio.play(Sfx::Click);
                    self.restart();
                }
            }
            Phase::Choosing(_) => {
                let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3];
                for (i, k) in keys.iter().enumerate() {
                    if is_key_pressed(*k) {
                        self.audio.play(Sfx::Click);
                        self.game.choose(i);
                    }
                }
                if is_mouse_button_pressed(MouseButton::Left)
                    && let Some(i) = self.card_under_mouse()
                {
                    self.audio.play(Sfx::Click);
                    self.game.choose(i);
                }
            }
            Phase::Placing { .. } => {
                if is_key_pressed(KeyCode::R) || is_mouse_button_pressed(MouseButton::Right) {
                    self.audio.play(Sfx::Click);
                    self.game.rotate_ghost();
                }
                if is_mouse_button_pressed(MouseButton::Left) {
                    let at = cell_at(self.mouse_world());
                    if self.game.place(at)
                        && let Some(b) = self.game.buildings.last()
                    {
                        let c = b.center();
                        self.fx.push(Fx::Blast {
                            p: c,
                            t: 0.4,
                            max: 0.4,
                            color: METAL,
                        });
                        for n in b.stats.notes.clone() {
                            self.text(c, n, GOLD);
                        }
                    }
                }
                let target = cell_center((GRID / 2, GRID / 2));
                self.cam = self.cam.lerp(v(target), (dt * 6.0).min(1.0));
            }
            Phase::Playing => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    self.paused = !self.paused;
                }
                if !self.paused {
                    let mut d = V2::ZERO;
                    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
                        d.y -= 1.0;
                    }
                    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
                        d.y += 1.0;
                    }
                    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
                        d.x -= 1.0;
                    }
                    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
                        d.x += 1.0;
                    }
                    self.game.step(dt, Input { move_dir: d });
                    let c = v(self.game.commander.pos);
                    self.cam = self.cam.lerp(c, (dt * 8.0).min(1.0));
                }
            }
        }
        for e in self.game.drain_events() {
            self.audio.on_event(&e);
            match e {
                Event::Beam { a, b, heavy } => self.fx.push(Fx::Beam {
                    a,
                    b,
                    heavy,
                    t: if heavy { 0.18 } else { 0.08 },
                }),
                Event::Kill { pos, big } => {
                    self.fx.push(Fx::Blast {
                        p: pos,
                        t: 0.35,
                        max: if big { 70.0 } else { 26.0 },
                        color: Color::new(1.0, 0.6, 0.2, 1.0),
                    });
                    if big {
                        self.shake = self.shake.max(0.5);
                    }
                }
                Event::Explosion { pos, r } => self.fx.push(Fx::Blast {
                    p: pos,
                    t: 0.3,
                    max: r,
                    color: Color::new(1.0, 0.75, 0.3, 1.0),
                }),
                Event::BuildingLost(p) => {
                    self.shake = 1.0;
                    self.fx.push(Fx::Blast {
                        p,
                        t: 0.6,
                        max: 90.0,
                        color: ENEMY,
                    });
                    self.text(p, "Building lost!".into(), ENEMY);
                }
                Event::WaveStart(n) if n > 1 => {
                    self.banner = Some((format!("Wave {n} / {WAVES}"), 2.0));
                }
                Event::BossSpawn => {
                    self.banner = Some(("A Juggernaut approaches!".into(), 3.0));
                }
                Event::CommanderHurt => self.shake = self.shake.max(0.25),
                _ => {}
            }
        }
    }

    fn text(&mut self, p: V2, s: String, color: Color) {
        let k = self
            .fx
            .iter()
            .filter(|f| matches!(f, Fx::Text { .. }))
            .count() as f32;
        self.fx.push(Fx::Text {
            p: V2::new(p.x, p.y - (k % 4.0) * 16.0),
            s,
            t: 1.6,
            color,
        });
    }

    fn card_rects(&self) -> [Rect; 3] {
        let w = (screen_width() - 80.0).min(900.0) / 3.0;
        let h = 230.0;
        let x0 = screen_width() / 2.0 - w * 1.5;
        let y = screen_height() / 2.0 - h / 2.0 + 20.0;
        [0, 1, 2].map(|i| Rect::new(x0 + i as f32 * w + 8.0, y, w - 16.0, h))
    }

    fn card_under_mouse(&self) -> Option<usize> {
        let (mx, my) = mouse_position();
        self.card_rects()
            .iter()
            .position(|r| r.contains(vec2(mx, my)))
    }

    // ---------------------------------------------------------------- drawing

    fn draw(&self) {
        clear_background(BG);
        if self.screen == Screen::Title {
            self.draw_title();
            return;
        }
        let o = self.offset();
        let sp = |p: V2| vec2(p.x - o.x, p.y - o.y);
        self.draw_ground(o);
        self.draw_buildings(o);
        // Pickups.
        for p in &self.game.pickups {
            let s = sp(p.pos);
            if p.heal {
                draw_rectangle(s.x - 6.0, s.y - 2.0, 12.0, 4.0, METAL);
                draw_rectangle(s.x - 2.0, s.y - 6.0, 4.0, 12.0, METAL);
            } else {
                draw_poly(s.x, s.y, 4, 4.0, 45.0, Color::new(0.6, 0.8, 0.6, 1.0));
            }
        }
        // Enemies.
        for e in &self.game.enemies {
            let s = sp(e.pos);
            let col = if e.flash > 0.0 { WHITE } else { ENEMY };
            let dark = Color::new(0.35, 0.08, 0.06, 1.0);
            match e.kind {
                EnemyKind::Pawn => draw_poly(s.x, s.y, 3, e.radius, 90.0, col),
                EnemyKind::Raider => {
                    draw_poly(s.x, s.y, 3, e.radius, get_time() as f32 * 400.0, col)
                }
                EnemyKind::Grunt => {
                    draw_poly(s.x, s.y, 5, e.radius, 0.0, dark);
                    draw_poly(s.x, s.y, 5, e.radius - 3.0, 0.0, col);
                }
                EnemyKind::Tank => {
                    draw_rectangle(
                        s.x - e.radius,
                        s.y - e.radius,
                        e.radius * 2.0,
                        e.radius * 2.0,
                        dark,
                    );
                    draw_rectangle(
                        s.x - e.radius + 3.0,
                        s.y - e.radius + 3.0,
                        e.radius * 2.0 - 6.0,
                        e.radius * 2.0 - 6.0,
                        col,
                    );
                    draw_circle(s.x, s.y, 5.0, dark);
                }
                EnemyKind::Boss => {
                    draw_poly(s.x, s.y, 8, e.radius, 22.5, dark);
                    draw_poly(s.x, s.y, 8, e.radius - 5.0, 22.5, col);
                    draw_circle(s.x, s.y, 10.0, GOLD);
                }
            }
            if e.hp < e.max_hp && e.kind != EnemyKind::Pawn {
                let w = e.radius * 2.0;
                let y = s.y - e.radius - 7.0;
                draw_rectangle(s.x - w / 2.0, y, w, 3.0, BLACK);
                draw_rectangle(s.x - w / 2.0, y, w * e.hp / e.max_hp, 3.0, ENEMY);
            }
        }
        // Projectiles.
        for p in &self.game.projectiles {
            let s = sp(p.pos);
            if p.hostile {
                draw_circle(s.x, s.y, 6.0, Color::new(1.0, 0.4, 0.9, 1.0));
            } else if p.rocket {
                let tail = s - v(p.vel.norm()) * 12.0;
                draw_line(
                    tail.x,
                    tail.y,
                    s.x,
                    s.y,
                    3.0,
                    Color::new(1.0, 0.8, 0.4, 0.8),
                );
                draw_circle(s.x, s.y, 3.5, WHITE);
            } else {
                draw_circle(s.x, s.y, 5.0, Color::new(0.4, 0.8, 1.0, 0.5));
                draw_circle(s.x, s.y, 2.5, WHITE);
            }
        }
        // Commander.
        let c = &self.game.commander;
        let s = sp(c.pos);
        draw_circle(s.x, s.y, 17.0, Color::new(0.1, 0.2, 0.4, 1.0));
        draw_circle(s.x, s.y, 14.0, BLUE);
        draw_poly(s.x, s.y, 4, 8.0, get_time() as f32 * 90.0, WHITE);
        draw_circle_lines(s.x, s.y, 300.0, 1.0, Color::new(0.35, 0.65, 1.0, 0.08));
        // Effects.
        for f in &self.fx {
            match f {
                Fx::Beam { a, b, heavy, t } => {
                    let (a, b) = (sp(*a), sp(*b));
                    let col = if *heavy {
                        Color::new(1.0, 0.3, 0.3, (t * 6.0).min(1.0))
                    } else {
                        Color::new(1.0, 0.55, 0.4, (t * 12.0).min(1.0))
                    };
                    draw_line(a.x, a.y, b.x, b.y, if *heavy { 5.0 } else { 2.0 }, col);
                }
                Fx::Blast { p, t, max, color } => {
                    let s = sp(*p);
                    let k = 1.0 - t / 0.6;
                    let mut c = *color;
                    c.a = (t * 2.5).min(0.8);
                    draw_circle_lines(s.x, s.y, max * k.max(0.3), 3.0, c);
                    c.a *= 0.4;
                    draw_circle(s.x, s.y, max * 0.5 * k.max(0.3), c);
                }
                Fx::Text { p, s, t, color } => {
                    let q = sp(*p) + vec2(0.0, -(1.6 - t) * 30.0);
                    let mut c = *color;
                    c.a = t.min(1.0);
                    let w = measure_text(s, None, 18, 1.0).width;
                    draw_text(s, q.x - w / 2.0, q.y, 18.0, c);
                }
            }
        }
        self.draw_ghost(o);
        self.draw_hud();
        match &self.game.phase {
            Phase::Choosing(cards) => self.draw_cards(cards),
            Phase::Won | Phase::Lost => self.draw_end(),
            _ => {}
        }
        if self.paused {
            self.overlay("PAUSED", "P / Esc to resume");
        }
    }

    fn draw_ground(&self, o: Vec2) {
        let step = 80.0;
        let x0 = (o.x / step).floor() * step;
        let y0 = (o.y / step).floor() * step;
        let mut x = x0;
        while x < o.x + screen_width() + step {
            let mut y = y0;
            while y < o.y + screen_height() + step {
                if (0.0..=WORLD).contains(&x) && (0.0..=WORLD).contains(&y) {
                    draw_circle(x - o.x, y - o.y, 1.5, Color::new(0.2, 0.24, 0.2, 1.0));
                }
                y += step;
            }
            x += step;
        }
        draw_rectangle_lines(
            -o.x,
            -o.y,
            WORLD,
            WORLD,
            4.0,
            Color::new(0.5, 0.2, 0.2, 1.0),
        );
        let g = grid_origin();
        let size = GRID as f32 * CELL;
        draw_rectangle(
            g.x - o.x,
            g.y - o.y,
            size,
            size,
            Color::new(0.12, 0.14, 0.13, 1.0),
        );
        let line = Color::new(0.2, 0.24, 0.22, 1.0);
        for i in 0..=GRID {
            let d = i as f32 * CELL;
            draw_line(
                g.x - o.x + d,
                g.y - o.y,
                g.x - o.x + d,
                g.y - o.y + size,
                1.0,
                line,
            );
            draw_line(
                g.x - o.x,
                g.y - o.y + d,
                g.x - o.x + size,
                g.y - o.y + d,
                1.0,
                line,
            );
        }
    }

    fn draw_buildings(&self, o: Vec2) {
        let bs = &self.game.buildings;
        let t = get_time() as f32;
        // Synergy links first so buildings draw on top.
        for i in 0..bs.len() {
            for j in neighbors(bs, i) {
                if j < i {
                    continue;
                }
                if let Some(mut col) = synergy_link(bs[i].kind, bs[j].kind) {
                    col.a = 0.5 + 0.3 * (t * 4.0).sin();
                    let a = v(bs[i].center()) - o;
                    let b = v(bs[j].center()) - o;
                    draw_line(a.x, a.y, b.x, b.y, 6.0, col);
                }
            }
        }
        for b in bs {
            let col = kind_color(b.kind);
            for &c in &b.cells {
                let p = v(cell_center(c)) - o;
                let h = CELL / 2.0 - 3.0;
                draw_rectangle(
                    p.x - h,
                    p.y - h,
                    h * 2.0,
                    h * 2.0,
                    Color::new(0.16, 0.18, 0.18, 1.0),
                );
                draw_rectangle_lines(p.x - h, p.y - h, h * 2.0, h * 2.0, 2.0, col);
            }
            let p = v(b.center()) - o;
            draw_glyph(b.kind, p, t, &b.cells);
            if b.flash > 0.0 {
                draw_circle(p.x, p.y, 14.0, Color::new(1.0, 1.0, 1.0, 0.5));
            }
            let max = b.kind.max_hp();
            if b.hp < max {
                let w = 36.0;
                draw_rectangle(p.x - w / 2.0, p.y + 14.0, w, 4.0, BLACK);
                let f = (b.hp / max).clamp(0.0, 1.0);
                draw_rectangle(
                    p.x - w / 2.0,
                    p.y + 14.0,
                    w * f,
                    4.0,
                    Color::new(1.0 - f, f, 0.2, 1.0),
                );
            }
        }
    }

    fn draw_ghost(&self, o: Vec2) {
        let Phase::Placing { kind, rot } = self.game.phase else {
            return;
        };
        let at = cell_at(self.mouse_world());
        let cells = cells_of(kind, at, rot);
        let ok = can_place(&self.game.buildings, &cells).is_ok();
        let col = if ok {
            Color::new(0.3, 1.0, 0.4, 0.45)
        } else {
            Color::new(1.0, 0.3, 0.3, 0.45)
        };
        for &c in &cells {
            let p = v(cell_center(c)) - o;
            draw_rectangle(p.x - CELL / 2.0, p.y - CELL / 2.0, CELL, CELL, col);
        }
        let mut sum = V2::ZERO;
        for &c in &cells {
            sum += cell_center(c);
        }
        let center = v(sum * (1.0 / cells.len() as f32)) - o;
        draw_glyph(kind, center, get_time() as f32, &cells);
        if let Some((range, _, _)) = kind.weapon() {
            let st = self.game.preview(kind, &cells);
            draw_circle_lines(
                center.x,
                center.y,
                range * st.range,
                1.5,
                Color::new(1.0, 1.0, 1.0, 0.3),
            );
        }
        if ok {
            let st = self.game.preview(kind, &cells);
            let lines = if st.notes.is_empty() {
                vec!["no synergy here".to_string()]
            } else {
                st.notes
            };
            for (i, l) in lines.iter().enumerate() {
                draw_text(
                    l,
                    center.x + 30.0,
                    center.y - 10.0 + i as f32 * 18.0,
                    20.0,
                    GOLD,
                );
            }
        }
        let msg = format!(
            "Place {}  -  click to build, R / right-click to rotate",
            kind.name()
        );
        let w = measure_text(&msg, None, 24, 1.0).width;
        draw_rectangle(
            screen_width() / 2.0 - w / 2.0 - 12.0,
            52.0,
            w + 24.0,
            34.0,
            PANEL,
        );
        draw_text(&msg, screen_width() / 2.0 - w / 2.0, 76.0, 24.0, WHITE);
        let sub = kind.blurb();
        let w = measure_text(sub, None, 20, 1.0).width;
        draw_text(sub, screen_width() / 2.0 - w / 2.0, 104.0, 20.0, GOLD);
    }

    fn draw_hud(&self) {
        let g = &self.game;
        let sw = screen_width();
        draw_rectangle(0.0, 0.0, sw, 44.0, PANEL);
        // HP.
        let c = &g.commander;
        draw_text("HP", 12.0, 28.0, 22.0, WHITE);
        draw_rectangle(44.0, 12.0, 200.0, 18.0, Color::new(0.2, 0.05, 0.05, 1.0));
        draw_rectangle(
            44.0,
            12.0,
            200.0 * (c.hp / c.max_hp).clamp(0.0, 1.0),
            18.0,
            Color::new(0.3, 0.85, 0.35, 1.0),
        );
        draw_text(
            format!("{:.0}/{:.0}", c.hp.max(0.0), c.max_hp),
            52.0,
            26.0,
            18.0,
            BLACK,
        );
        // Core.
        if let Some(core) = g.buildings.iter().find(|b| b.kind == Kind::Core) {
            draw_text("CORE", 260.0, 28.0, 22.0, WHITE);
            draw_rectangle(316.0, 12.0, 120.0, 18.0, Color::new(0.1, 0.1, 0.25, 1.0));
            draw_rectangle(
                316.0,
                12.0,
                120.0 * core.hp / Kind::Core.max_hp(),
                18.0,
                BLUE,
            );
        }
        let left = (RUN_TIME - g.time).max(0.0);
        let info = format!(
            "Wave {}/{}   {:02}:{:02}   Lv {}   Kills {}   Buildings {}",
            g.wave.max(1),
            WAVES,
            (left / 60.0) as u32,
            (left % 60.0) as u32,
            g.level,
            g.kills,
            g.buildings.len()
        );
        draw_text(&info, 460.0, 28.0, 22.0, WHITE);
        let mute = if self.audio.muted {
            "[M] sound off"
        } else {
            "[M] sound on"
        };
        let w = measure_text(mute, None, 18, 1.0).width;
        draw_text(mute, sw - w - 12.0, 26.0, 18.0, GRAY);
        // XP bar.
        let f = (g.xp / g.xp_needed()).clamp(0.0, 1.0);
        draw_rectangle(0.0, 44.0, sw, 6.0, Color::new(0.1, 0.15, 0.1, 1.0));
        draw_rectangle(0.0, 44.0, sw * f, 6.0, METAL);
        let income: f32 = g.buildings.iter().map(|b| b.stats.income).sum();
        let sh = screen_height();
        draw_text(
            format!(
                "Metal income {income:.1} XP/s   WASD move  -  auto-fire  -  P pause  -  M mute"
            ),
            12.0,
            sh - 12.0,
            18.0,
            GRAY,
        );
        if let Some((s, t)) = &self.banner {
            let a = t.min(1.0);
            let w = measure_text(s, None, 40, 1.0).width;
            draw_text(
                s,
                sw / 2.0 - w / 2.0,
                130.0,
                40.0,
                Color::new(1.0, 0.85, 0.3, a),
            );
        }
    }

    fn draw_cards(&self, cards: &[Card; 3]) {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.55),
        );
        let title = format!("LEVEL {}  -  choose your next tech", self.game.level);
        let w = measure_text(&title, None, 40, 1.0).width;
        let rects = self.card_rects();
        draw_text(
            &title,
            screen_width() / 2.0 - w / 2.0,
            rects[0].y - 30.0,
            40.0,
            GOLD,
        );
        let hover = self.card_under_mouse();
        for (i, (card, r)) in cards.iter().zip(rects.iter()).enumerate() {
            let hot = hover == Some(i);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.12, 0.14, 0.95));
            let (name, blurb, col, kind) = match card {
                Card::Build(k) => (k.name(), k.blurb(), kind_color(*k), Some(*k)),
                Card::Up(u) => (u.name(), u.blurb(), BLUE, None),
            };
            draw_rectangle_lines(
                r.x,
                r.y,
                r.w,
                r.h,
                if hot { 4.0 } else { 2.0 },
                if hot { WHITE } else { col },
            );
            draw_text(format!("[{}]", i + 1), r.x + 10.0, r.y + 24.0, 20.0, GRAY);
            let tag = if kind.is_some() {
                "BUILDING"
            } else {
                "UPGRADE"
            };
            draw_text(tag, r.x + r.w - 90.0, r.y + 24.0, 18.0, col);
            let icon = vec2(r.x + r.w / 2.0, r.y + 70.0);
            if let Some(k) = kind {
                let cells: Vec<(i32, i32)> = k.footprint().to_vec();
                for &(dx, dy) in &cells {
                    let p = icon + vec2(dx as f32, dy as f32) * 30.0;
                    draw_rectangle_lines(p.x - 14.0, p.y - 14.0, 28.0, 28.0, 2.0, col);
                }
                draw_glyph(k, icon, get_time() as f32, &[(0, 0)]);
            } else {
                draw_poly(icon.x, icon.y, 6, 22.0, 0.0, col);
                draw_text("+", icon.x - 9.0, icon.y + 10.0, 36.0, WHITE);
            }
            wrap(name, r.x + 12.0, r.y + 128.0, r.w - 24.0, 24.0, WHITE);
            wrap(
                blurb,
                r.x + 12.0,
                r.y + 170.0,
                r.w - 24.0,
                19.0,
                Color::new(0.85, 0.85, 0.8, 1.0),
            );
        }
        let hint = "Click a card or press 1-3";
        let w = measure_text(hint, None, 20, 1.0).width;
        draw_text(
            hint,
            screen_width() / 2.0 - w / 2.0,
            rects[0].y + rects[0].h + 30.0,
            20.0,
            GRAY,
        );
    }

    fn overlay(&self, title: &str, sub: &str) {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        let w = measure_text(title, None, 64, 1.0).width;
        draw_text(
            title,
            screen_width() / 2.0 - w / 2.0,
            screen_height() / 2.0 - 20.0,
            64.0,
            GOLD,
        );
        let w = measure_text(sub, None, 24, 1.0).width;
        draw_text(
            sub,
            screen_width() / 2.0 - w / 2.0,
            screen_height() / 2.0 + 24.0,
            24.0,
            WHITE,
        );
    }

    fn draw_end(&self) {
        let g = &self.game;
        let won = g.phase == Phase::Won;
        let title = if won {
            "BASE HOLDS - VICTORY"
        } else {
            "COMMANDER LOST"
        };
        let sub = format!(
            "Wave {}  -  Level {}  -  {} kills  -  {} buildings   |   Enter / click to play again",
            g.wave,
            g.level,
            g.kills,
            g.buildings.len()
        );
        self.overlay(title, &sub);
    }

    fn draw_title(&self) {
        let (sw, sh) = (screen_width(), screen_height());
        let t = get_time() as f32;
        // Decorative mini base.
        let demo = [
            (Kind::Solar, -1, 0),
            (Kind::Llt, 0, 0),
            (Kind::Nano, 1, 0),
            (Kind::Mex, 0, 1),
            (Kind::Radar, 0, -1),
        ];
        for (k, x, y) in demo {
            let p = vec2(
                sw / 2.0 + x as f32 * 52.0,
                sh / 2.0 - 40.0 + y as f32 * 52.0,
            );
            draw_rectangle_lines(p.x - 23.0, p.y - 23.0, 46.0, 46.0, 2.0, kind_color(k));
            draw_glyph(k, p, t, &[(0, 0)]);
        }
        let title = "TECH TREE TOWER";
        let w = measure_text(title, None, 80, 1.0).width;
        draw_text(title, sw / 2.0 - w / 2.0, sh / 2.0 - 160.0, 80.0, GOLD);
        let lines = [
            "Every level-up is a building. Place it next to the right neighbours:",
            "energy powers turrets and mexes, nanos boost damage, radars add range, walls armor.",
            "Survive 20 escalating waves (10 minutes). Lose the Commander or the Core and it's over.",
            "",
            "WASD move  -  Commander auto-fires  -  mouse picks cards and places  -  R rotate",
            "P pause  -  M mute",
        ];
        for (i, l) in lines.iter().enumerate() {
            let w = measure_text(l, None, 22, 1.0).width;
            draw_text(
                l,
                sw / 2.0 - w / 2.0,
                sh / 2.0 + 60.0 + i as f32 * 26.0,
                22.0,
                WHITE,
            );
        }
        let a = 0.5 + 0.5 * (t * 3.0).sin();
        let p = "Press any key or click to start";
        let w = measure_text(p, None, 32, 1.0).width;
        draw_text(
            p,
            sw / 2.0 - w / 2.0,
            sh - 60.0,
            32.0,
            Color::new(1.0, 1.0, 1.0, a),
        );
    }
}

/// Draws the procedural icon of a building centred at `p`.
fn draw_glyph(k: Kind, p: Vec2, t: f32, cells: &[(i32, i32)]) {
    let col = kind_color(k);
    match k {
        Kind::Core => {
            draw_poly(p.x, p.y, 8, 34.0, 22.5, Color::new(0.1, 0.15, 0.3, 1.0));
            draw_poly_lines(p.x, p.y, 8, 34.0, 22.5, 3.0, col);
            draw_circle(p.x, p.y, 12.0 + 2.0 * (t * 3.0).sin(), BLUE);
        }
        Kind::Mex => {
            draw_circle(p.x, p.y, 14.0, Color::new(0.3, 0.3, 0.32, 1.0));
            for i in 0..4 {
                let a = t * 2.0 + i as f32 * std::f32::consts::FRAC_PI_2;
                let d = vec2(a.cos(), a.sin()) * 13.0;
                draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 3.0, col);
            }
            draw_circle(p.x, p.y, 4.0, METAL);
        }
        Kind::Solar => {
            draw_rectangle(p.x - 15.0, p.y - 15.0, 30.0, 30.0, col);
            for i in 1..3 {
                let d = -15.0 + i as f32 * 10.0;
                draw_line(p.x + d, p.y - 15.0, p.x + d, p.y + 15.0, 1.0, BLACK);
                draw_line(p.x - 15.0, p.y + d, p.x + 15.0, p.y + d, 1.0, BLACK);
            }
        }
        Kind::Wind => {
            for i in 0..3 {
                let a = t * 5.0 + i as f32 * 2.094;
                let d = vec2(a.cos(), a.sin()) * 16.0;
                draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 3.0, col);
            }
            draw_circle(p.x, p.y, 4.0, GRAY);
        }
        Kind::Fusion => {
            let pulse = 0.7 + 0.3 * (t * 4.0).sin();
            draw_circle(p.x, p.y, 18.0, Color::new(0.1, 0.3, 0.15, 1.0));
            draw_circle(p.x, p.y, 12.0 * pulse, col);
            draw_circle_lines(p.x, p.y, 20.0, 2.0, col);
        }
        Kind::Storage => {
            draw_rectangle(
                p.x - 14.0,
                p.y - 12.0,
                28.0,
                24.0,
                Color::new(0.35, 0.3, 0.15, 1.0),
            );
            for i in 0..3 {
                let y = p.y - 8.0 + i as f32 * 8.0;
                draw_line(p.x - 12.0, y, p.x + 12.0, y, 2.0, col);
            }
        }
        Kind::Llt | Kind::Hlt | Kind::Rocket => {
            let r = if k == Kind::Hlt { 15.0 } else { 12.0 };
            draw_circle(p.x, p.y, r, Color::new(0.25, 0.1, 0.08, 1.0));
            draw_circle_lines(p.x, p.y, r, 2.0, col);
            if k == Kind::Rocket {
                for i in 0..4 {
                    let dx = -6.0 + (i % 2) as f32 * 12.0;
                    let dy = -6.0 + (i / 2) as f32 * 12.0;
                    draw_circle(p.x + dx, p.y + dy, 3.0, col);
                }
            } else {
                let a = t * 0.8;
                let d = vec2(a.cos(), a.sin()) * (r + 6.0);
                draw_line(
                    p.x,
                    p.y,
                    p.x + d.x,
                    p.y + d.y,
                    if k == Kind::Hlt { 6.0 } else { 3.0 },
                    col,
                );
                draw_circle(p.x, p.y, 5.0, col);
            }
        }
        Kind::Nano => {
            draw_rectangle(p.x - 3.0, p.y - 14.0, 6.0, 28.0, col);
            draw_rectangle(p.x - 14.0, p.y - 3.0, 28.0, 6.0, col);
            draw_circle_lines(p.x, p.y, 16.0 + 2.0 * (t * 5.0).sin(), 1.5, col);
        }
        Kind::Radar => {
            draw_circle(p.x, p.y, 5.0, col);
            let a = t * 3.0;
            let d = vec2(a.cos(), a.sin()) * 18.0;
            draw_line(
                p.x - d.x * 0.3,
                p.y - d.y * 0.3,
                p.x + d.x,
                p.y + d.y,
                3.0,
                col,
            );
            draw_circle_lines(p.x, p.y, 14.0, 1.0, Color::new(0.6, 0.8, 1.0, 0.4));
        }
        Kind::Wall => {
            let (cx, cy) = cells.iter().fold((0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
            let n = cells.len() as i32;
            for &(x, y) in cells {
                let d = vec2((x * n - cx) as f32, (y * n - cy) as f32) / n as f32 * CELL;
                let q = p + d;
                draw_triangle(
                    q + vec2(0.0, -16.0),
                    q + vec2(-14.0, 12.0),
                    q + vec2(14.0, 12.0),
                    col,
                );
            }
        }
    }
}

/// Draws word-wrapped text.
fn wrap(text: &str, x: f32, y: f32, w: f32, size: f32, col: Color) {
    let mut line = String::new();
    let mut yy = y;
    for word in text.split_whitespace() {
        let trial = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if measure_text(&trial, None, size as u16, 1.0).width > w && !line.is_empty() {
            draw_text(&line, x, yy, size, col);
            yy += size * 1.1;
            line = word.to_string();
        } else {
            line = trial;
        }
    }
    if !line.is_empty() {
        draw_text(&line, x, yy, size, col);
    }
}

#[macroquad::main(conf)]
async fn main() {
    let mut app = App::new().await;
    loop {
        app.update();
        app.draw();
        next_frame().await;
    }
}
