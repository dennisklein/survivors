# Mex Rush

A territory-control survivor. Metal spots dot a large map; every Metal
Extractor (mex) you capture raises your income but also becomes a target.
Raiders prefer your mexes over you, so you dash between them defending your
economy. Income buys global upgrades; kills drop XP for commander level-ups.
Survive 10:00 to extract (a Juggernaut arrives at 9:00). Die, or lose every
extractor for 20 seconds (starvation), and the run is over.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move (weapon fires automatically) |
| E / Space | Build a mex on the metal spot you stand on (first one free) |
| R | Fortify the mex with a Light Laser Tower, then upgrade it (3 levels) |
| 1-4 | Buy global upgrades: mex output, mex armor, LLT damage, repair all |
| 1-3 | Pick a card on level-up |
| Tab / N | Toggle minimap |
| M | Mute / unmute sound and music |

`M` is used for mute (the sound toggle shared by all games in this repo), so
the minimap lives on Tab / N.

## Development

```sh
mise run build   # dist/index.html + mex-rush.wasm + loader
mise run serve   # http://localhost:8080
mise run test    # unit tests for the rules in crates/sim
mise run run     # native build (needs libasound2-dev on Linux)
mise run lint
```

`crates/sim` holds the pure game rules (`game.rs`) and the sound
synthesiser (`synth.rs`); `crates/game` is the macroquad front end.
Audio starts after the first key press on the title screen.

## Credits

All graphics are drawn procedurally and all sound effects and music are
synthesised in code; see [assets/LICENSES.md](assets/LICENSES.md).
Theme and unit names are an homage to Beyond All Reason.
