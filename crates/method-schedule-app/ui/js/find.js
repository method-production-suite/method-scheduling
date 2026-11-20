// Edit > Find and Edit > Replace: a panel beside the tabs (src/find.rs, decisions.md 28 and
// 34). After Method Budget's Find tab (its ui/js/find.js), scaled down to the engine's
// typed search.
//
// A search has its text, Match case, Whole word, a field (everything, every sheet field,
// one header field, element names, category names, banner text or event names) and a
// scope (the whole schedule, the sub-board picked in the board bar, or the shown board's
// Boneyard: its unscheduled scenes and their elements). Results come in
// board order with their place and a line of context; a click goes there (main.js does
// the going), Enter and Shift+Enter in the search box, or Find Next and Find Previous
// anywhere, step through them. The panel searches again after every change to the
// schedule, so places that no longer match drop out.
//
// Replace shows a "Replace with" box and a checkbox on every result, all checked to begin
// with; Replace changes the checked ones as one undo step and says how many places
// changed, or why nothing did. Schedule text reaches the page only through textContent
// and input values.

import { el } from './dom.js';
import { withShortcut } from './shortcuts.js';
import { keepRoom } from './modal.js';
import { FIND_JOINS, FIND_OPS, FIND_REPLACE_HINT } from './logic.js';

/**
 * What a search looks at: a FieldFilter key from src/find.rs (method_schedule_core::find),
 * or '' for everything. One entry per line between the markers: a Rust test compares the
 * keys with the engine's.
 */
const FIELDS = [
  ['', 'Everything'],
  ['sheet', 'Every sheet field'],
  ['number', 'Sheet: Sheet number'],
  ['scenes', 'Sheet: Scenes'],
  ['int_ext', 'Sheet: I/E'],
  ['set', 'Sheet: Set'],
  ['time_of_day', 'Sheet: Time of day'],
  ['synopsis', 'Sheet: Synopsis'],
  ['location', 'Sheet: Location'],
  ['script_pages', 'Sheet: Script pages'],
  ['script_day', 'Sheet: Script day'],
  ['unit', 'Sheet: Unit'],
  ['sequence', 'Sheet: Sequence'],
  ['episode', 'Sheet: Episode'],
  ['comments', 'Sheet: Comments'],
  ['element', 'Element names'],
  ['category', 'Category names'],
  ['banner', 'Banner text'],
  ['event', 'Event names'],
];
// End of FIELDS.

/**
 * An advanced find's rows (decisions.md, entry 82; the engine's find::Condition, entry
 * 77): the fields a condition can look at, and how it compares.
 */
const COND_FIELDS = [
  ...FIELDS.filter(([k]) => k && !['sheet', 'element', 'category', 'banner', 'event'].includes(k)).map(([k, label]) => [k, label.replace('Sheet: ', '')]),
  ['pages', 'Pages'],
  ['shoot_time', 'Shoot time'],
  ['screen_time', 'Screen time'],
];
const NUMBER_FIELDS = new Set(['pages', 'shoot_time', 'screen_time']);
const OPS = FIND_OPS;
const TEXT_ONLY = new Set(['contains', 'not_contains', 'starts_with']);
const NO_VALUE = new Set(['empty', 'not_empty']);
const HINT = { pages: '2 or 1 3/8', shoot_time: 'h:mm', screen_time: 'm:ss' };

const SEARCH_DELAY_MS = 180;

/**
 * Builds the panel in `host` (hidden until opened). `ctx`:
 *   search(query)                  the find_text command, resolves with { count, hits, truncated }
 *   replace(query, with, picks)    the replace_text command; resolves with the notice
 *   goTo(hit, { keepFocus })       shows the result's place
 *   subBoard()                     { id, name } of the sub-board picked, or null
 *   board()                        { id, name, many } of the board shown (many: more than one), or null
 *   found(hits)                    the results now (none when closed), for the board's marks
 *   selectOnBoard(hits)            selects the board's strips the results name
 *   closed()                       after the panel closes
 * Returns { open({ replace }), close(), isOpen(), refresh(), step(delta), focusBox() }.
 */
export function createFind(host, ctx) {
  const st = {
    text: '',
    matchCase: false,
    wholeWord: false,
    field: '',
    scope: 'schedule',
    replace: false,
    withText: '',
    // Advanced find: rows of { field, op, value }, joined by AND (any false) or OR.
    advanced: false,
    conds: [],
    any: false,
    found: null, // { count, hits, truncated }
    sig: '', // the query `found` answers
    error: '',
    message: '',
    messageEpoch: -1,
    epoch: 0, // counts refreshes after edits
    unchecked: new Set(), // result keys the user unchecked
    current: null, // key of the result last gone to
    currentIndex: -1,
    busy: false,
    timer: 0,
    seq: 0,
  };

  // ---- The form, built once.
  const title = el('h2', { class: 'fp-title', text: 'Find' });
  const close = el('button', { class: 'btn', text: 'Close', attrs: { type: 'button' }, on: { click: () => api.close() } });
  const text = el('input', {
    class: 'fp-text',
    attrs: { id: 'find-text', type: 'search', placeholder: 'Find', 'aria-label': 'Find', autocomplete: 'off', spellcheck: 'false', 'aria-keyshortcuts': 'Enter Shift+Enter' },
  });
  const prev = el('button', { class: 'btn', text: 'Previous', attrs: { type: 'button', id: 'find-prev' } });
  const next = el('button', { class: 'btn', text: 'Next', attrs: { type: 'button', id: 'find-next' } });
  prev.title = withShortcut('Previous result', 'edit.find_previous');
  next.title = withShortcut('Next result', 'edit.find_next');
  const matchCase = el('input', { attrs: { id: 'find-case', type: 'checkbox' } });
  const wholeWord = el('input', { attrs: { id: 'find-word', type: 'checkbox' } });
  const field = el('select', { attrs: { id: 'find-field', 'aria-label': 'Look in' } }, ...FIELDS.map(([k, label]) => el('option', { text: label, attrs: { value: k } })));
  const scope = el('select', { attrs: { id: 'find-scope', 'aria-label': 'Where' } });
  const mode = el('button', { class: 'btn', attrs: { type: 'button', id: 'find-mode', 'aria-pressed': 'false' } });
  const withBox = el('input', {
    class: 'fp-text',
    attrs: { id: 'find-with', type: 'text', placeholder: FIND_REPLACE_HINT.placeholder, title: FIND_REPLACE_HINT.title, 'aria-label': 'Replace with', autocomplete: 'off', spellcheck: 'false' },
  });
  const go = el('button', { class: 'btn btn-primary', text: 'Replace', attrs: { type: 'button', id: 'find-replace' } });
  const replaceRow = el('div', { class: 'fp-row fp-replace' }, withBox, go);
  const status = el('div', { class: 'fp-status', attrs: { role: 'status', 'aria-live': 'polite' } });
  const error = keepRoom(el('p', { class: 'fp-error', attrs: { role: 'alert' } }));
  const results = el('div', { class: 'fp-results' });
  const onBoard = el('button', {
    class: 'btn',
    text: 'Select on Board',
    attrs: { type: 'button', id: 'find-on-board', title: 'Select every strip on the board shown whose scene or banner is in the results' },
  });
  const advToggle = el('button', { class: 'btn', text: 'Advanced…', attrs: { type: 'button', id: 'find-advanced', 'aria-pressed': 'false', title: 'Find sheets by their fields: rows of field, comparison and value, joined by AND or OR' } });
  const joinPick = el('select', { attrs: { id: 'find-join', 'aria-label': 'Join the rows by', title: 'All rows: a sheet meets every row (AND). Any row: it meets one of them (OR).' } }, ...FIND_JOINS.map(([k, label]) => el('option', { text: label, attrs: { value: k } })));
  const condRows = el('div', { class: 'fp-conds', attrs: { role: 'group', 'aria-label': 'Conditions' } });
  const addCond = el('button', { class: 'btn', text: 'Add Row', attrs: { type: 'button', id: 'find-add-row' } });
  const advanced = el('div', { class: 'fp-advanced' }, el('div', { class: 'fp-row' }, el('label', { class: 'fp-label', attrs: { for: 'find-join' }, text: 'Match' }), joinPick, addCond), condRows, el('p', { class: 'fp-hint', text: 'Only sheets that meet the rows are searched. With nothing typed above, each such sheet is listed.' }));
  advanced.hidden = true;

  host.replaceChildren(
    el('div', { class: 'fp-head' }, title, close),
    el('div', { class: 'fp-row', attrs: { role: 'search', 'aria-label': 'Find in the schedule' } }, text, prev, next),
    el(
      'div',
      { class: 'fp-row fp-options' },
      el('label', { class: 'el-check', attrs: { for: 'find-case' } }, matchCase, el('span', { text: 'Match case' })),
      el('label', { class: 'el-check', attrs: { for: 'find-word' } }, wholeWord, el('span', { text: 'Whole word' })),
    ),
    el('div', { class: 'fp-row' }, el('label', { class: 'fp-label', attrs: { for: 'find-field' }, text: 'Look in' }), field),
    el('div', { class: 'fp-row' }, el('label', { class: 'fp-label', attrs: { for: 'find-scope' }, text: 'Where' }), scope),
    el('div', { class: 'fp-row' }, mode, advToggle, onBoard),
    advanced,
    replaceRow,
    error,
    status,
    results,
  );
  host.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      api.close();
    }
  });

  // ---- Reading the form.

  /** The rows that are filled in enough to send. */
  const liveConds = () => (st.advanced ? st.conds.filter((c) => NO_VALUE.has(c.op) || c.value.trim() || c.op === 'is' || c.op === 'is_not') : []);

  function query() {
    const conds = liveConds();
    if (!st.text && !conds.length) return null;
    const sub = st.scope === 'sub' ? ctx.subBoard() : null;
    const yard = st.scope === 'boneyard' ? ctx.board() : null;
    const q = { text: st.text, match_case: st.matchCase, whole_word: st.wholeWord, field: st.field || null, sub_board: sub ? sub.id : null, boneyard: yard ? yard.id : null };
    if (conds.length) {
      q.conditions = conds.map((c) => ({ field: c.field, op: c.op, value: NO_VALUE.has(c.op) ? '' : c.value }));
      q.any = st.any;
    }
    return q;
  }

  /** One condition row: field, comparison, value and a remove button. */
  function condRow(c, i) {
    const field = el('select', { class: 'fp-cond-field', attrs: { 'aria-label': `Row ${i + 1} field` } }, ...COND_FIELDS.map(([k, label]) => el('option', { text: label, attrs: { value: k } })));
    field.value = c.field;
    const ops = OPS.filter(([k]) => !(NUMBER_FIELDS.has(c.field) && TEXT_ONLY.has(k)));
    if (!ops.some(([k]) => k === c.op)) c.op = ops[0][0];
    const op = el('select', { class: 'fp-cond-op', attrs: { 'aria-label': `Row ${i + 1} comparison` } }, ...ops.map(([k, label]) => el('option', { text: label, attrs: { value: k } })));
    op.value = c.op;
    const value = el('input', { class: 'fp-cond-value', attrs: { type: 'text', 'aria-label': `Row ${i + 1} value`, placeholder: HINT[c.field] || '', autocomplete: 'off', spellcheck: 'false', value: c.value } });
    value.hidden = NO_VALUE.has(c.op);
    const remove = el('button', { class: 'btn btn-mini fp-cond-x', text: '×', title: 'Remove this row', attrs: { type: 'button', 'aria-label': `Remove row ${i + 1}` } });
    field.addEventListener('change', () => {
      c.field = field.value;
      drawConds();
      searchSoon();
    });
    op.addEventListener('change', () => {
      c.op = op.value;
      value.hidden = NO_VALUE.has(c.op);
      searchSoon();
    });
    value.addEventListener('input', () => {
      c.value = value.value;
      searchSoon();
    });
    remove.addEventListener('click', () => {
      st.conds.splice(i, 1);
      drawConds();
      search();
    });
    return el('div', { class: 'fp-row fp-cond' }, field, op, value, remove);
  }

  function drawConds() {
    condRows.replaceChildren(...st.conds.map(condRow));
  }

  advToggle.addEventListener('click', () => {
    st.advanced = !st.advanced;
    if (st.advanced && !st.conds.length) st.conds.push({ field: 'scenes', op: 'contains', value: '' });
    drawConds();
    draw();
    search();
  });
  addCond.addEventListener('click', () => {
    st.conds.push({ field: 'set', op: 'contains', value: '' });
    drawConds();
    const v = condRows.querySelectorAll('select');
    if (v.length) v[v.length - 2].focus();
  });
  joinPick.addEventListener('change', () => {
    st.any = joinPick.value === 'or';
    search();
  });

  text.addEventListener('input', (e) => {
    st.text = text.value;
    if (e.isComposing) return;
    st.message = '';
    searchSoon();
  });
  text.addEventListener('compositionend', () => {
    st.text = text.value;
    searchSoon();
  });
  text.addEventListener('keydown', (e) => {
    if (e.isComposing) return;
    if (e.key === 'Enter' && !e.altKey && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      e.stopPropagation();
      step(e.shiftKey ? -1 : 1, true);
    }
  });
  prev.addEventListener('click', () => step(-1, false));
  next.addEventListener('click', () => step(1, false));
  matchCase.addEventListener('change', () => {
    st.matchCase = matchCase.checked;
    search();
  });
  wholeWord.addEventListener('change', () => {
    st.wholeWord = wholeWord.checked;
    search();
  });
  field.addEventListener('change', () => {
    st.field = field.value;
    search();
  });
  scope.addEventListener('change', () => {
    st.scope = scope.value === 'sub' || scope.value === 'boneyard' ? scope.value : 'schedule';
    search();
  });
  mode.addEventListener('click', () => {
    st.replace = !st.replace;
    st.message = '';
    draw();
    if (st.replace) withBox.focus();
  });
  withBox.addEventListener('input', () => {
    st.withText = withBox.value;
  });
  withBox.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.isComposing && !e.altKey && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
      e.preventDefault();
      e.stopPropagation();
      doReplace();
    }
  });
  go.addEventListener('click', () => doReplace());
  onBoard.addEventListener('click', () => ctx.selectOnBoard(hits()));

  // ---- Searching.

  async function search() {
    clearTimeout(st.timer);
    st.timer = 0;
    const q = query();
    const my = ++st.seq;
    const sig = q ? JSON.stringify(q) : '';
    if (sig !== st.sig) {
      st.unchecked.clear();
      st.current = null;
      st.currentIndex = -1;
    }
    if (!q) {
      st.found = null;
      st.sig = '';
      st.error = '';
      draw();
      return;
    }
    try {
      const found = await ctx.search(q);
      if (my !== st.seq) return;
      st.found = found && Array.isArray(found.hits) ? found : { count: 0, hits: [], truncated: false };
      st.error = '';
    } catch (e) {
      if (my !== st.seq) return;
      st.found = null;
      st.error = String(e);
    }
    st.sig = sig;
    draw();
  }

  function searchSoon() {
    clearTimeout(st.timer);
    st.timer = setTimeout(search, SEARCH_DELAY_MS);
  }

  const hits = () => (st.found && Array.isArray(st.found.hits) ? st.found.hits : []);

  async function step(delta, keepFocus) {
    if (st.timer) await search();
    const list = hits();
    if (!list.length) return;
    let i = st.current ? list.findIndex((h) => h.key === st.current) : -1;
    let at;
    if (i >= 0) at = (i + delta + list.length) % list.length;
    else if (st.currentIndex >= 0) {
      // The one we were on is gone (replaced): carry on from its place.
      i = Math.min(st.currentIndex, list.length);
      at = delta > 0 ? i % list.length : (i - 1 + list.length) % list.length;
    } else at = delta > 0 ? 0 : list.length - 1;
    await goTo(list[at], at, keepFocus);
  }

  async function goTo(h, index, keepFocus) {
    st.current = h.key;
    st.currentIndex = index;
    markCurrent();
    await ctx.goTo(h, { keepFocus });
    if (keepFocus) text.focus({ preventScroll: true });
  }

  // ---- Replacing.

  const checked = () => hits().filter((h) => !st.unchecked.has(h.key));

  async function doReplace() {
    const q = query();
    const picks = checked().map((h) => h.key);
    if (!q || !picks.length || st.busy) return;
    st.busy = true;
    st.error = '';
    st.message = '';
    draw();
    let done = false;
    try {
      const notice = await ctx.replace(q, st.withText, picks);
      st.message = notice || '';
      st.messageEpoch = st.epoch;
      st.current = null;
      st.currentIndex = -1;
      done = true;
    } catch (e) {
      st.error = String(e);
    } finally {
      st.busy = false;
    }
    // A refusal changed nothing: keep the results and the sentence saying why.
    if (done) await search();
    else draw();
  }

  // ---- Drawing.

  function drawScope() {
    const sub = ctx.subBoard();
    const options = [el('option', { text: 'The whole schedule', attrs: { value: 'schedule' } })];
    if (sub) options.push(el('option', { text: `Sub-board ${sub.name}`, attrs: { value: 'sub' } }));
    const board = ctx.board();
    if (board) options.push(el('option', { text: board.many ? `The Boneyard of ${board.name}` : 'The Boneyard', attrs: { value: 'boneyard' } }));
    scope.replaceChildren(...options);
    if ((!sub && st.scope === 'sub') || (!board && st.scope === 'boneyard')) st.scope = 'schedule';
    scope.value = st.scope;
  }

  function draw() {
    title.textContent = st.replace ? 'Find and Replace' : 'Find';
    mode.textContent = st.replace ? 'Hide Replace' : 'Replace…';
    mode.title = withShortcut('Show or hide Replace', 'edit.replace');
    mode.setAttribute('aria-pressed', String(st.replace));
    replaceRow.hidden = !st.replace;
    advanced.hidden = !st.advanced;
    advToggle.textContent = st.advanced ? 'Hide Advanced' : 'Advanced…';
    advToggle.setAttribute('aria-pressed', String(st.advanced));
    joinPick.value = st.any ? 'or' : 'and';
    if (text.value !== st.text) text.value = st.text;
    if (withBox.value !== st.withText) withBox.value = st.withText;
    matchCase.checked = st.matchCase;
    wholeWord.checked = st.wholeWord;
    field.value = st.field;
    drawScope();
    const n = checked().length;
    go.disabled = st.busy || !n || !query() || !st.text;
    go.title = n ? `Replace in the ${n} checked ${n === 1 ? 'result' : 'results'}` : 'Check one or more results first';
    prev.disabled = next.disabled = !hits().length;
    // The board marks every strip the results name (decision 130).
    const shown = st.error || !query() ? [] : hits();
    onBoard.disabled = !shown.some((h) => h.kind === 'sheet' || h.kind === 'banner');
    ctx.found(shown);
    error.textContent = st.error;
    error.hidden = !st.error;
    drawStatus();
    drawResults();
  }

  function drawStatus() {
    const lines = [];
    if (st.message) lines.push(el('p', { class: 'fp-message', text: st.message }));
    const f = st.found;
    if (!query()) lines.push(el('p', { class: 'fp-hint', text: st.advanced ? 'Type to search, or fill in a row to list the sheets that meet it.' : 'Type to search the schedule.' }));
    else if (st.error) {
      // The error says it.
    } else if (!f) lines.push(el('p', { class: 'fp-hint', text: 'Searching…' }));
    else if (!f.count) lines.push(el('p', { class: 'fp-hint', text: 'Nothing found.' }));
    else {
      let t = !st.text ? (f.count === 1 ? '1 sheet meets the rows.' : `${f.count} sheets meet the rows.`) : f.count === 1 ? '1 place matches.' : `${f.count} places match.`;
      if (f.truncated) t += ` Showing the first ${f.hits.length}.`;
      lines.push(el('p', { class: 'fp-count', text: t }));
    }
    status.replaceChildren(...lines);
  }

  function setChecked(keys, on) {
    for (const k of keys) {
      if (on) st.unchecked.delete(k);
      else st.unchecked.add(k);
    }
    st.message = '';
    draw();
  }

  function drawResults() {
    const list = hits();
    if (!list.length || st.error) {
      results.replaceChildren();
      return;
    }
    const out = [];
    if (st.replace) {
      const all = el('input', { attrs: { id: 'find-all', type: 'checkbox' } });
      const on = list.filter((h) => !st.unchecked.has(h.key)).length;
      all.checked = on === list.length;
      all.indeterminate = on > 0 && on < list.length;
      all.addEventListener('change', () => setChecked(list.map((h) => h.key), all.checked));
      out.push(el('label', { class: 'el-check fp-all', attrs: { for: 'find-all' } }, all, el('span', { text: 'All results' })));
    }
    const ul = el('ul', { class: 'fp-list', attrs: { 'aria-label': 'Results' } });
    list.forEach((h, index) => {
      const li = el('li', { class: 'fp-hit' });
      if (st.replace) {
        const box = el('input', { attrs: { type: 'checkbox', 'aria-label': `Replace here: ${h.place}` }, data: { key: h.key } });
        box.checked = !st.unchecked.has(h.key);
        box.addEventListener('change', () => setChecked([h.key], box.checked));
        li.append(box);
      }
      const b = el(
        'button',
        { class: 'fp-entry', attrs: { type: 'button', title: `Go to ${h.place}` }, data: { key: h.key } },
        el('span', { class: 'fp-place', text: h.place }),
        el('span', { class: 'fp-context', text: h.context || '(blank)' }),
      );
      b.addEventListener('click', () => goTo(h, index, false));
      b.addEventListener('keydown', (e) => {
        const by = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
        if (!by || e.altKey || e.metaKey || e.ctrlKey) return;
        const all = [...results.querySelectorAll('.fp-entry')];
        const to = all[all.indexOf(b) + by];
        if (to) {
          e.preventDefault();
          to.focus();
        }
      });
      li.append(b);
      ul.append(li);
    });
    out.push(ul);
    results.replaceChildren(...out);
    markCurrent();
  }

  /** Marks the result last gone to, and scrolls it into view in the list. */
  function markCurrent() {
    for (const b of results.querySelectorAll('.fp-entry')) {
      const on = b.dataset.key === st.current;
      if (on) {
        b.setAttribute('aria-current', 'true');
        if (typeof b.scrollIntoView === 'function') b.scrollIntoView({ block: 'nearest' });
      } else b.removeAttribute('aria-current');
    }
  }

  const api = {
    /** Shows the panel (Replace mode or not) with focus in the search box. */
    open({ replace = false } = {}) {
      if (replace !== st.replace) {
        st.replace = replace;
        st.message = '';
      }
      host.hidden = false;
      draw();
      search();
      text.focus();
      text.select();
    },
    close() {
      if (host.hidden) return;
      clearTimeout(st.timer);
      st.timer = 0;
      host.hidden = true;
      ctx.found([]);
      ctx.closed();
    },
    isOpen: () => !host.hidden,
    /** After every change to the schedule (or to the board or sub-board shown). */
    refresh() {
      st.epoch++;
      if (st.message && st.messageEpoch !== st.epoch) st.message = '';
      if (host.hidden) return;
      search();
    },
    /** Find Next (1) and Find Previous (-1); with nothing to find yet, opens the panel. */
    async step(delta) {
      if (!query()) {
        api.open({ replace: st.replace });
        return;
      }
      if (host.hidden) {
        host.hidden = false;
        draw();
        await search();
      }
      await step(delta, false);
    },
    focusBox() {
      text.focus({ preventScroll: true });
    },
  };
  host.hidden = true;
  return api;
}
