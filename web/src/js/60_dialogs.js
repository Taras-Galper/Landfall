// Modal dialogs: decisions, story moments, the assembly, reports, menu, saves.
const Dialogs = (() => {
  const queue = [];
  let showing = null;

  function show(d) {
    if (d.options) d.options = d.options.filter(Boolean);
    queue.push(d);
    if (!showing) next();
  }
  function next() {
    const el = $('#modal');
    showing = queue.shift() || null;
    if (!showing) { el.hidden = true; el.innerHTML = ''; App.afterDialogs(); return; }
    const d = showing;
    el.innerHTML = '';
    el.hidden = false;
    const box = h('div', { class: 'dialog ' + (d.cls || ''), role: 'dialog', 'aria-modal': 'true', 'aria-label': d.title });
    if (d.kicker) box.append(h('p', { class: 'kicker' }, d.kicker));
    box.append(h('h2', {}, d.title));
    for (const b of [].concat(d.body || [])) box.append(typeof b === 'string' ? h('p', {}, b) : b);
    const opts = h('div', { class: 'options' + (d.stack ? ' stack' : '') });
    (d.options || [{ label: 'Continue', primary: true }]).forEach((o, i) => {
      const b = h('button', { class: 'btn' + (o.primary ? ' primary' : '') + (o.danger ? ' danger' : ''), disabled: o.disabled, title: o.title || '' }, o.label);
      b.addEventListener('click', () => close(o));
      opts.append(b);
    });
    box.append(opts);
    el.append(box);
    const first = $('button.primary', opts) || $('button', opts);
    if (first) first.focus();
    d.cancel = d.cancel || (d.options || []).find(o => o.cancel) || (d.options && d.options.length === 1 ? d.options[0] : null) || (!d.options ? {} : null);
  }
  function close(o) {
    const d = showing;
    if (d && d.onclose) d.onclose();
    showing = null;
    if (o && o.fn) o.fn();
    if (!showing) next();
  }
  function cancel() { if (showing && showing.cancel) close(showing.cancel); }
  const open = () => !!showing;

  function confirm(title, text, yes, fn) {
    show({ title, body: [text], options: [{ label: yes, primary: true, fn }, { label: 'Cancel', cancel: true }] });
  }
  function ask(title, text, value, fn) {
    const input = h('input', { type: 'text', value, maxlength: 40, 'aria-label': title });
    input.addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); close({ fn: () => fn(input.value) }); } });
    show({ title, body: [text, input], options: [{ label: 'OK', primary: true, fn: () => fn(input.value) }, { label: 'Cancel', cancel: true }] });
    setTimeout(() => { input.focus(); input.select(); }, 0);
  }

  // --- prompts from moves
  function prompt(p) {
    const m = App.meta;
    switch (p.type) {
      case 'attack':
        show({ title: 'Attack?', body: [`Attack the ${p.target}? This means war.`, p.odds !== null && p.odds !== undefined ? h('p', { class: 'odds' }, `Chance of victory: ${Math.round(p.odds * 100)}%`) : null].filter(Boolean),
          options: [{ label: 'Attack', danger: true, primary: true, fn: () => App.act({ cmd: 'attack', unit: p.unit, x: p.x, y: p.y }) }, { label: 'Hold back', cancel: true }] });
        break;
      case 'sail_home':
        show({ title: 'The open sea', body: [`Your ship has reached the sea lanes. Sail on to ${p.homeland}?`],
          options: [{ label: `Sail to ${p.homeland.replace(/^the /, '')}`, primary: true, fn: () => App.act({ cmd: 'sail_home', unit: p.unit }) }, { label: 'Stay in the New World', cancel: true }] });
        break;
      case 'landfall':
        show({ title: 'Landfall', body: ['Send everyone aboard ashore here?'],
          options: [{ label: 'Go ashore', primary: true, fn: () => { App.act({ cmd: 'landfall', unit: p.unit, x: p.x, y: p.y }); App.selectAt(p.x, p.y); } }, { label: 'Stay aboard', cancel: true }] });
        break;
      case 'village': {
        const body = [`A ${p.tribe} ${p.capital ? 'capital' : 'village'} of ${p.pop}. The people seem ${p.mood}.`];
        if (p.teaches) body.push(`They can teach the skills of a ${p.teaches}.`);
        const options = p.actions.map(a => ({
          label: a.label + (a.action === 'attack' && p.odds !== null && p.odds !== undefined ? ` (${Math.round(p.odds * 100)}% chance)` : ''),
          danger: a.action === 'attack' || a.action === 'tribute', primary: a.action === p.actions[0].action && a.action !== 'attack',
          fn: () => App.act({ cmd: 'village', unit: p.unit, village: p.village, action: a.action }),
        }));
        if (!options.length) body.push('This unit has no business here.');
        options.push({ label: 'Leave', cancel: true });
        show({ title: `The ${p.tribe}`, body, options, stack: true });
        break;
      }
      case 'land_claim':
        show({ title: `${p.tribe} land`, body: [`The ${p.tribe} say this land is theirs. They ask ${p.price} gold for it. Taking it without payment will anger them.`],
          options: [{ label: `Pay ${p.price} gold`, primary: true, disabled: p.gold < p.price, fn: () => App.found(p.unit, 'pay') }, { label: 'Take the land', danger: true, fn: () => App.found(p.unit, 'take') }, { label: 'Cancel', cancel: true }] });
        break;
    }
  }

  // --- events from the turn
  function event(e) {
    const m = App.meta, st = App.st;
    switch (e.type) {
      case 'milestone':
        show({ cls: 'milestone', kicker: st.year_label, title: e.title, body: [e.text] }); break;
      case 'chief':
        show({ cls: 'speech', kicker: `The ${e.tribe}`, title: 'A leader speaks', body: [h('blockquote', {}, e.text)] }); break;
      case 'crown':
        show({ cls: 'crown', kicker: 'From the Crown', title: 'A royal message', body: [h('blockquote', {}, e.text)] }); break;
      case 'tax': {
        const t = st.tax_demand; if (!t) break;
        show({ cls: 'crown', kicker: 'From the Crown', title: `Taxes rise to ${t.rate}%`,
          body: [`The Crown raises the tax on all trade to ${t.rate}%.`, t.colony ? `If you refuse, the colonists of ${t.colony} will dump their ${t.good_name} in the harbour, and ${t.good_name} will be boycotted at home until you pay what is owed.` : 'Refusing will lead to a boycott.'],
          options: [{ label: 'Accept the tax', primary: true, fn: () => App.act({ cmd: 'tax', accept: true }) }, { label: 'Refuse', danger: true, fn: () => App.act({ cmd: 'tax', accept: false }) }] });
        break;
      }
      case 'native_trade':
        show({ kicker: e.colony_name, title: `${e.tribe_name} traders`, body: [`They offer ${e.price} gold for ${e.amount} ${e.good_name}.`],
          options: [{ label: `Sell for ${e.price}`, primary: true, fn: () => App.act({ cmd: 'native_trade', tribe: e.tribe, colony: e.colony, good: e.good, amount: e.amount, price: e.price }) },
            { label: 'Ask for more', title: 'They may pay a quarter more, or walk away', fn: () => App.act({ cmd: 'native_trade', tribe: e.tribe, colony: e.colony, good: e.good, amount: e.amount, price: e.price, haggle: true }) },
            { label: 'Decline', cancel: true }] });
        break;
      case 'native_demand':
        show({ kicker: e.colony_name, title: `The ${e.tribe_name} make demands`, body: [e.good_name ? `They demand ${e.amount} ${e.good_name}.` : `They demand ${e.amount} gold.`, 'Refusing will anger them.'],
          options: [{ label: 'Give it to them', primary: true, fn: () => App.act({ cmd: 'native_demand', tribe: e.tribe, colony: e.colony, good: e.good, amount: e.amount, give: true }) },
            { label: 'Refuse', danger: true, fn: () => App.act({ cmd: 'native_demand', tribe: e.tribe, colony: e.colony, good: e.good, amount: e.amount, give: false }) }] });
        break;
      case 'mercenaries':
        show({ title: 'Mercenaries', body: [`A company of veteran soldiers offers its services for ${e.price.toLocaleString()} gold.`],
          options: [{ label: `Hire them for ${e.price.toLocaleString()}`, primary: true, disabled: e.gold < e.price, fn: () => App.act({ cmd: 'mercenaries', hire: true, price: e.price }) }, { label: 'Send them away', cancel: true }] });
        break;
      case 'peace_offer':
        show({ title: 'An offer of peace', body: [`The ${e.name} propose an end to the war.`],
          options: [{ label: 'Make peace', primary: true, fn: () => App.act({ cmd: 'peace', nation: e.nation, accept: true }) }, { label: 'Fight on', cancel: true }] });
        break;
      case 'member':
        show({ cls: 'milestone', kicker: m.assembly, title: `${e.name} joins`, body: [e.effect] }); break;
      case 'declared':
        show({ cls: 'milestone', kicker: st.year_label, title: 'Independence declared', body: ['There is no turning back. The Crown\'s army is coming. Hold your colonies and defeat every regiment it sends.'] }); break;
      case 'war_over':
        show({ cls: 'milestone', title: e.won ? 'Victory' : 'Defeat', body: [e.won ? 'The Crown\'s army is beaten. A new nation is born.' : 'The rebellion has been crushed.'] }); break;
      case 'battle': {
        const who = e.won ? 'won' : 'lost';
        Log.add(`${e.attacker} attacked ${e.defender_name}: ${who} (${e.attack.toFixed(1)} vs ${e.defence.toFixed(1)}).`, e.won ? 'good' : 'bad');
        break;
      }
    }
  }

  // --- the assembly
  function assembly() {
    const st = App.st, m = App.meta, c = st.assembly;
    const body = [];
    if (c.candidate !== null && c.candidate !== undefined) {
      const f = m.members[c.candidate];
      body.push(h('div', { class: 'member cur' }, h('b', {}, f.name), h('span', {}, f.effect), h('span', { class: 'meter' }, h('i', { style: { width: Math.min(100, 100 * c.bells / Math.max(1, c.cost)) + '%' } })), h('small', {}, `${c.bells} of ${c.cost} liberty bells`)));
    } else if (c.offered.length) {
      body.push(h('p', {}, 'Choose who to work towards next. Liberty bells from your colonies bring them into the assembly.'));
    } else body.push(h('p', {}, 'Found a colony and ring liberty bells to bring great figures into the assembly.'));
    const kinds = ['Trade', 'Exploration', 'Military', 'Politics', 'Religion'];
    const options = [];
    if ((c.candidate === null || c.candidate === undefined) && c.offered.length) {
      for (const f of c.offered) options.push({ label: `${m.members[f].name} (${kinds[m.members[f].kind]}): ${m.members[f].effect}`, fn: () => App.act({ cmd: 'choose_member', member: f }), primary: false });
    }
    if (c.members.length) body.push(h('details', {}, h('summary', {}, `Members (${c.members.length})`), h('ul', { class: 'members' }, c.members.map(f => h('li', {}, h('b', {}, m.members[f].name), ' ', m.members[f].effect)))));
    body.push(h('p', { class: 'sub' }, `${st.rebel_percent}% of your colonists support independence.` + (st.can_declare ? ` ${st.can_declare}` : ' You can declare independence.')));
    if (!st.rebellion) options.push({ label: 'Declare independence…', danger: true, disabled: !!st.can_declare, fn: declare });
    options.push({ label: 'Close', cancel: true });
    show({ title: m.assembly, body, options, stack: true, cls: 'wide' });
  }
  function declare() {
    const st = App.st;
    show({ title: 'Declare independence?', body: [`The Crown will close ${App.meta.homeland} to you, seize what you have there, and send ${st.crown_units} regiments and ships to crush the rebellion. You win by defeating them.`],
      options: [{ label: 'Declare independence', danger: true, primary: true, fn: () => App.act({ cmd: 'declare' }) }, { label: 'Not yet', cancel: true }] });
  }

  function equip(u, prices, fn) {
    const k = App.meta.units;
    const list = prices || [
      { kind: 0, name: 'No equipment' }, { kind: 1, name: `${k[1]} (50 muskets)` }, { kind: 4, name: `${k[4]} (50 muskets, 50 horses)` },
      { kind: 2, name: `${k[2]} (20 to 100 tools)` }, { kind: 5, name: `${k[5]} (50 horses)` }, { kind: 3, name: `${k[3]} (needs a church)` },
    ];
    show({ title: `Equip ${u.name}`, options: [...list.map(o => ({ label: o.price ? `${o.name}: ${o.price.toLocaleString()} gold` : o.name, fn: () => fn(o.kind) })), { label: 'Cancel', cancel: true }], stack: true });
  }

  function route(u) {
    const r = Engine.call({ cmd: 'route_places', unit: u.id });
    const places = (r.data && r.data.places) || [];
    if (places.length < 2) { show({ title: 'Trade route', body: ['A route needs two stops this unit can reach: your colonies' + (isShip(u.kind) ? ', or the homeland.' : '.')] }); return; }
    const m = App.meta;
    const sel = (id) => h('select', { id }, places.map(p => h('option', { value: p.id }, p.name)));
    const a = sel('ra'), b = sel('rb'); b.selectedIndex = 1;
    const goods = (id) => h('div', { class: 'goods-pick', id }, m.goods.slice(0, 16).map((g, i) => h('label', { title: g }, h('input', { type: 'checkbox', value: i }), goodChip(i))));
    const ab = goods('rab'), ba = goods('rba');
    const pick = el => $$('input:checked', el).map(x => +x.value);
    const val = el => el.value === 'homeland' ? 'homeland' : +el.value;
    show({ title: `Trade route for the ${u.name}`, cls: 'wide',
      body: [u.route ? h('p', { class: 'sub' }, 'Current: ' + u.route) : null, h('div', { class: 'route-form' }, h('label', {}, 'From', a), h('label', {}, 'To', b),
        h('div', {}, h('span', { class: 'k' }, 'Carry there'), ab), h('div', {}, h('span', { class: 'k' }, 'Carry back'), ba))].filter(Boolean),
      options: [{ label: 'Start route', primary: true, fn: () => App.act({ cmd: 'set_route', unit: u.id, a: val(a), b: val(b), a_to_b: pick(ab), b_to_a: pick(ba) }) },
        u.route ? { label: 'Clear route', fn: () => App.act({ cmd: 'set_route', unit: u.id, clear: true }) } : null, { label: 'Cancel', cancel: true }].filter(Boolean) });
  }

  // where a ship can be sent: the homeland, or any coastal colony of ours
  function sendTo(u) {
    const st = App.st, m = App.meta;
    const turns = (x, y) => {
      const r = Engine.call({ cmd: 'path', unit: u.id, x, y });
      const p = r.data && r.data.path;
      return p && p.length ? p[p.length - 1][2] + 1 : null;
    };
    const opts = [];
    if (!st.rebellion) opts.push({ label: `${m.homeland.replace(/^the /, 'The ')}: sail to the sea lanes and cross`, primary: true,
      fn: () => App.act({ cmd: 'send_home', unit: u.id }, { keep: true }) });
    for (const c of st.colonies.filter(c => c.nation === st.human && c.coastal && !(c.x === u.x && c.y === u.y))) {
      const t = turns(c.x, c.y);
      opts.push({ label: `${c.name}${t ? `: ${plural(t, 'turn')}` : ': no known route'}`, disabled: !t,
        fn: () => App.act({ cmd: 'goto', unit: u.id, x: c.x, y: c.y }, { keep: true }) });
    }
    opts.push({ label: 'Cancel', cancel: true });
    show({ title: `Send the ${u.name} to…`, body: ['The ship sails on its own and stops if it meets land it cannot cross.'], options: opts, stack: true });
  }

  // what the profession colours and codes mean
  function professions() {
    const rows = Object.entries(PROF_INFO).map(([p, i]) => h('tr', {},
      h('td', {}, h('span', { class: 'pbadge inline', style: { '--p': i.color } }, i.abbr)),
      h('td', {}, h('b', {}, App.meta.professions[p] || i.short)), h('td', { class: 'sub' }, i.hint)));
    rows.push(h('tr', {}, h('td', {}), h('td', {}, h('b', {}, 'Free Colonist')), h('td', { class: 'sub' }, FREE_INFO.hint)));
    show({ title: 'Professions', cls: 'wide', body: [h('p', {}, 'Specialists carry a coloured badge with a short code. Land experts double what a square yields; craftsmen make 6 instead of 3 in their building. When you pick up a colonist in a colony, the squares or building where they work best light up.'),
      h('table', { class: 'keys profs' }, rows)] });
  }

  // --- menu and saves
  function menu() {
    show({ title: 'Menu', stack: true, options: [
      { label: 'Save game', fn: saveMenu, primary: true },
      { label: 'Load game', fn: loadMenu },
      { label: 'Foreign affairs', fn: diplomacy },
      { label: App.prefs.grid ? 'Hide map grid' : 'Show map grid', fn: () => { App.prefs.grid = !App.prefs.grid; App.savePrefs(); MapView.invalidate(); } },
      { label: 'Keys and controls', fn: help },
      { label: 'Professions', fn: professions },
      { label: 'New game', fn: () => confirm('Start a new game?', 'Unsaved progress since the last autosave will be lost.', 'New game', () => Start.show()) },
      { label: 'Close', cancel: true },
    ] });
  }
  function slotLabel(k) {
    const meta = store(k + '.meta');
    return meta ? meta : 'Empty';
  }
  function saveMenu() {
    const slots = [1, 2, 3].map(i => ({ label: `Slot ${i}: ${slotLabel('lf.slot' + i)}`, fn: () => App.saveTo('lf.slot' + i) }));
    show({ title: 'Save game', stack: true, options: [...slots, window.LF_EMBEDDED ? null : { label: 'Download save file', fn: () => App.download() }, { label: 'Cancel', cancel: true }] });
  }
  function loadMenu() {
    const keys = ['lf.auto', 'lf.slot1', 'lf.slot2', 'lf.slot3'];
    const opts = keys.filter(k => store(k)).map(k => ({ label: `${k === 'lf.auto' ? 'Autosave' : 'Slot ' + k.slice(-1)}: ${slotLabel(k)}`, fn: () => App.loadFrom(k) }));
    opts.push({ label: 'Open a save file…', fn: () => App.upload() });
    opts.push({ label: 'Cancel', cancel: true });
    show({ title: 'Load game', stack: true, options: opts });
  }
  function diplomacy() {
    const st = App.st;
    const opts = st.players.filter(p => p.nation !== st.human && p.known).map(p => p.at_war
      ? { label: `Offer peace to the ${p.adjective}`, fn: () => App.act({ cmd: 'propose_peace', nation: p.nation }) }
      : { label: `Declare war on the ${p.adjective}`, danger: true, fn: () => confirm(`War with the ${p.adjective}?`, 'Their ships and soldiers will attack you.', 'Declare war', () => App.act({ cmd: 'declare_war', nation: p.nation })) });
    show({ title: 'Foreign affairs', body: opts.length ? [] : ['You have not met another colonial power yet.'], stack: true, options: [...opts, { label: 'Close', cancel: true }] });
  }
  function help() {
    const rows = [['Click a unit', 'Select it (click again to cycle a stack)'], ['Click a square', 'Send the selected unit there'], ['Click your colony', 'Select a ship or wagon there to load it; click again to open the colony'], ['Arrows, numpad', 'Move one square'], ['Esc', 'Deselect, or open the menu'], ['Drag, wheel', 'Pan and zoom the map'], ['Tab', 'Next unit'], ['Space', 'Skip this unit'],
      ['B', 'Found or join a colony'], ['P / R', 'Plow / build a road'], ['F / S', 'Fortify / sentry'], ['T', 'Trade route'], ['U', 'Unload a ship or wagon in a colony'], ['G / E', 'Send a ship somewhere / home'], ['Right-click a good', 'Choose how many to load, unload, buy or sell'], ['C', 'Open colony'], ['H', 'Homeland'],
      ['Ctrl+Z', 'Undo the last move'], ['F2', 'Reports'], ['Enter', 'End turn'], ['+ / −', 'Zoom']];
    show({ title: 'Keys and controls', cls: 'wide', body: [h('table', { class: 'keys' }, rows.map(([k, d]) => h('tr', {}, h('td', {}, h('kbd', {}, k)), h('td', {}, d))))] });
  }

  return { show, prompt, event, sendTo, professions, assembly, confirm, ask, equip, route, menu, help, open, cancel, saveMenu, loadMenu };
})();

// Advisor reports.
const Reports = (() => {
  let tab = 'colonies';
  function open() { App.screen = 'reports'; render(); }
  function close() { $('#sheet').hidden = true; $('#sheet').innerHTML = ''; App.screen = 'map'; App.refresh(); }
  function render() {
    const r = Engine.call({ cmd: 'reports' }).data, m = App.meta, st = App.st;
    const sheet = $('#sheet'); sheet.hidden = false; sheet.innerHTML = '';
    const tabs = h('div', { class: 'tabs', role: 'tablist' }, [['colonies', 'Colonies'], ['natives', 'Native peoples'], ['rivals', 'Rivals'], ['military', 'Military'], ['crown', 'Independence']].map(([k, l]) =>
      h('button', { role: 'tab', 'aria-selected': String(tab === k), class: tab === k ? 'on' : '', onclick: () => { tab = k; render(); } }, l)));
    sheet.append(h('header', { class: 'sheet-head' }, h('h2', {}, 'Reports'), tabs, h('div', { class: 'spacer' }), h('button', { class: 'btn primary', onclick: close }, 'Done')));
    const body = h('div', { class: 'report' });
    if (tab === 'colonies') {
      if (!r.colonies.length) body.append(h('p', {}, 'No colonies yet. Land a colonist and press B to found one.'));
      else {
        const goods = [0, 5, 6, 14, 15, 1, 2, 3, 4, 9, 10, 11, 12, 8];
        const t = h('table', { class: 'grid' }, h('thead', {}, h('tr', {}, h('th', {}, 'Colony'), h('th', {}, 'Pop'), h('th', {}, 'Liberty'), goods.map(g => h('th', {}, goodChip(g))), h('th', {}, goodChip(16)), h('th', {}, goodChip(17)), h('th', {}, 'Building'))));
        const tb = h('tbody');
        for (const c of r.colonies) tb.append(h('tr', { onclick: () => { close(); Colony.open(c.id); } }, h('td', {}, h('b', {}, c.name)), h('td', {}, c.pop), h('td', {}, c.liberty + '%'),
          goods.map(g => h('td', { class: c.net[g] > 0 ? 'pos' : c.net[g] < 0 ? 'neg' : 'zero', title: `${m.goods[g]}: ${c.stock[g]} in store` }, c.net[g] ? signed(c.net[g]) : '·')),
          h('td', {}, c.hammers), h('td', {}, c.bells), h('td', {}, c.building || '—')));
        t.append(tb); body.append(t);
      }
    } else if (tab === 'natives') {
      if (!r.tribes.length) body.append(h('p', {}, 'You have not met any native peoples yet.'));
      for (const t of r.tribes) body.append(h('div', { class: 'rrow' }, h('b', {}, t.name), h('span', {}, t.extinct ? 'Destroyed' : `${plural(t.villages, 'settlement')}`),
        h('span', { class: 'meter alarm' }, h('i', { style: { width: t.alarm + '%' } })), h('span', {}, ['Content', 'Uneasy', 'Angry', 'Hostile'][Math.min(3, Math.floor(t.alarm / 25))])));
    } else if (tab === 'rivals') {
      for (const p of r.rivals) body.append(h('div', { class: 'rrow' }, h('span', { class: 'flag', style: { '--c': NATION_COLORS[p.nation] } }), h('b', {}, p.name),
        h('span', {}, p.known ? `${plural(p.colonies, 'colony', 'colonies')}, army strength ${p.strength}` : 'Not yet met'), h('span', { class: p.at_war ? 'neg' : '' }, p.at_war ? 'At war' : 'At peace')));
    } else if (tab === 'military') {
      body.append(h('p', {}, `Your army strength: ${r.my_strength}. The Crown holds ${r.crown[0]} regulars, ${r.crown[1]} cavalry, ${r.crown[3]} artillery and ${r.crown[2]} warships in reserve.`));
      for (const u of r.military) body.append(h('div', { class: 'rrow link', onclick: () => { close(); if (u.x !== null) MapView.centerOn(u.x, u.y); } }, h('b', {}, u.name), h('span', {}, u.x !== null ? `at ${u.x}, ${u.y}` : '')));
    } else {
      body.append(h('p', {}, `${r.rebels}% of your colonists support independence. You need at least 50% to declare it.`),
        h('p', {}, `Score so far: ${r.score}.`), h('button', { class: 'btn', onclick: () => { close(); Dialogs.assembly(); } }, App.meta.assembly));
    }
    sheet.append(body);
  }
  return { open, close, isOpen: () => App.screen === 'reports' };
})();
