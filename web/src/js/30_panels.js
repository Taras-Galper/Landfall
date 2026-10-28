// The top bar, the side panel, the news log and map tooltips.
const Topbar = {
  render() {
    const st = App.st, m = App.meta, me = st.players[st.human];
    const bar = $('#topbar');
    bar.innerHTML = '';
    const cong = st.assembly;
    const candidate = cong.candidate !== null && cong.candidate !== undefined ? m.members[cong.candidate].name : null;
    bar.append(
      h('div', { class: 'brand' }, h('span', { class: 'flag', style: { '--c': NATION_COLORS[st.human] } }), h('b', {}, me.name)),
      h('div', { class: 'stat date', title: 'Turn ' + (st.turn + 1) }, st.year_label),
      h('div', { class: 'stat', title: 'Treasury' }, h('span', { class: 'k' }, 'Gold'), h('b', {}, st.gold.toLocaleString())),
      h('div', { class: 'stat', title: `The Crown takes ${st.tax}% of every sale` }, h('span', { class: 'k' }, 'Tax'), h('b', {}, st.tax + '%')),
      h('div', { class: 'stat', title: 'Share of colonists who back independence' }, h('span', { class: 'k' }, 'Rebels'), h('b', {}, st.rebel_percent + '%')),
      h('button', { class: 'stat link', title: `${m.assembly}: liberty bells towards the next member`, onclick: () => Dialogs.assembly() },
        h('span', { class: 'k' }, m.assembly), h('b', {}, candidate ? `${candidate} ${cong.bells}/${cong.cost}` : 'Choose a candidate'),
        h('span', { class: 'meter' }, h('i', { style: { width: (candidate ? Math.min(100, 100 * cong.bells / Math.max(1, cong.cost)) : 0) + '%' } }))),
      h('div', { class: 'spacer' }),
      h('button', { class: 'btn ghost', onclick: () => Homeland.open(), title: 'Homeland (H)' }, m.homeland.replace(/^the /, 'The ')),
      h('button', { class: 'btn ghost', onclick: () => Reports.open(), title: 'Reports (F2)' }, 'Reports'),
      h('button', { class: 'btn ghost', onclick: () => Dialogs.menu(), title: 'Game menu (Esc)' }, 'Menu'),
      h('button', { class: 'btn primary', onclick: () => App.endTurn(), title: 'End turn (Enter)' }, st.next_active ? 'End turn' : 'End turn ✓'),
    );
  },
};

const Side = {
  render() {
    const side = $('#side');
    side.innerHTML = '';
    const st = App.st, m = App.meta;
    const u = App.selected();
    if (u) side.append(this.unitCard(u));
    else side.append(h('div', { class: 'card quiet' },
      h('p', {}, st.next_active ? 'Select a unit to give it orders.' : 'All units have their orders.'),
      st.next_active ? h('button', { class: 'btn', onclick: () => App.selectNext() }, 'Next unit (Tab)') : h('button', { class: 'btn primary', onclick: () => App.endTurn() }, 'End turn (Enter)')));
    // my colonies at a glance
    const mine = st.colonies.filter(c => c.nation === st.human);
    if (mine.length) {
      const list = h('div', { class: 'card colonies' }, h('h3', {}, 'Colonies'));
      for (const c of mine) {
        const warn = (c.warnings || []).filter(w => !w.startsWith('Nothing'));
        list.append(h('button', { class: 'colony-row' + (warn.length ? ' warn' : ''), onclick: () => Colony.open(c.id), title: (c.warnings || []).join('\n') || 'All is well' },
          h('b', {}, c.name), h('span', {}, `${c.pop}`), h('span', { class: c.food < 0 ? 'neg' : 'pos' }, `${signed(c.food)} food`),
          h('span', { class: 'build' }, c.building || 'idle'),
          warn.length ? h('i', { class: 'dot' }) : null));
      }
      side.append(list);
    }
    const mm = h('canvas', { id: 'minimap', width: 232, height: 288, title: 'Click to look there' });
    mm.addEventListener('click', e => Minimap.click(e));
    side.append(h('div', { class: 'card mini' }, mm));
    Minimap.draw();
  },

  unitCard(u) {
    const st = App.st, m = App.meta;
    const card = h('div', { class: 'card unit' });
    const col = App.colonyAt(u.x, u.y);
    const t = MapView.tileAt(u.x, u.y);
    const onLand = t && t.explored && !t.water;
    card.append(h('div', { class: 'unit-head' }, unitToken(u, 44),
      h('div', {}, h('h3', {}, u.name), h('p', { class: 'sub' }, col ? `In ${col.name}` : t && t.explored ? Tooltip.info(u.x, u.y).terrain : ''))));
    const mv = u.moves / 3, max = u.max_moves / 3;
    card.append(h('div', { class: 'pips', title: 'Moves left this turn' }, h('span', {}, 'Moves'),
      h('b', {}, `${Number.isInteger(mv) ? mv : mv.toFixed(1)} / ${max}`),
      u.orders !== 'none' ? h('span', { class: 'tag' }, ({ sentry: 'On sentry', fortify: 'Fortified', plow: 'Plowing', road: 'Building road', goto: 'Travelling', route: 'On trade route' })[u.orders] || u.orders) : null));
    if (u.tools) card.append(h('p', { class: 'sub' }, `Carries ${u.tools} tools`));
    if (u.treasure) card.append(h('p', { class: 'sub' }, `Treasure worth ${u.treasure.toLocaleString()} gold`));
    if (isCarrier(u.kind)) {
      card.append(Cargo.panel(u, col && col.nation === st.human ? col : null));
      if (u.route) card.append(h('p', { class: 'sub' }, 'Route: ' + u.route));
    }
    const others = st.units.filter(x => x.id !== u.id && x.nation === st.human && x.loc.type === 'map' && x.x === u.x && x.y === u.y);
    if (others.length && u.loc.type === 'map') {
      const row = h('div', { class: 'also' }, h('span', { class: 'k' }, 'Also here'));
      for (const o of others) {
        const b = h('button', { class: 'also-unit', title: `${o.name}: click to select`, onclick: () => App.pick(o) }, unitToken(o, 24), h('span', {}, o.name));
        row.append(b);
      }
      card.append(row);
    }
    const B = (label, key, fn, title, primary) => h('button', { class: 'btn' + (primary ? ' primary' : ''), onclick: fn, title: title || '' }, label, key ? h('kbd', {}, key) : null);
    const acts = h('div', { class: 'actions' });
    if (u.loc.type === 'map') {
      if (isColonistKind(u.kind) && onLand) {
        if (col && col.nation === st.human) acts.append(B('Join colony', 'B', () => App.act({ cmd: 'join', unit: u.id }), 'Put this colonist to work in the colony', true));
        else if (!col) acts.append(B('Found colony', 'B', () => App.found(u.id), 'Build a new colony on this square', true));
      }
      if (u.kind === 2 && onLand) {
        acts.append(B(t.plowed ? 'Plowed' : t.forest ? 'Clear forest' : 'Plow', 'P', () => App.act({ cmd: 'improve', unit: u.id, what: 'plow' }), 'Uses 20 tools'));
        if (!t.road) acts.append(B('Road', 'R', () => App.act({ cmd: 'improve', unit: u.id, what: 'road' }), 'Uses 20 tools'));
      }
      if (col && col.nation === st.human) acts.append(B('Open colony', 'C', () => Colony.open(col.id, u.id)));
      if (isShip(u.kind) && t && t.lane && !st.rebellion) acts.append(B(`Sail to ${m.homeland.replace(/^the /, '')}`, 'E', () => App.act({ cmd: 'sail_home', unit: u.id }), '', true));
      if (isCarrier(u.kind)) acts.append(B('Trade route', 'T', () => Dialogs.route(u)));
      if (!isCarrier(u.kind) || isShip(u.kind)) acts.append(B('Fortify', 'F', () => App.act({ cmd: 'orders', unit: u.id, orders: 'fortify' }, { next: true })));
      acts.append(B('Sentry', 'S', () => App.act({ cmd: 'orders', unit: u.id, orders: 'sentry' }, { next: true })));
      acts.append(B('Skip', 'Space', () => App.act({ cmd: 'skip', unit: u.id }, { next: true })));
      acts.append(B('Disband', 'Del', () => Dialogs.confirm('Disband this unit?', `The ${u.name} will be gone for good.`, 'Disband', () => App.act({ cmd: 'disband', unit: u.id }))));
    }
    card.append(acts);
    card.append(h('p', { class: 'hint' }, 'Click a square to move or travel there. Arrow keys move one square; Ctrl+Z undoes a move.'));
    return card;
  },
};

const Log = {
  items: [],
  add(text, kind = '') {
    if (!text) return;
    this.items.push({ text, kind, turn: App.st ? App.st.year_label : '' });
    if (this.items.length > 200) this.items.shift();
    this.render(true);
  },
  render(fresh) {
    const el = $('#log');
    const open = el.classList.contains('open');
    el.innerHTML = '';
    const shown = open ? this.items.slice(-40) : this.items.slice(-4);
    el.append(h('button', { class: 'log-toggle', onclick: () => { el.classList.toggle('open'); this.render(); }, title: 'Show more news' }, open ? 'Less' : `News (${this.items.length})`));
    const ul = h('ul', {});
    for (const it of shown) ul.append(h('li', { class: it.kind }, h('span', { class: 'when' }, it.turn), ' ', it.text));
    el.append(ul);
    if (fresh) el.classList.add('flash'), setTimeout(() => el.classList.remove('flash'), 600);
    ul.scrollTop = ul.scrollHeight;
  },
};

const Tooltip = {
  cache: {},
  turn: -1,
  hide() { $('#tooltip').hidden = true; },
  move(x, y) {
    const el = $('#tooltip'); if (el.hidden) return;
    const w = el.offsetWidth, hh = el.offsetHeight;
    el.style.left = Math.min(window.innerWidth - w - 8, x + 16) + 'px';
    el.style.top = Math.min(window.innerHeight - hh - 8, y + 18) + 'px';
  },
  info(x, y) {
    if (this.turn !== App.st.turn) { this.cache = {}; this.turn = App.st.turn; }
    const k = `${x},${y}`;
    if (!this.cache[k]) this.cache[k] = Engine.call({ cmd: 'tile', x, y }).data || { explored: false };
    return this.cache[k];
  },
  tile(t, cx, cy) {
    const st = App.st, m = App.meta;
    const el = $('#tooltip');
    if (!st || t.x < 0 || t.y < 0 || t.x >= st.width || t.y >= st.height) return this.hide();
    const d = this.info(t.x, t.y);
    if (!d.explored) return this.hide();
    el.innerHTML = '';
    el.append(h('b', {}, d.terrain));
    const notes = [];
    if (d.resource) notes.push(d.resource);
    if (d.rumour) notes.push('Rumours of something here');
    if (d.move_cost > 1) notes.push(`Costs ${d.move_cost} moves`);
    if (d.defence) notes.push(`Defence +${d.defence * 12.5}%`);
    if (notes.length) el.append(h('div', { class: 'sub' }, notes.join(' · ')));
    const y = h('div', { class: 'yields' });
    const seen = new Set();
    d.yields.forEach((n, job) => {
      const g = m.job_good[job];
      if (n > 0 && !(seen.has(g))) { seen.add(g); y.append(goodChip(g, n)); }
    });
    if (y.childNodes.length) el.append(y);
    const col = App.colonyAt(t.x, t.y);
    if (col) el.append(h('div', {}, `${col.name}, ${m.nations[col.nation]} colony of ${plural(col.pop, 'colonist')}`));
    const vg = st.villages.find(v => v.x === t.x && v.y === t.y);
    if (vg) el.append(h('div', {}, `${m.nations[vg.tribe]} ${vg.capital ? 'capital' : 'village'}`, vg.teaches ? `, teaches ${vg.teaches}` : '', `. Mood: ${['content', 'uneasy', 'angry', 'hostile'][Math.min(3, Math.floor(vg.alarm / 25))]}.`));
    const units = st.units.filter(u => u.x === t.x && u.y === t.y && (!u.loc || u.loc.type === 'map'));
    if (units.length) el.append(h('div', { class: 'sub' }, units.slice(0, 5).map(u => (u.nation === st.human ? '' : m.adjectives[u.nation] ? m.adjectives[u.nation] + ' ' : m.nations[u.nation] + ' ') + u.name).join(', ') + (units.length > 5 ? ` and ${units.length - 5} more` : '')));
    if (d.claim && !col) el.append(h('div', { class: 'warn' }, `${d.claim.tribe} land: ${d.claim.price} gold to settle`));
    const sel = App.selected();
    if (sel && sel.loc.type === 'map' && !(sel.x === t.x && sel.y === t.y)) {
      const enemy = units.find(u => u.nation !== st.human) || (col && col.nation !== st.human) || vg;
      if (enemy && !isShip(sel.kind) && Math.abs(sel.x - t.x) <= 1 && Math.abs(sel.y - t.y) <= 1) {
        const r = Engine.call({ cmd: 'odds', unit: sel.id, x: t.x, y: t.y });
        if (r.data && r.data.odds !== null && [1, 4, 6, 7, 8, 9, 11].includes(sel.kind)) el.append(h('div', { class: 'odds' }, `Attack: ${Math.round(r.data.odds * 100)}% chance to win`));
      }
    }
    el.hidden = false;
    this.move(cx, cy);
  },
};

// Loading and unloading a ship or wagon train from the map, without opening the colony.
const Cargo = {
  panel(u, col) {
    const st = App.st, m = App.meta;
    const box = h('div', { class: 'cargo' });
    const keep = { quiet: false, keep: true };
    const load = g => App.act({ cmd: 'load_goods', colony: col.id, carrier: u.id, good: g }, keep);
    const passengers = st.units.filter(x => x.loc && x.loc.type === 'aboard' && x.loc.ship === u.id);
    box.append(h('div', { class: 'cargo-head' }, h('span', { class: 'k' }, `Holds ${u.holds_used} of ${u.holds} used`),
      col && (u.cargo.length || passengers.length) ? h('button', { class: 'btn small', onclick: () => Cargo.unloadAll(u), title: 'Unload all goods and passengers (U)' }, 'Unload all') : null));
    // the holds: one box each; click a full one to unload it, drop goods on an empty one to load
    const holds = h('div', { class: 'hold-row', 'data-drop': 'holds' });
    u.cargo.forEach(([g, n], slot) => {
      const label = h('span', { class: 'hold-name' }, m.goods[g]);
      if (col) holds.append(h('button', { class: 'hold full', title: `Click to unload ${n} ${m.goods[g]} into ${col.name}`, onclick: () => App.act({ cmd: 'unload', carrier: u.id, slot }, keep) }, goodChip(g, n), label));
      else holds.append(h('span', { class: 'hold full' }, goodChip(g, n), label));
    });
    for (const p of passengers) {
      const t = unitToken(p, 24);
      const label = h('span', { class: 'hold-name' }, p.name);
      if (col) holds.append(h('button', { class: 'hold full', title: `Click to put ${p.name} ashore in ${col.name}`, onclick: () => App.act({ cmd: 'step_out', unit: p.id }, keep) }, t, label));
      else holds.append(h('span', { class: 'hold full' }, t, label));
    }
    for (let i = u.holds_used; i < u.holds; i++) holds.append(h('span', { class: 'hold free' }, 'Empty'));
    box.append(holds);
    if (!col) return box;
    // what the colony has to load
    const room = u.holds_used < u.holds;
    const list = h('div', { class: 'load-list' });
    (col.stock || []).forEach((n, g) => {
      if (n <= 0) return;
      const partial = u.cargo.find(([cg, cn]) => cg === g && cn < 100);
      const can = room || !!partial;
      const amount = Math.min(n, partial ? 100 - partial[1] : 100);
      const row = h('button', { class: 'load-row', disabled: !can, title: can ? `Load ${amount} ${m.goods[g]} onto the ${u.name}. You can also drag it to a hold.` : 'No room aboard' },
        goodChip(g), h('span', { class: 'lname' }, m.goods[g]), h('span', { class: 'lstock' }, `${n} in store`),
        h('span', { class: 'lgo' }, can ? `Load ${amount}` : 'Full'));
      if (can) row.addEventListener('pointerdown', e => Drag.start(e, { type: 'good', g }, row, { click: () => load(g), drop: (p, t) => { if (t.dataset.drop === 'holds') load(p.g); } }));
      list.append(row);
    });
    box.append(h('div', { class: 'cargo-head' }, h('span', { class: 'k' }, list.childNodes.length ? `Load from ${col.name}` : `${col.name}'s warehouse is empty`)));
    if (list.childNodes.length) box.append(list);
    // people on the colony square can come aboard ships
    if (isShip(u.kind)) {
      const people = st.units.filter(x => x.nation === st.human && x.loc.type === 'map' && x.x === u.x && x.y === u.y && !isCarrier(x.kind));
      if (people.length) {
        const plist = h('div', { class: 'load-list' });
        for (const p of people) plist.append(h('button', { class: 'load-row', disabled: !room, title: room ? `${p.name} boards the ${u.name}` : 'No room aboard', onclick: () => App.act({ cmd: 'board', unit: p.id, ship: u.id }, keep) },
          unitToken(p, 22), h('span', { class: 'lname' }, p.name), h('span', { class: 'lstock' }), h('span', { class: 'lgo' }, room ? 'Board' : 'Full')));
        box.append(h('div', { class: 'cargo-head' }, h('span', { class: 'k' }, 'Take aboard')), plist);
      }
    }
    return box;
  },
  unloadAll(u) {
    const col = App.colonyAt(u.x, u.y);
    if (!col || col.nation !== App.st.human) { toast('Sail or drive into one of your colonies to unload.', 'bad'); return; }
    let r;
    for (let i = u.cargo.length - 1; i >= 0; i--) r = App.act({ cmd: 'unload', carrier: u.id, slot: i }, { quiet: true, keep: true });
    for (const p of App.st.units.filter(x => x.loc && x.loc.type === 'aboard' && x.loc.ship === u.id)) r = App.act({ cmd: 'step_out', unit: p.id }, { quiet: true, keep: true });
    toast(`Unloaded in ${col.name}.`);
    return r;
  },
};
