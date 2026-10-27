mod render;

use macroquad::prelude::*;
use sim::train::Turn;
use sim::world::{DT, Input, World};

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

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new((macroquad::miniquad::date::now() * 1000.0) as u64);
    let mut acc = 0.0;
    loop {
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

        let cam = render::camera(world.head_pos());
        set_camera(&cam);
        render::draw_world(&world, &cam);
        set_default_camera();
        draw_text(
            format!(
                "HP {:.0}   speed {:.1}   A/D turn   S reverse",
                world.hp,
                world.train.speed()
            ),
            16.0,
            28.0,
            24.0,
            WHITE,
        );
        next_frame().await;
    }
}
