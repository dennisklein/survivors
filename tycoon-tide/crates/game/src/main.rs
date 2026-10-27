mod audio;
mod draw;
mod fx;

use audio::{Audio, Sfx};
use draw::{View, bar};
use fx::Fx;
use macroquad::prelude::*;
use rules::game::{
    BuildError, Event, Game, Input, MAX_LINK, Phase, RESCUE_COST, Steer, TOWN_RADIUS, WAVE_TIME,
    WAVES,
};

const DT: f32 = 1.0 / 60.0;

fn conf() -> Conf {
    Conf {
        window_title: "Tycoon Tide".into(),
        window_width: 1280,
        window_height: 760,
        high_dpi: true,
        ..Default::default()
    }
}

enum Screen {
    Title,
    Playing,
    Over,
}

struct Run {
    game: Game,
    fx: Fx,
    build_mode: bool,
    selected: Option<usize>,
    steer_latch: Steer,
    acc: f32,
}

impl Run {
    fn new(seed: u64) -> Self {
        Self {
            game: Game::new(seed),
            fx: Fx::default(),
            build_mode: false,
            selected: None,
            steer_latch: Steer::Straight,
            acc: 0.0,
        }
    }
}

fn text_c(s: &str, y: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    let x = (screen_width() - d.width) * 0.5;
    draw_text(s, x + 2.0, y + 2.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_text(s, x, y, size, c);
}

fn any_input() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || !touches().is_empty()
}

fn ui_scale() -> f32 {
    (screen_width() / 1280.0)
        .min(screen_height() / 760.0)
        .clamp(0.6, 2.0)
}

fn title_screen(t: f32, demo: &Game, muted: bool) {
    let v = View::fit(0.0, t);
    draw::terrain(&v, t);
    draw::network(&v, demo, None);
    draw::towns(&v, demo, t);
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    let k = ui_scale();
    let h = screen_height();
    text_c("TYCOON TIDE", h * 0.2, 96.0 * k, GOLD);
    text_c(
        "Run the railway. Grow the towns. Hold back the tide.",
        h * 0.2 + 50.0 * k,
        28.0 * k,
        WHITE,
    );
    let lines = [
        "Your HQ locomotive auto-fires at anything in range.",
        "Arrive at stations to collect profit; served towns grow and field militia.",
        "Each wave assaults one town. Send the rescue train to rally its defenders.",
        "Survive 25 waves. Score = company value.",
        "",
        "Up/Down: throttle & brake (Down when stopped: reverse)   Left/Right: pick junction",
        "Space: rescue train ($60)   P: pause & build (mouse)   M: mute   1-3: pick upgrade",
    ];
    for (i, l) in lines.iter().enumerate() {
        text_c(
            l,
            h * 0.42 + i as f32 * 30.0 * k,
            24.0 * k,
            Color::new(0.85, 0.9, 0.85, 1.0),
        );
    }
    if (t * 2.0).fract() < 0.65 {
        text_c("Press any key to start", h * 0.85, 40.0 * k, WHITE);
    }
    if muted {
        text_c("(muted)", h * 0.85 + 36.0 * k, 22.0 * k, GRAY);
    }
}

fn hud(run: &Run, muted: bool) {
    let g = &run.game;
    let k = ui_scale();
    let w = screen_width();
    let bh = 52.0 * k;
    draw_rectangle(0.0, 0.0, w, bh, Color::new(0.05, 0.07, 0.06, 0.82));
    let fs = 24.0 * k;
    let y1 = 22.0 * k;
    let y2 = 44.0 * k;
    draw_text(format!("${}", g.cash as i64), 12.0 * k, y1, fs * 1.2, GOLD);
    draw_text(
        format!("Company value ${}", g.company_value()),
        12.0 * k,
        y2,
        fs * 0.8,
        Color::new(0.9, 0.85, 0.6, 1.0),
    );
    let x = 220.0 * k;
    draw_text("HQ", x, y1, fs, WHITE);
    bar(
        x + 40.0 * k,
        y1 - 14.0 * k,
        160.0 * k,
        14.0 * k,
        g.hp / g.stats.max_hp,
        Color::new(0.9, 0.25, 0.2, 1.0),
    );
    draw_text(format!("Lv {}", g.level), x, y2, fs, SKYBLUE);
    bar(
        x + 60.0 * k,
        y2 - 12.0 * k,
        140.0 * k,
        10.0 * k,
        g.xp / g.xp_needed(),
        SKYBLUE,
    );
    let x = 460.0 * k;
    draw_text(format!("Wave {}/{}", g.wave, WAVES), x, y1, fs, WHITE);
    bar(
        x,
        y2 - 12.0 * k,
        150.0 * k,
        10.0 * k,
        g.wave_time / WAVE_TIME,
        Color::new(0.9, 0.6, 0.2, 1.0),
    );
    let x = 650.0 * k;
    let target = &g.towns[g.target];
    let tcol = if target.active() {
        Color::new(1.0, 0.45, 0.4, 1.0)
    } else {
        GRAY
    };
    draw_text(format!("Assault: {}", target.name), x, y1, fs, tcol);
    let rescue = if g.rescue.is_some() {
        "Rescue train en route".to_string()
    } else if g.rescue_cd > 0.0 {
        format!("Rescue ready in {:.0}s", g.rescue_cd.ceil())
    } else if g.cash < RESCUE_COST {
        format!("Rescue needs ${RESCUE_COST:.0}")
    } else {
        "SPACE: send rescue train".to_string()
    };
    let rcol = if g.rescue_ready() { GOLD } else { GRAY };
    draw_text(&rescue, x, y2, fs * 0.85, rcol);
    let x = w - 200.0 * k;
    draw_text(
        format!("Speed {:>3.0}", g.loco.speed),
        x,
        y1,
        fs * 0.85,
        WHITE,
    );
    let steer = match run.steer_latch {
        Steer::Straight => "Next: straight",
        Steer::Left => "Next: LEFT",
        Steer::Right => "Next: RIGHT",
    };
    draw_text(steer, x, y2, fs * 0.85, Color::new(1.0, 0.9, 0.3, 1.0));
    if muted {
        draw_text(
            "MUTED (M)",
            w - 120.0 * k,
            screen_height() - 12.0 * k,
            fs * 0.8,
            GRAY,
        );
    }
    if !run.build_mode {
        draw_text(
            "Arrows: drive/steer  Space: rescue  P: build mode  M: mute",
            12.0 * k,
            screen_height() - 12.0 * k,
            fs * 0.75,
            Color::new(1.0, 1.0, 1.0, 0.6),
        );
    }
    if let Some((b, t)) = &run.fx.banner {
        let mut c = WHITE;
        c.a = t.min(1.0);
        text_c(b, screen_height() * 0.22, 40.0 * k, c);
    }
}

fn hovered_town(g: &Game, v: &View) -> Option<usize> {
    let m = v.world(mouse_position().into());
    (0..g.towns.len()).find(|&i| g.towns[i].pos.dist(m) < TOWN_RADIUS + 8.0)
}

fn err_text(e: BuildError) -> &'static str {
    match e {
        BuildError::SameNode => "",
        BuildError::Exists => "Already linked",
        BuildError::TooLong => "Too far (max link length)",
        BuildError::NotConnected => "One end must be on your network",
        BuildError::NoCash => "Not enough cash",
        BuildError::MaxLevel => "Station at max level",
    }
}

fn build_overlay(run: &mut Run, v: &View, audio: &mut Audio) {
    let k = ui_scale();
    let g = &run.game;
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.05, 0.15, 0.25),
    );
    let hover = hovered_town(g, v);
    let mut tip: Option<(String, Color)> = None;
    if let Some(s) = run.selected {
        let a = v.p(g.towns[s].pos);
        draw_circle_lines(a.x, a.y, v.l(TOWN_RADIUS + 6.0), 3.0, GOLD);
        draw_circle_lines(
            a.x,
            a.y,
            v.l(MAX_LINK),
            1.0,
            Color::new(1.0, 1.0, 1.0, 0.25),
        );
        let (end, ok) = match hover {
            Some(h) if h != s => {
                let r = g.can_link(s, h);
                let text = match r {
                    Ok(c) => format!(
                        "Lay track {} -> {}: ${c}{}",
                        g.towns[s].name,
                        g.towns[h].name,
                        if rules::game::crosses_river(g.towns[s].pos, g.towns[h].pos) {
                            " (bridge)"
                        } else {
                            ""
                        }
                    ),
                    Err(e) => format!("${}  {}", g.link_cost(s, h), err_text(e)),
                };
                tip = Some((text, if r.is_ok() { GREEN } else { RED }));
                (v.p(g.towns[h].pos), r.is_ok())
            }
            _ => (mouse_position().into(), true),
        };
        let c = if ok {
            Color::new(0.4, 1.0, 0.4, 0.8)
        } else {
            Color::new(1.0, 0.3, 0.3, 0.8)
        };
        draw_line(a.x, a.y, end.x, end.y, 3.0, c);
    }
    if let Some(h) = hover {
        let t = &g.towns[h];
        let p = v.p(t.pos);
        draw_circle_lines(p.x, p.y, v.l(TOWN_RADIUS + 3.0), 2.0, WHITE);
        if tip.is_none() {
            let up = match g.station_cost(h) {
                Ok(c) if !t.alive => format!("click twice: rebuild ${c}"),
                Ok(c) => format!("click twice: upgrade station ${c}"),
                Err(e) => err_text(e).to_string(),
            };
            let info = if t.station == 0 {
                format!("{} (no station) - link it from your network", t.name)
            } else {
                format!(
                    "{}  pop {}  station Lv{}  militia {}  rating {:.0}%  - {}",
                    t.name,
                    t.pop as u32,
                    t.station,
                    t.militia(),
                    t.rating * 100.0,
                    up
                )
            };
            tip = Some((info, WHITE));
        }
    }
    let y = screen_height() - 44.0 * k;
    draw_rectangle(
        0.0,
        y - 30.0 * k,
        screen_width(),
        80.0 * k,
        Color::new(0.0, 0.0, 0.0, 0.7),
    );
    draw_text(
        "BUILD MODE (paused) - click a town, then another to lay track; click a town twice to upgrade. Right-click: cancel. P: resume",
        12.0 * k,
        y - 8.0 * k,
        20.0 * k,
        Color::new(0.6, 0.85, 1.0, 1.0),
    );
    if let Some((s, c)) = tip {
        draw_text(&s, 12.0 * k, y + 20.0 * k, 24.0 * k, c);
    }

    if is_mouse_button_pressed(MouseButton::Right) || is_key_pressed(KeyCode::Escape) {
        run.selected = None;
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        match (run.selected, hover) {
            (Some(s), Some(h)) if s == h => {
                if run.game.upgrade_station(h).is_ok() {
                    let p = run.game.towns[h].pos;
                    run.fx.burst(p, 25, 140.0, GOLD, 3.0);
                }
                run.selected = None;
            }
            (Some(s), Some(h)) => {
                if run.game.build_link(s, h).is_ok() {
                    let (a, b) = (run.game.towns[s].pos, run.game.towns[h].pos);
                    for i in 0..8 {
                        run.fx.burst(
                            a.lerp(b, i as f32 / 7.0),
                            4,
                            60.0,
                            Color::new(0.7, 0.5, 0.3, 1.0),
                            3.0,
                        );
                    }
                    run.selected = Some(h);
                }
            }
            (None, Some(h)) => {
                run.selected = Some(h);
                audio.play(Sfx::Click);
            }
            (_, None) => run.selected = None,
        }
    }
}

fn level_up_overlay(run: &mut Run, audio: &mut Audio) {
    let Some(choices) = run.game.choice else {
        return;
    };
    let k = ui_scale();
    let (w, h) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    text_c(
        &format!("LEVEL {} - choose an upgrade", run.game.level),
        h * 0.28,
        44.0 * k,
        SKYBLUE,
    );
    let cw = 300.0 * k;
    let ch = 150.0 * k;
    let gap = 24.0 * k;
    let x0 = (w - (cw * 3.0 + gap * 2.0)) * 0.5;
    let y0 = h * 0.38;
    let (mx, my) = mouse_position();
    let mut pick = None;
    for (i, u) in choices.iter().enumerate() {
        let x = x0 + i as f32 * (cw + gap);
        let hover = mx > x && mx < x + cw && my > y0 && my < y0 + ch;
        let bg = if hover {
            Color::new(0.2, 0.3, 0.45, 0.95)
        } else {
            Color::new(0.1, 0.15, 0.25, 0.95)
        };
        draw_rectangle(x, y0, cw, ch, bg);
        draw_rectangle_lines(x, y0, cw, ch, 3.0, SKYBLUE);
        draw_text(
            format!("[{}]", i + 1),
            x + 14.0 * k,
            y0 + 34.0 * k,
            28.0 * k,
            GOLD,
        );
        draw_text(u.name(), x + 60.0 * k, y0 + 34.0 * k, 30.0 * k, WHITE);
        draw_text(
            u.desc(),
            x + 14.0 * k,
            y0 + 90.0 * k,
            24.0 * k,
            Color::new(0.8, 0.9, 1.0, 1.0),
        );
        if hover && is_mouse_button_pressed(MouseButton::Left) {
            pick = Some(i);
        }
    }
    for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
        .into_iter()
        .enumerate()
    {
        if is_key_pressed(key) {
            pick = Some(i);
        }
    }
    if let Some(i) = pick {
        run.game.choose(i);
        audio.play(Sfx::Click);
    }
}

fn game_over_screen(run: &Run, t: f32) {
    let g = &run.game;
    let k = ui_scale();
    let h = screen_height();
    draw_rectangle(0.0, 0.0, screen_width(), h, Color::new(0.0, 0.0, 0.0, 0.65));
    let (title, c) = if g.phase == Phase::Won {
        ("THE TIDE IS BROKEN", GOLD)
    } else {
        ("YOUR COMPANY HAS FALLEN", Color::new(1.0, 0.35, 0.3, 1.0))
    };
    text_c(title, h * 0.3, 72.0 * k, c);
    text_c(
        &format!("Company value: ${}", g.company_value()),
        h * 0.43,
        44.0 * k,
        WHITE,
    );
    let towns = g.towns.iter().filter(|t| t.active()).count();
    text_c(
        &format!(
            "Waves survived: {}   Kills: {}   Level: {}   Towns served: {}",
            if g.phase == Phase::Won {
                WAVES
            } else {
                g.wave - 1
            },
            g.kills,
            g.level,
            towns
        ),
        h * 0.52,
        28.0 * k,
        Color::new(0.85, 0.85, 0.85, 1.0),
    );
    if (t * 2.0).fract() < 0.65 {
        text_c("Press R or Enter to play again", h * 0.68, 34.0 * k, WHITE);
    }
}

fn drain_events(run: &mut Run, audio: &mut Audio) {
    let events = std::mem::take(&mut run.game.events);
    for e in &events {
        audio.on_event(e);
        run.fx.on_event(e, &run.game);
        if *e == Event::Junction {
            run.steer_latch = Steer::Straight;
        }
    }
}

fn input(run: &mut Run) -> Input {
    let left = is_key_down(KeyCode::Left) || is_key_down(KeyCode::A);
    let right = is_key_down(KeyCode::Right) || is_key_down(KeyCode::D);
    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
        run.steer_latch = Steer::Left;
    }
    if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
        run.steer_latch = Steer::Right;
    }
    let steer = if left {
        Steer::Left
    } else if right {
        Steer::Right
    } else {
        run.steer_latch
    };
    let up = is_key_down(KeyCode::Up) || is_key_down(KeyCode::W);
    let down = is_key_down(KeyCode::Down) || is_key_down(KeyCode::S);
    Input {
        steer,
        throttle: if up {
            1.0
        } else if down {
            -1.0
        } else {
            0.0
        },
        reverse: is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S),
        rescue: is_key_pressed(KeyCode::Space),
    }
}

#[macroquad::main(conf)]
async fn main() {
    let mut audio = Audio::new().await;
    let mut screen = Screen::Title;
    let mut seed = 1u64;
    let demo = Game::new(seed);
    let mut run = Run::new(seed);
    loop {
        let t = get_time() as f32;
        let frame = get_frame_time().min(0.1);
        if is_key_pressed(KeyCode::M) {
            audio.toggle_mute();
        }
        match screen {
            Screen::Title => {
                title_screen(t, &demo, audio.muted);
                if any_input() {
                    audio.start_music();
                    audio.play(Sfx::Click);
                    seed = (macroquad::miniquad::date::now() * 1000.0) as u64;
                    run = Run::new(seed);
                    screen = Screen::Playing;
                }
            }
            Screen::Playing | Screen::Over => {
                let playing = matches!(screen, Screen::Playing);
                if playing && is_key_pressed(KeyCode::P) && run.game.choice.is_none() {
                    run.build_mode = !run.build_mode;
                    run.selected = None;
                    audio.play(Sfx::Click);
                }
                let paused = run.build_mode || run.game.choice.is_some() || !playing;
                let inp = input(&mut run);
                if !paused {
                    run.acc += frame;
                    let mut first = true;
                    while run.acc >= DT {
                        run.acc -= DT;
                        let step_input = if first {
                            inp
                        } else {
                            Input {
                                reverse: false,
                                rescue: false,
                                ..inp
                            }
                        };
                        first = false;
                        run.game.update(DT, &step_input);
                        drain_events(&mut run, &mut audio);
                    }
                }
                run.fx.update(if paused && playing { 0.0 } else { frame });
                let v = View::fit(run.fx.shake, t);
                draw::terrain(&v, t);
                let preview = run.game.preview_next(inp.steer);
                draw::network(&v, &run.game, Some(preview));
                draw::towns(&v, &run.game, t);
                draw::actors(&v, &run.game, t);
                draw::loco(&v, &run.game, &mut run.fx, t);
                draw::effects(&v, &run.fx);
                hud(&run, audio.muted);
                if run.build_mode {
                    build_overlay(&mut run, &v, &mut audio);
                    drain_events(&mut run, &mut audio);
                }
                level_up_overlay(&mut run, &mut audio);
                if playing && run.game.phase != Phase::Playing {
                    screen = Screen::Over;
                    run.build_mode = false;
                }
                if !playing {
                    game_over_screen(&run, t);
                    if is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Enter) {
                        seed = seed.wrapping_add(7919);
                        run = Run::new(seed);
                        screen = Screen::Playing;
                        audio.play(Sfx::Click);
                    }
                }
            }
        }
        next_frame().await;
    }
}
