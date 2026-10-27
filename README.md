# Transport Survivors

A single-player survivors game for the browser. You drive a train that lays its
own track. Wagons are weapons. Survive ten minutes and beat the boss.

See [docs/design.html](docs/design.html) for the game design.

## Play

```sh
mise install      # Rust, wasm target, miniserve, watchexec
mise run serve    # build for the browser, open http://localhost:8080
mise run run      # or run natively
```

## Games

Each subdirectory is a standalone game (Rust + macroquad, wasm) with its own
`mise.toml`, README and generated sound. "BAR" games are themed on
[Beyond All Reason](https://www.beyondallreason.info/). Run one with
`cd <dir> && mise run serve` (http://localhost:8080); build all with
`mise run build-all`.

| Dir | Title | Theme | Pitch |
|---|---|---|---|
| [`freight-siege/`](freight-siege/) | Freight Siege | Trains | Connect stations with routes; every wagon is a turret. Plan paused, survive 10 years. |
| [`depot-defender/`](depot-defender/) | Depot Defender | Trains | Defend a rail depot; unload timetabled supply trains for upgrades. |
| [`cargo-cascade/`](cargo-cascade/) | Cargo Cascade | Trains | Feed the coal-power-factory-town chain; supplied towns light defensive auras. |
| [`last-train-out/`](last-train-out/) | Last Train Out | Trains | Conductor of an armoured evacuation train circling a loop; hold out 15 minutes. |
| [`tycoon-tide/`](tycoon-tide/) | Tycoon Tide | Trains | Railway city-builder: grow towns, rally militia, drive your HQ loco on owned rails. |
| [`commander-holdout/`](commander-holdout/) | Commander Holdout | BAR | Reclaim, build towers next to you, D-Gun the swarm; survive 20 minutes. |
| [`nano-fortress/`](nano-fortress/) | Nano Fortress | BAR | No weapon: your nano beam builds the defences. Buildpower is your DPS. |
| [`mex-rush/`](mex-rush/) | Mex Rush | BAR | Capture metal extractors and dash between them as raiders hunt your economy. |
| [`factory-escort/`](factory-escort/) | Factory Escort | BAR | A Bot Lab pumps out a swarm that follows you; protect the lab. |
| [`wind-and-wreck/`](wind-and-wreck/) | Wind and Wreck | BAR | Laser and shield run off a field-built grid of wind, solar and storage. |
| [`dgun-dash/`](dgun-dash/) | D-Gun Dash | BAR | Baseless arcade kiting with mines, pop-up turrets and kill-charged D-Gun. |
| [`raptor-nest/`](raptor-nest/) | Raptor Nest | BAR | Raptor-mode tower defence: build phase, then fight waves from six nests. |
| [`shield-bubble/`](shield-bubble/) | Shield Bubble | BAR | Base inside a shield dome; venture into lethal fog for metal, grow the dome. |
| [`tech-tree-tower/`](tech-tree-tower/) | Tech Tree Tower | BAR | Every level-up is a building card; place for neighbour synergies. |
| [`nuke-countdown/`](nuke-countdown/) | Nuke Countdown | BAR | Feed and defend a Nuclear Silo; each launch wipes the map and raises the stakes. |
| [`corridor-commander/`](corridor-commander/) | Corridor Commander | BAR | Auto-scrolling front: rebuild your forward base and break three boss gates. |
| [`transport-drop/`](transport-drop/) | Transport Drop | BAR | Ride an air Transport; land to fight, air-drop crushing structure crates. |
| [`wall-weaver/`](wall-weaver/) | Wall Weaver | BAR | Walk trails that become walls; enemies re-path live around your maze. |
| [`arm-vs-cortex/`](arm-vs-cortex/) | Arm vs Cortex | BAR | Pick an Armada or Cortex commander and fight off the opposing faction. |
| [`scavenger-salvage/`](scavenger-salvage/) | Scavenger Salvage | BAR | Tow wrecks to the Recycler; met quotas auto-build your base. |

## Develop

| Task | What it does |
|---|---|
| `mise run test` | Unit tests (the `sim` crate holds all game rules) |
| `mise run lint` | rustfmt check and clippy |
| `mise run dev` | Rebuild the web bundle on change |
| `mise run ci` | lint, test and web build |
| `mise run build-all` | Build every game subdirectory into its `dist/` |
