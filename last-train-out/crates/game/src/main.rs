//! Last Train Out: macroquad front end (input, camera, drawing, audio).

mod audio;
mod draw;
mod ui;

use audio::Audio;
use draw::Fx;
use macroquad::prelude::*;
use rules::game::{CarKind, Game, Input, Phase};
use rules::math::{LOOP_R, V2};
use touch::{
    is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_position,
};

fn conf() -> Conf {
    Conf {
        window_title: "Last Train Out".into(),
        window_width: 1280,
        window_height: 760,
        high_dpi: false,
        sample_count: 4,
        ..Default::default()
    }
}

enum Screen {
    Title,
    Play,
    Over,
}

/// World units visible vertically.
const VIEW_H: f32 = 820.0;

fn read_input(cam: &Camera2D) -> Input {
    let mut walk = 0.0;
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        walk -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        walk += 1.0;
    }
    let mut car_step = 0;
    if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
        car_step -= 1;
    }
    if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
        car_step += 1;
    }
    let mut throttle = 0;
    if is_key_pressed(KeyCode::Q) {
        throttle -= 1;
    }
    if is_key_pressed(KeyCode::E) {
        throttle += 1;
    }
    let aim = cam.screen_to_world(mouse_position().into());
    Input {
        walk,
        car_step,
        fire: is_key_down(KeyCode::Space) || is_mouse_button_down(MouseButton::Left),
        aim: V2::new(aim.x, aim.y),
        throttle,
    }
}

fn camera(target: Vec2) -> Camera2D {
    let w = VIEW_H * screen_width() / screen_height().max(1.0);
    Camera2D::from_display_rect(Rect::new(
        target.x - w / 2.0,
        target.y - VIEW_H / 2.0,
        w,
        VIEW_H,
    ))
}

fn view_rect(target: Vec2) -> Rect {
    let w = VIEW_H * screen_width() / screen_height().max(1.0);
    Rect::new(target.x - w / 2.0, target.y - VIEW_H / 2.0, w, VIEW_H)
}

fn draw_world(game: &Game, fx: &Fx, cam_target: Vec2, time: f32) {
    let cam = camera(cam_target);
    set_camera(&cam);
    let view = view_rect(cam_target);
    draw::draw_ground(view, time);
    draw::draw_track(view);
    draw::draw_stations(game, time);
    draw::draw_projectiles(game, time);
    draw::draw_train(game, time);
    draw::draw_enemies(game);
    fx.draw();
    set_default_camera();
    draw::station_labels(game, &cam);
    fx.draw_floats(&cam);
}

fn crosshair() {
    let (x, y) = mouse_position();
    let c = Color::new(1.0, 1.0, 1.0, 0.7);
    draw_circle_lines(x, y, 10.0, 1.5, c);
    draw_line(x - 15.0, y, x - 5.0, y, 1.5, c);
    draw_line(x + 5.0, y, x + 15.0, y, 1.5, c);
    draw_line(x, y - 15.0, x, y - 5.0, 1.5, c);
    draw_line(x, y + 5.0, x, y + 15.0, 1.5, c);
}

/// Rasterises every glyph size the UI uses up front: growing the font atlas
/// mid-frame would rebind a texture that was just deleted.
async fn warm_glyph_cache() {
    let chars: String = (32u8..127).map(char::from).collect();
    for size in [
        13, 14, 16, 17, 18, 20, 22, 24, 26, 28, 32, 36, 42, 60, 80, 84,
    ] {
        measure_text(&chars, None, size, 1.0);
        touch::next_frame().await;
    }
}

#[macroquad::main(conf)]
async fn main() {
    touch::set_buttons(&[
        touch::key("Slower", KeyCode::Q),
        touch::key("Faster", KeyCode::E),
        touch::key("Yard", KeyCode::B),
        touch::small("1", KeyCode::Key1),
        touch::small("2", KeyCode::Key2),
        touch::small("3", KeyCode::Key3),
        touch::small("4", KeyCode::Key4),
        touch::small("5", KeyCode::Key5),
        touch::small("6", KeyCode::Key6),
        touch::small("Repair", KeyCode::R),
        touch::small("Shunt-", KeyCode::Z),
        touch::small("Shunt+", KeyCode::X),
        touch::small("Pause", KeyCode::P),
        touch::small("Mute", KeyCode::M),
        touch::small("Start", KeyCode::Enter),
    ]);
    warm_glyph_cache().await;
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut game = Game::new(1);
    let mut fx = Fx::default();
    let mut cam_target = vec2(LOOP_R, 0.0);
    let mut build_open = false;
    let mut paused = false;
    let mut over_t = 0.0;
    let mut seed = 1u64;

    loop {
        let dt = get_frame_time().min(0.05);
        let time = get_time() as f32;
        clear_background(Color::from_rgba(36, 32, 28, 255));
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }

        match screen {
            Screen::Title => {
                // Idle train circling behind the title.
                game.step(dt, &Input::default());
                game.events.clear();
                fx.update(dt, &game);
                cam_target = cam_target.lerp(draw::v(game.car_pos(1)), (dt * 2.0).min(1.0));
                draw_world(&game, &fx, cam_target, time);
                draw_rectangle(
                    0.0,
                    0.0,
                    screen_width(),
                    screen_height(),
                    Color::new(0.0, 0.0, 0.0, 0.55),
                );
                ui::title(time);
                let key = get_last_key_pressed();
                let any = key.is_some_and(|k| k != KeyCode::M)
                    || is_mouse_button_pressed(MouseButton::Left);
                if any {
                    audio.start_music();
                    seed = seed.wrapping_add((time * 1000.0) as u64);
                    game = Game::new(seed);
                    game.throttle = 1;
                    fx = Fx::default();
                    build_open = false;
                    paused = false;
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
                if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                    paused = !paused;
                }
                let cam = camera(cam_target);
                if !paused {
                    if game.phase == Phase::LevelUp {
                        build_open = false;
                        for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                            .into_iter()
                            .enumerate()
                        {
                            if is_key_pressed(key) {
                                game.choose(k);
                            }
                        }
                    } else {
                        if is_key_pressed(KeyCode::B) && game.at_station.is_some() {
                            build_open = !build_open;
                        }
                        if game.at_station.is_none() {
                            build_open = false;
                        }
                        if build_open {
                            let keys = [
                                KeyCode::Key1,
                                KeyCode::Key2,
                                KeyCode::Key3,
                                KeyCode::Key4,
                                KeyCode::Key5,
                                KeyCode::Key6,
                            ];
                            for (k, key) in keys.into_iter().enumerate() {
                                if is_key_pressed(key) {
                                    game.buy(CarKind::BUYABLE[k]);
                                }
                            }
                            if is_key_pressed(KeyCode::R) {
                                game.repair_all();
                            }
                            if is_key_pressed(KeyCode::Z) {
                                game.shunt(-1);
                            }
                            if is_key_pressed(KeyCode::X) {
                                game.shunt(1);
                            }
                        }
                    }
                    let input = read_input(&cam);
                    game.step(dt, &input);
                    for e in game.events.drain(..) {
                        audio.on_event(&e);
                        fx.on_event(&e);
                    }
                    fx.update(dt, &game);
                }
                let want = draw::v(game.conductor_pos());
                cam_target = cam_target.lerp(want, (dt * 5.0).min(1.0));
                let shake = vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * fx.shake;
                draw_world(&game, &fx, cam_target + shake, time);
                ui::hud(&game, audio.muted, build_open);
                if build_open {
                    ui::build_menu(&game);
                }
                if game.phase == Phase::LevelUp {
                    if let Some(h) = ui::level_up(&game)
                        && is_mouse_button_pressed(MouseButton::Left)
                    {
                        game.choose(h);
                    }
                } else {
                    crosshair();
                }
                if paused {
                    ui::paused();
                }
                if matches!(game.phase, Phase::Won | Phase::Lost) {
                    over_t = 0.0;
                    screen = Screen::Over;
                }
            }
            Screen::Over => {
                over_t += dt;
                fx.update(dt, &game);
                draw_world(&game, &fx, cam_target, time);
                ui::game_over(&game);
                if over_t > 1.0
                    && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space))
                {
                    game = Game::new(seed ^ 0x55);
                    fx = Fx::default();
                    screen = Screen::Title;
                }
            }
        }
        touch::next_frame().await;
    }
}
