# Shield Bubble

A survivor-like where your base lives inside a shield dome. Outside the bubble
the world is lethal artillery fog. Venture out for metal, return to unload it,
build generators to grow the dome and hold out for 20 waves.

## How to play

- Mine metal from deposits and wrecks outside the dome; it fills your cargo.
- Step back inside the dome to unload cargo and heal.
- Generators grow the dome and raise shield capacity and regen; buildings can
  only be placed inside the dome.
- Enemies drain the shield on contact and artillery shells it. When the shield
  is down, they hit your buildings. Lose the HQ or the commander and it's over.
- Kills grant XP; each level offers three upgrades.
- Survive 20 waves (10 minutes) and defeat any remaining Juggernauts to win.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move (weapons fire automatically) |
| B | Build generator (cost rises per generator) |
| T | Build turret |
| H | Return-to-base beacon (short channel, cooldown) |
| 1 / 2 / 3 | Pick a level-up upgrade |
| M | Mute / unmute |
| Enter | Restart after game over |

## Development

```sh
mise run build   # wasm build into dist/
mise run serve   # build and serve on http://localhost:8080
mise run test    # unit tests for the game rules (crates/sim)
mise run run     # native build (needs libasound2-dev on Linux)
```

## Assets

Graphics and audio are generated procedurally; see
[assets/LICENSES.md](assets/LICENSES.md).
