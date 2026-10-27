use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::prelude::*;
use rules::{
    BuildingKind, EnemyKind, Event, Game, Input, LAB_POS, LAB_RADIUS, Order, Phase, ShotKind,
    UnitKind, V2, WIN_TIME, WORLD, synth,
};

const BG: Color = Color::new(0.08, 0.09, 0.11, 1.0);
const ARMY: Color = Color::new(0.35, 0.65, 1.0, 1.0);
const FOE: Color = Color::new(0.95, 0.3, 0.25, 1.0);
const GEM: Color = Color::new(0.4, 1.0, 0.5, 1.0);
const METAL: Color = Color::new(0.75, 0.8, 0.9, 1.0);

fn window_conf() -> Conf {
    Conf {
        window_title: "Factory Escort".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum Sfx {
    Laser,
    Cannon,
    Rocket,
    Hit,
    Explosion,
    BigExplosion,
    Pickup,
    LevelUp,
    Build,
    Spawn,
    Alarm,
    Deny,
    GameOver,
    Victory,
}

struct Audio {
    sfx: Vec<(Sound, f32, f32)>, // sound, volume, min interval
    last: Vec<f64>,
    music: Sound,
    music_on: bool,
    muted: bool,
}

impl Audio {
    async fn load() -> Self {
        let defs: Vec<(Vec<u8>, f32, f32)> = vec![
            (synth::laser(), 0.25, 0.06),
            (synth::cannon(), 0.45, 0.1),
            (synth::rocket(), 0.35, 0.1),
            (synth::hit(), 0.2, 0.05),
            (synth::explosion(), 0.5, 0.08),
            (synth::big_explosion(), 0.8, 0.3),
            (synth::pickup(), 0.3, 0.07),
            (synth::level_up(), 0.6, 0.2),
            (synth::build(), 0.6, 0.1),
            (synth::spawn(), 0.35, 0.15),
            (synth::alarm(), 0.6, 1.0),
            (synth::deny(), 0.5, 0.15),
            (synth::game_over(), 0.7, 1.0),
            (synth::victory(), 0.7, 1.0),
        ];
        let mut sfx = Vec::new();
        for (bytes, vol, gap) in defs {
            sfx.push((load_sound_from_bytes(&bytes).await.unwrap(), vol, gap));
        }
        let music = load_sound_from_bytes(&synth::music()).await.unwrap();
        let n = sfx.len();
        Audio {
            sfx,
            last: vec![-10.0; n],
            music,
            music_on: false,
            muted: false,
        }
    }
    fn play(&mut self, s: Sfx) {
        let i = s as usize;
        let now = get_time();
        if self.muted || now - self.last[i] < self.sfx[i].2 as f64 {
            return;
        }
        self.last[i] = now;
        play_sound(
            &self.sfx[i].0,
            PlaySoundParams {
                looped: false,
                volume: self.sfx[i].1,
            },
        );
    }
    fn start_music(&mut self) {
        if !self.music_on {
            self.music_on = true;
            play_sound(
                &self.music,
                PlaySoundParams {
                    looped: true,
                    volume: if self.muted { 0.0 } else { 0.35 },
                },
            );
        }
    }
    fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        set_sound_volume(&self.music, if self.muted { 0.0 } else { 0.35 });
    }
}

enum Fx {
    Tracer { a: V2, b: V2, color: Color, w: f32 },
    Boom { p: V2, size: f32 },
    Ring { p: V2 },
}

struct Effect {
    fx: Fx,
    t: f32,
    life: f32,
}

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
}

struct Cam {
    off: Vec2,
}
impl Cam {
    fn to_screen(&self, p: V2) -> Vec2 {
        vec2(p.x, p.y) - self.off
    }
    fn to_world(&self, s: Vec2) -> V2 {
        let w = s + self.off;
        V2 { x: w.x, y: w.y }
    }
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, color: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, color);
}

fn text_c(t: &str, cx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(t, cx - d.width / 2.0, y, size, color);
}

fn draw_bot(p: Vec2, r: f32, facing: f32, color: Color, kind: usize) {
    let dir = vec2(facing.cos(), facing.sin());
    let side = vec2(-dir.y, dir.x);
    match kind {
        // Pawn: small triangle
        0 => draw_triangle(
            p + dir * r,
            p - dir * r + side * r,
            p - dir * r - side * r,
            color,
        ),
        // Grunt: square body
        1 => draw_poly(p.x, p.y, 4, r, facing.to_degrees() + 45.0, color),
        // Rocko: body with rocket pods
        2 => {
            draw_circle(p.x, p.y, r * 0.8, color);
            draw_rectangle(
                p.x + side.x * r - 3.0,
                p.y + side.y * r - 3.0,
                6.0,
                6.0,
                WHITE,
            );
            draw_rectangle(
                p.x - side.x * r - 3.0,
                p.y - side.y * r - 3.0,
                6.0,
                6.0,
                WHITE,
            );
        }
        // Hammer: hex with long barrel
        3 => {
            draw_poly(p.x, p.y, 6, r, facing.to_degrees(), color);
            let e = p + dir * r * 1.6;
            draw_line(p.x, p.y, e.x, e.y, 4.0, LIGHTGRAY);
        }
        _ => {
            draw_poly(p.x, p.y, 8, r, facing.to_degrees(), color);
            for s in [-0.4, 0.4] {
                let b = p + side * (r * s);
                let e = b + dir * r * 1.3;
                draw_line(b.x, b.y, e.x, e.y, 2.5, LIGHTGRAY);
            }
        }
    }
    let e = p + dir * r * 0.6;
    draw_circle(e.x, e.y, 2.0, WHITE);
}

fn draw_world(g: &Game, cam: &Cam, fx: &[Effect], rally_marker: Option<V2>) {
    clear_background(BG);
    // Grid.
    let step = 80.0;
    let (sw, sh) = (screen_width(), screen_height());
    let x0 = (cam.off.x / step).floor() * step;
    let y0 = (cam.off.y / step).floor() * step;
    let grid = Color::new(1.0, 1.0, 1.0, 0.04);
    let mut x = x0;
    while x < cam.off.x + sw + step {
        let s = x - cam.off.x;
        draw_line(s, 0.0, s, sh, 1.0, grid);
        x += step;
    }
    let mut y = y0;
    while y < cam.off.y + sh + step {
        let s = y - cam.off.y;
        draw_line(0.0, s, sw, s, 1.0, grid);
        y += step;
    }
    // World border and build radius.
    let o = cam.to_screen(V2 { x: 0.0, y: 0.0 });
    draw_rectangle_lines(o.x, o.y, WORLD, WORLD, 4.0, Color::new(0.9, 0.5, 0.2, 0.5));
    let lp = cam.to_screen(LAB_POS);
    draw_circle_lines(
        lp.x,
        lp.y,
        rules::BUILD_RADIUS,
        1.5,
        Color::new(0.4, 0.8, 1.0, 0.15),
    );

    // Buildings.
    for b in &g.buildings {
        let p = cam.to_screen(b.pos);
        match b.kind {
            BuildingKind::Solar => {
                draw_rectangle(
                    p.x - 18.0,
                    p.y - 18.0,
                    36.0,
                    36.0,
                    Color::new(0.2, 0.25, 0.45, 1.0),
                );
                for i in 0..3 {
                    draw_rectangle(
                        p.x - 15.0,
                        p.y - 14.0 + i as f32 * 10.0,
                        30.0,
                        7.0,
                        Color::new(0.3, 0.6, 1.0, 1.0),
                    );
                }
            }
            BuildingKind::Mex => {
                draw_circle(p.x, p.y, 18.0, Color::new(0.35, 0.33, 0.3, 1.0));
                let a = get_time() as f32 * 2.0;
                for k in 0..3 {
                    let d = vec2((a + k as f32 * 2.09).cos(), (a + k as f32 * 2.09).sin()) * 14.0;
                    draw_line(p.x, p.y, p.x + d.x, p.y + d.y, 4.0, METAL);
                }
            }
        }
        if b.hp < b.max_hp {
            bar(p.x - 18.0, p.y + 22.0, 36.0, 4.0, b.hp / b.max_hp, GREEN);
        }
    }

    // Bot Lab.
    let flash = g.lab_under_attack() && (get_time() * 6.0) as i32 % 2 == 0;
    let body = if flash {
        Color::new(0.7, 0.25, 0.2, 1.0)
    } else {
        Color::new(0.25, 0.32, 0.4, 1.0)
    };
    draw_rectangle(
        lp.x - LAB_RADIUS,
        lp.y - LAB_RADIUS,
        LAB_RADIUS * 2.0,
        LAB_RADIUS * 2.0,
        body,
    );
    draw_rectangle_lines(
        lp.x - LAB_RADIUS,
        lp.y - LAB_RADIUS,
        LAB_RADIUS * 2.0,
        LAB_RADIUS * 2.0,
        3.0,
        ARMY,
    );
    draw_rectangle(
        lp.x - 20.0,
        lp.y + LAB_RADIUS - 14.0,
        40.0,
        14.0,
        Color::new(0.1, 0.1, 0.12, 1.0),
    );
    if let Some(k) = g.lab.queue.front() {
        let frac = g.lab.progress / k.stats().build_time;
        draw_circle_lines(lp.x, lp.y - 6.0, 20.0, 3.0, Color::new(1.0, 1.0, 1.0, 0.15));
        let segs = (frac * 24.0) as i32;
        for s in 0..segs {
            let a = s as f32 / 24.0 * std::f32::consts::TAU - 1.57;
            draw_circle(
                lp.x + a.cos() * 20.0,
                lp.y - 6.0 + a.sin() * 20.0,
                2.5,
                ARMY,
            );
        }
        draw_bot(vec2(lp.x, lp.y - 6.0), 9.0, -1.57, ARMY, k.index());
    }
    text_c("BOT LAB", lp.x, lp.y - LAB_RADIUS - 14.0, 18.0, WHITE);
    bar(
        lp.x - LAB_RADIUS,
        lp.y - LAB_RADIUS - 8.0,
        LAB_RADIUS * 2.0,
        5.0,
        g.lab.hp / g.lab.max_hp,
        GREEN,
    );

    // Gems.
    for gm in &g.gems {
        let p = cam.to_screen(gm.pos);
        let r = 3.0 + (gm.xp as f32).sqrt();
        draw_poly(p.x, p.y, 4, r, 45.0, GEM);
    }

    if let Some(r) = rally_marker {
        let p = cam.to_screen(r);
        let pulse = 14.0 + (get_time() as f32 * 5.0).sin() * 3.0;
        draw_circle_lines(p.x, p.y, pulse, 2.0, YELLOW);
        draw_line(p.x, p.y, p.x, p.y - 26.0, 2.0, YELLOW);
        draw_triangle(
            vec2(p.x, p.y - 26.0),
            vec2(p.x + 12.0, p.y - 21.0),
            vec2(p.x, p.y - 16.0),
            YELLOW,
        );
    }

    // Army.
    for u in &g.units {
        let p = cam.to_screen(u.pos);
        if p.x < -40.0 || p.y < -40.0 || p.x > sw + 40.0 || p.y > sh + 40.0 {
            continue;
        }
        draw_bot(p, u.kind.stats().radius, u.facing, ARMY, u.kind.index());
        if u.hp < u.max_hp {
            bar(p.x - 10.0, p.y - 18.0, 20.0, 3.0, u.hp / u.max_hp, GREEN);
        }
    }

    // Enemies.
    for e in &g.enemies {
        let p = cam.to_screen(e.pos);
        if p.x < -60.0 || p.y < -60.0 || p.x > sw + 60.0 || p.y > sh + 60.0 {
            continue;
        }
        let st = e.kind.stats();
        let c = if e.flash > 0.0 { WHITE } else { FOE };
        let k = match e.kind {
            EnemyKind::Tick => 0,
            EnemyKind::Raider => 2,
            EnemyKind::Thud => 1,
            EnemyKind::Tank => 3,
            EnemyKind::Boss => 4,
        };
        draw_bot(p, st.radius, e.facing, c, k);
        if e.kind == EnemyKind::Boss || e.kind == EnemyKind::Tank {
            bar(
                p.x - st.radius,
                p.y - st.radius - 8.0,
                st.radius * 2.0,
                4.0,
                e.hp / e.max_hp,
                RED,
            );
        }
    }

    // Commander.
    let cp = cam.to_screen(g.cmd.pos);
    draw_circle(cp.x, cp.y, 17.0, Color::new(0.15, 0.3, 0.6, 1.0));
    draw_circle_lines(cp.x, cp.y, 17.0, 3.0, GOLD);
    draw_bot(cp, 12.0, g.cmd.facing, GOLD, 4);
    draw_circle_lines(cp.x, cp.y, g.magnet, 1.0, Color::new(0.4, 1.0, 0.5, 0.08));

    // Effects.
    for e in fx {
        let k = 1.0 - e.t / e.life;
        match e.fx {
            Fx::Tracer { a, b, color, w } => {
                let (a, b) = (cam.to_screen(a), cam.to_screen(b));
                draw_line(a.x, a.y, b.x, b.y, w, Color { a: k, ..color });
            }
            Fx::Boom { p, size } => {
                let p = cam.to_screen(p);
                let r = size * (0.4 + 0.8 * (1.0 - k));
                draw_circle(p.x, p.y, r, Color::new(1.0, 0.6, 0.2, 0.6 * k));
                draw_circle(p.x, p.y, r * 0.5, Color::new(1.0, 0.95, 0.6, 0.8 * k));
            }
            Fx::Ring { p } => {
                let p = cam.to_screen(p);
                draw_circle_lines(
                    p.x,
                    p.y,
                    10.0 + 30.0 * (1.0 - k),
                    2.0,
                    Color { a: k, ..ARMY },
                );
            }
        }
    }

    // Off-screen lab arrow.
    let center = vec2(sw / 2.0, sh / 2.0);
    if lp.x < 0.0 || lp.y < 0.0 || lp.x > sw || lp.y > sh {
        let d = (lp - center).normalize();
        let tip = center + d * (sh.min(sw) / 2.0 - 30.0);
        let side = vec2(-d.y, d.x);
        let col = if g.lab_under_attack() { RED } else { ARMY };
        draw_triangle(tip + d * 16.0, tip + side * 10.0, tip - side * 10.0, col);
        text_c(
            "LAB",
            tip.x - d.x * 18.0,
            tip.y - d.y * 18.0 + 5.0,
            16.0,
            col,
        );
    }
}

fn draw_hud(g: &Game, muted: bool) {
    let (sw, sh) = (screen_width(), screen_height());
    // Top: time, wave, XP.
    bar(0.0, 0.0, sw, 8.0, g.xp as f32 / g.xp_next as f32, GEM);
    let left = (WIN_TIME - g.t).max(0.0);
    let txt = format!(
        "Wave {}   Hold out {:02}:{:02}   Kills {}   Lv {}",
        g.wave(),
        (left / 60.0) as i32,
        (left % 60.0) as i32,
        g.kills,
        g.level
    );
    text_c(&txt, sw / 2.0, 32.0, 26.0, WHITE);

    // Left panel.
    let x = 14.0;
    draw_text("Commander", x, 60.0, 20.0, GOLD);
    bar(x, 66.0, 200.0, 10.0, g.cmd.hp / g.cmd.max_hp, GREEN);
    draw_text("Bot Lab", x, 96.0, 20.0, ARMY);
    bar(
        x,
        102.0,
        200.0,
        10.0,
        g.lab.hp / g.lab.max_hp,
        if g.lab_under_attack() { ORANGE } else { GREEN },
    );
    draw_text(
        format!("Metal {:.0}  (+{:.1}/s)", g.metal, g.income()),
        x,
        134.0,
        20.0,
        METAL,
    );
    draw_text(
        format!("Energy: build x{:.1}", g.build_speed()),
        x,
        156.0,
        20.0,
        YELLOW,
    );
    let order = match g.order {
        Order::Follow => "FOLLOW".to_string(),
        Order::Rally(_) => "RALLY".to_string(),
    };
    draw_text(
        format!("Army {}/{}  [{}]", g.units.len(), g.cap, order),
        x,
        178.0,
        20.0,
        ARMY,
    );

    // Bottom bar: unit queue keys.
    let bw = 150.0;
    let bx = sw / 2.0 - bw * 3.5 - 15.0;
    let by = sh - 70.0;
    for (i, k) in UnitKind::ALL.iter().enumerate() {
        let st = k.stats();
        let x = bx + i as f32 * (bw + 6.0);
        let unlocked = g.unlocked[k.index()];
        let ok = unlocked && g.metal >= st.cost;
        let bgc = if g.lab.repeat == Some(*k) {
            Color::new(0.15, 0.3, 0.5, 0.9)
        } else {
            Color::new(0.0, 0.0, 0.0, 0.6)
        };
        draw_rectangle(x, by, bw, 60.0, bgc);
        draw_bot(
            vec2(x + 20.0, by + 30.0),
            10.0,
            -1.57,
            if unlocked { ARMY } else { DARKGRAY },
            i,
        );
        let c = if ok { WHITE } else { GRAY };
        draw_text(
            format!("[{}] {}", i + 1, k.name()),
            x + 36.0,
            by + 24.0,
            20.0,
            c,
        );
        let sub = if unlocked {
            format!("{:.0} metal", st.cost)
        } else {
            "locked".into()
        };
        draw_text(&sub, x + 36.0, by + 46.0, 18.0, c);
    }
    let bx2 = bx + 5.0 * (bw + 6.0);
    for (j, (key, b)) in [("Q", BuildingKind::Mex), ("E", BuildingKind::Solar)]
        .iter()
        .enumerate()
    {
        let y = by + j as f32 * 31.0;
        draw_rectangle(bx2, y, bw, 29.0, Color::new(0.0, 0.0, 0.0, 0.6));
        let c = if g.metal >= b.cost() { WHITE } else { GRAY };
        draw_text(
            format!(
                "[{}] {} {:.0}",
                key,
                if j == 0 { "Mex" } else { "Solar" },
                b.cost()
            ),
            bx2 + 8.0,
            y + 20.0,
            18.0,
            c,
        );
    }
    let q: Vec<&str> = g.lab.queue.iter().map(|k| k.name()).collect();
    draw_text(
        format!("Queue: {}", q.join(", ")),
        bx,
        by - 8.0,
        18.0,
        LIGHTGRAY,
    );

    draw_text(
        format!(
            "WASD move  RMB rally  F follow  1-5 queue  Q/E build  M {}",
            if muted { "unmute" } else { "mute" }
        ),
        sw - 560.0,
        sh - 82.0,
        16.0,
        GRAY,
    );

    if g.lab_under_attack() && (get_time() * 3.0) as i32 % 2 == 0 {
        text_c("LAB UNDER ATTACK!  Hurry home!", sw / 2.0, 70.0, 30.0, RED);
    }
    if g.enemies.iter().any(|e| e.kind == EnemyKind::Boss) {
        text_c(
            "BOSS: Krogoth-class walker inbound",
            sw / 2.0,
            100.0,
            22.0,
            ORANGE,
        );
    }

    // Minimap.
    let ms = 150.0;
    let (mx, my) = (sw - ms - 12.0, 50.0);
    draw_rectangle(mx, my, ms, ms, Color::new(0.0, 0.0, 0.0, 0.6));
    let m = |p: V2| vec2(mx + p.x / WORLD * ms, my + p.y / WORLD * ms);
    for e in &g.enemies {
        let p = m(e.pos);
        draw_rectangle(p.x, p.y, 2.0, 2.0, FOE);
    }
    for u in &g.units {
        let p = m(u.pos);
        draw_rectangle(p.x, p.y, 2.0, 2.0, ARMY);
    }
    let lp = m(LAB_POS);
    draw_rectangle(lp.x - 3.0, lp.y - 3.0, 6.0, 6.0, WHITE);
    let cp = m(g.cmd.pos);
    draw_circle(cp.x, cp.y, 3.0, GOLD);
}

fn draw_choices(g: &Game) {
    let Some(ch) = g.choices else { return };
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    text_c(
        &format!("LEVEL {}  - choose an upgrade", g.level),
        sw / 2.0,
        sh / 2.0 - 140.0,
        40.0,
        GEM,
    );
    for (i, u) in ch.iter().enumerate() {
        let y = sh / 2.0 - 90.0 + i as f32 * 70.0;
        let r = Rect::new(sw / 2.0 - 260.0, y, 520.0, 58.0);
        let hover = r.contains(mouse_position().into());
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            if hover {
                Color::new(0.2, 0.35, 0.55, 1.0)
            } else {
                Color::new(0.1, 0.12, 0.16, 1.0)
            },
        );
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, ARMY);
        draw_text(
            format!("[{}]  {}", i + 1, u.title()),
            r.x + 18.0,
            r.y + 37.0,
            26.0,
            WHITE,
        );
    }
}

fn choice_rect(i: usize) -> Rect {
    let (sw, sh) = (screen_width(), screen_height());
    Rect::new(
        sw / 2.0 - 260.0,
        sh / 2.0 - 90.0 + i as f32 * 70.0,
        520.0,
        58.0,
    )
}

fn handle_events(g: &mut Game, audio: &mut Audio, fx: &mut Vec<Effect>) {
    for ev in g.events.drain(..) {
        match ev {
            Event::Shot { from, to, kind } => {
                let (color, w, s) = match kind {
                    ShotKind::Laser => (Color::new(0.5, 0.85, 1.0, 1.0), 2.0, Some(Sfx::Laser)),
                    ShotKind::Cannon => (Color::new(1.0, 0.85, 0.4, 1.0), 4.0, Some(Sfx::Cannon)),
                    ShotKind::Rocket => (Color::new(1.0, 0.6, 0.3, 1.0), 3.0, Some(Sfx::Rocket)),
                    ShotKind::Enemy => (Color::new(1.0, 0.3, 0.3, 1.0), 2.0, None),
                    ShotKind::DGun => (GOLD, 3.0, Some(Sfx::Laser)),
                };
                fx.push(Effect {
                    fx: Fx::Tracer {
                        a: from,
                        b: to,
                        color,
                        w,
                    },
                    t: 0.0,
                    life: 0.12,
                });
                if let Some(s) = s {
                    audio.play(s);
                }
            }
            Event::Hit { .. } => audio.play(Sfx::Hit),
            Event::Explosion { pos, size } => {
                fx.push(Effect {
                    fx: Fx::Boom { p: pos, size },
                    t: 0.0,
                    life: 0.4,
                });
                audio.play(if size > 50.0 {
                    Sfx::BigExplosion
                } else {
                    Sfx::Explosion
                });
            }
            Event::Pickup => audio.play(Sfx::Pickup),
            Event::LevelUp => audio.play(Sfx::LevelUp),
            Event::Built { pos } => {
                fx.push(Effect {
                    fx: Fx::Ring { p: pos },
                    t: 0.0,
                    life: 0.6,
                });
                audio.play(Sfx::Build);
            }
            Event::UnitSpawned { pos } => {
                fx.push(Effect {
                    fx: Fx::Ring { p: pos },
                    t: 0.0,
                    life: 0.5,
                });
                audio.play(Sfx::Spawn);
            }
            Event::Denied => audio.play(Sfx::Deny),
            Event::LabHit | Event::BossSpawned => audio.play(Sfx::Alarm),
            Event::GameOver => audio.play(Sfx::GameOver),
            Event::Victory => audio.play(Sfx::Victory),
        }
    }
    if fx.len() > 600 {
        let n = fx.len() - 600;
        fx.drain(..n);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut audio = Audio::load().await;
    let mut seed = 1u64;
    let mut g = Game::new(seed);
    let mut screen = Screen::Title;
    let mut fx: Vec<Effect> = Vec::new();
    let mut cam = Cam {
        off: vec2(0.0, 0.0),
    };

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        let any_input = get_last_key_pressed().is_some()
            || is_mouse_button_pressed(MouseButton::Left)
            || is_mouse_button_pressed(MouseButton::Right);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        if any_input {
            audio.start_music();
        }
        let (sw, sh) = (screen_width(), screen_height());
        let target = vec2(g.cmd.pos.x, g.cmd.pos.y) - vec2(sw, sh) / 2.0;
        cam.off += (target - cam.off) * (dt * 6.0).min(1.0);

        match screen {
            Screen::Title => {
                cam.off = target;
                draw_world(&g, &cam, &fx, None);
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.7));
                text_c("FACTORY ESCORT", sw / 2.0, sh / 2.0 - 150.0, 72.0, GOLD);
                text_c(
                    "Survivors vs survivors: your army grows as theirs does.",
                    sw / 2.0,
                    sh / 2.0 - 100.0,
                    26.0,
                    WHITE,
                );
                let lines = [
                    "Your Bot Lab pumps out bots that follow your Commander as a swarm.",
                    "Lead them out to farm salvage (XP), and hurry home when the lab is hit.",
                    "Hold out for 10 minutes and beat three bosses. Lose the lab or Commander and it's over.",
                    "",
                    "WASD move   Right-click rally point   F army follows you",
                    "1-5 queue Pawn / Grunt / Rocko / Hammer / Warrior (last pick repeats)",
                    "Q build Metal Extractor, E build Solar (near the lab)   M mute",
                ];
                for (i, l) in lines.iter().enumerate() {
                    text_c(
                        l,
                        sw / 2.0,
                        sh / 2.0 - 40.0 + i as f32 * 28.0,
                        22.0,
                        LIGHTGRAY,
                    );
                }
                if (get_time() * 2.0) as i32 % 2 == 0 {
                    text_c(
                        "Press any key to start",
                        sw / 2.0,
                        sh / 2.0 + 190.0,
                        32.0,
                        GEM,
                    );
                }
                if any_input && !is_key_pressed(KeyCode::M) {
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
                let mut input = Input::default();
                let mut d = V2::default();
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
                input.dir = d;
                let keys = [
                    KeyCode::Key1,
                    KeyCode::Key2,
                    KeyCode::Key3,
                    KeyCode::Key4,
                    KeyCode::Key5,
                ];
                if g.choices.is_some() {
                    for (i, k) in keys.iter().take(3).enumerate() {
                        if is_key_pressed(*k) {
                            input.choose = Some(i);
                        }
                    }
                    if is_mouse_button_pressed(MouseButton::Left) {
                        let mp: Vec2 = mouse_position().into();
                        input.choose = (0..3).find(|i| choice_rect(*i).contains(mp));
                    }
                } else {
                    for (i, k) in keys.iter().enumerate() {
                        if is_key_pressed(*k) {
                            input.queue = Some(UnitKind::ALL[i]);
                        }
                    }
                    if is_mouse_button_pressed(MouseButton::Right) {
                        input.rally = Some(cam.to_world(mouse_position().into()));
                    }
                    input.follow = is_key_pressed(KeyCode::F);
                    if is_key_pressed(KeyCode::Q) {
                        input.build = Some(BuildingKind::Mex);
                    }
                    if is_key_pressed(KeyCode::E) {
                        input.build = Some(BuildingKind::Solar);
                    }
                }
                g.step(dt, &input);
                handle_events(&mut g, &mut audio, &mut fx);
                for e in &mut fx {
                    e.t += dt;
                }
                fx.retain(|e| e.t < e.life);

                let rally = match g.order {
                    Order::Rally(p) => Some(p),
                    Order::Follow => None,
                };
                draw_world(&g, &cam, &fx, rally);
                draw_hud(&g, audio.muted);
                draw_choices(&g);

                if g.phase != Phase::Playing {
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.65));
                    let (title, col, why) = match g.phase {
                        Phase::Won => ("VICTORY", GEM, "The Bot Lab held. The front is yours."),
                        Phase::LostLab => ("GAME OVER", RED, "Your Bot Lab was destroyed."),
                        _ => ("GAME OVER", RED, "Your Commander was destroyed."),
                    };
                    text_c(title, sw / 2.0, sh / 2.0 - 60.0, 80.0, col);
                    text_c(why, sw / 2.0, sh / 2.0, 28.0, WHITE);
                    text_c(
                        &format!(
                            "Survived {:.0}s   Level {}   Kills {}",
                            g.t, g.level, g.kills
                        ),
                        sw / 2.0,
                        sh / 2.0 + 40.0,
                        26.0,
                        LIGHTGRAY,
                    );
                    text_c(
                        "Press R to play again",
                        sw / 2.0,
                        sh / 2.0 + 100.0,
                        30.0,
                        GOLD,
                    );
                    if is_key_pressed(KeyCode::R) {
                        seed = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        g = Game::new(seed ^ (get_time() * 1000.0) as u64);
                        fx.clear();
                    }
                }
            }
        }
        if audio.muted {
            draw_text("MUTED", 14.0, screen_height() - 90.0, 20.0, RED);
        }
        next_frame().await;
    }
}
