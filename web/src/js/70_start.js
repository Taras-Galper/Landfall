// The start screen: continue, load, or set up a new game.
const Start = (() => {
  const pick = { nation: 0, difficulty: 1, land: 1, shape: 1, temperature: 1, climate: 1 };

  function seg(label, key, values, hint) {
    return h('div', { class: 'seg-row' }, h('span', { class: 'k' }, label),
      h('div', { class: 'seg', role: 'radiogroup', 'aria-label': label }, values.map((v, i) =>
        h('button', { role: 'radio', 'aria-checked': String(pick[key] === i), class: pick[key] === i ? 'on' : '', title: hint ? hint[i] : '', onclick: () => { pick[key] = i; render(); } }, v))));
  }

  function render() {
    const m = App.meta;
    const el = $('#start');
    el.innerHTML = '';
    const auto = store('lf.auto');
    const hero = h('div', { class: 'hero' },
      h('h1', {}, 'Landfall'),
      h('p', { class: 'lede' }, 'Cross the ocean, found colonies, trade with the homeland and its rivals, live alongside the peoples of a new continent, and win your independence.'),
      h('div', { class: 'row' },
        auto ? h('button', { class: 'btn primary big', onclick: () => App.loadFrom('lf.auto') }, `Continue: ${store('lf.auto.meta') || 'last game'}`) : null,
        h('button', { class: 'btn ghost', onclick: () => Dialogs.loadMenu() }, 'Load a saved game')));
    const nations = h('div', { class: 'nations' });
    for (let n = 0; n < 4; n++) {
      nations.append(h('button', { class: 'nation' + (pick.nation === n ? ' on' : ''), 'aria-pressed': String(pick.nation === n), style: { '--c': NATION_COLORS[n] }, onclick: () => { pick.nation = n; render(); } },
        h('span', { class: 'flag big' }), h('b', {}, m.nations[n]), h('span', { class: 'leader' }, m.leaders[n]), h('span', { class: 'trait' }, NATION_TRAITS[n])));
    }
    const form = h('div', { class: 'newgame' },
      h('h2', {}, 'New game'),
      h('h3', {}, 'Sail for'), nations,
      h('h3', {}, 'The New World'),
      seg('Land', 'land', ['Small', 'Moderate', 'Large']),
      seg('Shape', 'shape', ['Islands', 'Continent']),
      seg('Climate', 'temperature', ['Cool', 'Temperate', 'Warm']),
      seg('Rainfall', 'climate', ['Dry', 'Normal', 'Wet']),
      h('h3', {}, 'Difficulty'),
      seg('Level', 'difficulty', m.difficulties, ['1000 gold to start, gentle Crown', '600 gold', '300 gold', '100 gold', 'No gold, a harsh Crown']),
      h('button', { class: 'btn primary big', onclick: begin }, `Set sail with ${m.nations[pick.nation]}`));
    el.append(h('div', { class: 'start-inner' }, hero, form));
    el.hidden = false;
  }

  function begin() {
    const fixed = parseInt(new URLSearchParams(location.search).get('seed'), 10);
    const seed = Number.isFinite(fixed) ? fixed : (Date.now() ^ Math.floor(Math.random() * 1e9)) >>> 0;
    const r = App.act({ cmd: 'new_game', seed, nation: pick.nation, difficulty: pick.difficulty,
      world: { land: pick.land, shape: pick.shape, temperature: pick.temperature, climate: pick.climate } }, { quiet: true, fresh: true });
    if (!r.ok) { toast(r.error, 'bad'); return; }
    hide();
    App.started();
  }
  function hide() { $('#start').hidden = true; }
  return { show: () => { App.screen = 'start'; render(); }, hide };
})();
