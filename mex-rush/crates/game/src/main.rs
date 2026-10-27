//! Mex Rush: macroquad front end (input, rendering, audio).

mod audio;

use audio::Audio;
use macroquad::prelude::*;
use sim::game::{
    Event, Game, Input, Kind, LLT_MAX, LossReason, Phase, SHOP, SPOT_RADIUS, STARVE_TIME, WIN_TIME,
    WORLD,
};
use sim::math::V2;

const BG: Color = Color::new(0.08, 0.1, 0.09, 1.0);
const METAL: Color = Color::new(0.55, 0.85, 1.0, 1.0);
const ALLY: Color = Color::new(0.3, 0.75, 1.0, 1.0);
const FOE: Color = Color::new(1.0, 0.35, 0.25, 1.0);

#[derive(PartialEq)]
enum Screen {
    Title,
    Game,
}

struct Spark {
    pos: V2,
    vel: V2,
    life: f32,
    color: Color,
}

struct Banner {
    text: String,
    life: f32,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Mex Rush".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

fn read_input() -> Input {
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
    let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];
    Input {
        dir: d,
        build: is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Space),
        fortify: is_key_pressed(KeyCode::R),
        pick: keys.iter().position(|k| is_key_pressed(*k)),
    }
}

fn any_input() -> bool {
    get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left)
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let m = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (screen_width() - m.width) / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
}

fn fmt_time(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

struct App {
    game: Game,
    audio: Audio,
    screen: Screen,
    minimap: bool,
    sparks: Vec<Spark>,
    banners: Vec<Banner>,
    shake: f32,
    seed: u64,
}

impl App {
    fn handle_events(&mut self) {
        let events = std::mem::take(&mut self.game.events);
        for e in &events {
            self.audio.on_event(e);
            match *e {
                Event::Kill { pos, big } => {
                    let n = if big { 30 } else { 8 };
                    for k in 0..n {
                        let a = k as f32 * 2.4 + pos.x;
                        let s = rand::gen_range(40.0, if big { 260.0 } else { 160.0 });
                        self.sparks.push(Spark {
                            pos,
                            vel: V2::from_angle(a) * s,
                            life: rand::gen_range(0.2, 0.6),
                            color: Color::new(1.0, rand::gen_range(0.4, 0.9), 0.2, 1.0),
                        });
                    }
                    if big {
                        self.shake = self.shake.max(6.0);
                    }
                }
                Event::MexLost { pos } => {
                    for k in 0..40 {
                        self.sparks.push(Spark {
                            pos,
                            vel: V2::from_angle(k as f32 * 0.7) * rand::gen_range(60.0, 300.0),
                            life: rand::gen_range(0.3, 0.9),
                            color: METAL,
                        });
                    }
                    self.shake = 10.0;
                    self.banner("EXTRACTOR LOST");
                }
                Event::Raid { .. } => self.banner("RAID INCOMING - defend the marked mex!"),
                Event::BossSpawn => self.banner("WARNING: Juggernaut approaching"),
                Event::PlayerHit => self.shake = self.shake.max(3.0),
                _ => {}
            }
        }
    }

    fn banner(&mut self, s: &str) {
        self.banners.push(Banner {
            text: s.to_string(),
            life: 3.0,
        });
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::M) {
            self.audio.toggle_mute();
        }
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::N) {
            self.minimap = !self.minimap;
        }
        match self.screen {
            Screen::Title => {
                if any_input() && !is_key_pressed(KeyCode::M) {
                    self.audio.start_music();
                    self.screen = Screen::Game;
                }
            }
            Screen::Game => match self.game.phase {
                Phase::Won | Phase::Lost(_) => {
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                        self.seed += 1;
                        self.game = Game::new(self.seed);
                        self.sparks.clear();
                        self.banners.clear();
                    }
                }
                _ => {
                    let input = read_input();
                    self.game.step(dt, &input);
                }
            },
        }
        self.handle_events();
        for s in &mut self.sparks {
            s.pos += s.vel * dt;
            s.vel = s.vel * 0.92;
            s.life -= dt;
        }
        self.sparks.retain(|s| s.life > 0.0);
        for b in &mut self.banners {
            b.life -= dt;
        }
        self.banners.retain(|b| b.life > 0.0);
        self.shake = (self.shake - dt * 30.0).max(0.0);
    }

    fn camera_offset(&self) -> Vec2 {
        let jitter = vec2(
            rand::gen_range(-1.0, 1.0) * self.shake,
            rand::gen_range(-1.0, 1.0) * self.shake,
        );
        v(self.game.player.pos) - vec2(screen_width(), screen_height()) / 2.0 + jitter
    }

    fn draw_world(&self) {
        let g = &self.game;
        let off = self.camera_offset();
        let s = |p: V2| v(p) - off;
        let t = get_time() as f32;

        // Ground grid and map border.
        let step = 100.0;
        let gx0 = (off.x / step).floor() * step;
        let gy0 = (off.y / step).floor() * step;
        let grid = Color::new(1.0, 1.0, 1.0, 0.04);
        let mut x = gx0;
        while x < off.x + screen_width() + step {
            draw_line(x - off.x, 0.0, x - off.x, screen_height(), 1.0, grid);
            x += step;
        }
        let mut y = gy0;
        while y < off.y + screen_height() + step {
            draw_line(0.0, y - off.y, screen_width(), y - off.y, 1.0, grid);
            y += step;
        }
        draw_rectangle_lines(
            -off.x,
            -off.y,
            WORLD,
            WORLD,
            4.0,
            Color::new(1.0, 0.3, 0.2, 0.5),
        );

        // Metal spots and extractors.
        for (i, sp) in g.spots.iter().enumerate() {
            let c = s(sp.pos);
            draw_circle(c.x, c.y, SPOT_RADIUS, Color::new(0.2, 0.25, 0.3, 1.0));
            draw_circle_lines(c.x, c.y, SPOT_RADIUS, 2.0, Color::new(0.5, 0.7, 0.9, 0.5));
            match &sp.mex {
                None => {
                    for k in 0..3 {
                        let a = k as f32 * 2.1 + i as f32;
                        let p = c + vec2(a.cos(), a.sin()) * 14.0;
                        draw_circle(p.x, p.y, 5.0, Color::new(0.6, 0.65, 0.7, 0.7));
                    }
                }
                Some(m) => {
                    let col = if m.flash > 0.0 { FOE } else { ALLY };
                    draw_poly(c.x, c.y, 8, 26.0, 22.5, Color::new(0.15, 0.3, 0.45, 1.0));
                    draw_poly_lines(c.x, c.y, 8, 26.0, 22.5, 2.0, col);
                    let rot = t * 3.0;
                    for k in 0..3 {
                        let a = rot + k as f32 * 2.094;
                        let p = c + vec2(a.cos(), a.sin()) * 18.0;
                        draw_line(c.x, c.y, p.x, p.y, 4.0, METAL);
                    }
                    draw_circle(c.x, c.y, 6.0, METAL);
                    if m.llt > 0 {
                        // LLT turret beside the mex, size grows per level.
                        let tp = c + vec2(30.0, -30.0);
                        let r = 8.0 + 2.0 * m.llt as f32;
                        draw_rectangle(
                            tp.x - r,
                            tp.y - r,
                            r * 2.0,
                            r * 2.0,
                            Color::new(0.3, 0.35, 0.2, 1.0),
                        );
                        draw_circle(tp.x, tp.y, r * 0.6, Color::new(0.9, 0.9, 0.3, 1.0));
                        for k in 0..m.llt {
                            draw_circle(
                                tp.x - r + 4.0 + k as f32 * 7.0,
                                tp.y + r + 6.0,
                                2.5,
                                YELLOW,
                            );
                        }
                    }
                    let frac = m.hp / g.mex_max_hp();
                    if frac < 0.999 {
                        bar(
                            c.x - 26.0,
                            c.y + 32.0,
                            52.0,
                            5.0,
                            frac,
                            if frac > 0.4 { GREEN } else { RED },
                        );
                    }
                    if m.flash > 0.0 {
                        let pulse = 40.0 + (t * 8.0).sin() * 6.0;
                        draw_circle_lines(c.x, c.y, pulse, 2.0, FOE);
                    }
                }
            }
        }

        // Gems.
        for gem in &g.gems {
            let c = s(gem.pos);
            let r = 3.0 + gem.xp.sqrt() * 1.5;
            draw_poly(c.x, c.y, 4, r, t * 90.0, Color::new(0.4, 1.0, 0.6, 1.0));
        }

        // Enemies.
        for e in &g.enemies {
            let c = s(e.pos);
            let (_, _, r, _, _) = e.kind.stats();
            let base = if e.flash > 0.0 { WHITE } else { FOE };
            match e.kind {
                Kind::Raider => {
                    let a = (v(g.player.pos) - c - off)
                        .y
                        .atan2((v(g.player.pos) - c - off).x);
                    draw_poly(c.x, c.y, 3, r, a.to_degrees(), base);
                }
                Kind::Grunt => draw_poly(c.x, c.y, 4, r, 45.0, base),
                Kind::Tank => {
                    draw_poly(c.x, c.y, 6, r, 0.0, Color::new(0.6, 0.15, 0.1, 1.0));
                    draw_poly_lines(c.x, c.y, 6, r, 0.0, 3.0, base);
                }
                Kind::Boss => {
                    draw_poly(c.x, c.y, 8, r, t * 20.0, Color::new(0.4, 0.05, 0.1, 1.0));
                    draw_poly_lines(c.x, c.y, 8, r, t * 20.0, 4.0, base);
                    draw_circle(c.x, c.y, r * 0.4, ORANGE);
                }
            }
            if matches!(e.kind, Kind::Tank | Kind::Boss) && e.hp < e.max_hp {
                bar(c.x - r, c.y - r - 9.0, r * 2.0, 4.0, e.hp / e.max_hp, RED);
            }
        }

        // Bullets.
        for b in &g.bullets {
            let c = s(b.pos);
            let tail = c - v(b.vel.norm()) * if b.tower { 18.0 } else { 10.0 };
            let col = if b.tower {
                YELLOW
            } else {
                Color::new(0.6, 0.95, 1.0, 1.0)
            };
            draw_line(tail.x, tail.y, c.x, c.y, 3.0, col);
        }

        // Commander.
        let p = &g.player;
        let c = s(p.pos);
        draw_circle_lines(c.x, c.y, p.range, 1.0, Color::new(0.4, 0.8, 1.0, 0.08));
        let body = if p.hurt > 0.0 {
            WHITE
        } else {
            Color::new(0.2, 0.5, 0.95, 1.0)
        };
        draw_circle(c.x, c.y, 16.0, body);
        draw_circle_lines(c.x, c.y, 16.0, 2.0, Color::new(0.8, 0.95, 1.0, 1.0));
        draw_rectangle(
            c.x - 5.0,
            c.y - 22.0,
            10.0,
            12.0,
            Color::new(0.85, 0.9, 1.0, 1.0),
        );

        for sp in &self.sparks {
            let c = s(sp.pos);
            let mut col = sp.color;
            col.a = (sp.life * 2.0).min(1.0);
            draw_rectangle(c.x - 2.0, c.y - 2.0, 4.0, 4.0, col);
        }

        // Edge arrows pointing at extractors under attack.
        let centre = vec2(screen_width(), screen_height()) / 2.0;
        for sp in &g.spots {
            if let Some(m) = &sp.mex {
                let c = s(sp.pos);
                let on = c.x > 0.0 && c.y > 0.0 && c.x < screen_width() && c.y < screen_height();
                if m.flash > 0.0 && !on {
                    let d = (c - centre).normalize();
                    let edge = centre + d * (centre.min_element() - 40.0);
                    let a = d.y.atan2(d.x).to_degrees();
                    draw_poly(edge.x, edge.y, 3, 16.0, a, FOE);
                }
            }
        }
    }

    fn draw_hud(&self) {
        let g = &self.game;
        let w = screen_width();
        let p = &g.player;
        bar(
            16.0,
            16.0,
            240.0,
            16.0,
            p.hp / p.max_hp,
            Color::new(0.3, 0.85, 0.4, 1.0),
        );
        draw_text(
            format!("HP {:.0}/{:.0}", p.hp.max(0.0), p.max_hp),
            22.0,
            29.0,
            18.0,
            WHITE,
        );
        bar(
            16.0,
            38.0,
            240.0,
            8.0,
            g.xp / g.xp_to_next(),
            Color::new(0.4, 1.0, 0.6, 1.0),
        );
        draw_text(format!("LV {}", g.level), 264.0, 47.0, 20.0, WHITE);
        draw_text(
            format!("Metal {:.0}  (+{:.1}/s)", g.metal, g.income()),
            16.0,
            72.0,
            24.0,
            METAL,
        );
        draw_text(
            format!("Mexes {}/{}   Kills {}", g.owned(), g.spots.len(), g.kills),
            16.0,
            96.0,
            20.0,
            WHITE,
        );

        let left = WIN_TIME - g.time;
        centered(
            &format!("Extraction in {}", fmt_time(left)),
            30.0,
            28.0,
            WHITE,
        );
        bar(w / 2.0 - 150.0, 38.0, 300.0, 5.0, g.time / WIN_TIME, ALLY);

        // Shop.
        let mut y = 130.0;
        draw_text("UPGRADES (metal)", 16.0, y, 18.0, GRAY);
        for (i, item) in SHOP.iter().enumerate() {
            y += 22.0;
            let cost = g.shop_cost(i);
            let col = if g.metal >= cost { WHITE } else { DARKGRAY };
            draw_text(
                format!("[{}] {} - {:.0}", i + 1, item.label(), cost),
                16.0,
                y,
                20.0,
                col,
            );
        }

        // Context hint for the spot under the commander.
        if let Some(i) = g.spot_here() {
            let msg = match &g.spots[i].mex {
                None => format!("[E] Build Metal Extractor ({:.0} metal)", g.mex_cost()),
                Some(m) if m.llt < LLT_MAX => format!(
                    "[R] {} LLT ({:.0} metal)",
                    if m.llt == 0 {
                        "Fortify with"
                    } else {
                        "Upgrade"
                    },
                    Game::llt_cost(m.llt)
                ),
                Some(_) => "Extractor fully fortified".to_string(),
            };
            centered(&msg, screen_height() - 80.0, 26.0, YELLOW);
        } else if g.owned() == 0 {
            centered(
                "Find a metal spot and press [E] to build an extractor",
                screen_height() - 80.0,
                24.0,
                YELLOW,
            );
        }

        if g.owned() == 0 && g.starve < STARVE_TIME - 0.01 {
            centered(
                &format!("NO INCOME - starving in {:.0}s", g.starve.max(0.0)),
                screen_height() / 2.0 - 120.0,
                36.0,
                FOE,
            );
        }

        for (k, b) in self.banners.iter().enumerate() {
            let mut c = Color::new(1.0, 0.8, 0.3, 1.0);
            c.a = b.life.min(1.0);
            centered(&b.text, 90.0 + k as f32 * 30.0, 28.0, c);
        }

        let mute = if self.audio.muted {
            "muted"
        } else {
            "sound on"
        };
        draw_text(
            format!("[M] {mute}   [Tab] minimap"),
            w - 250.0,
            screen_height() - 14.0,
            18.0,
            GRAY,
        );

        if self.minimap {
            self.draw_minimap();
        }
    }

    fn draw_minimap(&self) {
        let g = &self.game;
        let size = 200.0;
        let x0 = screen_width() - size - 16.0;
        let y0 = 16.0;
        let k = size / WORLD;
        draw_rectangle(x0, y0, size, size, Color::new(0.0, 0.0, 0.0, 0.65));
        draw_rectangle_lines(x0, y0, size, size, 1.0, GRAY);
        let m = |p: V2| {
            vec2(
                x0 + p.x.clamp(0.0, WORLD) * k,
                y0 + p.y.clamp(0.0, WORLD) * k,
            )
        };
        for e in &g.enemies {
            let c = m(e.pos);
            draw_rectangle(c.x - 1.0, c.y - 1.0, 2.0, 2.0, FOE);
        }
        for sp in &g.spots {
            let c = m(sp.pos);
            match &sp.mex {
                Some(mx) => {
                    let col = if mx.flash > 0.0 { ORANGE } else { ALLY };
                    draw_circle(c.x, c.y, 5.0, col);
                }
                None => draw_circle_lines(c.x, c.y, 4.0, 1.0, LIGHTGRAY),
            }
        }
        let c = m(g.player.pos);
        draw_circle(c.x, c.y, 3.5, WHITE);
    }

    fn draw_level_up(&self) {
        let g = &self.game;
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        centered("LEVEL UP - choose an upgrade", 150.0, 40.0, YELLOW);
        let cw = 260.0;
        let total = cw * 3.0 + 40.0;
        let x0 = (screen_width() - total) / 2.0;
        for (i, c) in g.choices.iter().enumerate() {
            let x = x0 + i as f32 * (cw + 20.0);
            let y = 200.0;
            draw_rectangle(x, y, cw, 150.0, Color::new(0.1, 0.15, 0.25, 0.95));
            draw_rectangle_lines(x, y, cw, 150.0, 2.0, ALLY);
            let (name, desc) = c.label();
            draw_text(format!("[{}]", i + 1), x + 12.0, y + 32.0, 28.0, YELLOW);
            draw_text(name, x + 12.0, y + 72.0, 26.0, WHITE);
            draw_text(desc, x + 12.0, y + 108.0, 20.0, LIGHTGRAY);
        }
    }

    fn draw_title(&self) {
        let h = screen_height();
        centered("MEX RUSH", h * 0.28, 96.0, METAL);
        centered(
            "A territory-control survivor",
            h * 0.28 + 44.0,
            28.0,
            LIGHTGRAY,
        );
        let lines = [
            "WASD / arrows: move   (your commander fires automatically)",
            "E / Space: build a Metal Extractor on a metal spot",
            "R: fortify the extractor with a Light Laser Tower (up to 3 levels)",
            "1-4: buy global upgrades with metal income",
            "Tab / N: minimap     M: mute",
            "Raiders target your extractors. Lose them all and you starve.",
            "Survive 10:00 to extract.",
        ];
        for (i, l) in lines.iter().enumerate() {
            centered(l, h * 0.45 + i as f32 * 30.0, 22.0, WHITE);
        }
        let blink = (get_time() * 2.0).sin() > 0.0;
        if blink {
            centered("Press any key to start", h * 0.85, 32.0, YELLOW);
        }
    }

    fn draw_end(&self) {
        let g = &self.game;
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.7),
        );
        let h = screen_height();
        let (title, sub, col) = match g.phase {
            Phase::Won => ("EXTRACTION COMPLETE", "Your economy held the line.", GREEN),
            Phase::Lost(LossReason::Starved) => (
                "STARVED",
                "Every extractor fell and the metal ran dry.",
                FOE,
            ),
            _ => ("COMMANDER DESTROYED", "The raiders broke through.", FOE),
        };
        centered(title, h * 0.35, 72.0, col);
        centered(sub, h * 0.35 + 50.0, 28.0, LIGHTGRAY);
        centered(
            &format!(
                "Survived {}   Level {}   Kills {}   Extractors built {}",
                fmt_time(g.time),
                g.level,
                g.kills,
                g.mexes_built
            ),
            h * 0.5,
            26.0,
            WHITE,
        );
        centered("Press Enter to play again", h * 0.65, 30.0, YELLOW);
    }

    fn draw(&self) {
        clear_background(BG);
        match self.screen {
            Screen::Title => self.draw_title(),
            Screen::Game => {
                self.draw_world();
                self.draw_hud();
                match self.game.phase {
                    Phase::LevelUp => self.draw_level_up(),
                    Phase::Won | Phase::Lost(_) => self.draw_end(),
                    Phase::Playing => {}
                }
            }
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let seed = (miniquad::date::now() * 1000.0) as u64;
    rand::srand(seed);
    let mut app = App {
        game: Game::new(seed),
        audio: Audio::new().await,
        screen: Screen::Title,
        minimap: true,
        sparks: Vec::new(),
        banners: Vec::new(),
        shake: 0.0,
        seed,
    };
    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        app.update(dt);
        app.draw();
        next_frame().await;
    }
}
