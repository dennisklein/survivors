# Transport Drop

A survivor-like where your Commander rides in an air Transport. In the air
you are untouchable but burn fuel and cannot reclaim; on the ground you
auto-fire and collect metal from wrecks. Spend metal to air-drop structures
anywhere: falling crates crush whatever they land on, and every touchdown
blasts the horde circling below you. Survive 10 escalating waves (40 s each)
and destroy the boss to win.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move (fast in the air, slower on the ground) |
| L / Space | Land or lift off (touchdown shockwave; liftoff needs fuel) |
| 1-4 | Pick crate: Laser Turret, Dragon's Teeth, Metal Extractor, Repair Pad |
| Left click | Air-drop the selected crate at the cursor |
| 1-3 / click | Choose an upgrade on level-up |
| M | Mute / unmute |

Fuel drains while airborne and refills on the ground (faster on a Repair
Pad, which also heals you). Running dry forces a landing.

## Development

```sh
mise run build   # wasm build into dist/ (index.html + wasm + assets)
mise run serve   # build and serve on http://localhost:8080
mise run run     # native (Linux needs libasound2-dev)
mise run test    # unit tests for the rules in crates/sim
mise run lint    # rustfmt + clippy
```

Game rules live in `crates/sim` (pure, unit tested); `crates/game` is the
macroquad front end. Audio starts after the first key press or click.

## Credits

All graphics and sounds are procedural and MIT licensed; see
[assets/LICENSES.md](assets/LICENSES.md). Inspired by Beyond All Reason.
