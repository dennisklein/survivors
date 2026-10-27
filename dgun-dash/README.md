# D-Gun Dash

A fast arcade survivor-like in the spirit of Beyond All Reason. Your
Commander has no base: kite the swarm, seed temporary mines, pop-up turrets
and EMP fields from your inventory, chain kills to recharge the D-Gun and
blast a line through everything. Survive 10 minutes for the best score.

## How to play

- The laser auto-fires at the nearest enemy.
- Every kill extends your **chain** (2 s window). Chains multiply score (up
  to x5) and feed the D-Gun meter faster; a full meter adds a D-Gun charge.
- The **D-Gun** destroys every normal enemy and enemy shot in its path and
  takes a big bite out of Juggernaut bosses (at 2:30, 5:30 and 8:00).
- Enemies drop XP, structure crates, repair kits and D-Gun cells.
- Level up to pick one of three upgrades. Surges hit every minute.
- Win by surviving 10:00 (remaining HP is a score bonus); lose at 0 HP.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Space / right click | Fire the D-Gun toward the mouse |
| 1 / 2 / 3 | Drop a mine / pop-up turret / EMP field (also picks level-up cards) |
| P / Esc | Pause |
| M | Mute |
| R / Enter, T | Restart, back to title (after a run) |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Development

Requires [mise](https://mise.jdx.dev/). From this directory:

```sh
mise run build   # wasm build into dist/ (index.html, wasm, JS loader, assets)
mise run serve   # build and serve on http://localhost:8080
mise run test    # unit tests for the pure rules in crates/sim
mise run run     # native build (Linux needs libasound2-dev)
mise run lint    # rustfmt check + clippy
```

Game rules live in `crates/sim` (pure, deterministic, unit tested); the
macroquad front end is `crates/game`. Audio starts after the first key or
click on the title screen, as browsers require.

## Credits

All graphics and sound are procedural (see `assets/LICENSES.md`). Theme
inspired by [Beyond All Reason](https://www.beyondallreason.info/); no BAR
assets are included. Built with [macroquad](https://macroquad.rs/). MIT.
