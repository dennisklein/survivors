# Scavenger Salvage

A hauling survivor-like. Scavenger drones swarm your Commander; every kill
leaves a wreck. Tow wrecks back to the **Recycler** at the centre of the map:
deposited salvage fills quotas, and each met quota auto-builds the next base
structure (laser towers, a repair pad, radar, a nano tower, flak, an
Annihilator). Towing is slow, and the heavier the haul the slower you get,
so every trip is a gamble against the swarm.

Build the eighth structure, the **Evac Gate**, to win. You lose if the
Commander or the Recycler is destroyed. Waves escalate every 35 seconds,
raiders go for the Recycler when you wander off, and a Scavenger Mothership
arrives every fifth wave.

## How to play

| Input              | Action                                         |
| ------------------ | ---------------------------------------------- |
| WASD / arrows      | Move the Commander (the gun fires by itself)   |
| E / Space          | Grab the nearest wreck, or release your haul   |
| Tab                | Toggle base overview (zoomed out, build list)  |
| 1 / 2 / 3 or click | Pick an upgrade on level-up                    |
| P / Esc            | Pause                                          |
| M                  | Mute / unmute                                  |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

Drag a towed wreck into the Recycler ring to deposit it. Upgrades include
Power Winch (less slowdown), Extra Tow Cable (tow a chain of wrecks),
Spread Shot, Piercing Slugs and Recycler Refit (+25% salvage).

## Tasks (mise)

- `mise run build`: build the browser game into `dist/`
- `mise run serve`: build and serve on http://localhost:8080
- `mise run run`: run natively (Linux needs `libasound2-dev`)
- `mise run test`: unit tests for the rules (`crates/sim`)
- `mise run lint`: rustfmt check and clippy

Rules live in the pure `sim` crate (`crates/sim/src/game.rs`); the
macroquad front end is `crates/game`.

## Credits

Graphics and audio are procedural and MIT licensed; see
[assets/LICENSES.md](assets/LICENSES.md). Inspired by
[Beyond All Reason](https://www.beyondallreason.info/).
