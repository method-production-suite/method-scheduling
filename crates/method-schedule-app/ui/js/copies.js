// Conflict copies a sync service left beside the schedule's file (src/copies.rs;
// decisions.md, entry 241). Rust lists them; this keeps a line in the footer offering to
// merge the newest into the schedule with Merge Changes From (diffmerge.js), or to
// dismiss them. Text goes in with textContent only.
//
// The list is asked for when the window gets focus (Rust sends `outside-check`) and when
// the window's file changes (Open, Save As).

import { invoke, listen } from './api.js';
import { $ } from './dom.js';
import { runMerge } from './diffmerge.js';
import { copiesLabel } from './logic.js';

/**
 * Sets up the footer line. `ctx`: { say(text, isError), show(view), go(where) }. Returns
 * `check()`, which asks for the list again.
 */
export function initCopies(ctx) {
  const bar = $('copies-found');
  const label = $('copies-found-label');
  let found = [];
  let busy = false;

  function draw() {
    label.textContent = copiesLabel(found);
    bar.hidden = !found.length;
  }

  async function check() {
    if (busy) return;
    try {
      found = await invoke('copies_found');
    } catch {
      found = [];
    }
    draw();
  }

  async function settle(name) {
    try {
      await invoke('copy_settle', { name });
    } catch (e) {
      ctx.say(e, true);
    }
  }

  $('btn-copies-merge').addEventListener('click', async () => {
    if (busy || !found.length) return;
    const { name } = found[0];
    busy = true;
    try {
      const picked = await invoke('copy_merge', { name });
      if (await runMerge(picked.token, ctx.say, ctx.show, ctx.go)) await settle(name);
    } catch (e) {
      ctx.say(e, true);
    } finally {
      busy = false;
      await check();
    }
  });

  $('btn-copies-dismiss').addEventListener('click', async () => {
    if (busy) return;
    busy = true;
    try {
      for (const c of found) await settle(c.name);
    } finally {
      busy = false;
      await check();
    }
  });

  listen('outside-check', () => check());
  return { check };
}
