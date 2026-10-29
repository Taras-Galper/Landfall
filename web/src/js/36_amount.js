// A small popover for choosing how much of a good to load, unload, buy or sell.
const Amount = (() => {
  let el = null;
  function close() {
    if (el) el.remove();
    el = null;
    document.removeEventListener('pointerdown', outside, true);
    document.removeEventListener('keydown', key, true);
  }
  function outside(e) { if (el && !el.contains(e.target)) close(); }
  function key(e) {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); }
  }
  // opts: { anchor, title, max, value, verb, unit, price, onOk(n) }
  function pick(opts) {
    close();
    const max = Math.max(1, Math.floor(opts.max));
    let n = Math.max(1, Math.min(max, Math.floor(opts.value ?? max)));
    const num = h('input', { type: 'number', min: 1, max, value: n, 'aria-label': 'Amount' });
    const range = h('input', { type: 'range', min: 1, max, value: n, 'aria-label': 'Amount' });
    const go = h('button', { class: 'btn primary' });
    const note = h('span', { class: 'sub' });
    const set = v => {
      n = Math.max(1, Math.min(max, Math.round(Number(v) || 1)));
      num.value = n; range.value = n;
      go.textContent = `${opts.verb} ${n}`;
      note.textContent = opts.price ? opts.price(n) : '';
    };
    num.addEventListener('input', () => set(num.value));
    range.addEventListener('input', () => set(range.value));
    const ok = () => { close(); opts.onOk(n); };
    go.addEventListener('click', ok);
    num.addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); ok(); } });
    const presets = [10, 25, 50, 100].filter(p => p < max);
    el = h('div', { class: 'amount-pop', role: 'dialog', 'aria-label': opts.title },
      h('b', {}, opts.title),
      h('div', { class: 'row' }, range, num),
      h('div', { class: 'row presets' }, presets.map(p => h('button', { class: 'btn small', onclick: () => set(p) }, p)), h('button', { class: 'btn small', onclick: () => set(max) }, `All ${max}`)),
      note,
      h('div', { class: 'row end' }, h('button', { class: 'btn ghost', onclick: close }, 'Cancel'), go));
    document.body.append(el);
    set(n);
    const r = opts.anchor ? opts.anchor.getBoundingClientRect() : { left: innerWidth / 2, bottom: innerHeight / 2, top: innerHeight / 2 };
    const w = el.offsetWidth, hh = el.offsetHeight;
    let left = Math.min(innerWidth - w - 8, Math.max(8, r.left));
    let top = r.bottom + 6;
    if (top + hh > innerHeight - 8) top = Math.max(8, r.top - hh - 6);
    el.style.left = left + 'px'; el.style.top = top + 'px';
    setTimeout(() => { num.focus(); num.select(); }, 0);
    setTimeout(() => {
      document.addEventListener('pointerdown', outside, true);
      document.addEventListener('keydown', key, true);
    }, 0);
  }
  return { pick, close, open: () => !!el };
})();
