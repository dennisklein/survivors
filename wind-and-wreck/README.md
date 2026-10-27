# Wind and Wreck

A survivor-like where **energy is everything**. Your commander's laser and
shield run off a power grid you build in the field: wind turbines swing with
the gusts shown on screen, solar collectors are steady but fragile, storage
banks buffer the difference. EMP bots drain your grid and smash generators.
Keep the lights on for 12 waves, then destroy the Overload Core to win.

## Play

```sh
mise run serve   # build the wasm bundle and open http://localhost:8080
mise run run     # native (Linux needs libasound2-dev)
mise run test    # unit tests for the rules in crates/sim
mise run build   # writes dist/index.html, wind-and-wreck.wasm, loader, assets
```

## Controls

| Input | Action |
| --- | --- |
| WASD / arrows | Move |
| Mouse | Aim |
| LMB | Fire laser (costs energy per shot) |
| RMB / Space | Shield (drains energy per second, blocks most damage and EMP drain) |
| B | Toggle build menu; LMB places within the ring |
| 1-4 | Pick Wind / Solar / Storage / Laser Tower |
| 1-3 or click | Choose a level-up upgrade |
| M | Mute / unmute |
| P / Esc | Pause |

Kill bots to collect scrap: it pays for buildings and fills your XP bar.
Watch the wind forecast in the HUD: build solar when lulls are coming, and
storage to bank the gusts.

## Layout

- `crates/sim`: pure rules (grid, weapons, waves, upgrades) and procedural
  sound synthesis, all unit tested.
- `crates/game`: macroquad front end (input, drawing, audio playback).
- `web/`: page and the macroquad JS loader with the audio plugin.

## Credits

Theme homage to [Beyond All Reason](https://www.beyondallreason.info/).
All graphics and audio are original and generated in code; see
[assets/LICENSES.md](assets/LICENSES.md).
