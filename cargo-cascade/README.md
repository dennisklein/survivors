# Cargo Cascade

An economy-first survivor-like about trains. Coal mines, power plants,
factories and towns dot the map. Goods must cascade down the chain
(coal -> power plant -> factory -> town), and every town that receives goods
lights up a defensive aura. At night monsters pour in from the map edges to
sabotage your industries. Connect every town before night 20 to win.

## Play

```sh
mise run serve   # build the wasm bundle and serve http://localhost:8080
mise run run     # native build (Linux needs libasound2-dev)
mise run test    # unit tests for the rules and the sound synth
mise run build   # dist/index.html + dist/cargo-cascade.wasm + JS loader
```

## How to play

- Click a producer, then a consumer, to plan a route: Coal Mine -> Power
  Plant, Power Plant -> Factory, Factory -> Town. Track costs money by length
  and is built automatically during the day (construction pauses at night).
  A train of the selected engine model is bought with the route if affordable.
- Trains load at the producer and are paid per unit on delivery (longer
  routes pay more per unit). Plants and factories only produce while fed.
- A town that received goods recently has a strong damage aura, stronger with
  more active factories feeding it. A town counts as **connected** after
  100 goods and while still supplied. Connect every town to win.
- Nights 1-20 bring escalating waves: Gremlins chew anything, Saboteurs go for
  industries, Brutes and Behemoths (every 5th night) march on towns. Wrecked
  industries stop the chain until repaired. If a town falls, the game is lost;
  if night 20 ends with towns still dark, too.
- Signal boxes are turrets placed beside track or industries.
- Kills drop scrap (cash) and XP. Each level-up offers a choice of three
  upgrades (aura power and reach, signal box damage, rate and range, freight
  income, train speed, fortification).
- The timetable panel lists routes: `+` adds a train of the selected model,
  `^` replaces the slowest engine with the selected faster one, `x` removes
  the route (engines sold at half price).

## Controls

| Input | Action |
| --- | --- |
| Left click industry A, then B | Plan route A -> B |
| 1 / 2 / 3 | Select engine: Kettle / Mogul / Express (or pick an upgrade) |
| T | Toggle signal box placement |
| R (hovering an industry) | Repair it |
| Mouse wheel | Zoom (over the map) / scroll the timetable |
| Right or middle drag, WASD, arrows | Pan |
| Space | Toggle 3x speed |
| P | Pause |
| Esc | Cancel selection / pause |
| M | Mute / unmute |

On a touch screen, drag on the left side to move. Buttons on the right
side press the keys of the current screen. A tap acts as a left click.

## Assets and credits

- All graphics are drawn procedurally with macroquad shapes.
- All sound effects and the music loop are synthesized at startup in
  `src/synth.rs` (16-bit PCM WAV generated in Rust); no audio files ship.
- `web/mq_js_bundle.js` is the macroquad JS loader (MIT/Apache-2.0) including
  its WebAudio plugin. Audio starts after the first key press or click.
