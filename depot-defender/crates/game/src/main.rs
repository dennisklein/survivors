mod audio;
mod draw;
mod fx;
mod ui;

use audio::Audio;
use fx::Fx;
use macroquad::prelude::*;
use rules::game::{Game, Input, Phase};
use rules::math::V2;
use touch::{is_key_down, is_key_pressed, is_mouse_button_pressed};

const DT: f32 = 1.0 / 60.0;
const MAX_STEPS: u32 = 5;

fn window_conf() -> Conf {
    Conf {
        window_title: "Depot Defender".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        ..Default::default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Run,
}

fn read_input() -> Input {
    let mut d = V2::ZERO;
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        d.x -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        d.x += 1.0;
    }
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        d.y -= 1.0;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        d.y += 1.0;
    }
    Input {
        move_dir: d,
        unload: is_key_pressed(KeyCode::E),
    }
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || !touches().is_empty()
}

fn seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

struct Run {
    game: Game,
    fx: Fx,
    map_open: bool,
    paused: bool,
    acc: f32,
}

impl Run {
    fn new() -> Self {
        let game = Game::new(seed());
        let fx = Fx::new(game.player.pos);
        Self {
            game,
            fx,
            map_open: false,
            paused: false,
            acc: 0.0,
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Unload", KeyCode::E),
        touch::key("Depart", KeyCode::Space),
        touch::small("Map", KeyCode::Tab),
        touch::small("Pause", KeyCode::P),
        touch::small("Mute", KeyCode::M),
        touch::small("Retry", KeyCode::R),
    ]);
    // Draw something while the sounds are synthesized and decoded.
    clear_background(ui::BG);
    ui::warm_glyph_cache();
    touch::next_frame().await;
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut run = Run::new();
    let mut title_time = 0.0f32;

    loop {
        let frame_dt = get_frame_time().min(0.1);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match screen {
            Screen::Title => {
                title_time += frame_dt;
                ui::title_screen(title_time, audio.muted);
                if any_input() && !is_key_pressed(KeyCode::M) {
                    audio.start_music();
                    run = Run::new();
                    screen = Screen::Run;
                }
            }
            Screen::Run => {
                update_run(&mut run, frame_dt);
                draw::world(&run.game, &run.fx);
                ui::hud(&run.game, &run.fx, audio.muted);
                match run.game.phase.clone() {
                    Phase::LevelUp(choices) => {
                        if let Some(i) = ui::level_up(&choices, &run.game) {
                            run.game.choose_upgrade(i);
                        }
                    }
                    Phase::Shop => {
                        ui::shop(&mut run.game);
                        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                            run.game.depart();
                        }
                    }
                    Phase::Won | Phase::Lost => {
                        ui::game_over(&run.game);
                        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::R) {
                            screen = Screen::Title;
                            title_time = 0.0;
                        }
                    }
                    Phase::Playing if run.map_open => ui::line_map(&run.game),
                    Phase::Playing if run.paused => ui::paused(),
                    Phase::Playing => {}
                }
                drain_events(&mut run, &mut audio);
            }
        }
        touch::next_frame().await;
    }
}

fn update_run(run: &mut Run, frame_dt: f32) {
    let playing = run.game.phase == Phase::Playing;
    if playing && is_key_pressed(KeyCode::Tab) {
        run.map_open = !run.map_open;
    }
    if playing && (is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape)) {
        if run.map_open {
            run.map_open = false;
        } else {
            run.paused = !run.paused;
        }
    }
    if !playing {
        run.map_open = false;
        run.paused = false;
    }
    if run.map_open || run.paused {
        return;
    }
    let mut input = read_input();
    run.acc += frame_dt;
    let mut steps = 0;
    while run.acc >= DT && steps < MAX_STEPS {
        run.game.step(DT, &input);
        // Only the first sub-step sees the key press.
        input.unload = false;
        run.acc -= DT;
        steps += 1;
    }
    if steps == MAX_STEPS {
        run.acc = 0.0;
    }
    run.fx.update(frame_dt, &run.game);
}

fn drain_events(run: &mut Run, audio: &mut Audio) {
    for e in std::mem::take(&mut run.game.events) {
        audio.on_event(&e);
        run.fx.on_event(&e, &run.game);
    }
}
