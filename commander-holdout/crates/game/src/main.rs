//! Commander Holdout: macroquad front end (input, rendering, audio).

mod audio;
mod draw;

use audio::{Audio, Sfx};
use macroquad::prelude::*;
use sim::game::{BUILD_KINDS, Event, Game, Input, Phase};
use sim::math::V2;

pub const VIEW_H: f32 = 760.0;

#[derive(PartialEq)]
enum Screen {
    Title,
    Play,
    Paused,
}

pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
    pub max: f32,
    pub size: f32,
    pub color: Color,
    pub ring: bool,
}

/// World-to-screen camera centred on the commander.
#[derive(Clone, Copy)]
pub struct Cam {
    pub center: V2,
    pub scale: f32,
}

impl Cam {
    pub fn to_screen(&self, p: V2) -> Vec2 {
        vec2(
            (p.x - self.center.x) * self.scale + screen_width() / 2.0,
            (p.y - self.center.y) * self.scale + screen_height() / 2.0,
        )
    }

    pub fn to_world(&self, s: Vec2) -> V2 {
        V2::new(
            (s.x - screen_width() / 2.0) / self.scale + self.center.x,
            (s.y - screen_height() / 2.0) / self.scale + self.center.y,
        )
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Commander Holdout".to_owned(),
        window_width: 1280,
        window_height: 760,
        high_dpi: true,
        ..Default::default()
    }
}

const NUM_KEYS: [KeyCode; 5] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
];

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
        || !touches().is_empty()
}

fn spawn_burst(parts: &mut Vec<Particle>, pos: V2, n: usize, speed: f32, color: Color, size: f32) {
    for i in 0..n {
        let a = rand::gen_range(0.0, std::f32::consts::TAU) + i as f32;
        let v = rand::gen_range(0.2, 1.0) * speed;
        let life = rand::gen_range(0.25, 0.7);
        parts.push(Particle {
            pos,
            vel: V2::from_angle(a) * v,
            life,
            max: life,
            size: size * rand::gen_range(0.5, 1.2),
            color,
            ring: false,
        });
    }
}

fn spawn_ring(parts: &mut Vec<Particle>, pos: V2, size: f32, color: Color) {
    parts.push(Particle {
        pos,
        vel: V2::ZERO,
        life: 0.35,
        max: 0.35,
        size,
        color,
        ring: true,
    });
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(miniquad::date::now() as u64);
    let mut audio = Audio::new().await;
    let mut game = Game::new(rand::rand() as u64);
    let mut screen = Screen::Title;
    let mut parts: Vec<Particle> = Vec::new();
    let mut selected: Option<usize> = None;
    let mut shake = 0.0f32;
    let mut banner: Option<(String, f32)> = None;
    let mut ended_at = 0.0f64;

    loop {
        let dt = get_frame_time().min(0.05);
        let scale = (screen_height() / VIEW_H).clamp(0.5, 2.5);
        let mut cam = Cam {
            center: game.cmd.pos,
            scale,
        };
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }

        match screen {
            Screen::Title => {
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    audio.play(Sfx::Build);
                    game = Game::new(rand::rand() as u64);
                    parts.clear();
                    selected = None;
                    banner = Some(("Hold out for 20 minutes".into(), 3.0));
                    screen = Screen::Play;
                }
            }
            Screen::Paused => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Play;
                }
            }
            Screen::Play => match game.phase.clone() {
                Phase::Playing => {
                    if is_key_pressed(KeyCode::P) {
                        screen = Screen::Paused;
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
                    let mouse = cam.to_world(mouse_position().into());
                    let mut place = None;
                    for (i, k) in NUM_KEYS.iter().enumerate() {
                        if is_key_pressed(*k) {
                            if is_key_down(KeyCode::B) {
                                place = Some((BUILD_KINDS[i], mouse));
                            } else {
                                selected = if selected == Some(i) { None } else { Some(i) };
                            }
                        }
                    }
                    if let Some(i) = selected {
                        if is_mouse_button_pressed(MouseButton::Left) {
                            place = Some((BUILD_KINDS[i], mouse));
                        }
                        if is_mouse_button_pressed(MouseButton::Right)
                            || is_key_pressed(KeyCode::Escape)
                        {
                            selected = None;
                        }
                    }
                    let dgun = is_key_pressed(KeyCode::Space)
                        || (selected.is_none() && is_mouse_button_pressed(MouseButton::Right));
                    let input = Input {
                        mv,
                        aim: mouse,
                        dgun,
                        place,
                    };
                    game.step(dt, &input);
                }
                Phase::LevelUp(_) => {
                    let mut pick = None;
                    for (i, k) in NUM_KEYS.iter().take(3).enumerate() {
                        if is_key_pressed(*k) {
                            pick = Some(i);
                        }
                    }
                    if is_mouse_button_pressed(MouseButton::Left) {
                        pick = draw::card_at(mouse_position().into());
                    }
                    if let Some(i) = pick {
                        audio.play(Sfx::Metal);
                        game.choose(i);
                    }
                }
                Phase::Won | Phase::Lost => {
                    if get_time() - ended_at > 1.0
                        && (is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Enter))
                    {
                        audio.play(Sfx::Build);
                        game = Game::new(rand::rand() as u64);
                        parts.clear();
                        selected = None;
                        banner = Some(("Hold out for 20 minutes".into(), 3.0));
                    }
                }
            },
        }

        for e in game.drain_events() {
            audio.on_event(&e);
            match e {
                Event::Hit { pos } => spawn_burst(
                    &mut parts,
                    pos,
                    2,
                    90.0,
                    Color::new(1.0, 0.8, 0.4, 1.0),
                    2.0,
                ),
                Event::Kill { pos, big } => {
                    let n = if big { 40 } else { 10 };
                    spawn_burst(
                        &mut parts,
                        pos,
                        n,
                        if big { 260.0 } else { 140.0 },
                        ORANGE,
                        4.0,
                    );
                    spawn_ring(&mut parts, pos, if big { 90.0 } else { 30.0 }, ORANGE);
                    if big {
                        shake = shake.max(0.4);
                    }
                }
                Event::BuildingLost { pos } => {
                    spawn_burst(&mut parts, pos, 30, 200.0, RED, 4.0);
                    spawn_ring(&mut parts, pos, 70.0, RED);
                    shake = shake.max(0.3);
                }
                Event::Reclaimed { pos } => spawn_ring(&mut parts, pos, 24.0, GREEN),
                Event::Built { pos, .. } => {
                    spawn_ring(&mut parts, pos, 40.0, Color::new(0.4, 1.0, 0.6, 1.0))
                }
                Event::DGun { from, to } => {
                    shake = shake.max(0.35);
                    for k in 0..12 {
                        let p = from + (to - from) * (k as f32 / 11.0);
                        spawn_burst(&mut parts, p, 4, 160.0, YELLOW, 4.0);
                    }
                }
                Event::CommanderHit => shake = shake.max(0.12),
                Event::LevelUp => spawn_ring(&mut parts, game.cmd.pos, 80.0, SKYBLUE),
                Event::Surge => banner = Some(("Drone surge incoming!".into(), 2.5)),
                Event::EliteSpawn => banner = Some(("Elite Juggernaut inbound".into(), 2.5)),
                Event::BossWarning => banner = Some(("T3 signature detected...".into(), 4.0)),
                Event::BossSpawn => banner = Some(("KORGOTH HAS LANDED - destroy it!".into(), 4.0)),
                Event::Won | Event::Lost => ended_at = get_time(),
                _ => {}
            }
        }

        for p in &mut parts {
            p.pos += p.vel * dt;
            p.vel = p.vel * (1.0 - 3.0 * dt);
            p.life -= dt;
        }
        parts.retain(|p| p.life > 0.0);
        if let Some((_, t)) = &mut banner {
            *t -= dt;
            if *t <= 0.0 {
                banner = None;
            }
        }
        shake = (shake - dt).max(0.0);
        if shake > 0.0 {
            cam.center += V2::new(
                rand::gen_range(-1.0, 1.0) * shake * 14.0,
                rand::gen_range(-1.0, 1.0) * shake * 14.0,
            );
        }

        draw::world(&game, &cam, &parts, selected, screen == Screen::Title);
        match screen {
            Screen::Title => draw::title(audio.muted),
            _ => {
                draw::hud(&game, selected, audio.muted);
                if let Some((text, t)) = &banner {
                    draw::banner(text, *t);
                }
                match &game.phase {
                    Phase::LevelUp(opts) => draw::level_up(&game, opts),
                    Phase::Won => draw::end_screen(&game, true),
                    Phase::Lost => draw::end_screen(&game, false),
                    Phase::Playing if screen == Screen::Paused => draw::paused(),
                    Phase::Playing => {}
                }
            }
        }
        next_frame().await;
    }
}
