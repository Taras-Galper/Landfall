'use strict';
// The rules engine (Rust compiled to WebAssembly). One call: JSON in, JSON out.
const Engine = (() => {
  let ex = null;
  const enc = new TextEncoder();
  const dec = new TextDecoder();

  async function load() {
    let bytes;
    if (window.ENGINE_WASM_B64) {
      const bin = atob(window.ENGINE_WASM_B64);
      bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    } else {
      bytes = new Uint8Array(await (await fetch('landfall.wasm')).arrayBuffer());
    }
    const { instance } = await WebAssembly.instantiate(bytes, {});
    ex = instance.exports;
  }

  function call(cmd) {
    const b = enc.encode(JSON.stringify(cmd));
    const p = ex.alloc(b.length);
    new Uint8Array(ex.memory.buffer, p, b.length).set(b);
    const r = ex.call(p, b.length);
    const n = ex.reply_len();
    const out = dec.decode(new Uint8Array(ex.memory.buffer, r, n));
    ex.dealloc(p, b.length);
    return JSON.parse(out);
  }

  return { load, call };
})();
