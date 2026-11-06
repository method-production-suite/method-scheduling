// Strip dialogs and helpers: the Merge Strips preview (what the engine's sheet merge would
// combine and leave out, src/sheet.rs), and the strip layout: the fields a board's strips
// show, on which line, how wide and how (Board > Strip Layout, src/board.rs), and the one
// cell renderer the board, the printed board and the one-line reports share (stripGrid,
// stripCell, sceneRows; decisions.md, entry 87). Text goes in through textContent and
// input values only (dom.js); sizes go through constructed stylesheets (dynstyle.js),
// numbers and fixed words only.

import { el } from './dom.js';
import { customDialog, errorLine } from './modal.js';
import { countCategories, countSay, draftColorClass, listSay, ruleClass } from './logic.js';

const cell = (tag, text, cls) => el(tag, { class: cls || '', text: text === null || text === undefined ? '' : String(text) });

/** A small table: `head` labels, then `rows` of text cells. */
function table(caption, head, rows) {
  return el(
    'table',
    { class: 'mg-table', attrs: { 'aria-label': caption } },
    el('thead', {}, el('tr', {}, ...head.map((h) => el('th', { text: h, attrs: { scope: 'col' } })))),
    el('tbody', {}, ...rows.map((r) => el('tr', {}, ...r.map((t, i) => cell(i === 0 ? 'th' : 'td', t, i === 0 ? 'mg-key' : ''))))),
  );
}

/** "scene 2 and scene 3", "scene 2, scene 3 and scene 4". */
function andList(items) {
  if (items.length <= 1) return items.join('');
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
}

const lower = (t) => (t ? t.charAt(0).toLowerCase() + t.slice(1) : t);

/**
 * Board > Merge Strips: shows `dto` (merge_sheets_preview) and asks before merging.
 * `submit()` runs the merge and rejects with a sentence, which the dialog shows. Resolves
 * true when merged.
 */
export function mergeDialog(dto, submit) {
  return customDialog((close) => {
    const error = errorLine();
    const n = dto.merged.length + 1;
    const others = andList(dto.merged.map((m) => lower(m.title)));
    const parts = [
      el('p', {
        class: 'modal-message',
        attrs: { id: 'merge-message' },
        text: `The merge applies on every board. ${dto.survivor.title}, the first selected, keeps its sheet, its fields and its place; the scenes and elements of ${others} go into it in the order they were selected, and ${dto.merged.length === 1 ? 'that strip leaves' : 'those strips leave'} every board. Banners and day breaks stay where they are. Undo brings it all back.`,
      }),
    ];
    if (dto.combined.length) {
      parts.push(
        el('h3', { class: 'mg-head', text: 'Fields the merged sheet gets' }),
        table(
          'Fields the merged sheet gets',
          ['Field', 'Now', 'After the merge'],
          dto.combined.map((c) => [c.label, c.before || '(empty)', c.after || '(empty)']),
        ),
      );
    }
    if (dto.elements_added.length) {
      parts.push(el('h3', { class: 'mg-head', text: 'Elements it gains' }), el('p', { class: 'mg-list', text: dto.elements_added.join(', ') }));
    }
    parts.push(el('h3', { class: 'mg-head', text: 'Values left out' }));
    if (dto.dropped.length) {
      parts.push(
        el('p', { class: 'mg-note', text: `These differ from what the merged sheet keeps, so they are not carried over.` }),
        table(
          'Values left out',
          ['Field', 'Sheet', 'Value'],
          dto.dropped.map((d) => [d.label, d.title, d.value]),
        ),
      );
    } else {
      parts.push(el('p', { class: 'mg-note', text: 'None. Every value is kept or combined.' }));
    }
    const ok = el('button', {
      class: 'btn btn-primary',
      text: 'Merge Strips',
      attrs: { type: 'button', id: 'merge-ok', 'data-autofocus': true },
      on: {
        click: async () => {
          ok.disabled = true;
          try {
            await submit();
            close(true);
          } catch (e) {
            error.textContent = String(e);
            error.hidden = false;
            ok.disabled = false;
          }
        },
      },
    });
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'merge-title', 'aria-describedby': 'merge-message' } },
      el('h2', { class: 'modal-title', text: `Merge ${n} strips into one sheet?`, attrs: { id: 'merge-title' } }),
      error,
      el('div', { class: 'mg-body' }, ...parts),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        ok,
      ),
    );
  }).then((r) => r === true);
}

// ---------------------------------------------------------------------------
// The strip layout (decisions.md, entries 39, 78 and 87)

/** Every field a strip can show, as the engine names them (StripField::ALL), with its label and default width. */
export const LAYOUT_FIELDS = [
  ['scene', 'Scene', 'narrow'],
  ['int_ext', 'I/E', 'narrow'],
  ['set', 'Set', 'wide'],
  ['time_of_day', 'Time of day', 'medium'],
  ['pages', 'Pages', 'narrow'],
  ['cast', 'Cast IDs', 'wide'],
  ['location', 'Location', 'wide'],
  ['script_day', 'Script day', 'narrow'],
  ['unit', 'Unit', 'narrow'],
  ['sequence', 'Sequence', 'narrow'],
  ['episode', 'Episode', 'narrow'],
  ['shoot_time', 'Shoot time', 'narrow'],
  ['synopsis', 'Synopsis', 'widest'],
  ['cast_count', 'Cast count', 'narrow'],
  ['element_count', 'Element count', 'narrow'],
  ['elements', 'Elements', 'wide'],
  ['category_elements', 'Category elements', 'wide'],
  ['spacer', 'Spacer', 'narrow'],
  ['text', 'Text', 'medium'],
];

/** The fields a layout may show more than once (StripField::repeats). */
const REPEATS = new Set(['spacer', 'text']);

/** The fields whose column picks a category (StripField::picks_category). */
const PICKS = new Set(['element_count', 'category_elements']);

/**
 * The fields that show something of the sheet (StripField::SHEET): what the calendar and
 * reports pick from. A category's elements needs a layout's category pick (entry 194).
 */
export const SHEET_FIELDS = LAYOUT_FIELDS.filter(([f]) => !REPEATS.has(f) && f !== 'category_elements');

/** The widths a column can take (StripWidth::ALL), with their labels. */
export const LAYOUT_WIDTHS = [
  ['narrow', 'Narrow'],
  ['medium', 'Medium'],
  ['wide', 'Wide'],
  ['widest', 'Widest'],
];

/** Where a field's text sits (Align::ALL): the field's usual way, or set. */
export const LAYOUT_ALIGNS = [
  ['auto', 'Usual'],
  ['left', 'Left'],
  ['center', 'Center'],
  ['right', 'Right'],
];

/** Text size steps (TextSize::ALL), for strip layouts and banners. */
export const TEXT_SIZES = [
  ['small', 'Small'],
  ['normal', 'Normal'],
  ['large', 'Large'],
];

/** The most lines a strip has (MAX_STRIP_LINES). */
export const MAX_LINES = 4;

/** The layout every new board starts with (StripLayout::default). */
export const DEFAULT_LAYOUT = ['scene', 'int_ext', 'set', 'time_of_day', 'pages', 'cast'].map((f) => ({
  field: f,
  width: LAYOUT_FIELDS.find((x) => x[0] === f)[2],
  line: 1,
  span: 1,
  align: 'auto',
}));

/** A board column's track for each width: fixed for the narrow two, sharing the rest. */
const BOARD_TRACK = { narrow: '5em', medium: '7.5em', wide: 'minmax(9em, 1fr)', widest: 'minmax(14em, 2fr)' };
/** A printed column's share of the page for each width. */
const PRINT_WEIGHT = { narrow: 2, medium: 3, wide: 6, widest: 9 };

/** The grid columns for a layout's columns on the board (fixed words only). */
export function boardTracks(cols) {
  return cols.map((c) => BOARD_TRACK[c.width] || BOARD_TRACK.narrow).join(' ');
}

/** Each printed column's width in percent, two decimals, adding up to 100. */
export function printShares(cols) {
  const w = cols.map((c) => PRINT_WEIGHT[c.width] || PRINT_WEIGHT.narrow);
  const total = w.reduce((a, b) => a + b, 0) || 1;
  return w.map((x) => Math.floor((x / total) * 10000) / 100);
}

/** Fields that hold numbers and line up at the right. */
export const NUMERIC_FIELDS = new Set(['pages', 'shoot_time', 'cast_count', 'element_count']);

/** A field's line, from 0 (the view sends 1 to 4). */
const lineOf = (c) => Math.min(MAX_LINES, Math.max(1, Number(c.line) || 1)) - 1;

/** A field's label: the view's, else the list's. */
export const labelOf = (c) => c.label || (LAYOUT_FIELDS.find((x) => x[0] === c.field) || [c.field, c.field])[1];

/**
 * Where each field of `layout` sits (decisions.md, entry 87), shared by the board, the
 * printed board and the one-line report: the first line's fields are the columns (`cols`),
 * one each; a later line's fields follow one another under them, each covering `span`
 * columns (a merged cell). Returns { cols, lines, cells: [{ c, k, line, col, span }] },
 * `k` being the field's place in the layout (and in a strip's `cells`).
 */
export function stripGrid(layout) {
  const lay = Array.isArray(layout) && layout.length ? layout : DEFAULT_LAYOUT;
  const cols = lay.filter((c) => lineOf(c) === 0);
  const n = Math.max(1, cols.length);
  const next = new Array(MAX_LINES).fill(0);
  const cells = lay.map((c, k) => {
    const line = lineOf(c);
    const col = Math.min(next[line], n - 1);
    const span = line === 0 ? 1 : Math.max(1, Math.min(Number(c.span) || 1, n - col));
    next[line] = col + span;
    return { c, k, line, col, span };
  });
  const lines = cells.reduce((m, g) => Math.max(m, g.line + 1), 1);
  return { cols, lines, cells };
}

/**
 * The cells of line `line` in column order, with gaps filled: [{ g, span }], `g` null for
 * an empty filler. For tables, where a later line's cells take colspans.
 */
export function lineCells(grid, line) {
  const n = Math.max(1, grid.cols.length);
  const out = [];
  let at = 0;
  for (const g of grid.cells.filter((x) => x.line === line)) {
    if (g.col > at) out.push({ g: null, span: g.col - at });
    out.push({ g, span: g.span });
    at = g.col + g.span;
  }
  if (at < n) out.push({ g: null, span: n - at });
  return out;
}

/** What a field's cell says: a custom text's own text, nothing for a spacer, else the strip's cell. */
export function cellText(g, cells) {
  if (g.c.field === 'text') return g.c.text || '';
  if (g.c.field === 'spacer') return '';
  return (cells && cells[g.k]) || '';
}

/**
 * A field's classes, the same on the board and on paper: what it holds (`fc-num` numbers
 * at the right, `fc-id` the scene number, `fc-cast` IDs, `fc-txt` text), its alignment
 * when set, bold and italic, and its line styles (decision 185).
 */
export function cellClass(c) {
  const kind = NUMERIC_FIELDS.has(c.field) ? 'fc-num' : c.field === 'scene' ? 'fc-id' : c.field === 'cast' ? 'fc-cast' : 'fc-txt';
  const align = c.align && c.align !== 'auto' && LAYOUT_ALIGNS.some((a) => a[0] === c.align) ? ` al-${c.align}` : '';
  const size = c.size && TEXT_SIZES.some((t) => t[0] === c.size) ? ` cz-${c.size}` : '';
  return `${kind}${align}${c.bold ? ' fb' : ''}${c.italic ? ' fi' : ''}${size}${ruleClass(c)}`;
}

/**
 * One field's cell: `tag` (span on the board, td on paper) with the field's classes plus
 * `extra`, the text, a title where text may be cut, and nodes before the text (`before`,
 * one or a list: the marks) and after it (a part number).
 */
export function stripCell(tag, g, text, { extra = '', span = 1, before = null, after = null } = {}) {
  const cls = cellClass(g.c);
  const cut = cls.startsWith('fc-txt') || cls.startsWith('fc-cast') || g.line > 0;
  return el(
    tag,
    { class: `${cls}${extra}`, title: cut && text ? (g.line > 0 ? `${labelOf(g.c)}: ${text}` : text) : null, attrs: span > 1 ? { colspan: String(span) } : {} },
    ...(Array.isArray(before) ? before : [before]),
    text,
    after,
  );
}

/**
 * A scene strip's rows on paper (shared by the printed board and the one-line report):
 * its first line, then each later line that says something, as table rows whose cells
 * span the columns their fields cover. `cls` goes on every row; `lead` (nodes) leads the
 * first line's first cell, `first` changes its text.
 */
export function sceneRows(grid, cells, cls, { lead = null, first = (t) => t, sub = null } = {}) {
  const out = [];
  for (let line = 0; line < grid.lines; line++) {
    const parts = lineCells(grid, line);
    if (line > 0 && parts.every((p) => !p.g || !cellText(p.g, cells))) continue;
    const tds = parts.map((p, i) => {
      if (!p.g) return el('td', { attrs: p.span > 1 ? { colspan: String(p.span) } : {} });
      const text = line === 0 && i === 0 ? first(cellText(p.g, cells)) : cellText(p.g, cells);
      const node = stripCell('td', p.g, text, { span: p.span, before: line === 0 && i === 0 ? lead : null });
      if (sub && line === 0 && p.g.k === sub.k && sub.text) node.append(el('div', { class: 'rpt-sub', text: sub.text }));
      return node;
    });
    out.push(el('tr', { class: `${cls}${line > 0 ? ' bp-line' : ''}` }, ...tds));
  }
  return out;
}

/**
 * The swatch on a scene a script draft changed (entry 247): the newest draft's color, said
 * in its title. Small, so the strip's own colors still read. On the board and in print.
 */
export function draftMark(strip) {
  return el('span', { class: `draft-mark ${draftColorClass(strip.draft_color)}`, title: strip.draft, attrs: { role: 'img', 'aria-label': strip.draft } });
}

/** The column headings for a layout's columns: the first line's fields. */
export function layoutHead(grid, labelFor = labelOf) {
  return el('tr', {}, ...grid.cols.map((c) => el('th', { class: cellClass(c), text: c.field === 'spacer' ? '' : c.field === 'text' ? c.text || '' : labelFor(c), attrs: { scope: 'col' } })));
}

/** The CSS that places each field of `grid` in a strip under `scope` (numbers only). */
export function gridRules(scope, grid) {
  return grid.cells.map((g) => `${scope} > .sc-${g.k} { grid-area: ${g.line + 1} / ${g.col + 1} / span 1 / span ${g.span}; }`).join('\n');
}

/** A layout's fields and size as one string, to tell whether anything changed. */
export function layoutKey(columns, size = 'normal', marks = null) {
  const m = marks || {};
  return JSON.stringify([size || 'normal', Boolean(m.hide_red_flags), Boolean(m.hide_conflicts), ...columns.map((c) => [c.field, c.width, lineOf(c), lineOf(c) ? Number(c.span) || 1 : 1, c.align || 'auto', Boolean(c.bold), Boolean(c.italic), c.text || '', c.category ?? null, Boolean(c.board_ids), Boolean(c.rule_after), Boolean(c.rule_under)])]);
}

/**
 * Board > Strip Layout: every field with Show, its width, line, span (lines 2 to 4), its
 * alignment, bold, italic, its line styles (a line after it, a rule under it; decisions.md,
 * entry 185) and (custom text) its text, and Up and Down to order the shown
 * ones; spacers and custom text are added as many times as wanted. Which layout the board
 * uses (its own or a named one from the library, decisions.md, entry 78), its text size
 * and which layout the one-line report uses.
 *
 * The strip designer's switches for the red flag and conflict marks (behavior note 25,
 * decisions.md, entry 136) and the category an element count counts (behavior note 27,
 * entry 147) go with the layout.
 *
 * opts: { board (name), own ([columns]), ownSize, ownMarks ({ hide_red_flags,
 * hide_conflicts }), uses (the named layout's id the board picked, or null), library
 * ([{ id, name, columns, size, marks }], the named strip layouts), categories ([{ id,
 * name }], what an element count can count), oneLine (the one-line report's
 * pick, or null), save({ target, columns, size, marks, pick, oneLine }), saveAs(name,
 * columns, size, marks), remove(id) }. `target` is 'own' or a named layout's id: where the
 * columns go. Each returns a promise that rejects with a sentence, shown in the dialog.
 * Resolves true when something was saved.
 */
export function layoutDialog({ board, own, ownSize = 'normal', ownMarks = null, uses = null, library = [], categories = [], oneLine = null, save, saveAs, remove }) {
  return customDialog((close) => {
    const error = errorLine();
    const fail = (e) => {
      error.textContent = String(e);
      error.hidden = false;
    };
    const label = (f) => (LAYOUT_FIELDS.find((x) => x[0] === f) || [f, f])[1];
    const layoutOf = (target) => {
      const named = library.find((l) => l.id === target);
      return target === 'own' || !named ? { columns: own, size: ownSize, marks: ownMarks || {} } : { columns: named.columns, size: named.size || 'normal', marks: named.marks || {} };
    };
    let target = uses === null || !library.some((l) => l.id === uses) ? 'own' : uses;
    let rows = [];
    let uid = 0;
    const row = (c, on) => ({
      id: uid++,
      field: c.field,
      width: c.width || (LAYOUT_FIELDS.find((x) => x[0] === c.field) || [c.field, c.field, 'narrow'])[2],
      line: lineOf(c) + 1,
      span: Number(c.span) || 1,
      align: c.align || 'auto',
      bold: Boolean(c.bold),
      italic: Boolean(c.italic),
      rule_after: Boolean(c.rule_after),
      rule_under: Boolean(c.rule_under),
      text: c.text || '',
      category: c.category ?? null,
      board_ids: Boolean(c.board_ids),
      on,
    });
    const load = (columns) => {
      const shown = columns.map((c) => c.field);
      rows = [...columns.map((c) => row(c, true)), ...LAYOUT_FIELDS.filter(([f]) => !REPEATS.has(f) && !shown.includes(f)).map(([f, , w]) => row({ field: f, width: w }, false))];
    };
    load(layoutOf(target).columns);
    const list = el('ol', { class: 'ly-list', attrs: { 'aria-label': 'Fields' } });
    // The mark switches (behavior note 25): on shows the marks on this layout's strips.
    const markBox = (id, text, key) => {
      const box = el('input', { attrs: { type: 'checkbox', id } });
      return { box, key, node: el('label', { class: 'el-check', attrs: { for: id }, title: `Show the ${text.toLowerCase()} on strips` }, box, el('span', { text })) };
    };
    const marks = [markBox('ly-red-flags', 'Red flag marks', 'hide_red_flags'), markBox('ly-conflicts', 'Conflict marks', 'hide_conflicts')];
    const setMarks = (m) => {
      for (const x of marks) x.box.checked = !m[x.key];
    };
    const marksNow = () => Object.fromEntries(marks.map((x) => [x.key, !x.box.checked]));
    setMarks(layoutOf(target).marks);
    // Which category a field picks: the element count counts it (behavior note 27), a
    // category's elements lists it (entry 194). The picker lists categories only, its
    // search finds their names, and one pick replaces another. None counts every element,
    // or lists nothing.
    function categoryPart(field, key, legend, aria, extra, say) {
      const rowOf = () => rows.find((r) => r.field === field);
      const words = el('span', { class: 'el-count', attrs: { 'aria-live': 'polite' } });
      const find = el('input', { attrs: { id: `ly-${key}-find`, type: 'search', placeholder: 'Find a category', 'aria-label': `Find a category: ${aria}`, autocomplete: 'off' } });
      const list = el('div', { class: 'rf-pick-list', attrs: { role: 'radiogroup', 'aria-label': aria } });
      const node = el('fieldset', { class: `rf-part ly-count ly-${key}` }, el('legend', { text: legend }), el('div', { class: 'el-row' }, find, ...extra(rowOf, () => draw()), words), list);
      function draw() {
        const r = rowOf();
        node.hidden = !r || !r.on || !categories.length;
        if (node.hidden) return;
        words.textContent = say(r);
        list.replaceChildren(
          ...countCategories(categories, find.value).map((c) => {
            const id = `ly-${key}-${c.id}`;
            const box = el('input', { attrs: { id, type: 'radio', name: `ly-${key}`, checked: r.category === c.id } });
            box.addEventListener('change', () => {
              r.category = c.id;
              draw();
              const again = document.getElementById(id);
              if (again) again.focus();
            });
            return el('label', { class: 'el-check', attrs: { for: id } }, box, el('span', { text: c.name }));
          }),
        );
      }
      find.addEventListener('input', () => draw());
      return { node, draw };
    }
    const countPart = categoryPart(
      'element_count',
      'count',
      'Element count counts',
      'The category the element count counts',
      (rowOf, redraw) => [el('button', { class: 'btn btn-mini', text: 'Every Element', title: 'Count every element, as a number', attrs: { type: 'button', id: 'ly-count-all' }, on: { click: () => ((rowOf().category = null), redraw()) } })],
      (r) => countSay(categories, r.category),
    );
    // A category's elements: by name, or by board ID with the switch (entry 194).
    const idsBox = el('input', { attrs: { type: 'checkbox', id: 'ly-list-ids' } });
    const listPart = categoryPart(
      'category_elements',
      'list',
      'Category elements lists',
      'The category whose elements the field lists',
      (rowOf, redraw) => {
        idsBox.addEventListener('change', () => {
          rowOf().board_ids = idsBox.checked;
          redraw();
        });
        return [el('label', { class: 'el-check', attrs: { for: 'ly-list-ids' }, title: 'Show board IDs (names where an element has none)' }, idsBox, el('span', { text: 'Board IDs' }))];
      },
      (r) => {
        idsBox.checked = r.board_ids;
        return listSay(categories, r.category, r.board_ids);
      },
    );
    function drawCount() {
      countPart.draw();
      listPart.draw();
    }
    const preview = el('p', { class: 'mg-note ly-preview', attrs: { 'aria-live': 'polite' } });
    const size = el('select', { attrs: { id: 'ly-size', 'aria-label': 'Text size of the strips' } }, ...TEXT_SIZES.map(([v, l]) => el('option', { text: l, attrs: { value: v } })));
    size.value = layoutOf(target).size;

    // Which layout the board uses: its own, or a named one (the columns below are that one's).
    const pick = el(
      'select',
      { attrs: { id: 'ly-pick', 'aria-label': 'The layout this board uses' } },
      el('option', { text: "This board's own layout", attrs: { value: 'own' } }),
      ...library.map((l) => el('option', { text: l.name, attrs: { value: String(l.id), selected: l.id === target } })),
    );
    pick.addEventListener('change', () => {
      target = pick.value === 'own' ? 'own' : Number(pick.value);
      load(layoutOf(target).columns);
      size.value = layoutOf(target).size;
      setMarks(layoutOf(target).marks);
      del.disabled = target === 'own';
      draw(null, null);
    });
    const del = el('button', {
      class: 'btn',
      text: 'Delete Layout',
      attrs: { type: 'button', disabled: target === 'own', title: 'Delete the named layout; boards that use it go back to their own' },
      on: {
        click: async () => {
          if (target === 'own') return;
          if (del.dataset.sure !== '1') {
            del.dataset.sure = '1';
            del.textContent = 'Delete it?';
            return;
          }
          try {
            await remove(target);
            close(true);
          } catch (e) {
            fail(e);
          }
        },
      },
    });
    const newName = el('input', { class: 'w-m', attrs: { type: 'text', id: 'ly-new-name', placeholder: 'Name', maxlength: '200', 'aria-label': 'Name of a new named layout' } });
    const saveNew = el('button', {
      class: 'btn',
      text: 'Save as New Layout',
      attrs: { type: 'button', title: 'Keep these fields as a named layout this board uses (other boards and the one-line report can pick it)' },
      on: {
        click: async () => {
          try {
            await saveAs(newName.value, columns(), size.value, marksNow());
            close(true);
          } catch (e) {
            fail(e);
          }
        },
      },
    });
    const oneLineSel = el(
      'select',
      { attrs: { id: 'ly-one-line', 'aria-label': 'The layout the one-line report uses' } },
      el('option', { text: "The board's layout", attrs: { value: '' } }),
      ...library.map((l) => el('option', { text: l.name, attrs: { value: String(l.id), selected: l.id === oneLine } })),
    );
    // Spacers and custom text go in as often as wanted, shown, at the end of the shown ones.
    const add = (field) => {
      const at = rows.filter((r) => r.on).length;
      const r = row({ field }, true);
      rows.splice(at, 0, r);
      draw(r.id, 'on');
    };

    function move(i, d) {
      const j = i + d;
      if (j < 0 || j >= rows.length) return;
      [rows[i], rows[j]] = [rows[j], rows[i]];
      draw(rows[j].id, d < 0 ? 'up' : 'down');
    }

    const select = (aria, options, value, disabled, onChange) => {
      const s = el('select', { attrs: { 'aria-label': aria, disabled } }, ...options.map(([v, l]) => el('option', { text: l, attrs: { value: String(v) } })));
      s.value = String(value);
      s.addEventListener('change', () => onChange(s.value));
      return s;
    };

    function draw(focusId, focusWhat) {
      const firstCount = Math.max(1, rows.filter((r) => r.on && r.line === 1).length);
      list.replaceChildren(
        ...rows.map((r, i) => {
          const id = `ly-${r.id}`;
          const name = label(r.field);
          const on = el('input', { attrs: { type: 'checkbox', id, checked: r.on } });
          on.addEventListener('change', () => {
            r.on = on.checked;
            if (!r.on && REPEATS.has(r.field)) rows.splice(rows.indexOf(r), 1);
            draw(r.id, 'on');
          });
          const off = !r.on;
          const width = select(`Width of ${name}`, LAYOUT_WIDTHS, r.width, off || r.line > 1, (v) => (r.width = v));
          const line = select(`Line of ${name}`, [1, 2, 3, 4].map((n) => [n, `Line ${n}`]), r.line, off, (v) => {
            r.line = Number(v);
            draw(null, null);
          });
          const spans = Array.from({ length: firstCount }, (_, n) => [n + 1, n === 0 ? '1 col' : `${n + 1} cols`]);
          const span = select(`Columns ${name} covers`, spans, Math.min(r.span, firstCount), off || r.line === 1, (v) => (r.span = Number(v)));
          const align = select(`Alignment of ${name}`, LAYOUT_ALIGNS, r.align, off, (v) => (r.align = v));
          const flag = (key, text, title) => {
            const box = el('input', { attrs: { type: 'checkbox', disabled: off, 'aria-label': `${title} ${name}`, checked: r[key] } });
            box.addEventListener('change', () => (r[key] = box.checked));
            return el('label', { class: `ly-flag ly-${key}`, title: `${title} ${name}` }, box, text);
          };
          const text =
            r.field === 'text'
              ? el('input', { class: 'ly-text', attrs: { type: 'text', value: r.text, maxlength: '100', placeholder: 'Text', disabled: off, 'aria-label': 'Text the field shows' }, on: { input: (e) => (r.text = e.target.value) } })
              : el('span', { class: 'ly-text' });
          const up = el('button', { class: 'btn', text: 'Up', attrs: { type: 'button', 'aria-label': `Move ${name} up`, disabled: i === 0 }, on: { click: () => move(i, -1) } });
          const down = el('button', { class: 'btn', text: 'Down', attrs: { type: 'button', 'aria-label': `Move ${name} down`, disabled: i === rows.length - 1 }, on: { click: () => move(i, 1) } });
          const node = el('li', { class: `ly-row ly-design${r.on ? '' : ' off'}` }, el('label', { class: 'ly-name', attrs: { for: id } }, on, name), width, line, span, align, flag('bold', 'B', 'Bold'), flag('italic', 'I', 'Italic'), flag('rule_after', '|', 'A line after'), flag('rule_under', '_', 'A rule under'), text, up, down);
          if (r.id === focusId) {
            const t = focusWhat === 'up' ? up : focusWhat === 'down' ? down : on;
            queueMicrotask(() => (t.disabled ? on : t).focus());
          }
          return node;
        }),
      );
      const shown = rows.filter((r) => r.on);
      const lines = [1, 2, 3, 4].map((n) => shown.filter((r) => r.line === n).map((r) => (r.field === 'text' ? `"${r.text}"` : label(r.field)))).filter((x, n) => n === 0 || x.length);
      preview.textContent = lines[0].length
        ? `Strips show: ${lines.map((x, n) => (n ? `line ${n + 1}: ${x.join(', ')}` : x.join(', '))).join('; ')}.`
        : 'Put at least one field on the first line.';
      drawCount();
    }
    draw(null, null);

    const columns = () => rows.filter((r) => r.on).map((r) => ({ field: r.field, width: r.width, line: r.line, span: r.line === 1 ? 1 : r.span, align: r.align, bold: r.bold, italic: r.italic, text: r.field === 'text' ? r.text : '', category: PICKS.has(r.field) ? r.category : null, board_ids: r.field === 'category_elements' && r.board_ids, rule_after: r.rule_after, rule_under: r.rule_under }));
    const go = async () => {
      try {
        await save({ target, columns: columns(), size: size.value, marks: marksNow(), pick: target === 'own' ? null : target, oneLine: oneLineSel.value ? Number(oneLineSel.value) : null });
        close(true);
      } catch (e) {
        fail(e);
      }
    };
    const reset = () => {
      load(DEFAULT_LAYOUT);
      size.value = 'normal';
      setMarks({});
      draw(null, null);
    };
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'ly-title', 'aria-describedby': 'ly-message' } },
      el('h2', { class: 'modal-title', text: `Strip Layout for ${board}`, attrs: { id: 'ly-title' } }),
      el('p', {
        class: 'modal-message',
        attrs: { id: 'ly-message' },
        text: 'Choose the fields scene strips show, left to right, on up to four lines. The first line\'s fields make the columns; a field on a later line covers as many of them as you choose. Save edits the layout picked above; a named layout changes on every board that uses it. Undo puts the old one back.',
      }),
      error,
      el('div', { class: 'ly-pick' }, el('label', { class: 'bd-field beside', attrs: { for: 'ly-pick' } }, el('span', { text: 'Layout' }), pick), del, newName, saveNew),
      el(
        'div',
        { class: 'ly-pick' },
        el('label', { class: 'bd-field beside', attrs: { for: 'ly-size' } }, el('span', { text: 'Text size' }), size),
        el('button', { class: 'btn', text: 'Add Spacer', attrs: { type: 'button', title: 'An empty cell that holds a place' }, on: { click: () => add('spacer') } }),
        el('button', { class: 'btn', text: 'Add Text', attrs: { type: 'button', title: 'Text of your own, the same on every strip' }, on: { click: () => add('text') } }),
        ...marks.map((x) => x.node),
      ),
      list,
      preview,
      countPart.node,
      listPart.node,
      el('div', { class: 'ly-pick' }, el('label', { class: 'bd-field beside', attrs: { for: 'ly-one-line' } }, el('span', { text: 'One-line report uses' }), oneLineSel)),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Default Layout', attrs: { type: 'button' }, on: { click: reset } }),
        el('span', { class: 'bd-spacer' }),
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Save', attrs: { type: 'button', id: 'ly-ok' }, on: { click: go } }),
      ),
    );
  }).then((r) => r === true);
}
