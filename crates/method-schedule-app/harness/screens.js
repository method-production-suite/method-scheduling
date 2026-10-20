// The README and site pictures (scripts/capture-screens.mjs, decision 215): the app's own
// page, loaded the way boot.js loads it, answering the schedule calls a picture makes from
// the engine's real answers for Field Station (examples/screen_answers.rs, served at
// /screens-data/answers.json). Any other call about the schedule fails the capture, naming
// it, so a picture can only show what the engine said. Calls that are not about the
// schedule (settings, the window, recovery) go to the stand-in (fake-tauri.js), which
// answers them as a fresh install would.
//
//   screens.html?shot=board|dood|import[&discover]
// `discover` lets every call through and lists them in window.__screensCalls.

import './fake-tauri.js';

const params = new URLSearchParams(location.search);
const shot = params.get('shot') || 'board';
const discover = params.has('discover');
window.__screensCalls = [];

function fail(why) {
  window.__screensFailed = String(why);
}

const answers = await fetch('/screens-data/answers.json', { cache: 'no-store' }).then((r) => r.json());
const viewFor = shot === 'import' ? answers.imported : answers.board;

// Answered from the recording.
const RECORDED = {
  get_view: () => viewFor,
  dood_grid: () => answers.dood_grid,
  // A fresh install: nothing to recover, no update waiting, no What's New.
  recovery_list: () => [],
  open_pending: () => null,
  update_status: () => ({ phase: 'idle', version: null, manual: false, said: null, automatic: true, available: true }),
  update_whats_new: () => null,
};
// Not about the schedule: the stand-in answers these.
// Found with `discover`: the display preferences (the stand-in's are the defaults) and
// the host, the desktop app.
const INCIDENTAL = new Set(['preferences', 'host_info']);

const stand = window.__TAURI__.core.invoke;
window.__TAURI__.core.invoke = async (cmd, args) => {
  window.__screensCalls.push(cmd);
  if (discover) return stand(cmd, args);
  if (Object.prototype.hasOwnProperty.call(RECORDED, cmd)) return RECORDED[cmd](args || {});
  if (INCIDENTAL.has(cmd)) return stand(cmd, args);
  fail(`unrecorded call: ${cmd}`);
  throw `screens: ${cmd} is not recorded`;
};

// No blinking caret, no transitions: the same pixels every run.
const still = document.createElement('style');
still.textContent = '*, *::before, *::after { caret-color: transparent !important; transition: none !important; animation: none !important; }';
document.head.append(still);

const html = await fetch('../ui/index.html').then((r) => r.text());
const doc = new DOMParser().parseFromString(html, 'text/html');
document.body.replaceChildren(...[...doc.body.childNodes].map((n) => document.importNode(n, true)));
try {
  await import('../ui/js/main.js');
} catch (e) {
  fail(e);
}

const settle = (ms) => new Promise((r) => setTimeout(r, ms));
async function ready() {
  await settle(600);
  if (shot === 'dood') {
    document.getElementById('tab-dood').click();
    await settle(600);
  }
  // Nothing focused, nothing hovered: the board's own first-strip selection stays.
  if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
  await settle(200);
  if (document.querySelector('.modal')) fail('a dialog is open');
  if (!window.__screensFailed) window.__screensReady = true;
}
ready().catch(fail);
