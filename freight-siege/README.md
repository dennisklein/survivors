# Freight Siege

A survivor-like about running a small rail company on a map overrun by
rust-beasts. Stations you connect with routes earn cash; every wagon on a
train is a turret that shoots the swarms as it passes. The world pauses while
you plan, so building is calm and combat is a spectacle. Survive 10 years and
keep the network solvent.

## Play

```sh
mise run serve   # build the wasm bundle and serve http://localhost:8080
mise run run     # native build (Linux needs libasound2-dev)
mise run test    # unit tests for the rules and the sound synth
mise run build   # dist/index.html + dist/freight-siege.wasm + JS loader
```

## How to play

- The game starts **paused** (planning mode). Build, then press **Space** to run.
- Trains shuttle between their two stations. Each delivery pays cash based on
  route length and the **origin station's cargo**:
  - **Coal**: x1.5 cash. **Goods**: turret overclock for 12 s. **Mail**: bonus XP.
- Wagons are turrets: Gatling (fast), Cannon (splash), Tesla (chains),
  Mortar (long range, big splash). Up to 8 per train.
- Rust-beasts swarm the **most profitable station** (red ring) and chew track
  on the way. Broken track (red X) stops trains until repaired.
- Killed beasts drop scrap; trains passing nearby collect it for XP and a
  little cash. Level-ups offer a choice of three upgrades.
- Each year lasts 60 s and starts with a bigger wave; bosses arrive in years
  5, 8 and 10. At year end upkeep is charged (track, stations, wagons) and the
  world pauses for planning. Negative cash = bankrupt. Losing every station = lost.

## Controls

| Input | Action |
| --- | --- |
| Space | Toggle pause / planning |
| Click empty tile (paused) | Build station ($120) |
| Drag from station to station (paused) | Lay track and open a line |
| Click station | Cycle cargo (Coal / Goods / Mail) |
| Click track / Tab | Select line that receives new wagons |
| 1-4 | Buy Gatling / Cannon / Tesla / Mortar wagon |
| R (paused) | Repair all track and stations |
| M | Mute / unmute |
| Esc | Menu (resume, restart, quit to title) |

## Assets and credits

- All graphics are drawn procedurally with macroquad shapes.
- All sound effects and the music loop are synthesized at startup in
  `src/synth.rs` (16-bit PCM WAV generated in Rust). No external audio files.
- `web/mq_js_bundle.js` is the macroquad JS loader (MIT/Apache-2.0) including
  its `macroquad_audio` plugin, with a one-line strict-mode fix in the
  `quad_net` plugin registration.

## Layout

- `src/rules.rs`: pure game rules (no rendering/audio), unit tested.
- `src/synth.rs`: procedural sound generation, unit tested.
- `src/main.rs`: macroquad front end (input, drawing, HUD, audio playback).
- `.cargo/config.toml`: lets the web audio imports link on wasm32.
