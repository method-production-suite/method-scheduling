// The Breakdown tab: one breakdown sheet's header fields and its elements by category, with
// quick add. Every category is a small box in a grid (its count, its elements, a field
// that adds to it), empty ones too unless Hide empty is on (decision 53). Rust sends what to draw (get_sheet, src/view.rs) and reads every field
// strictly (src/sheet.rs); this file lays the sheet out and sends what was typed. Every
// field commits on change (leaving it, or Enter in a one-line field) as one undo step.
//
// The form is built once per sheet and then only refreshed, so a redraw after a commit
// never moves focus or throws away what is being typed in another field.

import { el, showThumb } from './dom.js';
import { customDialog, keepRoom } from './modal.js';
import { boxHidden, categoryChoices, dragStarted, gridDesign, gridMove, headerMatches, quickAddPlan, semicolonCreate, sheetRows } from './logic.js';
import { withShortcut } from './shortcuts.js';

/**
 * The sheet's fields, labels beside the fields, as the reference app lays them out
 * (decision 53): [field, label, class, hint, accessible name, kind]. A field whose label
 * is shortened on screen keeps its full name for assistive technology. They are laid out
 * in the schedule's header field order (decisions.md, entry 81), in three rows of
 * ROW_SIZES fields; the default order gives the reference app's three rows with the
 * sheet number first.
 */
const FIELD_DEFS = [
  ['number', 'Sheet', 'w-xs', '', 'Sheet number'],
  ['scenes', 'Scenes', 'w-s', ''],
  ['int_ext', 'I/E', 'w-xs', ''],
  ['set', 'Set', 'bd-fill', ''],
  ['time_of_day', 'D/N', 'w-m', '', 'Time of day'],
  ['shoot_time', 'Shoot time', 'w-xs', 'h:mm'],
  ['synopsis', 'Synopsis', 'bd-fill', '', null, 'area'],
  ['location', 'Location', 'w-l', ''],
  ['pages', 'Pages', '', '', null, 'pages'],
  ['script_pages', 'Script pages', 'w-xs', ''],
  ['script_day', 'Script day', 'w-xs', ''],
  ['screen_time', 'Screen time', 'w-xs', 'm:ss'],
  ['unit', 'Unit', 'w-xs', ''],
  ['sequence', 'Sequence', 'w-xs', ''],
  ['episode', 'Episode', 'w-xs', ''],
  ['comments', 'Comments', 'bd-fill', '', null, 'area'],
  ['sunrise', 'Sunrise', 'w-xs', 'hh:mm'],
  ['sunset', 'Sunset', 'w-xs', 'hh:mm'],
];
const DEF = new Map(FIELD_DEFS.map((f) => [f[0], f]));
/** Every text field the sheet fills from the view (the page fields are filled apart). */
const FIELDS = FIELD_DEFS.filter((f) => f[5] !== 'pages').map((f) => f[0]);

/** The fields' names for the Breakdown Designer dialog: [key, name]. */
export const SHEET_FIELD_NAMES = FIELD_DEFS.map((f) => [f[0], f[4] || f[1]]);

/** The order the fields are laid out in: the view's, else the default. */
const orderOf = (dto) => (dto && Array.isArray(dto.field_order) && dto.field_order.length ? dto.field_order.filter((k) => DEF.has(k)) : FIELD_DEFS.map((f) => f[0]));

/** Where the decimal time display is remembered in this browser (decisions.md, entry 81). */
const DECIMAL_KEY = 'method-schedule.breakdown.decimal-time';

function loadDecimal() {
  try {
    return window.localStorage.getItem(DECIMAL_KEY) === '1';
  } catch {
    return false;
  }
}

function saveDecimal(on) {
  try {
    window.localStorage.setItem(DECIMAL_KEY, on ? '1' : '0');
  } catch {
    // Not remembered; the switch still works until the page reloads.
  }
}

/** The stepping orders offered beside Previous and Next (decisions.md, entry 81). */
const ORDERS = [
  ['board', 'Board order'],
  ['number', 'Sheet number'],
  ['scene', 'Scene number'],
];

/** Quick add's semicolon form, `Props;Lantern` (logic.js). */
export { semicolonCreate };

/** The widths a header field can be given (model::FieldWidth), and the class each uses. */
const FIELD_WIDTHS = [
  ['', 'Usual'],
  ['narrow', 'Narrow'],
  ['medium', 'Medium'],
  ['wide', 'Wide'],
];
const WIDTH_CLASS = { narrow: 'w-xs', medium: 'w-m', wide: 'w-l' };

/** A field's definition with the width set for it (none: its usual one). */
function sized(def, width) {
  if (!width || !WIDTH_CLASS[width] || def[5] === 'pages') return def;
  const out = [...def];
  out[2] = WIDTH_CLASS[width];
  return out;
}

/**
 * Breakdown > Breakdown Designer (decisions.md, entries 81, 101 and 186): every header
 * field with Show, a width (usual, narrow, medium, wide), Up and Down, and Default; and
 * the shown fields in their rows as the tab lays them out, each dragged to another place
 * or row (or to New row). Resolves with { keys, hidden, widths, rows } (widths: key to
 * width, only those set; rows: each row's length, empty for six, five and the rest), or
 * null.
 */
export function fieldOrderDialog(order, hiddenNow = [], widthsNow = [], rowsNow = []) {
  return customDialog((close) => {
    let keys = order.filter((k) => DEF.has(k));
    for (const f of FIELD_DEFS) if (!keys.includes(f[0])) keys.push(f[0]);
    let hidden = new Set(hiddenNow);
    let widths = new Map(widthsNow);
    let lengths = [...rowsNow];
    const list = el('ol', { class: 'fo-list', attrs: { 'aria-label': 'Header fields in order' } });
    const name = (k) => SHEET_FIELD_NAMES.find((x) => x[0] === k)[1];
    // The grid: the shown fields in rows, as the Breakdown tab lays them out.
    const grid = el('div', { class: 'fo-grid', attrs: { 'aria-label': 'Header fields in rows' } });
    const shownRows = () => sheetRows(keys.filter((k) => !hidden.has(k)), lengths);
    let drag = null;
    function drawGrid() {
      const rows = shownRows();
      grid.replaceChildren(
        ...rows.map((r, i) =>
          el(
            'div',
            { class: 'fo-grow', attrs: { 'data-row': String(i) } },
            el('span', { class: 'fo-rown', text: String(i + 1) }),
            ...r.map((k) => el('span', { class: 'fo-chip', text: name(k), attrs: { 'data-key': k, title: `Drag ${name(k)} to another place or row` } })),
          ),
        ),
        el('div', { class: 'fo-grow fo-newrow', attrs: { 'data-row': String(rows.length) } }, el('span', { class: 'fo-rown', text: '+' }), el('span', { class: 'bd-hint', text: 'New row' })),
      );
    }
    /** Where a drop at (x, y) goes: { row, index }, or null off the rows. */
    function dropAt(x, y) {
      const hit = document.elementFromPoint(x, y);
      const row = hit && hit.closest ? hit.closest('.fo-grow') : null;
      if (!row || !grid.contains(row)) return null;
      const r = Number(row.dataset.row);
      const chip = hit.closest('.fo-chip');
      const chips = [...row.querySelectorAll('.fo-chip')];
      if (!chip) return { row: r, index: chips.length };
      const box = chip.getBoundingClientRect();
      return { row: r, index: chips.indexOf(chip) + (x > box.left + box.width / 2 ? 1 : 0) };
    }
    grid.addEventListener('pointerdown', (e) => {
      const chip = e.target.closest ? e.target.closest('.fo-chip') : null;
      if (!chip || e.button !== 0) return;
      drag = { key: chip.dataset.key, x: e.clientX, y: e.clientY, pointer: e.pointerId, active: false, chip };
    });
    grid.addEventListener('pointermove', (e) => {
      if (!drag || e.pointerId !== drag.pointer) return;
      if (!drag.active) {
        if (!dragStarted(drag.x, drag.y, e.clientX, e.clientY)) return;
        try {
          grid.setPointerCapture(e.pointerId);
        } catch {
          // The pointer may be gone already; the drag still works over the grid.
        }
        drag.active = true;
        drag.chip.classList.add('dragging');
        document.addEventListener('keydown', onEscape, true);
      }
      e.preventDefault();
      for (const r of grid.querySelectorAll('.fo-grow')) r.classList.remove('aim');
      const at = dropAt(e.clientX, e.clientY);
      if (at) grid.querySelector(`.fo-grow[data-row="${at.row}"]`).classList.add('aim');
    });
    // Escape while dragging puts the field back and leaves the dialog open.
    const onEscape = (e) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      endDrag(false);
    };
    function endDrag(drop, x, y) {
      document.removeEventListener('keydown', onEscape, true);
      const d = drag;
      drag = null;
      if (!d || !d.active) return;
      const at = drop ? dropAt(x, y) : null;
      if (at) {
        const rows = gridMove(shownRows(), d.key, at.row, at.index);
        const design = gridDesign(rows, keys, hidden);
        keys = design.keys;
        lengths = design.rows;
      }
      draw();
    }
    grid.addEventListener('pointerup', (e) => {
      if (drag && e.pointerId === drag.pointer) endDrag(true, e.clientX, e.clientY);
    });
    grid.addEventListener('pointercancel', () => endDrag(false));
    function draw(focusKey = null, dir = '') {
      drawGrid();
      list.replaceChildren(
        ...keys.map((k, i) => {
          const up = el('button', { class: 'btn btn-mini', text: 'Up', attrs: { type: 'button', disabled: i === 0, 'aria-label': `Move ${name(k)} up` }, on: { click: () => move(i, -1) } });
          const down = el('button', { class: 'btn btn-mini', text: 'Down', attrs: { type: 'button', disabled: i === keys.length - 1, 'aria-label': `Move ${name(k)} down` }, on: { click: () => move(i, 1) } });
          up.dataset.key = k;
          down.dataset.key = k;
          const show = el('input', { attrs: { id: `fo-show-${k}`, type: 'checkbox', checked: !hidden.has(k) } });
          show.addEventListener('change', () => {
            if (show.checked) hidden.delete(k);
            else hidden.add(k);
            drawGrid();
          });
          const width = el('select', { attrs: { 'aria-label': `Width of ${name(k)}`, disabled: DEF.get(k)[5] === 'pages' } }, ...FIELD_WIDTHS.map(([v, t]) => el('option', { text: t, attrs: { value: v } })));
          width.value = widths.get(k) || '';
          width.addEventListener('change', () => (width.value ? widths.set(k, width.value) : widths.delete(k)));
          return el('li', { class: 'fo-item' }, el('label', { class: 'el-check fo-name', attrs: { for: `fo-show-${k}` } }, show, el('span', { text: name(k) })), width, up, down);
        }),
      );
      if (focusKey) {
        const b = list.querySelector(`button[data-key="${focusKey}"]${dir === 'up' ? '' : ':last-of-type'}`);
        if (b && !b.disabled) b.focus();
      }
    }
    function move(i, by) {
      const j = i + by;
      if (j < 0 || j >= keys.length) return;
      const k = keys[i];
      [keys[i], keys[j]] = [keys[j], keys[i]];
      draw(k, by < 0 ? 'up' : 'down');
    }
    draw();
    const done = () => close({ keys, hidden: keys.filter((k) => hidden.has(k)), widths: Object.fromEntries([...widths].filter(([, w]) => w)), rows: lengths });
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'fo-title' } },
      el('h2', { class: 'modal-title', text: 'Breakdown Designer', attrs: { id: 'fo-title' } }),
      el('p', { class: 'modal-message', text: 'The sheet\u2019s header fields in order, each shown or hidden, at its usual width or one set; below, the shown ones in their rows (six, five and the rest until a field is dragged to another row or to New row). A hidden field keeps its values. Kept in the schedule.' }),
      list,
      grid,
      el(
        'div',
        { class: 'modal-actions' },
        el('button', {
          class: 'btn',
          text: 'Default',
          attrs: { type: 'button' },
          on: {
            click: () => {
              keys = FIELD_DEFS.map((f) => f[0]);
              hidden = new Set();
              widths = new Map();
              lengths = [];
              draw();
            },
          },
        }),
        el('span', { class: 'bd-spacer' }),
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Use These', attrs: { type: 'button', 'data-autofocus': true }, on: { click: done } }),
      ),
    );
  });
}
/**
 * The one-value header fields (`reports::HeaderField`): typing in one offers the values
 * other sheets use, and Rust gives a committed value the existing spelling when it
 * matches ignoring case (decisions.md, entry 155). The list is the page's own, as quick
 * add's is, rather than a datalist.
 */
const HEADER_FIELDS = new Set(['set', 'location', 'int_ext', 'time_of_day', 'unit', 'sequence', 'episode', 'script_day']);

/** Where Hide empty is remembered in this browser (a convenience, not schedule data). */
const HIDE_EMPTY_KEY = 'method-schedule.breakdown.hide-empty';

function loadHideEmpty() {
  try {
    return window.localStorage.getItem(HIDE_EMPTY_KEY) === '1';
  } catch {
    return false;
  }
}

function saveHideEmpty(on) {
  try {
    window.localStorage.setItem(HIDE_EMPTY_KEY, on ? '1' : '0');
  } catch {
    // Storage may be off; the toggle still works until the page reloads.
  }
}

/** The left edge colors a category box cycles through (styles.css, `.bd-tone-N`). */
const TONES = 8;

const MATCHES_SHOWN = 8;

/** The category new elements go in, kept across sheets. */
let lastCategory = '';

/**
 * Asks which category a new element `name` goes in (BRK-13, decisions.md, entry 156): a
 * list with `current` (the New in category) chosen, so Return keeps it. Up and Down move,
 * Return or a double-click accepts, Escape cancels. Resolves with the name, or null.
 */
function askCategory(name, names, current) {
  const { choices, at } = categoryChoices(names, current);
  return customDialog((close) => {
    const pick = el(
      'select',
      { class: 'bd-pick-category', attrs: { id: 'bd-pick-category', size: String(Math.max(2, Math.min(choices.length, 10))), 'aria-label': 'Category', 'data-autofocus': true } },
      ...choices.map((c) => el('option', { text: c, attrs: { value: c } })),
    );
    pick.selectedIndex = at;
    const go = () => close(pick.value || null);
    pick.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        go();
      }
    });
    pick.addEventListener('dblclick', go);
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'New Element', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: `No element is called ${name}. Which category does it go in?` }),
      pick,
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Add Element', attrs: { type: 'button' }, on: { click: go } }),
      ),
    );
  });
}

/**
 * Builds the Breakdown tab's editor in `host`. `actions`: { field(sheet, field, value),
 * pages(sheet, whole, eighths), tag(sheet, element), create(sheet, category, name),
 * untag(sheet, element), quantity(sheet, element, count, daily), newEvent(dto),
 * step(sheetId), newSheet(sheet), duplicateSheet(dto), deleteSheet(dto) }. Each returns a
 * promise that rejects with a plain message. Returns { render(dto), showError(text, field),
 * focusFirst(), current(), mark(field, focus) }.
 */
export function createBreakdown(host, actions) {
  let dto = null;
  let built = null; // the sheet id the form was built for
  const inputs = {}; // field -> input or textarea

  const nav = {
    prev: el('button', { class: 'btn', text: 'Previous', title: withShortcut('Previous sheet', 'sheet.previous'), attrs: { type: 'button' }, on: { click: () => dto && dto.prev !== null && actions.step(dto.prev) } }),
    next: el('button', { class: 'btn', text: 'Next', title: withShortcut('Next sheet', 'sheet.next'), attrs: { type: 'button' }, on: { click: () => dto && dto.next !== null && actions.step(dto.next) } }),
    // Step by board order, sheet number or scene number (decisions.md, entry 81).
    order: el('select', { attrs: { id: 'bd-order', 'aria-label': 'Previous and Next step by' } }, ...ORDERS.map(([k, label]) => el('option', { text: label, attrs: { value: k } }))),
    // Shoot time as decimal hours (1.50) rather than h:mm (decisions.md, entry 81).
    decimal: el('input', { attrs: { type: 'checkbox', id: 'bd-decimal' } }),
    where: el('span', { class: 'bd-where' }),
    // Where the strip is (day and date): a link to it on the Board tab.
    place: el('button', {
      class: 'cf-link bd-place',
      attrs: { type: 'button', title: 'Show this strip on the Board' },
      on: { click: () => dto && dto.strip !== null && dto.strip !== undefined && actions.goStrip(dto.strip_sub_board ?? null, dto.strip) },
    }),
    // The newest script draft that changed it (entry 233), beside where it is.
    revised: el('span', { class: 'bd-where bd-revised' }),
    add: el('button', { class: 'btn', text: 'New Sheet', attrs: { type: 'button' }, on: { click: () => actions.newSheet(dto ? dto.id : null) } }),
    dup: el('button', { class: 'btn', text: 'Duplicate Sheet', attrs: { type: 'button', id: 'bd-duplicate' }, on: { click: () => dto && actions.duplicateSheet(dto) } }),
    del: el('button', { class: 'btn', text: 'Delete Sheet', attrs: { type: 'button' }, on: { click: () => dto && actions.deleteSheet(dto) } }),
    event: el('button', { class: 'btn', text: 'New Event…', title: 'A new event with this sheet’s elements', attrs: { type: 'button', id: 'bd-new-event' }, on: { click: () => dto && actions.newEvent(dto) } }),
  };
  nav.order.addEventListener('change', () => actions.order && actions.order(nav.order.value));
  let decimal = loadDecimal();
  nav.decimal.checked = decimal;
  nav.decimal.addEventListener('change', () => {
    decimal = nav.decimal.checked;
    saveDecimal(decimal);
    if (!dto) return;
    inputs.shoot_time.placeholder = decimal ? 'h.hh' : 'h:mm';
    if (document.activeElement !== inputs.shoot_time && inputs.shoot_time.getAttribute('aria-invalid') !== 'true') inputs.shoot_time.value = shown('shoot_time');
  });
  /** What a field shows: the view's value; a shoot time as decimal hours when that display is on. */
  const shown = (field) => (field === 'shoot_time' && decimal ? dto.shoot_time_decimal || '' : dto[field]);
  const error = keepRoom(el('p', { class: 'bd-error', attrs: { role: 'alert', id: 'bd-error' } }));
  // Red flag clashes (decisions.md, entry 84) and conflicts (entry 83) on the date the sheet shoots.
  const marks = el('div', { class: 'bd-marks', attrs: { role: 'status' } });
  function drawMarks() {
    const flags = dto.red_flags || [];
    const clash = dto.conflicts || [];
    const kind = dto.red_flag_types && dto.red_flag_types.length ? ` rft-${Number(dto.red_flag_types[0])}` : '';
    marks.replaceChildren(
      ...flags.map((t) => el('span', { title: `Red flag: ${t}` }, el('span', { class: `rf-mark${kind}`, text: '⚑', attrs: { 'aria-hidden': 'true' } }), `Red flag: ${t}`)),
      ...clash.map((t) => el('span', { title: `Conflict: ${t}` }, el('span', { class: 'cf-mark', text: '⚠', attrs: { 'aria-hidden': 'true' } }), `Conflict: ${t}`)),
    );
    marks.hidden = !flags.length && !clash.length;
  }
  error.hidden = true;
  const empty = el('p', { class: 'empty', text: 'No breakdown sheets yet. Import a script, or add one with New Sheet.' });
  const lists = { categories: el('datalist', { attrs: { id: 'bd-categories' } }) };

  /** The field the error is about ('quick' for quick add and removing elements). */
  let errorField = null;

  /** The inputs a field's error marks. */
  const targets = (field) => (field === 'pages' ? [inputs.pages, inputs.eighths] : field && inputs[field] ? [inputs[field]] : []);

  /** Clears the error when it is about `field` (or about nothing), or always with `all`. */
  function clearError(field, all = false) {
    if (!all && error.hidden) return;
    if (!all && errorField !== null && errorField !== field) return;
    error.hidden = true;
    error.textContent = '';
    errorField = null;
    for (const i of Object.values(inputs)) i.removeAttribute('aria-invalid');
  }

  function showError(text, field) {
    clearError(null, true);
    error.textContent = text;
    error.hidden = false;
    errorField = field || null;
    for (const i of targets(field)) i.setAttribute('aria-invalid', 'true');
  }

  // Sends a field when it changes. A refused value stays in its field, marked, with the
  // error, until that field is sent again or another sheet is shown.
  async function commit(field, run) {
    try {
      await run();
      clearError(field);
    } catch (e) {
      showError(String(e), field);
    }
  }

  /** Sends a field, unless it holds what the sheet already has and isn't marked wrong. */
  function send(field, input) {
    if (input.value === shown(field) && input.getAttribute('aria-invalid') !== 'true') return;
    const typed = input.value.trim().toLowerCase();
    commit(field, () => actions.field(dto.id, field, input.value)).then(() => {
      // A header value taken in another sheet's spelling shows it at once, focused or not.
      const now = shown(field) || '';
      if (HEADER_FIELDS.has(field) && input.getAttribute('aria-invalid') !== 'true' && now.toLowerCase() === typed) input.value = now;
    });
  }

  /**
   * The values other sheets use, offered under header field `field` as it is typed in:
   * Up and Down pick, Enter or a click takes one, Escape closes the list. Returns the list.
   */
  function suggest(field, input) {
    const list = el('ul', { class: 'bd-matches', attrs: { id: `bd-${field}-values`, role: 'listbox', 'aria-label': 'Values on other sheets' } });
    let matches = [];
    let active = -1;
    input.setAttribute('role', 'combobox');
    input.setAttribute('aria-autocomplete', 'list');
    input.setAttribute('aria-controls', list.id);
    const draw = () => {
      list.replaceChildren(
        ...matches.map((v, i) =>
          el('li', { class: i === active ? 'active' : '', attrs: { role: 'option', id: `${list.id}-${i}`, 'aria-selected': i === active ? 'true' : 'false' }, on: { mousedown: (ev) => ev.preventDefault(), click: () => take(v) } }, el('span', { text: v })),
        ),
      );
      list.hidden = !matches.length;
      input.setAttribute('aria-expanded', matches.length ? 'true' : 'false');
      if (active >= 0) input.setAttribute('aria-activedescendant', `${list.id}-${active}`);
      else input.removeAttribute('aria-activedescendant');
    };
    const close = () => {
      matches = [];
      active = -1;
      draw();
    };
    const take = (v) => {
      input.value = v;
      close();
      send(field, input);
    };
    input.addEventListener('input', () => {
      matches = headerMatches(dto && dto.header_values ? dto.header_values[field] : [], input.value);
      active = -1;
      draw();
    });
    input.addEventListener('keydown', (e) => {
      if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && matches.length) {
        e.preventDefault();
        const n = matches.length;
        active = e.key === 'ArrowDown' ? (active + 1) % n : (active - 1 + n) % n;
        draw();
      } else if (e.key === 'Enter' && active >= 0) {
        e.preventDefault();
        e.stopImmediatePropagation();
        take(matches[active]);
      } else if (e.key === 'Enter') {
        close();
      } else if (e.key === 'Escape' && matches.length) {
        e.preventDefault();
        e.stopPropagation();
        close();
      }
    });
    input.addEventListener('blur', close);
    list.hidden = true;
    return list;
  }

  function textField([field, label, cls, hint, name]) {
    const id = `bd-${field}`;
    const input = el('input', {
      class: cls,
      attrs: { id, type: 'text', autocomplete: 'off', spellcheck: 'false', placeholder: field === 'shoot_time' && decimal ? 'h.hh' : hint, 'aria-label': name || null, 'aria-describedby': 'bd-error' },
    });
    const values = HEADER_FIELDS.has(field) ? suggest(field, input) : null;
    input.addEventListener('change', () => send(field, input));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') input.dispatchEvent(new Event('change'));
    });
    inputs[field] = input;
    return el('label', { class: `bd-field${cls === 'bd-fill' ? ' bd-grow' : ''}`, title: name || null, attrs: { for: id } }, el('span', { text: label }), values ? el('div', { class: 'bd-combo bd-values' }, input, values) : input);
  }

  function pagesField() {
    const whole = el('input', { class: 'w-2', attrs: { id: 'bd-pages', type: 'text', inputmode: 'numeric', autocomplete: 'off', 'aria-label': 'Pages', 'aria-describedby': 'bd-error' } });
    const eighths = el('input', { class: 'w-2', attrs: { id: 'bd-eighths', type: 'text', inputmode: 'numeric', autocomplete: 'off', 'aria-label': 'Eighths of a page', 'aria-describedby': 'bd-error' } });
    inputs.pages = whole;
    inputs.eighths = eighths;
    // Both halves go together as one edit, once focus leaves the pair (or on Enter).
    const send = () => {
      const same = whole.value === dto.pages_whole && eighths.value === dto.pages_eighths;
      if (same && whole.getAttribute('aria-invalid') !== 'true') return;
      commit('pages', () => actions.pages(dto.id, whole.value, eighths.value));
    };
    for (const i of [whole, eighths]) {
      i.addEventListener('focusout', (e) => {
        if (e.relatedTarget === whole || e.relatedTarget === eighths) return;
        send();
      });
      i.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') send();
      });
    }
    return el('div', { class: 'bd-field' }, el('label', { attrs: { for: 'bd-pages' }, text: 'Pages' }), el('span', { class: 'bd-pages' }, whole, el('span', { text: '+' }), eighths, el('span', { text: '/8' })));
  }

  function textArea([field, label, cls]) {
    const id = `bd-${field}`;
    const fixed = cls && cls !== 'bd-fill';
    const area = el('textarea', { class: fixed ? cls : '', attrs: { id, rows: 1, spellcheck: 'true', 'aria-describedby': 'bd-error' } });
    area.addEventListener('change', () => send(field, area));
    inputs[field] = area;
    return el('label', { class: `bd-field${fixed ? '' : ' bd-grow'}`, attrs: { for: id } }, el('span', { text: label }), area);
  }

  function fieldOf(f) {
    if (f[5] === 'pages') return pagesField();
    if (f[5] === 'area') return textArea(f);
    return textField(f);
  }

  /** The elements a typed text matches, not on the sheet yet: names starting with it first. */
  function matchesFor(text, category = null) {
    const t = text.trim().toLowerCase();
    if (!t || !dto) return [];
    const on = new Set(dto.groups.flatMap((g) => g.elements.map((e) => e.id)));
    const starts = [];
    const contains = [];
    for (const e of dto.elements) {
      if (on.has(e.id)) continue;
      if (category !== null && e.category !== category) continue;
      const n = e.name.toLowerCase();
      if (n.startsWith(t)) starts.push(e);
      else if (n.includes(t)) contains.push(e);
    }
    return starts.concat(contains).slice(0, MATCHES_SHOWN);
  }

  /**
   * A field that adds elements: type, pick a match with the arrow keys and Enter (or a
   * click), or press Enter to create one. `input` and `list` are its nodes; `category()`
   * is the category it matches in (null: every category); `create(name)` makes a new one.
   * Quick add and every category box's add field work this way.
   */
  function adder(input, list, category, create) {
    const c = { input, list, matches: [], active: -1 };
    const optionId = (i) => `${list.id}-${i}`;
    c.draw = () => {
      list.replaceChildren(
        ...c.matches.map((e, i) =>
          el(
            'li',
            {
              class: i === c.active ? 'active' : '',
              attrs: { role: 'option', id: optionId(i), 'aria-selected': i === c.active ? 'true' : 'false' },
              on: {
                // Keep focus in the field while picking with the mouse.
                mousedown: (ev) => ev.preventDefault(),
                click: () => pick(e),
              },
            },
            el('span', { text: e.name }),
            el('span', { class: 'bd-match-cat', text: e.board_id ? `${e.category} ${e.board_id}` : e.category }),
          ),
        ),
      );
      const open = c.matches.length > 0;
      list.hidden = !open;
      input.setAttribute('aria-expanded', open ? 'true' : 'false');
      if (c.active >= 0) input.setAttribute('aria-activedescendant', optionId(c.active));
      else input.removeAttribute('aria-activedescendant');
    };
    c.close = () => {
      c.matches = [];
      c.active = -1;
      c.draw();
    };
    c.refresh = () => {
      if (document.activeElement !== input) return;
      c.matches = matchesFor(input.value, category());
      if (c.active >= c.matches.length) c.active = -1;
      c.draw();
    };
    const reset = () => {
      input.value = '';
      c.close();
      input.focus({ preventScroll: true });
    };
    async function pick(e) {
      await commit('quick', () => actions.tag(dto.id, e.id));
      reset();
    }
    async function createTyped() {
      const name = input.value.trim();
      if (!name) return;
      try {
        // False: nothing was added (the category was not chosen), so the text stays.
        if ((await create(name)) === false) {
          input.focus({ preventScroll: true });
          return;
        }
        clearError('quick');
        reset();
      } catch (err) {
        showError(String(err), 'quick');
        input.focus({ preventScroll: true });
      }
    }
    input.addEventListener('input', () => {
      c.matches = matchesFor(input.value, category());
      c.active = -1;
      c.draw();
    });
    input.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (!c.matches.length) return;
        e.preventDefault();
        const n = c.matches.length;
        c.active = e.key === 'ArrowDown' ? (c.active + 1) % n : (c.active - 1 + n) % n;
        c.draw();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (c.active >= 0) pick(c.matches[c.active]);
        else createTyped();
      } else if (e.key === 'Escape' && c.matches.length) {
        e.preventDefault();
        e.stopPropagation();
        c.close();
      }
    });
    input.addEventListener('blur', () => c.close());
    list.hidden = true;
    return c;
  }

  const comboAttrs = (id, listId, extra) => ({
    id,
    type: 'text',
    autocomplete: 'off',
    spellcheck: 'false',
    role: 'combobox',
    'aria-autocomplete': 'list',
    'aria-expanded': 'false',
    'aria-controls': listId,
    ...extra,
  });

  // ---- Quick add: type, pick a match or press Enter to create in the chosen category.
  const qaCategory = el('input', { attrs: { id: 'bd-category', type: 'text', autocomplete: 'off', list: 'bd-categories', 'aria-label': 'Category for a new element' } });
  qaCategory.addEventListener('change', () => {
    lastCategory = qaCategory.value.trim();
  });
  const qa = adder(
    el('input', { attrs: comboAttrs('bd-quick', 'bd-matches', { placeholder: 'Type an element' }) }),
    el('ul', { class: 'bd-matches', attrs: { id: 'bd-matches', role: 'listbox', 'aria-label': 'Matching elements' } }),
    () => null,
    // Enter with no match picked (decisions.md, entries 156 and 157): an element anywhere
    // with that name, ignoring case, is added; `Category;Name` creates without asking;
    // any other name asks for its category, the New in one chosen.
    async (name) => {
      const on = new Set(dto.groups.flatMap((g) => g.elements.map((e) => e.id)));
      const names = dto.categories.map((c) => c.name);
      const plan = quickAddPlan(name, dto.elements, on, names);
      if (plan.kind === 'none') return false;
      if (plan.kind === 'create') return actions.create(dto.id, plan.category, plan.name);
      if (plan.kind === 'tag') return actions.tag(dto.id, plan.element.id);
      if (plan.kind === 'on') throw `${plan.element.name} is already on this sheet.`;
      const sheet = dto.id;
      const category = await askCategory(plan.name, names, qaCategory.value);
      if (!category || !dto || dto.id !== sheet) return false;
      lastCategory = category;
      qaCategory.value = category;
      return actions.create(sheet, category, plan.name);
    },
  );

  // ---- Hide empty: leaves out the boxes of categories with nothing on this sheet.
  let hideEmpty = loadHideEmpty();
  const hideToggle = el('input', { attrs: { type: 'checkbox', id: 'bd-hide-empty' } });
  hideToggle.checked = hideEmpty;
  hideToggle.addEventListener('change', () => {
    hideEmpty = hideToggle.checked;
    saveHideEmpty(hideEmpty);
    if (dto) drawGroups();
  });

  // The elements chosen for Cut and Copy (the Edit menu, main.js): a click picks one,
  // Command-click or Ctrl-click adds or drops one, Shift-click picks the run from the last
  // one clicked. Focus goes to the list, so the Edit menu acts on it.
  const groups = el('div', { class: 'bd-groups', attrs: { tabindex: '-1', 'aria-label': 'Elements on this sheet' } });
  const noneShown = el('p', { class: 'empty', text: 'No elements on this sheet yet.' });
  let picked = new Set();
  let anchor = null;

  /** The elements on the sheet in screen order: [{ id, name }]. */
  const onSheet = () => (dto ? dto.groups.flatMap((g) => g.elements.map((e) => ({ id: e.id, name: e.name }))) : []);

  function pickElement(id, how) {
    const order = onSheet().map((e) => e.id);
    if (how.range && anchor !== null && order.includes(anchor)) {
      const [a, b] = [order.indexOf(anchor), order.indexOf(id)].sort((x, y) => x - y);
      picked = new Set(order.slice(a, b + 1));
    } else if (how.toggle) {
      if (picked.has(id)) picked.delete(id);
      else picked.add(id);
      anchor = id;
    } else {
      picked = picked.size === 1 && picked.has(id) ? new Set() : new Set([id]);
      anchor = id;
    }
    drawGroups();
    groups.focus({ preventScroll: true });
  }

  groups.addEventListener('click', (e) => {
    const b = e.target.closest('.bd-pick');
    if (!b || !groups.contains(b)) return;
    pickElement(Number(b.dataset.id), { range: e.shiftKey, toggle: e.metaKey || e.ctrlKey });
  });
  groups.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && picked.size && e.target.tagName !== 'INPUT') {
      e.preventDefault();
      e.stopPropagation();
      picked = new Set();
      drawGroups();
    }
  });

  /**
   * One box per category, by name, kept across redraws so typing in a box's add field
   * survives a commit: { name, node, count, list, add }.
   */
  let boxes = [];
  let boxNames = '';
  let boxSeq = 0;

  function makeBox(name) {
    const k = boxSeq++;
    const id = `bd-add-${k}`;
    const listId = `bd-add-${k}-matches`;
    const count = el('span', { class: 'bd-count' });
    const list = el('ul', { class: 'bd-items' });
    const add = adder(
      el('input', { class: 'bd-add', attrs: comboAttrs(id, listId, { placeholder: 'Add', 'aria-label': `Add an element to ${name}` }) }),
      el('ul', { class: 'bd-matches', attrs: { id: listId, role: 'listbox', 'aria-label': `Matching ${name} elements` } }),
      () => name,
      (typed) => actions.create(dto.id, name, typed),
    );
    const node = el(
      'section',
      { class: `bd-box bd-tone-${k % TONES}`, attrs: { 'aria-label': name } },
      el('h3', {}, el('span', { class: 'bd-cat', text: name, title: name }), count),
      list,
      el('div', { class: 'bd-combo' }, add.input, add.list),
    );
    return { name, node, count, list, add };
  }

  /** The quantity of `e` on the sheet, and whether it counts in the day's total. */
  async function editQuantity(e) {
    const got = await customDialog((close) => {
      const count = el('input', { class: 'w-xs', attrs: { id: 'bd-qty-count', type: 'text', inputmode: 'numeric', value: String(e.quantity || 1), 'data-autofocus': true } });
      const daily = el('input', { attrs: { id: 'bd-qty-daily', type: 'checkbox', checked: e.daily !== false } });
      const go = () => close({ count: count.value, daily: daily.checked });
      count.addEventListener('keydown', (ev) => {
        if (ev.key === 'Enter') go();
      });
      return el(
        'div',
        { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
        el('h2', { class: 'modal-title', text: `Quantity of ${e.name}`, attrs: { id: 'modal-title' } }),
        el('p', { class: 'modal-message', text: 'How many this sheet needs. In a category that uses daily quantities, it adds to the day’s total; untick it for a group already counted on another sheet that day.' }),
        el('div', { class: 'el-row' }, el('label', { class: 'bd-field', attrs: { for: 'bd-qty-count' } }, el('span', { text: 'Quantity' }), count), el('label', { class: 'el-check', attrs: { for: 'bd-qty-daily' } }, daily, el('span', { text: 'Counts in the day’s total' }))),
        el(
          'div',
          { class: 'modal-actions' },
          el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
          el('button', { class: 'btn btn-primary', text: 'Set Quantity', attrs: { type: 'button' }, on: { click: go } }),
        ),
      );
    });
    if (got) commit('quick', () => actions.quantity(dto.id, e.id, got.count, got.daily));
  }

  function drawBoxes() {
    const names = dto.categories.map((c) => c.name);
    const key = names.join('\n');
    if (key === boxNames) return;
    boxNames = key;
    const old = new Map(boxes.map((b) => [b.name, b]));
    boxes = names.map((n) => old.get(n) || makeBox(n));
    groups.replaceChildren(...boxes.map((b) => b.node), noneShown);
  }

  function drawGroups() {
    drawBoxes();
    const ids = new Set(onSheet().map((e) => e.id));
    picked = new Set([...picked].filter((id) => ids.has(id)));
    const clip = actions.clipboard ? actions.clipboard() : null;
    const cut = clip && clip.kind === 'elements' && clip.cut && clip.sheet === dto.id ? new Set(clip.elements) : new Set();
    const byName = new Map(dto.groups.map((g) => [g.category, g.elements]));
    for (const b of boxes) {
      const els = byName.get(b.name) || [];
      b.count.textContent = String(els.length);
      b.count.title = els.length === 1 ? `1 ${b.name} element on this sheet` : `${els.length} ${b.name} elements on this sheet`;
      b.node.classList.toggle('bd-empty', !els.length);
      // A box being typed in stays, so the add field keeps focus.
      b.node.hidden = boxHidden(hideEmpty, els.length, b.node.contains(document.activeElement));
      b.list.replaceChildren(
        ...els.map((e) =>
          el(
            'li',
            { class: `bd-chip${picked.has(e.id) ? ' picked' : ''}${cut.has(e.id) ? ' cut' : ''}` },
            el(
              'button',
              {
                class: 'bd-pick',
                title: cut.has(e.id) ? `${e.name} (cut: Paste on another sheet moves it there)` : `Select ${e.name} to cut or copy it`,
                attrs: { type: 'button', 'aria-pressed': picked.has(e.id) ? 'true' : 'false' },
                data: { id: e.id },
              },
              e.board_id ? el('span', { class: 'bd-id', text: e.board_id }) : null,
              el('span', { class: 'bd-name', text: e.name }),
            ),
            el('button', {
              class: e.quantity ? 'bd-qty set' : 'bd-qty',
              text: e.quantity ? `×${e.quantity}${e.daily === false ? '*' : ''}` : '×1',
              title: e.quantity
                ? `${e.quantity} needed on this sheet${e.daily === false ? ', not counted in the day’s total' : ''}. Click to change.`
                : `How many ${e.name} this sheet needs`,
              attrs: { type: 'button', 'aria-label': `Quantity of ${e.name}` },
              on: { click: () => editQuantity(e) },
            }),
            el('button', {
              class: 'bd-remove',
              text: '×',
              title: `Remove ${e.name} from this sheet`,
              attrs: { type: 'button', 'aria-label': `Remove ${e.name} from this sheet` },
              on: {
                click: () => {
                  commit('quick', () => actions.untag(dto.id, e.id));
                  qa.input.focus({ preventScroll: true });
                },
              },
            }),
          ),
        ),
      );
    }
    noneShown.hidden = !(hideEmpty && !dto.groups.length);
  }

  // ---- Images on the scene (decisions.md, entry 77): thumbnails, Add Image… and ×.
  const imageList = el('span', { class: 'el-images', attrs: { role: 'list', 'aria-label': 'Images of this scene' } });
  const addImage = el('button', { class: 'btn', text: 'Add Image…', attrs: { type: 'button', id: 'bd-add-image' }, on: { click: () => dto && actions.addImage && commit('quick', () => actions.addImage(dto.id)) } });
  let imagesFor = '';
  function drawImages() {
    const list = dto.images || [];
    const key = `${dto.id}:${list.join('\n')}`;
    if (key === imagesFor) return;
    imagesFor = key;
    imageList.replaceChildren(
      ...(list.length
        ? list.map((path, i) => {
            const name = path.split(/[\\/]/).pop();
            const thumb = el('span', { class: 'el-thumb', text: 'Loading', attrs: { 'aria-hidden': 'true' } });
            actions.image(dto.id, i).then(
              (url) => {
                if (url) showThumb(thumb, url, name);
                else {
                  thumb.textContent = 'Missing';
                  thumb.title = `No image file at ${path}`;
                }
              },
              () => (thumb.textContent = 'Missing'),
            );
            const sheetId = dto.id;
            const remove = el('button', { class: 'btn btn-mini', text: '×', title: `Take ${name} off this sheet (the file stays)`, attrs: { type: 'button', 'aria-label': `Take image ${name} off` }, on: { click: () => commit('quick', () => actions.removeImage(sheetId, i)) } });
            return el('span', { class: 'el-image', title: path, attrs: { role: 'listitem' } }, thumb, el('span', { class: 'el-image-name', text: name }), remove);
          })
        : [el('span', { class: 'bd-hint', text: 'None. The schedule keeps only where each image is.' })]),
    );
  }
  const imagesRow = el('div', { class: 'bd-row bd-images' }, el('span', { class: 'bd-label', text: 'Images' }), imageList, addImage);

  /** The header rows, laid out in `order` without the hidden fields, at the widths set: rebuilt only when that changes. */
  const form = el('div', { class: 'bd-form' });
  let formOrder = '';
  function layOut(all, hiddenKeys = [], widthPairs = [], lengths = []) {
    const hide = new Set(hiddenKeys);
    const widthOf = new Map(widthPairs);
    const order = all.filter((k) => !hide.has(k));
    const key = `${order.join(',')}|${widthPairs.map((p) => p.join(':')).join(',')}|${lengths.join(',')}`;
    if (key === formOrder) return;
    formOrder = key;
    // Keep what is typed: the inputs are made again, so carry their values over.
    const kept = Object.fromEntries(Object.entries(inputs).map(([k, i]) => [k, [i.value, i.getAttribute('aria-invalid')]]));
    // Six, five and the rest, or the rows set in the designer (decision 186).
    const rows = sheetRows(order, lengths);
    form.replaceChildren(...rows.map((row) => el('div', { class: 'bd-row' }, ...row.map((k) => fieldOf(sized(DEF.get(k), widthOf.get(k)))))), imagesRow);
    for (const [k, [v, bad]] of Object.entries(kept)) {
      if (!inputs[k]) continue;
      inputs[k].value = v;
      if (bad) inputs[k].setAttribute('aria-invalid', bad);
    }
  }
  layOut(orderOf(null));
  const elementsPart = el(
    'div',
    { class: 'bd-elements' },
    el(
      'div',
      { class: 'bd-quick' },
      el('h2', { text: 'Elements' }),
      el('label', { class: 'bd-field bd-grow', attrs: { for: 'bd-quick' } }, el('span', { text: 'Quick add' }), el('div', { class: 'bd-combo' }, qa.input, qa.list)),
      el('label', { class: 'bd-field', attrs: { for: 'bd-category' } }, el('span', { text: 'New in' }), qaCategory),
      el('label', { class: 'bd-check', attrs: { for: 'bd-hide-empty' } }, hideToggle, 'Hide empty'),
    ),
    el('p', {
      class: 'bd-hint',
      text: 'Type in Quick add or a box, pick a match with the arrow keys and Enter, or press Enter to add a new element. Click elements to select them (Command-click or Ctrl-click for more), then Cut or Copy, and Paste on another sheet.',
    }),
    groups,
  );
  const body = el(
    'div',
    { class: 'bd-body' },
    el(
      'div',
      { class: 'bd-head' },
      nav.prev,
      nav.next,
      el('label', { class: 'field', attrs: { for: 'bd-order', title: 'The order Previous and Next step through the sheets' } }, 'by', nav.order),
      nav.where,
      nav.place,
      nav.revised,
      el('span', { class: 'bd-spacer' }),
      el('label', { class: 'bd-check', attrs: { for: 'bd-decimal', title: 'Show the shoot time as decimal hours (1.50 for 1:30); either form can be typed' } }, nav.decimal, 'Decimal time'),
      nav.event,
      nav.add,
      nav.dup,
      nav.del,
    ),
    error,
    marks,
    form,
    elementsPart,
    lists.categories,
  );

  function fill(field, value) {
    const i = inputs[field];
    if (!i) return;
    // Never overwrite what is being typed, nor a refused value waiting to be fixed.
    if (document.activeElement === i || i.getAttribute('aria-invalid') === 'true') return;
    i.value = value;
  }

  function render(next) {
    dto = next;
    if (!dto) {
      built = null;
      host.replaceChildren(el('div', { class: 'bd-head' }, nav.add), empty);
      return;
    }
    if (built !== dto.id) {
      built = dto.id;
      picked = new Set();
      anchor = null;
      clearError(null, true);
      for (const i of Object.values(inputs)) i.value = '';
      qa.input.value = '';
      qa.close();
      for (const b of boxes) {
        b.add.input.value = '';
        b.add.close();
      }
      host.replaceChildren(body);
    }
    layOut(orderOf(dto), dto.hidden_fields || [], dto.field_widths || [], dto.field_rows || []);
    for (const field of FIELDS) fill(field, shown(field));
    drawImages();
    drawMarks();
    nav.order.value = dto.order || 'board';
    if (document.activeElement !== inputs.pages && document.activeElement !== inputs.eighths) {
      fill('pages', dto.pages_whole);
      fill('eighths', dto.pages_eighths);
    }
    const scene = dto.scenes ? `Scene ${dto.scenes}` : 'No scene number';
    nav.where.textContent = `Sheet ${dto.index + 1} of ${dto.count} · ${scene} ·`;
    nav.place.textContent = dto.place;
    nav.place.disabled = dto.strip === null || dto.strip === undefined;
    nav.revised.textContent = dto.revised ? `· ${dto.revised}` : '';
    nav.prev.disabled = dto.prev === null;
    nav.next.disabled = dto.next === null;
    const options = (list, values) => list.replaceChildren(...values.map((v) => el('option', { attrs: { value: v } })));
    options(lists.categories, dto.categories.map((c) => c.name));
    const names = dto.categories.map((c) => c.name);
    if (document.activeElement !== qaCategory && !qaCategory.value) {
      qaCategory.value = lastCategory || names[0] || '';
    }
    drawGroups();
    qa.refresh();
    for (const b of boxes) b.add.refresh();
  }

  return {
    render,
    showError,
    current: () => dto,
    /** The elements chosen for Cut and Copy, in screen order: [{ id, name }]. */
    pickedElements: () => onSheet().filter((e) => picked.has(e.id)),
    /** True when focus is on the element list, so the Edit menu's Cut, Copy and Paste act on it. */
    elementsFocused: () => Boolean(dto) && groups.contains(document.activeElement),
    /** Commits the field being typed in (its change event), before stepping away. */
    settle() {
      const a = document.activeElement;
      if (a && host.contains(a) && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA')) a.blur();
    },
    focusFirst() {
      inputs.scenes.focus({ preventScroll: true });
    },
    /**
     * Shows header field `field` as a Find result: marked until it is left or another is
     * marked, scrolled into view, and focused unless `focus` is false.
     */
    mark(field, focus = true) {
      for (const i of host.querySelectorAll('.found')) i.classList.remove('found');
      const input = inputs[field];
      if (!input) return;
      input.classList.add('found');
      input.addEventListener('blur', () => input.classList.remove('found'), { once: true });
      if (typeof input.scrollIntoView === 'function') input.scrollIntoView({ block: 'nearest' });
      if (focus) input.focus({ preventScroll: true });
    },
  };
}
