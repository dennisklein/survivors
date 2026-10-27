# Corridor Commander

A shmup-flavoured survivor-like: the battlefront scrolls forward on its own and
your forward base has to be rebuilt as you advance. Turrets you leave behind
become flank defenses against enemies sneaking in from the rear, until they
scroll off the map. Three gates block the corridor; each is guarded by a boss
and the front halts until it falls. Clear all three to win.

## How to play

| Input          | Action                                           |
| -------------- | ------------------------------------------------ |
| WASD / arrows  | Move (inside the scrolling window)               |
| (automatic)    | Your guns fire at the nearest enemy              |
| Left click     | Build a turret within your build radius (scrap)  |
| Space          | Dash with brief invulnerability                  |
| 1 / 2 / 3      | Pick an upgrade on level-up (or click a card)    |
| M              | Mute / unmute                                    |

Killed enemies drop salvage: green for XP and scrap, gold for big wrecks.
Waves grow denser the further you push, and more of them flank from behind.

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
