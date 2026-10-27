//! Freight Siege: macroquad front end (input, rendering, audio).

mod rules;
mod synth;

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::prelude::*;
use rules::{
    Cargo, EnemyKind, Event, Game, Phase, STATION_COST, TILE, Tile, V2, WAGON_KINDS, WORLD_H,
    WORLD_W, WagonKind,
};
use touch::{
    is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, is_mouse_button_released,
    mouse_position,
};

const HUD_TOP: f32 = 44.0;
const HUD_BOTTOM: f32 = 52.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Freight Siege".to_owned(),
        window_width: 1280,
        window_height: 800,
        high_dpi: false,
        ..Default::default()
    }
}

fn v(p: V2) -> Vec2 {
    vec2(p.x, p.y)
}

#[derive(Clone, Copy)]
enum Sfx {
    Gatling,
    Cannon,
    Tesla,
    Mortar,
    Hit,
    Explosion,
    Pickup,
    LevelUp,
    Build,
    Cash,
    Alarm,
    GameOver,
    Victory,
    Error,
}

struct Audio {
    sounds: Vec<Sound>,
    last: Vec<f64>,
    music: Sound,
    music_started: bool,
    muted: bool,
}

impl Audio {
    async fn load() -> Self {
        let gens: [fn() -> Vec<u8>; 14] = [
            synth::gatling,
            synth::cannon,
            synth::tesla,
            synth::mortar,
            synth::hit,
            synth::explosion,
            synth::pickup,
            synth::level_up,
            synth::build,
            synth::cash,
            synth::alarm,
            synth::game_over,
            synth::victory,
            synth::hit,
        ];
        let mut sounds = Vec::new();
        for g in gens {
            sounds.push(load_sound_from_bytes(&g()).await.expect("synth sound"));
        }
        let music = load_sound_from_bytes(&synth::music())
            .await
            .expect("synth music");
        Self {
            last: vec![-1.0; sounds.len()],
            sounds,
            music,
            music_started: false,
            muted: false,
        }
    }

    /// Start the music loop; must follow a user gesture on the web.
    fn unlock(&mut self) {
        if !self.music_started {
            self.music_started = true;
            play_sound(
                &self.music,
                PlaySoundParams {
                    looped: true,
                    volume: if self.muted { 0.0 } else { 0.45 },
                },
            );
        }
    }

    fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        set_sound_volume(&self.music, if self.muted { 0.0 } else { 0.45 });
    }

    /// Play with a per-sound cooldown so swarms don't stack hundreds of hits.
    fn play(&mut self, s: Sfx, volume: f32, min_gap: f64) {
        if self.muted || !self.music_started {
            return;
        }
        let i = s as usize;
        let now = get_time();
        if now - self.last[i] < min_gap {
            return;
        }
        self.last[i] = now;
        play_sound(
            &self.sounds[i],
            PlaySoundParams {
                looped: false,
                volume,
            },
        );
    }

    fn events(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Shoot(WagonKind::Gatling) => self.play(Sfx::Gatling, 0.35, 0.07),
                Event::Shoot(WagonKind::Cannon) => self.play(Sfx::Cannon, 0.5, 0.12),
                Event::Shoot(WagonKind::Tesla) => self.play(Sfx::Tesla, 0.45, 0.1),
                Event::Shoot(WagonKind::Mortar) => self.play(Sfx::Mortar, 0.6, 0.15),
                Event::Hit => self.play(Sfx::Hit, 0.25, 0.05),
                Event::Kill { big: true } => self.play(Sfx::Explosion, 0.8, 0.1),
                Event::Kill { big: false } => self.play(Sfx::Hit, 0.4, 0.04),
                Event::Pickup => self.play(Sfx::Pickup, 0.35, 0.06),
                Event::LevelUp => self.play(Sfx::LevelUp, 0.6, 0.2),
                Event::Build => self.play(Sfx::Build, 0.6, 0.05),
                Event::Delivered => self.play(Sfx::Cash, 0.45, 0.1),
                Event::StationLost => self.play(Sfx::Explosion, 1.0, 0.0),
                Event::YearStart(_) => self.play(Sfx::Alarm, 0.55, 0.5),
                Event::Won => self.play(Sfx::Victory, 0.8, 0.0),
                Event::Lost => self.play(Sfx::GameOver, 0.8, 0.0),
            }
        }
    }
}

enum Screen {
    Title,
    Play,
}

struct App {
    screen: Screen,
    game: Game,
    menu: bool,
    drag: Option<Vec<Tile>>,
    press_tile: Option<Tile>,
    toast: (String, f32),
    seed: u64,
}

impl App {
    fn new_game(&mut self) {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.game = Game::new(self.seed);
        self.menu = false;
        self.drag = None;
        self.toast = (
            "Planning: drag track, buy wagons, then press SPACE".into(),
            5.0,
        );
    }

    fn say(&mut self, msg: &str) {
        self.toast = (msg.to_owned(), 2.5);
    }
}

fn world_camera() -> Camera2D {
    let (sw, sh) = (screen_width(), screen_height());
    let avail_h = (sh - HUD_TOP - HUD_BOTTOM).max(100.0);
    let s = (sw / WORLD_W).min(avail_h / WORLD_H);
    let mut cam = Camera2D {
        target: vec2(WORLD_W / 2.0, WORLD_H / 2.0),
        zoom: vec2(2.0 * s / sw, 2.0 * s / sh),
        ..Default::default()
    };
    // Shift the world so it sits between the HUD bars.
    let center_y = HUD_TOP + avail_h / 2.0;
    cam.target.y -= (center_y - sh / 2.0) / s;
    cam
}

fn manhattan_steps(from: Tile, to: Tile) -> Vec<Tile> {
    let mut out = Vec::new();
    let mut c = from;
    while c != to {
        if c.0 != to.0 {
            c.0 += (to.0 - c.0).signum();
        } else {
            c.1 += (to.1 - c.1).signum();
        }
        out.push(c);
    }
    out
}

fn handle_world_input(app: &mut App, audio: &mut Audio, cam: &Camera2D) {
    let m = cam.screen_to_world(mouse_position().into());
    let tile = rules::world_to_tile(V2::new(m.x, m.y));
    let g = &mut app.game;

    if is_mouse_button_pressed(MouseButton::Left) {
        app.press_tile = tile;
        if let Some(t) = tile
            && g.paused
            && g.station_at(t).is_some()
        {
            app.drag = Some(vec![t]);
        }
    }
    if is_mouse_button_down(MouseButton::Left)
        && let (Some(path), Some(t)) = (app.drag.as_mut(), tile)
    {
        let last = *path.last().unwrap();
        if t != last {
            if let Some(k) = path.iter().position(|&p| p == t) {
                path.truncate(k + 1);
            } else {
                path.extend(manhattan_steps(last, t));
            }
        }
    }
    if is_mouse_button_released(MouseButton::Left) {
        let pressed = app.press_tile.take();
        if let Some(path) = app.drag.take() {
            if path.len() > 1 {
                match g.build_line(&path) {
                    Ok(()) => app.toast = ("Line built".into(), 1.5),
                    Err(e) => {
                        app.toast = (e.into(), 2.5);
                        audio.play(Sfx::Error, 0.4, 0.1);
                    }
                }
            } else if let Some(i) = g.station_at(path[0]) {
                g.cycle_cargo(i);
                let c = g.stations[i].cargo;
                app.toast = (format!("Station ships {}", cargo_blurb(c)), 2.5);
            }
        } else if let (Some(t), true) = (tile, pressed == tile) {
            if let Some(i) = g.station_at(t) {
                g.cycle_cargo(i);
            } else if let Some(li) = g.lines.iter().position(|l| l.path.contains(&t)) {
                g.selected_line = li;
                app.toast = (format!("Line {} selected for wagons", li + 1), 1.5);
            } else {
                match g.place_station(t) {
                    Ok(()) => app.toast = ("Station built. Drag track to it.".into(), 2.0),
                    Err(e) => {
                        app.toast = (e.into(), 2.5);
                        audio.play(Sfx::Error, 0.4, 0.1);
                    }
                }
            }
        }
    }
}

fn cargo_blurb(c: Cargo) -> &'static str {
    match c {
        Cargo::Coal => "Coal: x1.5 cash",
        Cargo::Goods => "Goods: turret overclock",
        Cargo::Mail => "Mail: bonus XP",
    }
}

fn update_play(app: &mut App, audio: &mut Audio) {
    let dt = get_frame_time().min(1.0 / 20.0);
    app.toast.1 -= dt;

    if app.game.is_over() {
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
            app.new_game();
        }
        if is_key_pressed(KeyCode::Escape) {
            app.screen = Screen::Title;
        }
        return;
    }
    if is_key_pressed(KeyCode::Escape) {
        app.menu = !app.menu;
    }
    if app.menu {
        if is_key_pressed(KeyCode::Enter) {
            app.new_game();
        } else if is_key_pressed(KeyCode::Q) {
            app.menu = false;
            app.screen = Screen::Title;
        }
        return;
    }

    if let Phase::LevelUp(_) = app.game.phase {
        let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3];
        for (i, k) in keys.iter().enumerate() {
            if is_key_pressed(*k) {
                app.game.choose_upgrade(i);
            }
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            for i in 0..3 {
                if card_rect(i).contains(vec2(mx, my)) {
                    app.game.choose_upgrade(i);
                }
            }
        }
    } else {
        if is_key_pressed(KeyCode::Space) {
            app.game.paused = !app.game.paused;
            app.drag = None;
        }
        let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];
        for (i, k) in keys.iter().enumerate() {
            if is_key_pressed(*k) {
                let kind = WAGON_KINDS[i];
                match app.game.buy_wagon(kind) {
                    Ok(()) => app.say(&format!("{} wagon coupled", kind.name())),
                    Err(e) => {
                        app.say(e);
                        audio.play(Sfx::Error, 0.4, 0.1);
                    }
                }
            }
        }
        if is_key_pressed(KeyCode::Tab) && !app.game.lines.is_empty() {
            app.game.selected_line = (app.game.selected_line + 1) % app.game.lines.len();
        }
        if is_key_pressed(KeyCode::R) {
            match app.game.repair_all() {
                Ok(()) => app.say("Network repaired"),
                Err(e) => app.say(e),
            }
        }
        let cam = world_camera();
        handle_world_input(app, audio, &cam);
    }

    let year = app.game.year;
    app.game.update(dt);
    if app.game.year != year {
        app.say("Year survived! Upkeep paid. Plan, repair, then SPACE");
    }
    let events = std::mem::take(&mut app.game.events);
    audio.events(&events);
}

// ---------------------------------------------------------------- drawing

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

fn draw_ground() {
    draw_rectangle(0.0, 0.0, WORLD_W, WORLD_H, Color::from_hex(0x2f3a24));
    for y in 0..rules::GRID_H {
        for x in 0..rules::GRID_W {
            let h = hash(x, y);
            let px = x as f32 * TILE;
            let py = y as f32 * TILE;
            if h.is_multiple_of(3) {
                draw_rectangle(px, py, TILE, TILE, Color::from_hex(0x334027));
            }
            for k in 0..3 {
                let hh = hash(x * 7 + k, y * 13 - k);
                let ox = (hh % 28) as f32 + 2.0;
                let oy = ((hh >> 8) % 28) as f32 + 2.0;
                draw_line(
                    px + ox,
                    py + oy,
                    px + ox + 1.0,
                    py + oy - 3.0,
                    1.0,
                    Color::from_hex(0x46572f),
                );
            }
            let edge = x == 0 || y == 0 || x == rules::GRID_W - 1 || y == rules::GRID_H - 1;
            if edge {
                draw_rectangle(px, py, TILE, TILE, Color::new(0.35, 0.12, 0.05, 0.35));
            }
        }
    }
}

fn draw_track_segment(a: Vec2, b: Vec2, rail: Color) {
    let d = (b - a).normalize_or_zero();
    let n = vec2(-d.y, d.x);
    let len = a.distance(b);
    let mut s = 0.0;
    while s <= len {
        let p = a + d * s;
        draw_line(
            p.x + n.x * 8.0,
            p.y + n.y * 8.0,
            p.x - n.x * 8.0,
            p.y - n.y * 8.0,
            3.0,
            Color::from_hex(0x5a3d22),
        );
        s += 8.0;
    }
    for off in [-5.0, 5.0] {
        draw_line(
            a.x + n.x * off,
            a.y + n.y * off,
            b.x + n.x * off,
            b.y + n.y * off,
            2.0,
            rail,
        );
    }
}

fn draw_tracks(g: &Game) {
    for (li, l) in g.lines.iter().enumerate() {
        for w in l.path.windows(2) {
            let a = v(rules::tile_center(w[0]));
            let b = v(rules::tile_center(w[1]));
            let hp = w
                .iter()
                .filter_map(|t| g.track.get(t))
                .fold(rules::TRACK_HP, |m, h| m.min(*h));
            let f = hp / rules::TRACK_HP;
            let mut rail = Color::new(0.75, 0.75 * f + 0.1, 0.7 * f + 0.1, 1.0);
            if li == g.selected_line && g.paused {
                rail = Color::from_hex(0xffe08a);
            }
            draw_track_segment(a, b, rail);
        }
    }
    for (t, hp) in &g.track {
        if *hp <= 0.0 {
            let c = v(rules::tile_center(*t));
            let red = Color::from_hex(0xff3b30);
            draw_line(c.x - 9.0, c.y - 9.0, c.x + 9.0, c.y + 9.0, 3.0, red);
            draw_line(c.x + 9.0, c.y - 9.0, c.x - 9.0, c.y + 9.0, 3.0, red);
        }
    }
}

fn cargo_color(c: Cargo) -> Color {
    match c {
        Cargo::Coal => Color::from_hex(0x6b6b6b),
        Cargo::Goods => Color::from_hex(0xc98a3a),
        Cargo::Mail => Color::from_hex(0x4f86c6),
    }
}

fn draw_stations(g: &Game, t: f32) {
    let target = g.target_station();
    for (i, s) in g.stations.iter().enumerate() {
        let c = v(s.pos());
        if !s.alive() {
            draw_rectangle(c.x - 13.0, c.y - 8.0, 26.0, 16.0, Color::from_hex(0x3d3530));
            draw_text("X", c.x - 6.0, c.y + 6.0, 22.0, RED);
            continue;
        }
        if Some(i) == target && !g.paused {
            let r = 22.0 + (t * 6.0).sin() * 3.0;
            draw_circle_lines(c.x, c.y, r, 2.0, Color::new(1.0, 0.25, 0.15, 0.7));
        }
        draw_rectangle(c.x - 14.0, c.y - 12.0, 28.0, 24.0, cargo_color(s.cargo));
        draw_triangle(
            vec2(c.x - 17.0, c.y - 10.0),
            vec2(c.x + 17.0, c.y - 10.0),
            vec2(c.x, c.y - 22.0),
            Color::from_hex(0x8c2f1e),
        );
        let letter = &s.cargo.name()[..1];
        draw_text(letter, c.x - 6.0, c.y + 8.0, 22.0, WHITE);
        let f = s.hp / rules::STATION_HP;
        draw_rectangle(c.x - 14.0, c.y + 14.0, 28.0, 4.0, BLACK);
        draw_rectangle(
            c.x - 14.0,
            c.y + 14.0,
            28.0 * f,
            4.0,
            Color::new(1.0 - f, f, 0.2, 1.0),
        );
    }
}

fn wagon_color(k: WagonKind) -> Color {
    match k {
        WagonKind::Gatling => Color::from_hex(0xd9b44a),
        WagonKind::Cannon => Color::from_hex(0x7d8c5a),
        WagonKind::Tesla => Color::from_hex(0x46b8c8),
        WagonKind::Mortar => Color::from_hex(0x9a5fb0),
    }
}

fn draw_trains(g: &Game) {
    for (li, l) in g.lines.iter().enumerate() {
        for k in (0..=l.train.wagons.len()).rev() {
            let idx = l.car_index(k);
            let p = v(l.point(idx));
            let ahead = v(l.point(idx + 0.2)) - v(l.point(idx - 0.2));
            let rot = ahead.y.atan2(ahead.x);
            let (w, h) = (22.0, 14.0);
            let params = |color| DrawRectangleParams {
                offset: vec2(0.5, 0.5),
                rotation: rot,
                color,
            };
            if k == 0 {
                let col = if l.train.blocked {
                    Color::from_hex(0x8a8a8a)
                } else {
                    Color::from_hex(0xb3261e)
                };
                draw_rectangle_ex(p.x, p.y, w + 4.0, h, params(col));
                draw_circle(p.x, p.y, 4.5, Color::from_hex(0x222222));
                if l.train.overclock > 0.0 {
                    draw_circle_lines(p.x, p.y, 12.0, 1.5, ORANGE);
                }
            } else {
                let kind = l.train.wagons[k - 1].kind;
                draw_rectangle_ex(p.x, p.y, w, h, params(Color::from_hex(0x3a3a3a)));
                draw_circle(p.x, p.y, 5.0, wagon_color(kind));
            }
        }
        if li == g.selected_line && g.lines.len() > 1 {
            let p = v(l.car_pos(0));
            draw_triangle(
                vec2(p.x - 5.0, p.y - 20.0),
                vec2(p.x + 5.0, p.y - 20.0),
                vec2(p.x, p.y - 12.0),
                YELLOW,
            );
        }
    }
}

fn draw_enemies(g: &Game, t: f32) {
    for (i, e) in g.enemies.iter().enumerate() {
        let r = e.kind.stats().3;
        let p = v(e.pos);
        let wob = (t * 12.0 + i as f32).sin() * r * 0.3;
        let leg = Color::from_hex(0x5a2a12);
        for k in 0..3 {
            let off = (k as f32 - 1.0) * r * 0.6;
            draw_line(p.x + off, p.y, p.x + off - r * 0.9, p.y + wob, 2.0, leg);
            draw_line(p.x + off, p.y, p.x + off + r * 0.9, p.y - wob, 2.0, leg);
        }
        let body = match e.kind {
            EnemyKind::Crawler => Color::from_hex(0xb5562a),
            EnemyKind::Runner => Color::from_hex(0xd8833a),
            EnemyKind::Brute => Color::from_hex(0x8a3b1c),
            EnemyKind::Behemoth => Color::from_hex(0x6e2410),
        };
        let body = if e.flash > 0.0 { WHITE } else { body };
        draw_circle(p.x, p.y, r, body);
        draw_circle(
            p.x - r * 0.3,
            p.y - r * 0.3,
            r * 0.3,
            Color::from_hex(0x7a3010),
        );
        draw_circle(
            p.x + r * 0.35,
            p.y - r * 0.2,
            r * 0.18,
            Color::from_hex(0xffd23a),
        );
        if matches!(e.kind, EnemyKind::Brute | EnemyKind::Behemoth) {
            let f = e.hp / e.max_hp;
            draw_rectangle(p.x - r, p.y - r - 7.0, 2.0 * r, 3.0, BLACK);
            draw_rectangle(p.x - r, p.y - r - 7.0, 2.0 * r * f, 3.0, RED);
        }
    }
}

fn draw_effects(g: &Game, t: f32) {
    for s in &g.scraps {
        let p = v(s.pos);
        let r = 3.0 + s.value.min(8) as f32 * 0.6;
        draw_poly(p.x, p.y, 4, r, t * 90.0, Color::from_hex(0xf2d14b));
    }
    for tr in &g.tracers {
        let (a, b) = (v(tr.from), v(tr.to));
        match tr.kind {
            WagonKind::Gatling => draw_line(a.x, a.y, b.x, b.y, 1.5, Color::from_hex(0xfff3a0)),
            WagonKind::Cannon => draw_line(a.x, a.y, b.x, b.y, 3.0, Color::from_hex(0xffa040)),
            WagonKind::Mortar => {
                let mid = (a + b) / 2.0 - vec2(0.0, 40.0);
                draw_line(a.x, a.y, mid.x, mid.y, 2.0, Color::from_hex(0xd29cf0));
                draw_line(mid.x, mid.y, b.x, b.y, 2.0, Color::from_hex(0xd29cf0));
            }
            WagonKind::Tesla => {
                let mut prev = a;
                for k in 1..=4 {
                    let f = k as f32 / 4.0;
                    let mut q = a.lerp(b, f);
                    if k < 4 {
                        q += vec2(rand::gen_range(-6.0, 6.0), rand::gen_range(-6.0, 6.0));
                    }
                    draw_line(prev.x, prev.y, q.x, q.y, 2.0, Color::from_hex(0x8ff6ff));
                    prev = q;
                }
            }
        }
    }
    for b in &g.blasts {
        let a = (b.ttl / 0.35).min(1.0);
        draw_circle(
            b.pos.x,
            b.pos.y,
            b.radius * (1.2 - a * 0.5),
            Color::new(1.0, 0.55, 0.15, a * 0.45),
        );
    }
    for f in &g.floaters {
        draw_text(
            &f.text,
            f.pos.x - 14.0,
            f.pos.y - 18.0,
            20.0,
            Color::new(0.5, 1.0, 0.5, f.ttl.min(1.0)),
        );
    }
}

fn draw_drag(app: &App) {
    if let Some(path) = &app.drag {
        for t in path {
            let c = v(rules::tile_center(*t));
            draw_rectangle(
                c.x - 14.0,
                c.y - 14.0,
                28.0,
                28.0,
                Color::new(1.0, 0.9, 0.4, 0.25),
            );
        }
        if let Some(t) = path.last() {
            let c = v(rules::tile_center(*t));
            let cost = app.game.path_cost(path);
            let ok = app.game.station_at(*t).is_some() && path.len() > 1;
            let col = if ok { GREEN } else { YELLOW };
            draw_text(format!("${cost}"), c.x + 12.0, c.y - 12.0, 22.0, col);
        }
    }
}

fn draw_world(app: &App, t: f32) {
    let g = &app.game;
    draw_ground();
    draw_tracks(g);
    draw_drag(app);
    draw_stations(g, t);
    draw_effects(g, t);
    draw_trains(g);
    draw_enemies(g, t);
}

fn text_centered(s: &str, y: f32, size: f32, color: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (screen_width() - d.width) / 2.0, y, size, color);
}

fn draw_hud(app: &App, audio: &Audio) {
    let g = &app.game;
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, HUD_TOP, Color::from_hex(0x15130f));
    let year_f = g.year_time / rules::YEAR_SECS;
    draw_text(
        format!("YEAR {}/{}", g.year, rules::YEARS_TO_WIN),
        12.0,
        28.0,
        28.0,
        WHITE,
    );
    draw_rectangle(140.0, 16.0, 120.0, 12.0, Color::from_hex(0x333333));
    draw_rectangle(140.0, 16.0, 120.0 * year_f, 12.0, Color::from_hex(0xe0a030));
    let cash_col = if g.cash < g.upkeep() {
        RED
    } else {
        Color::from_hex(0x8fe08f)
    };
    draw_text(format!("${:.0}", g.cash), 280.0, 30.0, 30.0, cash_col);
    draw_text(
        format!("upkeep ${:.0}/yr", g.upkeep()),
        400.0,
        28.0,
        20.0,
        LIGHTGRAY,
    );
    draw_text(
        format!("LV {}", g.level),
        560.0,
        28.0,
        26.0,
        Color::from_hex(0x9fd3ff),
    );
    let xf = g.xp as f32 / g.xp_needed() as f32;
    draw_rectangle(620.0, 16.0, 120.0, 12.0, Color::from_hex(0x333333));
    draw_rectangle(
        620.0,
        16.0,
        120.0 * xf.min(1.0),
        12.0,
        Color::from_hex(0x4fa3ff),
    );
    draw_text(format!("kills {}", g.kills), 760.0, 28.0, 20.0, LIGHTGRAY);
    draw_text(
        if audio.muted {
            "[M] sound off"
        } else {
            "[M] sound on"
        },
        sw - 140.0,
        28.0,
        20.0,
        GRAY,
    );

    // Bottom bar: shop and planning help.
    draw_rectangle(
        0.0,
        sh - HUD_BOTTOM,
        sw,
        HUD_BOTTOM,
        Color::from_hex(0x15130f),
    );
    let mut x = 12.0;
    for (i, k) in WAGON_KINDS.iter().enumerate() {
        let spec = k.spec();
        let afford = g.cash >= spec.cost;
        let col = if afford { WHITE } else { DARKGRAY };
        draw_circle(x + 6.0, sh - 34.0, 6.0, wagon_color(*k));
        draw_text(
            format!("[{}] {} ${:.0}", i + 1, k.name(), spec.cost),
            x + 16.0,
            sh - 28.0,
            20.0,
            col,
        );
        x += 170.0;
    }
    if let Some(l) = g.lines.get(g.selected_line) {
        draw_text(
            format!(
                "[Tab] line {}/{}  wagons {}/{}",
                g.selected_line + 1,
                g.lines.len(),
                l.train.wagons.len(),
                rules::MAX_WAGONS
            ),
            x,
            sh - 28.0,
            20.0,
            LIGHTGRAY,
        );
    }
    let repair = g.repair_cost();
    let mode = if g.paused {
        "[Space] RUN"
    } else {
        "[Space] PLAN"
    };
    draw_text(
        format!("[R] repair ${repair:.0}   {mode}   [Esc] menu"),
        12.0,
        sh - 8.0,
        18.0,
        GRAY,
    );

    if g.paused && g.phase == Phase::Playing {
        let banner = format!(
            "PLANNING - world paused | click empty tile: station ${STATION_COST:.0} | drag station->station: track | click station: cargo"
        );
        let d = measure_text(&banner, None, 18, 1.0);
        let bx = (sw - d.width) / 2.0 - 8.0;
        draw_rectangle(
            bx,
            HUD_TOP + 4.0,
            d.width + 16.0,
            26.0,
            Color::new(0.1, 0.25, 0.45, 0.85),
        );
        draw_text(&banner, bx + 8.0, HUD_TOP + 22.0, 18.0, WHITE);
    }
    if app.toast.1 > 0.0 {
        text_centered(
            &app.toast.0,
            sh - HUD_BOTTOM - 14.0,
            24.0,
            Color::new(1.0, 0.95, 0.7, app.toast.1.min(1.0)),
        );
    }
}

fn card_rect(i: usize) -> Rect {
    let w = 240.0;
    let gap = 24.0;
    let total = 3.0 * w + 2.0 * gap;
    let x = (screen_width() - total) / 2.0 + i as f32 * (w + gap);
    Rect::new(x, screen_height() / 2.0 - 80.0, w, 160.0)
}

fn draw_level_up(g: &Game) {
    let Phase::LevelUp(choices) = &g.phase else {
        return;
    };
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    text_centered(
        &format!("LEVEL {}! Choose an upgrade", g.level),
        screen_height() / 2.0 - 110.0,
        40.0,
        YELLOW,
    );
    let (mx, my) = mouse_position();
    for (i, u) in choices.iter().enumerate() {
        let r = card_rect(i);
        let hover = r.contains(vec2(mx, my));
        let bg = if hover {
            Color::from_hex(0x3b4a6b)
        } else {
            Color::from_hex(0x24304a)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, bg);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::from_hex(0x9fd3ff));
        draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 30.0, 24.0, GRAY);
        draw_text(u.title(), r.x + 12.0, r.y + 64.0, 24.0, WHITE);
        let mut y = r.y + 96.0;
        let mut line = String::new();
        for word in u.desc().split(' ') {
            if line.len() + word.len() > 24 {
                draw_text(&line, r.x + 12.0, y, 20.0, LIGHTGRAY);
                y += 22.0;
                line.clear();
            }
            line.push_str(word);
            line.push(' ');
        }
        draw_text(&line, r.x + 12.0, y, 20.0, LIGHTGRAY);
    }
}

fn draw_overlay_box() {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.7),
    );
}

fn draw_end(g: &Game) {
    let (title, sub, col) = match &g.phase {
        Phase::Won => (
            "NETWORK SURVIVED!",
            "Ten years of freight, delivered through the rust.".to_owned(),
            GOLD,
        ),
        Phase::Lost(why) => ("LINE CLOSED", (*why).to_owned(), RED),
        _ => return,
    };
    draw_overlay_box();
    let cy = screen_height() / 2.0;
    text_centered(title, cy - 60.0, 64.0, col);
    text_centered(&sub, cy - 16.0, 26.0, WHITE);
    text_centered(
        &format!(
            "Year {}  |  level {}  |  {} beasts scrapped  |  {} deliveries",
            g.year, g.level, g.kills, g.delivered
        ),
        cy + 24.0,
        24.0,
        LIGHTGRAY,
    );
    text_centered("Enter: play again   Esc: title", cy + 70.0, 24.0, YELLOW);
}

fn draw_menu() {
    draw_overlay_box();
    let cy = screen_height() / 2.0;
    text_centered("MENU", cy - 50.0, 48.0, WHITE);
    text_centered(
        "Esc: resume   Enter: restart   Q: quit to title   M: mute",
        cy,
        24.0,
        LIGHTGRAY,
    );
}

fn draw_title(t: f32, audio: &Audio) {
    clear_background(Color::from_hex(0x1b1f16));
    let sw = screen_width();
    let sh = screen_height();
    // A little train chugging across the screen.
    let y = sh * 0.62;
    draw_track_segment(
        vec2(-20.0, y),
        vec2(sw + 20.0, y),
        Color::from_hex(0xbbbbbb),
    );
    let x = (t * 120.0) % (sw + 300.0) - 150.0;
    for k in 0..5 {
        let px = x - k as f32 * 26.0;
        let col = if k == 0 {
            Color::from_hex(0xb3261e)
        } else {
            Color::from_hex(0x3a3a3a)
        };
        draw_rectangle(px - 11.0, y - 7.0, 24.0, 14.0, col);
        if k > 0 {
            draw_circle(px, y, 5.0, wagon_color(WAGON_KINDS[(k - 1) % 4]));
        }
    }
    for i in 0..6 {
        let ex = x + 120.0 + i as f32 * 40.0;
        let ey = y + 50.0 + (t * 3.0 + i as f32).sin() * 10.0;
        draw_circle(ex, ey, 7.0, Color::from_hex(0xb5562a));
    }
    text_centered("FREIGHT SIEGE", sh * 0.22, 84.0, Color::from_hex(0xe0a030));
    text_centered(
        "Run a rail company on a map overrun by rust-beasts.",
        sh * 0.22 + 44.0,
        26.0,
        WHITE,
    );
    let lines = [
        "SPACE pauses the world: lay track and stations, then let the trains run.",
        "Trains earn cash on delivery; every wagon is a turret. Survive 10 years, stay solvent.",
        "Mouse: click tile = station, drag station->station = track, click station = cargo",
        "1-4 buy wagons  |  Tab select line  |  R repair  |  M mute  |  Esc menu",
    ];
    for (i, l) in lines.iter().enumerate() {
        text_centered(l, sh * 0.36 + i as f32 * 28.0, 22.0, LIGHTGRAY);
    }
    if (t * 2.0) as i32 % 2 == 0 {
        text_centered("Press any key or click to start", sh * 0.8, 32.0, YELLOW);
    }
    let _ = audio;
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
        || !touches().is_empty()
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Plan", KeyCode::Space),
        touch::key("Gatling", KeyCode::Key1),
        touch::key("Cannon", KeyCode::Key2),
        touch::key("Tesla", KeyCode::Key3),
        touch::key("Mortar", KeyCode::Key4),
        touch::key("Repair", KeyCode::R),
        touch::small("Line", KeyCode::Tab),
        touch::small("Menu", KeyCode::Escape),
        touch::small("Start", KeyCode::Enter),
        touch::small("Mute", KeyCode::M),
    ]);
    let mut audio = Audio::load().await;
    let mut app = App {
        screen: Screen::Title,
        game: Game::new(1),
        menu: false,
        drag: None,
        press_tile: None,
        toast: (String::new(), 0.0),
        seed: (miniquad::date::now() * 1000.0) as u64,
    };
    loop {
        let t = get_time() as f32;
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match app.screen {
            Screen::Title => {
                draw_title(t, &audio);
                if any_input() {
                    audio.unlock();
                    audio.play(Sfx::Build, 0.6, 0.0);
                    app.new_game();
                    app.screen = Screen::Play;
                }
            }
            Screen::Play => {
                update_play(&mut app, &mut audio);
                clear_background(Color::from_hex(0x15130f));
                set_camera(&world_camera());
                draw_world(&app, t);
                set_default_camera();
                draw_hud(&app, &audio);
                draw_level_up(&app.game);
                if app.menu {
                    draw_menu();
                }
                draw_end(&app.game);
            }
        }
        touch::next_frame().await;
    }
}
