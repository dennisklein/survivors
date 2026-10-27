# Asset licenses

Transport Drop ships **no third-party asset files**.

- **Graphics** (Commander, Transport, crates, structures, enemies, effects)
  are drawn procedurally at runtime (`crates/game/src/main.rs`).
- **Sound effects and music** (plasma shots, turret lasers, hits, explosions,
  reclaim pickups, level-up, drop beacon, crate impact, touchdown, liftoff,
  wave and boss sirens, game over, victory and a looping synth track) are
  synthesised at startup as 16-bit PCM WAV data (`crates/sim/src/synth.rs`)
  and decoded by the browser. They are part of this project's code and MIT
  licensed.
- `web/mq_js_bundle.js` is the macroquad/miniquad JS loader including the
  `macroquad_audio`, `sapp_jsutils` and `quad_net` plugins, (c) the macroquad
  and miniquad authors, MIT OR Apache-2.0.

The setting (Commander, air Transport, Dragon's Teeth, Metal Extractors,
reclaiming wrecks) is an homage to Beyond All Reason
(https://www.beyondallreason.info/). BAR's unit pictures and icons are
CC BY-NC-ND 4.0 and its sounds are all rights reserved, so they are
deliberately **not** redistributed in this public repository; the game evokes
them with original procedural art and audio instead.
