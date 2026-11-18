// The Day out of days tab: the grid for the sub-board picked in the board bar and one
// category (dood_grid, src/view.rs). One row per element in the engine's order, one column
// per date from the first to the last date any row covers, the codes the engine gives
// (SW, W, WF, SWF, SWD, PW, PWF, H, FTH, TR, /, M ...), days off shaded, and each row's
// summary at the right.
//
// Readable at 100+ columns: the ID and name columns, the summary columns and the header
// stay put while the dates scroll. With many rows only the rows in view are drawn; the
// space above and below is two spacer rows sized through a constructed stylesheet
// (dynstyle.js), as the CSP allows no inline styles.
//
// Sorting, the switches and the views (decision 59): a click on the ID, Element, Work,
// Hold, Start or Finish heading sorts by it (again: the other way); Sort… sorts by
// up to four keys. The sort lasts until the tab or sub-board changes and is never saved
// (behavior note 27). Each row's H and D boxes are the element's hold and drop switches
// (one undo step each). Without holds shows the grid
// as if no element held; Vertical turns it on its side (a row per date, a column per
// element); Codes… changes the code letters the schedule keeps.
//
// Explain this count (src/explain.rs): a click on a row's Work, Hold or Total, or Enter
// (or Explain This Count) on the selected row, opens a panel beside the grid with the
// engine's sentences: the totals, every gap and what became of it, and every date. A
// click on a date there scrolls the grid to its column and marks it. Click a row's ID or
// name to select it; Up and Down move the selection while the grid has focus.
//
// Right-click a cell (either way up) to explain that one day: Explain This Day opens the
// element's panel with the date marked and its line in view; Explain This Count opens it
// as the totals' buttons do.

import { el } from './dom.js';
import { setRules, dynamicStyles } from './dynstyle.js';
import { openMenu } from './ctxmenu.js';
import { customDialog, errorLine } from './modal.js';
import { dayDate, showDate } from './dates.js';
import { verticalGrid, weekendDays } from './logic.js';

/** Rows drawn at once before the grid draws only what is in view. */
const ALL_ROWS_UP_TO = 80;
/** Must match `.dood-grid tbody tr.r` in styles.css. */
const ROW_PX = 20;
/** Extra rows drawn above and below the visible ones. */
const OVERSCAN = 12;

const SUMMARY = [
  ['Work', 'work', 's-work'],
  ['Hold', 'hold', 's-hold'],
  ['Start', 'start', 's-start'],
  ['Finish', 'finish', 's-finish'],
  ['Total', 'total', 's-total'],
];

/** The headings a click sorts by, and the engine's key for each. */
const SORT_BY = { ID: 'board_id', Element: 'name', Work: 'work', Hold: 'hold', Start: 'start', Finish: 'finish' };

/** Every sort key the engine takes, labeled as the reference app's Sort dialog names them (behavior note 19). */
const SORT_KEYS = [
  ['board_id', 'Board ID'],
  ['name', 'Element name'],
  ['start', 'First working day'],
  ['finish', 'Last working day'],
  ['work', 'Worked days'],
  ['hold', 'Held days'],
];

const LEGEND_TAIL =
  'An event on a work day shows the event code, then the work code. Shaded columns are days off; a red outline marks a red flag clash, a dashed one an event on a work day (a conflict).';

/** The legend in the schedule's own letters (dto.codes: [key, label, letters]). */
function legendText(codes) {
  const list = Array.isArray(codes) && codes.length ? codes.map(([, label, code]) => `${code} ${label}`).join(', ') : 'S start, W work, F finish, H hold, D drop, P pickup, FT fitting, TR travel, R rehearsal, / holiday, M company travel day';
  return `${list}. ${LEGEND_TAIL}`;
}

/** What kind of code it is, in the schedule's letters, for its color. */
function codeClass(code, letters) {
  if (!code) return '';
  const c = letters || {};
  if (code === (c.hold || 'H')) return 'c-hold';
  if (code === (c.holiday || '/')) return 'c-hol';
  if (code === (c.company_move || 'M')) return 'c-move';
  const ev = [c.fitting || 'FT', c.travel || 'TR', c.rehearsal || 'R'];
  if (ev.some((e) => code.startsWith(e)) && !code.startsWith(c.work || 'W') && !code.startsWith(c.start || 'S') && !code.startsWith(c.pickup || 'P')) return 'c-ev';
  return 'c-work';
}

const KIND = { off: 'k-off', holiday: 'k-holiday', move: 'k-move', work: '' };

const OUTCOME = {
  dropped: 'Dropped',
  too_short: 'Held',
  another_gap_dropped: 'Held',
  drops_off: 'Held',
  holds_off: 'Counts nowhere',
};

const COUNTED = { work: 'x-work', hold: 'x-hold', nowhere: 'x-none' };

/** Sort…: up to four keys, each ascending or descending. Resolves with ['work:desc', ...], [] for none, or null. */
function sortDialog(current) {
  return customDialog((close) => {
    const rows = [0, 1, 2, 3].map((i) => {
      const [key, dir] = (current[i] || '').split(':');
      const pick = el('select', { attrs: { id: `dd-sort-${i}`, 'aria-label': i === 0 ? 'Sort by' : 'Then by' } }, el('option', { text: i === 0 ? '(the engine\'s order)' : '(none)', attrs: { value: '' } }), ...SORT_KEYS.map(([k, t]) => el('option', { text: t, attrs: { value: k } })));
      pick.value = key || '';
      const desc = el('select', { attrs: { id: `dd-dir-${i}`, 'aria-label': 'Direction' } }, el('option', { text: 'Ascending', attrs: { value: 'asc' } }), el('option', { text: 'Descending', attrs: { value: 'desc' } }));
      desc.value = dir === 'desc' ? 'desc' : 'asc';
      return { pick, desc, node: el('div', { class: 'el-row' }, el('label', { class: 'el-label', attrs: { for: `dd-sort-${i}` }, text: i === 0 ? 'Sort by' : 'Then by' }), pick, desc) };
    });
    const done = () => {
      const keys = [];
      for (const r of rows) if (r.pick.value && !keys.some((k) => k.startsWith(`${r.pick.value}:`))) keys.push(`${r.pick.value}:${r.desc.value}`);
      close(keys);
    };
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'Sort rows', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: 'Rows missing a value go last; ties keep their order. The sort lasts while this tab and sub-board are shown and is never saved.' }),
      ...rows.map((r) => r.node),
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn', text: 'Clear Sort', attrs: { type: 'button' }, on: { click: () => close([]) } }), el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }), el('button', { class: 'btn btn-primary', text: 'Sort', attrs: { type: 'button', 'data-autofocus': true }, on: { click: done } })),
    );
  });
}

/** Codes…: the schedule's code letters. Resolves with { key: letters } for those changed, or null. */
function codesDialog(codes, submit) {
  return customDialog((close) => {
    const inputs = codes.map(([key, label, code]) => ({ key, code, input: el('input', { class: 'w-xs', attrs: { id: `dd-code-${key}`, type: 'text', autocomplete: 'off', maxlength: '3', value: code } }), label }));
    const problem = errorLine();
    const save = async (all) => {
      const out = {};
      for (const f of inputs) if (all || f.input.value.trim() !== f.code) out[f.key] = all ? all[f.key] : f.input.value.trim();
      try {
        await submit(out);
        close(true);
      } catch (e) {
        problem.textContent = String(e);
        problem.hidden = false;
      }
    };
    const usual = { start: 'S', work: 'W', finish: 'F', hold: 'H', drop: 'D', pickup: 'P', travel: 'TR', fitting: 'FT', rehearsal: 'R', holiday: '/', company_move: 'M' };
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'Code letters', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: 'Saved in the schedule. Each code is 1 to 3 letters, digits or /, and no two may be the same.' }),
      problem,
      el('div', { class: 'dd-codes' }, ...inputs.map((f) => el('label', { class: 'bd-field beside', attrs: { for: `dd-code-${f.key}` } }, el('span', { text: f.label }), f.input))),
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn', text: 'Usual Letters', attrs: { type: 'button' }, on: { click: () => save(usual) } }), el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }), el('button', { class: 'btn btn-primary', text: 'Save', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => save(null) } })),
    );
  });
}

/**
 * Builds the tab in `host`. `actions.pickCategory(id)` shows another category, and
 * `actions.pickHeader(key)` a header field (set, location, I/E and the rest) as one;
 * `actions.explain(element)` resolves with explain_count's panel; `actions.say(text,
 * isError)` shows a message; `actions.sort(keys)` and `actions.noHolds(on)` change the
 * session's view; `actions.setRule(element, field, on)` and `actions.setCodes(map)` change
 * the schedule (each resolves, or rejects with a sentence). Returns { render(dto),
 * explainSelected() }.
 */
/**
 * Whether Saturdays and Sundays show, each remembered for this viewer (a view choice, not
 * schedule data; decisions 121 and 158). The old single Weekends key is read for a day
 * whose own key was never set.
 */
const WEEKENDS_KEY = 'method-schedule.dood.weekends';
const DAY_KEYS = { sat: 'method-schedule.dood.saturdays', sun: 'method-schedule.dood.sundays' };

/** The DOOD tab's weekend switches: { sat, sun }. */
export function loadWeekends() {
  try {
    const get = (k) => window.localStorage.getItem(k);
    return weekendDays(get(WEEKENDS_KEY), get(DAY_KEYS.sat), get(DAY_KEYS.sun));
  } catch {
    return { sat: true, sun: true };
  }
}

function saveWeekend(day, on) {
  try {
    window.localStorage.setItem(DAY_KEYS[day], on ? '1' : '0');
  } catch {
    // Storage may be off; the choice still holds until the page reloads.
  }
}

/** A Saturday or Sunday column (or vertical row), hidden when its switch is off. */
const weekendClass = (d) => (d.weekday === 'Sat' ? ' g-sat' : d.weekday === 'Sun' ? ' g-sun' : '');

export function createDood(host, actions) {
  let dto = null;
  let sortKeys = [];
  let vertical = false;
  let letters = {};
  let drawn = { first: -1, last: -1 };
  /** The selected row's element, the explained one, and the date marked from the panel. */
  let selected = null;
  let explained = null;
  let marked = null;

  const title = el('h2', {});
  const picker = el('select', { attrs: { id: 'dood-category', 'aria-label': 'Category' } });
  picker.addEventListener('change', () => {
    const v = picker.value;
    if (v.startsWith('h:')) actions.pickHeader(v.slice(2));
    else actions.pickCategory(Number(v.slice(2)));
  });
  const sortBtn = el('button', { class: 'btn', text: 'Sort…', attrs: { type: 'button', id: 'dood-sort' }, on: { click: () => sortDialog(sortKeys).then((k) => k && actions.sort(k)) } });
  const noHolds = el('input', { attrs: { id: 'dood-no-holds', type: 'checkbox' } });
  noHolds.addEventListener('change', () => actions.noHolds(noHolds.checked));
  const turn = el('input', { attrs: { id: 'dood-vertical', type: 'checkbox' } });
  turn.addEventListener('change', () => {
    vertical = turn.checked;
    if (dto) render(dto);
  });
  // Show or hide Saturdays and Sundays, each on its own (behavior notes 24 and 27: the
  // reference app's View menu has a switch for each).
  const shownDays = loadWeekends();
  const dayBox = (day) => {
    const box = el('input', { attrs: { id: `dood-${day}`, type: 'checkbox' } });
    box.checked = shownDays[day];
    box.addEventListener('change', () => {
      saveWeekend(day, box.checked);
      table.classList.toggle(`no-${day}`, !box.checked);
    });
    return box;
  };
  const saturdays = dayBox('sat');
  const sundays = dayBox('sun');
  const codesBtn = el('button', { class: 'btn', text: 'Codes…', attrs: { type: 'button', id: 'dood-codes' }, on: { click: () => dto && codesDialog(dto.codes || [], actions.setCodes) } });
  const sortNote = el('span', { class: 'bd-hint dood-sorted' });
  const head = el(
    'div',
    { class: 'dood-head' },
    title,
    el('label', { class: 'field', attrs: { for: 'dood-category' } }, 'Category', picker),
    sortBtn,
    sortNote,
    el('label', { class: 'el-check', attrs: { for: 'dood-no-holds' }, title: 'As if no element had holds (and so no drops); not saved' }, noHolds, el('span', { text: 'Without holds' })),
    el('label', { class: 'el-check', attrs: { for: 'dood-vertical' }, title: 'A row per date, a column per element' }, turn, el('span', { text: 'Vertical' })),
    el('label', { class: 'el-check', attrs: { for: 'dood-sat' }, title: 'Show the Saturday columns, here and in the DOOD report; remembered on this computer' }, saturdays, el('span', { text: 'Saturdays' })),
    el('label', { class: 'el-check', attrs: { for: 'dood-sun' }, title: 'Show the Sunday columns, here and in the DOOD report; remembered on this computer' }, sundays, el('span', { text: 'Sundays' })),
    codesBtn,
  );
  const message = el('p', { class: 'empty' });
  const legend = el('p', { class: 'bd-hint', text: legendText(null) });
  const table = el('table', { class: 'dood-grid', attrs: { 'aria-label': 'Day out of days' } });
  table.classList.toggle('no-sat', !saturdays.checked);
  table.classList.toggle('no-sun', !sundays.checked);
  const thead = el('thead', {});
  const tbody = el('tbody', {});
  table.append(thead, tbody);
  const scroller = el('div', { class: 'dood-scroll', attrs: { tabindex: '0', 'aria-label': 'Day out of days grid' } }, table);
  const panel = el('aside', { class: 'dood-explain', attrs: { 'aria-label': 'Explain this count', tabindex: '-1' } });
  panel.hidden = true;
  const body = el('div', { class: 'dood-body' }, scroller, panel);
  host.replaceChildren(head, message, legend, body);

  const virtual = () => !vertical && dto && dto.rows.length > ALL_ROWS_UP_TO && dynamicStyles;

  function headerRow() {
    const cells = [
      el('th', { class: 'g-id', text: 'ID', attrs: { scope: 'col' } }),
      el('th', { class: 'g-name', text: 'Element', attrs: { scope: 'col' } }),
    ];
    dto.dates.forEach((d, i) => {
      const month = i === 0 || d.day === 1 ? d.month : '';
      cells.push(
        el(
          'th',
          { class: `g-date ${KIND[d.kind] || ''}${weekendClass(d)}${d.date === marked ? ' g-mark' : ''}`, title: `${dayDate(d.weekday, d.date)}${d.shoot_day ? `, shoot day ${d.shoot_day}` : ''}`, attrs: { scope: 'col' } },
          el('span', { class: 'g-month', text: month }),
          el('span', { class: 'g-day', text: String(d.day) }),
          el('span', { class: 'g-wd', text: d.weekday.slice(0, 2) }),
          el('span', { class: 'g-sd', text: d.shoot_day ? `D${d.shoot_day}` : '' }),
        ),
      );
    });
    // Total is no sort key of the engine's (DOD-12: the six of the Sort dialog), so its
    // heading does not sort.
    for (const [label, , cls] of SUMMARY) cells.push(SORT_BY[label] ? el('th', { class: `g-sum ${cls} g-sort`, text: label, title: `Sort by ${label.toLowerCase()}`, attrs: { scope: 'col' }, data: { sort: label } }) : el('th', { class: `g-sum ${cls}`, text: label, title: 'Work plus hold', attrs: { scope: 'col' } }));
    cells.push(el('th', { class: 'g-sum s-holds', text: 'H', title: 'Holds: the element\'s hold switch', attrs: { scope: 'col' } }));
    cells.push(el('th', { class: 'g-sum s-drops', text: 'D', title: 'Drops: the element\'s drop switch (needs holds)', attrs: { scope: 'col' } }));
    cells[0].classList.add('g-sort');
    cells[0].dataset.sort = 'ID';
    cells[1].classList.add('g-sort');
    cells[1].dataset.sort = 'Element';
    for (const c of cells) {
      const key = c.dataset && SORT_BY[c.dataset.sort];
      if (!key) continue;
      const on = sortKeys.find((k) => k.startsWith(`${key}:`));
      if (on && sortKeys[0] === on) c.setAttribute('aria-sort', on.endsWith(':desc') ? 'descending' : 'ascending');
    }
    return el('tr', {}, ...cells);
  }

  thead.addEventListener('click', (e) => {
    const h = e.target.closest('th.g-sort');
    if (!h) return;
    const key = SORT_BY[h.dataset.sort];
    const first = sortKeys[0] || '';
    // A heading sorts by its key; again, the other way.
    actions.sort([first === `${key}:asc` ? `${key}:desc` : `${key}:asc`]);
  });

  function rowNode(r) {
    const tr = el('tr', { class: r.element === selected ? 'r selected' : 'r', data: { element: r.element } });
    if (r.element === selected) tr.setAttribute('aria-selected', 'true');
    tr.append(el('td', { class: 'g-id g-pick', text: r.board_id, title: r.board_id || null }), el('th', { class: 'g-name g-pick', text: r.name, title: r.name, attrs: { scope: 'row' } }));
    const kinds = dto.dates.map((d) => (KIND[d.kind] || '') + weekendClass(d));
    const mark = marked === null ? -1 : dto.dates.findIndex((d) => d.date === marked);
    // Red flag clashes (src/redflags.rs): a red outline and the clash in the cell's title.
    // An event on a work day (src/view.rs, decisions.md 43): a dashed outline, a conflict.
    const flags = new Map((r.red_flags || []).map((f) => [f.at, f.text]));
    const warns = new Map((r.warnings || []).map((f) => [f.at, f.text]));
    r.cells.forEach((code, i) => {
      const span = r.span && i >= r.span[0] && i <= r.span[1] ? ' span' : '';
      const flag = flags.get(i);
      const warn = warns.get(i);
      const why = [flag ? `red flag: ${flag}` : '', warn || ''].filter(Boolean).join('; ');
      tr.append(
        el('td', {
          class: `g-cell ${kinds[i]} ${codeClass(code, letters)}${span}${i === mark ? ' g-mark' : ''}${flag ? ' g-flag' : ''}${warn ? ' g-warn' : ''}`,
          text: code,
          // The whole code too: a narrow cell may cut a long one (FTPW) with an ellipsis.
          title: why ? `${showDate(dto.dates[i].date)}: ${code ? `${code}, ` : ''}${why}` : code ? `${showDate(dto.dates[i].date)}: ${code}` : null,
          attrs: why ? { 'aria-label': `${code ? `${code}, ` : ''}${why}` } : {},
          data: { date: dto.dates[i].date },
        }),
      );
    });
    for (const [label, key, cls] of SUMMARY) {
      const v = r[key];
      const text = v === null || v === undefined ? '' : String(v);
      if (key === 'work' || key === 'hold' || key === 'total') {
        const btn = el('button', { class: 'g-explain', text, title: `Explain ${r.name}'s ${label.toLowerCase()}`, attrs: { type: 'button' }, data: { explain: r.element } });
        tr.append(el('td', { class: `g-sum ${cls}` }, btn));
      } else {
        const shown = key === 'start' || key === 'finish' ? showDate(text) : text;
        tr.append(el('td', { class: `g-sum ${cls}`, text: shown, title: shown || null }));
      }
    }
    // The element's own switches, edited from the row (one undo step each).
    for (const [field, cls, what] of [['holds', 's-holds', 'holds'], ['drops', 's-drops', 'drops']]) {
      const box = el('input', { class: 'g-rule', attrs: { type: 'checkbox', checked: r[field] === true, 'aria-label': `${r.name} ${what}` }, title: `${r.name}: ${what} ${r[field] ? 'on' : 'off'}`, data: { rule: field, element: r.element } });
      tr.append(el('td', { class: `g-sum ${cls}` }, box));
    }
    return tr;
  }

  tbody.addEventListener('change', (e) => {
    const box = e.target.closest('input.g-rule');
    if (!box) return;
    const on = box.checked;
    actions.setRule(Number(box.dataset.element), box.dataset.rule, on).catch((err) => {
      box.checked = !on;
      actions.say(err, true);
    });
  });

  const width = () => 2 + dto.dates.length + SUMMARY.length + 2;

  /** The grid on its side: a row per date, a column per element, the counts at the foot. */
  function drawVertical() {
    const v = verticalGrid(dto);
    const heads = [el('th', { class: 'v-date', text: 'Date', attrs: { scope: 'col' } }), el('th', { class: 'v-sd', text: 'Day', attrs: { scope: 'col' } })];
    dto.rows.forEach((r, i) => heads.push(el('th', { class: 'v-el', text: v.heads[i], title: r.name, attrs: { scope: 'col' } })));
    thead.replaceChildren(el('tr', {}, ...heads));
    const rows = v.rows.map((d) =>
      el(
        'tr',
        { class: `vr ${KIND[d.kind] || ''}${weekendClass(d)}` },
        el('th', { class: 'v-date', text: `${d.weekday.slice(0, 2)} ${showDate(d.date)}`, attrs: { scope: 'row' } }),
        el('td', { class: 'v-sd', text: d.day }),
        ...d.cells.map((c, j) => el('td', { class: `g-cell ${codeClass(c, letters)}`, text: c, title: c ? `${dto.rows[j].name}, ${showDate(d.date)}: ${c}` : null, data: { element: dto.rows[j].element, date: d.date } })),
      ),
    );
    for (const s of v.sums) {
      rows.push(el('tr', { class: 'vr v-sum' }, el('th', { class: 'v-date', text: s.label, attrs: { scope: 'row' } }), el('td', { class: 'v-sd' }), ...s.cells.map((c) => el('td', { class: 'g-cell', text: c }))));
    }
    tbody.replaceChildren(...rows);
  }

  function drawRows(force = false) {
    if (vertical) return;
    if (!dto || !dto.rows.length) {
      tbody.replaceChildren();
      return;
    }
    if (!virtual()) {
      if (force) tbody.replaceChildren(...dto.rows.map(rowNode));
      return;
    }
    const n = dto.rows.length;
    const headPx = thead.getBoundingClientRect().height || ROW_PX;
    const top = Math.max(0, scroller.scrollTop - headPx);
    const seen = Math.ceil((scroller.clientHeight || 600) / ROW_PX);
    const first = Math.max(0, Math.floor(top / ROW_PX) - OVERSCAN);
    const last = Math.min(n - 1, first + seen + 2 * OVERSCAN);
    if (!force && first === drawn.first && last === drawn.last) return;
    drawn = { first, last };
    setRules('dood-pad', `.dood-grid tr.pad-top { height: ${first * ROW_PX}px; } .dood-grid tr.pad-bottom { height: ${(n - 1 - last) * ROW_PX}px; }`);
    const pad = (cls) => el('tr', { class: cls, attrs: { 'aria-hidden': 'true' } }, el('td', { attrs: { colspan: String(width()) } }));
    const rows = [];
    for (let i = first; i <= last; i++) rows.push(rowNode(dto.rows[i]));
    tbody.replaceChildren(pad('pad pad-top'), ...rows, pad('pad pad-bottom'));
  }

  // Browsers send at most one scroll event a frame, and drawRows does nothing until the
  // rows in view change.
  scroller.addEventListener('scroll', () => {
    if (virtual()) drawRows();
  });

  function render(next) {
    const sameShape = dto && next && dto.category === next.category && dto.header === next.header && dto.dates.length === next.dates.length;
    dto = next;
    letters = Object.fromEntries((dto.codes || []).map(([k, , c]) => [k, c]));
    legend.textContent = legendText(dto.codes) + (dto.no_holds ? ' Shown without holds: no element holds or drops here.' : '');
    noHolds.checked = dto.no_holds === true;
    sortNote.textContent = sortKeys.length ? `Sorted by ${sortKeys.map((k) => { const [key, dir] = k.split(':'); return `${(SORT_KEYS.find((x) => x[0] === key) || [key, key])[1].toLowerCase()}${dir === 'desc' ? ' (descending)' : ''}`; }).join(', then ')}` : '';
    title.textContent = `Day out of days: ${dto.sub_board}`;
    // Categories, then the header fields that work as categories (inventory section 4).
    picker.replaceChildren(
      ...dto.categories.map((c) => el('option', { text: c.name, attrs: { value: `c:${c.id}` } })),
      el('optgroup', { attrs: { label: 'Header fields' } }, ...(dto.headers || []).map((h) => el('option', { text: h.label, attrs: { value: `h:${h.key}` } }))),
    );
    if (dto.header) picker.value = `h:${dto.header}`;
    else if (dto.category !== null) picker.value = `c:${dto.category}`;
    picker.disabled = !dto.categories.length;
    const empty = !dto.rows.length;
    message.hidden = !empty;
    message.textContent = empty ? dto.empty || 'Nothing to show.' : '';
    legend.hidden = empty;
    scroller.hidden = empty;
    if (empty) {
      thead.replaceChildren();
      tbody.replaceChildren();
      return;
    }
    if (!sameShape) scroller.scrollTop = 0;
    if (!dto.rows.some((r) => r.element === selected)) selected = null;
    if (marked !== null && !dto.dates.some((d) => d.date === marked)) marked = null;
    table.classList.toggle('dood-vertical', vertical);
    if (vertical) {
      table.classList.remove('virtual');
      drawVertical();
      return;
    }
    thead.replaceChildren(headerRow());
    table.classList.toggle('virtual', Boolean(virtual()));
    drawRows(true);
    // The schedule changed: the open explanation is worked out again.
    if (explained !== null) explain(explained, { quiet: true });
  }

  // ---- Selecting a row

  const rowIndex = (element) => (dto ? dto.rows.findIndex((r) => r.element === element) : -1);

  /** Selects a row by data, scrolling it into view (and drawing it, past 80 rows). */
  function select(element) {
    const i = rowIndex(element);
    if (i < 0) return;
    selected = element;
    const headPx = thead.getBoundingClientRect().height || ROW_PX;
    const top = i * ROW_PX;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (headPx + top + ROW_PX > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = headPx + top + ROW_PX - scroller.clientHeight;
    drawRows(true);
  }

  tbody.addEventListener('click', (e) => {
    const btn = e.target.closest('button.g-explain');
    const tr = e.target.closest('tr.r');
    if (!tr) return;
    const element = Number(tr.dataset.element);
    if (btn) {
      select(element);
      explain(element);
    } else if (e.target.closest('.g-pick')) {
      select(element);
      scroller.focus({ preventScroll: true });
    }
  });

  scroller.addEventListener('keydown', (e) => {
    if (!dto || !dto.rows.length || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const i = rowIndex(selected);
      const next = i < 0 ? (e.key === 'ArrowDown' ? 0 : dto.rows.length - 1) : Math.min(Math.max(i + (e.key === 'ArrowDown' ? 1 : -1), 0), dto.rows.length - 1);
      select(dto.rows[next].element);
      e.preventDefault();
    } else if (e.key === 'Enter' && selected !== null) {
      explain(selected);
      e.preventDefault();
    }
  });

  // A cell's menu: explain that day, or the row's count. Header field rows are values
  // worked out from the sheets, so both are off for them.
  table.addEventListener('contextmenu', (e) => {
    const td = e.target.closest && e.target.closest('td.g-cell');
    if (!td || !td.dataset.date || !dto) return;
    const tr = td.closest('tr');
    const element = Number(td.dataset.element || (tr && tr.dataset.element));
    const r = dto.rows.find((x) => x.element === element);
    if (!r) return;
    e.preventDefault();
    const iso = td.dataset.date;
    const off = Boolean(dto.header);
    openMenu(
      e.clientX,
      e.clientY,
      [
        { label: `Explain ${showDate(iso)}`, disabled: off, run: () => explainDay(element, iso) },
        { label: 'Explain This Count', disabled: off, run: () => explainRow(element) },
      ],
      { label: `${r.name}, ${showDate(iso)}`, back: scroller },
    );
  });

  function explainRow(element) {
    if (!vertical) select(element);
    explain(element);
  }

  /** The panel for `element` with `iso` marked and its line in view. */
  async function explainDay(element, iso) {
    if (!vertical) select(element);
    await explain(element);
    if (explained !== element || panel.hidden) return;
    goToDate(iso);
    const li = panel.querySelector('li.x-day.x-marked');
    if (li) li.scrollIntoView({ block: 'nearest' });
  }

  // ---- The explain panel

  /** Scrolls the grid to a date's column and marks it. */
  function goToDate(iso) {
    const i = dto ? dto.dates.findIndex((d) => d.date === iso) : -1;
    if (i < 0) {
      actions.say(`${iso} is not a column of this grid.`, true);
      return;
    }
    marked = iso;
    markInPanel(iso);
    // On its side the dates are rows and the heading is the elements': only the panel
    // marks the date.
    if (vertical) return;
    const cells = thead.rows[0] ? thead.rows[0].cells : null;
    if (cells && cells[2 + i]) {
      const th = cells[2 + i];
      const sticky = cells[0].offsetWidth + cells[1].offsetWidth;
      scroller.scrollLeft = Math.max(0, th.offsetLeft - sticky - 3 * th.offsetWidth);
    }
    thead.replaceChildren(headerRow());
    if (explained !== null && rowIndex(explained) >= 0) select(explained);
    else drawRows(true);
  }

  function markInPanel(iso) {
    for (const li of panel.querySelectorAll('li.x-day')) {
      const b = li.querySelector('button.x-date');
      li.classList.toggle('x-marked', Boolean(b) && b.dataset.date === iso);
    }
  }

  const dateButton = (iso) => el('button', { class: 'x-date', text: iso, title: `Show ${iso} in the grid`, attrs: { type: 'button' }, data: { date: iso } });

  panel.addEventListener('click', (e) => {
    const b = e.target.closest('button.x-date');
    if (b) goToDate(b.dataset.date);
  });
  panel.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      closePanel();
    }
  });

  function closePanel() {
    explained = null;
    panel.hidden = true;
    panel.replaceChildren();
    scroller.focus({ preventScroll: true });
  }

  function drawPanel(x) {
    const close = el('button', { class: 'btn', text: 'Close', attrs: { type: 'button' }, on: { click: closePanel } });
    const parts = [
      el('div', { class: 'x-head' }, el('h3', { class: 'x-title', text: `${x.name} (${x.category})` }), close),
      el('p', { class: 'x-totals', text: `Work ${x.work}, hold ${x.hold}, total ${x.total} on ${x.sub_board}` }),
      ...x.summary.map((t) => el('p', { class: 'x-line', text: t })),
    ];
    parts.push(el('h4', { class: 'x-sub', text: 'Gaps' }));
    if (x.gaps.length) {
      parts.push(
        el(
          'ul',
          { class: 'x-list' },
          ...x.gaps.map((g) =>
            el(
              'li',
              { class: `x-gap o-${g.outcome}` },
              el('span', { class: 'x-outcome', text: OUTCOME[g.outcome] || g.outcome }),
              el('span', { class: 'x-dates' }, dateButton(g.after), ' to ', dateButton(g.before)),
              el('span', { class: 'x-text', text: g.text }),
            ),
          ),
        ),
      );
    } else parts.push(el('p', { class: 'x-line x-muted', text: 'No gaps between work or event days.' }));
    parts.push(el('h4', { class: 'x-sub', text: 'Day by day' }));
    parts.push(
      el(
        'ul',
        { class: 'x-list x-days' },
        ...x.days.map((d) =>
          el(
            'li',
            { class: `x-day ${COUNTED[d.counted] || ''}${d.date === marked ? ' x-marked' : ''}` },
            el('span', { class: 'x-code', text: d.code || '.' }),
            dateButton(d.date),
            el('span', { class: 'x-text', text: d.text }),
          ),
        ),
      ),
    );
    panel.replaceChildren(...parts);
    panel.hidden = false;
  }

  /** Opens (or works out again) the panel for `element`. */
  async function explain(element, { quiet = false } = {}) {
    if (dto && dto.header) {
      // A header field's rows are values worked out from the sheets, not elements.
      explained = null;
      if (!quiet) actions.say('Explain This Count works on elements. A header field row comes from the sheets that have its value.', true);
      return;
    }
    explained = element;
    try {
      const x = await actions.explain(element);
      if (explained !== element) return;
      drawPanel(x);
      if (!quiet) panel.focus({ preventScroll: true });
    } catch (e) {
      if (explained !== element) return;
      if (quiet) closePanel();
      else actions.say(e, true);
    }
  }

  /** Explain This Count: the selected row, or says how to pick one. */
  function explainSelected() {
    if (selected === null || rowIndex(selected) < 0) {
      actions.say('Select a row first: click an element name in the grid, then choose Explain This Count.', true);
      return;
    }
    explain(selected);
  }

  return {
    render,
    explainSelected,
    /** The session's sort keys (main.js keeps them per sub-board). */
    setSort(keys) {
      sortKeys = Array.isArray(keys) ? keys : [];
    },
  };
}
