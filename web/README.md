# Landfall web edition

The browser interface and the WebAssembly build of the rules engine. To build it, see the main README.

## Source files

`web/src/js` is loaded in file-name order. `build.mjs` joins all of it into one page.

| File | Role |
|---|---|
| `00_engine.js` | Loads the WebAssembly engine and sends it commands. |
| `10_util.js` | DOM helpers, colours, the placeholder icon set, saving to browser storage. |
| `20_map.js` | The map canvas: drawing, zoom, pan, path preview, minimap. |
| `30_panels.js` | Top bar, side panel, loading cargo from the map, news log, tooltips. |
| `35_drag.js` | Drag and drop for the colony and homeland screens. |
| `40_colony.js` | The colony screen, with production previews. |
| `50_homeland.js` | The homeland: market, docks, recruiting, training, buying ships. |
| `60_dialogs.js` | Decisions, story moments, the assembly, trade routes, menus, reports. |
| `70_start.js` | The start screen and new-game settings. |
| `90_main.js` | App state, commands, selection, keyboard, saves. |

## The command interface

The page sends the engine one JSON command at a time:

```
{"cmd": "move", "unit": 12, "dx": -1, "dy": 0}
```

Every reply has the same shape:

```
{"ok": true, "msg": "...", "error": "...", "prompt": {...}, "news": [...], "events": [...], "state": {...}, "data": {...}}
```

- `prompt` means the player must decide something: attack, landfall, a village visit, a land claim, sailing home.
- `events` are story moments and decisions from the turn: milestones, a leader speaking, a tax demand, native offers.
- `state` is the human player's view of the game. It comes back after every command that changes something.
- `data` is the answer to a query command such as `colony`, `homeland`, `reports`, `path`, `tile` or `odds`.

The full list of commands is in `web/engine/src/lib.rs`, in the `command` function. `web/engine/tests/commands.rs` plays a game through them.

## Saving

Saves are the engine's JSON game state. They are kept in the browser's local storage as an autosave every turn plus three slots, and can be downloaded or opened as files. Add `?seed=123` to the page address to start games on a fixed map.
