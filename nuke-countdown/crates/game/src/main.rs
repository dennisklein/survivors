//! Nuke Countdown: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    ANTI_CHARGE, BuildKind, CYCLES, DEPOSIT_RANGE, ENEMY_NUKE_RADIUS, EnemyKind, Event, Game,
    Input, NUKE_FLIGHT, PLAYER_RADIUS, Phase, PickupKind, SILO_RADIUS, WORLD, energy_needed,
    metal_needed, xp_needed,
};
use sim::math::{V2, v2};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed};

const BG: Color = Color::new(0.08, 0.09, 0.07, 1.0);
const GRID: Color = Color::new(0.14, 0.16, 0.12, 1.0);
const ALLY: Color = Color::new(0.4, 0.85, 1.0, 1.0);
const FOE: Color = Color::new(1.0, 0.35, 0.22, 1.0);
const METAL: Color = Color::new(0.85, 0.85, 0.95, 1.0);
const ENERGY: Color = Color::new(1.0, 0.9, 0.25, 1.0);
const XP: Color = Color::new(0.4, 1.0, 0.55, 1.0);
const NUKE: Color = Color::new(1.0, 0.75, 0.2, 1.0);
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
    color: Color,
}

struct View {
    cam: V2,
    shake: f32,
    flash: f32,
}

impl View {
    fn w2s(&self, p: V2) -> Vec2 {
        let mut s = vec2(
            p.x - self.cam.x + screen_width() / 2.0,
            p.y - self.cam.y + screen_height() / 2.0,
        );
        if self.shake > 0.0 {
            s += vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * self.shake * 10.0;
        }
        s
    }

    fn visible(&self, s: Vec2, m: f32) -> bool {
        s.x > -m && s.y > -m && s.x < screen_width() + m && s.y < screen_height() + m
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Nuke Countdown".into(),
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

fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

struct App {
    game: Game,
    view: View,
    fx: Vec<Fx>,
    banner: Option<Banner>,
    build_menu: bool,
}

impl App {
    fn new(seed: u64) -> Self {
        App {
            game: Game::new(seed),
            view: View {
                cam: v2(0.0, 120.0),
                shake: 0.0,
                flash: 0.0,
            },
            fx: Vec::new(),
            banner: None,
            build_menu: false,
        }
    }

    fn read_input(&mut self) -> Input {
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
        let digit = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
            .iter()
            .position(|k| is_key_pressed(*k));
        let mut input = Input {
            dir,
            deposit: is_key_pressed(KeyCode::E),
            launch: is_key_pressed(KeyCode::N),
            ..Default::default()
        };
        if self.game.phase == Phase::LevelUp {
            input.choice = digit;
        } else if self.game.phase == Phase::Playing {
            if is_key_pressed(KeyCode::B) {
                self.build_menu = !self.build_menu;
            }
            if self.build_menu && is_key_pressed(KeyCode::Escape) {
                self.build_menu = false;
            }
            if self.build_menu
                && let Some(d) = digit
            {
                input.build = Some(BuildKind::ALL[d]);
                self.build_menu = false;
            }
        }
        input
    }

    fn boom(&mut self, pos: V2, size: f32, life: f32, color: Color) {
        self.fx.push(Fx {
            pos,
            life,
            max: life,
            size,
            color,
        });
    }

    fn say(&mut self, text: impl Into<String>, life: f32, color: Color) {
        self.banner = Some(Banner {
            text: text.into(),
            life,
            color,
        });
    }

    fn handle(&mut self, e: Event) {
        match e {
            Event::Kill { pos, big } => {
                self.boom(pos, if big { 60.0 } else { 24.0 }, 0.45, ORANGE);
                if big {
                    self.view.shake = self.view.shake.max(0.25);
                }
            }
            Event::Armed => self.say("SILO ARMED - PRESS N TO LAUNCH", 3.0, NUKE),
            Event::Launch => {
                self.say("NUCLEAR LAUNCH DETECTED", 2.5, NUKE);
                self.view.shake = 0.4;
            }
            Event::Detonate { cycle } => {
                self.view.flash = 1.0;
                self.view.shake = 1.0;
                if cycle < CYCLES {
                    self.say(
                        format!("STRIKE {cycle}/{CYCLES} - ENEMY RETALIATION EXPECTED"),
                        3.5,
                        NUKE,
                    );
                }
            }
            Event::JuggernautSpawn => self.say("JUGGERNAUT INBOUND", 3.0, FOE),
            Event::EnemyNukeLaunch => self.say("ENEMY NUKE LAUNCHED!", 3.0, FOE),
            Event::Intercepted { pos } => {
                self.boom(pos, 90.0, 0.8, ALLY);
                self.say("NUKE INTERCEPTED", 2.0, ALLY);
            }
            Event::EnemyNukeHit { pos } => {
                self.boom(pos, ENEMY_NUKE_RADIUS, 1.4, FOE);
                self.view.shake = 1.0;
                self.view.flash = 0.6;
            }
            Event::BuildingLost => self.view.shake = self.view.shake.max(0.3),
            Event::Built(k) => self.boom(self.game.player.pos, k.radius() * 2.0, 0.4, ALLY),
            _ => {}
        }
    }

    fn update_fx(&mut self, dt: f32) {
        self.view.shake = (self.view.shake - dt).max(0.0);
        self.view.flash = (self.view.flash - dt * 0.8).max(0.0);
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

    fn draw_grid(&self) {
        let v = &self.view;
        let step = 80.0;
        let (w, h) = (screen_width(), screen_height());
        let mut x = ((v.cam.x - w) / step).floor() * step;
        while x < v.cam.x + w {
            let a = v.w2s(v2(x, v.cam.y - h));
            let b = v.w2s(v2(x, v.cam.y + h));
            draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            x += step;
        }
        let mut y = ((v.cam.y - h) / step).floor() * step;
        while y < v.cam.y + h {
            let a = v.w2s(v2(v.cam.x - w, y));
            let b = v.w2s(v2(v.cam.x + w, y));
            draw_line(a.x, a.y, b.x, b.y, 1.0, GRID);
            y += step;
        }
        let c = v.w2s(v2(-WORLD, -WORLD));
        draw_rectangle_lines(c.x, c.y, WORLD * 2.0, WORLD * 2.0, 3.0, FOE);
    }

    fn draw_world(&self) {
        let g = &self.game;
        let v = &self.view;
        let t = get_time() as f32;
        clear_background(BG);
        self.draw_grid();

        // Deposit ring around the silo.
        let s = v.w2s(V2::ZERO);
        draw_circle_lines(s.x, s.y, DEPOSIT_RANGE, 1.0, alpha(NUKE, 0.25));

        // Metal deposits.
        for d in &g.deposits {
            let s = v.w2s(d.pos);
            if !v.visible(s, 40.0) {
                continue;
            }
            let r = 8.0 + d.metal.sqrt() * 1.3;
            draw_poly(s.x, s.y, 6, r, 0.0, Color::new(0.35, 0.35, 0.42, 1.0));
            draw_poly_lines(s.x, s.y, 6, r, 0.0, 2.0, METAL);
            draw_circle(s.x, s.y, 4.0, METAL);
        }

        // Silo.
        let armed = g.armed();
        draw_circle(s.x, s.y, SILO_RADIUS, Color::new(0.22, 0.24, 0.2, 1.0));
        draw_circle_lines(s.x, s.y, SILO_RADIUS, 4.0, ALLY);
        let hatch = if g.nuke.is_some() {
            WHITE
        } else if armed {
            alpha(NUKE, 0.6 + 0.4 * (t * 6.0).sin())
        } else {
            Color::new(0.1, 0.1, 0.1, 1.0)
        };
        draw_circle(s.x, s.y, SILO_RADIUS * 0.55, hatch);
        draw_poly_lines(s.x, s.y, 3, SILO_RADIUS * 0.4, t * 10.0, 2.0, NUKE);
        let mf = g.silo_metal as f32 / metal_needed(g.cycle) as f32;
        let ef = g.silo_energy as f32 / energy_needed(g.cycle) as f32;
        bar(s.x - 40.0, s.y + SILO_RADIUS + 6.0, 80.0, 5.0, mf, METAL);
        bar(s.x - 40.0, s.y + SILO_RADIUS + 13.0, 80.0, 5.0, ef, ENERGY);
        bar(
            s.x - 40.0,
            s.y - SILO_RADIUS - 12.0,
            80.0,
            6.0,
            g.silo_hp / g.silo_max,
            GREEN,
        );

        // Buildings.
        for b in &g.buildings {
            let s = v.w2s(b.pos);
            let r = b.kind.radius();
            match b.kind {
                BuildKind::Turret => {
                    draw_circle(s.x, s.y, r, Color::new(0.25, 0.3, 0.35, 1.0));
                    draw_circle_lines(s.x, s.y, r, 2.0, ALLY);
                    draw_circle(s.x, s.y, 5.0, ENERGY);
                }
                BuildKind::Solar => {
                    draw_rectangle(
                        s.x - r,
                        s.y - r,
                        r * 2.0,
                        r * 2.0,
                        Color::new(0.1, 0.2, 0.4, 1.0),
                    );
                    draw_rectangle_lines(s.x - r, s.y - r, r * 2.0, r * 2.0, 2.0, ENERGY);
                    draw_line(s.x - r, s.y, s.x + r, s.y, 1.0, ENERGY);
                    draw_line(s.x, s.y - r, s.x, s.y + r, 1.0, ENERGY);
                }
                BuildKind::AntiNuke => {
                    draw_poly(s.x, s.y, 8, r, 22.5, Color::new(0.15, 0.25, 0.35, 1.0));
                    draw_poly_lines(s.x, s.y, 8, r, 22.5, 3.0, ALLY);
                    let k = b.charge / ANTI_CHARGE;
                    let c = if k >= 1.0 { ALLY } else { GRAY };
                    draw_circle(s.x, s.y, 4.0 + 6.0 * k, c);
                }
            }
            if b.hp < b.kind.max_hp() {
                bar(
                    s.x - 20.0,
                    s.y - r - 10.0,
                    40.0,
                    4.0,
                    b.hp / b.kind.max_hp(),
                    GREEN,
                );
            }
        }

        // Pickups.
        for p in &g.pickups {
            let s = v.w2s(p.pos);
            if !v.visible(s, 10.0) {
                continue;
            }
            match p.kind {
                PickupKind::Xp => draw_poly(s.x, s.y, 4, 4.0, 45.0, XP),
                PickupKind::Metal => draw_poly(s.x, s.y, 6, 6.0, 0.0, METAL),
                PickupKind::Energy => draw_circle(s.x, s.y, 5.0 + (t * 8.0).sin(), ENERGY),
            }
        }

        // Enemies.
        for e in &g.enemies {
            let s = v.w2s(e.pos);
            let (_, _, r, ..) = e.kind.stats();
            if !v.visible(s, r + 10.0) {
                continue;
            }
            let col = if e.flash > 0.0 { WHITE } else { FOE };
            match e.kind {
                EnemyKind::Grunt => draw_poly(s.x, s.y, 4, r, 45.0, col),
                EnemyKind::Raider => draw_poly(s.x, s.y, 3, r, t * 120.0, col),
                EnemyKind::Brute => {
                    draw_poly(s.x, s.y, 6, r, 0.0, col);
                    draw_poly_lines(s.x, s.y, 6, r, 0.0, 2.0, BLACK);
                }
                EnemyKind::Juggernaut => {
                    draw_poly(s.x, s.y, 8, r, t * 12.0, Color::new(0.55, 0.08, 0.1, 1.0));
                    draw_poly_lines(s.x, s.y, 8, r, t * 12.0, 4.0, col);
                    draw_circle(s.x, s.y, r * 0.35, ORANGE);
                }
            }
            if e.hp < e.max_hp {
                bar(s.x - r, s.y - r - 8.0, r * 2.0, 4.0, e.hp / e.max_hp, FOE);
            }
        }

        // Bullets.
        for b in &g.bullets {
            let s = v.w2s(b.pos);
            draw_circle(s.x, s.y, 3.0, if b.turret { ENERGY } else { ALLY });
        }

        // Player commander.
        let p = &g.player;
        let s = v.w2s(p.pos);
        let col = if p.hurt > 0.0 { RED } else { ALLY };
        draw_circle(s.x, s.y, PLAYER_RADIUS, Color::new(0.15, 0.25, 0.4, 1.0));
        draw_circle_lines(s.x, s.y, PLAYER_RADIUS, 3.0, col);
        draw_circle_lines(s.x, s.y, p.range, 1.0, alpha(ALLY, 0.1));
        if self.build_menu {
            let ok = BuildKind::ALL
                .iter()
                .any(|k| g.can_place(p.pos, *k) && p.metal >= k.cost());
            let c = if ok { alpha(XP, 0.5) } else { alpha(FOE, 0.5) };
            draw_circle_lines(s.x, s.y, 24.0, 2.0, c);
        }

        // Our nuke climbing out of the silo.
        if let Some(nt) = g.nuke {
            let s0 = v.w2s(V2::ZERO);
            let k = nt / NUKE_FLIGHT;
            let y = s0.y - k * k * 900.0;
            draw_line(
                s0.x,
                s0.y,
                s0.x,
                y,
                6.0 * (1.0 - k) + 1.0,
                alpha(ORANGE, 0.6),
            );
            draw_circle(s0.x, y, 8.0, WHITE);
        }

        // Enemy nuke: target ring and the incoming warhead.
        if let Some(n) = &g.enemy_nuke {
            let to = v.w2s(n.to);
            let k = n.t / sim::game::ENEMY_NUKE_FLIGHT;
            draw_circle(to.x, to.y, ENEMY_NUKE_RADIUS * k, alpha(FOE, 0.1));
            draw_circle_lines(
                to.x,
                to.y,
                ENEMY_NUKE_RADIUS,
                3.0,
                alpha(FOE, 0.4 + 0.4 * (t * 8.0).sin().abs()),
            );
            let ps = v.w2s(n.pos());
            draw_circle(ps.x, ps.y, 7.0, FOE);
            draw_circle_lines(ps.x, ps.y, 12.0, 2.0, WHITE);
        }

        for f in &self.fx {
            let s = v.w2s(f.pos);
            let k = 1.0 - f.life / f.max;
            let r = f.size * (0.3 + 0.7 * k);
            draw_circle(s.x, s.y, r, alpha(f.color, (1.0 - k) * 0.4));
            draw_circle_lines(s.x, s.y, r, 2.0, alpha(f.color, 1.0 - k));
        }

        if v.flash > 0.0 {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(1.0, 0.95, 0.8, v.flash.min(1.0) * 0.8),
            );
        }
    }

    fn draw_hud(&self, muted: bool) {
        let g = &self.game;
        let p = &g.player;
        let (w, h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, w, 70.0, PANEL);
        draw_text("HP", 12.0, 22.0, 20.0, WHITE);
        bar(44.0, 10.0, 160.0, 14.0, p.hp / p.max_hp, GREEN);
        draw_text("SILO", 12.0, 50.0, 20.0, WHITE);
        bar(60.0, 38.0, 144.0, 14.0, g.silo_hp / g.silo_max, ALLY);
        draw_text(
            format!(
                "CARGO  metal {}  energy {}  ({}/{})",
                p.metal,
                p.energy,
                g.cargo(),
                p.cargo_max
            ),
            220.0,
            22.0,
            20.0,
            METAL,
        );
        draw_text("SILO METAL", 220.0, 50.0, 18.0, METAL);
        bar(
            315.0,
            39.0,
            110.0,
            12.0,
            g.silo_metal as f32 / metal_needed(g.cycle) as f32,
            METAL,
        );
        draw_text(
            format!("{}/{}", g.silo_metal, metal_needed(g.cycle)),
            430.0,
            50.0,
            18.0,
            METAL,
        );
        draw_text("ENERGY", 500.0, 50.0, 18.0, ENERGY);
        bar(
            560.0,
            39.0,
            110.0,
            12.0,
            g.silo_energy as f32 / energy_needed(g.cycle) as f32,
            ENERGY,
        );
        draw_text(
            format!("{}/{}", g.silo_energy, energy_needed(g.cycle)),
            675.0,
            50.0,
            18.0,
            ENERGY,
        );
        let jugg = if g.jugg_spawned {
            "JUGGERNAUT MARCHING".to_string()
        } else {
            let c = g.jugg_clock.max(0.0) as u32;
            format!("JUGGERNAUT IN {:02}:{:02}", c / 60, c % 60)
        };
        let jc = if g.jugg_spawned || g.jugg_clock < 20.0 {
            FOE
        } else {
            WHITE
        };
        draw_text(&jugg, 520.0, 22.0, 22.0, jc);
        draw_text(
            format!(
                "STRIKES {}/{}  CYCLE {}  LV {}  KILLS {}  ANTI {}{}",
                g.launches,
                CYCLES,
                g.cycle,
                g.level,
                g.kills,
                g.interceptors(),
                if muted { "  [M] MUTED" } else { "" }
            ),
            760.0,
            22.0,
            20.0,
            WHITE,
        );
        draw_text(
            "[E] deposit  [N] launch  [B] build  [M] mute",
            760.0,
            50.0,
            18.0,
            LIGHTGRAY,
        );
        bar(
            0.0,
            70.0,
            w,
            4.0,
            g.xp as f32 / xp_needed(g.level) as f32,
            XP,
        );

        // Context hints.
        if g.near_silo() && g.cargo() > 0 {
            centered("[E] Deposit cargo into silo", h - 60.0, 26.0, NUKE);
        } else if g.cargo() >= p.cargo_max {
            centered("Cargo full - return to the silo", h - 60.0, 26.0, ORANGE);
        }
        if g.armed() && g.nuke.is_none() {
            centered(
                "SILO ARMED - [N] LAUNCH",
                h - 28.0,
                28.0,
                alpha(NUKE, 0.6 + 0.4 * (get_time() as f32 * 5.0).sin()),
            );
        }

        if self.build_menu {
            let x = 16.0;
            let y = h - 170.0;
            draw_rectangle(x, y, 330.0, 130.0, PANEL);
            draw_text("BUILD (carried metal)", x + 10.0, y + 24.0, 22.0, WHITE);
            for (i, k) in BuildKind::ALL.iter().enumerate() {
                let ok = p.metal >= k.cost() && g.can_place(p.pos, *k);
                draw_text(
                    format!("[{}] {}  - {} metal", i + 1, k.name(), k.cost()),
                    x + 10.0,
                    y + 54.0 + i as f32 * 26.0,
                    20.0,
                    if ok { WHITE } else { GRAY },
                );
            }
        }

        // Off-screen silo arrow.
        let s = self.view.w2s(V2::ZERO);
        if s.x < 0.0 || s.y < 0.0 || s.x > w || s.y > h {
            let c = vec2(w / 2.0, h / 2.0);
            let d = (s - c).normalize();
            let tip = c + d * (h / 2.0 - 60.0);
            draw_circle(tip.x, tip.y, 8.0, NUKE);
            draw_text("SILO", tip.x + 10.0, tip.y, 18.0, NUKE);
        }
        if let Some(b) = &self.banner {
            centered(&b.text, 150.0, 42.0, alpha(b.color, b.life.min(1.0)));
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
                    ("ENEMY ANNIHILATED - VICTORY", XP)
                } else if g.silo_hp <= 0.0 {
                    ("SILO DESTROYED - GAME OVER", FOE)
                } else {
                    ("COMMANDER LOST - GAME OVER", FOE)
                };
                centered(t, h / 2.0 - 40.0, 52.0, c);
                centered(
                    &format!(
                        "Strikes {}/{}  Kills {}  Level {}  Time {}:{:02}",
                        g.launches,
                        CYCLES,
                        g.kills,
                        g.level,
                        g.time as u32 / 60,
                        g.time as u32 % 60
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
    let r = 120.0 + (t * 2.0).sin() * 8.0;
    draw_circle(w / 2.0, h / 2.0 - 10.0, r, alpha(NUKE, 0.08));
    draw_circle_lines(w / 2.0, h / 2.0 - 10.0, r, 3.0, alpha(NUKE, 0.5));
    centered("NUKE COUNTDOWN", h / 2.0 - 200.0, 76.0, NUKE);
    centered(
        "Arm the Nuclear Silo before the enemy Juggernaut arrives.",
        h / 2.0 - 140.0,
        26.0,
        WHITE,
    );
    let lines = [
        "WASD move - your commander fires automatically",
        "Mine metal deposits and grab drops (metal, energy, xp)",
        "E deposit cargo at the silo   N launch when armed",
        "B build menu: 1 LLT turret  2 Solar (silo energy)  3 Anti-Nuke",
        "Every launch wipes the map - and the enemy fires back. M mute",
        "Land 5 nuclear strikes to win. Lose the silo or commander and it's over.",
    ];
    for (i, l) in lines.iter().enumerate() {
        centered(l, h / 2.0 - 70.0 + i as f32 * 32.0, 24.0, LIGHTGRAY);
    }
    if ((t * 2.0) as u32).is_multiple_of(2) {
        centered("Press any key to start", h / 2.0 + 160.0, 32.0, YELLOW);
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
    touch::set_buttons(&[
        touch::key("Drop", KeyCode::E),
        touch::key("Nuke", KeyCode::N),
        touch::key("Build", KeyCode::B),
        touch::key("Turret", KeyCode::Key1),
        touch::key("Solar", KeyCode::Key2),
        touch::key("Anti", KeyCode::Key3),
        touch::small("Back", KeyCode::Escape),
        touch::small("Mute", KeyCode::M),
        touch::small("Retry", KeyCode::Enter),
    ]);
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
                    seed = (get_time() * 1000.0) as u64 + 1;
                    app = App::new(seed);
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
                let input = app.read_input();
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
        touch::next_frame().await;
    }
}
