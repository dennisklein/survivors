# Tech Tree Tower

A survivor-like where every level-up is a building. Your Commander auto-fires
at the swarm; wrecks give metal (XP); each level offers three cards, mostly
Beyond All Reason-style structures. Place them on the base grid next to the
right neighbours and the synergies carry you through 20 escalating waves.

## How to play

- Survive 20 waves (10 minutes). Every 5th wave a Juggernaut boss arrives.
- You lose if the Commander dies or the Command Core is destroyed.
- New buildings must touch the existing base. Synergies (4-adjacent):
  - Solar / Wind / Fusion (energy) next to a turret: +30% fire rate per energy.
  - Energy or Wind next to a Metal Extractor: x1.5 income each; Storage: x2.
  - Nano Turret next to a turret: +40% damage; Nanos also repair nearby
    buildings and the Commander.
  - Radar next to a turret: +30% range.
  - Dragon's Teeth (3 cells) next to a turret: turret takes half damage.
    Walls also block enemies.
  - Fusion (2 cells) is energy 3 but explodes when destroyed.
- Tanks and bosses siege the base; everything else hunts the Commander.

## Controls

| Input | Action |
| --- | --- |
| WASD / arrows | Move the Commander (fires automatically) |
| Mouse / 1-3 | Pick a level-up card |
| Mouse click | Place the building |
| R / right click | Rotate the building |
| P / Esc | Pause |
| M | Mute / unmute |

Audio starts after the first key press or click (browser autoplay rules).

## Build and run

Requires [mise](https://mise.jdx.dev/).

```sh
mise run build   # wasm build into dist/ (index.html, wasm, loader, assets)
mise run serve   # build and serve on http://localhost:8080
mise run test    # unit tests for the rules and sound synthesis
mise run run     # native build (Linux needs libasound2-dev)
mise run lint    # rustfmt check + clippy
```

Layout: `crates/sim` holds the pure, unit-tested rules (`game.rs`) and the
procedural sound synthesis (`synth.rs`); `crates/game` is the macroquad front
end.

## Asset credits

All graphics are drawn procedurally and all sound effects and music are
synthesised at startup, so no third-party media is shipped. The setting is an
homage to [Beyond All Reason](https://www.beyondallreason.info/). See
[assets/LICENSES.md](assets/LICENSES.md) for details, including the
macroquad JS loader (MIT OR Apache-2.0).
