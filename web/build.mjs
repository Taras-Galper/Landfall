// Bundle the web edition into one self-contained page: web/dist/index.html
// (styles, scripts and the WebAssembly engine inlined, so it opens with a double-click).
//   node web/build.mjs [path/to/landfall_web.wasm]
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const wasmPath = process.argv[2]
  || [path.join(root, 'target/wasm32-unknown-unknown/web/landfall_web.wasm'),
      path.join(root, 'target/wasm32-unknown-unknown/release/landfall_web.wasm'),
      path.join(here, 'dist/landfall.wasm')].find(p => fs.existsSync(p));
if (!wasmPath) {
  console.error('No engine found. Build it first:  cargo build --profile web -p landfall_web --target wasm32-unknown-unknown');
  process.exit(1);
}
const wasm = fs.readFileSync(wasmPath);
const css = fs.readFileSync(path.join(here, 'src/style.css'), 'utf8');
const jsDir = path.join(here, 'src/js');
const js = fs.readdirSync(jsDir).filter(f => f.endsWith('.js')).sort()
  .map(f => `// ---- ${f}\n` + fs.readFileSync(path.join(jsDir, f), 'utf8')).join('\n');
const html = fs.readFileSync(path.join(here, 'src/index.html'), 'utf8')
  .replace('/*CSS*/', () => css)
  .replace('/*WASM*/', () => `window.ENGINE_WASM_B64 = "${wasm.toString('base64')}";`)
  .replace('/*JS*/', () => js);
fs.mkdirSync(path.join(here, 'dist'), { recursive: true });
fs.writeFileSync(path.join(here, 'dist/index.html'), html);
// the same page without its own document skeleton, for hosts that add one (Claude artifacts)
const inner = html
  .replace(/^<!doctype html>\s*<html[^>]*>\s*<head>\s*/i, '')
  .replace(/<meta charset="utf-8">\s*<meta name="viewport"[^>]*>\s*/i, '')
  .replace(/<\/head>\s*<body>\s*/i, '')
  .replace(/<\/body>\s*<\/html>\s*$/i, '')
  .replace('<script>window.ENGINE_WASM_B64', '<script>window.LF_EMBEDDED = true; window.ENGINE_WASM_B64');
fs.writeFileSync(path.join(here, 'dist/embedded.html'), inner);
if (path.resolve(wasmPath) !== path.join(here, 'dist/landfall.wasm')) fs.copyFileSync(wasmPath, path.join(here, 'dist/landfall.wasm'));
console.log(`web/dist/index.html  ${(html.length / 1024).toFixed(0)} KB  (engine ${(wasm.length / 1024).toFixed(0)} KB)`);
