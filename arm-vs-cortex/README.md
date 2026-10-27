# Arm vs Cortex

A survivor-like for the browser (Rust + macroquad, compiled to wasm32),
themed on [Beyond All Reason](https://www.beyondallreason.info/).

Pick an **Armada** or **Cortex** commander. The opposing faction attacks
from four labs in the map corners while **scavengers** pour in from the
edges and fight everyone, including each other. Capture the enemy labs to
turn them into allied factories and steal their tech: each lab you hold
adds the other faction's weapon to your commander and grows it stronger.
Hold all four labs to win; lose your commander and it is over.

- **Armada**: rapid long-range lasers, light laser towers, fast units.
- **Cortex**: heavy splash plasma, heavy cannon towers, tough units.

Waves arrive every 25 seconds and grow; every 4th wave brings an enemy
commander. Wreck metal from kills funds towers and levels you up (pick one
of three upgrades). Labs you capture can be retaken if they are destroyed.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move (weapons fire automatically) |
| C (hold) | Capture the nearby enemy lab (twice as fast below half HP) |
| B | Build a faction tower at your position (costs metal) |
| 1 / 2 / 3 | Pick faction on the title screen, pick upgrade on level up |
| M | Mute / unmute |
| Enter | Start / return to the title screen |

Sound starts after the first key press (browser autoplay rules).

## Development

Requires [mise](https://mise.jdx.dev/).

```sh
mise run serve   # build to dist/ and serve on http://localhost:8080
mise run build   # wasm build into dist/
mise run test    # unit tests of the pure rules crate
mise run run     # native build (Linux needs libasound2-dev)
mise run lint    # rustfmt check + clippy
```

`crates/sim` holds the deterministic game rules and the sound synthesiser
(unit tested); `crates/game` is the macroquad front end.

## Credits

All graphics are drawn procedurally and all sound effects and music are
synthesised in code; see [assets/LICENSES.md](assets/LICENSES.md).
