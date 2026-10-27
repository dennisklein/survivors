# Nano Fortress

A survivor-like where you cannot attack. Your Commander is a builder: drop
blueprints and stand next to them so the nano beam constructs them while the
swarm chews on your walls. Buildpower is your DPS.

## How to play

- Click to place a blueprint of the selected building. Metal is drained while
  it is built, and only while the Commander (or a Nano Turret) is in range.
- **Laser Tower** and **Heavy Laser** shoot automatically, **Dragon Walls**
  soak damage, **Metal Extractors** raise income and **Nano Turrets** add
  buildpower: they assist construction, repair damaged buildings and patch up
  the Commander.
- Hold **Shift** to make the Commander repair damaged buildings first.
- Destroyed enemies drop metal; walk over it to collect. Kills give XP, and each
  level lets you pick one of three upgrades.
- Survive 12 escalating waves and destroy the Korgoth that arrives in the final
  wave. If the Commander dies, the game is over.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Left click | Place blueprint |
| Q / E, 1-5 | Select building |
| Shift (hold) | Repair priority |
| 1-3 / click | Pick upgrade on level-up |
| M | Mute / unmute |
| P / Esc | Pause |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Build and run

Requires [mise](https://mise.jdx.dev).

```sh
mise run build   # browser build into dist/ (index.html, wasm, loader, assets)
mise run serve   # build and serve on http://localhost:8080
mise run run     # native build (needs libasound2-dev on Linux)
mise run test    # unit tests for the rules crate
mise run lint    # rustfmt check + clippy
```

`crates/rules` holds the game rules and sound synthesis as pure, unit-tested
code; `crates/game` is the macroquad front end. Audio starts after the first
key press or click.

## Assets and credits

All graphics are drawn procedurally and every sound effect and the music loop
are synthesized in code. Building and enemy names nod to Beyond All Reason,
whose art (CC BY-NC-ND) and sounds (all rights reserved) cannot be
redistributed here, so none are included. See
[assets/LICENSES.md](assets/LICENSES.md). Licensed under MIT.
