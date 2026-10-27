//! Procedural rendering of the battlefield, HUD and menus.

use crate::{Cam, Particle};
use macroquad::prelude::*;
use sim::game::{
    BOSS_TIME, BUILD_KINDS, BUILD_RANGE, BUILDING_R, BeamKind, BuildKind, COMMANDER_R, EnemyKind,
    Game, Upgrade, xp_to_next,
};
use sim::math::V2;

const GROUND: Color = Color::new(0.10, 0.13, 0.11, 1.0);
const GRID: Color = Color::new(0.16, 0.20, 0.17, 1.0);
const PANEL: Color = Color::new(0.04, 0.06, 0.08, 0.82);
const ACCENT: Color = Color::new(0.35, 0.75, 1.0, 1.0);
const METAL: Color = Color::new(0.75, 0.80, 0.85, 1.0);
const ENERGY: Color = Color::new(1.0, 0.85, 0.25, 1.0);

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x85eb_ca6b) ^ (y as u32).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 13;
    h = h.wrapping_mul(0x27d4_eb2d);
    h ^ (h >> 15)
}

fn text_c(s: &str, x: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, x - d.width / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fg: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fg);
}

fn ground(cam: &Cam) {
    clear_background(GROUND);
    let step = 80.0;
    let tl = cam.to_world(vec2(0.0, 0.0));
    let br = cam.to_world(vec2(screen_width(), screen_height()));
    let (x0, x1) = ((tl.x / step).floor() as i32, (br.x / step).ceil() as i32);
    let (y0, y1) = ((tl.y / step).floor() as i32, (br.y / step).ceil() as i32);
    for gx in x0..=x1 {
        let a = cam.to_screen(V2::new(gx as f32 * step, 0.0));
        draw_line(a.x, 0.0, a.x, screen_height(), 1.0, GRID);
    }
    for gy in y0..=y1 {
        let a = cam.to_screen(V2::new(0.0, gy as f32 * step));
        draw_line(0.0, a.y, screen_width(), a.y, 1.0, GRID);
    }
    // Scattered rocks and craters.
    for gx in x0..=x1 {
        for gy in y0..=y1 {
            let h = hash(gx, gy);
            if !h.is_multiple_of(5) {
                continue;
            }
            let p = V2::new(
                (gx as f32 + (h % 97) as f32 / 97.0) * step,
                (gy as f32 + (h / 97 % 89) as f32 / 89.0) * step,
            );
            let s = cam.to_screen(p);
            let r = (4.0 + (h % 11) as f32) * cam.scale;
            if h.is_multiple_of(3) {
                draw_circle_lines(s.x, s.y, r * 1.6, 2.0, Color::new(0.07, 0.09, 0.08, 1.0));
            } else {
                draw_poly(
                    s.x,
                    s.y,
                    5,
                    r,
                    (h % 360) as f32,
                    Color::new(0.2, 0.22, 0.2, 1.0),
                );
            }
        }
    }
}

fn enemy_color(k: EnemyKind) -> Color {
    match k {
        EnemyKind::Tick => Color::new(0.95, 0.35, 0.3, 1.0),
        EnemyKind::Grunt => Color::new(0.85, 0.2, 0.2, 1.0),
        EnemyKind::Gunner => Color::new(1.0, 0.55, 0.15, 1.0),
        EnemyKind::Brute => Color::new(0.6, 0.12, 0.15, 1.0),
        EnemyKind::Elite => Color::new(0.65, 0.25, 0.85, 1.0),
        EnemyKind::Boss => Color::new(0.45, 0.1, 0.55, 1.0),
    }
}

fn building(b: &sim::game::Building, cam: &Cam, time: f32) {
    let s = cam.to_screen(b.pos);
    let r = BUILDING_R * cam.scale;
    let base = if b.flash > 0.0 {
        WHITE
    } else {
        Color::new(0.32, 0.36, 0.40, 1.0)
    };
    draw_rectangle(s.x - r, s.y - r, r * 2.0, r * 2.0, base);
    draw_rectangle_lines(s.x - r, s.y - r, r * 2.0, r * 2.0, 2.0, BLACK);
    match b.kind {
        BuildKind::LaserTower => {
            draw_circle(s.x, s.y, r * 0.6, Color::new(0.5, 0.55, 0.6, 1.0));
            draw_circle(s.x, s.y, r * 0.25, RED);
        }
        BuildKind::HeavyLaser => {
            draw_poly(
                s.x,
                s.y,
                8,
                r * 0.9,
                22.5,
                Color::new(0.45, 0.48, 0.55, 1.0),
            );
            draw_circle(s.x, s.y, r * 0.4, Color::new(1.0, 0.3, 0.6, 1.0));
        }
        BuildKind::Solar => {
            let blue = Color::new(0.15, 0.3, 0.6, 1.0);
            let q = r * 0.85;
            for (dx, dy) in [(-1.0, -1.0), (0.05, -1.0), (-1.0, 0.05), (0.05, 0.05)] {
                draw_rectangle(s.x + dx * q, s.y + dy * q, q * 0.95, q * 0.95, blue);
            }
        }
        BuildKind::Nano => {
            draw_circle(s.x, s.y, r * 0.7, Color::new(0.2, 0.6, 0.3, 1.0));
            let a = time * 2.0;
            let d = vec2(a.cos(), a.sin()) * r * 0.6;
            draw_line(s.x - d.x, s.y - d.y, s.x + d.x, s.y + d.y, 3.0, GREEN);
            draw_line(s.x + d.y, s.y - d.x, s.x - d.y, s.y + d.x, 3.0, GREEN);
        }
        BuildKind::Converter => {
            draw_poly(s.x, s.y, 4, r * 0.8, 45.0 + time * 40.0, ENERGY);
            draw_circle(s.x, s.y, r * 0.3, METAL);
        }
    }
    if b.hp < b.max_hp {
        bar(s.x - r, s.y + r + 3.0, r * 2.0, 4.0, b.hp / b.max_hp, GREEN);
    }
}

fn enemy(e: &sim::game::Enemy, cam: &Cam, cmd: V2, time: f32) {
    let s = cam.to_screen(e.pos);
    let st = e.kind.stats();
    let r = st.radius * cam.scale;
    let c = if e.flash > 0.0 {
        WHITE
    } else {
        enemy_color(e.kind)
    };
    let face = (cmd - e.pos).angle().to_degrees();
    match e.kind {
        EnemyKind::Tick => draw_poly(s.x, s.y, 3, r, face, c),
        EnemyKind::Grunt => {
            draw_circle(s.x, s.y, r, c);
            draw_circle(s.x, s.y, r * 0.45, Color::new(0.3, 0.05, 0.05, 1.0));
        }
        EnemyKind::Gunner => {
            draw_poly(s.x, s.y, 4, r, face + 45.0, c);
            let d = V2::from_angle(face.to_radians()) * (r * 1.3);
            draw_line(s.x, s.y, s.x + d.x, s.y + d.y, 3.0, DARKGRAY);
        }
        EnemyKind::Brute => {
            draw_poly(s.x, s.y, 6, r, face, c);
            draw_poly_lines(s.x, s.y, 6, r, face, 2.0, BLACK);
        }
        EnemyKind::Elite | EnemyKind::Boss => {
            let spikes = if e.kind == EnemyKind::Boss { 10 } else { 6 };
            for k in 0..spikes {
                let a = time * 0.8 + k as f32 / spikes as f32 * std::f32::consts::TAU;
                let d = vec2(a.cos(), a.sin());
                draw_triangle(
                    vec2(s.x, s.y) + d * r * 1.35,
                    vec2(s.x, s.y) + vec2(-d.y, d.x) * r * 0.4,
                    vec2(s.x, s.y) - vec2(-d.y, d.x) * r * 0.4,
                    Color::new(0.25, 0.05, 0.3, 1.0),
                );
            }
            draw_poly(s.x, s.y, 8, r, face, c);
            draw_circle(s.x, s.y, r * 0.4, Color::new(1.0, 0.3, 0.9, 1.0));
            bar(
                s.x - r,
                s.y - r - 12.0,
                r * 2.0,
                6.0,
                e.hp / e.max_hp,
                MAGENTA,
            );
        }
    }
}

fn commander(g: &Game, cam: &Cam) {
    let c = &g.cmd;
    let s = cam.to_screen(c.pos);
    let r = COMMANDER_R * cam.scale;
    let aim = cam.to_world(mouse_position().into());
    let a = (aim - c.pos).angle();
    let body = if c.hurt > 0.0 {
        Color::new(1.0, 0.4, 0.4, 1.0)
    } else {
        Color::new(0.25, 0.5, 0.9, 1.0)
    };
    let side = vec2(-c.facing.sin(), c.facing.cos()) * r * 0.9;
    draw_circle(
        s.x + side.x,
        s.y + side.y,
        r * 0.5,
        Color::new(0.15, 0.3, 0.6, 1.0),
    );
    draw_circle(
        s.x - side.x,
        s.y - side.y,
        r * 0.5,
        Color::new(0.15, 0.3, 0.6, 1.0),
    );
    draw_circle(s.x, s.y, r, body);
    let d = vec2(a.cos(), a.sin()) * r * 1.5;
    draw_line(s.x, s.y, s.x + d.x, s.y + d.y, 5.0 * cam.scale, DARKGRAY);
    draw_circle(s.x, s.y, r * 0.45, Color::new(0.8, 0.95, 1.0, 1.0));
    draw_circle_lines(s.x, s.y, r, 2.0, BLACK);
}

pub fn world(g: &Game, cam: &Cam, parts: &[Particle], selected: Option<usize>, idle: bool) {
    ground(cam);
    let t = get_time() as f32;
    let cs = cam.to_screen(g.cmd.pos);
    if !idle {
        draw_circle_lines(
            cs.x,
            cs.y,
            g.cmd.reclaim_r * cam.scale,
            1.0,
            Color::new(0.3, 0.9, 0.4, 0.18),
        );
    }
    if selected.is_some() {
        draw_circle_lines(
            cs.x,
            cs.y,
            BUILD_RANGE * cam.scale,
            2.0,
            Color::new(0.4, 0.9, 1.0, 0.35),
        );
    }
    for w in &g.wrecks {
        let s = cam.to_screen(w.pos);
        let r = if w.big { 16.0 } else { 8.0 } * cam.scale;
        let h = (w.pos.x as i32).wrapping_mul(31) ^ w.pos.y as i32;
        draw_poly(s.x, s.y, 5, r, h as f32, Color::new(0.22, 0.22, 0.2, 1.0));
        draw_poly_lines(
            s.x,
            s.y,
            5,
            r,
            h as f32,
            1.5,
            Color::new(0.35, 0.8, 0.4, 0.6),
        );
    }
    for b in &g.buildings {
        building(b, cam, t);
    }
    for e in &g.enemies {
        enemy(e, cam, g.cmd.pos, t);
    }
    if !idle {
        commander(g, cam);
    }
    for b in &g.bullets {
        let s = cam.to_screen(b.pos);
        let (r, c) = if b.big {
            (6.0, Color::new(1.0, 0.3, 0.9, 1.0))
        } else {
            (4.0, Color::new(1.0, 0.6, 0.2, 1.0))
        };
        draw_circle(s.x, s.y, r * cam.scale, c);
    }
    for bm in &g.beams {
        let a = cam.to_screen(bm.a);
        let b = cam.to_screen(bm.b);
        let (w, c) = match bm.kind {
            BeamKind::Commander => (3.0, Color::new(0.4, 0.8, 1.0, 1.0)),
            BeamKind::Tower => (2.0, Color::new(1.0, 0.3, 0.3, 1.0)),
            BeamKind::Heavy => (5.0, Color::new(1.0, 0.3, 0.7, 1.0)),
            BeamKind::Reclaim => (2.0, Color::new(0.3, 1.0, 0.4, 0.7)),
            BeamKind::Nano => (2.0, Color::new(0.5, 1.0, 0.6, 0.5)),
            BeamKind::DGun => {
                let k = (bm.ttl / 0.45).clamp(0.0, 1.0);
                draw_line(
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    40.0 * k * cam.scale,
                    Color::new(1.0, 0.5, 0.1, 0.5 * k),
                );
                (14.0 * k, Color::new(1.0, 1.0, 0.8, k))
            }
        };
        draw_line(a.x, a.y, b.x, b.y, w * cam.scale, c);
    }
    for p in parts {
        let s = cam.to_screen(p.pos);
        let k = p.life / p.max;
        let mut c = p.color;
        c.a *= k;
        if p.ring {
            draw_circle_lines(s.x, s.y, p.size * (1.0 - k) * cam.scale, 3.0, c);
        } else {
            draw_circle(s.x, s.y, p.size * k * cam.scale, c);
        }
    }
    // Placement ghost.
    if let Some(i) = selected {
        let kind = BUILD_KINDS[i];
        let m = cam.to_world(mouse_position().into());
        let ok = g.placement_ok(m) && g.can_afford(kind);
        let s = cam.to_screen(m);
        let r = BUILDING_R * cam.scale;
        let c = if ok {
            Color::new(0.3, 1.0, 0.5, 0.45)
        } else {
            Color::new(1.0, 0.2, 0.2, 0.45)
        };
        draw_rectangle(s.x - r, s.y - r, r * 2.0, r * 2.0, c);
        if let Some((range, _, _)) = kind.weapon() {
            draw_circle_lines(s.x, s.y, range * g.tower_range * cam.scale, 1.0, c);
        }
    }
}

fn fmt_time(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{:02}:{:02}", t / 60, t % 60)
}

pub fn hud(g: &Game, selected: Option<usize>, muted: bool) {
    let sw = screen_width();
    let sh = screen_height();
    // XP bar.
    bar(
        0.0,
        0.0,
        sw,
        6.0,
        g.xp as f32 / xp_to_next(g.level) as f32,
        ACCENT,
    );
    // Left panel.
    draw_rectangle(10.0, 14.0, 270.0, 96.0, PANEL);
    draw_text("COMMANDER", 20.0, 32.0, 18.0, ACCENT);
    draw_text(format!("Lv {}", g.level), 220.0, 32.0, 20.0, WHITE);
    bar(
        20.0,
        40.0,
        250.0,
        12.0,
        g.cmd.hp / g.cmd.max_hp,
        Color::new(0.3, 0.85, 0.4, 1.0),
    );
    draw_text(
        format!("{:.0} / {:.0}", g.cmd.hp, g.cmd.max_hp),
        24.0,
        51.0,
        14.0,
        BLACK,
    );
    draw_text(
        format!("Metal  {:>5.0}  (+{:.1}/s)", g.metal, g.metal_income),
        20.0,
        74.0,
        20.0,
        METAL,
    );
    draw_text(
        format!("Energy {:>5.0}  (+{:.1}/s)", g.energy, g.energy_rate()),
        20.0,
        96.0,
        20.0,
        ENERGY,
    );
    // Timer.
    let left = BOSS_TIME - g.time;
    let (label, col) = if g.boss_spawned {
        ("DESTROY THE KORGOTH".to_string(), MAGENTA)
    } else {
        (
            format!("{}   boss in {}", fmt_time(g.time), fmt_time(left)),
            WHITE,
        )
    };
    draw_rectangle(sw / 2.0 - 170.0, 14.0, 340.0, 34.0, PANEL);
    text_c(&label, sw / 2.0, 38.0, 24.0, col);
    // Right panel.
    draw_rectangle(sw - 190.0, 14.0, 180.0, 56.0, PANEL);
    draw_text(
        format!("Kills {}", g.stats.kills),
        sw - 180.0,
        36.0,
        20.0,
        WHITE,
    );
    draw_text(
        if muted {
            "[M] sound off"
        } else {
            "[M] sound on"
        },
        sw - 180.0,
        60.0,
        18.0,
        GRAY,
    );
    // Build bar.
    let slot = 150.0;
    let x0 = sw / 2.0 - slot * 2.5;
    let y0 = sh - 74.0;
    for (i, k) in BUILD_KINDS.iter().enumerate() {
        let x = x0 + i as f32 * slot;
        let afford = g.can_afford(*k);
        let sel = selected == Some(i);
        draw_rectangle(x + 2.0, y0, slot - 4.0, 64.0, PANEL);
        if sel {
            draw_rectangle_lines(x + 2.0, y0, slot - 4.0, 64.0, 3.0, ACCENT);
        }
        let c = if afford { WHITE } else { DARKGRAY };
        draw_text(format!("{}", i + 1), x + 8.0, y0 + 20.0, 22.0, ACCENT);
        let short = match k {
            BuildKind::LaserTower => "Laser Tower",
            BuildKind::Solar => "Solar",
            BuildKind::HeavyLaser => "Heavy Laser",
            BuildKind::Nano => "Nano Turret",
            BuildKind::Converter => "Converter",
        };
        draw_text(short, x + 26.0, y0 + 20.0, 18.0, c);
        let (m, e) = g.cost(*k);
        draw_text(
            format!("{m:.0} M"),
            x + 10.0,
            y0 + 42.0,
            18.0,
            if g.metal >= m { METAL } else { RED },
        );
        draw_text(
            format!("{e:.0} E"),
            x + 72.0,
            y0 + 42.0,
            18.0,
            if g.energy >= e { ENERGY } else { RED },
        );
        let desc = match k {
            BuildKind::LaserTower => "fast laser",
            BuildKind::Solar => "+energy",
            BuildKind::HeavyLaser => "heavy laser",
            BuildKind::Nano => "repair+reclaim",
            BuildKind::Converter => "E -> M",
        };
        draw_text(desc, x + 10.0, y0 + 58.0, 15.0, GRAY);
    }
    // D-Gun.
    let dx = sw - 170.0;
    draw_rectangle(dx, y0, 160.0, 64.0, PANEL);
    draw_text("D-GUN [Space]", dx + 10.0, y0 + 22.0, 18.0, ORANGE);
    let frac = 1.0 - g.cmd.dgun_cd / g.cmd.dgun_every;
    bar(
        dx + 10.0,
        y0 + 32.0,
        140.0,
        12.0,
        frac,
        if g.dgun_ready() { ORANGE } else { BROWN },
    );
    draw_text(
        if g.dgun_ready() { "READY" } else { "charging" },
        dx + 10.0,
        y0 + 58.0,
        16.0,
        if g.dgun_ready() { YELLOW } else { GRAY },
    );
    draw_text(
        "WASD move  |  1-5 select, click place  |  hold B + 1-5 quick build  |  P pause",
        10.0,
        sh - 84.0,
        16.0,
        Color::new(1.0, 1.0, 1.0, 0.45),
    );
}

pub fn banner(text: &str, t: f32) {
    let a = t.min(1.0);
    let y = screen_height() * 0.25;
    draw_rectangle(
        0.0,
        y - 36.0,
        screen_width(),
        52.0,
        Color::new(0.0, 0.0, 0.0, 0.5 * a),
    );
    text_c(
        text,
        screen_width() / 2.0,
        y,
        36.0,
        Color::new(1.0, 0.8, 0.3, a),
    );
}

fn card_rects() -> [Rect; 3] {
    let w = 280.0;
    let h = 170.0;
    let gap = 24.0;
    let x0 = screen_width() / 2.0 - (w * 3.0 + gap * 2.0) / 2.0;
    let y = screen_height() / 2.0 - h / 2.0;
    [0, 1, 2].map(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h))
}

pub fn card_at(p: Vec2) -> Option<usize> {
    card_rects().iter().position(|r| r.contains(p))
}

pub fn level_up(g: &Game, opts: &[Upgrade; 3]) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    text_c(
        &format!("LEVEL {} - choose an upgrade", g.level),
        screen_width() / 2.0,
        screen_height() / 2.0 - 120.0,
        36.0,
        ACCENT,
    );
    let mouse: Vec2 = mouse_position().into();
    for (i, (r, u)) in card_rects().iter().zip(opts).enumerate() {
        let hover = r.contains(mouse);
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.08, 0.1, 0.14, 0.95));
        let tag_c = if u.for_buildings() { ENERGY } else { ACCENT };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if hover { 4.0 } else { 2.0 }, tag_c);
        draw_text(format!("[{}]", i + 1), r.x + 12.0, r.y + 28.0, 24.0, WHITE);
        draw_text(
            if u.for_buildings() {
                "BUILDINGS"
            } else {
                "COMMANDER"
            },
            r.x + 60.0,
            r.y + 28.0,
            20.0,
            tag_c,
        );
        draw_text(u.title(), r.x + 12.0, r.y + 76.0, 26.0, WHITE);
        draw_text(u.text(), r.x + 12.0, r.y + 110.0, 18.0, LIGHTGRAY);
    }
}

pub fn title(muted: bool) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    text_c("COMMANDER HOLDOUT", sw / 2.0, sh * 0.26, 72.0, ACCENT);
    text_c(
        "Your Commander drops alone. Drones swarm from every side.",
        sw / 2.0,
        sh * 0.26 + 44.0,
        24.0,
        LIGHTGRAY,
    );
    let lines = [
        "WASD / arrows  -  move (the laser fires automatically)",
        "Space / right-click  -  D-Gun along the mouse direction",
        "1-5 then click, or hold B + 1-5  -  build at the cursor",
        "Walk near wrecks to reclaim metal; Solar gives energy",
        "Level-ups upgrade the Commander or your buildings",
        "Survive 20 minutes, then destroy the T3 Korgoth",
        "M  -  mute      P  -  pause",
    ];
    for (i, l) in lines.iter().enumerate() {
        text_c(l, sw / 2.0, sh * 0.45 + i as f32 * 28.0, 22.0, WHITE);
    }
    let blink = (get_time() * 2.0).sin() > -0.3;
    if blink {
        text_c("Press any key to deploy", sw / 2.0, sh * 0.85, 34.0, YELLOW);
    }
    if muted {
        text_c("(sound muted)", sw / 2.0, sh * 0.85 + 30.0, 18.0, GRAY);
    }
}

pub fn paused() {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.5),
    );
    text_c(
        "PAUSED",
        screen_width() / 2.0,
        screen_height() / 2.0,
        60.0,
        WHITE,
    );
    text_c(
        "press P to resume",
        screen_width() / 2.0,
        screen_height() / 2.0 + 40.0,
        24.0,
        GRAY,
    );
}

pub fn end_screen(g: &Game, won: bool) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.65));
    let (t, c) = if won {
        ("VICTORY - THE KORGOTH IS SCRAP", GREEN)
    } else {
        ("COMMANDER DESTROYED", RED)
    };
    text_c(t, sw / 2.0, sh * 0.35, 56.0, c);
    let lines = [
        format!("Survived {}", fmt_time(g.time)),
        format!("Level {}   Kills {}", g.level, g.stats.kills),
        format!(
            "Metal reclaimed {:.0}   Buildings placed {}",
            g.stats.metal_reclaimed, g.stats.built
        ),
    ];
    for (i, l) in lines.iter().enumerate() {
        text_c(l, sw / 2.0, sh * 0.47 + i as f32 * 32.0, 26.0, WHITE);
    }
    text_c("Press R to redeploy", sw / 2.0, sh * 0.72, 32.0, YELLOW);
}
