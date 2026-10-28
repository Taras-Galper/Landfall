// Pointer-based drag and drop used by the colony and homeland screens.
const Drag = (() => {
  let cur = null;
  function start(e, payload, el, handlers) {
    if (e.button !== 0) return;
    const sx = e.clientX, sy = e.clientY;
    cur = { payload, handlers, sx, sy, ghost: null, target: null, active: false, src: el };
    const move = ev => {
      if (!cur) return;
      if (!cur.active) {
        if (Math.abs(ev.clientX - sx) + Math.abs(ev.clientY - sy) < 6) return;
        cur.active = true;
        cur.ghost = el.cloneNode(true);
        cur.ghost.classList.add('drag-ghost');
        document.body.append(cur.ghost);
        el.classList.add('dragging');
        document.body.classList.add('is-dragging');
      }
      cur.ghost.style.left = ev.clientX - 18 + 'px';
      cur.ghost.style.top = ev.clientY - 18 + 'px';
      const under = document.elementFromPoint(ev.clientX, ev.clientY);
      const t = under && under.closest('[data-drop]');
      if (t !== cur.target) {
        if (cur.target) cur.target.classList.remove('over'), handlers.leave && handlers.leave(cur.target);
        cur.target = t;
        if (t) t.classList.add('over'), handlers.hover && handlers.hover(payload, t);
      }
    };
    const up = ev => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      const c = cur; cur = null;
      if (!c) return;
      document.body.classList.remove('is-dragging');
      if (c.ghost) c.ghost.remove();
      el.classList.remove('dragging');
      if (c.target) c.target.classList.remove('over'), handlers.leave && handlers.leave(c.target);
      if (!c.active) { handlers.click && handlers.click(payload, ev); return; }
      if (c.target) handlers.drop(payload, c.target);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }
  return { start, active: () => !!(cur && cur.active) };
})();
