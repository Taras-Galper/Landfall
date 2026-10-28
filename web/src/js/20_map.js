// The map: a zoomable, pannable chart drawn on a canvas.
const MapView = (() => {
  const cv = () => $('#map');
  let ctx, W = 0, H = 0, dpr = 1;
  let dirty = true;
  let hover = null;          // {x, y}
  let path = null;           // [[x, y, turn]]
  let pathKey = '';
  let drag = null;           // pointer drag state
  const BASE = ['#A7B096', '#D6C38E', '#BAC56F', '#CDC57A', '#8FB75E', '#ADB956', '#7F9B6D', '#5E7C5C'];
  const FOREST = ['#4E6A57', '#7E8A4E', '#4F7A3E', '#5F8A3C', '#3F6E47', '#4F7F35', '#486B4F', '#2F5E3E'];
  const OCEAN = '#2B5C70', SHALLOW = '#3A7488', LANE = '#24536A', ARCTIC = '#E4EBEE', FOG = '#0F1B24';

  function init() {
    ctx = cv().getContext('2d');
    resize();
    window.addEventListener('resize', resize);
    const c = cv();
    c.addEventListener('wheel', onWheel, { passive: false });
    c.addEventListener('pointerdown', onDown);
    c.addEventListener('pointermove', onMove);
    c.addEventListener('pointerup', onUp);
    c.addEventListener('pointerleave', () => { hover = null; path = null; Tooltip.hide(); dirty = true; });
    c.addEventListener('contextmenu', e => e.preventDefault());
    requestAnimationFrame(frame);
  }

  function resize() {
    const r = cv().getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    W = r.width; H = r.height;
    cv().width = Math.round(W * dpr); cv().height = Math.round(H * dpr);
    dirty = true;
  }

  const v = () => App.view;
  const T = () => App.view.T;
  function toTile(px, py) {
    return { x: Math.floor(v().x + px / T()), y: Math.floor(v().y + py / T()) };
  }
  function clampView() {
    const st = App.st; if (!st) return;
    const vw = (W - (W > 760 ? 330 : 0)) / T(), vh = H / T();
    v().x = Math.max(-2, Math.min(st.width - vw + 2, v().x));
    v().y = Math.max(-1, Math.min(st.height - vh + 1, v().y));
  }
  function centerOn(x, y, onlyIfOff = false) {
    const vw = (W - (W > 760 ? 330 : 0)) / T(), vh = (H - (W > 760 ? 0 : H * 0.46)) / T();
    if (onlyIfOff && x >= v().x + 1.5 && x < v().x + vw - 1.5 && y >= v().y + 1.5 && y < v().y + vh - 1.5) return;
    v().x = x + 0.5 - vw / 2; v().y = y + 0.5 - vh / 2;
    clampView(); dirty = true;
  }
  function zoomAt(px, py, f) {
    const old = T();
    const nt = Math.max(18, Math.min(110, old * f));
    const wx = v().x + px / old, wy = v().y + py / old;
    v().T = nt; v().x = wx - px / nt; v().y = wy - py / nt;
    clampView(); dirty = true; store('lf.zoom', String(nt));
  }

  // --- tiles
  function tileAt(x, y) {
    const st = App.st;
    if (x < 0 || y < 0 || x >= st.width || y >= st.height) return null;
    const i = y * st.width + x;
    const raw = st.raw[i];
    if (raw === 255) return { explored: false };
    const t = raw & 31, hr = raw >> 5;
    const water = t === 25 || t === 26 || t > 26;
    return {
      explored: true, raw, t, water, lane: t === 26, arctic: t === 24,
      base: t < 8 ? t : t < 16 ? t - 8 : t < 24 ? t - 16 : -1,
      forest: t >= 8 && t < 24, hills: hr === 1 || hr === 3, mountains: hr === 5 || hr === 7,
      river: hr === 2 || hr === 3 ? 1 : hr === 6 || hr === 7 ? 2 : 0,
      road: (st.mask[i] & 8) !== 0, plowed: (st.mask[i] & 0x40) !== 0, vis: st.vis[i] === 1,
    };
  }
  function rnd(x, y, k) {
    let n = (x * 374761393 + y * 668265263 + k * 2246822519) | 0;
    n = (n ^ (n >>> 13)) * 1274126177 | 0;
    return ((n ^ (n >>> 16)) >>> 0) / 4294967296;
  }

  function drawTile(x, y, px, py, s) {
    const t = tileAt(x, y);
    if (!t || !t.explored) {
      ctx.fillStyle = FOG; ctx.fillRect(px, py, s + 0.5, s + 0.5);
      return;
    }
    if (t.water) {
      let shore = false;
      for (let dy = -1; dy <= 1 && !shore; dy++) for (let dx = -1; dx <= 1; dx++) {
        const n = tileAt(x + dx, y + dy); if (n && n.explored && !n.water) { shore = true; break; }
      }
      ctx.fillStyle = t.lane ? LANE : shore ? SHALLOW : OCEAN;
      ctx.fillRect(px, py, s + 0.5, s + 0.5);
      if (s > 26) { // wave marks
        ctx.strokeStyle = 'rgba(255,255,255,0.10)'; ctx.lineWidth = 1;
        const wx = px + s * (0.2 + rnd(x, y, 1) * 0.5), wy = py + s * (0.25 + rnd(x, y, 2) * 0.5);
        ctx.beginPath(); ctx.moveTo(wx, wy); ctx.quadraticCurveTo(wx + s * 0.08, wy - s * 0.06, wx + s * 0.16, wy); ctx.quadraticCurveTo(wx + s * 0.24, wy + s * 0.06, wx + s * 0.32, wy); ctx.stroke();
      }
      if (t.lane && s > 30) {
        ctx.fillStyle = 'rgba(255,255,255,0.08)';
        ctx.fillRect(px + s * 0.48, py, 1, s);
      }
    } else if (t.arctic) {
      ctx.fillStyle = ARCTIC; ctx.fillRect(px, py, s + 0.5, s + 0.5);
    } else {
      ctx.fillStyle = BASE[t.base] || '#999'; ctx.fillRect(px, py, s + 0.5, s + 0.5);
      if (t.plowed) {
        ctx.strokeStyle = 'rgba(90,60,30,0.28)'; ctx.lineWidth = Math.max(1, s / 40);
        for (let k = 1; k < 5; k++) { ctx.beginPath(); ctx.moveTo(px + s * 0.1, py + s * k / 5); ctx.lineTo(px + s * 0.9, py + s * k / 5); ctx.stroke(); }
      }
      if (t.forest) {
        ctx.fillStyle = FOREST[t.base] || '#476';
        const n = 5;
        for (let k = 0; k < n; k++) {
          const tx = px + s * (0.15 + 0.7 * rnd(x, y, k * 2 + 3)), ty = py + s * (0.2 + 0.65 * rnd(x, y, k * 2 + 4));
          const r = s * 0.13;
          if (t.base === 0 || t.base === 4) { // conifers
            ctx.beginPath(); ctx.moveTo(tx, ty - r * 1.6); ctx.lineTo(tx + r, ty + r * 0.6); ctx.lineTo(tx - r, ty + r * 0.6); ctx.closePath(); ctx.fill();
          } else {
            ctx.beginPath(); ctx.arc(tx, ty, r, 0, Math.PI * 2); ctx.fill();
          }
        }
      }
      if (t.hills) {
        ctx.fillStyle = 'rgba(110,84,50,0.45)';
        for (const [ox, oy, r] of [[0.32, 0.62, 0.22], [0.66, 0.55, 0.26]]) {
          ctx.beginPath(); ctx.ellipse(px + s * ox, py + s * oy, s * r, s * r * 0.6, 0, Math.PI, 0); ctx.fill();
        }
      }
      if (t.mountains) {
        for (const [ox, w2, hgt] of [[0.3, 0.26, 0.5], [0.62, 0.32, 0.62]]) {
          const bx = px + s * ox, by = py + s * 0.82;
          ctx.fillStyle = '#7C7468';
          ctx.beginPath(); ctx.moveTo(bx - s * w2, by); ctx.lineTo(bx, by - s * hgt); ctx.lineTo(bx + s * w2, by); ctx.closePath(); ctx.fill();
          ctx.fillStyle = '#EDEBE6';
          ctx.beginPath(); ctx.moveTo(bx - s * w2 * 0.3, by - s * hgt * 0.7); ctx.lineTo(bx, by - s * hgt); ctx.lineTo(bx + s * w2 * 0.3, by - s * hgt * 0.7); ctx.closePath(); ctx.fill();
        }
      }
      // shoreline
      ctx.strokeStyle = 'rgba(232,220,180,0.55)'; ctx.lineWidth = Math.max(1, s / 24);
      const nb = [[0, -1, px, py, px + s, py], [1, 0, px + s, py, px + s, py + s], [0, 1, px, py + s, px + s, py + s], [-1, 0, px, py, px, py + s]];
      for (const [dx, dy, x1, y1, x2, y2] of nb) {
        const n = tileAt(x + dx, y + dy);
        if (n && n.explored && n.water) { ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke(); }
      }
    }
    if (t.river && !t.water) {
      ctx.strokeStyle = '#4C8DB0'; ctx.lineWidth = Math.max(1.5, s * (t.river === 2 ? 0.11 : 0.06)); ctx.lineCap = 'round';
      const cx = px + s / 2, cy = py + s / 2;
      let any = false;
      for (const [dx, dy] of [[0, -1], [1, 0], [0, 1], [-1, 0]]) {
        const n = tileAt(x + dx, y + dy);
        if (n && n.explored && (n.river || n.water)) {
          any = true;
          ctx.beginPath(); ctx.moveTo(cx, cy);
          ctx.quadraticCurveTo(cx + dx * s * 0.25 + dy * s * 0.1, cy + dy * s * 0.25 + dx * s * 0.1, cx + dx * s * 0.5, cy + dy * s * 0.5); ctx.stroke();
        }
      }
      if (!any) { ctx.beginPath(); ctx.moveTo(px + s * 0.25, cy); ctx.quadraticCurveTo(cx, cy - s * 0.2, px + s * 0.75, cy); ctx.stroke(); }
    }
    if (t.road) {
      ctx.strokeStyle = '#8A6239'; ctx.lineWidth = Math.max(1.2, s * 0.05); ctx.setLineDash([s * 0.08, s * 0.05]);
      const cx = px + s / 2, cy = py + s / 2;
      let any = false;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        if (!dx && !dy) continue;
        const n = tileAt(x + dx, y + dy);
        const col = App.colonyAt(x + dx, y + dy);
        if ((n && n.road) || col) { any = true; ctx.beginPath(); ctx.moveTo(cx, cy); ctx.lineTo(cx + dx * s * 0.5, cy + dy * s * 0.5); ctx.stroke(); }
      }
      if (!any) { ctx.beginPath(); ctx.arc(cx, cy, s * 0.08, 0, Math.PI * 2); ctx.stroke(); }
      ctx.setLineDash([]);
    }
    if (!t.vis) { ctx.fillStyle = 'rgba(15,27,36,0.42)'; ctx.fillRect(px, py, s + 0.5, s + 0.5); }
  }

  function drawToken(u, px, py, s, opts = {}) {
    const r = s * 0.34;
    const cx = px + s / 2 + (opts.ox || 0) * s, cy = py + s / 2 + (opts.oy || 0) * s;
    ctx.save();
    ctx.globalAlpha = opts.dim ? 0.6 : 1;
    ctx.fillStyle = 'rgba(0,0,0,0.35)';
    ctx.beginPath(); ctx.ellipse(cx, cy + r * 0.9, r * 0.9, r * 0.3, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = NATION_COLORS[u.nation] || '#999';
    ctx.strokeStyle = u.fortified || opts.fortified ? '#F2E6C4' : 'rgba(255,255,255,0.85)';
    ctx.lineWidth = u.fortified || opts.fortified ? Math.max(2, s * 0.06) : Math.max(1, s * 0.03);
    ctx.beginPath();
    if (isShip(u.kind)) ctx.roundRect(cx - r * 1.05, cy - r * 0.95, r * 2.1, r * 1.9, r * 0.5);
    else ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.fill(); ctx.stroke();
    const sc = (r * 1.45) / 24;
    ctx.translate(cx - 12 * sc, cy - 12 * sc); ctx.scale(sc, sc);
    ctx.fillStyle = u.nation === 2 || u.nation === 12 ? '#2A2216' : '#FFFFFF';
    for (const k of unitIcon(u.kind)) ctx.fill(iconPath(k));
    ctx.restore();
    if (opts.count > 1 && s > 22) {
      ctx.fillStyle = '#17222E'; ctx.strokeStyle = '#EEE7D6'; ctx.lineWidth = 1;
      const bx = cx + r * 0.85, by = cy - r * 0.85, br = Math.max(6, s * 0.13);
      ctx.beginPath(); ctx.arc(bx, by, br, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
      ctx.fillStyle = '#EEE7D6'; ctx.font = `600 ${Math.round(br * 1.25)}px Barlow, sans-serif`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(String(opts.count), bx, by + 0.5);
    }
  }

  function drawColony(c, px, py, s) {
    const color = NATION_COLORS[c.nation];
    const m = s * 0.12;
    ctx.fillStyle = 'rgba(0,0,0,0.3)'; ctx.fillRect(px + m + 2, py + m + 3, s - 2 * m, s - 2 * m);
    ctx.fillStyle = color; ctx.strokeStyle = c.stockade ? '#3B2A18' : '#F4EEDD'; ctx.lineWidth = c.stockade ? Math.max(2, s * 0.07) : Math.max(1, s * 0.035);
    ctx.beginPath(); ctx.roundRect(px + m, py + m, s - 2 * m, s - 2 * m, s * 0.08); ctx.fill(); ctx.stroke();
    ctx.save();
    const sc = (s * 0.62) / 24;
    ctx.translate(px + s / 2 - 12 * sc, py + s / 2 - 12 * sc); ctx.scale(sc, sc);
    ctx.fillStyle = c.nation === 2 ? '#2A2216' : '#FFF8EA'; ctx.fill(iconPath('colony'));
    ctx.restore();
    if (s >= 24) {
      const label = `${c.name}  ${c.pop}`;
      ctx.font = `600 ${Math.max(11, Math.round(s * 0.24))}px Spectral, serif`;
      const tw = ctx.measureText(label).width + 10;
      const lx = px + s / 2 - tw / 2, ly = py + s - 2;
      ctx.fillStyle = 'rgba(20,30,40,0.82)'; ctx.beginPath(); ctx.roundRect(lx, ly, tw, Math.max(15, s * 0.32), 4); ctx.fill();
      ctx.fillStyle = c.nation === App.st.human && c.warnings && c.warnings.some(w => w.startsWith('Starving')) ? '#F0907A' : '#F3ECDB';
      ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(label, px + s / 2, ly + Math.max(15, s * 0.32) / 2 + 0.5);
    }
  }

  function drawVillage(vg, px, py, s) {
    const color = NATION_COLORS[vg.tribe] || '#8a6';
    const cx = px + s / 2, cy = py + s / 2, r = s * 0.33;
    ctx.fillStyle = 'rgba(0,0,0,0.3)'; ctx.beginPath(); ctx.ellipse(cx, cy + r * 0.9, r, r * 0.3, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = color; ctx.strokeStyle = vg.capital ? '#F2D27A' : '#F4EEDD'; ctx.lineWidth = vg.capital ? Math.max(2, s * 0.06) : Math.max(1, s * 0.03);
    ctx.beginPath(); ctx.moveTo(cx, cy - r * 1.2); ctx.lineTo(cx + r * 1.15, cy + r * 0.8); ctx.lineTo(cx - r * 1.15, cy + r * 0.8); ctx.closePath(); ctx.fill(); ctx.stroke();
    ctx.fillStyle = 'rgba(40,25,10,0.7)';
    ctx.beginPath(); ctx.moveTo(cx, cy); ctx.lineTo(cx + r * 0.35, cy + r * 0.8); ctx.lineTo(cx - r * 0.35, cy + r * 0.8); ctx.closePath(); ctx.fill();
    if (vg.mission !== null && vg.mission !== undefined) {
      ctx.fillStyle = NATION_COLORS[vg.mission]; ctx.fillRect(cx + r * 0.6, cy - r * 1.1, s * 0.06, s * 0.22); ctx.fillRect(cx + r * 0.6 - s * 0.05, cy - r * 1.0, s * 0.16, s * 0.05);
    }
  }

  function frame(time) {
    requestAnimationFrame(frame);
    if (!App.st || App.screen !== 'map' && !dirty) return;
    const sel = App.selected();
    if (!dirty && !sel) return;
    if (!dirty && sel && time - (frame.last || 0) < 50) return;
    frame.last = time;
    dirty = false;
    draw(time);
  }

  function draw(time) {
    const st = App.st;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = FOG; ctx.fillRect(0, 0, W, H);
    const s = T();
    const x0 = Math.floor(v().x), y0 = Math.floor(v().y);
    const x1 = Math.ceil(v().x + W / s), y1 = Math.ceil(v().y + H / s);
    const P = (x, y) => [(x - v().x) * s, (y - v().y) * s];
    for (let y = Math.max(0, y0); y < Math.min(st.height, y1); y++)
      for (let x = Math.max(0, x0); x < Math.min(st.width, x1); x++) {
        const [px, py] = P(x, y); drawTile(x, y, px, py, s);
      }
    if (s >= 40 && App.prefs.grid) {
      ctx.strokeStyle = 'rgba(0,0,0,0.08)'; ctx.lineWidth = 1;
      for (let x = Math.max(0, x0); x <= Math.min(st.width, x1); x++) { const [px] = P(x, 0); ctx.beginPath(); ctx.moveTo(px, 0); ctx.lineTo(px, H); ctx.stroke(); }
      for (let y = Math.max(0, y0); y <= Math.min(st.height, y1); y++) { const [, py] = P(0, y); ctx.beginPath(); ctx.moveTo(0, py); ctx.lineTo(W, py); ctx.stroke(); }
    }
    const inView = (x, y) => x >= x0 - 1 && x <= x1 && y >= y0 - 1 && y <= y1;
    // markers
    for (const [x, y, r] of st.resources) if (inView(x, y) && s >= 26) {
      const [px, py] = P(x, y);
      ctx.fillStyle = '#F5D76E'; ctx.strokeStyle = '#5A4420'; ctx.lineWidth = 1;
      ctx.beginPath(); const cx = px + s * 0.82, cy = py + s * 0.18, rr = s * 0.08;
      ctx.moveTo(cx, cy - rr); ctx.lineTo(cx + rr, cy); ctx.lineTo(cx, cy + rr); ctx.lineTo(cx - rr, cy); ctx.closePath(); ctx.fill(); ctx.stroke();
    }
    for (const [x, y] of st.rumours) if (inView(x, y)) {
      const [px, py] = P(x, y);
      ctx.fillStyle = '#F1E3B3'; ctx.strokeStyle = '#6B5326'; ctx.lineWidth = Math.max(1, s * 0.03);
      ctx.beginPath(); ctx.roundRect(px + s * 0.3, py + s * 0.3, s * 0.4, s * 0.4, s * 0.06); ctx.fill(); ctx.stroke();
      ctx.fillStyle = '#6B5326'; ctx.font = `700 ${Math.round(s * 0.32)}px Spectral, serif`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText('?', px + s / 2, py + s / 2 + 1);
    }
    for (const vg of st.villages) if (inView(vg.x, vg.y)) { const [px, py] = P(vg.x, vg.y); drawVillage(vg, px, py, s); }
    for (const c of st.colonies) if (inView(c.x, c.y)) { const [px, py] = P(c.x, c.y); drawColony(c, px, py, s); }
    // units, grouped by square
    const groups = new Map();
    for (const u of st.units) {
      if (u.x === null || u.x === undefined) continue;
      if (u.loc && u.loc.type !== 'map') continue;
      if (!inView(u.x, u.y)) continue;
      const k = u.x + ',' + u.y;
      if (!groups.has(k)) groups.set(k, []);
      groups.get(k).push(u);
    }
    for (const [k, list] of groups) {
      const [x, y] = k.split(',').map(Number);
      const [px, py] = P(x, y);
      let top = list.find(u => u.id === App.sel) || list.find(u => !isShip(u.kind) && u.kind !== 12 && u.kind !== 10 && [1, 4, 6, 7, 8, 9, 11].includes(u.kind)) || list.find(u => isShip(u.kind)) || list[0];
      const col = App.colonyAt(x, y);
      const opts = { count: list.length, dim: top.loc && (top.orders === 'sentry' || top.orders === 'fortify') && top.id !== App.sel, fortified: top.orders === 'fortify' };
      if (col) Object.assign(opts, { ox: 0.3, oy: -0.3 });
      drawToken(top, px, py, col ? s * 0.7 : s, opts);
    }
    // selection
    const sel = App.selected();
    if (sel && sel.loc.type === 'map') {
      const [px, py] = P(sel.x, sel.y);
      const pulse = 0.5 + 0.5 * Math.sin((time || 0) / 260);
      ctx.strokeStyle = `rgba(242,200,90,${0.55 + 0.45 * pulse})`; ctx.lineWidth = Math.max(2, s * 0.06);
      ctx.beginPath(); ctx.roundRect(px + 1.5, py + 1.5, s - 3, s - 3, s * 0.12); ctx.stroke();
    }
    // path preview
    if (path && path.length && sel) {
      ctx.strokeStyle = 'rgba(242,215,140,0.9)'; ctx.lineWidth = Math.max(2, s * 0.05); ctx.setLineDash([s * 0.12, s * 0.1]);
      ctx.beginPath();
      const [sx, sy] = P(sel.x + 0.5, sel.y + 0.5); ctx.moveTo(sx, sy);
      for (const [x, y] of path) { const [px, py] = P(x + 0.5, y + 0.5); ctx.lineTo(px, py); }
      ctx.stroke(); ctx.setLineDash([]);
      let lastTurn = 0;
      path.forEach(([x, y, turn], i) => {
        const end = i === path.length - 1;
        if (turn !== lastTurn || end) {
          const [px, py] = P(x + 0.5, y + 0.5);
          const label = String(turn + 1);
          ctx.fillStyle = end ? '#F2C85A' : '#17222E'; ctx.strokeStyle = '#F2D78C'; ctx.lineWidth = 1.5;
          const rr = Math.max(8, s * 0.17);
          ctx.beginPath(); ctx.arc(px, py, rr, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
          ctx.fillStyle = end ? '#17222E' : '#F2D78C'; ctx.font = `700 ${Math.round(rr * 1.15)}px Barlow, sans-serif`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
          ctx.fillText(label, px, py + 0.5);
          lastTurn = turn;
        }
      });
    }
    if (hover && App.screen === 'map') {
      const [px, py] = P(hover.x, hover.y);
      ctx.strokeStyle = 'rgba(255,255,255,0.55)'; ctx.lineWidth = 1.5;
      ctx.strokeRect(px + 0.5, py + 0.5, s - 1, s - 1);
    }
    Minimap.draw();
  }

  // --- input
  function onWheel(e) {
    e.preventDefault();
    const r = cv().getBoundingClientRect();
    zoomAt(e.clientX - r.left, e.clientY - r.top, e.deltaY < 0 ? 1.12 : 1 / 1.12);
  }
  function onDown(e) {
    cv().setPointerCapture(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, vx: v().x, vy: v().y, moved: false, button: e.button };
  }
  function onMove(e) {
    const r = cv().getBoundingClientRect();
    const px = e.clientX - r.left, py = e.clientY - r.top;
    if (drag && drag.button === 0) {
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 5) drag.moved = true;
      if (drag.moved) { v().x = drag.vx - dx / T(); v().y = drag.vy - dy / T(); clampView(); dirty = true; Tooltip.hide(); return; }
    }
    const t = toTile(px, py);
    if (!hover || hover.x !== t.x || hover.y !== t.y) {
      hover = t; dirty = true;
      updatePath();
      Tooltip.tile(t, e.clientX, e.clientY);
    } else Tooltip.move(e.clientX, e.clientY);
  }
  function onUp(e) {
    const d = drag; drag = null;
    if (!d || d.moved) return;
    const r = cv().getBoundingClientRect();
    const t = toTile(e.clientX - r.left, e.clientY - r.top);
    if (e.button === 2 || (e.button === 0 && (e.altKey || e.ctrlKey))) App.orderTo(t.x, t.y);
    else if (e.button === 0) App.clickTile(t.x, t.y);
  }
  function updatePath() {
    const sel = App.selected();
    path = null;
    if (!sel || !hover || sel.loc.type !== 'map' || App.screen !== 'map') return;
    if (hover.x === sel.x && hover.y === sel.y) return;
    if (Math.abs(hover.x - sel.x) <= 1 && Math.abs(hover.y - sel.y) <= 1) return;
    const key = `${sel.id}:${sel.x},${sel.y}>${hover.x},${hover.y}`;
    if (key === pathKey && path) return;
    pathKey = key;
    if (!tileAt(hover.x, hover.y)) return;
    const r = Engine.call({ cmd: 'path', unit: sel.id, x: hover.x, y: hover.y });
    path = r.data && r.data.path;
  }

  function paintTile(canvas, x, y) {
    const old = ctx;
    ctx = canvas.getContext('2d');
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    drawTile(x, y, 0, 0, canvas.width);
    ctx = old;
  }

  return {
    init, centerOn, tileAt, zoomAt, paintTile,
    invalidate() { dirty = true; pathKey = ''; updatePath(); },
    size: () => ({ W, H }),
  };
})();

// The minimap, drawn into its own canvas in the side panel.
const Minimap = {
  draw() {
    const c = $('#minimap'); if (!c || !App.st) return;
    const st = App.st, ctx = c.getContext('2d');
    const s = Math.min(c.width / st.width, c.height / st.height);
    ctx.fillStyle = '#0F1B24'; ctx.fillRect(0, 0, c.width, c.height);
    const col = ['#A7B096', '#D6C38E', '#BAC56F', '#CDC57A', '#8FB75E', '#ADB956', '#7F9B6D', '#5E7C5C'];
    for (let y = 0; y < st.height; y++) for (let x = 0; x < st.width; x++) {
      const raw = st.raw[y * st.width + x]; if (raw === 255) continue;
      const t = raw & 31;
      ctx.fillStyle = t >= 25 ? (t === 26 ? '#24536A' : '#2B5C70') : t === 24 ? '#E4EBEE' : t >= 8 ? '#4F7350' : col[t];
      ctx.fillRect(x * s, y * s, Math.ceil(s), Math.ceil(s));
    }
    for (const cl of st.colonies) { ctx.fillStyle = NATION_COLORS[cl.nation]; ctx.fillRect(cl.x * s - 1, cl.y * s - 1, s + 2, s + 2); }
    for (const vg of st.villages) { ctx.fillStyle = '#E8D7A8'; ctx.fillRect(vg.x * s, vg.y * s, s, s); }
    const { W, H } = MapView.size();
    ctx.strokeStyle = '#F2C85A'; ctx.lineWidth = 1;
    ctx.strokeRect(App.view.x * s, App.view.y * s, (W - 330) / App.view.T * s, H / App.view.T * s);
  },
  click(e) {
    const c = $('#minimap'), st = App.st; const r = c.getBoundingClientRect();
    const s = Math.min(c.width / st.width, c.height / st.height);
    const x = (e.clientX - r.left) * (c.width / r.width) / s, y = (e.clientY - r.top) * (c.height / r.height) / s;
    MapView.centerOn(Math.floor(x), Math.floor(y));
  },
};
