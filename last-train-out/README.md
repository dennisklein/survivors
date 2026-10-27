# Last Train Out

A survivor-like on rails. An armoured evacuation train circles a fixed loop
line while the horde closes in. You are the conductor: walk the carriages, man
the gun cars, stop at stations to pick up refugees and spend their fares on
new carriages. Hold out for 15 minutes and the last train departs.

## How to play

- The train runs on its own; you set the throttle. Running fast ploughs
  through the horde (ramming hurts them and the engine), stopping is risky.
- Stop (throttle 0) inside the yellow markers of a station. Passengers who
  boarded elsewhere get off and pay their fare, then waiting refugees board
  into free coach seats.
- At a station open the yard (B) to buy carriages, repair the whole train and
  shunt your current carriage forward/back. Carriages buff their neighbours:
  - Gun car: auto turret; when you stand in it and fire, it aims at the
    cursor and fires much faster.
  - Flak car: slow exploding shells for crowds.
  - Coach: 6 seats. Infirmary next to it: +2 seats and fares x1.5.
  - Workshop: repairs itself and both neighbours.
  - Coal tender: neighbouring guns fire 35% faster, +8% top speed.
- Standing in a carriage slowly repairs it. Lose a carriage and its
  passengers are lost; lose the engine and the line is closed.
- Kills give XP; each level offers three upgrades. Coins dropped by the horde
  are collected by passing carriages. Every minute a surge rings the train and
  every three minutes a Juggernaut arrives.

## Controls

| Key | Action |
| --- | --- |
| A / D | walk toward the engine / the tail |
| W / S | jump one carriage forward / back |
| Space or left mouse | fire the manned gun (aim with the mouse) |
| Q / E | throttle down / up |
| B | yard menu at a station (1-6 buy, R repair, Z/X shunt) |
| 1-3 or click | pick a level-up upgrade |
| M | mute |
| P / Esc | pause |

## Building and running

Tools are pinned in `mise.toml` (Rust with the wasm32 target, miniserve).

```sh
mise run build   # wasm build into dist/ (index.html, last-train-out.wasm, mq_js_bundle.js)
mise run serve   # build and serve on http://localhost:8080
mise run run     # native build (needs libasound2-dev on Linux)
mise run test    # unit tests for the rules crate
mise run lint    # rustfmt check and clippy -D warnings
```

Layout: `crates/rules` holds the pure, deterministic game rules and sound
synthesis (unit tested); `crates/game` is the macroquad front end. This
directory is its own Cargo workspace, separate from the repository root.

Audio needs macroquad's `audio` feature and `--allow-undefined` at link time
(see `.cargo/config.toml`) because the audio imports are provided by the JS
loader. The browser only starts audio after the first key press or click,
which is what the title screen waits for.

## Credits

- All graphics are drawn procedurally with vector primitives at runtime.
- All sound effects and the music loop are synthesized in code
  (`crates/rules/src/synth.rs`) and generated as WAV data at startup; no
  audio files are shipped.
- `web/mq_js_bundle.js`: the JavaScript loader of
  [macroquad](https://github.com/not-fl3/macroquad) (MIT/Apache-2.0) including
  its audio plugin, with the one-line strict-mode fix also used by
  `depot-defender`.
- Built with macroquad 0.4.16.
