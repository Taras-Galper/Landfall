// Shared helpers, colours and the placeholder icon set.
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') {
      for (const [sk, sv] of Object.entries(v)) {
        if (sk.startsWith('--')) el.style.setProperty(sk, sv);
        else el.style[sk] = sv;
      }
    }
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids.flat(Infinity)) {
    if (kid === null || kid === undefined || kid === false) continue;
    el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const plural = (n, one, many) => `${n} ${n === 1 ? one : (many || one + 's')}`;
const signed = n => (n > 0 ? '+' : '') + n;

const NATION_COLORS = ['#B8423A', '#3D6FB6', '#D6A21E', '#E07A2E',
  '#9C6B2F', '#3F8F6B', '#4D8FAF', '#7F6A9E', '#A35C4D', '#B48A4A', '#6F7F3A', '#5A7F6E', '#E8E8EE'];

const NATION_TRAITS = [
  'Dissent at home sends settlers your way: new colonists arrive sooner.',
  'Trusted by the native peoples: your colonies alarm them half as much.',
  'Hardened soldiers: your troops fight native warriors half again as hard.',
  'Born traders: you start with a merchantman, and your markets recover quickly.',
];

const GOOD_COLORS = ['#9BBF4A', '#E6D8BC', '#9A7440', '#F2F0E6', '#8A6446', '#B4834F', '#7D8992', '#C9D3DC',
  '#9A653F', '#C0703A', '#6E4C2C', '#D9CBA7', '#6A4E3B', '#CFAA52', '#66727C', '#4A5158',
  '#C08850', '#E2BE55', '#DADCF0', '#7FB0D0'];

// Placeholder pictograms, 24x24. Real art comes later.
const ICON = {
  person: 'M12 2.5a3.2 3.2 0 1 1 0 6.4a3.2 3.2 0 1 1 0-6.4zM5.5 21.5v-4.2c0-3.3 2.9-5.6 6.5-5.6s6.5 2.3 6.5 5.6v4.2z',
  musket: 'M14.6 13.4L20.6 1.6l1.4.7-6 11.8zM20.2 1l1.2-.6.5 1-1.2.6z',
  tool: 'M12.6 5.2c2.6-2.6 6.6-3.5 9.9-2.3l-.5 1.5c-1.4-.5-2.9-.6-4.3-.3L15 14.2h-1.8l2.6-9.6c-.8.4-1.6.9-2.2 1.5z',
  cross: 'M17.6 1.5h1.6v3h2.6v1.6h-2.6v6h-1.6v-6H15V4.5h2.6z',
  feather: 'M15.5 1.2c2 .8 2.6 3 1.6 6.3l-1.6-.3c.6-2.4.3-4-.8-4.9z',
  horse: 'M4 21.5l1.7-6.3 3.2-2.8.8-5.2 2.6-2.2 1.2 1.6 2.8 1.4 3.7 3.2-1.4 1.8-3-1.6-1.2 2.6V21.5h-2.2v-4.6H9.8l-1.4 4.6z',
  ship: 'M2 15.5h20l-3.4 5H5.4zM11.2 2.5h1.6v12h-1.6zM13.4 3.6l6 9.4h-6zM10.6 4.8l-6 8.2h6z',
  wagon: 'M2.5 8h15v7.5h-15zM17.5 11h4.5v1.6h-4.5zM4 18.5a2.3 2.3 0 1 0 4.6 0a2.3 2.3 0 1 0-4.6 0zM11.5 18.5a2.3 2.3 0 1 0 4.6 0a2.3 2.3 0 1 0-4.6 0z',
  cannon: 'M2.5 12.6l14.2-5.4 1.2 3-14.2 5.4zM16.5 6.5l3 -1 1.3 3.2-3 1.1zM6.5 17.5a3.2 3.2 0 1 0 6.4 0a3.2 3.2 0 1 0-6.4 0z',
  chest: 'M3.5 11h17v9.5h-17zM3.5 10c0-3.4 3.6-5.5 8.5-5.5s8.5 2.1 8.5 5.5zM10.8 12.5h2.4v3h-2.4z',
  colony: 'M2.5 12l5.5-5.5 5.5 5.5v8.5h-11zM12.5 13.5l4.5-4.5 4.5 4.5v7h-9zM6.5 15.5h3v5h-3z',
  village: 'M12 2.5l9.5 18h-19zM12 13l3 7.5H9z',
  shield: 'M12 2l8 3v6c0 5.2-3.4 9.2-8 11-4.6-1.8-8-5.8-8-11V5z',
  eye: 'M12 6c4.6 0 8 3.6 9.5 6-1.5 2.4-4.9 6-9.5 6s-8-3.6-9.5-6C4 9.6 7.4 6 12 6zm0 3a3 3 0 1 0 0 6a3 3 0 1 0 0-6z',
  road: 'M9 2h2l-1.5 20h-3zM13 2h2l2.5 20h-3zM11.4 5h1.2v3h-1.2zM11.4 11h1.2v3h-1.2zM11.4 17h1.2v3h-1.2z',
  plow: 'M3 18h18v1.8H3zM3 13.5h18v1.8H3zM3 9h18v1.8H3zM3 4.5h18v1.8H3z',
  anchor: 'M11 2.5h2v3.2h3v1.8h-3v10.4c2.2-.3 4-1.6 4.8-3.6l-2-.6 4.2-2.6 1 4.6-1.6-.6c-1.2 3.4-4.4 5.4-8.4 5.4s-7.2-2-8.4-5.4l-1.6.6 1-4.6 4.2 2.6-2 .6c.8 2 2.6 3.3 4.8 3.6V7.5H8V5.7h3z',
  bell: 'M12 2.5c.8 0 1.4.6 1.4 1.3 2.8.6 4.6 3.1 4.6 6.2v4.5l2 2.5v1H4v-1l2-2.5V10c0-3.1 1.8-5.6 4.6-6.2 0-.7.6-1.3 1.4-1.3zM9.8 19h4.4a2.2 2.2 0 0 1-4.4 0z',
};

function unitIcon(kind) {
  // returns list of path keys composing the unit's pictogram
  switch (kind) {
    case 0: return ['person'];
    case 1: case 6: case 9: return ['person', 'musket'];
    case 2: return ['person', 'tool'];
    case 3: return ['person', 'cross'];
    case 4: case 7: case 8: return ['horse', 'musket'];
    case 5: return ['horse'];
    case 10: return ['chest'];
    case 11: return ['cannon'];
    case 12: return ['wagon'];
    case 13: case 14: case 15: case 16: case 17: case 18: return ['ship'];
    case 19: return ['person', 'feather'];
    case 20: return ['person', 'feather', 'musket'];
    case 21: return ['horse', 'feather'];
    case 22: return ['horse', 'feather', 'musket'];
    default: return ['person'];
  }
}
const isShip = k => k >= 13 && k <= 18;
const isCarrier = k => isShip(k) || k === 12;
const isColonistKind = k => k <= 5;

function svgIcon(keys, cls = 'ico') {
  const ks = Array.isArray(keys) ? keys : [keys];
  return h('span', { class: cls, html: `<svg viewBox="0 0 24 24" aria-hidden="true">${ks.map(k => `<path d="${ICON[k]}"/>`).join('')}</svg>` });
}

const PATH_CACHE = {};
function iconPath(k) {
  return PATH_CACHE[k] || (PATH_CACHE[k] = new Path2D(ICON[k]));
}

// Professions: a short code and colour for each, so specialists stand out
// before there is real art. Experts take the colour of what they produce.
// Codes follow the profession numbers used by the engine.
const PROF_INFO = (() => {
  const G = GOOD_COLORS;
  const land = (abbr, short, g, what) => ({ abbr, short, color: G[g], hint: `Produces twice as much ${what} when working the land.`, kind: 'land', good: g });
  const shop = (abbr, short, g, what) => ({ abbr, short, color: G[g], hint: `Makes twice as much ${what} in their building (6 instead of 3).`, kind: 'shop', good: g });
  return {
    0: land('Fa', 'Farmer', 0, 'food from farming'),
    1: land('Su', 'Sugar planter', 1, 'sugar'),
    2: land('To', 'Tobacco planter', 2, 'tobacco'),
    3: land('Co', 'Cotton planter', 3, 'cotton'),
    4: land('Tr', 'Fur trapper', 4, 'furs'),
    5: land('Lj', 'Lumberjack', 5, 'lumber'),
    6: land('Om', 'Ore miner', 6, 'ore'),
    7: land('Sm', 'Silver miner', 7, 'silver'),
    8: { ...land('Fi', 'Fisherman', 0, 'food'), hint: 'Catches twice as many fish on ocean squares. The colony needs docks.' },
    9: shop('Di', 'Distiller', 9, 'rum'),
    10: shop('Tb', 'Tobacconist', 10, 'cigars'),
    11: shop('We', 'Weaver', 11, 'cloth'),
    12: shop('Ft', 'Fur trader', 12, 'coats'),
    13: shop('Ca', 'Carpenter', 16, 'hammers'),
    14: shop('Bs', 'Blacksmith', 14, 'tools'),
    15: shop('Gs', 'Gunsmith', 15, 'muskets'),
    16: shop('Pr', 'Preacher', 18, 'crosses'),
    17: shop('St', 'Statesman', 17, 'liberty bells'),
    18: { abbr: 'Le', short: 'Student', color: G[19], hint: 'Learning a trade in the schoolhouse.', kind: 'other' },
    20: { abbr: 'Pi', short: 'Hardy pioneer', color: G[14], hint: 'Clears forests, plows and builds roads faster. Equip with tools.', kind: 'field' },
    21: { abbr: 'Ve', short: 'Veteran soldier', color: G[15], hint: 'Fights half again as hard as a soldier or dragoon. Equip with muskets.', kind: 'field' },
    22: { abbr: 'Sc', short: 'Seasoned scout', color: G[8], hint: 'A better explorer and envoy. Equip with horses.', kind: 'field' },
    23: { abbr: 'Vd', short: 'Veteran dragoon', color: '#5A6A8A', hint: 'Fights half again as hard as a dragoon.', kind: 'field' },
    24: { abbr: 'Ms', short: 'Missionary', color: G[18], hint: 'Converts natives faster at a mission.', kind: 'field' },
    25: { abbr: 'Sv', short: 'Servant', color: '#8D7B68', hint: 'Normal output on the land, but only 2 in buildings. A schoolhouse can teach them.', kind: 'low' },
    26: { abbr: 'Cr', short: 'Criminal', color: '#7A5555', hint: 'Normal output on the land, but only 1 in buildings. A schoolhouse can teach them.', kind: 'low' },
    27: { abbr: 'Cv', short: 'Convert', color: '#4E8F86', hint: 'One extra on any land job, but only 1 in buildings.', kind: 'low' },
  };
})();
const FREE_INFO = { abbr: '', short: 'Free colonist', color: '', hint: 'No specialty: normal output on the land, 3 in buildings. Can be taught.', kind: 'free' };
const profInfo = p => PROF_INFO[p] || FREE_INFO;

function unitToken(u, size = 34) {
  const color = NATION_COLORS[u.nation] || '#999';
  const info = isColonistKind(u.kind) && u.prof !== undefined ? profInfo(u.prof) : null;
  const title = info ? `${u.name}. ${info.hint}` : u.name;
  const el = h('span', { class: 'token' + (isShip(u.kind) ? ' ship' : ''), style: { '--c': color, width: size + 'px', height: size + 'px' }, title });
  el.append(svgIcon(unitIcon(u.kind)));
  if (info && info.abbr) el.append(h('span', { class: 'pbadge', style: { '--p': info.color } }, info.abbr));
  return el;
}

// Name tag shown under a colonist where there's room.
function profTag(u) {
  if (!isColonistKind(u.kind)) return null;
  const info = profInfo(u.prof);
  return h('span', { class: 'ptag' + (info.abbr ? '' : ' plain'), style: info.color ? { '--p': info.color } : {} }, info.short);
}

const GOOD_ABBR = ['Fd', 'Su', 'To', 'Co', 'Fu', 'Lu', 'Or', 'Si', 'Ho', 'Ru', 'Ci', 'Cl', 'Ct', 'TG', 'Tl', 'Mu', 'Hm', 'Be', 'Cr', 'Te'];
function goodChip(g, label) {
  const name = (App.meta?.goods || [])[g] || '';
  return h('span', { class: 'good', style: { '--g': GOOD_COLORS[g] }, title: name }, h('i', {}, GOOD_ABBR[g]), label !== undefined ? h('b', {}, label) : null);
}

function toast(text, kind = '') {
  if (!text) return;
  const t = h('div', { class: 'toast ' + kind }, text);
  const box = $('#toasts');
  while (box.children.length >= 3) box.firstChild.remove();
  box.append(t);
  setTimeout(() => t.classList.add('out'), 3800);
  setTimeout(() => t.remove(), 4400);
}

// localStorage wrapped: reads return the value or null, writes return true on success.
function store(key, val) {
  try {
    if (val === undefined) return localStorage.getItem(key);
    if (val === null) localStorage.removeItem(key);
    else localStorage.setItem(key, val);
    return true;
  } catch (e) { return val === undefined ? null : false; }
}
