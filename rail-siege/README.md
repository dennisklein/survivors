# Rail Siege

A survivors game in the style of Transport Tycoon. You are the railway
company: you do not drive. Build lines with the mouse from the industries
to the town. Your trains run the lines on their own, carry cargo to the
town and shoot the horde on the way. Delivered cargo pays money and XP.
Money builds more lines, XP gives cards.

Keep the town alive for 15 minutes, then shoot down the **Large UFO** to
win. The run ends when the town has no HP left.

- **Far means rich.** Far industries make more cargo and pay more per
  tile, but they stand near the map edge, where the horde comes in.
- **Weapons are company technology.** Every train carries a wagon of each
  weapon you own, so each new line adds firepower.
- **The horde attacks the whole company.** Residents walk to the town,
  saboteurs break track, rival buses charge trains, UFOs hunt them. Six
  enemies near a station stop the industry.

See [docs/design.html](docs/design.html) for the full design.

## How to play

| Input                    | Action                                          |
| ------------------------ | ----------------------------------------------- |
| Click a station          | Start a line (time runs at 25 %)                |
| Click grass              | Add a waypoint while a line is started          |
| Click a second station   | Build the line and its train                    |
| Esc or right click       | Cancel the line, close the train panel          |
| Click broken track       | Repair it ($10)                                 |
| Click a train            | Train panel: income and remove line (50 % back) |
| Space or P               | Pause; you can build while paused               |
| WASD or arrows           | Move the camera (near zoom)                     |
| Mouse wheel or + / -     | Zoom: whole map or near                         |
| 1 2 3 or click           | Pick a card                                     |
| M                        | Sound on or off                                 |

Build the same line twice for a second train. New lines join the track
you have, so shared track costs nothing. Two trains on one industry share
its output.

## Tasks (mise)

- `mise run build`: build the browser game into `dist/`
- `mise run serve`: build and serve on http://localhost:8080
- `mise run run`: run natively (Linux needs `libasound2-dev`)
- `mise run test`: unit tests for the rules (`crates/sim`)
- `mise run lint`: rustfmt check and clippy

All rules live in the pure `sim` crate: map and stations (`station.rs`),
route finder (`route.rs`), trains (`train.rs`), enemies and spawns
(`enemy.rs`, `director.rs`), weapons and cards (`weapon.rs`, `cards.rs`)
and the fixed-step world (`world.rs`). The macroquad front end is
`crates/game`.

## Credits

Graphics are drawn at runtime. Sound effects and music are synthesized at
startup as WAV data (`crates/sim/src/synth.rs`); the game ships no asset
files. All of it is MIT licensed. `web/mq_js_bundle.js` is the
macroquad/miniquad JS loader with the audio plugin, (c) the macroquad and
miniquad authors, MIT OR Apache-2.0.
