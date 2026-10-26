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

## Develop

| Task | What it does |
|---|---|
| `mise run test` | Unit tests (the `sim` crate holds all game rules) |
| `mise run lint` | rustfmt check and clippy |
| `mise run dev` | Rebuild the web bundle on change |
| `mise run ci` | lint, test and web build |
