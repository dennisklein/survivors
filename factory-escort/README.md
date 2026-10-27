# Factory Escort

A survivor-like where your army grows as theirs does. Your Commander guards a
stationary Bot Lab that keeps pumping out Pawns and Grunts, which follow you as
a swarm. Lead the blob out to farm salvage (XP), pick upgrades that unlock
Rocko, Hammer and Warrior blueprints, and hurry home when the lab is hit.

Hold out for 10 minutes and beat the three boss walkers to win. You lose if the
Commander or the Bot Lab is destroyed.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move the Commander (it auto-fires its D-Gun laser) |
| Right-click | Set a rally point for the army |
| F | Army follows the Commander again |
| 1-5 | Queue Pawn / Grunt / Rocko / Hammer / Warrior (the last pick auto-repeats) |
| Q / E | Build a Metal Extractor (more metal) / Solar (faster production) near the lab |
| 1-3 or click | Choose an upgrade on level-up |
| M | Mute / unmute |
| R | Restart after game over |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Building and running

Requires [mise](https://mise.jdx.dev).

- `mise run build` builds the browser game into `dist/` (`index.html`, wasm, JS loader, assets)
- `mise run serve` builds and serves it on http://localhost:8080
- `mise run test` runs the rules unit tests
- `mise run run` runs natively (Linux needs `libasound2-dev`)
- `mise run lint` checks formatting and clippy

Game rules live in `crates/rules` (pure, unit tested); `crates/game` is the
macroquad front end.

## Credits

Unit names and roles are inspired by [Beyond All Reason](https://www.beyondallreason.info).
All graphics are drawn procedurally and all sound effects and music are
synthesized in code; see [assets/LICENSES.md](assets/LICENSES.md).
