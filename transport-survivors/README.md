# Transport Survivors

A single-player survivors game for the browser. You drive a train that lays its
own track. Wagons are weapons. Survive ten minutes and beat the boss.

See [docs/design.html](docs/design.html) for the game design and
[docs/concepts.html](docs/concepts.html) for the concepts that followed it.

## Play

```sh
mise install      # Rust, wasm target, miniserve, watchexec
mise run serve    # build for the browser, open http://localhost:8080
mise run run      # or run natively
```

On a touch screen, drag on the left side to steer. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Develop

| Task | What it does |
|---|---|
| `mise run test` | Unit tests (the `sim` crate holds all game rules) |
| `mise run lint` | rustfmt check and clippy |
| `mise run dev` | Rebuild the web bundle on change |
| `mise run ci` | lint, test and web build |
