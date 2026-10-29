// The colony screen: land, buildings, production, people and the warehouse.
const Colony = (() => {
  let cid = null, view = null, sel = null, status = '';
  const previewCache = {};
  // squares in save order N, E, S, W, NW, NE, SE, SW → grid positions
  const DIR_POS = [[1, 0], [2, 1], [1, 2], [0, 1], [0, 0], [2, 0], [2, 2], [0, 2]];
  // job buildings: job index → BKind index
  const JOB_BUILDINGS = [
    { job: 17, kind: 3 }, { job: 13, kind: 14 }, { job: 14, kind: 16 }, { job: 15, kind: 1 },
    { job: 16, kind: 15 }, { job: 18, kind: 4 }, { job: 9, kind: 11 }, { job: 10, kind: 10 },
    { job: 11, kind: 9 }, { job: 12, kind: 13 },
  ];
  const OTHER_BUILDINGS = [0, 2, 5, 6, 7, 8, 12];

  function open(id, selectUnit) {
    cid = id; sel = selectUnit || null; status = '';
    App.screen = 'colony';
    render();
  }
  function close() {
    const s = sel;
    cid = null; view = null;
    $('#sheet').hidden = true; $('#sheet').innerHTML = '';
    App.screen = 'map';
    // a unit picked outside the colony becomes the active unit on the map
    const u = s && App.st.units.find(x => x.id === s);
    if (u && u.loc.type === 'map' && u.moves > 0) { App.act({ cmd: 'activate', unit: u.id }, { quiet: true }); App.select(u.id); }
    App.refresh();
  }
  const isOpen = () => cid !== null;

  function act(cmd) {
    const r = App.act(cmd, { quiet: true });
    status = r.error || r.msg || '';
    for (const k in previewCache) delete previewCache[k];
    render();
    return r;
  }

  function preview(unit, job) {
    const key = unit + ':' + JSON.stringify(job);
    if (!(key in previewCache)) {
      const r = Engine.call({ cmd: 'preview', colony: cid, unit, job });
      previewCache[key] = r.ok ? r.data : { error: r.error };
    }
    return previewCache[key];
  }

  function deltaChips(d) {
    const box = h('span', { class: 'delta' });
    if (d.error) { box.append(h('em', {}, d.error)); return box; }
    d.delta.forEach((n, g) => { if (n && g !== 19) box.append(goodChip(g, signed(n))); });
    if (!box.childNodes.length) box.append(h('em', {}, 'no change'));
    return box;
  }

  function render() {
    if (cid === null) return;
    const r = Engine.call({ cmd: 'colony', colony: cid });
    if (!r.ok) { close(); return; }
    view = r.data;
    const m = App.meta, st = App.st;
    const sheet = $('#sheet');
    sheet.hidden = false;
    sheet.innerHTML = '';
    const c = view;
    const pop = c.workers.length;

    const head = h('header', { class: 'sheet-head' },
      h('button', { class: 'title-btn', title: 'Rename (N)', onclick: rename }, h('h2', {}, c.name)),
      h('span', { class: 'chip' }, plural(pop, 'colonist')),
      h('span', { class: 'chip', title: 'Share of colonists backing independence: at 50% production rises by 1, at 100% by 2' }, `Liberty ${c.liberty_share}%`),
      c.loyalist_penalty ? h('span', { class: 'chip bad', title: 'Too many loyalists cut production' }, `Loyalist penalty −${c.loyalist_penalty}`) : null,
      h('span', { class: 'chip' }, `Gold ${c.gold.toLocaleString()}`),
      h('div', { class: 'spacer' }),
      status ? h('span', { class: 'status' }, status) : null,
      h('button', { class: 'btn ghost', onclick: () => Dialogs.confirm(`Abandon ${c.name}?`, 'Everyone leaves and everything in the warehouse is lost.', 'Abandon colony', () => { act({ cmd: 'abandon', colony: cid }); close(); }) }, 'Abandon'),
      h('button', { class: 'btn primary', onclick: close, title: 'Back to the map (Esc)' }, 'Done'),
    );

    // --- land
    const land = h('section', { class: 'land' }, h('h3', {}, 'Land'));
    const grid = h('div', { class: 'land-grid' });
    for (let gy = 0; gy < 3; gy++) for (let gx = 0; gx < 3; gx++) {
      const dir = DIR_POS.findIndex(([x, y]) => x === gx && y === gy);
      const cell = h('div', { class: 'sq' });
      const cvs = h('canvas', { width: 96, height: 96 });
      cell.append(cvs);
      const tx = dir < 0 ? c.x : c.squares[dir].x, ty = dir < 0 ? c.y : c.squares[dir].y;
      requestAnimationFrame(() => MapView.paintTile(cvs, tx, ty));
      if (dir < 0) {
        cell.classList.add('center');
        cell.append(h('div', { class: 'sq-out' }, goodChip(0, c.center.food), c.center.secondary ? goodChip(c.center.secondary[0], c.center.secondary[1]) : null));
        cell.append(h('span', { class: 'sq-colony' }, svgIcon('colony')));
        cell.title = `${c.center.terrain}: the colony square works itself`;
      } else {
        const sq = c.squares[dir];
        cell.dataset.drop = 'square'; cell.dataset.dir = dir;
        const w = c.workers.find(w => w.job.dir === dir);
        if (w) {
          cell.append(personEl(w, true));
          cell.append(h('div', { class: 'sq-out' }, goodChip(m.job_good[w.job.job], w.output)));
          cell.title = `${sq.terrain}: ${w.name} works as ${m.jobs[w.job.job]}. Click their token again to change crop.`;
        } else {
          const best = sq.yields.map((n, j) => [n, j]).filter(([n]) => n > 0).sort((a, b) => b[0] - a[0]).slice(0, 2);
          cell.append(h('div', { class: 'sq-out faint' }, best.map(([n, j]) => goodChip(m.job_good[j], n))));
          if (sq.taken) cell.classList.add('blocked'), cell.title = 'Worked by another colony';
          else if (sq.village) cell.classList.add('blocked'), cell.title = 'A native village stands here';
          else cell.title = `${sq.terrain || 'Unexplored'}. Drop a colonist here to work it.`;
        }
        cell.addEventListener('click', () => clickSquare(dir));
      }
      cell.append(h('span', { class: 'preview' }));
      grid.append(cell);
    }
    land.append(grid);

    // --- buildings
    const bl = h('section', { class: 'buildings' }, h('h3', {}, 'Buildings'));
    const bgrid = h('div', { class: 'bgrid' });
    for (const { job, kind } of JOB_BUILDINGS) {
      const lvl = c.buildings[kind].level;
      const info = m.job_buildings.find(b => b.job === job);
      const name = lvl ? m.buildings[kind].levels[lvl - 1] : m.buildings[kind].levels[0];
      const card = h('div', { class: 'bcard' + (lvl ? '' : ' missing'), title: lvl ? `${name}: up to 3 ${m.jobs[job]}s` : `${name} isn't built yet` });
      if (lvl) { card.dataset.drop = 'building'; card.dataset.job = job; }
      const workers = c.workers.filter(w => w.job.building === job);
      const total = workers.reduce((s, w) => s + w.output, 0);
      card.append(h('div', { class: 'bname' }, h('b', {}, name), h('span', { class: 'conv' },
        info.input !== null ? [goodChip(info.input), h('i', { class: 'arrow' }, '›')] : null, goodChip(info.output, total || null))));
      const slots = h('div', { class: 'slots' });
      for (let i = 0; i < 3; i++) {
        const w = workers[i];
        slots.append(w ? personEl(w, true, w.output) : h('span', { class: 'slot' }));
      }
      card.append(slots, h('span', { class: 'preview' }));
      card.addEventListener('click', e => { if (e.target.closest('.person')) return; if (lvl && sel) act({ cmd: 'assign', colony: cid, unit: sel, job: { building: job } }); });
      bgrid.append(card);
    }
    bl.append(bgrid);
    const others = h('div', { class: 'others' });
    for (const k of OTHER_BUILDINGS) {
      const lvl = c.buildings[k].level;
      if (lvl) others.append(h('span', { class: 'chip' }, m.buildings[k].levels[lvl - 1]));
    }
    if (others.childNodes.length) bl.append(others);

    // --- production and construction
    const prod = h('section', { class: 'production' }, h('h3', {}, 'Each turn'));
    const plist = h('div', { class: 'plist' });
    const growth = c.stock[0];
    for (let g = 0; g < 19; g++) {
      const n = c.net[g];
      const made = c.made[g];
      if (!n && !made && !(g < 16 && c.stock[g])) continue;
      if (g === 16) continue;
      const row = h('div', { class: 'prow', 'data-good': g, title: g < 16 ? `${m.goods[g]}: ${c.made[g]} made, ${c.used[g] + (g === 0 ? c.eaten : 0)} used${g === 8 && c.bred ? `, ${c.bred} bred` : ''}` : m.goods[g] },
        goodChip(g), h('span', { class: 'pname' }, m.goods[g]),
        h('b', { class: n > 0 ? 'pos' : n < 0 ? 'neg' : '' }, signed(n)),
        g < 16 ? h('span', { class: 'pstock' + (c.stock[g] + n > c.capacity && g > 0 ? ' over' : '') }, c.stock[g]) : h('span', { class: 'pstock' }));
      plist.append(row);
    }
    prod.append(plist);
    const foodNote = c.net[0] >= 0 ? (c.net[0] > 0 ? `New colonist in ${Math.max(0, Math.ceil((200 - growth) / c.net[0]))} turns` : 'Not growing') : `Food runs out in ${Math.max(0, Math.floor(growth / -c.net[0]))} turns`;
    prod.append(h('div', { class: 'meter-row' + (c.net[0] < 0 ? ' bad' : '') }, h('span', {}, foodNote), h('span', { class: 'meter' }, h('i', { style: { width: Math.min(100, growth / 2) + '%' } }))));

    const build = h('div', { class: 'build' }, h('h3', {}, 'Construction'));
    if (c.building) {
      const pct = Math.min(100, 100 * c.hammers / Math.max(1, c.building.hammers));
      build.append(h('div', { class: 'build-cur' }, h('b', {}, c.building.name),
        h('span', {}, `${c.hammers}/${c.building.hammers} hammers` + (c.building.tools ? `, ${Math.min(c.stock[14], c.building.tools)}/${c.building.tools} tools` : '')),
        h('span', { class: 'meter' }, h('i', { style: { width: pct + '%' } })),
        h('span', { class: 'sub' }, c.building.turns !== null ? `Done in ${plural(c.building.turns, 'turn')}${c.building.tools > c.stock[14] ? ' if the tools arrive' : ''}` : 'No hammers being made: put someone in the carpenter\'s house')));
    } else build.append(h('p', { class: 'warn' }, 'Nothing is being built.'));
    const selEl = h('select', { 'aria-label': 'Choose what to build', onchange: e => { const o = c.options[+e.target.value]; act({ cmd: 'build', colony: cid, build: o ? o.build : null }); } },
      h('option', { value: '' }, 'Change construction…'),
      c.options.map((o, i) => h('option', { value: i }, `${o.name} (${o.hammers} hammers${o.tools ? `, ${o.tools} tools` : ''})`)));
    build.append(h('div', { class: 'row' }, selEl,
      c.building && c.rush_price ? h('button', { class: 'btn', disabled: c.rush_price > c.gold, onclick: () => Dialogs.confirm(`Finish ${c.building.name} now?`, `Pay ${c.rush_price.toLocaleString()} gold to complete it at the end of this turn.`, `Pay ${c.rush_price.toLocaleString()} gold`, () => act({ cmd: 'rush', colony: cid })), title: 'Pay to finish it this turn' }, `Buy for ${c.rush_price.toLocaleString()}`) : null));
    prod.append(build);

    // --- people outside and carriers
    const yard = h('section', { class: 'yard' });
    const people = c.units.filter(u => !isCarrier(u.kind) && u.loc.type === 'map');
    const outside = h('div', { class: 'outside', 'data-drop': 'outside', title: 'Drop a worker here to send them outside' }, h('h3', {}, 'Outside the colony'));
    const ol = h('div', { class: 'tokens' });
    for (const u of people) ol.append(personEl(u, false, undefined, true));
    if (!people.length) ol.append(h('p', { class: 'sub' }, 'Drag a worker here to take them out of the colony.'));
    outside.append(ol);
    const selU = sel && (people.find(u => u.id === sel));
    if (selU && isColonistKind(selU.kind)) {
      outside.append(h('div', { class: 'row' }, h('span', { class: 'sub' }, selU.name),
        h('button', { class: 'btn', onclick: () => Dialogs.equip(selU, false, k => act({ cmd: 'equip', unit: selU.id, kind: k })) }, 'Equip…'),
        h('button', { class: 'btn', onclick: () => act({ cmd: 'join', unit: selU.id }) }, 'Join colony')));
    }
    yard.append(outside);
    const carriers = c.units.filter(u => isCarrier(u.kind));
    const dock = h('div', { class: 'dock' }, h('h3', {}, c.coastal ? 'In port' : 'Wagon yard'));
    if (!carriers.length) dock.append(h('p', { class: 'sub' }, c.coastal ? 'No ships or wagon trains here.' : 'No wagon trains here. Build one to carry goods overland.'));
    for (const s of carriers) {
      const card = h('div', { class: 'carrier' + (sel === s.id ? ' sel' : ''), 'data-drop': 'carrier', 'data-id': s.id });
      card.append(h('div', { class: 'chead' }, unitToken(s, 30), h('b', {}, s.name), h('span', { class: 'sub' }, `${s.holds_used}/${s.holds} holds`)));
      const holds = h('div', { class: 'holds' });
      s.cargo.forEach(([g, n], slot) => {
        const chip = goodChip(g, n);
        chip.classList.add('cargo-chip');
        chip.title = `${m.goods[g]}: click to unload all, right-click to choose how many, or drag to the warehouse`;
        chip.addEventListener('pointerdown', e => Drag.start(e, { type: 'cargo', carrier: s.id, slot }, chip, handlers));
        chip.addEventListener('contextmenu', e => { e.preventDefault(); Amount.pick({ anchor: chip, title: `Unload ${m.goods[g]}`, max: n, value: n, verb: 'Unload', onOk: k => act({ cmd: 'unload', carrier: s.id, slot, amount: k }) }); });
        holds.append(chip);
      });
      for (const p of st.units.filter(x => x.loc && x.loc.type === 'aboard' && x.loc.ship === s.id)) holds.append(personEl(p, false));
      for (let i = s.holds_used; i < s.holds; i++) holds.append(h('span', { class: 'hold empty' }));
      card.append(holds);
      card.addEventListener('click', e => { if (e.target.closest('.cargo-chip,.person')) return; sel = sel === s.id ? null : s.id; render(); });
      dock.append(card);
    }
    yard.append(dock);

    // --- warehouse
    const wh = h('section', { class: 'warehouse', 'data-drop': 'warehouse' }, h('h3', {}, `Warehouse (holds ${c.capacity} of each)`));
    const goods = h('div', { class: 'wgoods' });
    for (let g = 0; g < 16; g++) {
      const n = c.stock[g];
      const cell = h('div', { class: 'wgood' + (n ? '' : ' empty') + (n > c.capacity && g ? ' over' : ''), title: `${m.goods[g]}: ${n}` + (g ? ` of ${c.capacity}` : '') + (n ? '. Drag onto a ship or wagon to load; right-click to choose how many.' : '') },
        goodChip(g), h('b', {}, n), h('span', { class: 'meter' }, h('i', { style: { width: Math.min(100, 100 * n / c.capacity) + '%' } })));
      if (n > 0) {
        cell.addEventListener('pointerdown', e => Drag.start(e, { type: 'good', g }, cell, handlers));
        cell.addEventListener('contextmenu', e => {
          e.preventDefault();
          const all = view.units.filter(u => isCarrier(u.kind));
          const target = all.find(u => u.id === sel) || (all.length === 1 ? all[0] : null);
          if (!target) { status = all.length ? 'Select a ship or wagon first.' : 'No ship or wagon train here to load.'; render(); return; }
          const part = target.cargo.find(([cg, cn]) => cg === g && cn < 100);
          const room = part ? 100 - part[1] : target.holds_used < target.holds ? 100 : 0;
          if (!room) { status = 'No room aboard.'; render(); return; }
          Amount.pick({ anchor: cell, title: `Load ${m.goods[g]} onto the ${target.name}`, max: Math.min(n, room), verb: 'Load', onOk: k => act({ cmd: 'load_goods', colony: cid, carrier: target.id, good: g, amount: k }) });
        });
      }
      goods.append(cell);
    }
    wh.append(goods);

    const legend = h('button', { class: 'btn ghost small', onclick: () => Dialogs.professions(), title: 'What the colours and codes on colonists mean' }, 'Professions');
    head.insertBefore(legend, head.querySelector('.spacer'));
    const selected = sel !== null && (c.workers.concat(c.units).find(u => u.id === sel));
    if (selected && isColonistKind(selected.kind)) {
      const info = profInfo(selected.prof);
      head.insertBefore(h('span', { class: 'chip prof', style: info.color ? { '--p': info.color } : {} }, `${selected.name}: ${info.hint}`), head.querySelector('.spacer'));
    }
    sheet.append(head, h('div', { class: 'colony-body' }, land, bl, prod, yard, wh));
    if (selected) markSpecialty(selected.prof);
  }

  function personEl(u, inside, output, tag) {
    const info = profInfo(u.prof);
    const tip = `${u.name}. ${info.hint} ${inside ? 'Drag to another job, or outside.' : 'Drag onto a square or building to put them to work.'}`;
    const tok = unitToken({ ...u, nation: App.st.human }, 36);
    tok.title = tip;
    const el = h('span', { class: 'person' + (sel === u.id ? ' sel' : ''), title: tip }, tok);
    if (output !== undefined) el.append(h('small', {}, output));
    if (tag) el.append(profTag(u));
    el.addEventListener('pointerdown', e => {
      e.stopPropagation();
      markSpecialty(u.prof);
      Drag.start(e, { type: 'unit', id: u.id }, el, { ...handlers, drop: (p, t) => { markSpecialty(null); handlers.drop(p, t); }, click: (p, ev) => { markSpecialty(null); handlers.click(p, ev); } });
    });
    return el;
  }

  // light up where a specialist does their best work
  function markSpecialty(prof) {
    $$('.specialty').forEach(e => e.classList.remove('specialty'));
    if (prof === null || prof === undefined || !view) return;
    const info = profInfo(prof);
    if (info.kind === 'land') {
      const free = view.squares.filter(sq => !sq.taken && !sq.village && sq.yields[prof] > 0 && !view.workers.some(w => w.job.dir === sq.dir));
      const best = Math.max(0, ...free.map(sq => sq.yields[prof]));
      free.filter(sq => sq.yields[prof] === best).forEach(sq => { const c = $(`.sq[data-dir="${sq.dir}"]`); if (c) c.classList.add('specialty'); });
    } else if (info.kind === 'shop') {
      const c = $(`.bcard[data-job="${prof}"]`);
      if (c) c.classList.add('specialty');
    }
  }
  window.addEventListener('pointerup', () => setTimeout(() => { if (!Drag.active() && cid !== null) markSpecialty(sel && view ? (view.workers.concat(view.units).find(u => u.id === sel) || {}).prof : null); }, 0));

  function clickSquare(dir) {
    const w = view.workers.find(w => w.job.dir === dir);
    if (sel && (!w || w.id !== sel)) { act({ cmd: 'assign', colony: cid, unit: sel, job: { dir } }); return; }
    if (w && w.id === sel) {
      // cycle what this colonist grows here
      const sq = view.squares[dir];
      const jobs = sq.yields.map((n, j) => [n, j]).filter(([n]) => n > 0).map(([, j]) => j);
      if (jobs.length > 1) {
        const next = jobs[(jobs.indexOf(w.job.job) + 1) % jobs.length];
        act({ cmd: 'assign', colony: cid, unit: w.id, job: { dir, job: next } });
      }
    }
  }

  const handlers = {
    click(p) {
      if (p.type === 'unit') { sel = sel === p.id ? null : p.id; render(); }
      else if (p.type === 'cargo') act({ cmd: 'unload', carrier: p.carrier, slot: p.slot });
      else if (p.type === 'good') {
        const carriers = view.units.filter(u => isCarrier(u.kind));
        const target = carriers.find(u => u.id === sel) || (carriers.length === 1 ? carriers[0] : null);
        if (target) act({ cmd: 'load_goods', colony: cid, carrier: target.id, good: p.g });
        else { status = 'Drag goods onto a ship or wagon train to load them.'; render(); }
      }
    },
    hover(p, t) {
      const slot = $('.preview', t);
      if (p.type !== 'unit' || !slot) return;
      let job = null;
      if (t.dataset.drop === 'square') job = { dir: +t.dataset.dir };
      if (t.dataset.drop === 'building') job = { building: +t.dataset.job };
      if (!job) return;
      const d = preview(p.id, job);
      slot.innerHTML = '';
      slot.append(deltaChips(d));
      // mirror the change in the production list
      if (d.delta) $$('.prow').forEach(r => {
        const g = +r.dataset.good; const n = d.delta[g];
        r.classList.toggle('changing', !!n);
        r.dataset.delta = n ? signed(n) : '';
      });
    },
    leave(t) {
      const slot = $('.preview', t); if (slot) slot.innerHTML = '';
      $$('.prow').forEach(r => { r.classList.remove('changing'); r.dataset.delta = ''; });
    },
    drop(p, t) {
      const kind = t.dataset.drop;
      if (p.type === 'unit') {
        if (kind === 'square') act({ cmd: 'assign', colony: cid, unit: p.id, job: { dir: +t.dataset.dir } });
        else if (kind === 'building') act({ cmd: 'assign', colony: cid, unit: p.id, job: { building: +t.dataset.job } });
        else if (kind === 'outside') act({ cmd: 'step_out', unit: p.id });
        else if (kind === 'carrier') act({ cmd: 'board', unit: p.id, ship: +t.dataset.id });
      } else if (p.type === 'good' && kind === 'carrier') {
        act({ cmd: 'load_goods', colony: cid, carrier: +t.dataset.id, good: p.g });
      } else if (p.type === 'cargo' && kind === 'warehouse') {
        act({ cmd: 'unload', carrier: p.carrier, slot: p.slot });
      }
    },
  };

  function rename() {
    Dialogs.ask('Rename colony', 'Give the colony a new name:', view.name, name => act({ cmd: 'rename', colony: cid, name }));
  }

  return { open, close, isOpen, render, rename, id: () => cid };
})();
