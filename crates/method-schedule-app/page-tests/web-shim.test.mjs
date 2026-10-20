// The browser build's shim (crates/method-schedule-web/js/tauri-shim.js): what the page's
// address may open. Run in a stand-in window; the module itself never loads here.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const SHIM = readFileSync(new URL('../../method-schedule-web/js/tauri-shim.js', import.meta.url), 'utf8');

function load(href) {
  const window = {
    location: { href, search: '' },
    addEventListener() {},
    localStorage: { getItem: () => null, setItem() {} },
  };
  const document = {
    currentScript: { src: new URL('method-schedule-web/tauri-shim.js', href).href },
    readyState: 'complete',
    getElementById: () => null,
  };
  const context = vm.createContext({ window, document, URL, URLSearchParams, console: { error() {} }, setTimeout, Promise });
  vm.runInContext(SHIM, context);
  return window.__METHOD_SCHEDULE_WEB__;
}

test('?open= takes only a schedule file on the page\'s own site', () => {
  const here = 'https://methodschedule.com/try/';
  const { openTarget } = load(here);
  const ok = (open) => openTarget(open, here) !== null;
  assert.equal(ok('sample/Field%20Station.mtsc'), true);
  assert.equal(ok('/files/pilot.json'), true);
  assert.equal(ok('https://methodschedule.com/try/a.MTSC'), true);
  // Another site, another scheme, credentials, or not a schedule file: refused.
  assert.equal(ok('https://example.com/a.mtsc'), false);
  assert.equal(ok('//example.com/a.mtsc'), false);
  assert.equal(ok('http://methodschedule.com/try/a.mtsc'), false, 'http is another origin from https');
  assert.equal(ok('javascript:alert(1)//.mtsc'), false);
  assert.equal(ok('data:application/json,{}#.mtsc'), false);
  assert.equal(ok('https://user:pw@methodschedule.com/a.mtsc'), false);
  assert.equal(ok('a.html'), false);
  assert.equal(ok('a.mtsc?x=1'), true, 'a query string is the server\'s business');
  assert.equal(ok('a.mtsc.html'), false);
});
