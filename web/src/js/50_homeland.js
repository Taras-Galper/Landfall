// The homeland: market, docks, ships in port, recruiting and training.
const Homeland = (() => {
  let ship = null, dockSel = null, status = '', open_ = false, tab = 'recruit';
  const dests = {};

  function open() {
    if (App.st.rebellion) { toast(`${App.meta.homeland.replace(/^the /, 'The ')} is closed to the rebels.`); return; }
    open_ = true; status = '';
    App.screen = 'homeland';
    render();
  }
  function close() {
    open_ = false;
    $('#sheet').hidden = true; $('#sheet').innerHTML = '';
    App.screen = 'map';
    App.refresh();
  }
  function act(cmd) {
    const r = App.act(cmd, { quiet: true });
    status = r.error || r.msg || '';
    render();
    return r;
  }

  function render() {
    if (!open_) return;
    const r = Engine.call({ cmd: 'homeland' });
    const e = r.data, m = App.meta, st = App.st;
    const mine = st.units.filter(u => u.nation === st.human);
    const inPort = mine.filter(u => isShip(u.kind) && u.loc.type === 'homeland');
    const sailing = mine.filter(u => u.loc.type === 'sailing');
    const docks = mine.filter(u => !isShip(u.kind) && u.loc.type === 'homeland');
    if (!inPort.some(s => s.id === ship)) ship = inPort.length ? inPort[0].id : null;
    if (!docks.some(u => u.id === dockSel)) dockSel = null;

    const sheet = $('#sheet');
    sheet.hidden = false; sheet.innerHTML = '';
    const title = m.homeland.replace(/^the /, 'The ');
    sheet.append(h('header', { class: 'sheet-head' },
      h('h2', {}, title),
      h('span', { class: 'chip' }, `Gold ${e.gold.toLocaleString()}`),
      h('span', { class: 'chip', title: 'Taken from every sale' }, `Tax ${e.tax}%`),
      h('div', { class: 'spacer' }),
      status ? h('span', { class: 'status' }, status) : null,
      h('button', { class: 'btn primary', onclick: close, title: 'Back to the map (Esc)' }, 'Done')));

    // market
    const sel = inPort.find(s => s.id === ship);
    const market = h('section', { class: 'market' }, h('h3', {}, sel ? `Market: click a good to buy 100 into the ${sel.name}, right-click to choose how many, or drag cargo here to sell` : 'Market'));
    const row = h('div', { class: 'mrow', 'data-drop': 'market' });
    for (const g of e.market) {
      const cell = h('button', { class: 'mgood' + (g.boycott ? ' boycott' : ''), title: g.boycott ? `${g.name} are boycotted. Click to pay ${g.arrears} gold in back taxes.` : `${g.name}: they pay ${g.bid}, they sell at ${g.ask}` },
        goodChip(g.good), h('span', { class: 'mname' }, g.name), h('span', { class: 'price' }, h('b', {}, g.bid), '/', g.ask));
      cell.addEventListener('contextmenu', ev => {
        ev.preventDefault();
        if (g.boycott || !ship) return;
        const sh = inPort.find(x => x.id === ship);
        const part = sh.cargo.find(([cg, cn]) => cg === g.good && cn < 100);
        const room = part ? 100 - part[1] : sh.holds_used < sh.holds ? 100 : 0;
        const afford = Math.floor(e.gold / Math.max(1, g.ask));
        if (!room) { status = 'The ship\'s holds are full.'; render(); return; }
        if (!afford) { status = 'Not enough gold.'; render(); return; }
        Amount.pick({ anchor: cell, title: `Buy ${g.name}`, max: Math.min(room, afford), verb: 'Buy', price: k => `Costs ${k * g.ask} gold`, onOk: k => act({ cmd: 'buy', ship, good: g.good, amount: k }) });
      });
      cell.addEventListener('click', () => {
        if (g.boycott) return Dialogs.confirm(`Lift the boycott on ${g.name}?`, `Pay ${g.arrears} gold in back taxes to trade ${g.name} again.`, `Pay ${g.arrears} gold`, () => act({ cmd: 'pay_arrears', good: g.good }));
        if (!ship) { status = 'No ship in port to load.'; return render(); }
        act({ cmd: 'buy', ship, good: g.good });
      });
      row.append(cell);
    }
    market.append(row);

    // ships
    const port = h('section', { class: 'port' }, h('h3', {}, 'In port'));
    if (!inPort.length) port.append(h('p', { class: 'sub' }, sailing.length ? 'Your ships are at sea.' : 'No ships. You can buy one under Purchase.'));
    for (const s of inPort) {
      const card = h('div', { class: 'carrier' + (s.id === ship ? ' sel' : ''), 'data-drop': 'ship', 'data-id': s.id });
      const ports = st.colonies.filter(c => c.nation === st.human && c.coastal);
      const dest = h('select', { 'aria-label': 'Where to sail', title: 'Where the ship goes once it reaches the New World', onclick: ev => ev.stopPropagation() },
        h('option', { value: '' }, 'to the sea lanes'), ports.map(c => h('option', { value: c.id, selected: dests[s.id] == c.id }, `to ${c.name}`)));
      dest.addEventListener('change', () => { dests[s.id] = dest.value; });
      card.append(h('div', { class: 'chead' }, unitToken(s, 30), h('b', {}, s.name), h('span', { class: 'sub' }, `${s.holds_used}/${s.holds} holds`)),
        h('div', { class: 'row sail' },
          h('button', { class: 'btn primary small', onclick: ev => { ev.stopPropagation(); act({ cmd: 'sail_new_world', ship: s.id, colony: dest.value ? +dest.value : undefined }); } }, 'Sail to the New World'),
          ports.length ? dest : null));
      const holds = h('div', { class: 'holds' });
      s.cargo.forEach(([g, n], slot) => {
        const chip = goodChip(g, n);
        chip.classList.add('cargo-chip');
        chip.title = `Sell ${n} ${m.goods[g]} for about ${n * e.market[g].bid} gold before tax. Click or drag to the market; right-click to choose how many.`;
        chip.addEventListener('pointerdown', ev => Drag.start(ev, { type: 'cargo', ship: s.id, slot }, chip, handlers));
        chip.addEventListener('contextmenu', ev => { ev.preventDefault(); Amount.pick({ anchor: chip, title: `Sell ${m.goods[g]}`, max: n, value: n, verb: 'Sell',
          price: k => `About ${Math.round(k * e.market[g].bid * (100 - e.tax) / 100)} gold after tax`, onOk: k => act({ cmd: 'sell', ship: s.id, slot, amount: k }) }); });
        holds.append(chip);
      });
      for (const p of st.units.filter(x => x.loc && x.loc.type === 'aboard' && x.loc.ship === s.id)) {
        const t = unitToken(p, 30); t.classList.add('person'); t.title = p.name + ': click to put back on the docks';
        t.addEventListener('click', ev => { ev.stopPropagation(); act({ cmd: 'disembark', unit: p.id }); });
        holds.append(t);
      }
      for (let i = s.holds_used; i < s.holds; i++) holds.append(h('span', { class: 'hold empty' }));
      card.append(holds);
      card.addEventListener('click', () => { ship = s.id; render(); });
      port.append(card);
    }
    if (sailing.length) port.append(h('div', { class: 'sailing' }, h('h3', {}, 'At sea'),
      sailing.map(s => h('div', { class: 'row' }, unitToken(s, 24), h('span', {}, `${s.name}: ${s.loc.to_homeland ? 'arriving here' : 'reaching the New World'} in ${plural(s.loc.turns, 'turn')}`)))));

    // docks
    const dk = h('section', { class: 'docks' }, h('h3', {}, 'On the docks'));
    const tokens = h('div', { class: 'tokens' });
    for (const u of docks) {
      const t = unitToken(u, 40);
      const wrap = h('span', { class: 'person' + (u.id === dockSel ? ' sel' : ''), title: `${u.name}. ${profInfo(u.prof).hint} Drag onto a ship to board, or click to select.` }, t, profTag(u));
      wrap.addEventListener('pointerdown', ev => Drag.start(ev, { type: 'unit', id: u.id }, wrap, handlers));
      tokens.append(wrap);
    }
    if (!docks.length) tokens.append(h('p', { class: 'sub' }, 'Nobody is waiting. Recruit or train colonists to fill your ships.'));
    dk.append(tokens);
    const du = docks.find(u => u.id === dockSel);
    if (du) dk.append(h('div', { class: 'row' }, h('b', {}, du.name),
      ship ? h('button', { class: 'btn', onclick: () => act({ cmd: 'board', unit: du.id, ship }) }, 'Board ship') : null,
      isColonistKind(du.kind) ? h('button', { class: 'btn', onclick: () => Dialogs.equip(du, e.equip, k => act({ cmd: 'equip', unit: du.id, kind: k })) }, 'Equip…') : null));

    // recruit / train / purchase
    const shop = h('section', { class: 'shop' });
    const tabs = h('div', { class: 'tabs', role: 'tablist' }, ['recruit', 'train', 'purchase'].map(t =>
      h('button', { role: 'tab', 'aria-selected': String(tab === t), class: tab === t ? 'on' : '', onclick: () => { tab = t; render(); } }, { recruit: 'Recruit', train: 'Train', purchase: 'Purchase' }[t])));
    shop.append(tabs);
    const list = h('div', { class: 'shop-list' });
    if (tab === 'recruit') {
      list.append(h('p', { class: 'sub' }, `Each recruit costs ${e.recruit_price} gold. Crosses bring one free: ${e.crosses}/${e.crosses_needed}.`));
      e.recruits.forEach((r2, i) => list.append(h('button', { class: 'shop-item', disabled: e.gold < e.recruit_price, onclick: () => act({ cmd: 'recruit', slot: i }) }, h('span', {}, r2.name), h('b', {}, e.recruit_price))));
    } else if (tab === 'train') {
      for (const t of e.train) list.append(h('button', { class: 'shop-item', disabled: e.gold < t.price, onclick: () => act({ cmd: 'train', prof: t.prof }) }, h('span', {}, t.name), h('b', {}, t.price.toLocaleString())));
    } else {
      for (const p of e.purchase) list.append(h('button', { class: 'shop-item', disabled: e.gold < p.price, onclick: () => act({ cmd: 'buy_unit', kind: p.kind }) }, h('span', {}, p.name), h('b', {}, p.price.toLocaleString())));
    }
    shop.append(list);

    sheet.append(h('div', { class: 'homeland-body' }, market, port, dk, shop));
  }

  const handlers = {
    click(p) {
      if (p.type === 'unit') { dockSel = dockSel === p.id ? null : p.id; render(); }
      if (p.type === 'cargo') act({ cmd: 'sell', ship: p.ship, slot: p.slot });
    },
    drop(p, t) {
      if (p.type === 'unit' && t.dataset.drop === 'ship') act({ cmd: 'board', unit: p.id, ship: +t.dataset.id });
      if (p.type === 'cargo' && t.dataset.drop === 'market') act({ cmd: 'sell', ship: p.ship, slot: p.slot });
    },
  };

  return { open, close, render, isOpen: () => open_ };
})();
