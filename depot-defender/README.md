# Depot Defender

A survivor-like about trains. You are the depot master, defending a central
rail depot against swarms of scrap creatures. Supply trains arrive on a
timetable carrying upgrade crates; unload them before they leave, and between
waves lease new lines to towns that pay in steel, coal or goods. Each cargo
type powers its own weapon family.

## How to play

- Walk around; your weapons fire automatically at the nearest enemies.
- When a train docks, stand inside the ring at its platform and press **E** to
  unload. Crates spill onto the platform: walk over them for XP, cash and
  cargo. If you are too slow the train leaves and the crates are lost.
- The first crate of a new cargo unlocks its weapon:
  - **Steel** - Rivet Gun (piercing shots, more barrels at higher levels)
  - **Coal** - Firebox Mortar (area explosions)
  - **Goods** - Parcel Orbit (crates circling you)
- Scrapped enemies drop XP gems. Each level up offers three upgrades.
- After each wave the network map opens: click a town to lease its line, or
  extend an existing line for bigger, more frequent trains.
- Waves 5 and 10 bring a boss. Survive wave 10 and derail its boss to win.
  You lose if your health or the depot's health reaches zero.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| E | Unload a docked train |
| Tab | Network map (pauses the wave) |
| Mouse, 1-3 | Pick upgrades, lease and extend lines |
| Space / Enter | Depart for the next wave |
| P / Esc | Pause |
| M | Mute / unmute |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Development

Requires [mise](https://mise.jdx.dev). All commands run from this directory.

```sh
mise run build   # wasm build into dist/ (index.html, depot-defender.wasm, mq_js_bundle.js)
mise run serve   # build and serve on http://localhost:8080
mise run test    # unit tests
mise run lint    # rustfmt + clippy
mise run run     # native build (needs libasound2-dev on Linux)
```

Audio needs macroquad's `audio` feature and `--allow-undefined` at link time
(see `.cargo/config.toml`) because the audio imports are provided by the JS
loader. `crates/rules` holds the pure, unit-tested game rules and the sound
synthesizer; `crates/game` is the macroquad front end.

## Credits

- Code, vector art and all sounds and music: original, generated procedurally
  at startup (see `crates/rules/src/synth.rs`). MIT licensed.
- `web/mq_js_bundle.js`: the JavaScript loader of
  [macroquad](https://github.com/not-fl3/macroquad) (MIT/Apache-2.0), including
  its audio plugin (one-line fix: `register_plugin` is declared in the
  quad_net block so the bundle runs in strict mode without errors).
