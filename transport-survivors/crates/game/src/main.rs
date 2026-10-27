mod render;
mod ui;

use macroquad::prelude::*;
use sim::train::Turn;
use sim::world::{DT, Input, State, World};
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed};

/// At most this many simulation steps per frame, so a slow frame does not freeze the game.
const MAX_STEPS: u32 = 5;

fn window_conf() -> Conf {
    Conf {
        window_title: "Transport Survivors".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Run,
    Paused,
}

fn read_input() -> Input {
    let left = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
    let right = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);
    Input {
        turn: match (left, right) {
            (true, false) => Some(Turn::Left),
            (false, true) => Some(Turn::Right),
            _ => None,
        },
        reverse: is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down),
    }
}

fn confirm_pressed() -> bool {
    is_key_pressed(KeyCode::Enter)
        || is_key_pressed(KeyCode::Space)
        || is_mouse_button_pressed(MouseButton::Left)
}

fn new_world() -> World {
    World::new((macroquad::miniquad::date::now() * 1000.0) as u64)
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::small("Pause", KeyCode::P),
        touch::small("Restart", KeyCode::R),
        touch::small("Start", KeyCode::Enter),
    ]);
    let mut world = new_world();
    let mut screen = Screen::Title;
    let mut acc = 0.0;
    let mut last_hp = world.hp;
    let mut hurt_flash = 0.0f32;
    loop {
        let pause_key = is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::P);
        match screen {
            Screen::Title => {
                if confirm_pressed() {
                    screen = Screen::Run;
                }
            }
            Screen::Run => {
                if pause_key && world.state == State::Running {
                    screen = Screen::Paused;
                }
            }
            Screen::Paused => {
                if pause_key {
                    screen = Screen::Run;
                } else if is_key_pressed(KeyCode::R) {
                    world = new_world();
                    screen = Screen::Run;
                }
            }
        }
        if matches!(world.state, State::Lost | State::Won) && is_key_pressed(KeyCode::Enter) {
            world = new_world();
            last_hp = world.hp;
        }

        if screen == Screen::Run {
            let mut input = read_input();
            acc += get_frame_time();
            let mut steps = 0;
            while acc >= DT && steps < MAX_STEPS {
                world.step(&input);
                input.reverse = false;
                acc -= DT;
                steps += 1;
            }
            if steps == MAX_STEPS {
                acc = 0.0;
            }
        } else {
            acc = 0.0;
        }
        if world.hp < last_hp {
            hurt_flash = 0.3;
        }
        last_hp = world.hp;
        hurt_flash -= get_frame_time();

        let cam = render::camera(world.head_pos());
        set_camera(&cam);
        render::draw_world(&world, &cam);
        set_default_camera();
        if hurt_flash > 0.0 {
            ui::hurt(hurt_flash / 0.3);
        }

        match screen {
            Screen::Title => ui::title(),
            Screen::Paused => {
                ui::hud(&world);
                ui::paused();
            }
            Screen::Run => {
                ui::hud(&world);
                match world.state {
                    State::Running => {}
                    State::LevelUp(cards) => {
                        if let Some(i) = ui::level_up(&world, &cards) {
                            world.pick_card(i);
                            acc = 0.0;
                        }
                    }
                    State::Lost => ui::game_over(&world, false),
                    State::Won => ui::game_over(&world, true),
                }
            }
        }
        touch::next_frame().await;
    }
}
