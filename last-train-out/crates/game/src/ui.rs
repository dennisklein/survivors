//! Screen-space HUD, menus and title/game-over screens.

use crate::draw::{STATION_NAMES, car_color, car_letter};
use macroquad::prelude::*;
use rules::game::{CarKind, GAME_LEN, Game, MAX_CARS, Phase};
use rules::math::{CIRC, wrap};
use touch::mouse_position;

pub const INK: Color = Color::new(0.95, 0.92, 0.85, 1.0);
pub const DIM: Color = Color::new(0.65, 0.62, 0.58, 1.0);
const PANEL: Color = Color::new(0.06, 0.05, 0.05, 0.82);

pub fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(s, x, y, size, color);
}

pub fn text_c(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    let w = measure_text(s, None, size as u16, 1.0).width;
    draw_text(s, cx - w / 2.0, y, size, color);
}

fn bar(x: f32, y: f32, w: f32, h: f32, k: f32, fg: Color) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(x, y, w * k.clamp(0.0, 1.0), h, fg);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
}

pub fn hud(game: &Game, muted: bool, build_open: bool) {
    let sw = screen_width();
    let sh = screen_height();

    // Departure clock.
    let left = game.time_left();
    let clock = format!("{:02}:{:02}", (left / 60.0) as u32, (left % 60.0) as u32);
    draw_rectangle(sw / 2.0 - 120.0, 6.0, 240.0, 62.0, PANEL);
    text_c(&clock, sw / 2.0, 42.0, 42.0, INK);
    text_c("until the last train departs", sw / 2.0, 58.0, 16.0, DIM);
    bar(
        sw / 2.0 - 110.0,
        62.0,
        220.0,
        4.0,
        game.time / GAME_LEN,
        SKYBLUE,
    );

    // Economy and passengers.
    draw_rectangle(8.0, 8.0, 230.0, 96.0, PANEL);
    text(&format!("${}", game.money), 18.0, 36.0, 32.0, GOLD);
    text(
        &format!("Aboard {}/{}", game.aboard(), game.capacity()),
        18.0,
        60.0,
        22.0,
        SKYBLUE,
    );
    text(&format!("Rescued {}", game.rescued), 18.0, 80.0, 20.0, INK);
    text(&format!("Kills {}", game.kills), 130.0, 80.0, 20.0, DIM);
    if game.lost_passengers > 0 {
        text(
            &format!("Lost {}", game.lost_passengers),
            18.0,
            98.0,
            18.0,
            Color::from_rgba(230, 110, 100, 255),
        );
    }

    // Level and throttle.
    let rx = sw - 238.0;
    draw_rectangle(rx, 8.0, 230.0, 96.0, PANEL);
    text(&format!("Level {}", game.level), rx + 10.0, 32.0, 26.0, INK);
    bar(
        rx + 10.0,
        40.0,
        210.0,
        8.0,
        game.xp as f32 / game.xp_next as f32,
        Color::from_rgba(140, 220, 120, 255),
    );
    text("Throttle", rx + 10.0, 70.0, 20.0, DIM);
    for n in 0..4 {
        let on = n <= game.throttle && game.throttle > 0;
        let col = if on {
            Color::from_rgba(250, 160, 60, 255)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.15)
        };
        if n > 0 {
            draw_rectangle(
                rx + 80.0 + n as f32 * 22.0,
                72.0 - n as f32 * 6.0,
                16.0,
                n as f32 * 6.0 + 4.0,
                col,
            );
        }
    }
    text(
        &format!("{:.0} km/h", game.speed * 0.6),
        rx + 10.0,
        94.0,
        18.0,
        DIM,
    );
    if muted {
        text(
            "MUTED (M)",
            rx + 140.0,
            94.0,
            18.0,
            Color::from_rgba(230, 110, 100, 255),
        );
    }

    // Station status.
    let msg = if let Some(k) = game.at_station {
        if build_open {
            None
        } else {
            Some((
                format!("At {} - B: build & shunt cars, E: depart", STATION_NAMES[k]),
                GREEN,
            ))
        }
    } else {
        let (k, d) = next_station(game);
        if d < 500.0 {
            if game.throttle == 0 {
                Some((
                    format!("Overshot? {} is {:.0}m away", STATION_NAMES[k], d),
                    ORANGE,
                ))
            } else {
                Some((
                    format!("{} ahead in {:.0}m - Q to brake", STATION_NAMES[k], d),
                    YELLOW,
                ))
            }
        } else {
            Some((format!("Next: {} {:.0}m", STATION_NAMES[k], d), DIM))
        }
    };
    if let Some((m, c)) = msg {
        let w = measure_text(&m, None, 22, 1.0).width;
        draw_rectangle(sw / 2.0 - w / 2.0 - 10.0, 76.0, w + 20.0, 28.0, PANEL);
        text_c(&m, sw / 2.0, 97.0, 22.0, c);
    }

    consist_strip(game, sw, sh);
    text_c(
        "A/D walk  W/S jump car  Space/LMB fire  Q/E throttle  B build  M mute  P pause",
        sw / 2.0,
        sh - 8.0,
        16.0,
        DIM,
    );
}

/// The next station ahead along the direction of travel, and its distance.
pub fn next_station(game: &Game) -> (usize, f32) {
    game.stations
        .iter()
        .enumerate()
        .map(|(k, st)| (k, wrap(st.s - game.head_s)))
        .map(|(k, d)| (k, if d > CIRC - 160.0 { d - CIRC } else { d }))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, d)| (k, d.abs()))
        .unwrap_or((0, 0.0))
}

fn consist_strip(game: &Game, sw: f32, sh: f32) {
    let cw = 44.0;
    let total = MAX_CARS as f32 * (cw + 4.0);
    let x0 = sw / 2.0 - total / 2.0;
    let y = sh - 74.0;
    draw_rectangle(x0 - 8.0, y - 8.0, total + 12.0, 56.0, PANEL);
    let manned = game.conductor_car();
    for i in 0..MAX_CARS {
        let x = x0 + i as f32 * (cw + 4.0);
        let Some(car) = game.cars.get(i) else {
            draw_rectangle_lines(x, y, cw, 30.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.12));
            continue;
        };
        draw_rectangle(x, y, cw, 30.0, car_color(car.kind));
        if car.hurt < 0.15 {
            draw_rectangle(x, y, cw, 30.0, Color::new(1.0, 0.2, 0.1, 0.5));
        }
        let fg = if car.kind == CarKind::Infirmary {
            BLACK
        } else {
            INK
        };
        text_c(car_letter(car.kind), x + cw / 2.0, y + 21.0, 22.0, fg);
        if car.kind == CarKind::Coach {
            text(
                &format!("{}/{}", car.passengers.len(), game.seats(i)),
                x + 2.0,
                y + 29.0,
                13.0,
                SKYBLUE,
            );
        }
        let k = car.hp / car.max_hp;
        let col = if k > 0.5 {
            GREEN
        } else if k > 0.25 {
            ORANGE
        } else {
            RED
        };
        bar(x, y + 33.0, cw, 5.0, k, col);
        if i == manned {
            draw_rectangle_lines(x - 2.0, y - 2.0, cw + 4.0, 34.0, 2.0, YELLOW);
            text_c("you", x + cw / 2.0, y - 10.0, 16.0, YELLOW);
        }
    }
}

pub fn build_menu(game: &Game) {
    let sw = screen_width();
    let w = 520.0;
    let h = 350.0;
    let x = sw / 2.0 - w / 2.0;
    let y = 112.0;
    draw_rectangle(x, y, w, h, PANEL);
    draw_rectangle_lines(x, y, w, h, 2.0, GOLD);
    let name = game.at_station.map(|k| STATION_NAMES[k]).unwrap_or("");
    text(&format!("{name} yard"), x + 16.0, y + 30.0, 28.0, INK);
    text(
        &format!("${}  cars {}/{}", game.money, game.cars.len(), MAX_CARS),
        x + w - 170.0,
        y + 30.0,
        22.0,
        GOLD,
    );
    for (n, kind) in CarKind::BUYABLE.iter().enumerate() {
        let price = game.price(*kind);
        let ok = game.money >= price && game.cars.len() < MAX_CARS;
        let ty = y + 64.0 + n as f32 * 38.0;
        draw_rectangle(x + 16.0, ty - 18.0, 26.0, 26.0, car_color(*kind));
        let c = if ok { INK } else { DIM };
        text(
            &format!("{}", n + 1),
            x + 23.0,
            ty + 2.0,
            22.0,
            if *kind == CarKind::Infirmary {
                BLACK
            } else {
                INK
            },
        );
        text(
            &format!("{}  ${price}", kind.name()),
            x + 52.0,
            ty - 2.0,
            22.0,
            c,
        );
        text(kind.blurb(), x + 52.0, ty + 14.0, 16.0, DIM);
    }
    let ty = y + h - 46.0;
    text(
        &format!(
            "R repair all ${}   Z/X shunt your car forward/back",
            game.repair_cost()
        ),
        x + 16.0,
        ty,
        18.0,
        INK,
    );
    text(
        "1-6 buy (couples at the tail). Workshops, tenders and infirmaries",
        x + 16.0,
        ty + 20.0,
        16.0,
        DIM,
    );
    text(
        "buff their neighbours - arrange the consist!",
        x + 16.0,
        ty + 36.0,
        16.0,
        DIM,
    );
    text("B close", x + w - 70.0, y + h - 6.0, 16.0, DIM);
}

/// Draws level-up cards; returns the card under the mouse.
pub fn level_up(game: &Game) -> Option<usize> {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
    text_c(
        "PROMOTION!  Choose an upgrade",
        sw / 2.0,
        sh / 2.0 - 130.0,
        36.0,
        GOLD,
    );
    let cw = 230.0;
    let gap = 20.0;
    let x0 = sw / 2.0 - (3.0 * cw + 2.0 * gap) / 2.0;
    let (mx, my) = mouse_position();
    let mut hover = None;
    for (i, up) in game.choices.iter().enumerate() {
        let x = x0 + i as f32 * (cw + gap);
        let y = sh / 2.0 - 100.0;
        let r = Rect::new(x, y, cw, 170.0);
        let over = r.contains(vec2(mx, my));
        if over {
            hover = Some(i);
        }
        draw_rectangle(
            x,
            y,
            cw,
            170.0,
            if over {
                Color::new(0.2, 0.17, 0.1, 0.95)
            } else {
                PANEL
            },
        );
        draw_rectangle_lines(x, y, cw, 170.0, 2.0, if over { GOLD } else { DIM });
        text_c(&format!("[{}]", i + 1), x + cw / 2.0, y + 34.0, 26.0, GOLD);
        text_c(up.name(), x + cw / 2.0, y + 80.0, 26.0, INK);
        text_c(up.blurb(), x + cw / 2.0, y + 116.0, 17.0, DIM);
    }
    hover
}

pub fn title(time: f32) {
    let sw = screen_width();
    let sh = screen_height();
    text_c(
        "LAST TRAIN OUT",
        sw / 2.0,
        sh * 0.28,
        84.0,
        Color::from_rgba(250, 190, 90, 255),
    );
    text_c(
        "The horde is coming. One armoured train. One loop line. Fifteen minutes.",
        sw / 2.0,
        sh * 0.28 + 44.0,
        22.0,
        INK,
    );
    let lines = [
        "You are the conductor. Walk the carriages, man the guns, keep the engine alive.",
        "Stop at stations to load refugees; deliver them to another station for fares.",
        "Spend fares on new carriages and arrange the consist - neighbours buff each other.",
        "",
        "A/D walk along the train     W/S jump a carriage     Space or mouse: fire manned gun",
        "Q/E throttle down/up     B build menu at stations     M mute     P pause",
    ];
    for (i, l) in lines.iter().enumerate() {
        text_c(l, sw / 2.0, sh * 0.48 + i as f32 * 26.0, 20.0, DIM);
    }
    if (time * 2.0).fract() < 0.65 {
        text_c("Press any key to board", sw / 2.0, sh * 0.85, 32.0, INK);
    }
}

pub fn game_over(game: &Game) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
    let (head, col, sub) = if game.phase == Phase::Won {
        (
            "DEPARTED!",
            Color::from_rgba(140, 230, 120, 255),
            "The last train steams off the loop line to safety.",
        )
    } else {
        (
            "LINE CLOSED",
            Color::from_rgba(240, 90, 80, 255),
            "The engine fell to the horde.",
        )
    };
    text_c(head, sw / 2.0, sh * 0.32, 80.0, col);
    text_c(sub, sw / 2.0, sh * 0.32 + 40.0, 24.0, INK);
    let t = game.time;
    let score = game.rescued * 10
        + game.aboard() as u32 * 10
        + game.kills
        + if game.phase == Phase::Won { 500 } else { 0 };
    let stats = [
        format!("Survived {:02}:{:02}", (t / 60.0) as u32, (t % 60.0) as u32),
        format!(
            "Refugees rescued {}   still aboard {}",
            game.rescued,
            game.aboard()
        ),
        format!("Horde destroyed {}   level {}", game.kills, game.level),
        format!("Carriages {}", game.cars.len()),
        format!("SCORE {score}"),
    ];
    for (i, s) in stats.iter().enumerate() {
        text_c(
            s,
            sw / 2.0,
            sh * 0.5 + i as f32 * 32.0,
            26.0,
            if i == 4 { GOLD } else { INK },
        );
    }
    text_c(
        "Press Enter to return to the platform",
        sw / 2.0,
        sh * 0.85,
        26.0,
        DIM,
    );
}

pub fn paused() {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.5));
    text_c("PAUSED", sw / 2.0, sh / 2.0, 60.0, INK);
    text_c("P to resume", sw / 2.0, sh / 2.0 + 36.0, 24.0, DIM);
}
