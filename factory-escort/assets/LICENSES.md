# Asset licenses

Factory Escort ships no third-party art or audio files.

- Graphics: drawn procedurally with macroquad vector primitives in
  `crates/game/src/main.rs`. Unit names (Pawn, Grunt, Rocko, Hammer, Warrior,
  Bot Lab, Metal Extractor, Solar) are inspired by Beyond All Reason
  (https://www.beyondallreason.info), but no BAR files are included: BAR unit
  pictures are CC BY-NC-ND 4.0 and its sound effects are all rights reserved,
  which forbids redistributing or adapting them here.
- Sound effects (shots, hits, explosions, pickups, level-up, building, alarms,
  game over, victory) and the music loop: synthesized at startup as WAV data by
  `crates/rules/src/synth.rs` (MIT, part of this project).
- `web/mq_js_bundle.js`: macroquad JavaScript loader with the audio plugin,
  (c) Fedor Logachev and contributors, MIT OR Apache-2.0
  (https://github.com/not-fl3/macroquad).
