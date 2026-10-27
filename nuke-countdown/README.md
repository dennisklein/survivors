# Nuke Countdown

A survivor-like built around a Nuclear Silo. Gather metal and energy, feed the
silo and defend it from the horde. Once armed, launch: the strike wipes the
map and starts a harder cycle. Meanwhile the enemy Juggernaut marches on the
silo when its countdown expires, and after your first strike the enemy fires
nukes back - only an Anti-Nuke can intercept them.

## How to play

- Stand on metal deposits to mine; kills drop XP, metal and energy.
- Carry cargo to the silo and press E to deposit it.
- When the silo has enough metal and energy it is armed: press N to launch.
- Each strike raises the next silo requirement, spawn rate and enemy health,
  and shortens the Juggernaut countdown.
- Build (paid from carried metal) at your position: LLT turrets, Solar
  Collectors (trickle energy into the silo) and Anti-Nukes (intercept one
  enemy nuke per charge).
- Land 5 strikes to win. Lose the silo or your commander and it's over.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move (weapons fire automatically) |
| E | Deposit cargo at the silo |
| N | Launch the nuke when armed |
| B, then 1 / 2 / 3 | Build turret / solar / anti-nuke |
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

Graphics and audio (effects and music) are generated procedurally; see
[assets/LICENSES.md](assets/LICENSES.md).
