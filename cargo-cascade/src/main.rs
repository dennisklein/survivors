//! Cargo Cascade: macroquad front end (input, rendering, audio).

mod rules;
mod synth;

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::prelude::*;
use rules::{
    Cargo, EnemyKind, Engine, Event, Game, Kind, LAST_NIGHT, Outcome, Phase, TrainState, WORLD_H,
    WORLD_W,
};

const PANEL_W: f32 = 290.0;
const TOP_H: f32 = 44.0;
const BOTTOM_H: f32 = 30.0;
const MUSIC_VOL: f32 = 0.4;

#[derive(Clone, Copy)]
enum Sfx {
    Shoot,
    Hit,
    Explosion,
    Pickup,
    LevelUp,
    Build,
    Cash,
    Alarm,
    GameOver,
    Victory,
    Whistle,
    Error,
    Dawn,
    Wreck,
}

struct Audio {
    sounds: Vec<Sound>,
    last: Vec<f64>,
    music: Sound,
    started: bool,
    muted: bool,
}

impl Audio {
    async fn load() -> Self {
        let gens: [fn() -> Vec<u8>; 14] = [
            synth::shoot,
            synth::hit,
            synth::explosion,
            synth::pickup,
            synth::level_up,
            synth::build,
            synth::cash,
            synth::alarm,
            synth::game_over,
            synth::victory,
            synth::whistle,
            synth::error,
            synth::dawn,
            synth::wreck,
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
            started: false,
            muted: false,
        }
    }

    /// Start the music loop; called after the first user input so browsers
    /// allow the audio context to run.
    fn unlock(&mut self) {
        if !self.started {
            self.started = true;
            play_sound(
                &self.music,
                PlaySoundParams {
                    looped: true,
                    volume: if self.muted { 0.0 } else { MUSIC_VOL },
                },
            );
        }
    }

    fn set_music(&self, vol: f32) {
        if self.started {
            set_sound_volume(&self.music, if self.muted { 0.0 } else { vol });
        }
    }

    fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.set_music(MUSIC_VOL);
    }

    /// Play with a per-sound cooldown so swarms don't stack hundreds of hits.
    fn play(&mut self, s: Sfx, volume: f32, min_gap: f64) {
        if self.muted || !self.started {
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
}

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Title,
    Play,
    Over,
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    color: Color,
    size: f32,
}

struct Float {
    pos: Vec2,
    text: String,
    life: f32,
    color: Color,
}

struct App {
    game: Game,
    screen: Screen,
    cam: Vec2,
    zoom: f32,
    selected: Option<usize>,
    hover_route: Option<usize>,
    engine: Engine,
    turret_mode: bool,
    fast: bool,
    paused: bool,
    msg: String,
    msg_t: f32,
    particles: Vec<Particle>,
    floats: Vec<Float>,
    decor: Vec<(Vec2, f32, u8)>,
    scroll: usize,
    drag: Option<Vec2>,
    seed: u64,
}

impl App {
    fn new(seed: u64) -> Self {
        let mut rng = rules::Rng::new(seed ^ 0x9e37_79b9);
        let decor = (0..260)
            .map(|_| {
                (
                    vec2(rng.range(0.0, WORLD_W), rng.range(0.0, WORLD_H)),
                    rng.range(0.6, 1.4),
                    rng.pick(3) as u8,
                )
            })
            .collect();
        Self {
            game: Game::new(seed),
            screen: Screen::Title,
            cam: vec2(WORLD_W / 2.0, WORLD_H / 2.0),
            zoom: 0.0,
            selected: None,
            hover_route: None,
            engine: Engine::Kettle,
            turret_mode: false,
            fast: false,
            paused: false,
            msg: String::new(),
            msg_t: 0.0,
            particles: Vec::new(),
            floats: Vec::new(),
            decor,
            scroll: 0,
            drag: None,
            seed,
        }
    }

    fn restart(&mut self) {
        let seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let screen = self.screen;
        *self = App::new(seed);
        self.screen = screen;
    }

    fn say(&mut self, m: impl Into<String>) {
        self.msg = m.into();
        self.msg_t = 3.0;
    }

    fn view(&self) -> Rect {
        Rect::new(
            0.0,
            TOP_H,
            screen_width() - PANEL_W,
            screen_height() - TOP_H - BOTTOM_H,
        )
    }

    fn fit_zoom(&self) -> f32 {
        let v = self.view();
        (v.w / (WORLD_W + 60.0)).min(v.h / (WORLD_H + 60.0))
    }

    fn w2s(&self, p: Vec2) -> Vec2 {
        let v = self.view();
        (p - self.cam) * self.zoom + v.center()
    }

    fn s2w(&self, s: Vec2) -> Vec2 {
        let v = self.view();
        (s - v.center()) / self.zoom + self.cam
    }

    fn burst(&mut self, pos: Vec2, n: usize, color: Color, speed: f32, size: f32) {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU + rand::gen_range(0.0, 0.6);
            let v = vec2(a.cos(), a.sin()) * speed * rand::gen_range(0.3, 1.0);
            let life = rand::gen_range(0.3, 0.7);
            self.particles.push(Particle {
                pos,
                vel: v,
                life,
                max: life,
                color,
                size: size * rand::gen_range(0.6, 1.2),
            });
        }
    }

    fn float(&mut self, pos: Vec2, text: String, color: Color) {
        if self.floats.len() < 40 {
            self.floats.push(Float {
                pos,
                text,
                life: 1.2,
                color,
            });
        }
    }
}

fn cargo_color(c: Cargo) -> Color {
    match c {
        Cargo::Coal => Color::from_rgba(40, 40, 46, 255),
        Cargo::Power => Color::from_rgba(255, 220, 70, 255),
        Cargo::Goods => Color::from_rgba(240, 140, 60, 255),
    }
}

fn engine_color(e: Engine) -> Color {
    match e {
        Engine::Kettle => Color::from_rgba(70, 150, 80, 255),
        Engine::Mogul => Color::from_rgba(190, 60, 50, 255),
        Engine::Express => Color::from_rgba(60, 110, 210, 255),
    }
}

fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Mine => Color::from_rgba(120, 90, 60, 255),
        Kind::Plant => Color::from_rgba(150, 150, 165, 255),
        Kind::Factory => Color::from_rgba(90, 120, 160, 255),
        Kind::Town => Color::from_rgba(210, 180, 120, 255),
    }
}

fn button(r: Rect, label: &str, enabled: bool) -> bool {
    let m: Vec2 = mouse_position().into();
    let hot = enabled && r.contains(m);
    let bg = if !enabled {
        Color::from_rgba(50, 50, 55, 255)
    } else if hot {
        Color::from_rgba(90, 110, 90, 255)
    } else {
        Color::from_rgba(60, 75, 62, 255)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        1.0,
        Color::from_rgba(150, 170, 140, 255),
    );
    let ts = measure_text(label, None, 16, 1.0);
    draw_text(
        label,
        r.x + (r.w - ts.width) / 2.0,
        r.y + r.h / 2.0 + 5.0,
        16.0,
        if enabled { WHITE } else { GRAY },
    );
    hot && is_mouse_button_pressed(MouseButton::Left)
}

fn handle_events(app: &mut App, audio: &mut Audio, events: &[Event]) {
    for e in events {
        match *e {
            Event::Shoot(_) => audio.play(Sfx::Shoot, 0.22, 0.06),
            Event::Hit(p) => {
                audio.play(Sfx::Hit, 0.25, 0.05);
                app.burst(p, 4, Color::from_rgba(255, 230, 150, 255), 60.0, 2.0);
            }
            Event::Kill(p, k) => {
                let big = matches!(k, EnemyKind::Brute | EnemyKind::Behemoth);
                audio.play(
                    Sfx::Explosion,
                    if big { 0.8 } else { 0.35 },
                    if big { 0.0 } else { 0.08 },
                );
                let n = if big { 30 } else { 10 };
                app.burst(p, n, Color::from_rgba(150, 230, 120, 255), 120.0, 3.0);
                app.burst(p, n / 2, Color::from_rgba(255, 160, 60, 255), 80.0, 4.0);
            }
            Event::Pickup(p, v) => {
                audio.play(Sfx::Pickup, 0.2, 0.06);
                app.float(p, format!("+${v:.0}"), GOLD);
            }
            Event::Delivered(p, v) => {
                audio.play(Sfx::Cash, 0.22, 0.12);
                app.float(p, format!("+${v:.0}"), Color::from_rgba(140, 255, 140, 255));
            }
            Event::TrackDone(_) => {
                audio.play(Sfx::Build, 0.6, 0.0);
                app.say("Track complete - trains can run");
            }
            Event::Wrecked(i) => {
                audio.play(Sfx::Wreck, 0.7, 0.0);
                let ind = &app.game.industries[i];
                let (p, label) = (ind.pos, ind.label());
                app.burst(p, 24, Color::from_rgba(255, 90, 40, 255), 140.0, 4.0);
                app.say(format!("{label} wrecked! Hover it and press R to repair"));
            }
            Event::LevelUp => audio.play(Sfx::LevelUp, 0.55, 0.0),
            Event::NightFall(n) => {
                audio.play(Sfx::Alarm, 0.5, 0.0);
                audio.set_music(MUSIC_VOL * 1.2);
                let boss = if n.is_multiple_of(5) {
                    " A Behemoth stirs!"
                } else {
                    ""
                };
                app.say(format!("Night {n} falls - monsters approach!{boss}"));
            }
            Event::Dawn(n) => {
                audio.play(Sfx::Dawn, 0.5, 0.0);
                audio.set_music(MUSIC_VOL);
                app.say(format!("Dawn after night {n}. Tracks resume building."));
            }
            Event::TownConnected(i) => {
                audio.play(Sfx::Whistle, 0.6, 0.0);
                let label = app.game.industries[i].label();
                app.say(format!("{label} connected! Its aura blazes."));
            }
            Event::GameOver => {
                audio.play(Sfx::GameOver, 0.7, 0.0);
                audio.set_music(MUSIC_VOL * 0.3);
            }
            Event::Victory => {
                audio.play(Sfx::Victory, 0.7, 0.0);
                audio.set_music(MUSIC_VOL * 0.5);
            }
        }
    }
}

fn act_err(app: &mut App, audio: &mut Audio, r: Result<(), &'static str>) -> bool {
    match r {
        Ok(()) => {
            audio.play(Sfx::Build, 0.5, 0.0);
            true
        }
        Err(e) => {
            audio.play(Sfx::Error, 0.4, 0.1);
            app.say(e);
            false
        }
    }
}

fn update_input(app: &mut App, audio: &mut Audio) {
    let mouse: Vec2 = mouse_position().into();
    let view = app.view();
    let in_view = view.contains(mouse);
    let wm = app.s2w(mouse);

    if app.game.perk_choices.is_some() {
        for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
            .iter()
            .enumerate()
        {
            if is_key_pressed(*key) {
                pick_perk(app, audio, k);
            }
        }
        return;
    }

    if is_key_pressed(KeyCode::Space) {
        app.fast = !app.fast;
    }
    if is_key_pressed(KeyCode::P) {
        app.paused = !app.paused;
    }
    if is_key_pressed(KeyCode::Escape) {
        if app.selected.is_some() || app.turret_mode {
            app.selected = None;
            app.turret_mode = false;
        } else {
            app.paused = !app.paused;
        }
    }
    for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .iter()
        .enumerate()
    {
        if is_key_pressed(*key) {
            app.engine = Engine::ALL[k];
            app.say(format!(
                "Engine: {} (${:.0}, speed {:.0}, {} cargo)",
                app.engine.name(),
                app.engine.cost(),
                app.engine.speed(),
                app.engine.capacity()
            ));
        }
    }
    if is_key_pressed(KeyCode::T) {
        app.turret_mode = !app.turret_mode;
        app.selected = None;
    }

    // Camera: keys, drag and wheel zoom.
    let pan = 500.0 / app.zoom * get_frame_time();
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        app.cam.x -= pan;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        app.cam.x += pan;
    }
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        app.cam.y -= pan;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        app.cam.y += pan;
    }
    if in_view
        && (is_mouse_button_pressed(MouseButton::Right)
            || is_mouse_button_pressed(MouseButton::Middle))
    {
        app.drag = Some(mouse);
    }
    if let Some(start) = app.drag {
        if is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Middle) {
            app.cam -= (mouse - start) / app.zoom;
            app.drag = Some(mouse);
        } else {
            if (mouse - start).length() < 3.0 && is_mouse_button_released(MouseButton::Right) {
                app.selected = None;
                app.turret_mode = false;
            }
            app.drag = None;
        }
    }
    let wheel = mouse_wheel().1;
    if wheel != 0.0 && in_view {
        let before = app.s2w(mouse);
        let f = if wheel > 0.0 { 1.12 } else { 1.0 / 1.12 };
        let fit = app.fit_zoom();
        app.zoom = (app.zoom * f).clamp(fit * 0.8, fit * 4.0);
        let after = app.s2w(mouse);
        app.cam += before - after;
    } else if wheel != 0.0 && mouse.x > screen_width() - PANEL_W {
        if wheel < 0.0 {
            app.scroll += 1;
        } else {
            app.scroll = app.scroll.saturating_sub(1);
        }
    }
    app.cam.x = app.cam.x.clamp(0.0, WORLD_W);
    app.cam.y = app.cam.y.clamp(0.0, WORLD_H);

    let hovered = app.game.industry_at(wm, 30.0);
    if is_key_pressed(KeyCode::R) {
        if let Some(i) = hovered {
            let res = app.game.repair(i);
            if act_err(app, audio, res) {
                let label = app.game.industries[i].label();
                app.say(format!("{label} repaired"));
            }
        } else {
            app.say("Hover over a damaged industry and press R");
        }
    }

    if in_view && is_mouse_button_pressed(MouseButton::Left) {
        if app.turret_mode {
            let res = app.game.place_turret(wm);
            if act_err(app, audio, res) {
                app.say("Signal box built");
            }
        } else if let Some(i) = hovered {
            match app.selected {
                None => {
                    if app.game.industries[i].kind.output().is_some() {
                        app.selected = Some(i);
                        let to = match app.game.industries[i].kind {
                            Kind::Mine => "a Power Plant",
                            Kind::Plant => "a Factory",
                            _ => "a Town",
                        };
                        app.say(format!("Now click {to} to lay a route"));
                    } else {
                        audio.play(Sfx::Error, 0.4, 0.1);
                        app.say("Towns receive goods: click a Factory first");
                    }
                }
                Some(a) if a == i => app.selected = None,
                Some(a) => match app.game.create_route(a, i) {
                    Ok(r) => {
                        audio.play(Sfx::Build, 0.6, 0.0);
                        let auto = app.game.add_train(r, app.engine).is_ok();
                        app.say(if auto {
                            "Route planned; a train is waiting for the track"
                        } else {
                            "Route planned; buy a train in the timetable"
                        });
                        app.selected = None;
                        app.scroll = app.game.routes.len().saturating_sub(6);
                    }
                    Err(e) => {
                        audio.play(Sfx::Error, 0.4, 0.1);
                        app.say(e);
                    }
                },
            }
        } else {
            app.selected = None;
        }
    }
}

fn pick_perk(app: &mut App, audio: &mut Audio, k: usize) {
    if let Some(p) = app.game.choose_perk(k) {
        audio.play(Sfx::Build, 0.5, 0.0);
        app.say(format!("{}: {}", p.title(), p.desc()));
    }
}

fn update_play(app: &mut App, audio: &mut Audio) {
    update_input(app, audio);
    if app.screen != Screen::Play {
        return;
    }
    let frame = get_frame_time().min(0.05);
    if !app.paused {
        let dt = frame * if app.fast { 3.0 } else { 1.0 };
        let n = (dt / 0.034).ceil().max(1.0) as usize;
        for _ in 0..n {
            let ev = app.game.update(dt / n as f32);
            handle_events(app, audio, &ev);
        }
    }
    for p in &mut app.particles {
        p.pos += p.vel * frame;
        p.vel *= 0.92;
        p.life -= frame;
    }
    app.particles.retain(|p| p.life > 0.0);
    if app.particles.len() > 600 {
        let extra = app.particles.len() - 600;
        app.particles.drain(0..extra);
    }
    for f in &mut app.floats {
        f.pos.y -= 30.0 * frame;
        f.life -= frame;
    }
    app.floats.retain(|f| f.life > 0.0);
    app.msg_t -= frame;
    if app.game.outcome.is_some() {
        app.screen = Screen::Over;
    }
}

// ---------------------------------------------------------------- drawing

fn draw_world(app: &App) {
    let v = app.view();
    let z = app.zoom;
    let t = get_time() as f32;
    let g = &app.game;
    draw_rectangle(v.x, v.y, v.w, v.h, Color::from_rgba(18, 28, 22, 255));
    let tl = app.w2s(vec2(0.0, 0.0));
    draw_rectangle(
        tl.x,
        tl.y,
        WORLD_W * z,
        WORLD_H * z,
        Color::from_rgba(52, 84, 50, 255),
    );
    for &(p, s, k) in &app.decor {
        let sp = app.w2s(p);
        if !v.contains(sp) {
            continue;
        }
        match k {
            0 => draw_circle(sp.x, sp.y, 6.0 * s * z, Color::from_rgba(38, 66, 40, 255)),
            1 => {
                draw_triangle(
                    sp + vec2(0.0, -12.0) * s * z,
                    sp + vec2(-6.0, 3.0) * s * z,
                    sp + vec2(6.0, 3.0) * s * z,
                    Color::from_rgba(30, 58, 36, 255),
                );
            }
            _ => draw_circle(sp.x, sp.y, 2.0 * s * z, Color::from_rgba(70, 100, 60, 255)),
        }
    }

    // Town auras (under everything).
    for (i, town) in g.towns() {
        let (r, d) = g.aura(i);
        if r <= 0.0 {
            continue;
        }
        let sp = app.w2s(town.pos);
        let strong = d > 5.0;
        let pulse = 0.5 + 0.5 * (t * 3.0 + i as f32).sin();
        let c = if strong {
            Color::new(1.0, 0.85, 0.35, 0.10 + 0.05 * pulse)
        } else {
            Color::new(0.7, 0.7, 0.9, 0.06)
        };
        draw_circle(sp.x, sp.y, r * z, c);
        draw_circle_lines(
            sp.x,
            sp.y,
            r * z,
            1.5,
            Color::new(c.r, c.g, c.b, 0.35 + 0.2 * pulse),
        );
    }

    // Tracks.
    for (ri, r) in g.routes.iter().enumerate() {
        let a = app.w2s(r.pa);
        let b = app.w2s(r.pb);
        let built = app.w2s(r.point(r.built));
        let dir = (b - a).normalize_or_zero();
        let perp = vec2(-dir.y, dir.x);
        let hl = app.hover_route == Some(ri);
        // Planned (dashed) part.
        let total = (b - built).length();
        let mut d = 0.0;
        while d < total {
            let p0 = built + dir * d;
            let p1 = built + dir * (d + 6.0).min(total);
            draw_line(p0.x, p0.y, p1.x, p1.y, 2.0, Color::new(0.9, 0.9, 0.7, 0.45));
            d += 12.0;
        }
        if r.built > 0.0 {
            let w = 3.5 * (z * 1.3).max(0.75);
            let blen = (built - a).length();
            let step = (12.0 * z).max(5.0);
            let mut s = 0.0;
            while s < blen {
                let p = a + dir * s;
                let q0 = p + perp * w * 1.3;
                let q1 = p - perp * w * 1.3;
                draw_line(
                    q0.x,
                    q0.y,
                    q1.x,
                    q1.y,
                    2.0,
                    Color::from_rgba(90, 60, 40, 255),
                );
                s += step;
            }
            let rail = if hl {
                YELLOW
            } else {
                Color::from_rgba(170, 170, 180, 255)
            };
            for side in [-1.0, 1.0] {
                let o = perp * w * 0.7 * side;
                draw_line(
                    a.x + o.x,
                    a.y + o.y,
                    built.x + o.x,
                    built.y + o.y,
                    1.5,
                    rail,
                );
            }
        }
        if !r.is_built() {
            let pct = r.built / r.len;
            let c = app.w2s(r.point(r.built));
            draw_circle(c.x, c.y, 4.0, ORANGE);
            draw_text(
                format!("{:.0}%", pct * 100.0),
                c.x + 6.0,
                c.y - 6.0,
                14.0,
                ORANGE,
            );
        }
    }

    // Industries.
    for (i, ind) in g.industries.iter().enumerate() {
        draw_industry(app, i, ind, t);
    }

    // Trains.
    for r in &g.routes {
        if !r.is_built() {
            continue;
        }
        let dir = r.dir();
        for tr in &r.trains {
            let towards_b = matches!(tr.state, TrainState::ToB | TrainState::Unloading(_));
            let fwd = if towards_b { dir } else { -dir };
            let p = app.w2s(r.point(tr.dist));
            let rot = fwd.y.atan2(fwd.x);
            let s = (z * 1.3).max(0.75);
            // Wagon behind.
            let wp = p - fwd * 15.0 * s;
            let wc = if tr.load > 0.0 {
                cargo_color(r.cargo)
            } else {
                Color::from_rgba(110, 100, 90, 255)
            };
            draw_rectangle_ex(
                wp.x,
                wp.y,
                12.0 * s,
                8.0 * s,
                DrawRectangleParams {
                    offset: vec2(0.5, 0.5),
                    rotation: rot,
                    color: wc,
                },
            );
            draw_rectangle_ex(
                p.x,
                p.y,
                16.0 * s,
                9.0 * s,
                DrawRectangleParams {
                    offset: vec2(0.5, 0.5),
                    rotation: rot,
                    color: engine_color(tr.engine),
                },
            );
            let lamp = p + fwd * 8.0 * s;
            draw_circle(
                lamp.x,
                lamp.y,
                2.0 * s,
                Color::from_rgba(255, 250, 200, 255),
            );
            if matches!(tr.state, TrainState::ToA | TrainState::ToB) {
                let puff = p - fwd * 3.0 * s + vec2(0.0, -6.0 * s - (t * 4.0).fract() * 6.0);
                draw_circle(
                    puff.x,
                    puff.y,
                    (2.0 + (t * 4.0).fract() * 3.0) * s,
                    Color::new(0.9, 0.9, 0.9, 0.5 * (1.0 - (t * 4.0).fract())),
                );
            }
        }
    }

    // Signal boxes.
    for tur in &g.turrets {
        let p = app.w2s(tur.pos);
        let s = (z * 1.3).max(0.75);
        draw_rectangle(
            p.x - 6.0 * s,
            p.y - 5.0 * s,
            12.0 * s,
            10.0 * s,
            Color::from_rgba(120, 70, 45, 255),
        );
        draw_rectangle(
            p.x - 7.0 * s,
            p.y - 8.0 * s,
            14.0 * s,
            4.0 * s,
            Color::from_rgba(70, 40, 30, 255),
        );
        let tip = p + tur.aim * 12.0 * s;
        draw_line(
            p.x,
            p.y,
            tip.x,
            tip.y,
            3.0 * s,
            Color::from_rgba(60, 60, 70, 255),
        );
        let firing = tur.cooldown > 0.35;
        draw_circle(
            p.x,
            p.y - 12.0 * s,
            3.0 * s,
            if firing { RED } else { GREEN },
        );
        draw_line(p.x, p.y - 8.0 * s, p.x, p.y - 12.0 * s, 1.5, DARKGRAY);
    }

    // Enemies.
    for e in &g.enemies {
        let p = app.w2s(e.pos);
        if !v.contains(p) {
            continue;
        }
        let r = e.radius * (z * 1.3).max(0.75);
        let base = match e.kind {
            EnemyKind::Gremlin => Color::from_rgba(150, 70, 170, 255),
            EnemyKind::Saboteur => Color::from_rgba(200, 60, 60, 255),
            EnemyKind::Brute => Color::from_rgba(110, 40, 40, 255),
            EnemyKind::Behemoth => Color::from_rgba(60, 20, 70, 255),
        };
        let c = if e.flash > 0.0 { WHITE } else { base };
        let wob = (t * 8.0 + e.pos.x * 0.1).sin() * 0.12;
        draw_circle(p.x, p.y, r * (1.0 + wob), c);
        draw_circle_lines(p.x, p.y, r * (1.0 + wob), 1.0, BLACK);
        let eye = if e.attacking { RED } else { YELLOW };
        draw_circle(p.x - r * 0.35, p.y - r * 0.2, r * 0.22, eye);
        draw_circle(p.x + r * 0.35, p.y - r * 0.2, r * 0.22, eye);
        if e.hp < e.max_hp && e.kind != EnemyKind::Gremlin {
            let w = r * 2.0;
            draw_rectangle(p.x - w / 2.0, p.y - r - 6.0, w, 3.0, BLACK);
            draw_rectangle(p.x - w / 2.0, p.y - r - 6.0, w * e.hp / e.max_hp, 3.0, RED);
        }
    }

    for pr in &g.projectiles {
        let p = app.w2s(pr.pos);
        let tail = app.w2s(pr.pos - pr.vel * 0.03);
        draw_line(
            p.x,
            p.y,
            tail.x,
            tail.y,
            2.5,
            Color::from_rgba(255, 240, 160, 255),
        );
    }

    for p in &app.particles {
        let sp = app.w2s(p.pos);
        let a = p.life / p.max;
        let mut c = p.color;
        c.a = a;
        draw_circle(sp.x, sp.y, p.size * (z * 1.3).max(0.75) * (0.5 + a), c);
    }

    for c in &g.coins {
        let p = app.w2s(c.pos);
        draw_circle(p.x, p.y, 4.0 * (z * 1.3).max(0.75), GOLD);
        draw_circle_lines(p.x, p.y, 4.0 * (z * 1.3).max(0.75), 1.0, ORANGE);
    }

    // Night tint over the world, then glowing lamps above it.
    let night_a = match g.phase {
        Phase::Night => {
            let into = g.phase_len - g.phase_t;
            (into / 4.0).min(1.0).min(g.phase_t / 4.0 + 0.3) * 0.45
        }
        Phase::Day => 0.0,
    };
    if night_a > 0.0 {
        draw_rectangle(v.x, v.y, v.w, v.h, Color::new(0.03, 0.04, 0.18, night_a));
        for (_, town) in g.towns() {
            let p = app.w2s(town.pos);
            draw_circle(p.x, p.y, 26.0 * z, Color::new(1.0, 0.8, 0.4, night_a * 0.5));
        }
    }

    // Placement preview.
    let mouse: Vec2 = mouse_position().into();
    if app.turret_mode && v.contains(mouse) {
        let wm = app.s2w(mouse);
        let ok = g.check_turret(wm).is_ok();
        let c = if ok { GREEN } else { RED };
        draw_circle_lines(mouse.x, mouse.y, g.turret_range() * z, 1.5, c);
        draw_rectangle(
            mouse.x - 6.0,
            mouse.y - 5.0,
            12.0,
            10.0,
            Color::new(c.r, c.g, c.b, 0.6),
        );
    }
    if let Some(a) = app.selected {
        let p = app.w2s(g.industries[a].pos);
        draw_line(p.x, p.y, mouse.x, mouse.y, 2.0, YELLOW);
        if let Some(b) = g.industry_at(app.s2w(mouse), 30.0)
            && b != a
        {
            let cost = g.route_cost(a, b);
            let (txt, c) = match g.check_route(a, b) {
                Ok(()) => (format!("Track ${cost:.0}"), GREEN),
                Err(e) => (e.to_string(), RED),
            };
            draw_text(&txt, mouse.x + 12.0, mouse.y + 20.0, 18.0, c);
        }
    }

    for f in &app.floats {
        let p = app.w2s(f.pos);
        let mut c = f.color;
        c.a = (f.life / 1.2).min(1.0);
        draw_text(&f.text, p.x - 10.0, p.y, 16.0, c);
    }
}

fn draw_industry(app: &App, i: usize, ind: &rules::Industry, t: f32) {
    let g = &app.game;
    let z = (app.zoom * 1.3).max(0.75);
    let p = app.w2s(ind.pos);
    let c = kind_color(ind.kind);
    let dark = Color::new(c.r * 0.55, c.g * 0.55, c.b * 0.55, 1.0);
    match ind.kind {
        Kind::Mine => {
            draw_triangle(
                p + vec2(-20.0, 12.0) * z,
                p + vec2(20.0, 12.0) * z,
                p + vec2(0.0, -10.0) * z,
                c,
            );
            for k in 0..4 {
                let o = vec2(-10.0 + k as f32 * 6.0, 10.0 - (k % 2) as f32 * 3.0) * z;
                draw_circle(
                    p.x + o.x,
                    p.y + o.y,
                    4.0 * z,
                    Color::from_rgba(30, 30, 34, 255),
                );
            }
            draw_line(p.x - 6.0 * z, p.y - 4.0 * z, p.x, p.y - 20.0 * z, 2.0, dark);
            draw_line(p.x + 6.0 * z, p.y - 4.0 * z, p.x, p.y - 20.0 * z, 2.0, dark);
            let wa = t * 3.0;
            let wh = p + vec2(0.0, -20.0) * z;
            draw_circle_lines(wh.x, wh.y, 4.0 * z, 1.5, dark);
            draw_line(
                wh.x,
                wh.y,
                wh.x + wa.cos() * 4.0 * z,
                wh.y + wa.sin() * 4.0 * z,
                1.0,
                dark,
            );
        }
        Kind::Plant => {
            draw_rectangle(p.x - 18.0 * z, p.y - 8.0 * z, 36.0 * z, 20.0 * z, c);
            for k in [-10.0, 6.0] {
                draw_rectangle(p.x + k * z, p.y - 24.0 * z, 6.0 * z, 16.0 * z, dark);
                if ind.output > 0.5 && !ind.wrecked() {
                    let ph = (t * 0.8 + k * 0.1).fract();
                    draw_circle(
                        p.x + (k + 3.0 + ph * 6.0) * z,
                        p.y - (26.0 + ph * 14.0) * z,
                        (3.0 + ph * 4.0) * z,
                        Color::new(0.8, 0.8, 0.8, 0.6 * (1.0 - ph)),
                    );
                }
            }
            draw_line(
                p.x - 2.0 * z,
                p.y - 4.0 * z,
                p.x + 3.0 * z,
                p.y + 2.0 * z,
                2.0,
                YELLOW,
            );
            draw_line(
                p.x + 3.0 * z,
                p.y + 2.0 * z,
                p.x - 1.0 * z,
                p.y + 8.0 * z,
                2.0,
                YELLOW,
            );
        }
        Kind::Factory => {
            draw_rectangle(p.x - 20.0 * z, p.y - 6.0 * z, 40.0 * z, 18.0 * z, c);
            for k in 0..4 {
                let x = p.x - 20.0 * z + k as f32 * 10.0 * z;
                draw_triangle(
                    vec2(x, p.y - 6.0 * z),
                    vec2(x + 10.0 * z, p.y - 6.0 * z),
                    vec2(x + 10.0 * z, p.y - 16.0 * z),
                    dark,
                );
            }
            draw_rectangle(p.x - 4.0 * z, p.y + 2.0 * z, 8.0 * z, 10.0 * z, dark);
        }
        Kind::Town => {
            let houses = [
                (-14.0, 4.0, 0),
                (0.0, -6.0, 1),
                (14.0, 4.0, 2),
                (-4.0, 12.0, 3),
                (10.0, -14.0, 4),
            ];
            for (hx, hy, k) in houses {
                let hp = p + vec2(hx, hy) * z;
                let col = [
                    Color::from_rgba(220, 200, 160, 255),
                    Color::from_rgba(200, 170, 140, 255),
                    Color::from_rgba(230, 210, 180, 255),
                    Color::from_rgba(190, 160, 130, 255),
                    Color::from_rgba(210, 190, 150, 255),
                ][k];
                draw_rectangle(hp.x - 5.0 * z, hp.y - 3.0 * z, 10.0 * z, 8.0 * z, col);
                draw_triangle(
                    hp + vec2(-6.0, -3.0) * z,
                    hp + vec2(6.0, -3.0) * z,
                    hp + vec2(0.0, -9.0) * z,
                    Color::from_rgba(160, 60, 50, 255),
                );
                let lit = ind.supplied > 0.0;
                draw_rectangle(
                    hp.x - 1.5 * z,
                    hp.y,
                    3.0 * z,
                    3.0 * z,
                    if lit { YELLOW } else { DARKGRAY },
                );
            }
        }
    }

    if ind.wrecked() {
        draw_circle(p.x, p.y, 22.0 * z, Color::new(0.1, 0.05, 0.05, 0.6));
        draw_line(
            p.x - 12.0 * z,
            p.y - 12.0 * z,
            p.x + 12.0 * z,
            p.y + 12.0 * z,
            3.0,
            RED,
        );
        draw_line(
            p.x + 12.0 * z,
            p.y - 12.0 * z,
            p.x - 12.0 * z,
            p.y + 12.0 * z,
            3.0,
            RED,
        );
    }

    // Selection / hover / valid-target highlights.
    let mouse: Vec2 = mouse_position().into();
    let hovered = g.industry_at(app.s2w(mouse), 30.0) == Some(i);
    if app.selected == Some(i) {
        draw_circle_lines(p.x, p.y, 28.0 * z, 3.0, YELLOW);
    } else if let Some(a) = app.selected {
        if g.industries[a].kind.output() == ind.kind.input() {
            let pulse = 0.5 + 0.5 * (t * 6.0).sin();
            draw_circle_lines(
                p.x,
                p.y,
                28.0 * z,
                2.0,
                Color::new(0.3, 1.0, 0.3, 0.4 + 0.5 * pulse),
            );
        }
    } else if hovered {
        draw_circle_lines(p.x, p.y, 28.0 * z, 2.0, WHITE);
    }

    // Label and bars.
    let fs = (14.0 * z).clamp(11.0, 18.0);
    let label = ind.label();
    let ts = measure_text(&label, None, fs as u16, 1.0);
    draw_text(&label, p.x - ts.width / 2.0, p.y + 28.0 * z, fs, WHITE);
    let bw = 40.0 * z;
    let by = p.y + 32.0 * z;
    if ind.hp < ind.max_hp {
        let f = (ind.hp / ind.max_hp).max(0.0);
        draw_rectangle(p.x - bw / 2.0, by, bw, 4.0, BLACK);
        let hc = if f > 0.5 {
            GREEN
        } else if f > 0.25 {
            ORANGE
        } else {
            RED
        };
        draw_rectangle(p.x - bw / 2.0, by, bw * f, 4.0, hc);
    }
    if ind.kind == Kind::Town {
        let f = (ind.received / rules::TOWN_GOAL).min(1.0);
        let col = if ind.connected() {
            GOLD
        } else {
            Color::from_rgba(240, 140, 60, 255)
        };
        draw_rectangle(p.x - bw / 2.0, by + 6.0, bw, 4.0, BLACK);
        draw_rectangle(p.x - bw / 2.0, by + 6.0, bw * f, 4.0, col);
    }
    if let Some(out) = ind.kind.output() {
        let f = ind.output / rules::STOCK_CAP;
        draw_rectangle(
            p.x + 22.0 * z,
            p.y - 14.0 * z,
            4.0 * z,
            28.0 * z,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
        let h = 28.0 * z * f;
        draw_rectangle(
            p.x + 22.0 * z,
            p.y + 14.0 * z - h,
            4.0 * z,
            h,
            cargo_color(out),
        );
    }
    if ind.kind != Kind::Town
        && let Some(inp) = ind.kind.input()
    {
        let f = ind.input / rules::STOCK_CAP;
        draw_rectangle(
            p.x - 26.0 * z,
            p.y - 14.0 * z,
            4.0 * z,
            28.0 * z,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
        let h = 28.0 * z * f;
        draw_rectangle(
            p.x - 26.0 * z,
            p.y + 14.0 * z - h,
            4.0 * z,
            h,
            cargo_color(inp),
        );
    }
    if hovered && app.selected.is_none() && !app.turret_mode {
        let cost = g.repair_cost(i);
        let mut lines = vec![label.clone()];
        if let Some(inp) = ind.kind.input() {
            lines.push(format!("In ({inp:?}): {:.0}", ind.input));
        }
        if let Some(out) = ind.kind.output() {
            lines.push(format!("Out ({out:?}): {:.0}", ind.output));
        }
        lines.push(format!("HP {:.0}/{:.0}", ind.hp.max(0.0), ind.max_hp));
        if ind.kind == Kind::Town {
            lines.push(format!(
                "Goods received {:.0}/{:.0}",
                ind.received.min(rules::TOWN_GOAL),
                rules::TOWN_GOAL
            ));
            let (r, d) = g.aura(i);
            lines.push(format!(
                "{} - aura {:.0}dps r{:.0}",
                if ind.connected() {
                    "CONNECTED"
                } else if ind.supplied > 0.0 {
                    "supplied"
                } else {
                    "needs goods"
                },
                d,
                r
            ));
        } else if ind.kind.output().is_some() {
            lines.push("Click to start a route".into());
        }
        if cost > 0.0 {
            lines.push(format!("R: repair ${cost:.0}"));
        }
        let x = p.x + 30.0 * z;
        let y = p.y - 30.0 * z;
        draw_rectangle(
            x,
            y,
            190.0,
            lines.len() as f32 * 18.0 + 8.0,
            Color::new(0.0, 0.0, 0.0, 0.75),
        );
        for (k, l) in lines.iter().enumerate() {
            draw_text(l, x + 6.0, y + 18.0 + k as f32 * 18.0, 16.0, WHITE);
        }
    }
}

fn draw_hud(app: &App, audio: &Audio) {
    let g = &app.game;
    let w = screen_width();
    draw_rectangle(0.0, 0.0, w, TOP_H, Color::from_rgba(24, 30, 26, 255));
    let phase = match g.phase {
        Phase::Day => format!("Day {}", g.night + 1),
        Phase::Night => format!("NIGHT {}/{}", g.night, LAST_NIGHT),
    };
    let pc = if g.phase == Phase::Night {
        Color::from_rgba(150, 160, 255, 255)
    } else {
        Color::from_rgba(255, 230, 140, 255)
    };
    draw_text(&phase, 12.0, 28.0, 26.0, pc);
    let bar_x = 150.0;
    draw_rectangle(bar_x, 14.0, 120.0, 14.0, BLACK);
    draw_rectangle(
        bar_x,
        14.0,
        120.0 * (g.phase_t / g.phase_len).clamp(0.0, 1.0),
        14.0,
        pc,
    );
    draw_text(
        format!("{:.0}s", g.phase_t.max(0.0)),
        bar_x + 126.0,
        27.0,
        18.0,
        WHITE,
    );
    draw_text(format!("${:.0}", g.money), 330.0, 30.0, 28.0, GOLD);
    let (done, total) = g.connected_towns();
    draw_text(
        format!("Towns {done}/{total}"),
        450.0,
        28.0,
        22.0,
        if done == total { GREEN } else { WHITE },
    );
    draw_text(format!("Lv {}", g.level), 580.0, 28.0, 22.0, WHITE);
    draw_rectangle(630.0, 16.0, 100.0, 10.0, BLACK);
    draw_rectangle(
        630.0,
        16.0,
        100.0 * g.xp as f32 / g.xp_needed() as f32,
        10.0,
        Color::from_rgba(120, 200, 255, 255),
    );
    draw_text(format!("Kills {}", g.kills), 745.0, 28.0, 20.0, WHITE);
    let speed = if app.paused {
        "PAUSED"
    } else if app.fast {
        ">> x3"
    } else {
        "> x1"
    };
    draw_text(speed, 850.0, 28.0, 20.0, WHITE);
    draw_text(
        if audio.muted {
            "[M] muted"
        } else {
            "[M] sound"
        },
        925.0,
        28.0,
        18.0,
        GRAY,
    );

    // Bottom bar: engines and tools.
    let y = screen_height() - BOTTOM_H;
    draw_rectangle(
        0.0,
        y,
        w - PANEL_W,
        BOTTOM_H,
        Color::from_rgba(24, 30, 26, 255),
    );
    let mut x = 8.0;
    for (k, e) in Engine::ALL.iter().enumerate() {
        let sel = *e == app.engine;
        let txt = format!("{} {} ${:.0}", k + 1, e.name(), e.cost());
        let tw = measure_text(&txt, None, 16, 1.0).width + 12.0;
        if sel {
            draw_rectangle(x - 4.0, y + 4.0, tw, 22.0, engine_color(*e));
        }
        draw_text(&txt, x + 2.0, y + 20.0, 16.0, WHITE);
        x += tw + 4.0;
    }
    let tt = format!(
        "T signal box ${:.0}{}",
        g.turret_cost(),
        if app.turret_mode { " (placing)" } else { "" }
    );
    draw_text(
        &tt,
        x + 10.0,
        y + 20.0,
        16.0,
        if app.turret_mode { GREEN } else { WHITE },
    );
    draw_text(
        "Space speed  P pause  R repair  Wheel zoom  RMB/WASD pan",
        x + 230.0,
        y + 20.0,
        14.0,
        GRAY,
    );

    if app.msg_t > 0.0 {
        let a = app.msg_t.min(1.0);
        let ts = measure_text(&app.msg, None, 22, 1.0);
        let mx = (w - PANEL_W - ts.width) / 2.0;
        draw_rectangle(
            mx - 10.0,
            TOP_H + 10.0,
            ts.width + 20.0,
            32.0,
            Color::new(0.0, 0.0, 0.0, 0.6 * a),
        );
        draw_text(
            &app.msg,
            mx,
            TOP_H + 33.0,
            22.0,
            Color::new(1.0, 1.0, 1.0, a),
        );
    }
}

fn draw_panel(app: &mut App, audio: &mut Audio) {
    let x0 = screen_width() - PANEL_W;
    let h = screen_height();
    app.hover_route = None;
    draw_rectangle(
        x0,
        TOP_H,
        PANEL_W,
        h - TOP_H,
        Color::from_rgba(30, 36, 34, 255),
    );
    draw_line(x0, TOP_H, x0, h, 2.0, Color::from_rgba(80, 90, 80, 255));
    draw_text("TIMETABLE", x0 + 12.0, TOP_H + 26.0, 24.0, WHITE);
    draw_text(
        "Click a producer, then a consumer",
        x0 + 12.0,
        TOP_H + 44.0,
        14.0,
        GRAY,
    );
    let row_h = 64.0;
    let top = TOP_H + 56.0;
    let rows = ((h - top - 10.0) / row_h).floor().max(1.0) as usize;
    let n = app.game.routes.len();
    app.scroll = app.scroll.min(n.saturating_sub(rows));
    if n == 0 {
        for (k, l) in [
            "No routes yet.",
            "",
            "Coal Mine -> Power Plant",
            "Power Plant -> Factory",
            "Factory -> Town",
            "",
            "A town with goods arriving",
            "gets a strong defence aura.",
            "Deliver 100 goods to a town",
            "to connect it. Connect every",
            "town (and keep it fed) to win!",
            "",
            "Tracks build in daylight.",
        ]
        .iter()
        .enumerate()
        {
            draw_text(l, x0 + 14.0, top + 20.0 + k as f32 * 20.0, 17.0, LIGHTGRAY);
        }
        return;
    }
    let mouse: Vec2 = mouse_position().into();
    let mut action: Option<(usize, u8)> = None;
    for (row, ri) in (app.scroll..n).take(rows).enumerate() {
        let y = top + row as f32 * row_h;
        let r = &app.game.routes[ri];
        let box_r = Rect::new(x0 + 6.0, y, PANEL_W - 12.0, row_h - 6.0);
        if box_r.contains(mouse) {
            app.hover_route = Some(ri);
        }
        draw_rectangle(
            box_r.x,
            box_r.y,
            box_r.w,
            box_r.h,
            Color::from_rgba(42, 50, 46, 255),
        );
        draw_rectangle(box_r.x, box_r.y, 5.0, box_r.h, cargo_color(r.cargo));
        let a = app.game.industries[r.a].label();
        let b = app.game.industries[r.b].label();
        draw_text(format!("{a} > {b}"), x0 + 16.0, y + 17.0, 15.0, WHITE);
        let status = if r.is_built() {
            format!("{:?}  earned ${:.0}", r.cargo, r.earned)
        } else {
            format!("building track {:.0}%", r.built / r.len * 100.0)
        };
        draw_text(
            &status,
            x0 + 16.0,
            y + 34.0,
            14.0,
            if r.is_built() { LIGHTGRAY } else { ORANGE },
        );
        for (k, tr) in r.trains.iter().enumerate() {
            let tx = x0 + 16.0 + k as f32 * 18.0;
            draw_rectangle(tx, y + 41.0, 14.0, 10.0, engine_color(tr.engine));
        }
        if r.trains.is_empty() {
            draw_text("no trains!", x0 + 16.0, y + 51.0, 14.0, RED);
        }
        let bx = x0 + PANEL_W - 104.0;
        let can_add = r.trains.len() < rules::MAX_TRAINS && app.game.money >= app.engine.cost();
        if button(Rect::new(bx, y + 38.0, 30.0, 18.0), "+", can_add) {
            action = Some((ri, 0));
        }
        if button(
            Rect::new(bx + 32.0, y + 38.0, 30.0, 18.0),
            "^",
            !r.trains.is_empty(),
        ) {
            action = Some((ri, 1));
        }
        if button(Rect::new(bx + 64.0, y + 38.0, 30.0, 18.0), "x", true) {
            action = Some((ri, 2));
        }
    }
    if n > rows {
        draw_text(
            format!(
                "{}-{} of {} (wheel)",
                app.scroll + 1,
                (app.scroll + rows).min(n),
                n
            ),
            x0 + 12.0,
            h - 6.0,
            14.0,
            GRAY,
        );
    }
    match action {
        Some((ri, 0)) => {
            let e = app.engine;
            let res = app.game.add_train(ri, e);
            if act_err(app, audio, res) {
                app.say(format!("{} added", e.name()));
            }
        }
        Some((ri, 1)) => {
            let e = app.engine;
            let res = app.game.upgrade_train(ri, e);
            if act_err(app, audio, res) {
                app.say(format!("Upgraded to {}", e.name()));
            }
        }
        Some((ri, _)) => {
            app.game.remove_route(ri);
            audio.play(Sfx::Wreck, 0.3, 0.0);
            app.say("Route removed (engines sold at half price)");
        }
        None => {}
    }
}

fn draw_perk_modal(app: &mut App, audio: &mut Audio) {
    let Some(choices) = app.game.perk_choices else {
        return;
    };
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    let title = format!("LEVEL {} - choose an upgrade", app.game.level);
    let ts = measure_text(&title, None, 34, 1.0);
    draw_text(&title, (w - ts.width) / 2.0, h / 2.0 - 110.0, 34.0, GOLD);
    let cw = 230.0;
    let total = cw * 3.0 + 40.0;
    let mut pick = None;
    for (k, p) in choices.iter().enumerate() {
        let r = Rect::new(
            (w - total) / 2.0 + k as f32 * (cw + 20.0),
            h / 2.0 - 80.0,
            cw,
            150.0,
        );
        let hot = r.contains(mouse_position().into());
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            if hot {
                Color::from_rgba(70, 90, 70, 255)
            } else {
                Color::from_rgba(40, 50, 44, 255)
            },
        );
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
        draw_text(format!("[{}]", k + 1), r.x + 10.0, r.y + 26.0, 22.0, GRAY);
        draw_text(p.title(), r.x + 10.0, r.y + 62.0, 24.0, WHITE);
        draw_text(p.desc(), r.x + 10.0, r.y + 100.0, 17.0, LIGHTGRAY);
        if hot && is_mouse_button_pressed(MouseButton::Left) {
            pick = Some(k);
        }
    }
    if let Some(k) = pick {
        pick_perk(app, audio, k);
    }
}

fn draw_title(t: f32) {
    let (w, h) = (screen_width(), screen_height());
    clear_background(Color::from_rgba(20, 30, 24, 255));
    // A little train looping across the screen.
    let y = h * 0.62;
    draw_line(0.0, y + 8.0, w, y + 8.0, 3.0, GRAY);
    let mut s = 0.0;
    while s < w {
        draw_line(
            s,
            y + 4.0,
            s,
            y + 12.0,
            3.0,
            Color::from_rgba(90, 60, 40, 255),
        );
        s += 16.0;
    }
    let x = (t * 120.0) % (w + 300.0) - 150.0;
    for (k, c) in [
        Color::from_rgba(240, 140, 60, 255),
        Color::from_rgba(255, 220, 70, 255),
        Color::from_rgba(40, 40, 46, 255),
    ]
    .iter()
    .enumerate()
    {
        draw_rectangle(x - 40.0 - k as f32 * 36.0, y - 14.0, 32.0, 18.0, *c);
    }
    draw_rectangle(x, y - 20.0, 44.0, 24.0, Color::from_rgba(190, 60, 50, 255));
    draw_rectangle(x + 30.0, y - 32.0, 8.0, 14.0, DARKGRAY);
    for k in 0..4 {
        let ph = (t * 1.5 + k as f32 * 0.25).fract();
        draw_circle(
            x + 34.0 - ph * 60.0,
            y - 36.0 - ph * 40.0,
            4.0 + ph * 10.0,
            Color::new(0.85, 0.85, 0.85, 0.6 * (1.0 - ph)),
        );
    }
    let title = "CARGO CASCADE";
    let ts = measure_text(title, None, 72, 1.0);
    draw_text(title, (w - ts.width) / 2.0, h * 0.24, 72.0, GOLD);
    let lines = [
        "Coal -> Power Plant -> Factory -> Town.",
        "Every town fed with goods glows with a defensive aura.",
        "Monsters come at night. Build signal boxes, repair, reinvest.",
        "Connect every town before night 20 to win.",
    ];
    for (k, l) in lines.iter().enumerate() {
        let ts = measure_text(l, None, 22, 1.0);
        draw_text(
            l,
            (w - ts.width) / 2.0,
            h * 0.33 + k as f32 * 28.0,
            22.0,
            WHITE,
        );
    }
    let blink = (t * 2.0).sin() > -0.3;
    if blink {
        let l = "Click or press any key to start";
        let ts = measure_text(l, None, 28, 1.0);
        draw_text(
            l,
            (w - ts.width) / 2.0,
            h * 0.8,
            28.0,
            Color::from_rgba(140, 255, 140, 255),
        );
    }
    let l = "Mouse: routes & timetable | 1-3 engines | T signal box | Space speed | M mute";
    let ts = measure_text(l, None, 18, 1.0);
    draw_text(l, (w - ts.width) / 2.0, h * 0.88, 18.0, GRAY);
}

fn draw_over(app: &App) {
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
    let g = &app.game;
    let (title, sub, c) = match g.outcome {
        Some(Outcome::Victory) => (
            "ALL TOWNS CONNECTED!",
            "The cascade flows and the night retreats.".to_string(),
            GOLD,
        ),
        Some(Outcome::TownLost(i)) => (
            "GAME OVER",
            format!("{} was overrun.", g.industries[i].label()),
            RED,
        ),
        _ => (
            "GAME OVER",
            format!("Night {LAST_NIGHT} ended with towns still dark."),
            RED,
        ),
    };
    let ts = measure_text(title, None, 60, 1.0);
    draw_text(title, (w - ts.width) / 2.0, h * 0.35, 60.0, c);
    let ts = measure_text(&sub, None, 26, 1.0);
    draw_text(&sub, (w - ts.width) / 2.0, h * 0.35 + 40.0, 26.0, WHITE);
    let (done, total) = g.connected_towns();
    let stats = format!(
        "Nights {}   Towns {}/{}   Kills {}   Earned ${:.0}   Level {}",
        g.night, done, total, g.kills, g.earned, g.level
    );
    let ts = measure_text(&stats, None, 22, 1.0);
    draw_text(
        &stats,
        (w - ts.width) / 2.0,
        h * 0.35 + 90.0,
        22.0,
        LIGHTGRAY,
    );
    let l = "Press Enter or click to play again";
    let ts = measure_text(l, None, 24, 1.0);
    draw_text(
        l,
        (w - ts.width) / 2.0,
        h * 0.35 + 140.0,
        24.0,
        Color::from_rgba(140, 255, 140, 255),
    );
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Cargo Cascade".into(),
        window_width: 1280,
        window_height: 760,
        high_dpi: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let seed = (macroquad::miniquad::date::now() * 1000.0) as u64;
    rand::srand(seed);
    let mut audio = Audio::load().await;
    let mut app = App::new(seed);
    let mut over_t = 0.0;
    loop {
        let t = get_time() as f32;
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        if app.zoom <= 0.0 {
            app.zoom = app.fit_zoom();
        }
        match app.screen {
            Screen::Title => {
                draw_title(t);
                if get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) {
                    audio.unlock();
                    audio.play(Sfx::Whistle, 0.6, 0.0);
                    app.screen = Screen::Play;
                    app.zoom = app.fit_zoom();
                    app.say("Click a Coal Mine, then a Power Plant to lay your first route");
                }
            }
            Screen::Play | Screen::Over => {
                clear_background(BLACK);
                if app.screen == Screen::Play {
                    update_play(&mut app, &mut audio);
                    over_t = 0.0;
                } else {
                    over_t += get_frame_time();
                }
                draw_world(&app);
                draw_hud(&app, &audio);
                draw_panel(&mut app, &mut audio);
                if app.screen == Screen::Play {
                    draw_perk_modal(&mut app, &mut audio);
                    if app.paused {
                        let l = "PAUSED - press P";
                        let ts = measure_text(l, None, 40, 1.0);
                        let v = app.view();
                        draw_text(l, v.center().x - ts.width / 2.0, v.center().y, 40.0, WHITE);
                    }
                } else {
                    draw_over(&app);
                    if over_t > 1.0
                        && (is_key_pressed(KeyCode::Enter)
                            || is_mouse_button_pressed(MouseButton::Left))
                    {
                        app.restart();
                        app.screen = Screen::Play;
                        app.zoom = app.fit_zoom();
                        audio.set_music(MUSIC_VOL);
                        audio.play(Sfx::Whistle, 0.6, 0.0);
                    }
                }
            }
        }
        next_frame().await;
    }
}
