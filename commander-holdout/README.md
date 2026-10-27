# Commander Holdout

A survivor-like homage to Beyond All Reason: your Commander drops alone and
drones swarm from every side. Reclaim wrecks for metal, throw down Light
Laser Towers and Solar Collectors right next to you, and clear lines of
enemies with the D-Gun. Survive 20 minutes, then destroy the T3 Korgoth.

## How to play

- Your Commander's laser fires automatically at the nearest drone.
- Kills give experience and leave wrecks; stand near wrecks to reclaim
  their metal. Solar Collectors produce energy.
- Spend metal and energy on buildings placed within the cyan build ring.
- Every level-up offers three upgrades: one for the Commander, one for
  your buildings, and a wildcard.
- Surges arrive every 2.5 minutes, an Elite Juggernaut every 5 minutes, and
  the Korgoth at 20:00. Destroy it to win; lose the Commander and it is over.

| Key | Building | Cost (M/E) | Effect |
| --- | --- | --- | --- |
| 1 | Light Laser Tower | 50 / 60 | fast, short-range laser |
| 2 | Solar Collector | 35 / 0 | +5 energy/s |
| 3 | Heavy Laser Tower | 160 / 300 | slow, long-range heavy laser |
| 4 | Nano Turret | 90 / 150 | repairs Commander and buildings, reclaims wrecks |
| 5 | Energy Converter | 60 / 100 | turns 10 energy/s into 1 metal/s |

## Controls

| Input | Action |
| --- | --- |
| WASD / arrow keys | move |
| Space / right-click | fire the D-Gun towards the mouse |
| 1-5, then left-click | select a building, place it at the cursor |
| hold B + 1-5 | place a building instantly at the cursor |
| Esc / right-click | cancel building selection |
| 1-3 or click | pick a level-up upgrade |
| P | pause |
| M | mute / unmute |
| R | redeploy after victory or defeat |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

Sound starts after the first key press or click (browser autoplay policy).

## Development

Requires [mise](https://mise.jdx.dev/); run from this directory:

```sh
mise run serve   # build to dist/ and serve on http://localhost:8080
mise run build   # wasm build into dist/
mise run test    # unit tests for the pure rules in crates/sim
mise run run     # native build (Linux needs libasound2-dev)
mise run ci      # fmt check, clippy, tests and web build
```

`crates/sim` holds the deterministic game rules and the sound synthesiser;
`crates/game` is the macroquad front end.

## Credits

Code, procedural graphics, sound effects and music: this project (MIT).
Concept and names: homage to Beyond All Reason. See
[assets/LICENSES.md](assets/LICENSES.md) for details.
