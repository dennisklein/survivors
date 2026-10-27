# Asset licenses

Wind and Wreck ships **no third-party asset files**.

- **Graphics** are drawn procedurally at runtime (`crates/game/src/main.rs`).
- **Sound effects and music** (laser, shield, EMP crackle, hits, explosions,
  pickups, building, level-up, brownout, wave alarm, game over, victory and a
  looping synth track with a wind bed) are synthesised at startup as 16-bit
  PCM WAV data (`crates/sim/src/synth.rs`) and decoded by the browser. They
  are part of this project's code and MIT licensed.
- `web/mq_js_bundle.js` is the macroquad/miniquad JS loader including the
  `macroquad_audio`, `sapp_jsutils` and `quad_net` plugins, (c) the macroquad
  and miniquad authors, MIT OR Apache-2.0 (patched so `quad_net` registers
  under strict mode).

Unit names (Wind Turbine, Solar Collector, Energy Storage, Light Laser Tower)
and the setting are an homage to Beyond All Reason. BAR's unit pictures and
icons are CC BY-NC-ND 4.0 and its sounds are all rights reserved, so they are
deliberately **not** redistributed here; the game evokes them with original
procedural art and audio instead.
