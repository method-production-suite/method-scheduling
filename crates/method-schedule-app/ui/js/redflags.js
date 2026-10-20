// Red flags (decisions.md, entries 33 and 36): the Red flags tab, the Red Flag Types
// dialog, and the dialog that adds or changes one element's flag.
//
// The tab lists every clash on the sub-board picked in the board bar (red_flags_view,
// src/redflags.rs), in date order: an element working, or at a travel day, fitting or
// rehearsal, on a date one of its red flags rules out. A click on a scene goes to its
// strip on the Board tab; a click on a date or an event goes to the Calendar on that date.
// Rust checks and makes every change, each one undo step. Schedule text only reaches the
// page through textContent.

import { el, isMac } from './dom.js';
import { nameToSend, pickDate, pickMods } from './logic.js';
import { setRules } from './dynstyle.js';
import { DATE_HINT, dateInput, dayDate, showDate, typedDate } from './dates.js';
import { customDialog, errorLine } from './modal.js';

/** "hard" or "soft", as the page says them. */
export const firmness = (hard) => (hard ? 'Hard' : 'Soft');

/**
 * Builds the tab in `host`. `actions`: goStrip(subBoard, strip), goDate(subBoard, date),
 * manageTypes(), goElements(category, element), types() (the view's list), weekStart()
 * (`sun` or `mon`), addFlags(input) and deleteFlags(ids) (promises that reject with a
 * sentence), exportTable(), print(pdf). Returns { render(dto) }.
 *
 * The tab has three parts (behavior note 23): adding flags to several elements and dates
 * at once (elements checked by category, dates picked on a small calendar or typed as a
 * range or an open end, then a type, note and firmness), a table of every flag with
 * check boxes, an element filter and Delete Checked, and the clashes on the sub-board.
 */
export function createRedFlags(host, actions) {
  const weekStart = () => (actions.weekStart ? actions.weekStart() : 'sun');
  function dateButton(dto, c) {
    return el('button', {
      class: 'cf-link cf-date',
      text: dayDate(c.weekday, c.date),
      title: `Show ${showDate(c.date)} on the Calendar tab`,
      attrs: { type: 'button' },
      on: { click: () => actions.goDate(dto.sub_board, c.date) },
    });
  }

  function clash(dto, c) {
    const where = [];
    if (c.day !== null && c.day !== undefined) {
      where.push(
        el(
          'li',
          { class: 'cf-booking' },
          el('span', { class: 'cf-where', text: `Shoot day ${c.day}:` }),
          ...c.sheets.map((sh) =>
            sh.strip === null || sh.strip === undefined
              ? el('span', { class: 'cf-scene', text: sh.scene })
              : el('button', {
                  class: 'cf-link',
                  text: sh.scene,
                  title: `Show ${sh.scene} on ${dto.sub_board_name}, day ${c.day}`,
                  attrs: { type: 'button' },
                  on: { click: () => actions.goStrip(dto.sub_board, sh.strip) },
                }),
          ),
        ),
      );
    }
    for (const ev of c.events) {
      where.push(
        el(
          'li',
          { class: 'cf-booking' },
          el('span', { class: 'cf-where', text: 'Event:' }),
          el('button', {
            class: 'cf-link',
            text: ev.name,
            title: `Show ${showDate(c.date)} on ${dto.sub_board_name}'s calendar`,
            attrs: { type: 'button' },
            on: { click: () => actions.goDate(dto.sub_board, c.date) },
          }),
        ),
      );
    }
    return el(
      'li',
      { class: `cf-item rf-item${c.hard ? ' rf-hard' : ''}`, attrs: { 'aria-label': `Red flag on ${showDate(c.date)}: ${c.text}` } },
      el(
        'div',
        { class: 'cf-head' },
        dateButton(dto, c),
        el('button', {
          class: 'cf-link cf-name',
          text: c.name,
          title: `Show ${c.name}'s red flags on the Elements tab`,
          attrs: { type: 'button' },
          on: { click: () => actions.goElements(c.category, c.element) },
        }),
        el('span', { class: 'cf-cat', text: c.category_name }),
        el('span', { class: 'rf-type' }, el('span', { class: `rf-swatch rft-${Number(c.red_flag_type)}`, attrs: { 'aria-hidden': 'true' } }), c.type_name),
        el('span', { class: `rf-firm${c.hard ? ' hard' : ''}`, text: firmness(c.hard) }),
        el('span', { class: 'rf-dates', text: c.dates }),
      ),
      c.note ? el('p', { class: 'rf-note', text: c.note }) : null,
      el('ul', { class: 'cf-bookings' }, ...where),
    );
  }

  // ---- What the tab keeps between draws (behavior note 23): the add panel's picks, the
  // table's filter and checked rows. Session only; nothing here is in the schedule.
  const keep = {
    category: null,
    elements: new Set(),
    dates: new Set(),
    /** The last date clicked, where a Shift-click's run starts. */
    anchor: null,
    month: null,
    form: 'picked',
    from: '',
    to: '',
    type: null,
    note: '',
    hard: true,
    filter: '',
    checked: new Set(),
    armed: false,
  };
  const addError = errorLine();
  const tableError = errorLine();
  const pad = (n) => String(n).padStart(2, '0');
  const iso = (y, m, d) => `${y}-${pad(m)}-${pad(d)}`;

  /** The sub-board's start date from the last draw (null: none). */
  let start = null;

  /**
   * The month the small calendar shows: kept, else the first picked date's, else the
   * sub-board's start month (live note 26), else today's.
   */
  function monthShown() {
    if (keep.month) return keep.month;
    const first = [...keep.dates].sort()[0] || start;
    if (first) return first.split('-').slice(0, 2).map(Number);
    const t = new Date();
    return [t.getFullYear(), t.getMonth() + 1];
  }

  /** The element picker: a category, then a check box per element. */
  function elementPicker(dto) {
    const cats = dto.categories || [];
    if (keep.category === null || !cats.some((c) => c.id === keep.category)) keep.category = cats.length ? cats[0].id : null;
    const all = new Set(cats.flatMap((c) => c.elements.map((e) => e.id)));
    for (const id of [...keep.elements]) if (!all.has(id)) keep.elements.delete(id);
    const count = el('span', { class: 'el-count', attrs: { 'aria-live': 'polite' } });
    const sayCount = () => {
      count.textContent = keep.elements.size === 1 ? '1 checked' : `${keep.elements.size} checked`;
    };
    const pick = el('select', { attrs: { id: 'rf-add-category', 'aria-label': 'Category' } }, ...cats.map((c) => el('option', { text: c.name, attrs: { value: String(c.id) } })));
    if (keep.category !== null) pick.value = String(keep.category);
    const list = el('div', { class: 'rf-pick-list', attrs: { role: 'group', 'aria-label': 'Elements to flag' } });
    const boxes = [];
    function drawList() {
      const cat = cats.find((c) => c.id === keep.category);
      boxes.length = 0;
      list.replaceChildren(
        ...(cat && cat.elements.length
          ? cat.elements.map((e) => {
              const id = `rf-add-el-${e.id}`;
              const box = el('input', { attrs: { id, type: 'checkbox', checked: keep.elements.has(e.id) } });
              box.addEventListener('change', () => {
                if (box.checked) keep.elements.add(e.id);
                else keep.elements.delete(e.id);
                sayCount();
              });
              boxes.push([box, e.id]);
              return el('label', { class: 'el-check', attrs: { for: id } }, box, el('span', { text: e.board_id ? `${e.board_id} ${e.name}` : e.name }));
            })
          : [el('p', { class: 'empty', text: 'No elements in this category.' })]),
      );
    }
    pick.addEventListener('change', () => {
      keep.category = Number(pick.value);
      drawList();
    });
    const every = (on) => {
      for (const [box, id] of boxes) {
        box.checked = on;
        if (on) keep.elements.add(id);
        else keep.elements.delete(id);
      }
      sayCount();
    };
    drawList();
    sayCount();
    return el(
      'fieldset',
      { class: 'rf-part' },
      el('legend', { text: 'Elements' }),
      el('div', { class: 'el-row' }, pick, el('button', { class: 'btn btn-mini', text: 'All', title: 'Check every element of this category', attrs: { type: 'button' }, on: { click: () => every(true) } }), el('button', { class: 'btn btn-mini', text: 'None', title: 'Uncheck every element of this category', attrs: { type: 'button' }, on: { click: () => every(false) } }), count),
      list,
    );
  }

  /** The dates: picked on a small month calendar, or a range, or an open end. */
  function datePicker() {
    const forms = [
      ['picked', 'Dates picked'],
      ['range', 'From a date to a date'],
      ['not_before', 'Not before a date'],
      ['not_after', 'Not after a date'],
    ];
    const count = el('span', { class: 'el-count', attrs: { 'aria-live': 'polite' } });
    const sayCount = () => {
      count.textContent = keep.dates.size === 1 ? '1 date picked' : `${keep.dates.size} dates picked`;
    };
    const cal = el('div', { class: 'rf-cal' });
    const fields = el('div', { class: 'el-row rf-range' });
    const from = dateInput({ class: 'w-m', id: 'rf-add-from', value: showDate(keep.from, true), 'aria-label': 'First date' });
    const to = dateInput({ class: 'w-m', id: 'rf-add-to', value: showDate(keep.to, true), 'aria-label': 'Last date' });
    for (const [input, key] of [[from, 'from'], [to, 'to']]) {
      const hold = () => (keep[key] = typedDate(input.value) || input.value);
      input.addEventListener('input', hold);
      input.addEventListener('change', hold);
    }
    const fromLabel = el('span', {});
    const toField = el('label', { class: 'bd-field beside', attrs: { for: 'rf-add-to' } }, el('span', { text: 'To' }), to);
    fields.append(el('label', { class: 'bd-field beside', attrs: { for: 'rf-add-from' } }, fromLabel, from), toField);

    function drawCal() {
      const [y, m] = monthShown();
      const monday = weekStart() === 'mon';
      const first = new Date(Date.UTC(y, m - 1, 1)).getUTCDay();
      const lead = (first - (monday ? 1 : 0) + 7) % 7;
      const days = new Date(Date.UTC(y, m, 0)).getUTCDate();
      const names = monday ? ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'] : ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];
      const cells = [...names.map((n) => el('span', { class: 'rf-cal-head', text: n, attrs: { 'aria-hidden': 'true' } }))];
      for (let i = 0; i < lead; i++) cells.push(el('span', {}));
      for (let d = 1; d <= days; d++) {
        const date = iso(y, m, d);
        const on = keep.dates.has(date);
        const b = el('button', { class: `rf-cal-day${on ? ' on' : ''}`, text: String(d), attrs: { type: 'button', 'aria-pressed': on ? 'true' : 'false', 'aria-label': showDate(date) } });
        // A click picks this date alone, Command-click (Ctrl-click off the Mac) adds or
        // removes it, Shift-click adds the run from the last date picked (note 27).
        b.addEventListener('click', (e) => {
          ({ picked: keep.dates, anchor: keep.anchor } = pickDate(keep.dates, keep.anchor, date, pickMods(e, isMac)));
          drawCal();
          sayCount();
          const again = cal.querySelector(`[data-date="${date}"]`);
          if (again) again.focus();
        });
        b.dataset.date = date;
        cells.push(b);
      }
      const step = (n) => {
        const [yy, mm] = monthShown();
        const t = new Date(Date.UTC(yy, mm - 1 + n, 1));
        keep.month = [t.getUTCFullYear(), t.getUTCMonth() + 1];
        drawCal();
      };
      const title = new Date(Date.UTC(y, m - 1, 1)).toLocaleDateString(undefined, { month: 'long', year: 'numeric', timeZone: 'UTC' });
      cal.replaceChildren(
        el(
          'div',
          { class: 'rf-cal-nav' },
          el('button', { class: 'btn btn-mini', text: '‹', attrs: { type: 'button', 'aria-label': 'Previous month' }, on: { click: () => step(-1) } }),
          el('span', { class: 'rf-cal-title', text: title }),
          el('button', { class: 'btn btn-mini', text: '›', attrs: { type: 'button', 'aria-label': 'Next month' }, on: { click: () => step(1) } }),
        ),
        el('div', { class: 'rf-cal-grid', attrs: { role: 'group', 'aria-label': `Dates in ${title}` } }, ...cells),
        el('div', { class: 'el-row' }, count, el('button', { class: 'btn btn-mini', text: 'Clear', title: 'Unpick every date', attrs: { type: 'button' }, on: { click: () => ((keep.dates = new Set()), (keep.anchor = null), drawCal(), sayCount()) } })),
      );
    }

    const radios = forms.map(([value, label]) => {
      const input = el('input', { attrs: { id: `rf-add-form-${value}`, type: 'radio', name: 'rf-add-form', value, checked: value === keep.form } });
      input.addEventListener('change', () => {
        if (input.checked) {
          keep.form = value;
          drawForm();
        }
      });
      return el('label', { class: 'el-check', attrs: { for: `rf-add-form-${value}` } }, input, el('span', { text: label }));
    });
    function drawForm() {
      cal.hidden = keep.form !== 'picked';
      fields.hidden = keep.form === 'picked';
      fromLabel.textContent = keep.form === 'range' ? 'From' : keep.form === 'not_before' ? 'Not before' : 'Not after';
      toField.hidden = keep.form !== 'range';
    }
    drawCal();
    sayCount();
    drawForm();
    return el('fieldset', { class: 'rf-part' }, el('legend', { text: 'Dates' }), el('div', { class: 'rf-forms' }, ...radios), cal, fields);
  }

  /** The flag's type, note and firmness, and Add. */
  function flagPart(dto) {
    const types = actions.types();
    if (keep.type === null || !types.some((t) => t.id === keep.type)) keep.type = types.length ? types[0].id : null;
    const type = el('select', { attrs: { id: 'rf-add-type' } }, ...types.map((t) => el('option', { text: t.name, attrs: { value: String(t.id) } })));
    if (keep.type !== null) type.value = String(keep.type);
    type.addEventListener('change', () => (keep.type = Number(type.value)));
    const note = el('input', { class: 'w-l', attrs: { type: 'text', id: 'rf-add-note', value: keep.note, autocomplete: 'off', maxlength: '2000' } });
    note.addEventListener('input', () => (keep.note = note.value));
    const hard = el('input', { attrs: { type: 'checkbox', id: 'rf-add-hard', checked: keep.hard } });
    hard.addEventListener('change', () => (keep.hard = hard.checked));
    const add = el('button', { class: 'btn btn-primary', text: 'Add Red Flags', attrs: { type: 'button', id: 'rf-add', disabled: !types.length } });
    add.addEventListener('click', async () => {
      const picked = keep.form === 'picked' ? [...keep.dates].sort() : [];
      const from = keep.form === 'picked' ? '' : document.getElementById('rf-add-from').value;
      const to = keep.form === 'range' ? document.getElementById('rf-add-to').value : '';
      try {
        await actions.addFlags({ elements: [...keep.elements], red_flag_type: Number(type.value), dates: keep.form, picked, from, to, note: note.value, hard: hard.checked });
        keep.dates.clear();
        keep.note = '';
        addError.hidden = true;
      } catch (e) {
        addError.textContent = String(e);
        addError.hidden = false;
      }
    });
    return el(
      'fieldset',
      { class: 'rf-part' },
      el('legend', { text: 'Flag' }),
      el('label', { class: 'bd-field beside', attrs: { for: 'rf-add-type' } }, el('span', { text: 'Type' }), type),
      el('label', { class: 'bd-field beside', attrs: { for: 'rf-add-note' } }, el('span', { text: 'Note' }), note),
      el('label', { class: 'el-check', attrs: { for: 'rf-add-hard' } }, hard, el('span', { text: 'Hard: can’t work then' })),
      el('p', { class: 'cf-note', text: 'One flag for each checked element and each run of picked dates.' }),
      el('div', { class: 'el-row' }, add, el('button', { class: 'btn', text: 'Red Flag Types…', attrs: { type: 'button' }, on: { click: () => actions.manageTypes() } })),
    );
  }

  /** Every red flag: a table with check boxes, an element filter and Delete Checked. */
  function flagTable(dto) {
    const rows = dto.rows || [];
    const ids = new Set(rows.map((r) => r.id));
    for (const id of [...keep.checked]) if (!ids.has(id)) keep.checked.delete(id);
    const flagged = [];
    const seen = new Set();
    for (const r of rows) {
      if (!seen.has(r.element)) {
        seen.add(r.element);
        flagged.push(r);
      }
    }
    if (keep.filter && !seen.has(Number(keep.filter))) keep.filter = '';
    const filter = el('select', { attrs: { id: 'rf-filter', 'aria-label': 'Show the flags of' } }, el('option', { text: 'Every element', attrs: { value: '' } }), ...flagged.map((r) => el('option', { text: r.board_id ? `${r.board_id} ${r.name}` : r.name, attrs: { value: String(r.element) } })));
    filter.value = keep.filter;
    const shown = rows.filter((r) => !keep.filter || r.element === Number(keep.filter));
    const del = el('button', { class: 'btn', attrs: { type: 'button', id: 'rf-delete' } });
    const all = el('input', { attrs: { type: 'checkbox', id: 'rf-check-all', 'aria-label': 'Check every flag shown' } });
    const boxes = [];
    // Checking rows changes only the Delete button and the head's box, never the table.
    function sync() {
      const n = keep.checked.size;
      del.className = keep.armed ? 'btn btn-danger' : 'btn';
      del.textContent = keep.armed ? `Click again to delete ${n}` : 'Delete Checked';
      del.disabled = !n;
      del.setAttribute('aria-label', keep.armed ? `Confirm deleting ${n === 1 ? '1 red flag' : `${n} red flags`}` : 'Delete the checked red flags');
      all.checked = shown.length > 0 && shown.every((r) => keep.checked.has(r.id));
      for (const [box, id] of boxes) box.checked = keep.checked.has(id);
    }
    del.addEventListener('click', async () => {
      if (!keep.armed) {
        keep.armed = true;
        sync();
        return;
      }
      keep.armed = false;
      try {
        await actions.deleteFlags([...keep.checked]);
        keep.checked.clear();
        tableError.hidden = true;
      } catch (e) {
        tableError.textContent = String(e);
        tableError.hidden = false;
        sync();
      }
    });
    filter.addEventListener('change', () => {
      keep.filter = filter.value;
      keep.armed = false;
      render(dto);
      const again = document.getElementById('rf-filter');
      if (again) again.focus();
    });
    all.addEventListener('change', () => {
      for (const r of shown) {
        if (all.checked) keep.checked.add(r.id);
        else keep.checked.delete(r.id);
      }
      keep.armed = false;
      sync();
    });
    const open = (d) => (d ? showDate(d) : 'Open');
    const body = shown.map((r) => {
      const box = el('input', { attrs: { type: 'checkbox', checked: keep.checked.has(r.id), 'aria-label': `Check the flag on ${r.name}, ${r.text}` } });
      box.addEventListener('change', () => {
        if (box.checked) keep.checked.add(r.id);
        else keep.checked.delete(r.id);
        keep.armed = false;
        sync();
      });
      boxes.push([box, r.id]);
      return el(
        'tr',
        { class: r.hard ? 'rf-hard' : '' },
        el('td', {}, box),
        el('td', { class: 'rf-dates', text: open(r.start) }),
        el('td', { class: 'rf-dates', text: open(r.end) }),
        el('td', { text: r.category_name }),
        el('td', {}, el('button', { class: 'cf-link cf-name', text: r.board_id ? `${r.board_id} ${r.name}` : r.name, title: `Show ${r.name}'s red flags on the Elements tab`, attrs: { type: 'button' }, on: { click: () => actions.goElements(r.category, r.element) } })),
        el('td', {}, el('span', { class: `rf-swatch rft-${Number(r.red_flag_type)}`, attrs: { 'aria-hidden': 'true' } }), r.type_name),
        el('td', { class: `rf-firm${r.hard ? ' hard' : ''}`, text: firmness(r.hard) }),
        el('td', { class: 'rf-note', text: r.note }),
      );
    });
    const table = el(
      'table',
      { class: 'summary rf-table rf-all', attrs: { 'aria-label': 'Every red flag' } },
      el('thead', {}, el('tr', {}, el('th', {}, all), ...['Start', 'End', 'Category', 'Element', 'Type', 'Hard', 'Note'].map((t) => el('th', { text: t, attrs: { scope: 'col' } })))),
      el('tbody', {}, ...body),
    );
    sync();
    const flagsCount = rows.length === 1 ? '1 red flag' : `${rows.length} red flags`;
    return el(
      'section',
      { class: 'cf-section', attrs: { 'aria-label': 'Every red flag' } },
      el('div', { class: 'el-row rf-head' }, el('h3', { text: `Every red flag (${flagsCount}${dto.flags > rows.length ? `, showing the first ${rows.length}` : ''})` }), el('span', { class: 'bd-spacer' }), el('label', { class: 'bd-field beside', attrs: { for: 'rf-filter' } }, el('span', { text: 'Element' }), filter), del),
      tableError,
      rows.length ? table : el('p', { class: 'empty', text: 'No element has a red flag yet. Check elements and dates above, then Add Red Flags.' }),
    );
  }

  function render(dto) {
    start = dto.start || null;
    const count = dto.count === 1 ? '1 clash' : `${dto.count} clashes`;
    const parts = [
      el(
        'div',
        { class: 'el-row' },
        el('h2', { text: `Red flags on ${dto.board_name}: ${dto.sub_board_name}` }),
        el('span', { class: 'bd-spacer' }),
        el('button', { class: 'btn', text: 'Export…', title: 'Every red flag as a spreadsheet (.xlsx) or CSV', attrs: { type: 'button', id: 'rf-export' }, on: { click: () => actions.exportTable() } }),
        el('button', { class: 'btn', text: 'Print…', title: 'Print the Red flags report: every flag, then the clashes', attrs: { type: 'button', id: 'rf-print' }, on: { click: () => actions.print(false) } }),
        el('button', { class: 'btn', text: 'Save as PDF…', title: 'Save the Red flags report as a PDF', attrs: { type: 'button', id: 'rf-pdf' }, on: { click: () => actions.print(true) } }),
      ),
      el('section', { class: 'rf-add', attrs: { 'aria-label': 'Add red flags' } }, el('div', { class: 'rf-parts' }, elementPicker(dto), datePicker(), flagPart(dto)), addError),
      flagTable(dto),
    ];
    const clashes = [
      el('div', { class: 'el-row rf-head' }, el('h3', { text: `Clashes on ${dto.sub_board_name}` })),
      el('p', {
        class: 'cf-note',
        text: 'A clash is an element working, or at a travel day, fitting or rehearsal, on a date one of its red flags rules out. Hard and soft flags are warnings alike; nothing is blocked.',
      }),
    ];
    if (dto.undated) clashes.push(el('p', { class: 'bd-hint', text: `${dto.sub_board_name} has no start date, so its scenes have no dates yet. Only events were checked.` }));
    if (!dto.flags) clashes.push(el('p', { class: 'empty', text: 'No element has a red flag yet.' }));
    else if (!dto.count) clashes.push(el('p', { class: 'empty', text: `No red flag clashes on ${dto.sub_board_name}.` }));
    else {
      clashes.push(el('p', { class: 'cf-count', text: dto.clashes.length < dto.count ? `${count}. Showing the first ${dto.clashes.length}.` : `${count}.` }));
      clashes.push(el('ul', { class: 'cf-list', attrs: { 'aria-label': 'Red flag clashes' } }, ...dto.clashes.map((c) => clash(dto, c))));
    }
    parts.push(el('section', { class: 'cf-section', attrs: { 'aria-label': 'Clashes' } }, ...clashes));
    host.replaceChildren(el('div', { class: 'cf-tab rf-tab' }, ...parts));
  }

  return { render };
}

/**
 * Paints each red flag type's marks in its color (decisions.md, entry 84): classes
 * `rft-<id>` on marks, strips and calendar dates, colors set through a constructed
 * stylesheet. Only checked `#rrggbb` values go in.
 */
export function paintFlagTypes(types) {
  let css = '';
  for (const t of types || []) {
    if (!t || !/^#[0-9a-fA-F]{6}$/.test(t.color || '') || !Number.isSafeInteger(t.id)) continue;
    const c = t.color;
    const k = `rft-${t.id}`;
    css += `.rf-mark.${k} { color: ${c}; }\n.row.flagged.${k} { box-shadow: inset -4px 0 0 ${c}; }\n.row.flagged.${k}.selected { box-shadow: inset 4px 0 0 var(--select-line), inset -4px 0 0 ${c}; }\n.cal-day.flagged.${k} { box-shadow: inset 0 -3px 0 ${c}; }\n.cal-day.flagged.${k}.picked { box-shadow: inset 0 0 0 2px var(--select-line), inset 0 -3px 0 ${c}; }\n.rf-swatch.${k} { background: ${c}; }\n`;
  }
  setRules('red-flag-types', css);
}

/**
 * The Red Flag Types dialog: add, rename, recolor, reorder and delete types. `ctx`:
 * types() (the view's list), missing() (standard types the schedule lacks), add(name),
 * rename(id, name), color(id, color), move(id, to), remove(id), standard(); each returns
 * a promise that rejects with a plain sentence, shown in the dialog.
 */
export function manageTypes(ctx) {
  return customDialog((close, later) => {
    const error = errorLine();
    const body = el('div', { class: 'mb-body' });
    let armed = null;

    // The redraw waits for a click in progress (decisions.md, entry 170), as in Boards
    // and Sub-boards (entry 151).
    const run = async (pending, focusKey) => {
      try {
        await pending;
        error.hidden = true;
      } catch (e) {
        error.textContent = String(e);
        error.hidden = false;
      }
      later(() => draw(focusKey));
    };

    function draw(focusKey) {
      const types = ctx.types();
      const items = types.map((t, i) => {
        const key = `t${t.id}`;
        const name = el('input', { class: 'mb-name', attrs: { type: 'text', value: t.name, 'aria-label': `Name of red flag type ${t.name}`, maxlength: '200', 'data-focus': `name-${key}` } });
        // Enter keeps the cursor in the name; leaving the field lets focus go where it went.
        let keep = false;
        // Each name goes once: Return sends it, the change on leaving the field does not
        // send it again, and a field a redraw took away sends nothing (entry 151).
        let sent = null;
        name.addEventListener('change', () => {
          const typed = name.isConnected ? nameToSend(t.name, sent, name.value) : null;
          if (typed !== null) {
            sent = typed;
            run(ctx.rename(t.id, typed), keep ? `name-${key}` : null);
          }
          keep = false;
        });
        name.addEventListener('keydown', (e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            keep = true;
            name.dispatchEvent(new Event('change'));
          }
        });
        // A standard type (decision 113) is renamed and recolored, never deleted.
        const del = t.builtin
          ? el('span', { class: 'mb-shown rf-std', text: 'Standard', title: t.event_code ? `A standard type: renamed and recolored, never deleted. Its word is the ${t.name} event kind's; an event of that kind shows ${t.event_code} on the day out of days, a flag never counts.` : 'A standard type: renamed and recolored, never deleted. Its word is an event kind\u2019s; the flag itself never counts.' })
          : el('button', {
          class: armed === key ? 'btn btn-danger' : 'btn',
          text: armed === key ? 'Click again to delete' : 'Delete',
          attrs: { type: 'button', 'aria-label': armed === key ? `Confirm deleting ${t.name}` : `Delete ${t.name}`, 'data-focus': `del-${key}` },
          on: {
            click: () => {
              if (armed === key) {
                armed = null;
                run(ctx.remove(t.id), 'rf-new-type');
              } else {
                armed = key;
                draw(`del-${key}`);
              }
            },
          },
        });
        // Its color (decisions.md, entry 84): the marks of its flags are drawn in it.
        const color = el('input', { attrs: { type: 'color', value: /^#[0-9a-fA-F]{6}$/.test(t.color || '') ? t.color : '#cc3333', 'aria-label': `Color of ${t.name}`, title: t.color ? `Its marks are drawn in ${t.color}` : 'No color: its marks are red', 'data-focus': `color-${key}` } });
        color.addEventListener('change', () => ctx.color && run(ctx.color(t.id, color.value), `color-${key}`));
        const plain = el('button', { class: 'btn btn-mini', text: 'Red', title: 'No color of its own: its marks are the usual red', attrs: { type: 'button', disabled: !t.color, 'aria-label': `No color for ${t.name}` }, on: { click: () => ctx.color && run(ctx.color(t.id, ''), `color-${key}`) } });
        return el(
          'li',
          { class: 'mb-item' },
          name,
          color,
          plain,
          el('span', { class: 'mb-shown', text: t.uses === 1 ? '1 flag' : `${t.uses} flags` }),
          el(
            'span',
            { class: 'mb-actions' },
            el('button', { class: 'btn', text: 'Up', attrs: { type: 'button', disabled: i === 0, 'aria-label': `Move ${t.name} up`, 'data-focus': `up-${key}` }, on: { click: () => run(ctx.move(t.id, i - 1), `up-${key}`) } }),
            el('button', { class: 'btn', text: 'Down', attrs: { type: 'button', disabled: i === types.length - 1, 'aria-label': `Move ${t.name} down`, 'data-focus': `down-${key}` }, on: { click: () => run(ctx.move(t.id, i + 1), `down-${key}`) } }),
            del,
          ),
        );
      });
      const fresh = el('input', { attrs: { type: 'text', id: 'rf-new-type', placeholder: 'New type, like Unavailable', 'aria-label': 'New red flag type', maxlength: '200' } });
      const add = () => {
        if (fresh.value.trim()) run(ctx.add(fresh.value), 'rf-new-type');
      };
      fresh.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          add();
        }
      });
      // An older schedule may lack the standard types: one step adds them (decision 113).
      const missing = ctx.missing ? ctx.missing() : 0;
      const standard = missing && ctx.standard
        ? el('div', { class: 'mb-new' }, el('button', { class: 'btn', text: 'Add the Standard Types', attrs: { type: 'button', id: 'rf-standard' }, on: { click: () => run(ctx.standard(), 'rf-new-type') } }), el('span', { class: 'mb-shown', text: missing === 1 ? '1 standard type is missing: travel, fitting, rehearsal and the other event kinds.' : `${missing} standard types are missing: travel, fitting, rehearsal and the other event kinds.` }))
        : null;
      body.replaceChildren(
        types.length ? el('ul', { class: 'mb-list', attrs: { 'aria-label': 'Red flag types' } }, ...items) : el('p', { class: 'empty', text: 'No red flag types yet.' }),
        el('div', { class: 'mb-new' }, fresh, el('button', { class: 'btn', text: 'Add Type', attrs: { type: 'button' }, on: { click: add } })),
        standard,
      );
      const target = focusKey === 'rf-new-type' ? fresh : focusKey ? body.querySelector(`[data-focus="${focusKey}"]`) : null;
      if (target && !target.disabled) target.focus();
    }

    draw(null);
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'rf-types-title' } },
      el('h2', { class: 'modal-title', text: 'Red Flag Types', attrs: { id: 'rf-types-title' } }),
      el('p', { class: 'modal-message', text: 'What you call each kind of limit, like Unavailable or Second job. The standard types share their words with the event kinds; they and any type that flags still use can be renamed and recolored but not deleted.' }),
      error,
      body,
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn btn-primary', text: 'Done', attrs: { type: 'button' }, on: { click: () => close(true) } })),
    );
  });
}

const DATE_FORMS = [
  ['range', 'From a date to a date'],
  ['not_before', 'Not before a date'],
  ['not_after', 'Not after a date'],
];

/**
 * The dialog that adds a red flag to an element (`flag` null) or changes one. `types` is
 * the view's list; `submit(input)` sends { id, red_flag_type, dates, from, to, note, hard }
 * and rejects with a plain sentence, shown in the dialog; `manage()` opens the types
 * dialog when there are none. Resolves true once saved.
 */
export function flagDialog({ element, flag, types, submit, manage, from: startFrom = '' }) {
  return customDialog((close) => {
    const error = errorLine();
    if (!types.length) {
      return el(
        'div',
        { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'rf-flag-title' } },
        el('h2', { class: 'modal-title', text: 'Add a red flag type first', attrs: { id: 'rf-flag-title' } }),
        el('p', { class: 'modal-message', text: 'Every red flag has a type, like Unavailable or Second job. Add one, then add the flag.' }),
        el(
          'div',
          { class: 'modal-actions' },
          el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
          el('button', { class: 'btn btn-primary', text: 'Red Flag Types…', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => (close(null), manage()) } }),
        ),
      );
    }
    const type = el('select', { attrs: { id: 'rf-type', 'data-autofocus': true } }, ...types.map((t) => el('option', { text: t.name, attrs: { value: String(t.id) } })));
    if (flag) type.value = String(flag.red_flag_type);
    let form = flag ? flag.dates : 'range';
    // A new flag can start from a date (the Calendar's Actions menu).
    const from = dateInput({ class: 'w-m', id: 'rf-from', value: flag ? flag.from : startFrom });
    const to = dateInput({ class: 'w-m', id: 'rf-to', value: flag ? flag.to : startFrom });
    const fromLabel = el('span', {});
    const toField = el('label', { class: 'bd-field', attrs: { for: 'rf-to' } }, el('span', { text: 'To' }), to);
    const forms = DATE_FORMS.map(([value, label]) => {
      const input = el('input', { attrs: { id: `rf-form-${value}`, type: 'radio', name: 'rf-form', value, checked: value === form } });
      input.addEventListener('change', () => {
        if (input.checked) {
          form = value;
          drawForm();
        }
      });
      return el('label', { class: 'el-check', attrs: { for: `rf-form-${value}` } }, input, el('span', { text: label }));
    });
    function drawForm() {
      fromLabel.textContent = form === 'range' ? 'From' : form === 'not_before' ? 'Not before' : 'Not after';
      toField.hidden = form !== 'range';
    }
    drawForm();
    const note = el('input', { class: 'w-l', attrs: { type: 'text', id: 'rf-note', value: flag ? flag.note : '', autocomplete: 'off' } });
    const hard = el('input', { attrs: { type: 'checkbox', id: 'rf-hard', checked: flag ? flag.hard : true } });
    const save = async () => {
      if (form === 'range' && from.value.trim() && !to.value.trim()) to.value = from.value.trim();
      try {
        await submit({
          id: flag ? flag.id : null,
          red_flag_type: Number(type.value),
          dates: form,
          from: from.value,
          to: form === 'range' ? to.value : '',
          note: note.value,
          hard: hard.checked,
        });
        close(true);
      } catch (e) {
        error.textContent = String(e);
        error.hidden = false;
      }
    };
    for (const input of [from, to, note]) {
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          save();
        }
      });
    }
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'rf-flag-title' } },
      el('h2', { class: 'modal-title', text: flag ? `Change a red flag on ${element}` : `Add a red flag to ${element}`, attrs: { id: 'rf-flag-title' } }),
      el('p', { class: 'modal-message', text: `The dates this element can’t work, or would rather not. Type dates as ${DATE_HINT}. A flag is a warning: the board, calendar and day out of days mark the days it rules out.` }),
      error,
      el('div', { class: 'el-row' }, el('label', { class: 'bd-field', attrs: { for: 'rf-type' } }, el('span', { text: 'Type' }), type)),
      el('fieldset', { class: 'cal-kinds' }, el('legend', { text: 'Dates' }), ...forms),
      el('div', { class: 'el-row' }, el('label', { class: 'bd-field', attrs: { for: 'rf-from' } }, fromLabel, from), toField),
      el('div', { class: 'el-row' }, el('label', { class: 'bd-field', attrs: { for: 'rf-note' } }, el('span', { text: 'Note' }), note)),
      el('div', { class: 'el-row' }, el('label', { class: 'el-check', attrs: { for: 'rf-hard' } }, hard, el('span', { text: 'Hard: can’t work then (otherwise soft: would rather not)' }))),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: flag ? 'Save' : 'Add Red Flag', attrs: { type: 'button', id: 'rf-save' }, on: { click: save } }),
      ),
    );
  });
}
