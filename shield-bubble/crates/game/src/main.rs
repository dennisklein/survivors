//! Shield Bubble: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    BuildingKind, DURATION, EnemyKind, Event, Game, Input, PLAYER_RADIUS, Phase, SHELL_RADIUS,
    TURRET_COST, WAVES, WORLD, gen_cost, xp_needed,
};
use sim::math::{V2, v2};

const BG: Color = Color::new(0.09, 0.07, 0.06, 1.0);
const FOG: Color = Color::new(0.35, 0.2, 0.1, 0.18);
const DOME: Color = Color::new(0.3, 0.7, 1.0, 1.0);
const ALLY: Color = Color::new(0.4, 0.85, 1.0, 1.0);
const FOE: Color = Color::new(1.0, 0.35, 0.22, 1.0);
const METAL: Color = Color::new(0.85, 0.85, 0.95, 1.0);
const XP: Color = Color::new(0.4, 1.0, 0.55, 1.0);
const PANEL: Color = Color::new(0.0, 0.0, 0.0, 0.6);

#[derive(PartialEq)]
enum Screen {
    Title,
    Game,
}

struct Fx {
    pos: V2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
}

struct Banner {
    text: String,
    life: f32,
}

struct View {
    cam: V2,
    shake: f32,
}

impl View {
    fn w2s(&self, p: V2) -> Vec2 {
        let mut s = vec2(
            p.x - self.cam.x + screen_width() / 2.0,
            p.y - self.cam.y + screen_height() / 2.0,
        );
        if self.shake > 0.0 {
            s += vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * self.shake * 8.0;
        }
        s
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Shield Bubble".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: false,
        ..Default::default()
    }
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.1, 0.1, 0.1, 0.9));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
    draw_rectangle_lines(x, y, w, h, 1.0, GRAY);
}

fn read_input(g: &Game) -> Input {
    let mut dir = V2::ZERO;
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        dir.y -= 1.0;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        dir.y += 1.0;
    }
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        dir.x -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        dir.x += 1.0;
    }
    let choice = if g.phase == Phase::LevelUp {
        [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
            .iter()
            .position(|k| is_key_pressed(*k))
    } else {
        None
    };
    Input {
        dir,
        build_gen: is_key_pressed(KeyCode::B),
        build_turret: is_key_pressed(KeyCode::T),
        beacon: is_key_pressed(KeyCode::H),
        choice,
    }
}

struct App {
    game: Game,
    view: View,
    fx: Vec<Fx>,
    banner: Option<Banner>,
}

impl App {
    fn new(seed: u64) -> Self {
        App {
            game: Game::new(seed),
            view: View {
                cam: V2::ZERO,
                shake: 0.0,
            },
            fx: Vec::new(),
            banner: None,
        }
    }

    fn boom(&mut self, pos: V2, size: f32, color: Color) {
        self.fx.push(Fx {
            pos,
            life: 0.45,
            max: 0.45,
            size,
            color,
        });
    }

    fn handle(&mut self, e: Event) {
        match e {
            Event::Kill { pos, big } => {
                self.boom(pos, if big { 60.0 } else { 26.0 }, ORANGE);
                if big {
                    self.view.shake = 0.3;
                }
            }
            Event::ShellLand { pos, shielded } => {
                self.boom(pos, SHELL_RADIUS, if shielded { DOME } else { FOE });
                if !shielded {
                    self.view.shake = self.view.shake.max(0.15);
                }
            }
            Event::Wave(n) => {
                self.banner = Some(Banner {
                    text: format!("WAVE {n}/{WAVES}"),
                    life: 2.0,
                })
            }
            Event::BossSpawn => {
                self.banner = Some(Banner {
                    text: "JUGGERNAUT INBOUND".into(),
                    life: 2.5,
                })
            }
            Event::ShieldDown => {
                self.banner = Some(Banner {
                    text: "SHIELD DOWN!".into(),
                    life: 2.0,
                })
            }
            Event::BuildingLost => self.view.shake = 0.4,
            _ => {}
        }
    }

    fn update_fx(&mut self, dt: f32) {
        self.view.shake = (self.view.shake - dt).max(0.0);
        for f in &mut self.fx {
            f.life -= dt;
        }
        self.fx.retain(|f| f.life > 0.0);
        if let Some(b) = &mut self.banner {
            b.life -= dt;
            if b.life <= 0.0 {
                self.banner = None;
            }
        }
        let target = self.game.player.pos;
        self.view.cam += (target - self.view.cam) * (dt * 6.0).min(1.0);
    }

    fn draw_world(&self) {
        let g = &self.game;
        let v = &self.view;
        let t = get_time() as f32;
        clear_background(BG);
        // Ground grid.
        let step = 80.0;
        let x0 = ((v.cam.x - screen_width()) / step).floor() * step;
        let y0 = ((v.cam.y - screen_height()) / step).floor() * step;
        let mut x = x0;
        while x < v.cam.x + screen_width() {
            let a = v.w2s(v2(x, v.cam.y - screen_height()));
            let b = v.w2s(v2(x, v.cam.y + screen_height()));
            draw_line(a.x, a.y, b.x, b.y, 1.0, Color::new(0.16, 0.12, 0.1, 1.0));
            x += step;
        }
        let mut y = y0;
        while y < v.cam.y + screen_height() {
            let a = v.w2s(v2(v.cam.x - screen_width(), y));
            let b = v.w2s(v2(v.cam.x + screen_width(), y));
            draw_line(a.x, a.y, b.x, b.y, 1.0, Color::new(0.16, 0.12, 0.1, 1.0));
            y += step;
        }
        // World edge.
        let c = v.w2s(v2(-WORLD, -WORLD));
        draw_rectangle_lines(c.x, c.y, WORLD * 2.0, WORLD * 2.0, 3.0, FOE);

        // Dome interiors (filled), then rims.
        let shield_frac = g.shield / g.shield_max();
        let fill = Color::new(0.2, 0.45, 0.7, 0.10 + 0.08 * shield_frac);
        for b in g.buildings.iter().filter(|b| b.dome_radius() > 0.0) {
            let s = v.w2s(b.pos);
            draw_circle(s.x, s.y, b.dome_radius(), fill);
        }
        let rim = if g.shield > 0.0 {
            Color::new(
                DOME.r,
                DOME.g,
                DOME.b,
                0.4 + 0.4 * shield_frac + 0.1 * (t * 4.0).sin(),
            )
        } else {
            Color::new(1.0, 0.3, 0.3, 0.3 + 0.3 * (t * 10.0).sin().abs())
        };
        for b in g.buildings.iter().filter(|b| b.dome_radius() > 0.0) {
            // Draw only the rim segments not covered by another dome.
            let n = 72;
            let r = b.dome_radius();
            for i in 0..n {
                let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                let p0 = b.pos + V2::from_angle(a0) * r;
                let p1 = b.pos + V2::from_angle(a1) * r;
                let mid = b.pos + V2::from_angle((a0 + a1) / 2.0) * (r - 2.0);
                if g.dome_sd(mid) < -4.0 {
                    continue;
                }
                let s0 = v.w2s(p0);
                let s1 = v.w2s(p1);
                draw_line(s0.x, s0.y, s1.x, s1.y, 3.0, rim);
            }
        }

        // Deposits.
        for d in &g.deposits {
            let s = v.w2s(d.pos);
            if s.x < -40.0
                || s.y < -40.0
                || s.x > screen_width() + 40.0
                || s.y > screen_height() + 40.0
            {
                continue;
            }
            let r = 6.0 + d.metal.sqrt() * 1.2;
            draw_poly(s.x, s.y, 6, r, t * 20.0, Color::new(0.5, 0.5, 0.6, 1.0));
            draw_poly_lines(s.x, s.y, 6, r, t * 20.0, 2.0, METAL);
        }

        // Buildings.
        for b in &g.buildings {
            let s = v.w2s(b.pos);
            match b.kind {
                BuildingKind::Hq => {
                    draw_rectangle(
                        s.x - 30.0,
                        s.y - 30.0,
                        60.0,
                        60.0,
                        Color::new(0.2, 0.3, 0.45, 1.0),
                    );
                    draw_rectangle_lines(s.x - 30.0, s.y - 30.0, 60.0, 60.0, 3.0, ALLY);
                    draw_circle(s.x, s.y, 12.0 + (t * 3.0).sin() * 2.0, DOME);
                }
                BuildingKind::Generator => {
                    draw_poly(s.x, s.y, 8, 20.0, 0.0, Color::new(0.15, 0.3, 0.45, 1.0));
                    draw_poly_lines(s.x, s.y, 8, 20.0, 0.0, 2.0, ALLY);
                    draw_circle(s.x, s.y, 7.0 + (t * 5.0 + b.pos.x).sin() * 2.0, DOME);
                }
                BuildingKind::Turret => {
                    draw_circle(s.x, s.y, 15.0, Color::new(0.25, 0.3, 0.35, 1.0));
                    draw_circle_lines(s.x, s.y, 15.0, 2.0, ALLY);
                    draw_circle(s.x, s.y, 5.0, YELLOW);
                }
            }
            if b.hp < b.max_hp {
                bar(
                    s.x - 25.0,
                    s.y - b.radius() - 12.0,
                    50.0,
                    5.0,
                    b.hp / b.max_hp,
                    GREEN,
                );
            }
        }

        // Enemies.
        for e in &g.enemies {
            let s = v.w2s(e.pos);
            let (_, _, r, ..) = e.kind.stats();
            let col = if e.flash > 0.0 { WHITE } else { FOE };
            match e.kind {
                EnemyKind::Raider => draw_poly(s.x, s.y, 3, r, t * 90.0, col),
                EnemyKind::Artillery => {
                    draw_rectangle(s.x - r, s.y - r * 0.7, r * 2.0, r * 1.4, col);
                    draw_circle(s.x, s.y, r * 0.45, BLACK);
                }
                EnemyKind::Tank => {
                    draw_poly(s.x, s.y, 6, r, 0.0, col);
                    draw_poly_lines(s.x, s.y, 6, r, 0.0, 2.0, BLACK);
                }
                EnemyKind::Boss => {
                    draw_poly(s.x, s.y, 8, r, t * 15.0, Color::new(0.7, 0.1, 0.15, 1.0));
                    draw_poly_lines(s.x, s.y, 8, r, t * 15.0, 4.0, col);
                }
            }
            if e.hp < e.max_hp {
                bar(s.x - r, s.y - r - 8.0, r * 2.0, 4.0, e.hp / e.max_hp, FOE);
            }
        }

        // Bullets.
        for b in &g.bullets {
            let s = v.w2s(b.pos);
            draw_circle(
                s.x,
                s.y,
                if b.turret { 4.0 } else { 3.0 },
                if b.turret { YELLOW } else { ALLY },
            );
        }

        // Shells: landing warning ring and the shell itself.
        for sh in &g.shells {
            let k = sh.t / sh.flight;
            let to = v.w2s(sh.to);
            draw_circle_lines(
                to.x,
                to.y,
                SHELL_RADIUS,
                2.0,
                Color::new(1.0, 0.3, 0.2, 0.3 + 0.5 * k),
            );
            draw_circle(
                to.x,
                to.y,
                SHELL_RADIUS * k,
                Color::new(1.0, 0.3, 0.2, 0.12),
            );
            let p = v.w2s(sh.pos());
            let arc = (k * std::f32::consts::PI).sin() * 80.0;
            draw_circle(p.x, p.y - arc, 4.0, ORANGE);
        }

        // Player.
        let p = &g.player;
        let s = v.w2s(p.pos);
        let col = if p.hurt > 0.0 { RED } else { ALLY };
        draw_circle(s.x, s.y, PLAYER_RADIUS, Color::new(0.15, 0.25, 0.4, 1.0));
        draw_circle_lines(s.x, s.y, PLAYER_RADIUS, 3.0, col);
        draw_circle_lines(s.x, s.y, p.range, 1.0, Color::new(0.4, 0.85, 1.0, 0.12));
        if p.cargo > 0.0 {
            draw_circle(s.x, s.y, 5.0 * (p.cargo / p.cargo_max) + 2.0, METAL);
        }
        if let Some(c) = p.channel {
            let k = 1.0 - c / sim::game::BEACON_CHANNEL;
            draw_circle_lines(s.x, s.y, 40.0 * (1.0 - k) + 16.0, 3.0, DOME);
        }
        // Placement preview inside the dome.
        if g.in_dome(p.pos) && g.phase == Phase::Playing {
            let ok = g.can_place(p.pos);
            let c = if ok {
                Color::new(0.3, 1.0, 0.4, 0.25)
            } else {
                Color::new(1.0, 0.3, 0.3, 0.2)
            };
            draw_circle_lines(s.x, s.y, sim::game::GEN_DOME, 1.0, c);
        }

        // Effects.
        for f in &self.fx {
            let s = v.w2s(f.pos);
            let k = 1.0 - f.life / f.max;
            let c = Color::new(f.color.r, f.color.g, f.color.b, 1.0 - k);
            draw_circle(
                s.x,
                s.y,
                f.size * (0.3 + 0.7 * k),
                Color::new(c.r, c.g, c.b, c.a * 0.4),
            );
            draw_circle_lines(s.x, s.y, f.size * (0.3 + 0.7 * k), 2.0, c);
        }

        // Artillery fog vignette outside the dome.
        if !g.in_dome(p.pos) {
            let a = (0.12 + 0.05 * (t * 3.0).sin()).max(0.0);
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(FOG.r, FOG.g, FOG.b, a),
            );
        }
    }

    fn draw_hud(&self, muted: bool) {
        let g = &self.game;
        let p = &g.player;
        draw_rectangle(0.0, 0.0, screen_width(), 64.0, PANEL);
        draw_text("HP", 12.0, 22.0, 20.0, WHITE);
        bar(44.0, 10.0, 180.0, 14.0, p.hp / p.max_hp, GREEN);
        draw_text("SHIELD", 12.0, 48.0, 20.0, WHITE);
        bar(84.0, 36.0, 140.0, 14.0, g.shield / g.shield_max(), DOME);
        draw_text("HQ", 240.0, 22.0, 20.0, WHITE);
        bar(
            270.0,
            10.0,
            120.0,
            14.0,
            g.buildings[0].hp / g.buildings[0].max_hp,
            ALLY,
        );
        draw_text(
            format!(
                "METAL {}   CARGO {}/{}",
                g.metal, p.cargo as u32, p.cargo_max as u32
            ),
            240.0,
            48.0,
            20.0,
            METAL,
        );
        let gc = gen_cost(g.generators());
        draw_text(
            format!(
                "[B] Generator {gc}  [T] Turret {TURRET_COST}  [H] Beacon {}",
                if p.beacon_cd > 0.0 {
                    format!("{:.0}s", p.beacon_cd)
                } else {
                    "ready".into()
                }
            ),
            480.0,
            22.0,
            20.0,
            WHITE,
        );
        let remain = (DURATION - g.time).max(0.0);
        draw_text(
            format!(
                "WAVE {}/{}  {:02}:{:02}  KILLS {}  GENS {}  LV {}{}",
                g.wave,
                WAVES,
                remain as u32 / 60,
                remain as u32 % 60,
                g.kills,
                g.generators(),
                g.level,
                if muted { "  [M] MUTED" } else { "" }
            ),
            480.0,
            48.0,
            20.0,
            WHITE,
        );
        bar(
            0.0,
            64.0,
            screen_width(),
            4.0,
            g.xp as f32 / xp_needed(g.level) as f32,
            XP,
        );

        if !g.in_dome(p.pos) {
            centered(
                "OUTSIDE THE DOME: ARTILLERY FOG",
                screen_height() - 50.0,
                26.0,
                ORANGE,
            );
        }
        // Arrow back to base when it is off screen.
        let s = self.view.w2s(V2::ZERO);
        let (w, h) = (screen_width(), screen_height());
        if s.x < 0.0 || s.y < 0.0 || s.x > w || s.y > h {
            let c = vec2(w / 2.0, h / 2.0);
            let d = (s - c).normalize();
            let tip = c + d * (h / 2.0 - 60.0);
            draw_circle(tip.x, tip.y, 8.0, DOME);
            draw_text("HQ", tip.x + 10.0, tip.y, 18.0, DOME);
        }
        if let Some(b) = &self.banner {
            centered(
                &b.text,
                140.0,
                44.0,
                Color::new(1.0, 0.8, 0.3, b.life.min(1.0)),
            );
        }

        match g.phase {
            Phase::LevelUp => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
                centered("LEVEL UP - choose an upgrade", h / 2.0 - 90.0, 40.0, XP);
                for (i, u) in g.choices.iter().enumerate() {
                    centered(
                        &format!("[{}] {}", i + 1, u.name()),
                        h / 2.0 - 30.0 + i as f32 * 40.0,
                        30.0,
                        WHITE,
                    );
                }
            }
            Phase::Won | Phase::Lost => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
                let (t, c) = if g.phase == Phase::Won {
                    ("THE DOME HOLDS - VICTORY", XP)
                } else {
                    ("BUBBLE BURST - GAME OVER", FOE)
                };
                centered(t, h / 2.0 - 40.0, 52.0, c);
                centered(
                    &format!(
                        "Wave {}  Kills {}  Generators {}  Level {}",
                        g.wave,
                        g.kills,
                        g.generators(),
                        g.level
                    ),
                    h / 2.0 + 10.0,
                    28.0,
                    WHITE,
                );
                centered("Press ENTER to play again", h / 2.0 + 60.0, 28.0, GRAY);
            }
            Phase::Playing => {}
        }
    }
}

fn draw_title(t: f32) {
    clear_background(BG);
    let (w, h) = (screen_width(), screen_height());
    draw_circle(
        w / 2.0,
        h / 2.0 + 40.0,
        200.0,
        Color::new(0.2, 0.45, 0.7, 0.15),
    );
    draw_circle_lines(
        w / 2.0,
        h / 2.0 + 40.0,
        200.0 + (t * 2.0).sin() * 6.0,
        3.0,
        DOME,
    );
    centered("SHIELD BUBBLE", h / 2.0 - 180.0, 72.0, DOME);
    centered(
        "Your base lives inside a shield dome. Outside: lethal artillery fog.",
        h / 2.0 - 120.0,
        24.0,
        WHITE,
    );
    let lines = [
        "WASD move - weapons fire automatically",
        "Venture out, mine metal, return inside the dome to unload it",
        "B build a generator (grows the dome)   T build a turret",
        "H return-to-base beacon   M mute",
        "Survive 20 waves (10 minutes). Keep the HQ alive.",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(l, h / 2.0 + i as f32 * 30.0, 24.0, LIGHTGRAY);
    }
    if ((t * 2.0) as u32).is_multiple_of(2) {
        centered("Press any key to start", h / 2.0 + 200.0, 32.0, YELLOW);
    }
    centered(
        "Homage to Beyond All Reason - procedural graphics and sound",
        h - 20.0,
        18.0,
        GRAY,
    );
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut seed = 1;
    let mut app = App::new(seed);
    loop {
        let dt = get_frame_time().min(0.05);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match screen {
            Screen::Title => {
                draw_title(get_time() as f32);
                if get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) {
                    audio.start_music();
                    screen = Screen::Game;
                }
            }
            Screen::Game => {
                if matches!(app.game.phase, Phase::Won | Phase::Lost)
                    && is_key_pressed(KeyCode::Enter)
                {
                    seed += 1;
                    app = App::new(seed);
                }
                let input = read_input(&app.game);
                app.game.update(dt, &input);
                let events = std::mem::take(&mut app.game.events);
                for e in events {
                    audio.on_event(&e);
                    app.handle(e);
                }
                app.update_fx(dt);
                app.draw_world();
                app.draw_hud(audio.muted);
            }
        }
        next_frame().await;
    }
}
