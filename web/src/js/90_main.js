// The application: state, commands, selection, keyboard, saving.
const UNDOABLE = new Set(['move', 'goto', 'orders', 'skip', 'improve', 'assign', 'step_out', 'board', 'load_goods', 'unload', 'build', 'join', 'equip', 'disembark', 'rename', 'set_route']);

const App = {
  st: null, meta: null, sel: null, screen: 'start',
  view: { x: 0, y: 0, T: 48 },
  prefs: { grid: false },
  undo: [],
  colIndex: new Map(),
  memberAskedTurn: -1,
  overShown: false,

  colonyAt(x, y) { return this.colIndex.get(x + ',' + y) || null; },
  selected() {
    if (this.sel === null || !this.st) return null;
    const u = this.st.units.find(u => u.id === this.sel && u.nation === this.st.human);
    return u || null;
  },

  // Send a command to the engine and show what came back.
  act(cmd, opts = {}) {
    let snapshot = null;
    if (UNDOABLE.has(cmd.cmd) && this.st) {
      const s = Engine.call({ cmd: 'save' });
      snapshot = s.data && s.data.json;
    }
    const r = Engine.call(cmd);
    if (opts.fresh) { Log.items = []; this.undo = []; this.sel = null; this.overShown = false; this.memberAskedTurn = -1; }
    if (r.state) this.setState(r.state);
    const news = r.news || [], events = r.events || [];
    if (snapshot && r.ok) {
      if (news.length || events.length || r.prompt || cmd.cmd === 'goto') this.undo = [];
      else { this.undo.push(snapshot); if (this.undo.length > 30) this.undo.shift(); }
    }
    if (cmd.cmd === 'end_turn' || cmd.cmd === 'new_game') this.undo = [];
    for (const n of news) Log.add(n, /WAR|destroyed|captured|lost|starv|famine|burn/i.test(n) ? 'bad' : '');
    if (!opts.quiet) {
      if (r.error) toast(r.error, 'bad');
      else if (r.msg) toast(r.msg);
    }
    for (const e of events) Dialogs.event(e);
    if (r.prompt) Dialogs.prompt(r.prompt);
    if (r.state) {
      if (opts.next) this.autoNext();
      else if (this.sel !== null && !(opts.keep && this.selected())) {
        const u = this.selected();
        if (!u || u.loc.type !== 'map' || u.moves <= 0 || u.done || u.orders !== 'none') { if (!r.prompt) this.autoNext(); }
        else MapView.centerOn(u.x, u.y, true);
      }
      this.refresh();
    }
    return r;
  },

  setState(st) {
    this.st = st;
    this.colIndex = new Map(st.colonies.map(c => [c.x + ',' + c.y, c]));
  },

  refresh() {
    if (!this.st) return;
    Topbar.render();
    Side.render();
    Log.render();
    MapView.invalidate();
    if (this.screen === 'colony' && !Drag.active()) { /* colony screen re-renders itself after its own actions */ }
  },

  select(id) {
    this.sel = id;
    const u = this.selected();
    if (u && u.loc.type === 'map') MapView.centerOn(u.x, u.y, true);
    this.refresh();
  },
  selectNext() {
    const st = this.st;
    const cands = st.units.filter(u => u.nation === st.human && u.loc.type === 'map' && u.moves > 0 && !u.done && u.orders === 'none' && u.kind !== 10);
    if (!cands.length) { this.sel = null; this.refresh(); return; }
    const i = cands.findIndex(u => u.id === this.sel);
    this.select(cands[(i + 1) % cands.length].id);
  },
  autoNext() {
    const id = this.st.next_active;
    this.sel = id === null || id === undefined ? null : id;
    const u = this.selected();
    if (u) MapView.centerOn(u.x, u.y, true);
  },
  selectAt(x, y) {
    const u = this.st.units.find(u => u.nation === this.st.human && u.x === x && u.y === y && u.loc.type === 'map' && !isShip(u.kind) && u.moves > 0);
    if (u) this.select(u.id);
  },

  clickTile(x, y) {
    const st = this.st;
    const col = this.colonyAt(x, y);
    const ownCol = col && col.nation === st.human ? col : null;
    const mine = st.units.filter(u => u.nation === st.human && u.loc.type === 'map' && u.x === x && u.y === y);
    const sel = this.selected();
    const t = MapView.tileAt(x, y);
    // a selected unit elsewhere goes where you click: into your colony, or onto an empty square
    if (sel && sel.loc.type === 'map' && (sel.x !== x || sel.y !== y) && t && (ownCol || !mine.length)) {
      this.orderTo(x, y);
      return;
    }
    if (ownCol) {
      // ships and wagons in port first, so they can load right here; then the colony itself
      const carriers = mine.filter(u => isCarrier(u.kind));
      const i = carriers.findIndex(u => u.id === this.sel);
      if (carriers.length && i < carriers.length - 1) { this.pick(carriers[i + 1]); return; }
      this.sel = null;
      Colony.open(ownCol.id);
      return;
    }
    if (mine.length) {
      const i = mine.findIndex(u => u.id === this.sel);
      this.pick(mine[(i + 1) % mine.length]);
      return;
    }
    this.sel = null; this.refresh();
  },
  pick(u) {
    if (u.orders !== 'none' || u.done) this.act({ cmd: 'activate', unit: u.id }, { quiet: true, keep: true });
    this.select(u.id);
  },

  orderTo(x, y) {
    const u = this.selected();
    if (!u || u.loc.type !== 'map') { MapView.centerOn(x, y); return; }
    if (u.x === x && u.y === y) return;
    if (Math.abs(u.x - x) <= 1 && Math.abs(u.y - y) <= 1) this.act({ cmd: 'move', unit: u.id, dx: x - u.x, dy: y - u.y });
    else this.act({ cmd: 'goto', unit: u.id, x, y });
  },

  moveSel(dx, dy) {
    const u = this.selected();
    if (!u) { this.selectNext(); return; }
    if (u.loc.type !== 'map') return;
    this.act({ cmd: 'move', unit: u.id, dx, dy });
  },

  found(unit, claim, name) {
    const go = nm => {
      const r = this.act({ cmd: 'found', unit, name: nm, claim }, { quiet: !!claim });
      if (r.prompt && r.prompt.type === 'land_claim') { this.pendingName = nm; return; }
      if (r.ok && r.data && r.data.colony) { this.sel = null; Colony.open(r.data.colony); }
    };
    if (name !== undefined) return go(name);
    if (claim) return go(this.pendingName || '');
    const sug = Engine.call({ cmd: 'suggest_name' }).data.name;
    Dialogs.ask('Name your colony', 'What shall the new colony be called?', sug, go);
  },

  endTurn(force) {
    if (Dialogs.open()) return;
    if (!force && this.st.next_active !== null && this.st.next_active !== undefined) {
      Dialogs.confirm('End the turn?', 'Some units still have moves left.', 'End turn', () => this.endTurn(true));
      return;
    }
    const before = Log.items.length;
    const r = this.act({ cmd: 'end_turn', force: true }, { next: true });
    const n = Log.items.length - before;
    toast(`${this.st.year_label}${n ? `: ${plural(n, 'report')}` : ''}`);
    this.autosave();
    return r;
  },

  // after the last dialog closes: prompts that wait their turn
  afterDialogs() {
    const st = this.st; if (!st || this.screen === 'start') return;
    if (st.game_over && !this.overShown) {
      this.overShown = true;
      Dialogs.show({ cls: 'milestone', title: st.game_over.won ? 'Victory' : 'The game is over', body: [st.game_over.text, `Final score: ${st.score}.`],
        options: [{ label: 'Keep playing', primary: true }, { label: 'New game', fn: () => Start.show() }] });
      return;
    }
    const c = st.assembly;
    if ((c.candidate === null || c.candidate === undefined) && c.offered.length && st.colonies.some(x => x.nation === st.human) && this.memberAskedTurn !== st.turn) {
      this.memberAskedTurn = st.turn;
      Dialogs.assembly();
      return;
    }
    if (this.screen === 'colony') Colony.render();
    if (this.screen === 'homeland') Homeland.render();
  },

  undoLast() {
    const snap = this.undo.pop();
    if (!snap) { toast('Nothing to undo.'); return; }
    const sel = this.sel;
    const r = Engine.call({ cmd: 'load', json: snap });
    if (r.state) this.setState(r.state);
    this.sel = sel;
    this.refresh();
    if (this.screen === 'colony') Colony.render();
    toast('Undone.');
  },

  // --- saving
  label() { return `${this.meta.nations[this.st.human]}, ${this.st.year_label}`; },
  autosave() {
    const s = Engine.call({ cmd: 'save' });
    if (s.data) { store('lf.auto', s.data.json); store('lf.auto.meta', this.label()); }
  },
  saveTo(key) {
    const s = Engine.call({ cmd: 'save' });
    if (!s.data) return;
    if (!store(key, s.data.json)) { toast('The browser refused to store the save. Download it instead.', 'bad'); return; }
    store(key + '.meta', this.label());
    toast('Game saved.');
  },
  loadFrom(key) {
    const j = store(key);
    if (!j) { toast('That save is empty.', 'bad'); return; }
    this.loadJson(j);
  },
  loadJson(j) {
    const r = this.act({ cmd: 'load', json: j }, { quiet: true, fresh: true });
    if (!r.ok) { toast(r.error, 'bad'); return; }
    Start.hide();
    this.started();
  },
  download() {
    const s = Engine.call({ cmd: 'save' });
    const blob = new Blob([s.data.json], { type: 'application/json' });
    const a = h('a', { href: URL.createObjectURL(blob), download: `landfall-${this.label().replace(/[^A-Za-z0-9]+/g, '-')}.json` });
    document.body.append(a); a.click(); a.remove();
  },
  upload() {
    const inp = h('input', { type: 'file', accept: '.json,application/json' });
    inp.addEventListener('change', async () => { const f = inp.files[0]; if (f) this.loadJson(await f.text()); });
    inp.click();
  },
  savePrefs() { store('lf.prefs', JSON.stringify(this.prefs)); },

  started() {
    this.screen = 'map';
    $('#sheet').hidden = true;
    this.autoNext();
    const u = this.selected();
    if (u) MapView.centerOn(u.x, u.y);
    else {
      const c = this.st.colonies.find(c => c.nation === this.st.human);
      if (c) MapView.centerOn(c.x, c.y);
    }
    this.refresh();
    if (!Dialogs.open()) this.afterDialogs();
  },
};

function onKey(e) {
  if (e.target.matches('input, select, textarea')) return;
  const k = e.key;
  if (Dialogs.open()) { if (k === 'Escape') { e.preventDefault(); Dialogs.cancel(); } return; }
  if (App.screen === 'start') return;
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'z') { e.preventDefault(); App.undoLast(); return; }
  if (App.screen === 'colony') {
    if (k === 'Escape' || k === 'Enter') { e.preventDefault(); Colony.close(); }
    if (k === 'n' || k === 'N') { e.preventDefault(); Colony.rename(); }
    return;
  }
  if (App.screen === 'homeland') { if (k === 'Escape' || k === 'Enter') { e.preventDefault(); Homeland.close(); } return; }
  if (App.screen === 'reports') { if (k === 'Escape' || k === 'Enter') { e.preventDefault(); Reports.close(); } return; }
  const dirs = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1], Home: [-1, -1], PageUp: [1, -1], End: [-1, 1], PageDown: [1, 1] };
  const pad = { 1: [-1, 1], 2: [0, 1], 3: [1, 1], 4: [-1, 0], 6: [1, 0], 7: [-1, -1], 8: [0, -1], 9: [1, -1] };
  if (dirs[k] || (e.code && e.code.startsWith('Numpad') && pad[k])) { e.preventDefault(); const [dx, dy] = dirs[k] || pad[k]; App.moveSel(dx, dy); return; }
  const u = App.selected();
  const key = k.length === 1 ? k.toLowerCase() : k;
  switch (key) {
    case 'Enter': e.preventDefault(); App.endTurn(e.shiftKey); break;
    case 'Escape': e.preventDefault(); if (App.sel !== null) { App.sel = null; App.refresh(); } else Dialogs.menu(); break;
    case 'Tab': e.preventDefault(); App.selectNext(); break;
    case ' ': e.preventDefault(); if (u) App.act({ cmd: 'skip', unit: u.id }, { next: true }); break;
    case 'f': if (u) App.act({ cmd: 'orders', unit: u.id, orders: 'fortify' }, { next: true }); break;
    case 's': if (u) App.act({ cmd: 'orders', unit: u.id, orders: 'sentry' }, { next: true }); break;
    case 'b': if (u && isColonistKind(u.kind)) { const c = App.colonyAt(u.x, u.y); if (c && c.nation === App.st.human) App.act({ cmd: 'join', unit: u.id }); else App.found(u.id); } break;
    case 'p': if (u) App.act({ cmd: 'improve', unit: u.id, what: 'plow' }); break;
    case 'r': if (u) App.act({ cmd: 'improve', unit: u.id, what: 'road' }); break;
    case 't': if (u && isCarrier(u.kind)) Dialogs.route(u); break;
    case 'u': if (u && isCarrier(u.kind)) Cargo.unloadAll(u); break;
    case 'e': if (u && isShip(u.kind)) App.act({ cmd: 'send_home', unit: u.id }, { keep: true }); break;
    case 'g': if (u && isShip(u.kind)) Dialogs.sendTo(u); break;
    case 'h': Homeland.open(); break;
    case 'c': {
      const mine = App.st.colonies.filter(c => c.nation === App.st.human);
      const here = u && App.colonyAt(u.x, u.y);
      if (here && here.nation === App.st.human) Colony.open(here.id, u.id);
      else if (mine.length) { App._ci = ((App._ci ?? -1) + 1) % mine.length; MapView.centerOn(mine[App._ci].x, mine[App._ci].y); }
      break;
    }
    case 'Delete': if (u) Dialogs.confirm('Disband this unit?', `The ${u.name} will be gone for good.`, 'Disband', () => App.act({ cmd: 'disband', unit: u.id })); break;
    case 'F2': e.preventDefault(); Reports.open(); break;
    case '+': case '=': MapView.zoomAt(window.innerWidth / 2, window.innerHeight / 2, 1.15); break;
    case '-': MapView.zoomAt(window.innerWidth / 2, window.innerHeight / 2, 1 / 1.15); break;
    case '?': Dialogs.help(); break;
  }
}

async function boot() {
  try { App.prefs = Object.assign(App.prefs, JSON.parse(store('lf.prefs') || '{}')); } catch (e) { /* defaults */ }
  const z = parseFloat(store('lf.zoom')); if (z) App.view.T = z;
  try {
    await Engine.load();
  } catch (e) {
    const el = $('#start');
    el.hidden = false;
    el.append(h('div', { class: 'start-inner' }, h('div', { class: 'hero' }, h('h1', {}, 'Landfall'),
      h('p', { class: 'lede' }, 'This browser would not start the game engine (WebAssembly). Open the game in a current version of Chrome, Edge, Firefox or Safari.'),
      h('p', { class: 'sub' }, String(e && e.message || e)))));
    return;
  }
  App.meta = Engine.call({ cmd: 'meta' }).data;
  MapView.init();
  window.addEventListener('keydown', onKey);
  // right-click is a game control everywhere, so the browser's menu never opens (text fields keep it)
  document.addEventListener('contextmenu', e => { if (!e.target.closest('input, textarea, select')) e.preventDefault(); });
  Start.show();
}
boot();
