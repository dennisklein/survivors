# Tycoon Tide

A railway city-builder / survivor-like hybrid. Your armoured HQ locomotive can
only drive on the rails you own. Serve towns to earn profit and grow them;
bigger towns field militia that fight the tide alongside you. Every wave
assaults one town - dispatch the rescue train to rally its defenders. Pause to
lay new track, bridge the river and upgrade stations. Survive 25 waves; your
score is the company value.

## How to play

- The locomotive auto-fires at the nearest enemy in range (faint ring).
- Passing through a station collects income (more for towns that waited
  longer), raises the town's service rating and patches up your HQ.
- Towns with a good rating (green bar) grow; every 18 inhabitants adds a
  militia defender. Neglected towns shrink.
- Each wave targets one town (red ring). If its hit points run out it falls;
  ruined stations can be rebuilt in build mode.
- Kills give XP; on level-up pick one of three upgrades. Enemies drop coins
  that your coin magnet pulls in.
- Every fifth wave brings a boss. You lose when the HQ is destroyed or every
  town has fallen; you win after wave 25.

## Controls

| Key | Action |
| --- | --- |
| Up / W | Accelerate |
| Down / S | Brake; press again when stopped to reverse |
| Left / Right (A / D) | Pick the branch at the next junction (highlighted) |
| Space | Send the rescue train to the assaulted town ($60, cooldown) |
| P | Pause & build mode: click a town, then another to lay track; click a town twice to upgrade or rebuild its station; right-click cancels |
| 1 / 2 / 3 or click | Choose a level-up upgrade |
| M | Mute / unmute |
| R / Enter | Restart after game over |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Development

Requires [mise](https://mise.jdx.dev/). On Linux, native runs need ALSA
headers (`libasound2-dev`).

```sh
mise run build   # wasm build into dist/ (index.html, tycoon-tide.wasm, mq_js_bundle.js)
mise run serve   # build and serve on http://localhost:8080
mise run run     # native build
mise run test    # unit tests for the rules crate
mise run lint    # rustfmt check + clippy
```

`crates/rules` holds all game logic and sound synthesis as pure, unit-tested
code; `crates/game` is the macroquad front end.

## Assets and credits

All graphics are drawn procedurally with vector primitives, and every sound
effect and the music loop are synthesized in code (`crates/rules/src/synth.rs`).
No third-party art or audio is used. `web/mq_js_bundle.js` is the macroquad
JavaScript loader (MIT/Apache-2.0, not-fl3/macroquad) including its audio
plugin. Licensed under MIT.
