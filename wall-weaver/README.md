# Wall Weaver

A maze-building survivor-like. Swarms pour in through four gates and march on
your core. Hold Shift while you walk and your Commander leaves a trail that
solidifies into Dragon's Teeth walls. Every wall changes the route: enemies
re-path live along the shortest way to the core, so you can stretch their walk
into long corridors and line the exits with turrets. Walls can bend the path
but never seal it; a trail segment that would close the maze crumbles and is
refunded.

Survive 20 escalating waves (a boss every fifth wave) to win. You lose if the
core or the Commander is destroyed.

## How to play

| Input                 | Action                                              |
| --------------------- | --------------------------------------------------- |
| WASD / arrows         | Move the Commander                                  |
| Hold Shift            | Lay wall trail under you (1 scrap per tile)         |
| Left click            | Build a turret on a tile / upgrade a turret (max 3) |
| X or right click      | Demolish the wall or turret under the mouse (refund)|
| 1 / 2 / 3 or click    | Pick an upgrade on level-up                         |
| M                     | Mute / unmute                                       |

Your Commander's gun fires automatically. Kills drop salvage: walk over it for
scrap (to build) and XP (to level up). Turrets can be built on top of walls.

## Tasks (mise)

- `mise run build`: build the browser game into `dist/`
- `mise run serve`: build and serve on http://localhost:8080
- `mise run run`: run natively (needs `libasound2-dev` on Linux)
- `mise run test` / `mise run lint`

Game rules live in `crates/sim` (pure, unit tested); `crates/game` is the
macroquad front end. Audio starts after the first key press or click.

## Credits

Graphics are drawn procedurally and all sound effects and music are
synthesised at startup (`crates/sim/src/synth.rs`). The theme is an homage to
[Beyond All Reason](https://www.beyondallreason.info/); see
[assets/LICENSES.md](assets/LICENSES.md) for why no BAR files are shipped.
