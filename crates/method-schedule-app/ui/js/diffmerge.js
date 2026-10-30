// File > What Changed… (and What Changed Since Saved) and File > Merge Changes From…
// (src/diffmerge.rs; docs/MERGE.md; decisions.md, entries 25 and 26). After Method
// Budget's ui/js/diffmerge.js, scaled down to one schedule per window.
//
// What Changed: every change from another schedule file (or this schedule's file as it
// is on disk) to the schedule in the window, in the engine's plain sentences, grouped
// under Schedule, Categories, Elements, Sheets, Boards, Calendars and Strips. Read-only.
//
// Merge Changes From: Rust shows the Open dialog for the other copy (theirs) and keeps
// it by token. The dialog shows the base (the version both started from) and why it was
// picked, with the backups and the file on disk to pick from instead; every conflict
// with Keep Yours and Keep Theirs (yours to begin with); what the merge changes here;
// and anything the merge kept yours so the schedule hangs together. Each choice works
// the merge out again in Rust. Merge applies it as one undo step. A conflict or change
// about something in this schedule is a link that shows it (decision 240): the merge
// waits in the footer, picks and base kept, until Back to Merge or Cancel Merge.
//
// Every string is Rust's, put in with textContent. No inline styles (the CSP).

import { invoke } from './api.js';
import { $, el } from './dom.js';
import { customDialog, errorLine } from './modal.js';
import { baseChoiceOf, baseValueOf } from './logic.js';

let uid = 0;
const nextId = (prefix) => `${prefix}-${++uid}`;

const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

/**
 * Grouped changes: a heading and a list for each group. With `onGo`, a change that says
 * where it is (`go`, entry 235) is a link that shows it.
 */
function changeList(groups, onGo = null) {
  const out = [];
  for (const g of groups) {
    out.push(el('h4', { class: 'dm-group', text: g.group }));
    const item = (c) =>
      onGo && c.go
        ? el('li', { class: `dm-change k-${c.kind}` }, el('button', { class: 'cf-link dm-go', text: c.text, attrs: { type: 'button', title: 'Show it' }, on: { click: () => onGo(c.go) } }))
        : el('li', { class: `dm-change k-${c.kind}`, text: c.text });
    out.push(el('ul', { class: 'dm-list' }, ...g.changes.map(item)));
  }
  return out;
}

/**
 * File > What Changed (`against` "file"), What Changed Since Saved ("saved"), and a backup
 * from Revert to Backup's list ("backup", by its name in the backups folder).
 */
export async function whatChanged(against, say, backup = null, go = null) {
  let dto;
  try {
    dto = await invoke('what_changed', { against, backup });
  } catch (e) {
    say(e, true);
    return;
  }
  if (!dto) return;
  await customDialog((close) => {
    const done = el('button', { class: 'btn btn-primary', text: 'Close', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(null) } });
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'wc-title' } },
      el('h2', { class: 'modal-title', text: 'What Changed', attrs: { id: 'wc-title' } }),
      el('p', { class: 'modal-message', text: `From ${dto.from} to ${dto.to}.` }),
      el('p', { class: 'dm-count', text: dto.count ? plural(dto.count, 'change', 'changes') : 'No changes.' }),
      dto.count && go ? el('p', { class: 'modal-message', text: 'Click a change to see it.' }) : null,
      el('div', { class: 'dm-body' }, ...changeList(dto.groups, go ? (where) => close(null) || go(where) : null)),
      el('div', { class: 'modal-actions' }, done),
    );
  });
}

/** One conflict with its pick; with `onGo`, what it is about is a link that shows it. */
function conflictNode(c, onPick, onGo = null) {
  const name = nextId('dm-pick');
  const radio = (side, label) => {
    const id = nextId('dm-side');
    const box = el('input', { attrs: { type: 'radio', name, id, value: side }, data: { key: c.key } });
    box.checked = c.taken === side;
    box.addEventListener('change', () => {
      if (box.checked) onPick(c.key, side);
    });
    return el('label', { class: 'dm-side', attrs: { for: id } }, box, label);
  };
  // Yours, theirs and the base side by side (entry 235).
  const col = (label, value, cls) => el('div', { class: `dm-value ${cls}` }, el('span', { class: 'dm-label', text: label }), el('span', { text: value }));
  return el(
    'li',
    { class: 'dm-conflict' },
    onGo && c.go
      ? el('div', { class: 'dm-what' }, el('button', { class: 'cf-link dm-go', text: c.what, attrs: { type: 'button', title: 'Show it' }, on: { click: () => onGo(c.go) } }))
      : el('div', { class: 'dm-what', text: c.what }),
    el(
      'div',
      { class: 'dm-cols' },
      col('Yours', c.ours, 'v-ours'),
      col('Theirs', c.theirs, 'v-theirs'),
      c.base !== null && c.base !== undefined ? col('Was', c.base, 'v-base') : null,
    ),
    el('div', { class: 'dm-sides', attrs: { role: 'radiogroup', 'aria-label': `Keep for ${c.what}` } }, radio('ours', 'Keep yours'), radio('theirs', 'Keep theirs')),
  );
}

/**
 * File > Merge Conflicts to Review (entry 238): every conflict a merge settled one way, kept
 * with the file until marked done. A conflict on a sheet, element or category still in the
 * schedule is a link that shows it. `run(promise)` applies a command's view; `go(where)`
 * shows a place, as Find does.
 */
export async function reviewConflicts(say, run, go) {
  let list;
  try {
    list = await invoke('conflicts_data');
  } catch (e) {
    say(e, true);
    return;
  }
  await customDialog((close) => {
    const body = el('ul', { class: 'dm-conflicts' });
    const count = el('p', { class: 'dm-count' });
    const error = errorLine();
    const allDone = el('button', { class: 'btn', text: 'All Done', attrs: { type: 'button' } });
    const draw = () => {
      count.textContent = list.length ? `${plural(list.length, 'conflict', 'conflicts')} to review.` : 'No conflicts to review.';
      allDone.disabled = !list.length;
      body.replaceChildren(...list.map(item));
    };
    const shout = (t) => {
      error.textContent = t;
      error.hidden = !t;
    };
    const settle = async (keys) => {
      shout('');
      try {
        await run(invoke('settle_conflicts', { keys }));
      } catch (e) {
        shout(String(e));
        return;
      }
      list = keys ? list.filter((c) => !keys.includes(c.key)) : [];
      draw();
    };
    const col = (label, value, cls) => el('div', { class: `dm-value ${cls}` }, el('span', { class: 'dm-label', text: label }), el('span', { text: value }));
    function item(c) {
      const what = c.go
        ? el('button', { class: 'cf-link dm-go', text: c.what, attrs: { type: 'button', title: 'Show it' }, on: { click: () => close(null) || go(c.go) } })
        : el('span', { text: c.what });
      const kept = `Kept ${c.taken === 'theirs' ? 'theirs' : 'yours'}${c.date ? `, merged ${c.date}` : ''}.`;
      return el(
        'li',
        { class: 'dm-conflict', data: { key: c.key } },
        el('div', { class: 'dm-what' }, what),
        el(
          'div',
          { class: 'dm-cols' },
          col('Yours', c.ours, 'v-ours'),
          col('Theirs', c.theirs, 'v-theirs'),
          c.base !== null && c.base !== undefined ? col('Was', c.base, 'v-base') : null,
        ),
        el(
          'div',
          { class: 'dm-sides' },
          el('span', { class: 'dm-kept', text: kept }),
          el('button', { class: 'btn', text: 'Done', attrs: { type: 'button', title: 'Take it off the list' }, on: { click: () => settle([c.key]) } }),
        ),
      );
    }
    allDone.addEventListener('click', () => settle(null));
    draw();
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'rc-title' } },
      el('h2', { class: 'modal-title', text: 'Merge Conflicts to Review', attrs: { id: 'rc-title' } }),
      el('p', { class: 'modal-message', text: 'Each conflict a merge settled one way stays here, with the file, until you mark it done. Click one to see it; change it there if the other value was right.' }),
      count,
      el('div', { class: 'dm-body' }, body),
      error,
      el('div', { class: 'modal-actions' }, allDone, el('button', { class: 'btn btn-primary', text: 'Close', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(null) } })),
    );
  });
}

/**
 * A merge left to look at something (decision 240): the footer's bar says so, and the
 * promise its dialog waits on resolves true for Back to Merge, false for Cancel Merge.
 */
let waiting = null;

function wait(theirs) {
  const box = $('merge-waiting');
  $('merge-waiting-label').textContent = `Merge from ${theirs} waiting`;
  box.hidden = false;
  return new Promise((resolve) => {
    waiting = (back) => {
      waiting = null;
      box.hidden = true;
      resolve(back);
    };
  });
}

/** The footer's buttons. Run once. */
export function initMergeWaiting() {
  $('btn-merge-back').addEventListener('click', () => waiting && waiting(true));
  $('btn-merge-cancel').addEventListener('click', () => waiting && waiting(false));
}

/** Brings a waiting merge back (Save does, so nothing is saved past it): true if one was. */
export function resumeMerge() {
  if (!waiting) return false;
  waiting(true);
  return true;
}

/**
 * File > Merge Changes From: pick the file, preview, apply. `show(view)` draws the result,
 * `go(where)` shows a place. A merge already waiting is cancelled for the new one.
 */
export async function mergeChanges(say, show, go = null) {
  if (waiting) waiting(false);
  let picked;
  try {
    picked = await invoke('merge_pick_file');
  } catch (e) {
    say(e, true);
    return;
  }
  if (!picked) return;
  await runMerge(picked.token, say, show, go);
}

/**
 * The merge dialog for file `token` (Merge Changes From's pick, or the schedule's own file
 * changed outside the app, src/outside.rs): preview, then apply. True when merged. With
 * `go`, a link in it shows its place and the merge waits to come back, worked out again
 * then on the schedule as it is, with the same base and picks.
 */
export async function runMerge(token, say, show, go = null) {
  const choice = { base: { kind: 'auto' }, picks: {} };
  for (;;) {
    const out = await mergeDialog(token, choice, say, go);
    if (out && out.go) {
      go(out.go);
      if (await wait(out.theirs)) continue;
      return false;
    }
    if (!out) return false;
    try {
      const view = await invoke('merge_apply', { token, base: choice.base, picks: choice.picks });
      if (view) show(view);
      return true;
    } catch (e) {
      say(e, true);
      return false;
    }
  }
}

/** The dialog: resolves `true` (Merge), `{ go, theirs }` (a link) or null (Cancel). */
async function mergeDialog(token, choice, say, onGo) {
  let dto;
  try {
    dto = await invoke('merge_preview', { token, base: choice.base, picks: choice.picks });
  } catch (e) {
    say(e, true);
    return null;
  }

  return customDialog((close) => {
    const body = el('div', { class: 'dm-body' });
    const error = errorLine('dm-error');
    const merge = el('button', { class: 'btn btn-primary', text: 'Merge', attrs: { type: 'button' }, on: { click: () => close(true) } });
    const cancel = el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(null) } });
    const baseSelect = el('select', { attrs: { id: 'dm-base', 'aria-describedby': 'dm-base-why' } });
    const baseWhy = el('p', { class: 'dm-why', attrs: { id: 'dm-base-why' } });
    let seq = 0;
    /** The pick just made, to keep focus on it when the preview is drawn again. */
    let focusPick = null;
    /** A link's click: the merge waits while the place is shown (decision 240). */
    const goTo = onGo ? (where) => close({ go: where, theirs: dto.theirs }) : null;

    async function refresh() {
      const mine = ++seq;
      merge.disabled = true;
      try {
        const next = await invoke('merge_preview', { token, base: choice.base, picks: choice.picks });
        if (mine !== seq) return;
        dto = next;
        error.hidden = true;
        draw();
      } catch (e) {
        if (mine !== seq) return;
        error.textContent = String(e);
        error.hidden = false;
      }
    }

    baseSelect.addEventListener('change', async () => {
      if (baseSelect.value === 'pick') {
        // Another file as the base: picked as Merge Changes From picks one.
        let picked = null;
        try {
          picked = await invoke('merge_pick_file');
        } catch (e) {
          error.textContent = String(e);
          error.hidden = false;
        }
        if (!picked) {
          baseSelect.value = baseValueOf(choice.base);
          return;
        }
        choice.base = { kind: 'file', token: picked.token };
        choice.picks = {};
        refresh();
        return;
      }
      choice.base = baseChoiceOf(baseSelect.value);
      // Conflict keys belong to one base's merge: start the picks again.
      choice.picks = {};
      refresh();
    });

    function draw() {
      baseSelect.replaceChildren(
        el('option', { text: 'Found automatically', attrs: { value: 'auto' } }),
        ...dto.bases.map((b) => el('option', { text: b.label, attrs: { value: b.id } })),
        el('option', { text: 'Another file…', attrs: { value: 'pick' } }),
      );
      baseSelect.value = baseValueOf(choice.base);
      baseWhy.textContent = dto.base_label ? `${dto.base_label}. ${dto.base_reason}` : dto.base_reason;
      baseWhy.classList.toggle('dm-warn', !dto.base_clear);

      const parts = [];
      if (dto.problem) parts.push(el('p', { class: 'dm-error', text: dto.problem }));
      else {
        parts.push(el('h3', { class: 'dm-head', text: dto.conflicts.length ? `Conflicts (${dto.conflicts.length})` : 'No conflicts' }));
        if (dto.conflicts.length) {
          parts.push(el('p', { class: 'dm-note', text: 'Both copies changed these differently. Yours is kept unless you pick theirs.' }));
          parts.push(
            el(
              'ul',
              { class: 'dm-conflicts' },
              ...dto.conflicts.map((c) =>
                conflictNode(
                  c,
                  (key, side) => {
                    if (side === 'ours') delete choice.picks[key];
                    else choice.picks[key] = side;
                    focusPick = { key, side };
                    refresh();
                  },
                  goTo,
                ),
              ),
            ),
          );
        }
        parts.push(el('h3', { class: 'dm-head', text: dto.count ? `What will change here (${plural(dto.count, 'change', 'changes')})` : 'What will change here' }));
        if (dto.count) parts.push(...changeList(dto.changes, goTo));
        else parts.push(el('p', { class: 'dm-note', text: `Nothing changes: this schedule already has what it takes from ${dto.theirs}.` }));
        if (dto.renumbered) parts.push(el('p', { class: 'dm-note', text: `${plural(dto.renumbered, 'thing', 'things')} from ${dto.theirs} got new ids, as this schedule added different ones with the same ids.` }));
        if (dto.fallbacks.length) {
          parts.push(el('h3', { class: 'dm-head', text: 'Kept yours so the schedule hangs together' }));
          parts.push(el('ul', { class: 'dm-list' }, ...dto.fallbacks.map((f) => el('li', { class: 'dm-change', text: f }))));
        }
      }
      body.replaceChildren(...parts);
      merge.disabled = Boolean(dto.problem) || dto.count === 0;
      if (focusPick) {
        const want = focusPick;
        focusPick = null;
        const box = [...body.querySelectorAll('input[type="radio"]')].find((b) => b.dataset.key === want.key && b.value === want.side);
        if (box) box.focus();
      }
    }

    draw();
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'dm-title' } },
      el('h2', { class: 'modal-title', text: `Merge Changes From ${dto.theirs}`, attrs: { id: 'dm-title' } }),
      el('p', { class: 'modal-message', text: `What changed in ${dto.theirs} comes into ${dto.ours}. The merge compares both with the version they started from, the base.${onGo ? ' Click a link to see it here; the merge waits, with your picks, until you come back.' : ''}` }),
      el('div', { class: 'dm-base' }, el('label', { class: 'field', attrs: { for: 'dm-base' } }, 'Base', baseSelect), baseWhy),
      error,
      body,
      el('div', { class: 'modal-actions' }, cancel, merge),
    );
  });
}
