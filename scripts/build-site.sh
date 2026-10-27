#!/usr/bin/env bash
# Assemble site/ from every built game and generate the landing page.
# The game list comes from the "Games" table in README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

out=site
rm -rf "$out"
mkdir -p "$out"

esc() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }

copy_game() { # <dist dir> <slug>
  [ -f "$1/index.html" ] || { echo "missing build: $1" >&2; exit 1; }
  mkdir -p "$out/$2"
  cp -r "$1"/. "$out/$2/"
}

card() { # <slug> <title> <pitch>
  printf '      <a class="card" href="%s/"><h3>%s</h3><p>%s</p></a>\n' \
    "$1" "$(esc <<<"$2")" "$(esc <<<"$3")"
}

cards_trains=""
cards_bar=""
while IFS='|' read -r _ dir title theme pitch _; do
  slug=$(sed -E 's/.*`([^`/]+)\/`.*/\1/' <<<"$dir")
  title=$(xargs <<<"$title")
  theme=$(xargs <<<"$theme")
  pitch=$(sed -E 's/^ +| +$//g' <<<"$pitch")
  copy_game "$slug/dist" "$slug"
  c=$(card "$slug" "$title" "$pitch")
  case "$theme" in
    Trains) cards_trains+="$c"$'\n' ;;
    *) cards_bar+="$c"$'\n' ;;
  esac
done < <(grep -E '^\| \[`[a-z0-9-]+/`\]' README.md)

cat >"$out/index.html" <<HTML
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Survivors</title>
  <style>
    :root { color-scheme: dark; --bg: #0d0f14; --card: #171a22; --line: #2a2f3b; --fg: #e6e8ee; --dim: #9aa1b2; --accent: #f0b429; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--bg); color: var(--fg); font: 16px/1.5 system-ui, sans-serif; }
    main { max-width: 1100px; margin: 0 auto; padding: 32px 16px 64px; }
    h1 { margin: 0 0 4px; font-size: 2rem; }
    h2 { margin: 40px 0 12px; font-size: 1.1rem; color: var(--accent); text-transform: uppercase; letter-spacing: .08em; }
    .lead { margin: 0; color: var(--dim); }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
    .card { display: block; padding: 14px 16px; background: var(--card); border: 1px solid var(--line); border-radius: 8px; color: inherit; text-decoration: none; }
    .card:hover, .card:focus-visible { border-color: var(--accent); outline: none; }
    .card h3 { margin: 0 0 4px; font-size: 1.05rem; }
    .card p { margin: 0; color: var(--dim); font-size: .92rem; }
    footer { margin-top: 48px; color: var(--dim); font-size: .85rem; }
    a { color: var(--accent); -webkit-tap-highlight-color: transparent; }
    .lead + .lead { margin-top: 8px; }
    @media (max-width: 520px) { h1 { font-size: 1.6rem; } main { padding-top: 20px; } .grid { grid-template-columns: 1fr; } }
  </style>
</head>
<body>
  <main>
    <h1>Survivors</h1>
    <p class="lead">Survivor-like games for the browser, written in Rust with macroquad. Pick a game to play.</p>
    <p class="lead">On a phone or tablet, turn it to landscape. Drag on the left side to move, and tap the buttons on the right to act.</p>
    <h2>Trains</h2>
    <div class="grid">
$cards_trains    </div>
    <h2>Beyond All Reason</h2>
    <div class="grid">
$cards_bar    </div>
    <footer>Source: <a href="https://github.com/dennisklein/survivors">github.com/dennisklein/survivors</a>. "BAR" games are themed on <a href="https://www.beyondallreason.info/">Beyond All Reason</a>.</footer>
  </main>
</body>
</html>
HTML

echo "site/ ready: $(ls -d "$out"/*/ | wc -l) games"
