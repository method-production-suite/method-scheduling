#!/usr/bin/env node
// Retakes every README and site picture (SUITE-STANDARDS.md section 7; decision 215).
//
//   node scripts/capture-screens.mjs           take them, writing only pictures that changed
//   node scripts/capture-screens.mjs --serve   serve the capture pages and wait (to look)
//
// From the invented Field Station (fixtures/field-station.mtsc): the engine's real answers
// (examples/screen_answers.rs) are replayed into the app's own page
// (crates/method-schedule-app/harness/screens.js), which refuses any call about the
// schedule that isn't recorded; the command line picture shows real output of the real
// command. Headless Chrome, driven over its remote-control protocol with Node's own
// WebSocket (no dependencies), draws each one at 1280 by 800 logical pixels, 2x, light and
// dark, and nothing beyond the window. Same pixels every run on one machine.
//
// Writes assets/screens/<name>-<light|dark>.png (the README) and the same file in site/img/
// (the website copies site/ only). A file is replaced only when its bytes differ.
// Chrome: /Applications/Google Chrome.app, or CHROME=path.

import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const APP = join(ROOT, 'crates/method-schedule-app');
// Cargo's build folder (CARGO_TARGET_DIR or ./target), as `cargo metadata` says.
const TARGET = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: ROOT, encoding: 'utf8' })).target_directory;
const DATA = join(TARGET, 'screens');
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const SIZE = { width: 1280, height: 800, scale: 2 };

// The four pictures, in the suite's order: hero, the signature feature, import, the file you own.
export const SHOTS = [
  { name: 'board', page: 'harness/screens.html?shot=board', alt: 'The stripboard of Field Station: the first days of an 18-day shoot, with scenes, banners and day breaks' },
  { name: 'dood', page: 'harness/screens.html?shot=dood', alt: 'The day out of days for Field Station: each cast member\'s work days across the shoot' },
  { name: 'import', page: 'harness/screens.html?shot=import', alt: 'The Field Station script just imported: 90 scenes in the Boneyard, ready to schedule' },
  { name: 'cli', page: 'harness/screens-cli.html', alt: 'The method-schedule command listing Field Station\'s shoot days, and the plain text schedule file' },
];

function run(cmd, args, opts = {}) {
  return execFileSync(cmd, args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], ...opts });
}

/** The engine's answers and the command line's output, into <build folder>/screens. */
function prepare() {
  mkdirSync(DATA, { recursive: true });
  run('cargo', ['run', '-q', '-p', 'method-schedule-app', '--example', 'screen_answers', '--', join(DATA, 'answers.json')]);
  run('cargo', ['build', '-q', '-p', 'method-schedule-cli']);
  const cli = join(TARGET, 'debug/method-schedule');
  const fixtures = join(ROOT, 'fixtures');
  const file = 'field-station.mtsc';
  const head = readFileSync(join(fixtures, file), 'utf8').split('\n').slice(0, 14).join('\n') + '\n';
  const runs = [
    { command: `method-schedule days ${file}`, output: run(cli, ['days', file], { cwd: fixtures }) },
    { command: `method-schedule dood ${file} | head -6`, output: run(cli, ['dood', file], { cwd: fixtures }).split('\n').slice(0, 6).join('\n') + '\n' },
    { command: `head -14 ${file}`, output: head },
  ];
  writeFileSync(join(DATA, 'cli.json'), JSON.stringify(runs, null, 2));
}

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml', '.mtsc': 'application/json' };

/** Serves the app crate at / and <build folder>/screens at /screens-data/, never cached. */
function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    const [base, rest] = path.startsWith('/screens-data/') ? [DATA, path.slice('/screens-data/'.length)] : [APP, path.slice(1)];
    const file = normalize(join(base, rest));
    if (!file.startsWith(base) || !existsSync(file)) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'Content-Type': TYPES[extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

/** A headless Chrome with a throwaway profile, and a protocol session on a new tab. */
async function chrome() {
  if (!existsSync(CHROME)) throw new Error(`no Chrome at ${CHROME} (set CHROME=...)`);
  const profile = mkdtempSync(join(tmpdir(), 'ms-screens-'));
  const proc = spawn(CHROME, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--hide-scrollbars', '--force-color-profile=srgb', '--font-render-hinting=none',
    // Software drawing, whole-pixel text: the same pixels every run (the GPU's antialiasing
    // varied by a few pixels between runs).
    '--disable-gpu', '--disable-gpu-compositing', '--disable-lcd-text', '--disable-font-subpixel-positioning', 'about:blank'], { stdio: 'ignore' });
  const portFile = join(profile, 'DevToolsActivePort');
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await new Promise((r) => setTimeout(r, 100));
  const [port, path] = readFileSync(portFile, 'utf8').trim().split('\n');
  const ws = new WebSocket(`ws://127.0.0.1:${port}${path}`);
  await new Promise((ok, no) => ((ws.onopen = ok), (ws.onerror = no)));
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, no } = waiting.get(msg.id);
      waiting.delete(msg.id);
      if (msg.error) no(new Error(msg.error.message));
      else ok(msg.result);
    }
  };
  const send = (method, params = {}, sessionId) =>
    new Promise((ok, no) => {
      id += 1;
      waiting.set(id, { ok, no });
      ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const tab = (method, params) => send(method, params, sessionId);
  await tab('Page.enable');
  await tab('Emulation.setDeviceMetricsOverride', { width: SIZE.width, height: SIZE.height, deviceScaleFactor: SIZE.scale, mobile: false });
  // Chrome writes to its profile until it has exited: wait for that before removing it.
  const exited = new Promise((ok) => proc.once('exit', ok));
  const close = async () => {
    ws.close();
    proc.kill();
    await exited;
    rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  };
  return { tab, close };
}

/** One picture: the page in a colour scheme, once it says it is ready. */
async function capture(tab, url, scheme) {
  await tab('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scheme }] });
  await tab('Page.navigate', { url });
  const deadline = Date.now() + 30000;
  for (;;) {
    const { result } = await tab('Runtime.evaluate', { expression: '({ ready: !!window.__screensReady, failed: window.__screensFailed || null })', returnByValue: true });
    const v = result.value || {};
    if (v.failed) throw new Error(`${url} (${scheme}): ${v.failed}`);
    if (v.ready) break;
    if (Date.now() > deadline) throw new Error(`${url} (${scheme}): not ready after 30 s`);
    await new Promise((r) => setTimeout(r, 150));
  }
  const { data } = await tab('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  return Buffer.from(data, 'base64');
}

/** Writes `bytes` to `file` only when they differ; true when it wrote. */
function writeIfChanged(file, bytes) {
  if (existsSync(file) && Buffer.compare(readFileSync(file), bytes) === 0) return false;
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, bytes);
  return true;
}

async function main() {
  prepare();
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  if (process.argv.includes('--serve')) {
    console.log(`capture pages at ${origin}/${SHOTS[0].page} (Ctrl-C to stop)`);
    return;
  }
  const { tab, close } = await chrome();
  const changed = [];
  try {
    for (const shot of SHOTS) {
      for (const scheme of ['light', 'dark']) {
        const png = await capture(tab, `${origin}/${shot.page}`, scheme);
        const name = `${shot.name}-${scheme}.png`;
        const a = writeIfChanged(join(ROOT, 'assets/screens', name), png);
        const b = writeIfChanged(join(ROOT, 'site/img', name), png);
        if (a || b) changed.push(name);
      }
    }
  } finally {
    await close();
    server.close();
  }
  console.log(changed.length ? `changed: ${changed.join(', ')}` : 'every picture is unchanged');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(`capture-screens: ${e.message}`);
    process.exit(1);
  });
}
