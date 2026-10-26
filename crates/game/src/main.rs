use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Transport Survivors".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::from_rgba(52, 92, 44, 255));
        draw_text("Transport Survivors", 20.0, 40.0, 32.0, WHITE);
        next_frame().await;
    }
}
