# Asset licenses

Mex Rush ships **no third-party asset files**.

- **Graphics** are drawn procedurally at runtime (`crates/game/src/main.rs`).
- **Sound effects and music** are synthesised at startup as 16-bit PCM WAV
  data (`crates/sim/src/synth.rs`) and decoded by the browser's
  `decodeAudioData`. They are part of this project's code and MIT licensed.
- `web/mq_js_bundle.js` is the macroquad/miniquad JS loader with the
  `macroquad_audio` plugin, (c) the macroquad and miniquad authors,
  MIT OR Apache-2.0.

The theme and names (Metal Extractor, Light Laser Tower, Commander) are an
homage to [Beyond All Reason](https://www.beyondallreason.info/). BAR's unit
pictures and icons are CC BY-NC-ND 4.0 and its sounds are all rights
reserved, so they are deliberately **not** redistributed in this public
repository.
