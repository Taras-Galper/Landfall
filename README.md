# Landfall

A turn-based colonial strategy game that runs in the browser.

Four powers from the Old Kingdoms race to settle an uncharted continent. You lead one of them, and play runs like this:

1. Cross the ocean and found colonies.
2. Put your colonists to work on the land and in the workshops.
3. Trade with the homeland, and with the eight native peoples who already live there.
4. Ring enough liberty bells to win support for independence.
5. Declare independence and hold out against the Crown's army.

**Play:** open `web/dist/index.html` in any current browser. The whole game is in that one file, so it also works offline.

## The game

- **Colonies.** Food feeds the colonists and lets the colony grow. Farmers, fishermen, trappers, lumberjacks and miners work the land around each colony. Craftsmen turn raw goods into finished ones: sugar into rum, cotton into cloth, ore into tools, tools into muskets. Carpenters turn lumber into the hammers that put up buildings.
- **Trade.** Ships carry goods to the homeland market and come back with colonists, tools and horses. Prices fall as you sell. The Crown raises taxes, and if you refuse, a harbour protest leads to a boycott.
- **Native peoples.** Each of the eight peoples has its own lands, villages and temper. You can trade with them, learn their skills, send missionaries, pay for land or take it. Taking it has consequences.
- **The colonial assembly.** Liberty bells bring great figures into the assembly. Each one changes the rules in your favour.
- **Rivals.** Three computer powers expand, trade and go to war.
- **Independence.** Once half your colonists back independence, you can declare it. You then win by defeating every regiment the Crown sends.

The interface is built to keep information visible and clicks few:
- a production preview while you assign colonists,
- warnings for starving colonies and full warehouses,
- route previews with turn counts, and combat odds before an attack,
- a news log instead of a pile of message boxes,
- undo for moves and colony changes,
- loading and unloading ships straight from the map,
- autosave and three save slots.

All graphics are placeholders for now.

## Project layout

| Folder | Contents |
|---|---|
| `core/` | The rules engine (Rust crate `landfall_core`, library `landfall`). It has no interface and reads no files. |
| `data/rules.json` | The setting and every number the rules use: terrain, goods, units, buildings, prices, the peoples and powers, colony names, assembly members. |
| `web/engine/` | The engine for the browser: a JSON command interface compiled to WebAssembly (crate `landfall_web`). |
| `web/src/` | The interface: plain HTML, CSS and JavaScript, no framework. |
| `web/dist/` | The built game, ready to play or host. |

## Building

You need [Rust](https://rustup.rs) and [Node.js](https://nodejs.org).

Do this once:

```
rustup target add wasm32-unknown-unknown
```

Then, after any change, rebuild:

| On | Command |
|---|---|
| Windows (PowerShell) | `cargo build --profile web -p landfall_web --target wasm32-unknown-unknown; node web/build.mjs` |
| Linux / macOS | `sh web/build.sh` |

`node web/build.mjs` writes three files:
- `web/dist/index.html`: the whole game in one file.
- `web/dist/embedded.html`: the same page without its `<html>` wrapper, for embedding in another page.
- `web/dist/landfall.wasm`: the engine on its own.

Run the tests with `cargo test --release`.

## Hosting on GitHub Pages

Push this repository to GitHub. Then go to **Settings → Pages**, choose **Deploy from a branch**, then your branch and the `/ (root)` folder, and Save.

The game will be at `https://<user>.github.io/<repo>/web/dist/`.

To serve it from the site's root instead, copy `web/dist/index.html` to the top of the repository, or to a `docs/` folder and pick that folder in the Pages settings.
