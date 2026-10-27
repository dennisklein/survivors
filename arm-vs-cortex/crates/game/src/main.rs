//! Arm vs Cortex: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    BuildingKind, CAPTURE_RANGE, ENEMY_FACTORIES, Event, Faction, Game, Input, PLAYER_RADIUS,
    Phase, ShotStyle, Team, UnitKind, WORLD, commander_weapon, xp_needed,
};
use sim::math::{V2, v2};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed};

const BG: Color = Color::new(0.07, 0.08, 0.06, 1.0);
const GRID: Color = Color::new(0.13, 0.15, 0.11, 1.0);
const ARM: Color = Color::new(0.35, 0.7, 1.0, 1.0);
const CORTEX: Color = Color::new(1.0, 0.35, 0.25, 1.0);
const SCAV: Color = Color::new(0.85, 0.6, 0.2, 1.0);
const METAL: Color = Color::new(0.75, 0.85, 0.95, 1.0);
const XP: Color = Color::new(0.4, 1.0, 0.55, 1.0);
const PANEL: Color = Color::new(0.0, 0.0, 0.0, 0.6);
const STEP: f32 = 1.0 / 60.0;

fn faction_color(f: Option<Faction>) -> Color {
    match f {
        Some(Faction::Arm) => ARM,
        Some(Faction::Cortex) => CORTEX,
        None => SCAV,
    }
}

fn team_ring(t: Team) -> Color {
    match t {
        Team::Player => XP,
        Team::Enemy => Color::new(1.0, 0.2, 0.2, 1.0),
        Team::Scav => SCAV,
    }
}

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

struct Banner {
    text: String,
    life: f32,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Arm vs Cortex".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fg: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.1, 0.1, 0.1, 0.85));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fg);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
}

struct App {
    screen: Screen,
    game: Game,
    audio: Audio,
    fx: Vec<Fx>,
    banners: Vec<Banner>,
    cam: V2,
    shake: f32,
    acc: f32,
    seed: u64,
    audio_started: bool,
    selected: Faction,
}

impl App {
    fn start(&mut self, f: Faction) {
        self.seed = self.seed.wrapping_add(0x51ed);
        self.game = Game::new(f, self.seed);
        self.fx.clear();
        self.banners.clear();
        self.cam = self.game.player.pos;
        self.screen = Screen::Game;
        self.banner(format!("{} commander deployed", f.name()));
    }

    fn banner(&mut self, text: String) {
        self.banners.push(Banner { text, life: 2.5 });
    }

    fn burst(&mut self, pos: V2, n: usize, speed: f32, color: Color, size: f32) {
        for _ in 0..n {
            let a = rand::gen_range(0.0, std::f32::consts::TAU);
            let s = rand::gen_range(0.2, 1.0) * speed;
            let life = rand::gen_range(0.25, 0.6);
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

    fn ring(&mut self, pos: V2, r: f32, color: Color) {
        self.fx.push(Fx {
            pos,
            vel: V2::ZERO,
            life: 0.35,
            max: 0.35,
            size: r,
            color,
            ring: true,
        });
    }

    fn handle_events(&mut self) {
        let events = std::mem::take(&mut self.game.events);
        for e in &events {
            self.audio.on_event(e);
            match e {
                Event::Hit { pos } => self.burst(*pos, 3, 80.0, WHITE, 2.0),
                Event::Impact { pos, splash } => {
                    self.ring(*pos, *splash, ORANGE);
                    self.burst(*pos, 6, 120.0, ORANGE, 3.0);
                }
                Event::Kill { pos, big } => {
                    let n = if *big { 40 } else { 12 };
                    self.burst(*pos, n, if *big { 260.0 } else { 150.0 }, ORANGE, 3.5);
                    self.burst(*pos, n / 2, 90.0, DARKGRAY, 5.0);
                    if *big {
                        self.ring(*pos, 90.0, YELLOW);
                        self.shake = self.shake.max(0.4);
                    }
                }
                Event::PlayerHurt => self.shake = self.shake.max(0.15),
                Event::Captured => {
                    let n = self.game.captured();
                    self.banner(format!("Factory captured! Enemy tech level {n}"));
                    let p = self.game.player.pos;
                    self.ring(p, CAPTURE_RANGE, XP);
                }
                Event::FactoryLost => self.banner("A captured factory was lost!".into()),
                Event::Wave(w) => self.banner(format!("Wave {w}")),
                Event::BossSpawn => self.banner("Warning: enemy commander inbound".into()),
                Event::Built => {
                    let p = self.game.player.pos;
                    self.ring(p, 30.0, XP);
                }
                _ => {}
            }
        }
        self.game.events = events;
    }

    fn read_input(&self) -> Input {
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
        let choice = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
            .iter()
            .position(|k| is_key_pressed(*k));
        Input {
            dir,
            capture: is_key_down(KeyCode::C),
            build: is_key_pressed(KeyCode::B),
            choice,
        }
    }

    fn frame(&mut self) {
        let dt = get_frame_time().min(0.1);
        let any = get_last_key_pressed().is_some()
            || is_mouse_button_pressed(MouseButton::Left)
            || !touches().is_empty();
        if any && !self.audio_started {
            self.audio_started = true;
            self.audio.start_music();
        }
        if is_key_pressed(KeyCode::M) {
            self.audio.toggle_mute();
        }
        match self.screen {
            Screen::Title => {
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Key1) {
                    self.selected = Faction::Arm;
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Key2) {
                    self.selected = Faction::Cortex;
                }
                if [KeyCode::Key1, KeyCode::Key2, KeyCode::Enter, KeyCode::Space]
                    .into_iter()
                    .any(is_key_pressed)
                {
                    self.start(self.selected);
                }
            }
            Screen::Game => {
                if matches!(self.game.phase, Phase::Won | Phase::Lost) {
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::R) {
                        self.screen = Screen::Title;
                    }
                } else {
                    self.acc += dt;
                    let mut input = self.read_input();
                    while self.acc >= STEP {
                        self.acc -= STEP;
                        self.game.step(STEP, &input);
                        self.handle_events();
                        input.build = false;
                        input.choice = None;
                    }
                }
                let target = self.game.player.pos;
                self.cam += (target - self.cam) * (dt * 6.0).min(1.0);
            }
        }
        self.shake = (self.shake - dt).max(0.0);
        for f in &mut self.fx {
            f.pos += f.vel * dt;
            f.vel = f.vel * (1.0 - 3.0 * dt).max(0.0);
            f.life -= dt;
        }
        self.fx.retain(|f| f.life > 0.0);
        for b in &mut self.banners {
            b.life -= dt;
        }
        self.banners.retain(|b| b.life > 0.0);
    }

    fn scale(&self) -> f32 {
        (screen_width().min(screen_height()) / 820.0).clamp(0.55, 1.6)
    }

    fn w2s(&self, p: V2) -> Vec2 {
        let s = self.scale();
        let (mut ox, mut oy) = (0.0, 0.0);
        if self.shake > 0.0 {
            ox = rand::gen_range(-6.0, 6.0) * self.shake;
            oy = rand::gen_range(-6.0, 6.0) * self.shake;
        }
        vec2(
            (p.x - self.cam.x) * s + screen_width() / 2.0 + ox,
            (p.y - self.cam.y) * s + screen_height() / 2.0 + oy,
        )
    }

    fn draw_world(&self) {
        let s = self.scale();
        let g = &self.game;
        // Terrain grid.
        let step = 100.0;
        let mut x = 0.0;
        while x <= WORLD {
            let a = self.w2s(v2(x, 0.0));
            let b = self.w2s(v2(x, WORLD));
            draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            let a = self.w2s(v2(0.0, x));
            let b = self.w2s(v2(WORLD, x));
            draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            x += step;
        }
        let tl = self.w2s(v2(0.0, 0.0));
        draw_rectangle_lines(tl.x, tl.y, WORLD * s, WORLD * s, 4.0, SCAV);

        // Metal pickups.
        for k in &g.pickups {
            let p = self.w2s(k.pos);
            let r = (3.0 + (k.value as f32).sqrt()) * s;
            draw_poly(p.x, p.y, 4, r, 45.0, METAL);
        }

        // Buildings.
        for b in &g.buildings {
            let p = self.w2s(b.pos);
            let r = b.radius() * s;
            let mut c = faction_color(Some(b.faction));
            if b.flash > 0.0 {
                c = WHITE;
            }
            match b.kind {
                BuildingKind::Factory => {
                    draw_rectangle(p.x - r, p.y - r, 2.0 * r, 2.0 * r, Color { a: 0.35, ..c });
                    draw_rectangle_lines(p.x - r, p.y - r, 2.0 * r, 2.0 * r, 3.0, c);
                    draw_rectangle(p.x - r * 0.5, p.y - r * 0.3, r, r * 1.3, c);
                    draw_rectangle(p.x - r * 0.25, p.y - r * 0.9, r * 0.5, r * 0.6, c);
                    draw_circle_lines(p.x, p.y, r * 1.5, 2.0, team_ring(b.team));
                    if b.capture > 0.0 {
                        bar(p.x - r, p.y + r + 6.0, 2.0 * r, 6.0, b.capture, XP);
                    }
                    let label = if b.team == Team::Player {
                        if b.home { "HOME" } else { "CAPTURED" }
                    } else {
                        "ENEMY LAB"
                    };
                    let d = measure_text(label, None, 14, 1.0);
                    draw_text(label, p.x - d.width / 2.0, p.y - r - 18.0, 14.0, WHITE);
                }
                BuildingKind::Turret => {
                    draw_circle(p.x, p.y, r, Color { a: 0.5, ..c });
                    draw_circle_lines(p.x, p.y, r, 2.0, c);
                    draw_rectangle(p.x - 3.0 * s, p.y - r * 1.4, 6.0 * s, r * 1.2, c);
                }
            }
            if b.hp < b.max_hp {
                bar(
                    p.x - r,
                    p.y - r - 10.0,
                    2.0 * r,
                    4.0,
                    b.hp / b.max_hp,
                    team_ring(b.team),
                );
            }
        }

        // Units.
        for u in &g.units {
            let p = self.w2s(u.pos);
            let r = u.stats.radius * s;
            let c = if u.flash > 0.0 {
                WHITE
            } else {
                faction_color(u.faction)
            };
            let sides = match u.kind {
                UnitKind::Raider => 3,
                UnitKind::Tank => 4,
                UnitKind::Artillery => 5,
                UnitKind::Boss => 8,
            };
            draw_poly(p.x, p.y, sides, r, u.facing.to_degrees(), c);
            draw_poly_lines(
                p.x,
                p.y,
                sides,
                r,
                u.facing.to_degrees(),
                2.0,
                team_ring(u.team),
            );
            let d = V2::from_angle(u.facing) * (r * 1.3 / s);
            let e = self.w2s(u.pos + d);
            draw_line(p.x, p.y, e.x, e.y, 3.0 * s, DARKGRAY);
            if u.kind == UnitKind::Boss {
                bar(
                    p.x - r,
                    p.y - r - 10.0,
                    2.0 * r,
                    5.0,
                    u.hp / u.stats.hp,
                    RED,
                );
            }
        }

        // Player commander.
        let pl = &g.player;
        let p = self.w2s(pl.pos);
        let c = faction_color(Some(pl.faction));
        let r = PLAYER_RADIUS * s;
        if let Some(i) = g.capture_target() {
            let b = self.w2s(g.buildings[i].pos);
            let pulse = (get_time() as f32 * 10.0).sin() * 0.5 + 0.5;
            draw_line(
                p.x,
                p.y,
                b.x,
                b.y,
                2.0,
                Color {
                    a: 0.3 + 0.5 * pulse,
                    ..XP
                },
            );
        }
        draw_circle(p.x, p.y, r * 1.4, Color { a: 0.2, ..c });
        draw_circle(p.x, p.y, r, if pl.hurt > 0.0 { WHITE } else { c });
        draw_circle_lines(p.x, p.y, r, 3.0, XP);
        let gun = self.w2s(pl.pos + V2::from_angle(pl.facing) * 24.0);
        draw_line(p.x, p.y, gun.x, gun.y, 6.0 * s, LIGHTGRAY);
        let range = commander_weapon(pl.faction).range * s;
        draw_circle_lines(p.x, p.y, range, 1.0, Color::new(1.0, 1.0, 1.0, 0.06));

        // Shots.
        for sh in &g.shots {
            let p = self.w2s(sh.pos);
            match sh.style {
                ShotStyle::Laser => {
                    let t = self.w2s(sh.pos - sh.vel.norm() * 18.0);
                    let col = if sh.team == Team::Player { ARM } else { RED };
                    draw_line(p.x, p.y, t.x, t.y, 3.0 * s, col);
                }
                ShotStyle::Plasma => {
                    draw_circle(p.x, p.y, 6.0 * s, ORANGE);
                    draw_circle(p.x, p.y, 3.0 * s, YELLOW);
                }
                ShotStyle::Bolt => {
                    let col = match sh.team {
                        Team::Player => XP,
                        Team::Enemy => faction_color(sh.faction),
                        Team::Scav => SCAV,
                    };
                    draw_circle(p.x, p.y, 3.0 * s, col);
                }
            }
        }

        for f in &self.fx {
            let p = self.w2s(f.pos);
            let k = f.life / f.max;
            if f.ring {
                draw_circle_lines(
                    p.x,
                    p.y,
                    f.size * s * (1.2 - k * 0.5),
                    3.0,
                    Color { a: k, ..f.color },
                );
            } else {
                draw_circle(p.x, p.y, f.size * s * k.max(0.3), Color { a: k, ..f.color });
            }
        }
    }

    fn draw_minimap(&self) {
        let size = 150.0;
        let x0 = screen_width() - size - 12.0;
        let y0 = 12.0;
        let k = size / WORLD;
        draw_rectangle(x0, y0, size, size, PANEL);
        draw_rectangle_lines(x0, y0, size, size, 1.0, GRAY);
        let g = &self.game;
        for u in &g.units {
            draw_rectangle(
                x0 + u.pos.x * k,
                y0 + u.pos.y * k,
                2.0,
                2.0,
                team_ring(u.team),
            );
        }
        for b in &g.buildings {
            let s = if b.kind == BuildingKind::Factory {
                8.0
            } else {
                3.0
            };
            draw_rectangle(
                x0 + b.pos.x * k - s / 2.0,
                y0 + b.pos.y * k - s / 2.0,
                s,
                s,
                team_ring(b.team),
            );
        }
        draw_circle(x0 + g.player.pos.x * k, y0 + g.player.pos.y * k, 3.0, WHITE);
    }

    fn draw_hud(&self) {
        let g = &self.game;
        let p = &g.player;
        draw_rectangle(0.0, 0.0, 330.0, 132.0, PANEL);
        draw_text(
            format!("{} Commander  Lv {}", p.faction.name(), p.level),
            12.0,
            24.0,
            24.0,
            faction_color(Some(p.faction)),
        );
        bar(
            12.0,
            34.0,
            300.0,
            14.0,
            p.hp / p.max_hp,
            Color::new(0.9, 0.25, 0.25, 1.0),
        );
        draw_text(
            format!("HP {:.0}/{:.0}", p.hp, p.max_hp),
            16.0,
            46.0,
            16.0,
            WHITE,
        );
        bar(
            12.0,
            52.0,
            300.0,
            8.0,
            p.xp as f32 / xp_needed(p.level) as f32,
            XP,
        );
        draw_text(
            format!(
                "Metal {}   Wave {}  (next {:.0}s)",
                p.metal,
                g.wave,
                g.wave_timer.max(0.0)
            ),
            12.0,
            80.0,
            20.0,
            METAL,
        );
        draw_text(
            format!(
                "Enemy labs captured {}/{}   Kills {}",
                g.captured(),
                ENEMY_FACTORIES,
                g.kills
            ),
            12.0,
            102.0,
            20.0,
            XP,
        );
        let cost = g.next_turret_cost();
        let col = if p.metal >= cost { WHITE } else { GRAY };
        draw_text(
            format!("[B] build tower: {cost} metal   [C] capture"),
            12.0,
            124.0,
            18.0,
            col,
        );
        self.draw_minimap();
        let mute = if self.audio.muted {
            "[M] sound OFF"
        } else {
            "[M] sound on"
        };
        draw_text(mute, screen_width() - 162.0, 180.0, 18.0, GRAY);

        if let Some(i) = g.capture_target() {
            let b = &g.buildings[i];
            let msg = if is_key_down(KeyCode::C) {
                format!("Capturing... {:.0}%", b.capture * 100.0)
            } else {
                "Hold C to capture this lab (faster when damaged)".to_owned()
            };
            centered(&msg, screen_height() - 40.0, 24.0, XP);
        }
        for (i, b) in self.banners.iter().enumerate() {
            let a = b.life.min(1.0);
            centered(
                &b.text,
                170.0 + i as f32 * 34.0,
                32.0,
                Color::new(1.0, 0.9, 0.5, a),
            );
        }
    }

    fn draw_overlay(&self) {
        let g = &self.game;
        match &g.phase {
            Phase::LevelUp(choices) => {
                draw_rectangle(0.0, 0.0, screen_width(), screen_height(), PANEL);
                centered(
                    "LEVEL UP - choose an upgrade",
                    screen_height() * 0.25,
                    40.0,
                    XP,
                );
                let w = 300.0;
                let total = w * 3.0 + 40.0;
                let x0 = (screen_width() - total) / 2.0;
                let y = screen_height() * 0.35;
                for (i, u) in choices.iter().enumerate() {
                    let x = x0 + i as f32 * (w + 20.0);
                    draw_rectangle(x, y, w, 140.0, Color::new(0.1, 0.14, 0.1, 0.95));
                    draw_rectangle_lines(x, y, w, 140.0, 2.0, XP);
                    draw_text(format!("[{}]", i + 1), x + 14.0, y + 34.0, 30.0, YELLOW);
                    draw_text(u.name(), x + 60.0, y + 34.0, 24.0, WHITE);
                    draw_text(u.desc(), x + 14.0, y + 84.0, 19.0, LIGHTGRAY);
                }
            }
            Phase::Won | Phase::Lost => {
                draw_rectangle(0.0, 0.0, screen_width(), screen_height(), PANEL);
                let (t, c) = if g.phase == Phase::Won {
                    ("VICTORY - all enemy labs are yours", XP)
                } else {
                    ("COMMANDER DESTROYED", RED)
                };
                let h = screen_height();
                centered(t, h * 0.38, 52.0, c);
                centered(
                    &format!(
                        "Wave {}   Level {}   Kills {}   Time {:.0}s",
                        g.wave, g.player.level, g.kills, g.time
                    ),
                    h * 0.48,
                    26.0,
                    WHITE,
                );
                centered(
                    "Press Enter to return to faction select",
                    h * 0.58,
                    24.0,
                    GRAY,
                );
            }
            Phase::Playing => {}
        }
    }

    fn draw_title(&self) {
        let h = screen_height();
        let w = screen_width();
        centered("ARM vs CORTEX", h * 0.2, 72.0, WHITE);
        centered(
            "Survive the war. Capture enemy labs. Steal their tech.",
            h * 0.2 + 44.0,
            24.0,
            LIGHTGRAY,
        );
        let cw = 330.0;
        for (i, f) in [Faction::Arm, Faction::Cortex].into_iter().enumerate() {
            let x = w / 2.0 - cw - 15.0 + i as f32 * (cw + 30.0);
            let y = h * 0.34;
            let c = faction_color(Some(f));
            let sel = self.selected == f;
            draw_rectangle(
                x,
                y,
                cw,
                200.0,
                Color {
                    a: if sel { 0.25 } else { 0.1 },
                    ..c
                },
            );
            draw_rectangle_lines(x, y, cw, 200.0, if sel { 4.0 } else { 1.0 }, c);
            draw_text(
                format!("[{}] {}", i + 1, f.name()),
                x + 16.0,
                y + 38.0,
                32.0,
                c,
            );
            let lines: [&str; 3] = match f {
                Faction::Arm => [
                    "Rapid light lasers, long range",
                    "Builds: Light Laser Towers",
                    "Fast, agile units",
                ],
                Faction::Cortex => [
                    "Heavy plasma with splash",
                    "Builds: Heavy Cannon Towers",
                    "Tough, slow units",
                ],
            };
            for (k, l) in lines.iter().enumerate() {
                draw_text(l, x + 16.0, y + 84.0 + k as f32 * 30.0, 21.0, WHITE);
            }
        }
        let y = h * 0.34 + 240.0;
        centered(
            "1 / 2 or Left/Right + Enter to pick a commander",
            y,
            24.0,
            YELLOW,
        );
        centered(
            "WASD move  |  auto-fire  |  hold C near enemy lab to capture  |  B build tower  |  M mute",
            y + 36.0,
            20.0,
            LIGHTGRAY,
        );
        centered(
            "Scavengers fight everyone. Capture all 4 enemy labs to win.",
            y + 64.0,
            20.0,
            LIGHTGRAY,
        );
        if !self.audio_started {
            let a = (get_time() as f32 * 3.0).sin() * 0.3 + 0.7;
            centered(
                "Press any key to enable sound",
                h - 40.0,
                22.0,
                Color::new(1.0, 1.0, 1.0, a),
            );
        }
    }

    fn draw(&self) {
        clear_background(BG);
        match self.screen {
            Screen::Title => self.draw_title(),
            Screen::Game => {
                self.draw_world();
                self.draw_hud();
                self.draw_overlay();
            }
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Capture", KeyCode::C),
        touch::key("Tower", KeyCode::B),
        touch::small("Start", KeyCode::Enter),
        touch::small("Mute", KeyCode::M),
    ]);
    clear_background(BG);
    centered("Loading...", screen_height() / 2.0, 32.0, WHITE);
    touch::next_frame().await;
    let audio = Audio::new().await;
    let mut app = App {
        screen: Screen::Title,
        game: Game::new(Faction::Arm, 1),
        audio,
        fx: Vec::new(),
        banners: Vec::new(),
        cam: V2::ZERO,
        shake: 0.0,
        acc: 0.0,
        seed: macroquad::miniquad::date::now() as u64,
        audio_started: false,
        selected: Faction::Arm,
    };
    loop {
        app.frame();
        app.draw();
        touch::next_frame().await;
    }
}
