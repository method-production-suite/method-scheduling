// The page's own rules, apart from the page: pure functions with no imports and no DOM,
// so `node --test crates/method-schedule-app/page-tests` runs them in CI with no npm
// (decisions.md, entry 137). The modules that draw call these; what Rust decides stays
// in Rust.

// ---- Dates as typed (dates.js; Rust reads them the same way, src/calendar.rs)

const DASHED = /^(\d{4})-(\d{2})-(\d{2})$/;
const SLASHED = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/;
const pad = (n, w = 2) => String(n).padStart(w, '0');

/**
 * A typed date as `YYYY-MM-DD`: `2027-03-01`, or month, day and year with slashes
 * (`3/1/2027`). Null when it isn't one, or the day is past the month's end.
 */
export function typedDate(text) {
  const t = String(text ?? '').trim();
  let y;
  let m;
  let d;
  let r = DASHED.exec(t);
  if (r) [y, m, d] = [r[1], r[2], r[3]].map(Number);
  else {
    r = SLASHED.exec(t);
    if (!r) return null;
    [m, d, y] = [r[1], r[2], r[3]].map(Number);
  }
  if (m < 1 || m > 12 || d < 1) return null;
  const last = new Date(0);
  last.setUTCFullYear(y, m, 0);
  if (d > last.getUTCDate()) return null;
  return `${pad(y, 4)}-${pad(m)}-${pad(d)}`;
}

/** Every date from `a` to `b` (either order), as `YYYY-MM-DD`, at most `max` of them. */
export function datesBetween(a, b, max = 1000) {
  const [lo, hi] = a <= b ? [a, b] : [b, a];
  const out = [];
  const d = new Date(`${lo}T00:00:00Z`);
  while (out.length < max) {
    const iso = d.toISOString().slice(0, 10);
    if (iso > hi) break;
    out.push(iso);
    d.setUTCDate(d.getUTCDate() + 1);
  }
  return out;
}

// ---- The Red flags tab's small calendar (RFL-05, behavior note 25, decision 138)

/**
 * The dates picked after a click on `date`. A click replaces the pick with that date;
 * Command-click (Ctrl-click off the Mac: `add`) adds it, or removes it when picked;
 * Shift-click (`range`) adds every date from the anchor to this one and keeps the rest
 * of the pick, with or without Command. The anchor is the last date clicked that ended
 * up picked: a plain click, a Command-click that adds, or a Shift-click's end. A
 * Command-click that removes leaves it (behavior note 27, E-E17). Returns
 * { picked, anchor }; `picked` is a new Set.
 */
export function pickDate(picked, anchor, date, { add = false, range = false } = {}) {
  if (range && anchor) {
    return { picked: new Set([...picked, ...datesBetween(anchor, date)]), anchor: date };
  }
  if (add || range) {
    const next = new Set(picked);
    if (next.has(date)) {
      next.delete(date);
      return { picked: next, anchor };
    }
    next.add(date);
    return { picked: next, anchor: date };
  }
  return { picked: new Set([date]), anchor: date };
}

/** The modifiers of a click for [`pickDate`]: Command on the Mac, Ctrl elsewhere. */
export function pickMods(e, mac) {
  return { add: Boolean(mac ? e.metaKey : e.ctrlKey), range: Boolean(e.shiftKey) };
}

// ---- The Breakdown tab (BRK-11, BRK-18)

/**
 * Quick add's semicolon form (the older reference app's quick create): `Props;Lantern`
 * creates Lantern in Props without asking. Null when the text has no semicolon or a side
 * is empty.
 */
export function semicolonCreate(text) {
  const at = text.indexOf(';');
  if (at < 0) return null;
  const category = text.slice(0, at).trim();
  const name = text.slice(at + 1).trim();
  return category && name ? { category, name } : null;
}

/**
 * What Enter does in Quick add with no match picked (decisions.md, entries 156 and 157):
 * `{ kind: 'none' }` for empty text; `{ kind: 'create', category, name }` for the
 * semicolon form, made without asking; `{ kind: 'tag', element }` when an element
 * anywhere has the name, ignoring case, and is not on the sheet (the first in category
 * order, then list order); `{ kind: 'on', element }` when every such element is on the
 * sheet already; else `{ kind: 'ask', name }`, a new element whose category is asked for.
 * `elements` are { id, name, category }, `onSheet` a Set of ids, `categories` names in
 * category order.
 */
export function quickAddPlan(text, elements, onSheet, categories) {
  const t = String(text ?? '').trim();
  if (!t) return { kind: 'none' };
  const semi = semicolonCreate(t);
  if (semi) return { kind: 'create', ...semi };
  const low = t.toLowerCase();
  const rank = new Map((categories || []).map((c, i) => [c, i]));
  const last = rank.size;
  const same = (elements || [])
    .map((e, i) => [e, i])
    .filter(([e]) => String(e.name).trim().toLowerCase() === low)
    .sort((a, b) => (rank.get(a[0].category) ?? last) - (rank.get(b[0].category) ?? last) || a[1] - b[1])
    .map(([e]) => e);
  if (!same.length) return { kind: 'ask', name: t };
  const off = same.find((e) => !(onSheet && onSheet.has(e.id)));
  return off ? { kind: 'tag', element: off } : { kind: 'on', element: same[0] };
}

/**
 * The categories Quick add's picker offers for a new element, and which is chosen at
 * first: the New in category (`current`), so Return keeps it one keystroke. A name typed
 * in New in that is no category yet comes first; with none, the first category.
 */
export function categoryChoices(names, current) {
  const c = String(current ?? '').trim();
  const at = names.findIndex((n) => n.toLowerCase() === c.toLowerCase());
  if (at >= 0) return { choices: [...names], at };
  return c ? { choices: [c, ...names], at: 0 } : { choices: [...names], at: 0 };
}

/**
 * Whether a category's box is hidden: Hide empty is on, it has no element on the sheet,
 * and nothing is being typed in it (so its add field keeps focus).
 */
export function boxHidden(hideEmpty, count, typing) {
  return Boolean(hideEmpty && !count && !typing);
}

// ---- A sheet's one-value header fields (BRK-02, E-L12, decision 155)

/**
 * The values offered as a header field is typed in: those used on any sheet that start
 * with the text, then those holding it, ignoring case, at most `max`. A value spelled
 * exactly as typed is left out (there is nothing to pick), and so is an empty text.
 * Rust gives a committed value the existing spelling (`reports::header_spelling`).
 */
export function headerMatches(values, typed, max = 8) {
  const t = String(typed ?? '').trim();
  if (!t) return [];
  const low = t.toLowerCase();
  const starts = [];
  const holds = [];
  for (const v of values || []) {
    if (v === t) continue;
    const n = v.toLowerCase();
    if (n.startsWith(low)) starts.push(v);
    else if (n.includes(low)) holds.push(v);
  }
  return starts.concat(holds).slice(0, max);
}

// ---- The Elements table's sort (ELM-16)

/** Pages as eighths, for sorting: `1 3/8` is 11; anything else 0. */
export function eighthsOf(text) {
  const m = /^(?:(\d+))?\s*(?:(\d)\/8)?$/.exec(String(text || '').trim());
  return m ? Number(m[1] || 0) * 8 + Number(m[2] || 0) : 0;
}

/**
 * A click on a column heading: the first sorts up, the second down, the third goes back
 * to the engine's order (key null). Returns { key, down }.
 */
export function nextSort(key, down, clicked) {
  if (key !== clicked) return { key: clicked, down: false };
  return down ? { key: null, down: false } : { key, down: true };
}

/**
 * `rows` in the order the table shows: as given with no key, else by `row[key]` (pages
 * by their eighths), numbers by value and text naturally (2 before 10, any case), blanks
 * last either way, ties in the given order.
 */
export function sortRows(rows, key, down) {
  const list = rows.map((e, i) => ({ e, i }));
  if (!key) return list.map((x) => x.e);
  const value = (e) => (key === 'pages' ? eighthsOf(e.pages) : e[key]);
  const blank = (v) => v === null || v === undefined || v === '';
  list.sort((a, b) => {
    const [x, y] = [value(a.e), value(b.e)];
    if (blank(x) || blank(y)) return blank(x) === blank(y) ? a.i - b.i : blank(x) ? 1 : -1;
    let c = 0;
    if (typeof x === 'number' && typeof y === 'number') c = x - y;
    else c = String(x).localeCompare(String(y), undefined, { numeric: true, sensitivity: 'base' });
    return (down ? -c : c) || a.i - b.i;
  });
  return list.map((x) => x.e);
}

// ---- The Calendar tab (CAL-05, CAL-07)

/**
 * Go To's field: { date } to show, { error } when the text isn't a date, or {} when the
 * field is empty.
 */
export function goToDate(text) {
  const t = String(text ?? '').trim();
  if (!t) return {};
  const date = typedDate(t);
  return date ? { date } : { error: `"${t}" is not a date. Type it as 3/1/2027 or 2027-03-01.` };
}

/**
 * The Calendar's filter made to fit the view (behavior note 27, E-C11): `elements` the
 * view's list ({ id, category }, in category order), `filter` the checked categories
 * (names) and elements (ids), each empty for All. Returns the categories in use (those
 * with an element), the element groups the list shows (every category's under All, else
 * the checked ones'), and the filter without checks that are no longer there.
 */
export function calendarFilter(elements, filter) {
  const categories = [...new Set(elements.map((e) => e.category))];
  const cats = filter.categories.filter((c) => categories.includes(c));
  const groups = (cats.length ? categories.filter((c) => cats.includes(c)) : categories).map((category) => ({ category, elements: elements.filter((e) => e.category === category) }));
  const listed = new Set(groups.flatMap((g) => g.elements.map((e) => e.id)));
  return { categories, groups, filter: { categories: cats, elements: filter.elements.filter((id) => listed.has(id)) } };
}

/**
 * Whether the Calendar's filter shows a strip: `filter` as [`calendarFilter`] returns
 * it. Elements checked show the strips holding any of them; categories checked alone
 * show the strips holding any element of those categories; nothing checked shows all.
 * Day headers and events stay whatever the filter. `elements` is the view's list.
 */
export function stripShown(strip, filter, elements) {
  if (filter.elements.length) return strip.elements.some((e) => filter.elements.includes(e));
  if (!filter.categories.length) return true;
  const ids = new Set(elements.filter((e) => filter.categories.includes(e.category)).map((e) => e.id));
  return strip.elements.some((e) => ids.has(e));
}

/**
 * CAL-06 (behavior note 27, decisions.md, entry 160): what a drag on the calendar does
 * where it is let go. `drag` is { kind: 'strip' | 'event', id }; `cell` is the date
 * under the pointer, { drop, date } (`drop` is the shoot day's place on the board, null
 * on a date that is no shoot day), or null off the month. A strip goes to the end of
 * another shoot day's strips; an event moves to start on any date. Null does nothing.
 */
export function calendarDrop(drag, cell) {
  if (!drag || !cell) return null;
  if (drag.kind === 'strip') return cell.drop == null || cell.drop === '' ? null : { strip: drag.id, at: Number(cell.drop) };
  if (drag.kind === 'event') return cell.date ? { event: drag.id, date: cell.date } : null;
  return null;
}

/** Whether a pointer has moved far enough from where it went down to start a drag. */
export function dragStarted(x0, y0, x, y, px = 5) {
  return Math.abs(x - x0) + Math.abs(y - y0) >= px;
}

// ---- New banners (BRD-10, behavior note 27, E-B12)

/**
 * Where Insert Banner Above or Below puts a banner: `rows` the selected rows in board
 * order ({ list, index }), `shown` the sub-board shown ({ id, len }). Both go by the
 * bottommost selected strip, as the reference app's New Banner does: Above goes just
 * above it, Below just below. With nothing selected, Above goes to the start of the
 * shown sub-board and Below to its end. Returns { sub, index }, or { boneyard: true }
 * when that strip is in the Boneyard.
 */
export function bannerPlace(rows, where, shown) {
  if (!rows.length) return { sub: shown.id, index: where === 'above' ? 0 : shown.len };
  const row = rows[rows.length - 1];
  if (row.list === null) return { boneyard: true };
  return { sub: row.list, index: where === 'above' ? row.index : row.index + 1 };
}

// ---- The board's strip menu and Delete (KEY-03, KEY-04)

/**
 * The strip menu's items for a selection: `rows` the selected rows ({ kind, list, id }),
 * `focus` the focused one, `clip` the app's clipboard kind, `subBoards` the board's
 * sub-boards ({ id, name }). Each item is { id, label, disabled } or 'sep'; Move To and
 * the two Insert items have `items`. The page adds the keys and what each one runs.
 */
export function stripMenu({ rows, focus, clip, subBoards }) {
  const scenes = rows.filter((r) => r.kind === 'scene');
  const lists = new Set(rows.map((r) => r.list));
  const onYard = !focus || focus.list === null;
  const none = !rows.length;
  const move = subBoards
    .filter((sb) => !(lists.size === 1 && lists.has(sb.id)))
    .map((sb) => ({ id: `move:${sb.id}`, label: sb.name, disabled: none }));
  move.push({ id: 'move:yard', label: 'Boneyard', disabled: none || rows.some((r) => r.kind === 'banner') || (lists.size === 1 && lists.has(null)) });
  return [
    { id: 'open', label: focus && focus.kind === 'banner' ? 'Edit Banner…' : 'Edit Sheet', disabled: !focus || focus.kind === 'break' },
    'sep',
    { id: 'cut', label: 'Cut', disabled: none },
    { id: 'copy', label: 'Copy', disabled: none },
    { id: 'paste', label: 'Paste', disabled: clip !== 'strips' },
    { id: 'delete', label: 'Delete…', disabled: none },
    'sep',
    { id: 'merge', label: 'Merge Strips…', disabled: scenes.length < 2 },
    { id: 'duplicate', label: 'Duplicate Strip', disabled: !scenes.length },
    { id: 'continue', label: 'Continue on Next Day', disabled: !focus || focus.kind !== 'scene' || focus.list === null },
    { id: 'shot', label: 'Shot…', disabled: !focus || focus.kind !== 'scene' || focus.list === null },
    { id: 'move', label: 'Move To', items: move, disabled: none },
    'sep',
    { id: 'break', label: 'Insert Day Break', disabled: onYard, items: [{ id: 'break_above', label: 'Above' }, { id: 'break_below', label: 'Below' }] },
    { id: 'banner', label: 'Insert Banner', disabled: onYard, items: [{ id: 'banner_above', label: 'Above…' }, { id: 'banner_below', label: 'Below…' }] },
  ];
}

/**
 * What Delete asks before deleting `rows` ({ kind, strip: { parts, part, scene } }):
 * null when they are day breaks only (they go at once; Undo brings them back), else
 * { title, message } (decisions.md, entries 54 and 76).
 */
export function deleteQuestion(rows) {
  const pieces = rows.filter((r) => r.kind === 'scene' && (r.strip.parts || 0) > 1);
  const scenes = rows.filter((r) => r.kind === 'scene' && !pieces.includes(r));
  const banners = rows.filter((r) => r.kind === 'banner').length;
  const breaks = rows.length - scenes.length - pieces.length - banners;
  if (!scenes.length && !banners && !pieces.length) return null;
  const parts = [];
  if (pieces.length === 1) parts.push(`part ${pieces[0].strip.part} of scene ${pieces[0].strip.scene || ''}`.trim());
  else if (pieces.length) parts.push(`${pieces.length} parts of scenes spanning days`);
  if (scenes.length === 1) parts.push(scenes[0].strip.scene ? `scene ${scenes[0].strip.scene}` : '1 scene');
  else if (scenes.length) parts.push(`${scenes.length} scenes`);
  if (banners) parts.push(banners === 1 ? '1 banner' : `${banners} banners`);
  if (breaks) parts.push(breaks === 1 ? '1 day break' : `${breaks} day breaks`);
  const what = parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}` : parts[0];
  const sheets = scenes.length ? ` ${scenes.length === 1 ? 'Its sheet leaves' : 'Their sheets leave'} every board; the elements stay in the schedule.` : '';
  return {
    title: rows.length === 1 ? (scenes.length ? 'Delete this scene?' : pieces.length ? 'Delete this part?' : 'Delete this banner?') : 'Delete these strips?',
    message: `Delete ${what}?${sheets} Undo brings ${rows.length === 1 ? 'it' : 'them'} back.`,
  };
}

// ---- Help > Keyboard Shortcuts (KEY-05)

/**
 * The sheet's groups narrowed by `text`: a row shows when its name, its keys or its
 * group's title holds the text (any case); empty groups are left out.
 */
export function filterShortcuts(groups, text) {
  const t = String(text ?? '').trim().toLowerCase();
  return groups
    .map((g) => ({ ...g, rows: g.rows.filter((r) => !t || r.label.toLowerCase().includes(t) || r.keys.toLowerCase().includes(t) || g.title.toLowerCase().includes(t)) }))
    .filter((g) => g.rows.length);
}

// ---- Help > User Guide (guide.js; decision 199)

/**
 * The guide's Markdown (docs/GUIDE.md) as blocks for the page to draw with text nodes:
 * `{ kind: 'h1' | 'h2' | 'h3' | 'p' | 'pre', text }` and `{ kind: 'ul', items: [text] }`.
 * Only what the guide and the third-party notices use: headings, paragraphs, `- ` lists
 * (a wrapped line joining the one before) and fenced blocks kept line for line (the
 * license texts, decision 219). Nothing in it is ever HTML.
 */
export function guideBlocks(text) {
  const out = [];
  let para = null;
  let list = null;
  let fence = null;
  const close = () => {
    para = null;
    list = null;
  };
  for (const raw of String(text ?? '').split(/\r?\n/)) {
    if (fence) {
      if (/^```\s*$/.test(raw)) fence = null;
      else fence.text += (fence.text ? '\n' : '') + raw.trimEnd();
      continue;
    }
    if (/^```/.test(raw)) {
      close();
      out.push((fence = { kind: 'pre', text: '' }));
      continue;
    }
    const line = raw.trimEnd();
    const heading = /^(#{1,3}) (.*)$/.exec(line);
    if (!line.trim()) {
      close();
    } else if (heading) {
      close();
      out.push({ kind: `h${heading[1].length}`, text: heading[2].trim() });
    } else if (/^- /.test(line)) {
      para = null;
      if (!list) out.push((list = { kind: 'ul', items: [] }));
      list.items.push(line.slice(2).trim());
    } else if (list && /^\s/.test(line)) {
      list.items[list.items.length - 1] += ` ${line.trim()}`;
    } else if (para) {
      para.text += ` ${line.trim()}`;
    } else {
      list = null;
      out.push((para = { kind: 'p', text: line.trim() }));
    }
  }
  return out;
}

/**
 * A guide line as runs of text: `{ text, bold, code, em }`. `**bold**`, `*em*` and
 * `` `code` `` as Markdown writes them; a link `[words](target)` keeps its words only (the
 * guide opens nothing). An unclosed mark stays as typed.
 */
export function guideRuns(text) {
  const s = String(text ?? '').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1');
  const out = [];
  const push = (t, mark = {}) => t && out.push({ text: t, bold: !!mark.bold, code: !!mark.code, em: !!mark.em });
  const re = /`([^`]+)`|\*\*([^*]+)\*\*|\*([^*\s][^*]*)\*/g;
  let at = 0;
  for (let m; (m = re.exec(s)); ) {
    push(s.slice(at, m.index));
    if (m[1] !== undefined) push(m[1], { code: true });
    else if (m[2] !== undefined) push(m[2], { bold: true });
    else push(m[3], { em: true });
    at = re.lastIndex;
  }
  push(s.slice(at));
  return out;
}

// ---- Find and Replace's advanced rows (find.js; decision 165)

/** How the rows join: every row (AND) or any (OR), in words short enough for the panel. */
export const FIND_JOINS = [
  ['and', 'All rows'],
  ['or', 'Any row'],
];

/** How a row compares (the engine's find::Op). */
export const FIND_OPS = [
  ['contains', 'contains'],
  ['not_contains', 'does not contain'],
  ['is', 'is'],
  ['is_not', 'is not'],
  ['starts_with', 'starts with'],
  ['empty', 'is empty'],
  ['not_empty', 'is not empty'],
  ['greater', 'greater than'],
  ['less', 'less than'],
];

/**
 * The replace box's hint, short enough for the panel beside the Replace button, and its
 * tooltip with the whole meaning (decision 169).
 */
export const FIND_REPLACE_HINT = {
  placeholder: 'Replace with',
  title: 'The text that replaces what was found in the checked results. Leave it empty to remove that text.',
};

// ---- Dialogs and text fields (modal.js, dom.js; decision 164)

/**
 * Whether a key closes the open dialog: Escape, wherever focus is (in the dialog, or on
 * the page after a click on plain text in it), unless something in the dialog took it
 * first (a list, a menu) or an input method is composing.
 */
export function dialogEscape(e) {
  return Boolean(e) && e.key === 'Escape' && !e.defaultPrevented && !e.isComposing;
}

/**
 * Whether a key closes an open drop-down list (the calendar's check box lists, decisions.md,
 * entry 168): Escape while it is `open`, taken before a dialog's (dialogEscape), so the
 * one key closes the list and nothing else.
 */
export function dropEscape(e, open) {
  return Boolean(open) && dialogEscape(e);
}

/**
 * The attributes a new field gets unless it sets them: every text field, no saved
 * values offered (WebKit's own AutoFill list); a search or filter box, no spelling,
 * correction or capitals either. Other inputs (check boxes, dates, colors) get none.
 */
export function quietField(tag, attrs) {
  const a = attrs || {};
  const t = String(tag).toLowerCase();
  const type = String(a.type ?? 'text').toLowerCase();
  const typed = t === 'textarea' || (t === 'input' && ['text', 'search', 'email', 'url', 'tel', 'number', 'password'].includes(type));
  if (!typed) return {};
  const add = { autocomplete: 'off' };
  if (t === 'input' && type === 'search') Object.assign(add, { spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' });
  for (const k of Object.keys(add)) if (a[k] !== undefined && a[k] !== null) delete add[k];
  return add;
}

// ---- The split screen (SPL-01 to SPL-08)

/** The panes beside the board, in the picker's order. */
export const PANE_KINDS = ['breakdown', 'board', 'calendar', 'images'];

/**
 * What a View item asks the split for. `item`: a pane kind or 'single'; `tab` the tab
 * shown; `besideBoard` what is beside the board now. The item of the pane already beside
 * the board closes it (Single View); any other opens it, showing the Board tab first.
 * Returns { kind, showBoard }.
 */
export function splitToggle(item, tab, besideBoard) {
  if (item === 'single' || !PANE_KINDS.includes(item)) return { kind: null, showBoard: false };
  if (tab === 'board' && besideBoard === item) return { kind: null, showBoard: false };
  return { kind: item, showBoard: tab !== 'board' };
}

/** View > Images Beside Breakdown: on unless the images are beside the Breakdown tab now. */
export function breakdownImagesToggle(tab, shown) {
  const on = !(tab === 'breakdown' && shown === 'images');
  return { on, showBreakdown: on && tab !== 'breakdown' };
}

/** The pane a tab shows beside it: the board's pick on the Board tab, the images on the Breakdown tab when on, else none. */
export function paneFor(tab, kind, breakdownImages) {
  if (tab === 'board') return PANE_KINDS.includes(kind) ? kind : null;
  return tab === 'breakdown' && breakdownImages ? 'images' : null;
}

/**
 * The pane's width in pixels: at least `min`, and at most what leaves the board `board`
 * pixels of `room` (the row's width less the Find panel and the bar).
 */
export function paneWidth(w, room, min = 280, board = 320) {
  return Math.max(min, Math.min(Math.round(w), Math.max(min, room - board)));
}

/** The split's remembered state as stored, checked: { kind, width, bdImages }. */
export function readSplit(raw, width = 520) {
  let v = null;
  try {
    v = JSON.parse(raw || 'null');
  } catch {
    v = null;
  }
  if (!v || typeof v !== 'object') return { kind: null, width, bdImages: false };
  return { kind: PANE_KINDS.includes(v.kind) ? v.kind : null, width: Number.isFinite(v.width) ? Math.round(v.width) : width, bdImages: v.bdImages === true };
}

// ---- The DOOD tab (DOD-10, DOD-13, DOD-19)

/**
 * The DOOD tab's sort keys after a move: `from` and `to` the tab and sub-board before and
 * after ({ tab, sub }). Leaving the tab or showing another sub-board clears the sort, as
 * the reference app does (behavior notes 19 and 27, E-D25); anything else keeps it.
 */
export function doodSortAfter(keys, from, to) {
  const left = from.tab === 'dood' && to.tab !== 'dood';
  return left || from.sub !== to.sub ? [] : keys;
}

/** Whether a date ({ weekday }) is a Saturday or Sunday. */
export function isWeekend(d) {
  return d.weekday === 'Sat' || d.weekday === 'Sun';
}

/** A stored switch's value: on unless stored as `0`. */
export function weekendsOn(stored) {
  return stored !== '0';
}

/**
 * The DOOD's Saturdays and Sundays switches as stored (DOD-19, decision 158): each its own
 * key, and while one has never been set, the old single Weekends box's value (decision
 * 121) stands for it. Returns { sat, sun }.
 */
export function weekendDays(old, sat, sun) {
  return { sat: weekendsOn(sat ?? old), sun: weekendsOn(sun ?? old) };
}

/** Whether date `d` shows with weekend switches `days` ({ sat, sun }, or true or false for both). */
export function dayShown(d, days) {
  if (typeof days === 'boolean') return days || !isWeekend(d);
  if (d.weekday === 'Sat') return days.sat !== false;
  if (d.weekday === 'Sun') return days.sun !== false;
  return true;
}

/**
 * The DOOD report's pages (DOD-19, decisions 141 and 158): the dates' indexes, page by
 * page, in whole weeks from the week's first day (`sun` or `mon`), as many weeks as fit in
 * `per` columns (at least one). `weekends` is the DOOD tab's switches, { sat, sun } (or
 * true or false for both): a day switched off is left out, as the tab hides it, so with
 * both off a week is five columns and Friday is followed by Monday.
 */
export function doodReportPages(dates, per, weekends, weekStart) {
  const first = weekStart === 'mon' ? 'Mon' : 'Sun';
  const weeks = [];
  let fresh = true;
  dates.forEach((d, i) => {
    if (i === 0 || d.weekday === first) fresh = true;
    if (!dayShown(d, weekends)) return;
    if (fresh) weeks.push([]);
    fresh = false;
    weeks[weeks.length - 1].push(i);
  });
  const week = 5 + (dayShown({ weekday: 'Sat' }, weekends) ? 1 : 0) + (dayShown({ weekday: 'Sun' }, weekends) ? 1 : 0);
  const fit = Math.max(1, Math.floor(per / week));
  const pages = [];
  for (let w = 0; w < weeks.length; w += fit) pages.push(weeks.slice(w, w + fit).flat());
  return pages;
}

/**
 * The DOOD grid on its side (Vertical): a heading per element (its board ID, else its
 * name), a row per date with its shoot day and each element's code, then the work, hold
 * and total rows.
 */
export function verticalGrid(dto) {
  return {
    heads: dto.rows.map((r) => r.board_id || r.name),
    rows: dto.dates.map((d, i) => ({ date: d.date, weekday: d.weekday, kind: d.kind, weekend: isWeekend(d), day: d.shoot_day ? `D${d.shoot_day}` : '', cells: dto.rows.map((r) => r.cells[i]) })),
    sums: [
      ['Work', 'work'],
      ['Hold', 'hold'],
      ['Total', 'total'],
    ].map(([label, key]) => ({ label, cells: dto.rows.map((r) => String(r[key])) })),
  };
}

// ---- The strip designer's element count (strips.js; Rust counts, src/view.rs)

/**
 * BRD-18 (behavior note 27, decisions.md, entry 147): the categories an element count's
 * picker lists for a search, as the reference app's: categories only, by a name holding
 * the text ignoring case (an element's name finds nothing), in their order.
 */
export function countCategories(categories, text) {
  const t = String(text ?? '').trim().toLowerCase();
  return (categories || []).filter((c) => !t || String(c.name).toLowerCase().includes(t));
}

/** What the element count counts, in words: `Counts Props`, or every element with none. */
export function countSay(categories, id) {
  const c = id == null ? null : (categories || []).find((x) => x.id === id);
  return c ? `Counts ${c.name}` : 'Counts every element';
}

/**
 * What a category's elements field lists, in words (decisions.md, entry 194): `Lists
 * Props by name`, `Lists Props by board ID`, or a nudge to pick one when none is (or the
 * one picked is gone): the field is empty then.
 */
export function listSay(categories, id, boardIds) {
  const c = id == null ? null : (categories || []).find((x) => x.id === id);
  if (!c) return 'Pick a category: until then the field is empty';
  return `Lists ${c.name} by ${boardIds ? 'board ID' : 'name'}`;
}

// ---- What heads a day on the board (DBK-03, behavior note 27, decision 148)

/**
 * The line that heads each day on the board and its printout: `shoot` when the board's
 * day look has the shoot day strip (it replaces our day heading, which says less), else
 * `day` when Show on Board has day headings, else null. Either goes above the day's
 * first strip, so just under the day before's End of day line.
 */
export function dayLead(look, show) {
  if (look && look.shoot_day_strip) return 'shoot';
  return show && show.heads ? 'day' : null;
}

// ---- The marks on a strip (behavior note 27, live check; decision 149)

/**
 * The conflict and red flag marks' text on a strip. Each asks for the plain text form
 * (U+FE0E), so the warning sign is not drawn as a yellow picture that vanishes on a
 * yellow strip; styles.css puts each on a badge of fixed colors.
 */
export const STRIP_MARK = Object.freeze({ conflict: '⚠︎', flag: '⚑︎' });

// ---- The Boards and Sub-boards dialog (live check, decision 151)

/**
 * The name a name field sends, or null: the typed name trimmed, unless it is the name the
 * field was drawn with or the one it already sent (Return sends it, and the change that
 * follows on leaving the field must not send it again).
 */
export function nameToSend(was, sent, typed) {
  const t = String(typed ?? '').trim();
  if (t === String(was ?? '').trim() || t === sent) return null;
  return t;
}

/**
 * Holds work while the pointer is down: `run(f)` runs `f` at once, or keeps it (the last
 * one only) until `release()`. A dialog redraws through it, so a redraw never replaces
 * the button between the press and the click.
 */
export function pressGate() {
  let down = false;
  let waiting = null;
  return {
    press() {
      down = true;
    },
    release() {
      down = false;
      const f = waiting;
      waiting = null;
      if (f) f();
    },
    run(f) {
      if (down) waiting = f;
      else f();
    },
  };
}

// ---- Banner colors (live check against the reference app's palette, decision 154)

/**
 * The banner palette's colors by value, after the nine named ones (model::BannerColor):
 * ten hues (red, orange, yellow, lime, green, teal, blue, indigo, purple, gray), each pale,
 * light, mid and strong. With the names, 49 swatches.
 */
export const BANNER_PALETTE = Object.freeze([
  '#ffd6d6', '#ffe4c7', '#fff6bf', '#e6f5c4', '#d4f0d4', '#ccefea', '#d6e6ff', '#dcdcfb', '#ecd9fb', '#eceef1',
  '#ffadad', '#ffc98f', '#ffec80', '#cdea8a', '#a9dfa9', '#99ded4', '#a9c9ff', '#b9b9f5', '#d6b3f5', '#d3d7dd',
  '#f26b6b', '#f5a14a', '#f2d43d', '#9fcc3b', '#5fbf5f', '#3fb8a8', '#5b93ec', '#7b7be0', '#ad74e0', '#9aa1ab',
  '#c62828', '#d9730d', '#b8930b', '#5f8a12', '#2e7d32', '#00796b', '#1f5fbf', '#3f3fb0', '#7b3fb0', '#4a5059',
]);

const BANNER_HEX = /^#[0-9a-f]{6}$/;

/** A banner color by value as Rust sends it (lowercase `#rrggbb`), or null. */
export function bannerHex(c) {
  return typeof c === 'string' && BANNER_HEX.test(c) ? c : null;
}

/** Black or white text on `hex`, as Rust's `Rgb::ink`: luma 299, 587, 114, black from half way. */
export function inkFor(hex) {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 299 * r + 587 * g + 114 * b >= 128 * 1000 ? '#000000' : '#ffffff';
}

/**
 * The stylesheet for banners colored by value: class `bh-rrggbb` on the board's row, a
 * printed row's cells and a palette swatch. Only checked colors reach the CSS.
 */
export function bannerHexRules(colors) {
  let css = '';
  for (const c of colors) {
    const hex = bannerHex(c);
    if (!hex) continue;
    const k = `bh-${hex.slice(1)}`;
    const ink = inkFor(hex);
    css += `.row.banner.${k}, .bn-swatch.${k} { background-color: ${hex}; color: ${ink}; }\n`;
    css += `.method-report .rpt-table tr.bp.banner.${k} td, .method-report .rpt-table tr.rpt-banner.${k} td { background: ${hex}; color: ${ink}; }\n`;
  }
  return css;
}

// ---- The sheet shown after an undo (live check, decision 152)

/**
 * The sheet the Breakdown tab shows after a change: `current`, unless it is gone and New
 * Sheet or Duplicate Sheet made it, when it is the sheet shown before that (and so on
 * back, for several undone at once). `made` is [{ made, back }], oldest first; `exists(id)`
 * says whether a sheet is still in the schedule.
 */
export function sheetBack(current, made, exists) {
  let s = current;
  for (let i = made.length - 1; i >= 0; i--) {
    if (made[i].made === s && !exists(s) && made[i].back !== null && made[i].back !== undefined) s = made[i].back;
  }
  return s;
}

// ---- Find marks on a board (decisions.md, entries 130 and 184)

/**
 * The strips of board `boardId` that the Find panel's `hits` name: { sheets, strips }
 * (sheet ids for scenes, on every board; banner strip ids, on their own board only), or
 * null for none. The Board tab and the board beside it each ask for their own board.
 */
export function foundOn(hits, boardId) {
  const list = Array.isArray(hits) ? hits : [];
  const sheets = new Set(list.filter((h) => h.kind === 'sheet').map((h) => h.sheet));
  const strips = new Set(list.filter((h) => h.kind === 'banner' && h.board === boardId).map((h) => h.strip));
  return sheets.size || strips.size ? { sheets, strips } : null;
}

// ---- Strip designer line styles (strips.js, decisions.md, entry 185)

/**
 * The classes a strip layout field's line styles add, the same on the board and on
 * paper: `rl-after` for a line down its right edge (between it and the next column),
 * `rl-under` for a rule along its foot. Each with a leading space; empty for none.
 */
export function ruleClass(c) {
  return `${c && c.rule_after ? ' rl-after' : ''}${c && c.rule_under ? ' rl-under' : ''}`;
}

// ---- Outside changes and Revert to Backup (outside.js; src/outside.rs decides)

const OUTSIDE_LABELS = { merge: 'Merge', keep_mine: 'Keep Mine', load_theirs: 'Load Theirs' };

/**
 * The outside-change dialog's buttons, left to right: Later, then the other choices in
 * Rust's order, then the default (primary, on the right, as every dialog's OK).
 */
export function outsideButtons(offer) {
  const label = (c) => OUTSIDE_LABELS[c] || c;
  const rest = offer.choices.filter((c) => c !== offer.default);
  return [
    { choice: 'later', label: 'Later', primary: false },
    ...rest.map((c) => ({ choice: c, label: label(c), primary: false })),
    { choice: offer.default, label: label(offer.default), primary: true },
  ];
}

/** What the outside-change dialog says, a line each: what happened, then each choice. */
export function outsideMessage(dto) {
  const when = dto.changed_at ? ` (last changed ${dto.changed_at})` : '';
  const head = dto.dirty
    ? `${dto.name} was changed outside Method Schedule${when}, and this window has unsaved changes too.`
    : `${dto.name} was changed outside Method Schedule${when}. This window has no unsaved changes.`;
  const says = {
    merge: 'Merge brings the changes made outside into yours, against the version both started from.',
    keep_mine: "Keep Mine saves this window's version over it; the version on disk goes into the backups.",
    load_theirs: dto.dirty ? 'Load Theirs opens the version on disk here and drops your changes (Undo brings them back).' : 'Load Theirs opens the version on disk here.',
  };
  const lines = [head];
  if (dto.problem) lines.push(dto.problem);
  for (const c of dto.choices) if (says[c]) lines.push(says[c]);
  lines.push('Later leaves the file alone; autosave into it stays paused until this is settled.');
  return lines;
}

/** The merge dialog's base list value as the base Rust is asked for, and back. */
export function baseChoiceOf(value) {
  if (value === 'auto' || value === 'disk' || value === 'last_read') return { kind: value };
  // A schedule file picked as the base (M4), by its token.
  const file = /^file:(\d+)$/.exec(value);
  if (file) return { kind: 'file', token: Number(file[1]) };
  return { kind: 'backup', name: value };
}

export function baseValueOf(base) {
  if (base.kind === 'file') return `file:${base.token}`;
  return base.kind === 'backup' ? base.name : base.kind;
}

/** One backup in Revert to Backup: its date, and how far it is from the schedule now. */
export function backupLine(b) {
  let from;
  if (b.changes === null || b.changes === undefined) from = "can't be read";
  else if (b.changes === 0) from = 'the same as now';
  else from = `${b.changes} ${b.changes === 1 ? 'change' : 'changes'} from now`;
  return `${b.label}, ${from}`;
}

// ---- The breakdown designer's grid (breakdown.js, decisions.md, entries 101 and 186)

/** The rows a sheet's shown fields fall in when no lengths are set: six, five, the rest. */
export const SHEET_ROWS = [6, 5];

/**
 * The shown fields `shown` (keys in order) in rows: `lengths` how many each row holds
 * (empty: six, five and the rest); fields past them make one more row. No empty rows.
 */
export function sheetRows(shown, lengths) {
  const sizes = Array.isArray(lengths) && lengths.length ? lengths : SHEET_ROWS;
  const rows = [];
  let at = 0;
  for (const n of sizes) {
    if (at >= shown.length) break;
    rows.push(shown.slice(at, at + Math.max(1, Number(n) || 1)));
    at += rows[rows.length - 1].length;
  }
  if (at < shown.length) rows.push(shown.slice(at));
  return rows;
}

/**
 * The rows after dragging field `key` to row `toRow` before its `toIndex`th field
 * (`toRow` equal to the number of rows starts a new row at the end). A row left empty
 * goes; nothing changes for a key not in the rows.
 */
export function gridMove(rows, key, toRow, toIndex) {
  const from = rows.findIndex((r) => r.includes(key));
  if (from < 0) return rows.map((r) => [...r]);
  const out = rows.map((r) => [...r]);
  const at = out[from].indexOf(key);
  out[from].splice(at, 1);
  let index = toIndex;
  if (toRow === from && at < toIndex) index -= 1;
  if (toRow >= out.length) out.push([key]);
  else out[toRow].splice(Math.max(0, Math.min(index, out[toRow].length)), 0, key);
  return out.filter((r) => r.length);
}

/**
 * The design the rows make: { keys, rows }. `keys` every field in order: the shown ones
 * as the rows have them, each hidden one (in `order`, the order before) put back at its
 * place in that order. `rows` each row's length.
 */
export function gridDesign(rows, order, hidden) {
  const keys = rows.flat();
  order.forEach((k, i) => {
    if (hidden.has(k) && !keys.includes(k)) keys.splice(Math.min(i, keys.length), 0, k);
  });
  return { keys, rows: rows.map((r) => r.length) };
}

// ---- Banner font families (board.js, boards.js, decisions.md, entry 187)

/** The font families a banner takes (model::BannerFont): [name, what the dialog says]. */
export const BANNER_FONTS = [
  ['usual', "The board's"],
  ['serif', 'Serif'],
  ['sans', 'Sans serif'],
  ['mono', 'Monospaced'],
];

/** A banner's font family as a class (` bf-serif`), from the fixed names only; empty for the usual. */
export function bannerFontClass(font) {
  return font && font !== 'usual' && BANNER_FONTS.some((f) => f[0] === font) ? ` bf-${font}` : '';
}

// ---- Fountain lines per page (settings.js, decisions.md, entry 195)

/** The fewest and most lines a Fountain page may be set to, and the default. */
export const FOUNTAIN_LINES = { min: 40, max: 70, usual: 56 };

/** Typed lines per page as a whole number in range, or null. */
export function fountainLines(text) {
  const t = String(text ?? '').trim();
  if (!/^\d{1,3}$/.test(t)) return null;
  const n = Number(t);
  return n >= FOUNTAIN_LINES.min && n <= FOUNTAIN_LINES.max ? n : null;
}

// ---------------------------------------------------------------------------
// The printed call sheet's words (callsheet.js, decision 207)

const CS_WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const CS_MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "Tuesday, Oct 20, 2026" from `YYYY-MM-DD`; null for no date. */
export function callSheetDate(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso || '');
  if (!m) return null;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return `${CS_WEEKDAYS[d.getUTCDay()]}, ${CS_MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}, ${d.getUTCFullYear()}`;
}

/** Minutes as "8:40". */
export function lengthText(min) {
  const n = Math.max(0, Math.round(Number(min) || 0));
  return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`;
}

/** A scene's clock times, "07:20–08:50", or one end, or nothing without a call time. */
export function spanText(start, end) {
  if (start && end) return `${start}–${end}`;
  return start || end || '';
}

/** Sunrise and sunset as the sheet says them. */
export function sunText(sun) {
  if (!sun) return '';
  if (sun.state === 'times') return `${sun.sunrise} · ${sun.sunset}`;
  if (sun.state === 'always_up') return 'Sun up all day';
  if (sun.state === 'always_down') return 'Sun down all day';
  return '';
}

// ---------------------------------------------------------------------------
// Looks (decision 208)

/** The app's looks, the default first: [value, name in Settings, what it is]. */
export const THEMES = [
  ['paper', 'Paper', 'Warm paper and ink; light or dark with the system'],
  ['graphite', 'Graphite', 'Always dark; strips marked by a colored edge'],
  ['ledger', 'Ledger', 'Always light; full strip colors and ruled columns'],
];

/** The data-theme attribute for a look: null for the default (Paper) or one the app lacks. */
export function themeAttr(theme) {
  return THEMES.some(([v]) => v === theme) && theme !== THEMES[0][0] ? theme : null;
}

// ---------------------------------------------------------------------------
// Updates (updates.js, decision 213)

const UPDATE_SAID = {
  current: 'Method Schedule is up to date.',
  failed: "Couldn't check for updates. Try again later.",
  unavailable: "This copy doesn't update itself yet: new versions are at methodschedule.com.",
  'install-failed': "The update couldn't be installed. Choose Restart to Update to try again.",
};

/**
 * What the status line says about an update, or null: a one-time answer, the progress of a
 * check the user asked for, or installing. An automatic check says nothing.
 */
export function updateMessage(s) {
  if (!s) return null;
  if (s.said && UPDATE_SAID[s.said]) return { text: UPDATE_SAID[s.said], error: s.said === 'failed' || s.said === 'install-failed' };
  if (s.phase === 'installing') return { text: 'Installing the update…', error: false };
  if (!s.manual) return null;
  if (s.phase === 'checking') return { text: 'Checking for updates…', error: false };
  if (s.phase === 'downloading') return { text: `Downloading Method Schedule ${s.version}…`, error: false };
  return null;
}

