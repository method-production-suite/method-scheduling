// A stand-in for the app's Rust commands: a small in-memory board with the same view
// shape src/view.rs sends, so the page's drawing, keys and command arguments can be
// checked in a browser. The real rules live in Rust; this only mimics them roughly.
// Every call is logged in window.__calls for checks.
//
// Boards: the board shown keeps its strips in model.subs and model.boneyard; the others
// wait in model.store by id, and model.boards lists every board's id and name in order.
// Per-sub-board commands throw when the page names a sub-board that isn't on the board
// shown, so a stale id shows up in checks.

// Dates as typed (src/calendar.rs, typed_date): 2027-03-01, 3/1/2027 or 03/01/2027.
import { typedDate } from '../ui/js/dates.js';
import { planFakes } from './fake-plan.js';

const typedDateFake = (v) => {
  const t = String(v ?? '').trim();
  if (!t) return '';
  let m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(t);
  let y;
  let mo;
  let d;
  if (m) [y, mo, d] = [m[1], m[2], m[3]].map(Number);
  else {
    m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(t);
    if (!m) return null;
    [mo, d, y] = [m[1], m[2], m[3]].map(Number);
  }
  const dt = new Date(Date.UTC(y, mo - 1, d));
  if (dt.getUTCMonth() !== mo - 1 || dt.getUTCDate() !== d) return null;
  return dt.toISOString().slice(0, 10);
};
// The event kinds and colors (src/model.rs, EventKind::keys and EVENT_COLORS; decision 109).
const FAKE_EVENT_KINDS = [
  ['travel', 'Travel'], ['fitting', 'Fitting'], ['rehearsal', 'Rehearsal'], ['unavailable', 'Unavailable'], ['camera_test', 'Camera test'],
  ['post_production', 'Post-production'], ['visual_effects', 'Visual effects'], ['dubbing', 'Dubbing'], ['sync_sound', 'Sync sound'], ['music_recording', 'Music recording'],
  ['picture_editing', 'Picture editing'], ['sound_editing', 'Sound editing'], ['sound_mixing', 'Sound mixing'], ['titles_graphics', 'Titles and graphics'], ['trailer', 'Trailer'], ['color_correction', 'Color correction'],
  ['other', 'Other'],
];
/** As Event::title: the event's name, or its kind in words when it has none. */
const eventTitle = (ev) => (ev.name || '').trim() || (ev.kind === 'other' && (ev.label || '').trim()) || (FAKE_EVENT_KINDS.find(([k]) => k === ev.kind) || [null, 'Event'])[1];
const FAKE_EVENT_COLORS = [['red', 'Red'], ['orange', 'Orange'], ['yellow', 'Yellow'], ['green', 'Green'], ['teal', 'Teal'], ['blue', 'Blue'], ['indigo', 'Indigo'], ['purple', 'Purple'], ['pink', 'Pink'], ['brown', 'Brown']];
const notADate = (what, v) => `${what} ${JSON.stringify(String(v).trim())} is not a date. Type it as 3/1/2027 or 2027-03-01.`;

const sheets = [
  ['1', 'INT', 'NET LOFT', 'NIGHT', 2, ['KIT', 'TOVE']],
  ['2', 'EXT', 'BOATYARD', 'NIGHT', 1, ['KIT']],
  ['3', 'EXT', 'HARBOR WALL', 'DAY', 2, ['HARBORMASTER LUND', 'KIT']],
  ['3A', 'INT/EXT', "KIT'S VAN", 'DAY', 1, []],
  ['4', 'INT', 'NET LOFT', 'DAY', 1, ['KIT']],
];

let nextId = 100;
const castIds = {};
const elements = [];
const castId = (name) => {
  if (!castIds[name]) {
    castIds[name] = nextId++;
    elements.push({ id: castIds[name], name, category: 'Cast', board_id: '', locked: false, holds: false, drops: false, gap: 1, links: [], notes: '', props: [], images: [] });
  }
  return castIds[name];
};
// A scene strip carries its sheet: `sheet` is the sheet id, `f` the other header fields as
// the page shows them, `els` the element ids on it.
const scene = ([n, ie, set, tod, eighths, cast]) => ({ id: nextId++, sheet: nextId++, kind: 'scene', n, ie, set, tod, eighths, els: cast.map(castId), f: {} });
const brk = () => ({ id: nextId++, kind: 'break' });

let model = {
  title: 'Harbor Light',
  subs: [{ id: 2, name: 'Main', start: null, strips: [scene(sheets[0]), scene(sheets[1]), brk(), scene(sheets[2]), brk(), scene(sheets[3]), brk()], cal: { weekly_off: ['sat', 'sun'], marks: {}, events: [] } }],
  boneyard: [scene(sheets[4])],
  boards: [{ id: 1, name: 'Board 1' }],
  shown: 1,
  store: {},
  // Categories by name (elements point at them by name); the cast category is built in
  // (`cast`), whatever it is called.
  // After the cast, our own default list (model::DEFAULT_CATEGORIES), as a new schedule has.
  categories: [
    { id: 1, name: 'Cast', cast: true, auto: false, holds: false, drops: false, gap: 1, exclude: false, daily: false, props: ['Full name', 'Address', 'Phone', 'Agent', 'Agent phone', 'Rate', 'Rate per', 'Deal notes'], multi: ['Address', 'Deal notes'] },
    ...[
      ['Background', false, true],
      ['Stand-ins', false, true],
      ['Stunts', true, false],
      ['Hand props', false, false],
      ['Set dressing', false, false],
      ['Wardrobe', false, false],
      ['Hair and makeup', false, false],
      ['Picture vehicles', false, false],
      ['Animals', false, true],
      ['Practical effects', false, false],
      ['Visual effects', false, false],
      ['Camera', false, false],
      ['Grip and electric', false, false],
      ['Sound and music', false, false],
      ['Special equipment', false, false],
      ['Extra crew', false, true],
      ['Safety', false, false],
      ['Other', false, false],
    ].map(([name, auto, daily], i) => ({ id: 3 + i, name, auto, holds: false, drops: false, gap: 1, exclude: false, daily, props: [] })),
  ],
  elements,
  // Red flag types in order; each element keeps its flags in `red_flags` (src/redflags.rs).
  // Travel and Unavailable are standard types (decision 113); the rest of the standard
  // list is missing, as in an older schedule, so Red Flag Types offers to add it.
  redFlagTypes: [
    { id: 89, name: 'Travel', builtin: 'travel', event_code: 'TR' },
    { id: 90, name: 'Unavailable', builtin: 'unavailable', event_code: null },
    { id: 91, name: 'Second job', builtin: null, event_code: null },
  ],
  // Each board's strip layout by board id (src/board.rs); missing means the default.
  layouts: {},
  // The strip colors (src/board.rs, set_strip_colors): rules and text colors.
  colors: { rules: [], scene_text: null, banner_text: null, break_text: null },
};
// TOVE links to KIT, so deleting either asks to unlink first.
elements.find((e) => e.name === 'TOVE').links = [castIds.KIT];
const undo = [];
const redo = [];
let saved = JSON.stringify(model);
let autosave = true;
// Saving safety: a recovery copy left by a "crash", the recent list and a file the system
// asked to open (set window.__pending = 'name.mtsc' before loading, or later with an
// open-request event).
let recovered = [{ key: 'untitled-1-2-3', name: 'Untitled', title: 'Night Ferry', saved_at_ms: Date.UTC(2026, 9, 10, 14, 3) }];
const recent = ['harbor.mtsc', 'ferry.mtsc'];
let fileName = 'harbor.mtsc';
// The app's own clipboard (src/clipboard.rs): { kind: 'strips', board, cut, items } with
// snapshots of the strips, or { kind: 'elements', sheet, cut, elements }.
let clip = null;
const clipDto = () =>
  clip && {
    kind: clip.kind,
    count: clip.kind === 'strips' ? clip.items.length : clip.elements.length,
    cut: clip.cut,
    board: clip.kind === 'strips' ? clip.board : null,
    strips: clip.kind === 'strips' && clip.cut ? clip.items.map((s) => s.id) : [],
    sheet: clip.kind === 'elements' ? clip.sheet : null,
    elements: clip.kind === 'elements' && clip.cut ? [...clip.elements] : [],
    scenes: clip.kind === 'strips' ? clip.items.filter((s) => s.kind === 'scene').length : 0,
  };
const countText = (n, one, many) => (n === 1 ? `1 ${one}` : `${n} ${many}`);

const pages = (e) => (e % 8 === 0 ? `${e / 8}` : e < 8 ? `${e % 8}/8` : `${Math.floor(e / 8)} ${e % 8}/8`);
const tone = (s) => `${s.ie.startsWith('EXT') ? 'ext' : 'int'}-${s.tod.includes('NIGHT') ? 'night' : 'day'}`;

/** `h:mm` or `m:ss` as typed into a sheet, to whole minutes or seconds (0 when empty). */
const hmOf = (v) => {
  const m = /^(\d+):(\d\d)$/.exec(String(v || '').trim());
  return m ? Number(m[1]) * 60 + Number(m[2]) : 0;
};
const hm = (mins) => `${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, '0')}`;

// The strip layout: StripField names, labels and default widths, as the engine has them.
const LAYOUT = [
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
/** The fields whose column picks a category (StripField::picks_category). */
const PICKS = ['element_count', 'category_elements'];
/** A layout field as the engine keeps it (decisions.md, entry 87): every key, defaults filled. */
const fullCol = (c) => ({ field: c.field, width: c.width, line: Number(c.line) || 1, span: Number(c.line) > 1 ? Number(c.span) || 1 : 1, align: c.align || 'auto', bold: Boolean(c.bold), italic: Boolean(c.italic), text: c.field === 'text' ? String(c.text || '').trim() : '', category: PICKS.includes(c.field) ? c.category ?? null : null, board_ids: c.field === 'category_elements' && Boolean(c.board_ids), rule_after: Boolean(c.rule_after), rule_under: Boolean(c.rule_under) });
const DEFAULT_COLUMNS = ['scene', 'int_ext', 'set', 'time_of_day', 'pages', 'cast'].map((f) => fullCol({ field: f, width: LAYOUT.find((x) => x[0] === f)[2] }));
// The layout library (src/layouts.rs): named layouts in m.library, each board's picks in
// m.uses[board], the one-line report's pick in m.oneLine. A board's own layout stays in
// m.layouts[board].
const ownLayout = (m, board) => (m.layouts && m.layouts[board]) || DEFAULT_COLUMNS;
const named = (m, id) => (m.library || []).find((l) => l.id === id) || null;
const usesOf = (m, board) => ((m.uses || {})[board]) || { strips: null, day_breaks: null, shoot_day: null };
const layoutOf = (m, board) => {
  const n = named(m, usesOf(m, board).strips);
  return n ? n.columns : ownLayout(m, board);
};
const ownSize = (m, board) => (m.layoutSizes || {})[board] || 'normal';
// The strip designer's mark switches (decision 136), the board's own and a named layout's.
const NO_HIDE = { hide_red_flags: false, hide_conflicts: false };
const ownMarks = (m, board) => (m.layoutMarks || {})[board] || NO_HIDE;
const marksOf = (m, board) => {
  const n = named(m, usesOf(m, board).strips);
  return n ? n.marks || NO_HIDE : ownMarks(m, board);
};
const marksIn = (x) => ({ hide_red_flags: Boolean(x && x.hide_red_flags), hide_conflicts: Boolean(x && x.hide_conflicts) });
const sizeOf = (m, board) => {
  const n = named(m, usesOf(m, board).strips);
  return n ? n.size || 'normal' : ownSize(m, board);
};
const colDto = (c) => {
  const f = fullCol(c);
  // A category's elements is headed by its category's name (view::layout_dto, entry 194).
  const cat = f.field === 'category_elements' ? model.categories.find((x) => x.id === f.category) : null;
  return { ...f, label: cat ? cat.name : LAYOUT.find((x) => x[0] === c.field)[1] };
};
const layoutDto = (m, board) => layoutOf(m, board).map(colDto);
const NO_LOOK = { shoot_day_strip: false, screen_time: false, sunrise: false, sunset: false, quantity: false, text: '' };
const ownLook = (m, board) => (m.dayLooks || {})[board] || NO_LOOK;
function lookOf(m, board) {
  const u = usesOf(m, board);
  let look = { ...ownLook(m, board) };
  const d = named(m, u.day_breaks);
  if (d) look = { ...d.look, shoot_day_strip: look.shoot_day_strip };
  const sd = named(m, u.shoot_day);
  if (sd) look.shoot_day_strip = true;
  return [look, sd ? sd.look.text : ''];
}
const libraryDto = (m) =>
  (m.library || []).map((l) => ({
    id: l.id,
    name: l.name,
    kind: l.kind,
    ...(l.kind === 'strips' ? { columns: l.columns.map(colDto), size: l.size || 'normal', marks: l.marks || NO_HIDE } : { look: l.look }),
    boards: m.boards.filter((b) => Object.values(usesOf(m, b.id)).includes(l.id)).length,
    one_line: m.oneLine === l.id,
  }));
function checkColumns(columns) {
  if (!Array.isArray(columns) || !columns.length) throw 'Choose at least one field for the strips to show.';
  const seen = new Set();
  for (const c of columns) {
    const keys = Object.keys(c).sort().join(',');
    if (keys !== 'align,board_ids,bold,category,field,italic,line,rule_after,rule_under,span,text,width') throw `fake: a layout field got ${keys}`;
    if (c.category != null && !PICKS.includes(c.field)) throw `${c.field} counts no category.`;
    if (c.board_ids && c.field !== 'category_elements') throw `${c.field} has no board ID switch.`;
    if (c.category != null && !model.categories.some((x) => x.id === c.category)) throw 'That category is no longer in the schedule.';
    const f = LAYOUT.find((x) => x[0] === c.field);
    if (!f) throw `${JSON.stringify(c.field)} is not a field strips can show.`;
    if (!['narrow', 'medium', 'wide', 'widest'].includes(c.width)) throw `${JSON.stringify(c.width)} is not a column width.`;
    if (!(c.line >= 1 && c.line <= 4)) throw 'A strip has lines 1 to 4.';
    if (!['auto', 'left', 'center', 'right'].includes(c.align)) throw `${JSON.stringify(c.align)} is not an alignment.`;
    if (seen.has(c.field) && !['spacer', 'text'].includes(c.field)) throw `The strip layout shows ${f[1]} twice.`;
    seen.add(c.field);
  }
  const cols = columns.map(fullCol);
  const first = cols.filter((c) => c.line === 1).length;
  if (!first) throw 'A strip layout needs a field on its first line.';
  for (const n of [2, 3, 4]) {
    const wide = cols.filter((c) => c.line === n).reduce((a, c) => a + c.span, 0);
    if (wide > first) throw `Line ${n} covers ${wide} columns but the first line has ${first}.`;
  }
  return cols;
}
function checkSize(size) {
  const v = size || 'normal';
  if (!['small', 'normal', 'large'].includes(v)) throw `${JSON.stringify(size)} is not a text size.`;
  return v;
}
const lookIn = (l) => ({ shoot_day_strip: false, screen_time: Boolean(l.screen_time), sunrise: Boolean(l.sunrise), sunset: Boolean(l.sunset), quantity: Boolean(l.quantity), text: String(l.text || '').trim() });
const freeLayoutName = (m, name, except = null) => {
  const t = String(name || '').trim();
  if (!t) throw 'Type a name for the layout.';
  if ((m.library || []).some((l) => l.id !== except && l.name.toLowerCase() === t.toLowerCase())) throw `There is already a layout named ${JSON.stringify(t)}.`;
  return t;
};
/** A day's element quantities as a day break shows them (reports::day_quantity_text): a sample category. */
function quantityOf(scenes) {
  const n = scenes.reduce((a, s) => a + (s.els.length ? 10 : 0), 0);
  return n ? `Background ${n}` : '';
}
/** A scene's text for one layout field (src/view.rs, reports::strip_cell). */
function cellText(s, field) {
  switch (field) {
    case 'scene':
      return s.n;
    case 'int_ext':
      return s.ie;
    case 'set':
      return s.set;
    case 'time_of_day':
      return s.tod;
    case 'pages':
      return pages(s.eighths);
    case 'cast':
      return s.els
        .map((id) => model.elements.find((e) => e.id === id))
        .filter((e) => e && isCast(e))
        .map((e) => e.board_id || e.name)
        .join(', ');
    case 'cast_count': {
      const n = s.els.map((id) => model.elements.find((e) => e.id === id)).filter((e) => e && isCast(e)).length;
      return n ? String(n) : '';
    }
    case 'element_count':
      return s.els.length ? String(s.els.length) : '';
    case 'elements':
      return s.els
        .map((id) => model.elements.find((e) => e.id === id))
        .filter((e) => e && !isCast(e))
        .map((e) => e.board_id || e.name)
        .join(', ');
    case 'spacer':
    case 'text':
    case 'category_elements':
      return '';
    default:
      return s.f[field] || '';
  }
}

function stripDto(s, index) {
  const base = { id: s.id, index, kind: s.kind, sheet: null, scene: '', int_ext: '', set: '', time_of_day: '', pages: '', cast: [], text: '', duration: '', tone: '' };
  if (s.kind === 'banner') return { ...base, text: s.text, duration: hm(s.minutes || 0), bold: Boolean(s.style && s.style.bold), banner_color: (s.style && s.style.color) || '', tall: Boolean(s.style && s.style.tall), ...(s.style && s.style.size && s.style.size !== 'normal' ? { banner_size: s.style.size } : {}), ...(s.style && ['center', 'right'].includes(s.style.align) ? { banner_align: s.style.align } : {}), ...(s.style && s.style.font && s.style.font !== 'usual' ? { banner_font: s.style.font } : {}) };
  if (s.kind !== 'scene') return base;
  const cast = s.els.map((id) => model.elements.find((e) => e.id === id)).filter((e) => e && isCast(e));
  // An element count of one category: `Props (2)`, leaving out elements off the board
  // (reports::column_cell, decision 147).
  const countOf = (c) => {
    const cat = model.categories.find((x) => x.id === c.category);
    if (!cat) return null;
    const n = s.els.map((id) => model.elements.find((e) => e.id === id)).filter((e) => e && e.category === cat.name && !e.exclude_from_board).length;
    return `${cat.name} (${n})`;
  };
  // A category's elements, by name or board ID, in board ID order (reports::category_elements, entry 194).
  const listOf = (c) => {
    const cat = model.categories.find((x) => x.id === c.category);
    if (!cat) return '';
    const els = s.els.map((id) => model.elements.find((e) => e.id === id)).filter((e) => e && e.category === cat.name && !e.exclude_from_board);
    els.sort((a, b) => (a.board_id && b.board_id ? a.board_id.localeCompare(b.board_id, undefined, { numeric: true }) : a.board_id ? -1 : b.board_id ? 1 : 0) || a.name.localeCompare(b.name));
    return els.map((e) => (c.board_ids && e.board_id) || e.name).join(', ');
  };
  const cells = layoutOf(model, model.shown).map((c) => (c.field === 'category_elements' ? listOf(c) : c.field === 'element_count' && c.category != null ? countOf(c) ?? cellText(s, c.field) : cellText(s, c.field)));
  const same = (r, v) => !r.trim() || r.trim().toLowerCase() === v.trim().toLowerCase();
  const rule = ((model.colors || {}).rules || []).find((r) => same(r.int_ext, s.ie) && same(r.time_of_day, s.tod));
  const ink = (hex) => {
    const n = parseInt(hex.slice(1), 16);
    return 299 * (n >> 16) + 587 * ((n >> 8) & 255) + 114 * (n & 255) >= 128000 ? '#000000' : '#ffffff';
  };
  const colored = rule ? { color: rule.color, ink: rule.text || model.colors.scene_text || ink(rule.color) } : {};
  // Parts of a scene spanning days (src/view.rs, parts_of): the strips of one sheet on the sub-boards.
  const sameSheet = model.subs.flatMap((x) => x.strips).filter((x) => x.kind === 'scene' && x.sheet === s.sheet);
  const partOf = sameSheet.length > 1 ? { part: sameSheet.findIndex((x) => x.id === s.id) + 1, parts: sameSheet.length } : {};
  return { ...base, ...partOf, sheet: s.sheet, scene: s.n, int_ext: s.ie, set: s.set, time_of_day: s.tod, pages: pages(s.eighths), cast: cast.map((e) => e.name), cells, tone: tone(s), ...colored };
}

// The calendar: weekly days off and per-date marks ({ date: 'work' | 'off' | 'holiday' |
// 'move' }), as each sub-board keeps them.
const WDKEY = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];
const calOf = (sb) => (sb.cal = sb.cal || { weekly_off: ['sat', 'sun'], marks: {}, events: [] });
function kindOf(sb, iso) {
  const c = calOf(sb);
  if (c.marks[iso]) return c.marks[iso];
  return c.weekly_off.includes(WDKEY[new Date(`${iso}T00:00:00Z`).getUTCDay()]) ? 'off' : 'work';
}

/** Shoot day `n`'s date on sub-board `sb` (its start date first), and its weekday. */
function addDays(sb, n) {
  const d = new Date(`${sb.start}T00:00:00Z`);
  let left = n;
  for (let guard = 0; guard < 5000; guard++) {
    if (kindOf(sb, d.toISOString().slice(0, 10)) === 'work' && --left === 0) break;
    d.setUTCDate(d.getUTCDate() + 1);
  }
  return [d.toISOString().slice(0, 10), ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'][d.getUTCDay()]];
}

function view(notice = null) {
  const sub_boards = model.subs.map((sb) => {
    const flagged = fakeRedFlags(model, sb);
    const marks = new Map();
    const kinds = new Map();
    for (const c of flagged)
      for (const sh of c.sheets)
        if (sh.strip !== null) {
          marks.set(sh.strip, [...(marks.get(sh.strip) || []), c.text]);
          kinds.set(sh.strip, [...new Set([...(kinds.get(sh.strip) || []), c.red_flag_type])]);
        }
    const cmarks = fakeConflictMarks(model);
    const days = [];
    let cur = [];
    sb.strips.forEach((s, i) => {
      const dto = stripDto(s, i);
      const hide = marksOf(model, model.shown);
      if (marks.has(s.id) && !hide.hide_red_flags) {
        dto.red_flags = [...new Set(marks.get(s.id))];
        dto.red_flag_types = kinds.get(s.id);
      }
      if (cmarks.strips.has(s.id) && !hide.hide_conflicts) dto.conflicts = cmarks.strips.get(s.id);
      if (s.kind !== 'break') return cur.push(dto);
      const number = days.length + 1;
      const e = cur.reduce((a, x) => a + (sb.strips[x.index].eighths || 0), 0);
      const mins = cur.reduce((a, x) => a + (sb.strips[x.index].minutes || 0), 0);
      const [date, weekday] = sb.start ? addDays(sb, number) : [null, null];
      const scenesOf = cur.filter((x) => x.kind === 'scene').map((x) => sb.strips[x.index]);
      const secs = scenesOf.reduce((a, x) => a + hmOf(x.f.screen_time), 0);
      const times = (k) => scenesOf.map((x) => x.f[k]).filter(Boolean).sort();
      const full = date ? new Date(`${date}T00:00:00Z`).toLocaleDateString('en-US', { timeZone: 'UTC', weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' }) : null;
      days.push({ number, date, weekday, pages: pages(e), time: hm(mins), scenes: cur.filter((x) => x.kind === 'scene').length, strips: cur, end: dto, screen_time: `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`, sunrise: times('sunrise')[0] || '', sunset: times('sunset').slice(-1)[0] || '', full_date: full, ...(lookOf(model, model.shown)[0].quantity ? { quantity: quantityOf(scenesOf) } : {}) });
      cur = [];
    });
    return { id: sb.id, name: sb.name, start: sb.start, days, unscheduled: cur, len: sb.strips.length, red_flags: flagged.length };
  });
  const all = model.subs.flatMap((s) => s.strips).concat(model.boneyard).filter((s) => s.kind === 'scene');
  return {
    title: model.title,
    file: { name: fileName, saved_once: true, dirty: JSON.stringify(model) !== saved },
    can_undo: undo.length > 0,
    can_redo: redo.length > 0,
    boards: model.boards.map((b) => ({ ...b })),
    board: { id: model.shown, name: boardName(model), layout: layoutDto(model, model.shown), layout_size: sizeOf(model, model.shown), layout_marks: marksOf(model, model.shown), own_layout: ownLayout(model, model.shown).map(colDto), own_layout_size: ownSize(model, model.shown), own_layout_marks: ownMarks(model, model.shown), uses: usesOf(model, model.shown), colors: model.colors || { rules: [] }, day_look: lookOf(model, model.shown)[0], shoot_text: lookOf(model, model.shown)[1], own_day_look: ownLook(model, model.shown), sub_boards, boneyard: model.boneyard.map(stripDto), scenes: all.length, pages: pages(all.reduce((a, s) => a + s.eighths, 0)) },
    notice,
    autosave,
    sheet: null,
    category: null,
    sub_board: null,
    strip: null,
    strips: [],
    clipboard: clipDto(),
    conflicts: fakeConflicts(model).count,
    layouts: libraryDto(model),
    one_line_layout: model.oneLine ?? null,
    red_flag_types: model.redFlagTypes.map((t) => ({ builtin: null, event_code: null, ...t, uses: model.elements.reduce((a, e) => a + (e.red_flags || []).filter((f) => f.red_flag_type === t.id).length, 0) })),
    standard_types_missing: STANDARD_TYPES.filter(([k]) => !model.redFlagTypes.some((t) => t.builtin === k)).length,
  };
}

// The standard red flag types (decision 113): every event kind but other, in order.
const STANDARD_TYPES = [
  ['travel', 'Travel', 'TR'],
  ['fitting', 'Fitting', 'FT'],
  ['rehearsal', 'Rehearsal', 'R'],
  ['unavailable', 'Unavailable', null],
  ['camera_test', 'Camera test', null],
  ['post_production', 'Post-production', null],
  ['visual_effects', 'Visual effects', null],
  ['dubbing', 'Dubbing', null],
  ['sync_sound', 'Sync sound', null],
  ['music_recording', 'Music recording', null],
  ['picture_editing', 'Picture editing', null],
  ['sound_editing', 'Sound editing', null],
  ['sound_mixing', 'Sound mixing', null],
  ['titles_graphics', 'Titles and graphics', null],
  ['trailer', 'Trailer', null],
  ['color_correction', 'Color correction', null],
];

/** `iso` moved by `n` days. */
function shiftDate(iso, n) {
  const [y, m, d] = iso.split('-').map(Number);
  const t = new Date(Date.UTC(y, m - 1, d + n));
  return `${t.getUTCFullYear()}-${String(t.getUTCMonth() + 1).padStart(2, '0')}-${String(t.getUTCDate()).padStart(2, '0')}`;
}

/** Every red flag as the Red flags tab's table has it (src/redflags.rs, flag_rows). */
function flagRows(m) {
  const rows = [];
  m.elements.forEach((e, place) => {
    // Elements point at their category by name in this fake.
    const c = m.categories.find((x) => x.name === e.category);
    (e.red_flags || []).forEach((f, k) => {
      const start = f.dates === 'range' ? f.from : f.dates === 'not_after' ? shiftDate(f.from, 1) : null;
      const end = f.dates === 'range' ? f.to : f.dates === 'not_before' ? shiftDate(f.from, -1) : null;
      const t = m.redFlagTypes.find((x) => x.id === f.red_flag_type);
      rows.push({ key: [start || '', place, k], row: { id: f.id, start, end, element: e.id, name: e.name, board_id: e.board_id || '', category: c ? c.id : 0, category_name: c ? c.name : '', red_flag_type: f.red_flag_type, type_name: t ? t.name : '', note: f.note, hard: f.hard, text: flagText(m, f) } });
    });
  });
  const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  rows.sort((a, b) => cmp(a.key[0], b.key[0]) || a.key[1] - b.key[1] || a.key[2] - b.key[2]);
  return rows.map((r) => r.row);
}

// ---- Boards

const boardName = (m) => m.boards.find((b) => b.id === m.shown).name;
/** A sub-board of the board shown; anything else is a mistake in the page. */
function subOf(m, id) {
  const sb = m.subs.find((s) => s.id === id);
  if (!sb) throw `fake: sub-board ${id} is not on the board shown`;
  return sb;
}
/** Shows board `id`, parking the one shown in m.store. */
function switchTo(m, id) {
  if (id === m.shown) return;
  if (!m.store[id]) throw 'That board is no longer in the schedule.';
  m.store[m.shown] = { subs: m.subs, boneyard: m.boneyard };
  ({ subs: m.subs, boneyard: m.boneyard } = m.store[id]);
  delete m.store[id];
  m.shown = id;
}
/** Every board's lists: [{ subs, boneyard }], the one shown first. */
const allBoards = (m) => [{ subs: m.subs, boneyard: m.boneyard }, ...Object.values(m.store)];
const needBoard = (m, id) => {
  const b = m.boards.find((x) => x.id === id);
  if (!b) throw 'That board is no longer in the schedule.';
  return b;
};
const boardNameFree = (m, name, except = null) => {
  const n = String(name).trim();
  if (!n) throw 'Type a name.';
  if (m.boards.some((b) => b.id !== except && same(b.name, n))) throw `There is already a board ${JSON.stringify(n)}.`;
  return n;
};
const subNameFree = (subs, name, except = null) => {
  const n = String(name).trim();
  if (!n) throw 'Type a name.';
  if (subs.some((s) => s.id !== except && same(s.name, n))) throw `This board already has a sub-board ${JSON.stringify(n)}.`;
  return n;
};
/** The lists a board keeps (the one shown, or one waiting in m.store). */
const listsOf = (m, id) => (id === m.shown ? m : m.store[id]);
const freshCopy = (strips) => strips.map((s) => ({ ...JSON.parse(JSON.stringify(s)), id: nextId++ }));
const natural = (a, b) => {
  const runs = (s) => s.match(/\d+|\D+/g) || [];
  const [x, y] = [runs(a), runs(b)];
  for (let i = 0; i < Math.min(x.length, y.length); i++) {
    const [p, q] = [x[i], y[i]];
    const [dp, dq] = [/^\d/.test(p), /^\d/.test(q)];
    let o = 0;
    if (dp && dq) o = Number(p) - Number(q);
    else if (dp !== dq) o = dp ? -1 : 1;
    else o = p.toLowerCase() < q.toLowerCase() ? -1 : p.toLowerCase() > q.toLowerCase() ? 1 : 0;
    if (o) return o;
  }
  return x.length - y.length;
};
const SORT_KEYS = { scene: 'n', int_ext: 'ie', set: 'set', time_of_day: 'tod', pages: 'eighths' };

// ---- Breakdown sheets

const scenesInOrder = (m) => m.subs.flatMap((s) => s.strips).concat(m.boneyard).filter((s) => s.kind === 'scene');
const sceneOf = (m, sheet) => scenesInOrder(m).find((s) => s.sheet === sheet);
const FIELD_KEYS = { scenes: 'n', int_ext: 'ie', set: 'set', time_of_day: 'tod' };
const TIME_RULES = {
  shoot_time: [/^(\d+:[0-5]\d|\d*\.\d{1,2})$/, 'Shoot time {v} must look like 1:30 (hours and minutes) or 1.5 (hours).'],
  screen_time: [/^\d+:[0-5]\d$/, 'Screen time {v} must look like 2:05 (minutes and seconds).'],
  sunrise: [/^([01]?\d|2[0-3]):[0-5]\d$/, 'Sunrise {v} must be a 24-hour time like 06:45.'],
  sunset: [/^([01]?\d|2[0-3]):[0-5]\d$/, 'Sunset {v} must be a 24-hour time like 06:45.'],
};

/** The header fields in the Breakdown tab's default order (model::SHEET_FIELDS). */
const SHEET_FIELDS = ['number', 'scenes', 'int_ext', 'set', 'time_of_day', 'shoot_time', 'synopsis', 'location', 'pages', 'script_pages', 'script_day', 'screen_time', 'unit', 'sequence', 'episode', 'comments', 'sunrise', 'sunset'];
const fieldOrder = (m) => {
  const out = [...(m.sheetFields || [])];
  for (const f of SHEET_FIELDS) if (!out.includes(f)) out.push(f);
  return out;
};
/** Scenes in the stepping order (decisions.md, entry 81): board order, sheet number or scene number. */
function scenesBy(m, order) {
  const all = scenesInOrder(m);
  if (order !== 'number' && order !== 'scene') return all;
  const key = (s) => String(order === 'number' ? s.f.number || '' : s.n || '').trim();
  return all
    .map((s, i) => [s, i])
    .sort(([a, i], [b, j]) => {
      const [x, y] = [key(a), key(b)];
      if (!x || !y) return !x && !y ? i - j : x ? -1 : 1;
      return natural(x, y) || i - j;
    })
    .map(([s]) => s);
}
const decimalHours = (v) => {
  const m = hmOf(v);
  if (!v) return '';
  const h = Math.floor((m * 100 + 30) / 60);
  return `${Math.floor(h / 100)}.${String(h % 100).padStart(2, '0')}`;
};

function sheetDto(sheet, index, order = 'board') {
  const all = scenesBy(model, order);
  if (!all.length) return null;
  let at = all.findIndex((s) => s.sheet === sheet);
  if (at < 0) at = Math.min(index || 0, all.length - 1);
  const s = all[at];
  const el = (id) => {
    const e = model.elements.find((x) => x.id === id);
    if (!e) return null;
    const q = (s.qty || {})[id];
    return q ? { ...e, quantity: q.count, ...(q.daily ? {} : { daily: false }) } : e;
  };
  const groups = model.categories
    .map((c) => c.name)
    .map((c) => ({ category: c, elements: s.els.map(el).filter((e) => e && e.category === c) }))
    .filter((g) => g.elements.length);
  const where = model.boneyard.includes(s) ? 'Boneyard' : 'Main';
  const sub = model.subs.find((x) => x.strips.includes(s));
  const distinct = (k) => [...new Set(all.map((x) => x[k]).filter(Boolean))].sort();
  return {
    id: s.sheet,
    number: s.f.number || '',
    scenes: s.n,
    int_ext: s.ie,
    set: s.set,
    time_of_day: s.tod,
    sunrise: s.f.sunrise || '',
    sunset: s.f.sunset || '',
    pages_whole: String(Math.floor(s.eighths / 8)),
    pages_eighths: String(s.eighths % 8),
    pages: pages(s.eighths),
    shoot_time: s.f.shoot_time || '',
    shoot_time_decimal: decimalHours(s.f.shoot_time || ''),
    screen_time: s.f.screen_time || '',
    synopsis: s.f.synopsis || '',
    location: s.f.location || '',
    script_pages: s.f.script_pages || '',
    script_day: s.f.script_day || '',
    unit: s.f.unit || '',
    sequence: s.f.sequence || '',
    episode: s.f.episode || '',
    comments: s.f.comments || '',
    place: where,
    strip: s.id,
    strip_sub_board: sub ? sub.id : null,
    date: where === 'Main' && model.subs[0].start ? model.subs[0].start : null,
    index: at,
    count: all.length,
    prev: at > 0 ? all[at - 1].sheet : null,
    next: at + 1 < all.length ? all[at + 1].sheet : null,
    first: all[0].sheet,
    last: all[all.length - 1].sheet,
    order: order === 'number' || order === 'scene' ? order : 'board',
    field_order: fieldOrder(model),
    hidden_fields: SHEET_FIELDS.filter((k) => (model.sheetHidden || []).includes(k)),
    field_widths: SHEET_FIELDS.filter((k) => (model.sheetWidths || {})[k]).map((k) => [k, model.sheetWidths[k]]),
    field_rows: model.sheetRows || [],
    images: [...(s.images || [])],
    red_flags: (() => {
      const sb = model.subs.find((x) => x.strips.includes(s));
      return sb ? [...new Set(fakeRedFlags(model, sb).filter((c) => c.sheets.some((h) => h.sheet === s.sheet)).map((c) => c.text))] : [];
    })(),
    red_flag_types: [],
    conflicts: fakeConflictMarks(model).strips.get(s.id) || [],
    element_images: s.els
      .map((id) => model.elements.find((e) => e.id === id))
      .filter((e) => e && e.images && e.images.length)
      .map((e) => ({ element: e.id, name: e.name, category: e.category, images: [...e.images] })),
    groups,
    categories: model.categories.map((c) => ({ id: c.id, name: c.name })),
    elements: [...model.elements].sort((a, b) => a.name.localeCompare(b.name)),
    header_values: Object.fromEntries(HEADER_KEYS.map((k) => [k, headerValues(model, k)])),
  };
}

function needSheet(m, sheet) {
  const s = sceneOf(m, sheet);
  if (!s) throw 'That sheet is no longer in the schedule.';
  return s;
}

// The fake's edit() always takes a step; the real one skips edits that change nothing.
function sheetEdit(sheet, fn) {
  needSheet(model, sheet);
  return edit((m) => {
    for (const b of allBoards(m)) {
      const s = b.subs.flatMap((x) => x.strips).concat(b.boneyard).find((x) => x.sheet === sheet);
      if (s) fn(s, m);
    }
  });
}

/** A banner's look as src/board.rs reads it (decisions.md, entry 65). */
function bannerStyle(style) {
  if (!style) return { bold: false, color: null, tall: false, size: 'normal', align: 'left', font: 'usual' };
  const colors = ['yellow', 'orange', 'red', 'pink', 'purple', 'blue', 'teal', 'green', 'gray'];
  // Or any color by value, lowercase #rrggbb (decision 154).
  if (style.color && !colors.includes(style.color) && !/^#[0-9a-f]{6}$/.test(style.color)) throw `${JSON.stringify(style.color)} is not one of the banner colors.`;
  // Its text size step and alignment (decisions.md, entry 87).
  if (style.size && !['small', 'normal', 'large'].includes(style.size)) throw `${JSON.stringify(style.size)} is not a text size.`;
  if (style.align && !['left', 'center', 'right', 'auto'].includes(style.align)) throw `${JSON.stringify(style.align)} is not an alignment.`;
  if (style.font && !['usual', 'serif', 'sans', 'mono'].includes(style.font)) throw `${JSON.stringify(style.font)} is not one of the banner fonts.`;
  return { bold: Boolean(style.bold), color: style.color || null, tall: Boolean(style.tall), size: style.size || 'normal', align: style.align || 'left', font: style.font || 'usual' };
}

function edit(fn) {
  const before = JSON.stringify(model);
  const next = JSON.parse(before);
  fn(next);
  undo.push(before);
  redo.length = 0;
  model = next;
  return view();
}

const list = (m, sub) => (sub === null ? m.boneyard : subOf(m, sub).strips);

// ---- The Elements tab

const isBuiltIn = (c) => !!c.cast;
// An element of the cast category, whatever that is called now.
function isCast(e) {
  const c = model.categories.find(isBuiltIn);
  return !!c && e.category === c.name;
}
const same = (a, b) => a.trim().toLowerCase() === b.trim().toLowerCase();
// Natural board ID order, as reports::id_order: 1, 1a, 2, 7, 07, 9, 10, X, then none.
const idOrder = (a, b) => {
  if (!a.board_id && !b.board_id) return 0;
  if (!a.board_id) return 1;
  if (!b.board_id) return -1;
  const split = (id) => /^(\d*)(.*)$/s.exec(id).slice(1);
  const [[xd, xr], [yd, yr]] = [split(a.board_id), split(b.board_id)];
  const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
  if (xd && !yd) return -1;
  if (!xd && yd) return 1;
  const [xv, yv] = [xd.replace(/^0+/, ''), yd.replace(/^0+/, '')];
  return xv.length - yv.length || cmp(xv, yv) || cmp(xr, yr) || xd.length - yd.length || cmp(a.board_id, b.board_id);
};
const listed = (m, cat) =>
  m.elements.filter((e) => e.category === cat).sort((a, b) => idOrder(a, b) || a.name.toUpperCase().localeCompare(b.name.toUpperCase()));
const needCat = (m, id) => {
  const c = m.categories.find((x) => x.id === id);
  if (!c) throw 'That category is no longer in the schedule.';
  return c;
};
const needEl = (m, id) => {
  const e = m.elements.find((x) => x.id === id);
  if (!e) throw 'That element is no longer in the schedule.';
  return e;
};
const bool = (v) => {
  if (v !== 'true' && v !== 'false') throw 'That switch takes true or false.';
  return v === 'true';
};
const gapOf = (v) => {
  const t = String(v).trim();
  if (!/^\d{1,3}$/.test(t) || Number(t) < 1) throw 'The minimum drop gap is a whole number of days from 1 to 999.';
  return Number(t);
};

// ---- The day out of days grid: work days only (no holds, drops or events), Saturday and
// Sunday off. Set window.__bigDood = { rows, days } before opening the tab for a large grid
// (synthetic codes) to check scrolling and the rows drawn.

const isoDay = (iso, n) => {
  const d = new Date(`${iso}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d;
};
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const WD = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const dateDto = (d, shoot, sb = model.subs[0]) => ({
  date: d.toISOString().slice(0, 10),
  weekday: WD[d.getUTCDay()],
  day: d.getUTCDate(),
  month: MONTHS[d.getUTCMonth()],
  kind: kindOf(sb, d.toISOString().slice(0, 10)),
  shoot_day: shoot,
});

function doodGrid(category, sub, header = null) {
  const m = model;
  const sb = subOf(m, sub);
  const cats = m.categories.map((c) => ({ id: c.id, name: c.name }));
  const hk = HEADERS.find((h) => h[0] === header);
  if (hk) {
    // A header field as a category: its values stand in for elements, read from the sheets.
    const values = headerValues(m, hk[0]);
    const fakeEls = values.map((v, i) => ({ id: 5000 + i, name: v, board_id: '', category: '\u0000header', links: [] }));
    const saved = model;
    const copy = JSON.parse(JSON.stringify(m));
    copy.categories.push({ id: 4999, name: '\u0000header', auto: false, holds: false, drops: false, gap: 1, props: [] });
    copy.elements.push(...fakeEls);
    for (const s of copy.subs.flatMap((x) => x.strips).concat(copy.boneyard)) {
      if (s.kind !== 'scene') continue;
      const i = values.findIndex((v) => same(v, hk[2](s) || ''));
      if (i >= 0) s.els = [...s.els, 5000 + i];
    }
    model = copy;
    try {
      const g = doodGrid(4999, sub);
      return { ...g, categories: cats, headers: headersDto(saved), header: hk[0], category: null, excluded: false };
    } finally {
      model = saved;
    }
  }
  const chosen = m.categories.find((c) => c.id === category) || m.categories.find(isBuiltIn) || m.categories[0];
  const out = { sub_board: sb.name, start: sb.start, categories: cats, headers: headersDto(m), header: null, excluded: !!chosen && m.elements.some((e) => e.category === chosen.name) && m.elements.filter((e) => e.category === chosen.name).every((e) => e.exclude_from_board), category: chosen ? chosen.id : null, dates: [], rows: [], empty: null };
  if (!sb.start) return { ...out, empty: `Set a start date for ${sb.name} to see the day out of days.` };
  const big = window.__bigDood;
  if (big) {
    let shoot = 0;
    for (let i = 0; i < big.days; i++) {
      const d = isoDay(sb.start, i);
      const dto = dateDto(d, null);
      if (dto.kind === 'work') dto.shoot_day = ++shoot;
      out.dates.push(dto);
    }
    for (let r = 0; r < big.rows; r++) {
      const from = (r * 3) % Math.max(1, big.days - 20);
      const to = Math.min(big.days - 1, from + 10 + (r % 15));
      const cells = out.dates.map((d, i) => {
        if (i < from || i > to || d.kind !== 'work') return '';
        if (i === from) return 'SW';
        if (i === to || (i + 1 <= to && out.dates.slice(i + 1, to + 1).every((x) => x.kind !== 'work'))) return 'WF';
        return (i + r) % 3 ? 'H' : 'W';
      });
      const work = cells.filter((c) => c.includes('W')).length;
      const hold = cells.filter((c) => c === 'H').length;
      out.rows.push({ element: 1000 + r, board_id: String(r + 1), name: `PLAYER ${r + 1}`, cells, span: [from, to], work, hold, start: out.dates[from].date, finish: out.dates[to].date, total: work + hold });
    }
    return out;
  }
  // Each closed day's date, and who works it.
  const work = new Map();
  let day = 0;
  let cur = [];
  for (const st of sb.strips) {
    if (st.kind !== 'break') {
      cur.push(st);
      continue;
    }
    day++;
    const [date] = addDays(sb, day);
    for (const x of cur) for (const e of x.els || []) (work.get(e) || work.set(e, new Set()).get(e)).add(date);
    cur = [];
  }
  const rows = listed(m, chosen.name).filter((e) => work.has(e.id));
  const flagged = fakeRedFlags(m, sb);
  if (!rows.length) return { ...out, empty: `No one in ${chosen.name} works on a shoot day or has an event yet.` };
  const all = [...rows.flatMap((e) => [...work.get(e.id)])].sort();
  const first = all[0];
  const last = all[all.length - 1];
  const shootDates = new Map();
  for (let n = 1; n <= day; n++) shootDates.set(addDays(sb, n)[0], n);
  for (let i = 0; ; i++) {
    const d = isoDay(first, i);
    const iso = d.toISOString().slice(0, 10);
    if (iso > last) break;
    out.dates.push(dateDto(d, shootDates.get(iso) || null));
  }
  for (const e of rows) {
    const ds = [...work.get(e.id)].sort();
    const cells = out.dates.map((d) => {
      if (!ds.includes(d.date)) return '';
      return (d.date === ds[0] ? 'S' : '') + 'W' + (d.date === ds[ds.length - 1] ? 'F' : '');
    });
    const at = (iso) => out.dates.findIndex((d) => d.date === iso);
    const red_flags = flagged.filter((x) => x.element === e.id && at(x.date) >= 0).map((x) => ({ at: at(x.date), text: x.text }));
    out.rows.push({ element: e.id, board_id: e.board_id, name: e.name, cells, span: [at(ds[0]), at(ds[ds.length - 1])], work: ds.length, hold: 0, start: ds[0], finish: ds[ds.length - 1], total: ds.length, red_flags });
  }
  return out;
}

// ---- The Reports tab: report_data in the shape src/reports.rs sends, from the fake
// board. Set window.__bigReport = n before opening the tab to add n synthetic shoot days
// (for paging checks). print_report and the exports record what they were asked in
// window.__printed and window.__exported; window.__cancel = true makes the next dialog
// "cancelled".

const REPORT_NAMES = { one_line: 'One-line schedule', shooting: 'Shooting schedule', dood: 'Day out of days', elements: 'Elements', totals: 'Daily category totals' };

function reportDays(m, sb) {
  const out = [];
  let cur = [];
  const elOf = (id) => m.elements.find((e) => e.id === id);
  const lineOf = (s) => {
    // A banner as the view draws it, with its look (src/reports.rs, banner).
    if (s.kind === 'banner') {
      const b = stripDto(s, 0);
      return { kind: 'banner', scenes: '', int_ext: '', set: '', time_of_day: '', pages: '', synopsis: '', location: '', cast: [], cells: [], groups: [], text: b.text, duration: b.duration, bold: b.bold, banner_color: b.banner_color, tall: b.tall, banner_size: b.banner_size, banner_align: b.banner_align, banner_font: b.banner_font };
    }
    const els = s.els.map(elOf).filter(Boolean);
    const cast = els.filter(isCast).sort(idOrder).map((e) => e.board_id || e.name);
    const groups = m.categories
      .map((c) => ({ category: c.name, elements: els.filter((e) => e.category === c.name).sort(idOrder).map((e) => (e.board_id ? `${e.board_id} ${e.name}` : e.name)) }))
      .filter((g) => g.elements.length);
    const cells = layoutOf(m, m.shown).map((c) => cellText(s, c.field));
    return { kind: 'scene', scenes: s.n, int_ext: s.ie, set: s.set, time_of_day: s.tod, pages: pages(s.eighths), synopsis: s.f.synopsis || '', location: s.f.location || '', cast, cells, groups, text: '', duration: '' };
  };
  const strips = [...sb.strips];
  for (let i = 0; i < (window.__bigReport || 0); i++) strips.push(...sb.strips.filter((s) => s.kind === 'scene').slice(0, 1 + (i % 3)), { kind: 'break' });
  for (const s of strips) {
    if (s.kind !== 'break') {
      cur.push(s);
      continue;
    }
    const number = out.length + 1;
    const [date, weekday] = sb.start ? addDays(sb, number) : [null, null];
    const lines = cur.map(lineOf);
    out.push({ number, date, weekday, pages: pages(cur.reduce((a, x) => a + (x.eighths || 0), 0)), time: '0:00', scenes: lines.filter((l) => l.kind === 'scene').length, lines });
    cur = [];
  }
  return out;
}

function reportData(report, sub) {
  if (!REPORT_NAMES[report]) throw "That report isn't one the app knows.";
  const m = model;
  const sb = subOf(m, sub);
  const out = { report, name: REPORT_NAMES[report], title: m.title, board: boardName(m), sub_board: sb.name, start: sb.start, empty: null, days: [], elements: [], dood: [], totals: null, layout: report === 'one_line' ? (named(m, m.oneLine) ? named(m, m.oneLine).columns.map(colDto) : layoutDto(m, m.shown)) : [] };
  const noDays = `There are no shoot days on ${sb.name} yet. Put day breaks on the Board first.`;
  const days = reportDays(m, sb);
  if (report === 'one_line' || report === 'shooting') {
    out.days = days;
    if (!days.length) out.empty = noDays;
  } else if (report === 'dood') {
    for (const c of m.categories) {
      const grid = doodGrid(c.id, sub);
      if (grid.rows.length) out.dood.push({ category: c.name, grid: { ...grid, categories: [] } });
      else if (!sb.start) out.empty = grid.empty;
    }
    if (!out.dood.length && !out.empty) out.empty = 'No element works on a shoot day or has an event yet.';
  } else if (report === 'elements') {
    const grid = doodGrid(null, sub);
    for (const c of m.categories) {
      for (const e of listed(m, c.name)) {
        const on = scenesInOrder(m).filter((s) => s.els.includes(e.id));
        const r = grid.rows.find((x) => x.element === e.id);
        out.elements.push({ category: c.name, board_id: e.board_id, name: e.name, sheets: on.length, pages: pages(on.reduce((a, s) => a + s.eighths, 0)), days: r ? r.work : 0, start: r ? r.start : null, finish: r ? r.finish : null, properties: (e.props || []).map((p) => `${p.name}: ${p.value}`) });
      }
    }
    if (!out.elements.length) out.empty = 'The schedule has no elements yet.';
  } else {
    const counts = days.map((d) => m.categories.map((c) => new Set(d.lines.flatMap((l) => l.groups.filter((g) => g.category === c.name).flatMap((g) => g.elements))).size));
    const used = m.categories.map((_, i) => counts.some((r) => r[i] > 0));
    out.totals = {
      categories: m.categories.filter((_, i) => used[i]).map((c) => c.name),
      left_out: used.filter((u) => !u).length,
      days: days.map((d, k) => ({ number: d.number, date: d.date, weekday: d.weekday, counts: counts[k].filter((_, i) => used[i]), partial: counts[k].filter((_, i) => used[i]).map(() => false) })),
    };
    if (!days.length) out.empty = noDays;
  }
  return out;
}

// ---- The report list and its options (src/reportsetup.rs, decision 59), roughly: the
// list lives in model.reports (null: the default), production info in model.production,
// code letters in model.codes, the user's preferences in `fakePrefs`.

const KINDS = [
  ['one_line', 'One-line schedule', false],
  ['shooting', 'Shooting schedule', false],
  ['breakdown', 'Breakdown by strip', false],
  ['breakdown_day', 'Breakdown by day', false],
  ['dood', 'Day out of days', true],
  ['elements', 'Elements', false],
  ['element_detail', 'Element detail', false],
  ['totals', 'Daily category totals', false],
  ['calendar', 'Calendar', true],
  ['vertical', 'Vertical one-line', true],
  ['multi_unit', 'Multi-unit one-line', false],
  ['conflicts', 'Conflicts', false],
  ['red_flags', 'Red flags', false],
];
const DEFAULT_OPTIONS = { from: '', to: '', landscape: null, synopsis: true, cast_names: false, page_break: 'none', shoot_days: true, day_breaks: true, total_count: false, element_columns: [], layout: null, banners: true, categories: [], elements: [], colors: false, icons: true, header: 1, text: 'normal', columns: [], no_holds: false, weekdays_only: false };
/** The header layouts a schedule starts with (headers::default_headers), and what a row can hold. */
const DEFAULT_HEADERS = [
  { id: 1, name: 'Full', include: true, first_page_only: true, bold: false, italic: false, align: 'left', size: 'normal', rows: [['title'], ['company', 'director', 'producer'], ['report_name', 'board', 'sub_board', 'start_date']] },
  { id: 2, name: 'Compact', include: true, first_page_only: true, bold: true, italic: false, align: 'left', size: 'normal', rows: [['title', 'report_name', 'board', 'sub_board']] },
  { id: 3, name: 'Title only', include: true, first_page_only: true, bold: false, italic: false, align: 'left', size: 'normal', rows: [['title']] },
];
const headerList = (m) => m.headers || DEFAULT_HEADERS.map((h) => JSON.parse(JSON.stringify(h)));
const HEADER_ITEMS = () => [['space', 'Space'], ['title', 'Title'], ['report_name', 'Report name'], ['today', "Today's date"], ['board', 'Board'], ['sub_board', 'Sub-board'], ['start_date', 'Start date'], ...PRODUCTION_FIELDS];
const reportList = (m) => m.reports || KINDS.map(([kind, name]) => ({ name, kind, options: { ...DEFAULT_OPTIONS } }));
const kindOfReport = (k) => KINDS.find((x) => x[0] === k);
const CODE_KEYS = [['start', 'start', 'S'], ['work', 'work', 'W'], ['finish', 'finish', 'F'], ['hold', 'hold', 'H'], ['drop', 'drop', 'D'], ['pickup', 'pickup', 'P'], ['travel', 'travel', 'TR'], ['fitting', 'fitting', 'FT'], ['rehearsal', 'rehearsal', 'R'], ['holiday', 'holiday', '/'], ['company_move', 'company travel day', 'M']];
const codesOf = (m) => CODE_KEYS.map(([k, label, d]) => [k, label, (m.codes && m.codes[k]) || d]);
const PRODUCTION_FIELDS = [['company', 'Company'], ['director', 'Director'], ['producer', 'Producer'], ['unit_production_manager', 'Unit production manager'], ['assistant_director', 'Assistant director'], ['art_director', 'Art director'], ['set_dresser', 'Set dresser'], ['script_date', 'Script date'], ['production_number', 'Production number'], ['preparer', 'Prepared by'], ['timing', 'Timing'], ['episode', 'Episode'], ['custom_1', 'Custom 1'], ['custom_2', 'Custom 2'], ['custom_3', 'Custom 3'], ['custom_4', 'Custom 4']];
let fakePrefs = { week_start: 'sun', date_format: 'iso', calendar_fields: ['scene', 'set', 'pages'], fountain_lines: 56, theme: 'paper' };

function reportListDto() {
  const m = model;
  return {
    reports: reportList(m).map((r, index) => ({ index, name: r.name, kind: r.kind, kind_label: kindOfReport(r.kind)[1], landscape: r.options.landscape === null ? kindOfReport(r.kind)[2] : r.options.landscape, options: r.options })),
    title: m.title,
    company: (m.production || {}).company || '',
    director: (m.production || {}).director || '',
    producer: (m.production || {}).producer || '',
    production: PRODUCTION_FIELDS.map(([k, label]) => [k, label, (m.production || {})[k] || '']),
    headers: headerList(m),
    header_items: HEADER_ITEMS(),
    strip_layouts: (m.library || []).filter((l) => l.kind === 'strips').map((l) => ({ id: l.id, name: l.name })),
    categories: m.categories.map((c) => ({ id: c.id, name: c.name })),
    elements: m.categories.flatMap((cat) => listed(m, cat.name).map((e) => ({ id: e.id, name: e.name, board_id: e.board_id, category: cat.name }))),
    fields: LAYOUT.filter((x) => !['spacer', 'text'].includes(x[0])).map((x) => [x[0], x[1]]),
  };
}

function reportEdit(fn) {
  return edit((m) => {
    const listNow = reportList(m).map((r) => ({ ...r, options: { ...r.options } }));
    fn(listNow, m);
    const names = listNow.map((r) => r.name.trim().toLowerCase());
    const twice = listNow.find((r, i) => names.indexOf(names[i]) !== i);
    if (twice) throw `A report named ${JSON.stringify(twice.name)} is already in the list.`;
    m.reports = listNow;
  });
}

/** A report column as Rust keeps it (src/reportsetup.rs ColumnDto): a name or a cell, every key filled. */
const reportCell = (c) => {
  const o = typeof c === 'string' ? { field: c } : c || {};
  const line = Number(o.line) || 1;
  return { field: o.field, line, span: line > 1 ? Number(o.span) || 1 : 1, align: o.align || 'auto', bold: Boolean(o.bold), italic: Boolean(o.italic), size: o.size || '' };
};

const inRange = (o, iso) => !iso || ((!o.from || iso >= o.from) && (!o.to || iso <= o.to));

/** A report's header as it prints (src/reports.rs head_of). */
function headOf(m, r, sb) {
  const h = headerList(m).find((x) => x.id === r.options.header) || headerList(m)[0];
  const p = m.production || {};
  const label = (k) => (PRODUCTION_FIELDS.find((x) => x[0] === k) || [k, k])[1];
  const text = (i) => {
    if (typeof i !== 'string') return i.text;
    if (i === 'space' || i === 'today') return '';
    if (i === 'title') return m.title || 'Untitled';
    if (i === 'report_name') return r.name;
    if (i === 'board') return boardName(m);
    if (i === 'sub_board') return sb.name;
    if (i === 'start_date') return sb.start || '';
    const v = p[i] || '';
    if (!v) return '';
    return i === 'company' || i.startsWith('custom_') ? v : `${label(i)} ${v}`;
  };
  return { include: h.include, every_page: !h.first_page_only, bold: h.bold, italic: h.italic, align: h.align, size: h.size, rows: h.rows.map((row) => row.map((i) => ({ kind: typeof i === 'string' ? i : 'text', text: text(i) }))) };
}

function reportDataFor(index, sub, weekStart) {
  const m = model;
  const r = reportList(m)[index];
  if (!r) throw 'That report is no longer in the list.';
  const o = r.options;
  const base = r.kind in REPORT_NAMES ? reportData(r.kind, sub) : { report: r.kind, title: m.title, board: boardName(m), sub_board: subOf(m, sub).name, start: subOf(m, sub).start, empty: null, days: [], elements: [], dood: [], totals: null, layout: [] };
  const sb = subOf(m, sub);
  const own = o.columns.map(reportCell);
  const pickedLayout = o.layout ? named(m, o.layout) : null;
  const follows = pickedLayout ? pickedLayout.columns : r.kind === 'one_line' || r.kind === 'multi_unit' ? layoutOf(m, m.shown) : null;
  const cols = own.length ? own.map((c) => c.field) : follows ? follows.map((c) => c.field) : ['scene', 'int_ext', 'set', 'time_of_day', 'pages'];
  // A report's own columns keep their lines, merged cells, looks and sizes (src/reports.rs).
  const widthOf = (f) => (layoutOf(m, m.shown).find((c) => c.field === f) || { width: LAYOUT.find((x) => x[0] === f)[2] }).width;
  const out = {
    ...base,
    report: r.kind,
    index,
    name: r.name,
    head: headOf(m, r, sb),
    text: o.text,
    landscape: o.landscape === null ? kindOfReport(r.kind)[2] : o.landscape,
    range: o.from || o.to ? `${o.from || '...'} to ${o.to || '...'}` : null,
    synopsis: o.synopsis,
    banners: o.banners,
    shoot_days: o.shoot_days !== false,
    day_breaks: o.day_breaks !== false,
    total: null,
    colors: o.colors,
    icons: o.icons,
    weekdays_only: o.weekdays_only,
    filtered: m.categories.filter((c) => o.categories.length && !o.categories.includes(c.id)).map((c) => c.name),
    layout: own.length
      ? own.map((c) => ({ ...colDto({ ...c, width: widthOf(c.field) }), ...(c.size ? { size: c.size } : {}) }))
      : follows
        ? follows.map(colDto)
        : cols.map((f) => colDto({ field: f, width: LAYOUT.find((x) => x[0] === f)[2] })),
    blocks: [],
    details: [],
    vertical: null,
    units: [],
    undated: [],
    months: [],
    week: [],
    conflicts: null,
    red_flags: null,
  };
  const days = reportDays(m, sb).filter((d) => inRange(o, d.date));
  const scenes = scenesInOrder(m);
  const lineOf = (l) => {
    if (l.kind === 'banner') return l;
    const sc = scenes.find((x) => x.n === l.scenes && x.set === l.set);
    return { ...l, synopsis: o.synopsis ? l.synopsis : '', cells: sc ? cols.map((f) => cellText(sc, f)) : [], tone: sc ? tone(sc) : 'int-day', flags: [] };
  };
  for (const d of out.days || []) d.groups = [];
  if (r.kind === 'one_line' || r.kind === 'shooting') {
    out.days = days.map((d) => ({ ...d, groups: [], lines: d.lines.filter((l) => o.banners || l.kind !== 'banner').map(lineOf) }));
    if (!out.days.length && !out.empty) out.empty = 'No shoot day falls in the report\'s date range.';
  } else if (r.kind === 'breakdown') {
    out.blocks = days.flatMap((d) => d.lines.filter((l) => l.kind === 'scene').map((l) => ({ day: d.number, date: d.date, weekday: d.weekday, line: lineOf(l) })));
    if (!out.blocks.length) out.empty = `${sb.name} has no scenes yet.`;
  } else if (r.kind === 'breakdown_day') {
    out.days = days.map((d) => {
      const merged = new Map();
      const scenesOnly = d.lines.filter((l) => l.kind === 'scene');
      for (const l of scenesOnly) for (const g of l.groups) for (const e of g.elements) {
        const key = `${g.category}|${e}`;
        merged.set(key, (merged.get(key) || 0) + 1);
      }
      const groups = m.categories.map((c) => ({ category: c.name, elements: [...merged].filter(([k]) => k.startsWith(`${c.name}|`)).map(([k, n]) => `${k.split('|')[1]} (${n})`) })).filter((g) => g.elements.length);
      return { ...d, lines: scenesOnly.map(lineOf), groups };
    });
    if (!out.days.length) out.empty = `There are no shoot days on ${sb.name} yet. Put day breaks on the Board first.`;
  } else if (r.kind === 'element_detail') {
    for (const c of m.categories) {
      for (const e of listed(m, c.name)) {
        const on = days.flatMap((d) => d.lines.filter((l) => l.kind === 'scene').map((l) => ({ d, l }))).filter(({ l }) => (scenes.find((x) => x.n === l.scenes && x.set === l.set) || { els: [] }).els.includes(e.id));
        if (!on.length) continue;
        out.details.push({ category: c.name, name: e.board_id ? `${e.board_id} ${e.name}` : e.name, work_dates: [...new Set(on.map(({ d }) => d.date).filter(Boolean))], scenes: on.map(({ d, l }) => ({ day: d.number, date: d.date, weekday: d.weekday, line: lineOf(l) })) });
      }
    }
    if (!out.details.length) out.empty = `No element is on a scene of ${sb.name} yet.`;
  } else if (r.kind === 'vertical') {
    const cast = m.elements.filter(isCast).sort(idOrder);
    const columns = [];
    for (const d of days) {
      const ls = d.lines.filter((l) => l.kind === 'scene');
      ls.forEach((l, k) => {
        const sc = scenes.find((x) => x.n === l.scenes && x.set === l.set) || { els: [] };
        // The day's last scene carries its totals, for its day break column (behavior note 23).
        const end = k === ls.length - 1 ? { pages: d.pages, time: d.time } : null;
        columns.push({ day: d.number, date: d.date, weekday: d.weekday, first: k === 0, scene: l.scenes, int_ext: l.int_ext, set: l.set, time_of_day: l.time_of_day, pages: l.pages, rows: cast.map((e, i) => (sc.els.includes(e.id) ? i : -1)).filter((i) => i >= 0), end });
      });
    }
    const pr = m.production || {};
    const production = [['Producer', pr.producer], ['Director', pr.director], ['Assistant director', pr.assistant_director], ['Script date', pr.script_date]].filter(([, v]) => v && String(v).trim());
    out.vertical = { cast: cast.map((e) => [e.board_id || '', e.name]), columns, production };
    if (!columns.length) out.empty = `There are no shoot days on ${sb.name} yet. Put day breaks on the Board first.`;
  } else if (r.kind === 'multi_unit') {
    const byDate = new Map();
    for (const u of m.subs) {
      if (!u.start) {
        out.undated.push(u.name);
        continue;
      }
      for (const d of reportDays(m, u).filter((x) => inRange(o, x.date))) {
        if (!byDate.has(d.date)) byDate.set(d.date, { date: d.date, weekday: d.weekday, units: [] });
        byDate.get(d.date).units.push({ name: u.name, day: d.number, pages: d.pages, time: d.time, lines: d.lines.map(lineOf) });
      }
    }
    out.units = [...byDate.values()].sort((a, b) => (a.date < b.date ? -1 : 1));
    if (!out.units.length) out.empty = 'No sub-board of this board has a dated shoot day in the report\'s range.';
  } else if (r.kind === 'calendar') {
    const names = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const order = weekStart === 'mon' ? [1, 2, 3, 4, 5, 6, 0] : [0, 1, 2, 3, 4, 5, 6];
    const keep = (i) => !o.weekdays_only || (i >= 1 && i <= 5);
    out.week = order.filter(keep).map((i) => names[i]);
    if (!sb.start || !days.length) out.empty = `There are no dated shoot days on ${sb.name} yet: set its start date and put day breaks on the Board, or give the report a date range.`;
    else {
      const [y, mo] = (o.from || days[0].date).split('-').map(Number);
      const cal = calendarView(sub, y, mo);
      const byDay = new Map(days.map((d) => [d.date, d]));
      const flat = cal.weeks.flat();
      const startAt = flat.findIndex((d) => names.indexOf(d.weekday) === order[0]);
      const cells = flat.slice(startAt);
      const weeks = [];
      for (let w = 0; w * 7 < cells.length && w < 6; w++) weeks.push(cells.slice(w * 7, w * 7 + 7).filter((d) => keep(names.indexOf(d.weekday))).map((d) => {
        const sd = byDay.get(d.date);
        return { date: d.date, day: d.day, in_month: d.in_month, kind: d.kind, shoot_day: d.shoot_day, scenes: sd ? sd.lines.filter((l) => l.kind === 'scene').map((l) => l.scenes) : [], pages: sd ? sd.pages : '', time: sd ? sd.time : '', events: d.events.map((i) => cal.events[i].name || cal.events[i].kind), flags: d.red_flags || [] };
      }));
      out.months = [{ title: cal.title, weeks }];
    }
  } else if (r.kind === 'conflicts') {
    out.conflicts = fakeConflicts(m);
    if (!out.conflicts.conflicts.length && !out.conflicts.on_work.length && !out.conflicts.event_clashes.length) out.empty = `${boardName(m)} has no conflicts.`;
  } else if (r.kind === 'red_flags') {
    out.red_flags = commands.red_flags_view({ sub_board: sub });
    out.red_flags.clashes = out.red_flags.clashes.filter((x) => inRange(o, x.date));
    out.red_flags.categories = [];
    if (!out.red_flags.clashes.length && !out.red_flags.rows.length) out.empty = out.red_flags.flags ? `No red flags or clashes on ${sb.name} meet the report's filters.` : 'The schedule has no red flags yet.';
  } else if (r.kind === 'dood') {
    for (const part of out.dood) part.grid = { ...part.grid, codes: codesOf(m), no_holds: o.no_holds };
  }
  // Pages per day or week, the total line and category columns (src/reports.rs).
  const weekOf = (iso) => Math.floor((Date.parse(`${iso}T00:00:00Z`) / 86400000 + (weekStart === 'mon' ? 3 : 4)) / 7);
  const mark = (list) => {
    let last = null;
    list.forEach((d, k) => {
      if (o.page_break === 'day') d.new_page = k > 0;
      else if (o.page_break === 'week' && d.date) {
        const w = weekOf(d.date);
        d.new_page = last !== null && last !== w;
        last = w;
      } else d.new_page = false;
    });
  };
  mark(out.days || []);
  mark(out.blocks || []);
  if (o.total_count) {
    const n = (out.days || []).reduce((a, d) => a + d.lines.filter((l) => l.kind === 'scene').length, 0);
    if ((out.days || []).length) out.total = `Total: ${n === 1 ? '1 scene' : `${n} scenes`} on ${out.days.length === 1 ? '1 shoot day' : `${out.days.length} shoot days`}`;
    else if (out.blocks.length) out.total = `Total: ${out.blocks.length} scenes`;
  }
  for (const id of o.element_columns || []) {
    const c = m.categories.find((x) => x.id === id);
    if (!c) continue;
    out.layout.push({ field: 'elements', label: c.name, width: 'medium', line: 1, span: 1, align: 'auto' });
    const add = (l) => {
      if (l.kind === 'banner' || !l.cells) return;
      const g = (l.groups || []).find((x) => x.category === c.name);
      l.cells.push(g ? g.elements.join(', ') : '');
    };
    for (const d of out.days || []) d.lines.forEach(add);
    for (const b of out.blocks || []) add(b.line);
  }
  return out;
}

window.__printed = [];
window.__exported = [];
const cancelled = () => {
  const c = window.__cancel === true;
  window.__cancel = false;
  return c;
};

// ---- The Calendar tab

const FULL_MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const ISO = /^\d{4}-\d{2}-\d{2}$/;
const validDate = (v) => ISO.test(v) && new Date(`${v}T00:00:00Z`).toISOString().slice(0, 10) === v;

function calendarView(sub, year, month, span = false) {
  const m = model;
  const sb = subOf(m, sub);
  const c = calOf(sb);
  if ((year === null) !== (month === null) || (month !== null && (month < 1 || month > 12))) throw 'That is not a month.';
  if (year === null) [year, month] = (sb.start || new Date().toISOString().slice(0, 10)).split('-').map(Number);
  const first = new Date(Date.UTC(year, month - 1, 1));
  let gridStart = new Date(first);
  gridStart.setUTCDate(1 - first.getUTCDay());
  let weekCount = 6;
  let days = 0;
  for (const st of sb.strips) if (st.kind === 'break') days++;
  const numbers = new Map();
  if (sb.start) for (let n = 1; n <= days; n++) numbers.set(addDays(sb, n)[0], n);
  const name = (id) => {
    const e = m.elements.find((x) => x.id === id);
    return e ? (e.board_id ? `${e.board_id} ${e.name}` : e.name) : null;
  };
  const events = c.events.map((ev, index) => ({ ...ev, color: ev.color || null, index, element_names: ev.elements.map(name).filter(Boolean), counts: ['travel', 'fitting', 'rehearsal'].includes(ev.kind) }));
  const flagged = fakeRedFlags(m, sb);
  // The weeks view (src/view.rs, CalOpts::span): the shoot's span and the month, then the
  // events within 26 weeks of it; the others are counted (decision 135).
  let outside = 0;
  if (span) {
    const iso = (d) => d.toISOString().slice(0, 10);
    const day = (t) => new Date(`${t}T00:00:00Z`);
    const shift = (t, n) => {
      const d = day(t);
      d.setUTCDate(d.getUTCDate() + n);
      return iso(d);
    };
    const monthEnd = new Date(Date.UTC(year, month, 0));
    const lows = [iso(first), sb.start, ...[...numbers.keys()]].filter(Boolean).sort();
    const highs = [iso(monthEnd), sb.start, ...[...numbers.keys()]].filter(Boolean).sort();
    let [lo0, hi0] = [lows[0], highs[highs.length - 1]];
    const near = 26 * 7;
    for (const e of c.events) {
      if (e.to < shift(lows[0], -near) || e.from > shift(highs[highs.length - 1], near)) outside++;
      else {
        if (e.from < lo0) lo0 = e.from < shift(lows[0], -near) ? shift(lows[0], -near) : e.from;
        if (e.to > hi0) hi0 = e.to > shift(highs[highs.length - 1], near) ? shift(highs[highs.length - 1], near) : e.to;
      }
    }
    const lo = day(lo0);
    const hi = day(hi0);
    gridStart = new Date(lo);
    gridStart.setUTCDate(lo.getUTCDate() - lo.getUTCDay());
    weekCount = Math.min(110, Math.max(6, Math.floor((hi - gridStart) / 86400000 / 7) + 1));
  }
  const weeks = [];
  for (let w = 0; w < weekCount; w++) {
    const week = [];
    for (let d = 0; d < 7; d++) {
      const date = new Date(gridStart);
      date.setUTCDate(gridStart.getUTCDate() + w * 7 + d);
      const iso = date.toISOString().slice(0, 10);
      week.push({
        date: iso,
        day: date.getUTCDate(),
        weekday: WD[date.getUTCDay()],
        in_month: span || date.getUTCMonth() === month - 1,
        kind: kindOf(sb, iso),
        weekly_off: c.weekly_off.includes(WDKEY[date.getUTCDay()]),
        shoot_day: numbers.get(iso) || null,
        events: events.filter((ev) => ev.from <= iso && iso <= ev.to).map((ev) => ev.index),
        red_flags: flagged.filter((x) => x.date === iso).map((x) => x.text),
        red_flag_types: [...new Set(flagged.filter((x) => x.date === iso).map((x) => x.red_flag_type))],
        conflicts: fakeConflictMarks(model).dates.get(`${sb.id}|${iso}`) || [],
      });
    }
    weeks.push(week);
  }
  const all = [...numbers.keys()].sort();
  const shift = (by) => {
    const n = year * 12 + month - 1 + by;
    return [Math.floor(n / 12), (n % 12) + 1];
  };
  return {
    sub_board: sb.name,
    start: sb.start,
    year,
    month,
    title: `${FULL_MONTHS[month - 1]} ${year}`,
    outside_events: outside,
    weekly_off: WDKEY.slice(1).concat('sun').filter((k) => c.weekly_off.includes(k)),
    weeks,
    events,
    elements: m.categories.flatMap((cat) => listed(m, cat.name).map((e) => ({ id: e.id, name: e.name, board_id: e.board_id, category: cat.name }))),
    first_day: all[0] || null,
    last_day: all[all.length - 1] || null,
    prev: shift(-1),
    next: shift(1),
  };
}

/** The element and every element its links reach, each once (model::link_chain, decision 116). */
function linkChain(m, id) {
  const out = [id];
  const walk = (x) => {
    const e = m.elements.find((y) => y.id === x);
    for (const l of e ? e.links : []) if (!out.includes(l)) { out.push(l); walk(l); }
  };
  walk(id);
  return out;
}

const HEADERS = [
  ['set', 'Set', (s) => s.set],
  ['location', 'Location', (s) => s.f.location || ''],
  ['unit', 'Unit', (s) => s.f.unit || ''],
  ['episode', 'Episode', (s) => s.f.episode || ''],
  ['int_ext', 'I/E', (s) => s.ie || ''],
  ['time_of_day', 'Time of day', (s) => s.tod || ''],
  ['script_day', 'Script day', (s) => s.f.script_day || ''],
  ['sequence', 'Sequence', (s) => s.f.sequence || ''],
];
const headerValues = (m, key) => {
  const get = HEADERS.find((h) => h[0] === key)[2];
  const out = [];
  for (const s of scenesInOrder(m)) {
    const v = (get(s) || '').trim();
    if (v && !out.some((x) => same(x, v))) out.push(v);
  }
  return out.sort((a, b) => a.toUpperCase().localeCompare(b.toUpperCase()));
};
const headersDto = (m) => HEADERS.map(([key, label]) => ({ key, label, values: headerValues(m, key).length }));

function elementDays(element, sub) {
  const e = needEl(model, element);
  const sb = subOf(model, sub);
  const out = { element, name: e.board_id ? `${e.board_id} ${e.name}` : e.name, sub_board: sb.name, days: [], empty: null };
  if (!sb.start) return { ...out, empty: `Set a start date for ${sb.name} to see its days and dates.` };
  const cat = model.categories.find((c) => c.name === e.category);
  const row = doodGrid(cat.id, sub).rows.find((r) => r.element === element);
  if (!row) return { ...out, empty: `${e.name} has no work days or events on ${sb.name}.` };
  const grid = doodGrid(cat.id, sub);
  for (let i = row.span[0]; i <= row.span[1]; i++) {
    const d = grid.dates[i];
    const code = row.cells[i];
    out.days.push({ date: d.date, weekday: d.weekday, shoot_day: d.shoot_day, code, text: code ? 'works' : 'counts nowhere' });
  }
  return out;
}

function elementsView(category, sub, header = null) {
  const m = model;
  const users = m.categories.filter((c) => !isBuiltIn(c));
  const chosen = m.categories.find((c) => c.id === category) || m.categories[0];
  const sb = subOf(m, sub);
  const scenes = scenesInOrder(m);
  const scheduled = sb.strips.filter((x) => x.kind === 'scene');
  const idName = (e) => (e.board_id ? `${e.board_id} ${e.name}` : e.name);
  const hk = HEADERS.find((h) => h[0] === header);
  if (hk) {
    const get = hk[2];
    return {
      ...elementsView(category, sub),
      category: null,
      header: hk[0],
      all_elements: [],
      elements: headerValues(m, hk[0]).map((v, i) => {
        const on = scenes.filter((x) => same(get(x) || '', v));
        const days = sb.start ? scheduled.filter((x) => same(get(x) || '', v)).length : 0;
        return { id: 5000 + i, board_id: '', locked: false, exclude_from_board: false, name: v, sheets: on.length, pages: pages(on.reduce((a, x) => a + x.eighths, 0)), days, start: days ? sb.start : null, finish: days ? sb.start : null, events: 0, conflicts: 0, holds: false, drops: false, min_drop_gap: '', notes: '', properties: [], links: [], link_names: [], linked_by: [], linked: false, images: [], red_flags: [] };
      }),
    };
  }
  return {
    headers: headersDto(m),
    header: null,
    all_elements: m.categories.flatMap((cat) => listed(m, cat.name).map((e) => ({ id: e.id, name: e.name, board_id: e.board_id, category: cat.name }))),
    categories: m.categories.map((c) => ({
      id: c.id,
      name: c.name,
      built_in: isBuiltIn(c),
      place: isBuiltIn(c) ? null : users.indexOf(c),
      user_count: users.length,
      auto_board_id: c.auto,
      exclude_from_board: !!c.exclude,
      daily_quantity: !!c.daily,
      properties: [...(c.props || [])],
      multi_line: [...(c.multi || [])],
      holds: c.holds,
      drops: c.drops,
      min_drop_gap: String(c.gap),
      elements: m.elements.filter((e) => e.category === c.name).length,
    })),
    category: chosen ? chosen.id : null,
    elements: chosen
      ? listed(m, chosen.name).map((e) => {
          const on = scenes.filter((x) => x.els.includes(e.id));
          const days = sb.start ? scheduled.filter((x) => x.els.includes(e.id)).length : 0;
          return {
            id: e.id,
            board_id: e.board_id,
            locked: e.locked,
            exclude_from_board: !!e.exclude_from_board,
            name: e.name,
            sheets: on.length,
            pages: pages(on.reduce((a, x) => a + x.eighths, 0)),
            days,
            start: days ? sb.start : null,
            finish: days ? sb.start : null,
            events: (calOf(sb).events || []).filter((ev) => ev.elements.includes(e.id)).length,
            conflicts: e.exclude_from_board ? 0 : (calOf(sb).events || []).filter((ev) => ev.elements.includes(e.id) && ['travel', 'fitting', 'rehearsal'].includes(ev.kind) && days > 0 && ev.from <= sb.start && sb.start <= ev.to).length,
            clashes: fakeRedFlags(m, sb).filter((c) => c.element === e.id).length,
            holds: e.holds,
            drops: e.drops,
            min_drop_gap: String(e.gap),
            notes: e.notes || '',
            properties: (chosen.props || []).map((p) => ({ name: p, value: ((e.props || []).find((v) => v.name === p) || { value: '' }).value, multi_line: (chosen.multi || []).includes(p) })),
            links: [...e.links],
            link_names: e.links.map((l) => m.elements.find((x) => x.id === l)).filter(Boolean).map(idName),
            linked_by: m.elements.filter((x) => x.links.includes(e.id)).map(idName),
            linked: e.links.length > 0 || m.elements.some((x) => x.links.includes(e.id)),
            images: [...(e.images || [])],
            red_flags: (e.red_flags || []).map((f) => flagDto(m, f)),
          };
        })
      : [],
    sub_board: sb.name,
    dates_note: sb.start ? null : `Set a start date for ${sb.name} to see work days and dates.`,
  };
}

// ---- Explain this count, What Changed and Merge Changes From (src/explain.rs,
// src/diffmerge.rs), roughly: explain_count retells the fake grid's row; the diff and
// merge are canned. window.__merged records what merge_apply was asked;
// window.__mergeUnclear = true makes the automatic base unclear; window.__cancel = true
// makes the next file dialog act cancelled.

function explainCount(element, sub) {
  // With window.__bigDood the synthetic rows stand in for elements.
  const big = window.__bigDood ? doodGrid(null, sub).rows.find((r) => r.element === element) : null;
  const el = big ? { name: big.name, category: 'Cast' } : model.elements.find((e) => e.id === element);
  if (!el) throw 'That element is no longer in the schedule.';
  const grid = doodGrid(model.categories.find((c) => c.name === el.category).id, sub);
  if (grid.empty) throw grid.empty;
  const row = grid.rows.find((r) => r.element === element);
  if (!row) throw `"${el.name}" works on no shoot day of ${grid.sub_board}.`;
  const days = [];
  const work = [];
  grid.dates.forEach((d, i) => {
    if (i < row.span[0] || i > row.span[1]) return;
    const code = row.cells[i];
    if (code) work.push(d.date);
    days.push({
      date: d.date,
      code,
      counted: code ? 'work' : 'nowhere',
      text: code ? `${d.weekday} ${d.date}, shoot day ${d.shoot_day}: works` : `${d.weekday} ${d.date}: ${d.kind === 'off' ? 'a day off, counts nowhere' : 'holds off, counts nowhere'}`,
    });
  });
  const gaps = [];
  for (let i = 1; i < work.length; i++) {
    const n = Math.round((Date.parse(work[i]) - Date.parse(work[i - 1])) / 86400000) - 1;
    if (n > 0) gaps.push({ after: work[i - 1], before: work[i], days: n, outcome: 'holds_off', text: `${n} day${n === 1 ? '' : 's'} between ${work[i - 1]} and ${work[i]}: counts nowhere: holds are off` });
  }
  return {
    element,
    name: el.name,
    category: el.category,
    sub_board: grid.sub_board,
    work: row.work,
    hold: row.hold,
    total: row.total,
    summary: [
      `"${el.name}" (${el.category}) on "${grid.sub_board}" (board "${boardName(model)}"): work ${row.work} + hold ${row.hold} = total ${row.total}, start ${row.start}, finish ${row.finish}.`,
      `Work ${row.work}: ${row.work} shoot day${row.work === 1 ? '' : 's'} worked.`,
      'Rules: holds off, drops off, minimum drop gap 1 day.',
    ],
    gaps,
    days,
  };
}

const CANNED_CHANGES = [
  { group: 'Schedule', changes: [{ kind: 'changed', subject: 'title', text: 'Title "Harbor Light" -> "Harbor Light II"' }] },
  { group: 'Elements', changes: [{ kind: 'added', subject: 'element', text: 'Element "DOG" (Cast) added' }] },
  { group: 'Sheets', changes: [{ kind: 'changed', subject: 'sheet', text: 'Scene 1: set "NET LOFT" -> "LOFT", elements +DOG' }] },
  { group: 'Strips', changes: [{ kind: 'moved', subject: 'strip', text: 'Board "Board 1": scene 3 moved on "Main" from position 4, day 2 to position 1, day 1' }] },
];

function whatChanged(against) {
  if (against === 'file') {
    if (cancelled()) return null;
    return { from: 'harbor-copy.mtsc', to: `${fileName} as it is now`, count: 4, groups: CANNED_CHANGES };
  }
  if (against !== 'saved') throw 'Unknown comparison.';
  const dirty = JSON.stringify(model) !== saved;
  return { from: `${fileName} as saved on disk`, to: `${fileName} as it is now`, count: dirty ? 1 : 0, groups: dirty ? [CANNED_CHANGES[0]] : [] };
}

const MERGE_BASES = [
  { id: 'harbor 2026-10-11 14.03.22.123 UTC.mtsc', label: 'Backup from 2026-10-11 14:03 UTC' },
  { id: 'harbor 2026-10-10 09.12.00.001 UTC.mtsc', label: 'Backup from 2026-10-10 09:12 UTC' },
];
const MERGE_CONFLICTS = [
  { key: 'sheet:4:set', what: 'Scene 1 set', base: '"NET LOFT"', ours: '"LOFT"', theirs: '"BARN"' },
  { key: 'element:101', what: 'Element "TOVE" (Cast)', base: null, ours: 'deleted it', theirs: 'changed it' },
];

function mergePreview(token, base, picks) {
  if (token !== 1) throw 'That file is no longer at hand; choose it again.';
  const bases = [...MERGE_BASES, { id: 'disk', label: `${fileName} on disk now` }];
  let id;
  let label;
  let reason;
  let clear = true;
  if (base.kind === 'auto') {
    if (window.__mergeUnclear) {
      id = 'disk';
      clear = false;
      reason = `Every backup of ${fileName} is newer than harbor-copy.mtsc (last changed 2026-10-09 08:00 UTC), so none is sure to come before both versions. The base is ${fileName} as it is on disk now. Anything saved into it after harbor-copy.mtsc was copied will read as undone in theirs, so check what will change, or pick a backup.`;
    } else {
      id = MERGE_BASES[0].id;
      reason = `The newest backup of ${fileName} from before both versions: it was made 2026-10-11 14:03 UTC, harbor-copy.mtsc was last changed after that (2026-10-11 15:20 UTC), and this schedule has been saved since.`;
    }
  } else if (base.kind === 'disk') {
    id = 'disk';
    reason = 'You picked the file on disk.';
  } else if (base.kind === 'backup' && MERGE_BASES.some((b) => b.id === base.name)) {
    id = base.name;
    reason = 'You picked this backup.';
  } else {
    return { theirs: 'harbor-copy.mtsc', ours: fileName, base: '', base_label: '', base_reason: '', base_clear: true, bases, conflicts: [], count: 0, changes: [], renumbered: 0, fallbacks: [], problem: 'The base can\'t be read: That backup is no longer in the backups folder.' };
  }
  label = bases.find((b) => b.id === id).label;
  const conflicts = MERGE_CONFLICTS.map((c) => {
    const taken = picks[c.key] === 'theirs' ? 'theirs' : 'ours';
    return { ...c, taken, text: `${c.what}: ours ${c.ours}, theirs ${c.theirs}` };
  });
  const changes = [CANNED_CHANGES[1]];
  if (picks['sheet:4:set'] === 'theirs') changes.push({ group: 'Sheets', changes: [{ kind: 'changed', subject: 'sheet', text: 'Scene 1: set "LOFT" -> "BARN"' }] });
  const count = changes.reduce((n, g) => n + g.changes.length, 0);
  return { theirs: 'harbor-copy.mtsc', ours: fileName, base: id, base_label: label, base_reason: reason, base_clear: clear, bases, conflicts, count, changes, renumbered: 0, fallbacks: ['Element "DOG" (Cast): the name collides with one in ours, so it joins it'], problem: null };
}

window.__merged = [];

const commands = {
  // window.__bigView, when set before the page loads, stands in for the real view.
  get_view: () => window.__bigView || view(),
  dood_grid: ({ sub_board, category, header, sort, no_holds }) => {
    const g = doodGrid(category, sub_board, header || null);
    const keys = sort || [];
    const KEYS = ['board_id', 'name', 'start', 'finish', 'work', 'hold'];
    for (const k of keys) if (!KEYS.includes(k.split(':')[0])) throw `"${k}" is not a sort key.`;
    for (const r of g.rows) {
      const e = model.elements.find((x) => x.id === r.element);
      r.holds = Boolean(e && e.holds);
      r.drops = Boolean(e && e.drops);
    }
    const val = (r, k) => (k === 'board_id' ? r.board_id || null : k === 'name' ? r.name : r[k] === undefined ? null : r[k]);
    g.rows.sort((a, b) => {
      for (const k of keys) {
        const [key, dir] = k.split(':');
        const [x, y] = [val(a, key), val(b, key)];
        if (x === y) continue;
        if (x === null || x === '') return 1;
        if (y === null || y === '') return -1;
        const c = key === 'board_id' ? natural(x, y) : x < y ? -1 : 1;
        return dir === 'desc' ? -c : c;
      }
      return 0;
    });
    const codes = codesOf(model);
    const letter = Object.fromEntries(codes.map(([k, , c]) => [k, c]));
    for (const r of g.rows) r.cells = r.cells.map((c) => c.replace(/^S/, letter.start).replace(/W/, letter.work).replace(/F$/, letter.finish).replace(/^H$/, letter.hold));
    if (no_holds) for (const r of g.rows) {
      r.cells = r.cells.map((c) => (c === letter.hold ? '' : c));
      r.total -= r.hold;
      r.hold = 0;
    }
    return { ...g, codes, no_holds: Boolean(no_holds) };
  },
  explain_count: ({ sub_board, element }) => explainCount(element, sub_board),
  find_text: ({ query }) => findText(query),
  replace_text: ({ query, with: withText, picks }) => replaceText(query, withText, picks),
  red_flags_view: ({ sub_board }) => {
    const sb = subOf(model, sub_board);
    const clashes = fakeRedFlags(model, sb);
    return {
      board: model.shown,
      board_name: boardName(model),
      sub_board: sb.id,
      sub_board_name: sb.name,
      start: sb.start || null,
      count: clashes.length,
      clashes,
      undated: !sb.start,
      flags: model.elements.reduce((a, e) => a + (e.red_flags || []).length, 0),
      truncated: false,
      rows: flagRows(model),
      categories: model.categories.map((c) => ({ id: c.id, name: c.name, elements: model.elements.filter((e) => e.category === c.name).map((e) => ({ id: e.id, name: e.name, board_id: e.board_id || '' })) })),
    };
  },
  add_red_flags: ({ flags }) => {
    const keys = Object.keys(flags).sort().join(',');
    if (keys !== 'dates,elements,from,hard,note,picked,red_flag_type,to') throw `fake: add_red_flags got ${keys}`;
    if (!flags.elements.length) throw 'Check at least one element to flag.';
    for (const id of flags.elements) needEl(model, id);
    if (!model.redFlagTypes.some((x) => x.id === flags.red_flag_type)) throw 'That red flag type is no longer in the schedule.';
    const date = (what, v) => {
      const t = String(v).trim();
      if (!t) throw `Type the ${what} date, as 3/1/2027 or 2027-03-01.`;
      const iso = typedDate(t);
      if (!iso) throw notADate(`The ${what} date`, t);
      return iso;
    };
    const ranges = [];
    if (flags.dates === 'picked') {
      const ds = [...new Set(flags.picked.map((d) => date('picked', d)))].sort();
      if (!ds.length) throw 'Pick at least one date on the calendar.';
      for (const d of ds) {
        const last = ranges[ranges.length - 1];
        if (last && shiftDate(last.to, 1) === d) last.to = d;
        else ranges.push({ dates: 'range', from: d, to: d });
      }
    } else if (flags.dates === 'range') {
      const from = date('first', flags.from);
      const to = String(flags.to).trim() ? date('last', flags.to) : from;
      if (to < from) throw "The last date is before the first. A red flag can't end before it starts.";
      ranges.push({ dates: 'range', from, to });
    } else if (flags.dates === 'not_before' || flags.dates === 'not_after') {
      ranges.push({ dates: flags.dates, from: date(flags.dates.replace('_', ' '), flags.from), to: '' });
    } else throw 'A red flag\'s dates are a range, not before a date, or not after one.';
    return edit((m) => {
      for (const id of [...new Set(flags.elements)]) {
        const e = needEl(m, id);
        e.red_flags = e.red_flags || [];
        for (const r of ranges) e.red_flags.push({ id: nextId++, red_flag_type: flags.red_flag_type, ...r, note: String(flags.note).trim(), hard: Boolean(flags.hard) });
      }
    });
  },
  delete_red_flags: ({ flags }) => {
    if (!flags.length) throw 'Check the red flags to delete first.';
    for (const id of flags) if (!model.elements.some((e) => (e.red_flags || []).some((f) => f.id === id))) throw 'That red flag is no longer in the schedule.';
    return edit((m) => {
      for (const e of m.elements) e.red_flags = (e.red_flags || []).filter((f) => !flags.includes(f.id));
    });
  },
  add_standard_red_flag_types: () => {
    if (STANDARD_TYPES.every(([k]) => model.redFlagTypes.some((t) => t.builtin === k))) throw 'The schedule already has every standard type.';
    return edit((m) => {
      for (const [k, name, code] of STANDARD_TYPES) {
        if (m.redFlagTypes.some((t) => t.builtin === k)) continue;
        const own = m.redFlagTypes.find((t) => !t.builtin && same(t.name, name));
        if (own) Object.assign(own, { builtin: k, event_code: code });
        else m.redFlagTypes.push({ id: nextId++, name, builtin: k, event_code: code });
      }
    });
  },
  set_red_flag: ({ element, flag }) => {
    const e = needEl(model, element);
    const keys = Object.keys(flag).sort().join(',');
    if (keys !== 'dates,from,hard,id,note,red_flag_type,to') throw `fake: set_red_flag got ${keys}`;
    if (!model.redFlagTypes.some((x) => x.id === flag.red_flag_type)) throw 'That red flag type is no longer in the schedule.';
    const date = (what, v) => {
      const s = String(v).trim();
      if (!s) throw `Type the ${what} date, as 3/1/2027 or 2027-03-01.`;
      const iso = typedDate(s);
      if (!iso) throw notADate(`The ${what} date`, s);
      return iso;
    };
    let from;
    let to = '';
    if (flag.dates === 'range') {
      from = date('first', flag.from);
      to = date('last', flag.to);
      if (to < from) throw "The last date is before the first. A red flag can't end before it starts.";
    } else if (flag.dates === 'not_before') from = date('not before', flag.from);
    else if (flag.dates === 'not_after') from = date('not after', flag.from);
    else throw 'A red flag\'s dates are a range, not before a date, or not after one.';
    if (flag.id !== null && !(e.red_flags || []).some((f) => f.id === flag.id)) {
      if (model.elements.some((x) => (x.red_flags || []).some((f) => f.id === flag.id))) throw 'That red flag belongs to another element.';
      throw 'That red flag is no longer in the schedule.';
    }
    const f = { id: flag.id ?? nextId++, red_flag_type: flag.red_flag_type, dates: flag.dates, from, to, note: String(flag.note).trim(), hard: Boolean(flag.hard) };
    return edit((m) => {
      const me = needEl(m, element);
      me.red_flags = me.red_flags || [];
      const at = me.red_flags.findIndex((x) => x.id === f.id);
      if (at < 0) me.red_flags.push(f);
      else me.red_flags[at] = f;
    });
  },
  delete_red_flag: ({ flag }) => {
    if (!model.elements.some((e) => (e.red_flags || []).some((f) => f.id === flag))) throw 'That red flag is no longer in the schedule.';
    return edit((m) => {
      for (const e of m.elements) e.red_flags = (e.red_flags || []).filter((f) => f.id !== flag);
    });
  },
  add_red_flag_type: ({ name }) => {
    const n = String(name).trim();
    if (!n) throw 'Type a name for the red flag type.';
    if (model.redFlagTypes.some((x) => same(x.name, n))) throw `There is already a red flag type called ${n}.`;
    const id = nextId++;
    return edit((m) => m.redFlagTypes.push({ id, name: n, builtin: null, event_code: null }));
  },
  rename_red_flag_type: ({ red_flag_type, name }) => {
    if (!model.redFlagTypes.some((x) => x.id === red_flag_type)) throw 'That red flag type is no longer in the schedule.';
    const n = String(name).trim();
    if (!n) throw 'Type a name for the red flag type.';
    if (model.redFlagTypes.some((x) => x.id !== red_flag_type && same(x.name, n))) throw `There is already a red flag type called ${n}.`;
    return edit((m) => (m.redFlagTypes.find((x) => x.id === red_flag_type).name = n));
  },
  move_red_flag_type: ({ red_flag_type, to }) => {
    const from = model.redFlagTypes.findIndex((x) => x.id === red_flag_type);
    if (from < 0) throw 'That red flag type is no longer in the schedule.';
    return edit((m) => {
      const [t] = m.redFlagTypes.splice(from, 1);
      m.redFlagTypes.splice(Math.min(to, m.redFlagTypes.length), 0, t);
    });
  },
  delete_red_flag_type: ({ red_flag_type }) => {
    const t = model.redFlagTypes.find((x) => x.id === red_flag_type);
    if (!t) throw 'That red flag type is no longer in the schedule.';
    if (t.builtin) throw `${t.name} is a standard type, so it can't be deleted. It can be renamed and recolored.`;
    if (model.elements.some((e) => (e.red_flags || []).some((f) => f.red_flag_type === red_flag_type)))
      throw `${t.name} is used by red flags, so it can't be deleted. Change or delete those flags first.`;
    return edit((m) => (m.redFlagTypes = m.redFlagTypes.filter((x) => x.id !== red_flag_type)));
  },
  conflicts_view: ({ board }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    return fakeConflicts(model);
  },
  what_changed: ({ against }) => whatChanged(against),
  merge_pick_file: () => {
    if (window.__mergeUntitled) throw 'Save this schedule first: a merge needs the version both copies started from, and a schedule never saved has no file or backups to find it in.';
    if (cancelled()) return null;
    return { token: 1, name: 'harbor-copy.mtsc' };
  },
  merge_preview: ({ token, base, picks }) => mergePreview(token, base, picks || {}),
  merge_apply: ({ token, base, picks }) => {
    const p = mergePreview(token, base, picks || {});
    if (p.problem) throw p.problem;
    window.__merged.push({ token, base, picks });
    edit((m) => {
      m.title = `${m.title} (merged)`;
    });
    return view('Merged the changes from harbor-copy.mtsc. Undo takes the whole merge back.');
  },
  // Outside changes (src/outside.rs): none unless window.__outside holds the dto to offer.
  outside_check: () => window.__outside || null,
  outside_resolve: ({ choice }) => {
    window.__outside = null;
    if (choice === 'load_theirs') return view('Loaded harbor.mtsc as it is on disk.');
    if (choice === 'keep_mine') return view('Kept yours and saved it into harbor.mtsc.');
    return view();
  },
  backup_list: () => ({ file: 'harbor.mtsc', backups: [{ name: 'harbor 2026-10-11 14.03.00.000 UTC.mtsc', label: 'Backup from 2026-10-11 14:03 UTC', changes: 2 }] }),
  revert_to_backup: () => view('Reverted to the backup from 2026-10-11 14:03 UTC. Undo takes it back.'),
  report_data: ({ sub_board, index, week_start }) => reportDataFor(index, sub_board, week_start || 'sun'),
  report_list: () => reportListDto(),
  // In Rust's order and words (src/reportsetup.rs apply_report).
  save_report: ({ index, name, options }) => {
    if (!reportList(model)[index]) throw 'That report is no longer in the list.';
    if (!String(name).trim()) throw 'A report needs a name.';
    if ([...String(name).trim()].length > 200) throw 'A report name is 200 characters at most.';
    for (const k of ['from', 'to']) if (options[k] && !typedDateFake(options[k])) throw `The ${k === 'from' ? 'From' : 'To'} date "${options[k]}" is not a date. Type it as 3/1/2027 or 2027-03-01.`;
    const f = typedDateFake(options.from);
    const t = typedDateFake(options.to);
    if (f && t && t < f) throw `The date range ends (${t}) before it starts (${f}).`;
    const columns = (options.columns || []).map(reportCell);
    const first = columns.filter((c) => c.line === 1).length;
    if (columns.length && !first) throw "The report's columns: a strip layout needs a field on its first line.";
    for (const n of [2, 3, 4]) {
      const wide = columns.filter((c) => c.line === n).reduce((a, c) => a + c.span, 0);
      if (wide > first) throw `The report's columns: line ${n} covers ${wide} columns but the first line has ${first}.`;
    }
    return reportEdit((l) => {
      l[index].name = String(name).trim();
      l[index].options = { ...DEFAULT_OPTIONS, ...options, columns, from: f || '', to: t || '' };
    });
  },
  duplicate_report: ({ index }) => {
    let made = '';
    const v = reportEdit((l) => {
      const r = l[index];
      if (!r) throw 'That report is not in the list.';
      let nameNow = `${r.name} copy`;
      for (let n = 2; l.some((x) => x.name.toLowerCase() === nameNow.toLowerCase()); n++) nameNow = `${r.name} copy ${n}`;
      made = nameNow;
      l.splice(index + 1, 0, { name: nameNow, kind: r.kind, options: { ...r.options } });
    });
    return { ...v, notice: `Added ${made}.` };
  },
  move_report: ({ index, to }) =>
    reportEdit((l) => {
      if (!l[index] || to < 0 || to >= l.length) throw 'That report is not in the list.';
      l.splice(to, 0, ...l.splice(index, 1));
    }),
  delete_report: ({ index }) =>
    reportEdit((l) => {
      const r = l[index];
      if (!r) throw 'That report is not in the list.';
      if (l.filter((x) => x.kind === r.kind).length < 2) throw `${r.name} can't be deleted: it is the last of its kind.`;
      l.splice(index, 1);
    }),
  save_header_layout: ({ design }) => {
    const list = headerList(model);
    const name = String(design.name || '').trim();
    if (!name) throw 'A header layout needs a name.';
    if (list.some((h) => h.id !== design.id && h.name.toLowerCase() === name.toLowerCase())) throw `A header layout named ${JSON.stringify(name)} is already in the list.`;
    if ((design.rows || []).length > 3) throw 'A header has 3 rows at most.';
    return edit((m) => {
      const next = headerList(m);
      const h = { ...design, name };
      // Past every layout's id and every id a report still picks (headers.rs add_header).
      if (!design.id) next.push({ ...h, id: Math.max(3, ...next.map((x) => x.id), ...reportList(m).map((r) => r.options.header || 0)) + 1 });
      else {
        const at = next.findIndex((x) => x.id === design.id);
        if (at < 0) throw 'No such header layout.';
        next[at] = h;
      }
      m.headers = next;
    });
  },
  delete_header_layout: ({ id }) =>
    edit((m) => {
      const next = headerList(m);
      if (next.length < 2) throw "The last header layout can't be deleted.";
      m.headers = next.filter((h) => h.id !== id);
    }),
  move_header_layout: ({ id, to }) =>
    edit((m) => {
      const next = headerList(m);
      const at = next.findIndex((h) => h.id === id);
      if (at < 0) throw 'No such header layout.';
      const [h] = next.splice(at, 1);
      next.splice(Math.min(to, next.length), 0, h);
      m.headers = next;
    }),
  set_production: ({ field, value }) => {
    if (field !== 'title' && !PRODUCTION_FIELDS.some(([k]) => k === field)) throw `"${field}" is not a production field.`;
    if (field === 'title' && !String(value).trim()) throw 'The schedule needs a title.';
    return edit((m) => {
      if (field === 'title') m.title = String(value).trim();
      else m.production = { ...(m.production || {}), [field]: String(value).trim() };
    });
  },
  set_dood_codes: ({ codes }) => {
    const next = Object.fromEntries(codesOf(model).map(([k, , c]) => [k, c]));
    for (const [k, v] of Object.entries(codes)) {
      if (!(k in next)) throw `"${k}" is not a day out of days code.`;
      next[k] = String(v).trim();
    }
    const seen = new Set();
    for (const [k, v] of Object.entries(next)) {
      // As DoodCodes::problem: the length, then the characters, then a code used twice.
      const label = k.replace('company_move', 'company travel');
      if ([...v].length < 1 || [...v].length > 3) throw `The ${label} code must be 1 to 3 characters.`;
      if (!/^[\p{L}\p{N}/]+$/u.test(v)) throw `The ${label} code ${JSON.stringify(v)} may hold only letters, digits and /.`;
      if (seen.has(v.toUpperCase())) throw `The code ${JSON.stringify(v)} is used twice.`;
      seen.add(v.toUpperCase());
    }
    return edit((m) => {
      m.codes = next;
    });
  },
  set_calendar_hidden: ({ hidden }) => {
    const keys = ['day_labels', 'pages', 'time', 'scenes', 'events', 'strips', 'banners', 'red_flags'];
    for (const k of hidden) if (!keys.includes(k)) throw `${JSON.stringify(k)} is not something the calendar shows.`;
    return edit((m) => {
      m.calendarHidden = keys.filter((k) => hidden.includes(k));
    });
  },
  set_calendar_fields: ({ fields }) => {
    if (!fields.length) throw 'A calendar strip shows at least one field.';
    if (new Set(fields).size !== fields.length) throw 'Each field can show once on a calendar strip.';
    for (const x of fields) if (!LAYOUT.some((l) => l[0] === x) || x === 'spacer' || x === 'text') throw `"${x}" is not a field.`;
    return edit((m) => {
      m.calendarFields = fields.join() === 'scene,set,elements,cast' ? null : [...fields];
    });
  },
  set_prep_wrap: ({ sub_board, prep_start, wrap_end }) => {
    const p = prep_start.trim() ? typedDateFake(prep_start) : '';
    const w = wrap_end.trim() ? typedDateFake(wrap_end) : '';
    if (p === null) throw notADate('The prep start', prep_start);
    if (w === null) throw notADate('The wrap end', wrap_end);
    if (p && w && w < p) throw `Wrap can't end (${w}) before prep starts (${p}).`;
    subOf(model, sub_board);
    return edit((m) => {
      const sb = subOf(m, sub_board);
      sb.prep = p || null;
      sb.wrap = w || null;
    });
  },
  // The host (src/host.rs, ui/js/host.js): window.__hostKind = 'web' plays the browser build.
  host_info: () => ({ kind: window.__hostKind || 'desktop', version: '0.3.0', commands_version: 1, read_only: false }),
  menu_table: () => [
    { title: 'File', items: [{ id: 'file.open', label: 'Open\u2026' }, null, { id: 'file.save', label: 'Save' }] },
    { title: 'Edit', items: [{ id: 'edit.undo', label: 'Undo' }] },
  ],
  preferences: () => ({ ...fakePrefs }),
  set_preferences: ({ prefs }) => {
    if (!['sun', 'mon'].includes(prefs.week_start)) throw `"${prefs.week_start}" is not a week start (sun or mon).`;
    if (!['iso', 'us', 'long'].includes(prefs.date_format)) throw `"${prefs.date_format}" is not a date format.`;
    if (!prefs.calendar_fields.length) throw 'A calendar strip shows at least one field.';
    if (!(prefs.fountain_lines >= 40 && prefs.fountain_lines <= 70)) throw 'Fountain lines per page are 40 to 70.';
    if (!['paper', 'graphite', 'ledger'].includes(prefs.theme)) throw `"${prefs.theme}" is not a look the app has.`;
    fakePrefs = { ...prefs };
    // Every window hears it, as from Rust.
    setTimeout(() => window.__emit('prefs-changed', { ...fakePrefs }), 0);
    return { ...fakePrefs };
  },
  print_report: ({ pdf, landscape, title, subject }) => {
    // What the print operation would see: the pages in #print-root while <html> is printing.
    const root = document.getElementById('print-root');
    window.__printed.push({
      pdf,
      landscape,
      title,
      subject,
      printing: document.documentElement.classList.contains('printing'),
      pages: root ? root.querySelectorAll('.rpt-page').length : 0,
      footers: root ? [...root.querySelectorAll('.rpt-foot .right')].map((n) => n.textContent) : [],
    });
    if (cancelled()) return { supported: true, done: false, file: null };
    return { supported: true, done: true, file: pdf ? `${title.replace(/[/\\:*?"<>|]/g, '-')}.pdf` : null };
  },
  export_report: ({ sub_board, report, format }) => {
    subOf(model, sub_board);
    if (!['dood', 'one_line', 'one_line_fixed', 'elements', 'dood_grid', 'element_detail', 'breakdown', 'breakdown_day', 'board', 'totals', 'red_flags'].includes(report)) throw "That report can't be exported.";
    if (!['xlsx', 'csv'].includes(format)) throw "That export format isn't one the app knows.";
    window.__exported.push({ report, format, sub_board });
    if (cancelled()) return { done: false, file: null };
    const label = { dood: 'Day Out of Days', one_line: 'One-Line', one_line_fixed: 'One-Line', elements: 'Elements', dood_grid: 'Day Out of Days Grid', element_detail: 'Element Detail', breakdown: 'Breakdown', breakdown_day: 'Breakdown by Day', board: 'Board', totals: 'Daily Totals' }[report];
    return { done: true, file: `harbor - ${label}.${format}` };
  },
  export_budget: ({ sub_board }) => {
    const sb = subOf(model, sub_board);
    if (!sb.start) throw `Set a start date for ${sb.name} before exporting for a budget: the file counts days by date.`;
    window.__exported.push({ report: 'budget', sub_board });
    if (cancelled()) return { done: false, file: null };
    return { done: true, file: 'harbor - Budget Days.json' };
  },
  new_schedule: () => null,
  open_schedule: () => null,
  save_schedule: () => {
    saved = JSON.stringify(model);
    return view();
  },
  import_script: () => view('Added 5 scenes to the Boneyard.'),
  undo: () => {
    if (undo.length) {
      redo.push(JSON.stringify(model));
      model = JSON.parse(undo.pop());
    }
    return view();
  },
  redo: () => {
    if (redo.length) {
      undo.push(JSON.stringify(model));
      model = JSON.parse(redo.pop());
    }
    return view();
  },
  move_strips: ({ board, strips, sub_board, index }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const all = model.subs.flatMap((s) => s.strips).concat(model.boneyard);
    if (sub_board === null && all.some((s) => strips.includes(s.id) && s.kind === 'banner')) throw 'Banners go on a sub-board, not in the Boneyard.';
    if (sub_board !== null) subOf(model, sub_board);
    return edit((m) => {
      let at = index;
      const moving = [];
      for (const [sub, l] of [...m.subs.map((s) => [s.id, s.strips]), [null, m.boneyard]]) {
        const found = [];
        for (let i = l.length - 1; i >= 0; i--) {
          if (!strips.includes(l[i].id)) continue;
          if (sub === sub_board && i < index) at--;
          found.unshift(...l.splice(i, 1));
        }
        moving.push(...found);
      }
      const dest = list(m, sub_board);
      dest.splice(Math.min(at, dest.length), 0, ...moving);
    });
  },
  insert_day_break: ({ sub_board, index }) => {
    subOf(model, sub_board);
    const b = brk();
    const v = edit((m) => {
      const l = list(m, sub_board);
      l.splice(Math.min(index, l.length), 0, b);
    });
    return { ...v, strip: b.id };
  },
  delete_strips: ({ board, strips }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const all = model.subs.flatMap((s) => s.strips).concat(model.boneyard);
    const sheets = [];
    for (const id of strips) {
      const s = all.find((x) => x.id === id);
      if (!s) throw 'That strip is no longer on the board.';
      if (s.kind === 'scene' && all.filter((x) => x.kind === 'scene' && x.sheet === s.sheet).length < 2) sheets.push(s.sheet);
    }
    // A scene's sheet leaves every board (src/board.rs).
    const gone = (x) => strips.includes(x.id) || (x.kind === 'scene' && sheets.includes(x.sheet));
    return edit((m) => {
      for (const b of allBoards(m)) {
        for (const sb of b.subs) sb.strips = sb.strips.filter((x) => !gone(x));
        b.boneyard.splice(0, b.boneyard.length, ...b.boneyard.filter((x) => !gone(x)));
      }
    });
  },
  insert_banner: ({ sub_board, index, text, duration, style }) => {
    const minutes = bannerMinutes(text, duration);
    subOf(model, sub_board);
    const b = { id: nextId++, kind: 'banner', text: text.trim(), minutes, style: bannerStyle(style) };
    const v = edit((m) => {
      const l = list(m, sub_board);
      l.splice(Math.min(index, l.length), 0, b);
    });
    return { ...v, strip: b.id };
  },
  set_banner: ({ board, strip, text, duration, style }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const minutes = bannerMinutes(text, duration);
    const s = model.subs.flatMap((x) => x.strips).find((x) => x.id === strip);
    if (!s) throw 'That banner is no longer on the board.';
    if (s.kind !== 'banner') throw 'That strip is not a banner.';
    return edit((m) => Object.assign(m.subs.flatMap((x) => x.strips).find((x) => x.id === strip), { text: text.trim(), minutes }, style ? { style: bannerStyle(style) } : {}));
  },
  sort_strips: ({ board, sub_board, keys }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    if (!keys.length) throw 'Choose a field to sort by.';
    if (keys.length > 4) throw 'Sort by at most 4 fields.';
    return edit((m) => {
      const l = list(m, sub_board);
      const banners = l.filter((s) => s.kind === 'banner');
      const scenes = l.filter((s) => s.kind === 'scene');
      const value = (s, f) => (SORT_KEYS[f] ? s[SORT_KEYS[f]] : s.f[f] || '');
      scenes.sort((a, b) => {
        for (const k of keys) {
          const [x, y] = [value(a, k.field), value(b, k.field)];
          let o;
          if (k.field === 'pages') o = x - y;
          else if (!x || !y) {
            if (x || y) return x ? -1 : 1;
            continue;
          } else o = natural(String(x), String(y));
          if (o) return k.descending ? -o : o;
        }
        return 0;
      });
      l.splice(0, l.length, ...banners, ...scenes);
    });
  },
  show_board: ({ board }) => {
    needBoard(model, board);
    switchTo(model, board);
    return view();
  },
  add_board: ({ name }) => {
    const n = boardNameFree(model, name);
    const id = nextId++;
    edit((m) => {
      const scenes = m.subs.flatMap((s) => s.strips).concat(m.boneyard).filter((s) => s.kind === 'scene');
      m.boards.push({ id, name: n });
      // A new board's sub-board starts today (decision 153).
      m.store[id] = { subs: [{ id: nextId++, name: 'Main', start: new Date().toISOString().slice(0, 10), strips: [], cal: { weekly_off: ['sat', 'sun'], marks: {}, events: [] } }], boneyard: freshCopy(scenes) };
      switchTo(m, id);
    });
    return view();
  },
  duplicate_board: ({ board, name }) => {
    needBoard(model, board);
    const n = boardNameFree(model, name);
    const id = nextId++;
    edit((m) => {
      const from = listsOf(m, board);
      m.store[id] = { subs: from.subs.map((sb) => ({ ...JSON.parse(JSON.stringify(sb)), id: nextId++, strips: freshCopy(sb.strips) })), boneyard: freshCopy(from.boneyard) };
      m.boards.push({ id, name: n });
      if (m.layouts[board]) m.layouts[id] = JSON.parse(JSON.stringify(m.layouts[board]));
      switchTo(m, id);
    });
    return view();
  },
  rename_board: ({ board, name }) => {
    needBoard(model, board);
    const n = boardNameFree(model, name, board);
    return edit((m) => (needBoard(m, board).name = n));
  },
  move_board: ({ board, to }) => {
    needBoard(model, board);
    return edit((m) => {
      const at = m.boards.findIndex((b) => b.id === board);
      const [b] = m.boards.splice(at, 1);
      m.boards.splice(Math.min(to, m.boards.length), 0, b);
    });
  },
  delete_board: ({ board }) => {
    needBoard(model, board);
    if (model.boards.length === 1) throw 'A schedule keeps at least one board.';
    return edit((m) => {
      if (m.shown === board) switchTo(m, m.boards.find((b) => b.id !== board).id);
      delete m.store[board];
      m.boards = m.boards.filter((b) => b.id !== board);
    });
  },
  add_sub_board: ({ board, name }) => {
    needBoard(model, board);
    const n = subNameFree(listsOf(model, board).subs, name);
    const id = nextId++;
    // A new sub-board starts today (decision 123).
    const today = new Date().toISOString().slice(0, 10);
    const v = edit((m) => listsOf(m, board).subs.push({ id, name: n, start: today, strips: [], cal: { weekly_off: ['sat', 'sun'], marks: {}, events: [] } }));
    return { ...v, sub_board: id };
  },
  rename_sub_board: ({ sub_board, name }) => {
    const n = subNameFree(model.subs, name, subOf(model, sub_board).id);
    return edit((m) => (subOf(m, sub_board).name = n));
  },
  move_sub_board: ({ sub_board, to }) => {
    subOf(model, sub_board);
    return edit((m) => {
      const at = m.subs.findIndex((s) => s.id === sub_board);
      const [s] = m.subs.splice(at, 1);
      m.subs.splice(Math.min(to, m.subs.length), 0, s);
    });
  },
  delete_sub_board: ({ sub_board }) => {
    subOf(model, sub_board);
    if (model.subs.length === 1) throw 'A board keeps at least one sub-board.';
    return edit((m) => {
      const s = subOf(m, sub_board);
      m.subs = m.subs.filter((x) => x !== s);
      m.boneyard.push(...s.strips.filter((x) => x.kind === 'scene'));
    });
  },
  auto_day_breaks: ({ sub_board, by, limit }) => {
    const hm = (v, what) => {
      const m = /^(\d+):(\d\d)$/.exec(String(v).trim());
      if (!m || Number(m[1]) * 60 + Number(m[2]) === 0) throw `${what} per day must look like 1:30 and be more than 0:00.`;
      return Number(m[1]) * 60 + Number(m[2]);
    };
    let max;
    let weight;
    if (by === 'pages') {
      max = Number(limit) * 8;
      if (!(max > 0)) throw 'Pages per day: page length must look like 2, 3/8 or 1 3/8 (eighths only).';
      weight = (s) => s.eighths || 0;
    } else if (by === 'time') {
      max = hm(limit, 'Shoot time');
      weight = (s) => (s.kind === 'banner' ? s.minutes || 0 : hmOf(s.f.shoot_time));
    } else if (by === 'screen_time') {
      max = hm(limit, 'Screen time');
      weight = (s) => hmOf(s.f.screen_time);
    } else throw `${JSON.stringify(by)} is not a way to break days.`;
    let n = 0;
    const v = edit((m) => {
      const sb = m.subs.find((s) => s.id === sub_board);
      const out = [];
      let total = 0;
      for (const s of sb.strips.filter((x) => x.kind !== 'break')) {
        out.push(s);
        total += weight(s);
        if (total >= max && s.kind === 'scene') {
          out.push(brk());
          total = 0;
        }
      }
      if (out.length && out[out.length - 1].kind !== 'break') out.push(brk());
      sb.strips = out;
      n = out.filter((x) => x.kind === 'break').length;
    });
    return { ...v, notice: `Day breaks set: ${n === 1 ? '1 shoot day' : `${n} shoot days`}.` };
  },
  remove_day_breaks: ({ sub_board }) => {
    const sb = subOf(model, sub_board);
    const n = sb.strips.filter((x) => x.kind === 'break').length;
    if (!n) throw 'There are no day breaks to remove.';
    const v = edit((m) => {
      const x = m.subs.find((s) => s.id === sub_board);
      x.strips = x.strips.filter((s) => s.kind !== 'break');
    });
    return { ...v, notice: `Removed ${n === 1 ? '1 day break' : `${n} day breaks`}. Undo puts them back.` };
  },
  set_autosave: ({ on }) => {
    autosave = Boolean(on);
    setTimeout(() => window.__emit('autosave-changed', autosave), 0);
    return autosave;
  },
  open_pending: () => {
    const name = window.__pending;
    window.__pending = null;
    if (!name) return null;
    fileName = name;
    saved = JSON.stringify(model);
    return view();
  },
  open_recent: ({ index }) => {
    if (!recent[index]) return null;
    fileName = recent[index];
    saved = JSON.stringify(model);
    return view();
  },
  recovery_list: () => recovered,
  open_recovered: ({ key }) => {
    const e = recovered.find((r) => r.key === key);
    if (!e) throw 'That recovered schedule is gone.';
    recovered = recovered.filter((r) => r.key !== key);
    model.title = e.title;
    fileName = 'Untitled';
    saved = '';
    return view();
  },
  discard_recovered: ({ key }) => {
    recovered = recovered.filter((r) => r.key !== key);
    return recovered;
  },
  get_sheet: ({ board, sheet, index, order }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    if (order && !['board', 'number', 'scene'].includes(order)) throw 'Step by board order, sheet number or scene number.';
    return sheetDto(sheet, index, order || 'board');
  },
  // Split screen (src/split.rs): another board as the page draws it, the board shown kept.
  board_view: ({ board }) => {
    needBoard(model, board);
    const keep = model;
    model = JSON.parse(JSON.stringify(model));
    try {
      switchTo(model, board);
      return view().board;
    } finally {
      model = keep;
    }
  },
  // Images on scenes: the fake picks images/scene-<n>-<k>.png; the first "exists".
  add_sheet_image: ({ sheet }) => {
    if (cancelled()) return null;
    const s = needSheet(model, sheet);
    if ((s.images || []).length >= 20) throw 'A sheet holds at most 20 images.';
    return sheetEdit(sheet, (x) => {
      x.images = x.images || [];
      x.images.push(`images/scene-${String(x.n || x.sheet).toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${x.images.length + 1}.png`);
    });
  },
  remove_sheet_image: ({ sheet, at }) => {
    if (!(needSheet(model, sheet).images || [])[at]) throw 'That image is no longer on this sheet.';
    return sheetEdit(sheet, (x) => x.images.splice(at, 1));
  },
  sheet_image: ({ sheet, at }) => {
    if (!(needSheet(model, sheet).images || [])[at]) throw 'That image is no longer on this sheet.';
    return at === 0 ? 'data:image/gif;base64,R0lGODlhAQABAIAAAMzMzP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==' : null;
  },
  set_sheet_field_order: ({ fields, hidden, widths, rows }) => {
    for (const n of rows || []) if (!Number.isInteger(n) || n < 1) throw 'A sheet row holds at least one field.';
    for (const k of hidden || []) if (!SHEET_FIELDS.includes(k)) throw `"${k}" is not a sheet field.`;
    for (const [k, w] of Object.entries(widths || {})) if (!SHEET_FIELDS.includes(k) || !['narrow', 'medium', 'wide'].includes(w)) throw `"${w}" is not a field width.`;
    const seen = new Set();
    for (const f of fields) {
      if (!SHEET_FIELDS.includes(f) || seen.has(f)) throw 'Each field can be named once, and only the sheet\'s own fields.';
      seen.add(f);
    }
    const order = [...fields];
    for (const f of SHEET_FIELDS) if (!order.includes(f)) order.push(f);
    return edit((m) => {
      m.sheetFields = order.join() === SHEET_FIELDS.join() ? [] : order;
      if (hidden) m.sheetHidden = [...hidden];
      if (widths) m.sheetWidths = { ...widths };
      if (rows) m.sheetRows = [...rows];
    });
  },
  set_red_flag_type_color: ({ red_flag_type, color }) => {
    const v = String(color || '').trim();
    if (v && !/^#[0-9a-fA-F]{6}$/.test(v)) throw `${JSON.stringify(v)} is not a color like #cc3333.`;
    if (!model.redFlagTypes.some((t) => t.id === red_flag_type)) throw 'That red flag type is no longer in the schedule.';
    return edit((m) => {
      const t = m.redFlagTypes.find((x) => x.id === red_flag_type);
      t.color = v ? v.toLowerCase() : null;
    });
  },
  // Templates (src/templates.rs): the fake writes nothing and starts an untitled schedule.
  save_template: () => (cancelled() ? { done: false, file: null } : ((window.__template = true), { done: true, file: 'Harbor Light.mtst' })),
  // As Rust: a fresh untitled schedule in place of the open one (not an edit, so nothing to
  // undo and nothing unsaved), every board emptied of strips and events, and the notice.
  new_from_template: () => {
    if (cancelled()) return null;
    const m = JSON.parse(JSON.stringify(model));
    m.title = 'Untitled';
    for (const b of allBoards(m)) {
      for (const sb of b.subs) {
        sb.strips = [];
        calOf(sb).events = [];
      }
      b.boneyard.length = 0;
    }
    for (const e of m.elements) e.red_flags = [];
    model = m;
    undo.length = 0;
    redo.length = 0;
    clip = null;
    fileName = 'Untitled';
    saved = JSON.stringify(model);
    return view('A new schedule from the template Harbor Light.mtst.');
  },
  // The Help menu (src/help.rs). The fake keeps its own schedule for Open Sample (it can't
  // read the real sample) and a few lines of the guide, enough to see the dialog's parts.
  open_sample: () => null,
  // What's New, Licenses and Notices and the Help links (decision 212): short stand-ins;
  // open_link records what it was asked in window.__links.
  whats_new: () => ({ version: '0.1.0', notes: window.__whatsNew === undefined ? null : window.__whatsNew }),
  notices: () => ({ mit: 'MIT License\n\nCopyright (c) 2026 Method Production Suite Team and the Method Schedule contributors\n', apache: 'Apache License\nVersion 2.0, January 2004\n', third_party: '# Third-party notices\n\n### tauri 2.11.6\n\nLicense: MIT (offered: Apache-2.0 OR MIT).\n' }),
  // Updates (src/updater.rs, decision 213): window.__updateStatus sets what update_status
  // says; window.__emit('update-status', {...}) plays an event; update_whats_new gives
  // window.__updateNotes once.
  update_status: () => window.__updateStatus || { phase: 'idle', version: null, manual: false, said: null, automatic: true, available: false },
  update_restart: () => {
    if (!window.__updateStatus || window.__updateStatus.phase !== 'ready') throw 'No update is ready to install.';
    window.__restarted = true;
    return null;
  },
  update_whats_new: () => {
    const notes = window.__updateNotes || null;
    window.__updateNotes = null;
    return notes;
  },
  open_link: ({ which }) => {
    if (!['website', 'suite', 'bug'].includes(which)) throw "That isn't a link the app opens.";
    (window.__links ||= []).push(which);
    return null;
  },
  user_guide: () =>
    [
      '# Method Schedule user guide',
      '',
      'A short guide. The real one is docs/GUIDE.md.',
      '',
      '## Getting started',
      '',
      'Choose **Help > Open Sample**. It opens *Saltmarsh Road*, untitled.',
      '',
      '- **File > New** gives an empty schedule.',
      '- **File > Import Script** reads a `.fountain` or `.fdx`',
      '  script.',
      '',
      '## Keyboard shortcuts',
      '',
      'Choose **Help > Keyboard Shortcuts** (`⌘/`).',
    ].join('\n'),
  // Several windows (src/windows.rs): the fake counts what was asked.
  new_window: () => {
    window.__windows = (window.__windows || 0) + 1;
    return null;
  },
  open_in_new_window: () => {
    if (cancelled()) return false;
    window.__windows = (window.__windows || 0) + 1;
    return true;
  },
  close_window: () => {
    window.__closed = (window.__closed || 0) + 1;
    return null;
  },
  set_sheet_field: ({ sheet, field, value }) => {
    const v = String(value).trim();
    const rule = TIME_RULES[field];
    if (rule && v && !rule[0].test(v)) throw rule[1].replace('{v}', JSON.stringify(v));
    if (field === 'shoot_time' && v && hmMinutes(v) > MAX_DURATION) throw tooLong('Shoot time', v);
    // Decimal hours are kept as h:mm, as Rust keeps minutes (decisions.md, entry 81).
    const stored = field === 'shoot_time' && /^\d*\.\d{1,2}$/.test(v) ? hm(Math.floor(((Number(v.split('.')[0] || 0) * 100 + Number(v.split('.')[1].padEnd(2, '0'))) * 60 + 50) / 100)) : v;
    return sheetEdit(sheet, (s, m) => {
      // A one-value field takes another sheet's spelling, ignoring case (decision 155).
      const other = HEADER_KEYS.includes(field) && stored ? scenesInOrder(m).filter((x) => x.sheet !== s.sheet).map((x) => sheetGet(x, field).trim()).find((v) => v.toLowerCase() === stored.toLowerCase()) : null;
      sheetSet(s, field, other || stored);
    });
  },
  set_sheet_pages: ({ sheet, pages: p, eighths }) => {
    for (const [v, what] of [[p, 'Pages'], [eighths, 'Eighths']]) {
      if (v.trim() && !/^\d+$/.test(v.trim())) throw `${what} ${JSON.stringify(v.trim())} must be a whole number.`;
    }
    return sheetEdit(sheet, (s) => {
      s.eighths = Number(p || 0) * 8 + Number(eighths || 0);
    });
  },
  tag_element: ({ sheet, element }) =>
    sheetEdit(sheet, (s, m) => {
      // Adding the main element adds the ones it links to (behavior note 04).
      for (const id of linkChain(m, element)) if (!s.els.includes(id)) s.els.push(id);
    }),
  set_quantity: ({ sheet, element, count, daily }) => {
    const t = String(count).trim() || '1';
    if (!/^\d{1,5}$/.test(t) || Number(t) < 1) throw 'A quantity is a whole number from 1 to 99999.';
    if (!needSheet(model, sheet).els.includes(element)) throw 'That element is not on the sheet.';
    return sheetEdit(sheet, (s) => {
      s.qty = s.qty || {};
      if (Number(t) === 1 && daily) delete s.qty[element];
      else s.qty[element] = { count: Number(t), daily };
    });
  },
  create_element: ({ sheet, category, name }) => {
    if (!name.trim()) throw 'Type a name for the element.';
    if (!category.trim()) throw 'Choose a category for the new element.';
    return sheetEdit(sheet, (s, m) => {
      let c = m.categories.find((x) => x.name.toLowerCase() === category.trim().toLowerCase());
      if (!c) m.categories.push((c = { id: nextId++, name: category.trim(), auto: false, holds: false, drops: false, gap: 1 }));
      const cat = c.name;
      let e = m.elements.find((x) => x.category === cat && x.name.toLowerCase() === name.trim().toLowerCase());
      if (!e) m.elements.push((e = { id: nextId++, name: name.trim(), category: cat, board_id: '', locked: false, exclude_from_board: !!c.exclude, holds: c.holds, drops: c.drops, gap: c.gap, links: [], notes: '', props: [], images: [] }));
      for (const id of linkChain(m, e.id)) if (!s.els.includes(id)) s.els.push(id);
    });
  },
  untag_element: ({ sheet, element }) =>
    sheetEdit(sheet, (s) => {
      s.els = s.els.filter((x) => x !== element);
      if (s.qty) delete s.qty[element];
    }),
  new_sheet: ({ board, after }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const fresh = { id: nextId++, sheet: nextId++, kind: 'scene', n: '', ie: '', set: '', tod: '', eighths: 0, els: [], f: {} };
    const v = edit((m) => {
      const sub = after === null ? null : m.subs.find((x) => x.strips.some((st) => st.sheet === after));
      if (sub) sub.strips.splice(sub.strips.findIndex((st) => st.sheet === after) + 1, 0, fresh);
      else m.boneyard.push(fresh);
      for (const b of Object.values(m.store)) b.boneyard.push({ ...fresh, id: nextId++ });
    });
    return { ...v, sheet: fresh.sheet };
  },
  delete_sheet: ({ board, sheet, order: by }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const order = scenesBy(model, by || 'board').map((s) => s.sheet);
    const at = order.indexOf(sheet);
    if (at < 0) throw 'That sheet is no longer in the schedule.';
    const next = order[at + 1] !== undefined ? order[at + 1] : at > 0 ? order[at - 1] : null;
    const v = edit((m) => {
      for (const b of allBoards(m)) {
        for (const sb of b.subs) sb.strips = sb.strips.filter((x) => x.sheet !== sheet);
        b.boneyard.splice(0, b.boneyard.length, ...b.boneyard.filter((x) => x.sheet !== sheet));
      }
    });
    return { ...v, sheet: next };
  },
  merge_sheets_preview: ({ board, sheets }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    return fakeMerge(model, sheets).dto;
  },
  merge_sheets: ({ board, sheets }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const { survivor } = fakeMerge(model, sheets);
    const v = edit((m) => fakeMerge(m, sheets, true));
    const title = sceneOf(model, survivor).n;
    const strip = model.subs.flatMap((s) => s.strips).concat(model.boneyard).find((s) => s.sheet === survivor);
    return { ...v, notice: `Merged ${sheets.length} strips into one sheet: Scene ${title}.`, sheet: survivor, strip: strip ? strip.id : null };
  },
  duplicate_sheets: ({ board, sheets }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    if (!sheets.length) throw 'Select a scene to duplicate.';
    for (const s of sheets) needSheet(model, s);
    const order = scenesInOrder(model).map((s) => s.sheet);
    const ordered = [...sheets].sort((a, b) => order.indexOf(a) - order.indexOf(b));
    const copies = [];
    const v = edit((m) => {
      for (const sheet of ordered) {
        const copy = nextId++;
        copies.push(copy);
        for (const b of allBoards(m)) {
          const sub = b.subs.find((x) => x.strips.some((st) => st.sheet === sheet));
          const orig = b.subs.flatMap((x) => x.strips).concat(b.boneyard).find((st) => st.sheet === sheet);
          const fresh = { ...JSON.parse(JSON.stringify(orig)), id: nextId++, sheet: copy };
          if (sub) sub.strips.splice(sub.strips.findIndex((st) => st.sheet === sheet) + 1, 0, fresh);
          else b.boneyard.push(fresh);
        }
      }
    });
    const strips = copies.map((c) => model.subs.flatMap((s) => s.strips).concat(model.boneyard).find((s) => s.sheet === c).id);
    const notice = copies.length === 1 ? 'Duplicated the scene. The copy is selected.' : `Duplicated ${copies.length} scenes. The copies are selected.`;
    return { ...v, notice, strips, sheet: copies[copies.length - 1] };
  },
  continue_scene: ({ board, strip }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    const sb = model.subs.find((x) => x.strips.some((y) => y.id === strip));
    if (!sb) throw 'Schedule the scene on a sub-board before it spans days.';
    const i = sb.strips.findIndex((y) => y.id === strip);
    if (sb.strips[i].kind !== 'scene') throw 'Only a scene can span days.';
    const part = { ...JSON.parse(JSON.stringify(sb.strips[i])), id: nextId++ };
    const v = edit((m) => {
      const l = m.subs.find((x) => x.id === sb.id).strips;
      let j = l.findIndex((y, k) => k > i && y.kind === 'break');
      if (j < 0) {
        l.splice(i + 1, 0, brk());
        j = i + 1;
      }
      l.splice(j + 1, 0, part);
    });
    return { ...v, strip: part.id };
  },
  set_day_look: ({ board, look }) => {
    needBoard(model, board);
    const text = String(look.text || '').trim();
    if (text.length > 200) throw "A day break's text is at most 200 characters.";
    return edit((m) => {
      m.dayLooks = m.dayLooks || {};
      m.dayLooks[board] = { shoot_day_strip: Boolean(look.shoot_day_strip), screen_time: Boolean(look.screen_time), sunrise: Boolean(look.sunrise), sunset: Boolean(look.sunset), quantity: Boolean(look.quantity), text };
    });
  },
  set_strip_colors: ({ rules, scene_text, banner_text, break_text }) => {
    const hex = /^#[0-9a-fA-F]{6}$/;
    const out = [];
    for (const r of rules) {
      if (!hex.test(r.color)) throw `Strip color ${JSON.stringify(r.color)} is not a color like #1f3524.`;
      if (out.some((o) => o.int_ext.toLowerCase() === r.int_ext.trim().toLowerCase() && o.time_of_day.toLowerCase() === r.time_of_day.trim().toLowerCase())) throw 'Two strip colors for the same values.';
      const text = r.text && r.text.trim() ? r.text.trim() : null;
      if (text && !hex.test(text)) throw `Text color ${JSON.stringify(text)} is not a color like #1f3524.`;
      out.push({ int_ext: r.int_ext.trim(), time_of_day: r.time_of_day.trim(), color: r.color.toLowerCase(), text: text ? text.toLowerCase() : null });
    }
    for (const t of [scene_text, banner_text, break_text]) if (t && !hex.test(t)) throw `${JSON.stringify(t)} is not a color like #1f3524.`;
    return edit((m) => {
      m.colors = { rules: out, scene_text: scene_text || null, banner_text: banner_text || null, break_text: break_text || null };
    });
  },
  set_strip_layout: ({ board, columns, size, marks }) => {
    needBoard(model, board);
    const cols = checkColumns(columns);
    const sz = checkSize(size);
    return edit((m) => {
      const same = JSON.stringify(cols) === JSON.stringify(DEFAULT_COLUMNS);
      if (same) delete m.layouts[board];
      else m.layouts[board] = cols;
      m.layoutSizes = m.layoutSizes || {};
      m.layoutSizes[board] = sz;
      m.layoutMarks = m.layoutMarks || {};
      m.layoutMarks[board] = marksIn(marks);
    });
  },
  add_layout: ({ name, kind, layout, board }) => {
    if (!['strips', 'day_breaks', 'shoot_day'].includes(kind)) throw `${JSON.stringify(kind)} is not a layout type.`;
    const n = freeLayoutName(model, name);
    const entry = { id: nextId++, name: n, kind, ...(kind === 'strips' ? { columns: checkColumns(layout.columns), size: checkSize(layout.size), marks: marksIn(layout.marks) } : { look: lookIn(layout.look || {}) }) };
    if (board !== null && board !== undefined) needBoard(model, board);
    return edit((m) => {
      m.library = [...(m.library || []), entry];
      if (board !== null && board !== undefined) {
        m.uses = m.uses || {};
        m.uses[board] = { ...usesOf(m, board), [kind]: entry.id };
      }
    });
  },
  save_layout: ({ id, layout }) => {
    const l = named(model, id);
    if (!l) throw 'That layout is no longer in the schedule.';
    const body = l.kind === 'strips' ? { columns: checkColumns(layout.columns), size: checkSize(layout.size), marks: marksIn(layout.marks) } : { look: lookIn(layout.look || {}) };
    return edit((m) => Object.assign(named(m, id), body));
  },
  rename_layout: ({ id, name }) => {
    if (!named(model, id)) throw 'That layout is no longer in the schedule.';
    const n = freeLayoutName(model, name, id);
    return edit((m) => (named(m, id).name = n));
  },
  move_layout: ({ id, to }) => {
    if (!named(model, id)) throw 'That layout is no longer in the schedule.';
    return edit((m) => {
      const i = m.library.findIndex((l) => l.id === id);
      const [l] = m.library.splice(i, 1);
      m.library.splice(Math.min(Math.max(to, 0), m.library.length), 0, l);
    });
  },
  delete_layout: ({ id }) => {
    if (!named(model, id)) throw 'That layout is no longer in the schedule.';
    return edit((m) => {
      m.library = m.library.filter((l) => l.id !== id);
      if (m.oneLine === id) m.oneLine = null;
      for (const u of Object.values(m.uses || {})) for (const k of Object.keys(u)) if (u[k] === id) u[k] = null;
    });
  },
  use_layout: ({ board, kind, id }) => {
    needBoard(model, board);
    if (id !== null && (!named(model, id) || named(model, id).kind !== kind)) throw `That layout is not a ${kind.replace('_', ' ')} layout.`;
    return edit((m) => {
      m.uses = m.uses || {};
      m.uses[board] = { ...usesOf(m, board), [kind]: id };
    });
  },
  use_one_line_layout: ({ id }) => {
    if (id !== null && (!named(model, id) || named(model, id).kind !== 'strips')) throw 'That layout is not a strip layout.';
    return edit((m) => (m.oneLine = id));
  },
  export_layouts: ({ ids }) => {
    if (cancelled()) return { done: false, file: null };
    if (!(model.library || []).length) throw 'There are no layouts to save. Save a strip layout as a named layout first.';
    window.__exported = { layouts: ids };
    return { done: true, file: 'Harbor Light - Layouts.mtlayouts' };
  },
  import_layouts: () => {
    if (cancelled()) return null;
    const entry = { id: nextId++, name: freeLayoutName(model, (model.library || []).some((l) => l.name === 'Loaded') ? `Loaded ${nextId}` : 'Loaded'), kind: 'strips', columns: DEFAULT_COLUMNS, size: 'normal' };
    const v = edit((m) => (m.library = [...(m.library || []), entry]));
    return { ...v, notice: 'Loaded 1 layout.' };
  },
  copy_strips: ({ board, strips, cut }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    if (typeof cut !== 'boolean') throw 'fake: copy_strips needs cut';
    if (!strips.length) throw 'Select the strips to copy first.';
    const all = model.subs.flatMap((s) => s.strips).concat(model.boneyard);
    const items = all.filter((s) => strips.includes(s.id)).map((s) => JSON.parse(JSON.stringify(s)));
    if (items.length !== strips.length) throw 'That strip is no longer on the board.';
    clip = { kind: 'strips', board, cut, items };
    const n = countText(items.length, 'strip', 'strips');
    return view(cut ? `Cut ${n}. Paste moves them below the selection on this board.` : `Copied ${n}. Paste puts copies below the selection.`);
  },
  paste_strips: ({ board, sub_board, index }) => {
    if (board !== model.shown) throw `fake: board ${board} is not the board shown`;
    if (!clip) throw 'Nothing has been cut or copied yet.';
    if (clip.kind !== 'strips') throw 'The clipboard holds elements. Paste them on a breakdown sheet.';
    if (sub_board !== null) subOf(model, sub_board);
    const c = clip;
    const all = model.subs.flatMap((s) => s.strips).concat(model.boneyard);
    if (c.cut && c.board === board) {
      const ids = all.filter((s) => c.items.some((i) => i.id === s.id));
      if (!ids.length) throw 'The cut strips are no longer on this board.';
      if (sub_board === null && ids.some((s) => s.kind === 'banner')) throw "Banners don't go in the Boneyard. Select a strip on a sub-board to paste below it.";
      commands.move_strips({ board, strips: ids.map((s) => s.id), sub_board, index });
      clip = null;
      return { ...view(`Moved ${countText(ids.length, 'strip', 'strips')}.`), strips: ids.map((s) => s.id) };
    }
    if (sub_board === null && c.items.some((s) => s.kind === 'banner')) throw "Banners don't go in the Boneyard. Select a strip on a sub-board to paste below it.";
    const made = [];
    edit((m) => {
      const dest = list(m, sub_board);
      c.items.forEach((item, k) => {
        const copy = { ...JSON.parse(JSON.stringify(item)), id: nextId++ };
        if (copy.kind === 'scene') {
          copy.sheet = nextId++;
          for (const b of Object.values(m.store)) b.boneyard.push({ ...JSON.parse(JSON.stringify(copy)), id: nextId++ });
        }
        dest.splice(Math.min(index + k, dest.length), 0, copy);
        made.push(copy.id);
      });
    });
    const n = countText(made.length, 'strip', 'strips');
    const other = c.cut ? `Pasted copies of ${n}. A cut moves strips only on its own board, so the originals stay on ${(model.boards.find((b) => b.id === c.board) || { name: '' }).name}.` : `Pasted ${n}.`;
    if (c.cut) clip = null;
    return { ...view(other), strips: made };
  },
  copy_elements: ({ sheet, elements, cut }) => {
    const s = needSheet(model, sheet);
    if (typeof cut !== 'boolean') throw 'fake: copy_elements needs cut';
    if (!elements.length) throw 'Select the elements to copy first.';
    if (elements.some((e) => !s.els.includes(e))) throw 'That element is no longer on this sheet.';
    clip = { kind: 'elements', sheet, cut, elements: s.els.filter((e) => elements.includes(e)) };
    const n = countText(elements.length, 'element', 'elements');
    return view(cut ? `Cut ${n}. Paste on another sheet moves them there.` : `Copied ${n}. Paste puts them on the sheet shown.`);
  },
  paste_elements: ({ sheet }) => {
    needSheet(model, sheet);
    if (!clip) throw 'Nothing has been cut or copied yet.';
    if (clip.kind !== 'elements') throw 'The clipboard holds strips. Paste them on the Board.';
    const c = clip;
    if (c.cut && c.sheet === sheet) return view('Those elements are already on this sheet. Paste them on another sheet to move them.');
    const target = sceneOf(model, sheet);
    const adding = c.elements.filter((e) => !target.els.includes(e));
    sheetEdit(sheet, (s) => s.els.push(...adding));
    if (c.cut) {
      sheetEdit(c.sheet, (s) => (s.els = s.els.filter((e) => !c.elements.includes(e))));
      undo.splice(undo.length - 1, 1);
      clip = null;
      return view(`Moved ${countText(c.elements.length, 'element', 'elements')} from scene ${sceneOf(model, c.sheet).n} to scene ${target.n}.`);
    }
    return view(adding.length ? `Pasted ${countText(adding.length, 'element', 'elements')} onto scene ${target.n}.` : 'Those elements are already on this sheet.');
  },
  elements_view: ({ sub_board, category, header }) => elementsView(category, sub_board, header || null),
  element_days: ({ sub_board, element }) => elementDays(element, sub_board),
  add_property: ({ category, name }) => {
    const c = needCat(model, category);
    const n = String(name).trim();
    if (!n) throw 'Type a name for the property.';
    if (c.props.some((p) => same(p, n))) throw `${n} is already a property of this category.`;
    return edit((m) => needCat(m, category).props.push(n));
  },
  rename_property: ({ category, at, name }) => {
    const c = needCat(model, category);
    const n = String(name).trim();
    if (at >= c.props.length) throw 'That property is no longer in the category.';
    if (!n) throw 'Type a name for the property.';
    if (c.props.some((p, i) => i !== at && same(p, n))) throw `${n} is already a property of this category.`;
    return edit((m) => {
      const mc = needCat(m, category);
      const old = mc.props[at];
      mc.props[at] = n;
      mc.multi = (mc.multi || []).map((x) => (x === old ? n : x));
      for (const e of m.elements.filter((x) => x.category === mc.name)) for (const v of e.props) if (v.name === old) v.name = n;
    });
  },
  move_property: ({ category, at, to }) => {
    const c = needCat(model, category);
    if (at >= c.props.length || to >= c.props.length) throw 'That property is no longer in the category.';
    return edit((m) => {
      const mc = needCat(m, category);
      const [p] = mc.props.splice(at, 1);
      mc.props.splice(to, 0, p);
    });
  },
  delete_property: ({ category, at }) => {
    const c = needCat(model, category);
    if (at >= c.props.length) throw 'That property is no longer in the category.';
    return edit((m) => {
      const mc = needCat(m, category);
      const [old] = mc.props.splice(at, 1);
      mc.multi = (mc.multi || []).filter((x) => x !== old);
      for (const e of m.elements.filter((x) => x.category === mc.name)) e.props = e.props.filter((v) => v.name !== old);
    });
  },
  set_property_multi_line: ({ category, at, multi_line }) => {
    const c = needCat(model, category);
    if (at >= c.props.length) throw 'That property is no longer in the category.';
    return edit((m) => {
      const mc = needCat(m, category);
      const p = mc.props[at];
      const on = new Set((mc.multi || []).filter((x) => x !== p));
      if (multi_line) on.add(p);
      mc.multi = mc.props.filter((x) => on.has(x));
    });
  },
  set_property_value: ({ element, name, value }) => {
    const e = needEl(model, element);
    const c = model.categories.find((x) => x.name === e.category);
    const p = c.props.find((x) => same(x, name));
    if (!p) throw `The category has no property ${JSON.stringify(name)}.`;
    const multi = (c.multi || []).includes(p);
    if ((!multi && /[\n\r]/.test(value)) || value.length > 1000) throw multi ? 'A property value is at most 1000 characters.' : 'A property value is one line of at most 1000 characters.';
    return edit((m) => {
      const x = needEl(m, element);
      x.props = x.props.filter((v) => v.name !== p);
      if (value.trim()) x.props.push({ name: p, value: value.trim() });
    });
  },
  set_links: ({ element, links, retroactive }) => {
    needEl(model, element);
    if (links.includes(element)) throw 'An element can\'t link to itself.';
    for (const l of links) needEl(model, l);
    let n = 0;
    const v = edit((m) => {
      needEl(m, element).links = [...new Set(links)];
      if (!retroactive) return;
      for (const sc of scenesInOrder(m)) {
        if (!sc.els.includes(element)) continue;
        const before = sc.els.length;
        for (const l of linkChain(m, element).slice(1)) if (!sc.els.includes(l)) sc.els.push(l);
        if (sc.els.length > before) n++;
      }
    });
    return retroactive ? { ...v, notice: n === 1 ? 'Added its linked elements to 1 sheet.' : `Added its linked elements to ${n} sheets.` } : v;
  },
  add_element_image: ({ element }) => {
    // The real command shows a Rust dialog; the fake picks images/<name>-<n>.png.
    if (cancelled()) return null;
    const e = needEl(model, element);
    if (e.images.length >= 20) throw 'An element holds at most 20 images.';
    return edit((m) => {
      const x = needEl(m, element);
      x.images.push(`images/${x.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${x.images.length + 1}.png`);
    });
  },
  remove_element_image: ({ element, at }) => {
    if (!needEl(model, element).images[at]) throw 'That image is no longer on this element.';
    return edit((m) => needEl(m, element).images.splice(at, 1));
  },
  // The first image of each element "exists" (a tiny grey square); the third is a type
  // the page can't draw (it shows the name); the others are missing.
  element_image: ({ element, at }) => {
    if (!needEl(model, element).images[at]) throw 'That image is no longer on this element.';
    if (at === 2) return 'data:image/heic;base64,AAAAGGZ0eXBoZWlj';
    return at === 0 ? 'data:image/gif;base64,R0lGODlhAQABAIAAAMzMzP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==' : null;
  },
  calendar_view: ({ sub_board, year, month, week_start, fields, span }) => {
    const cal = calendarView(sub_board, year ?? null, month ?? null, Boolean(span));
    const sb = subOf(model, sub_board);
    const names = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const monday = week_start === 'mon';
    let flat = cal.weeks.flat();
    if (monday) {
      const d = new Date(`${flat[0].date}T00:00:00Z`);
      d.setUTCDate(d.getUTCDate() + 1);
      flat = flat.slice(1);
      const extra = new Date(`${flat[flat.length - 1].date}T00:00:00Z`);
      extra.setUTCDate(extra.getUTCDate() + 1);
      const iso = extra.toISOString().slice(0, 10);
      flat.push({ ...flat[flat.length - 1], date: iso, day: extra.getUTCDate(), weekday: names[extra.getUTCDay()], in_month: false, shoot_day: null, events: [], red_flags: [] });
    }
    // Each shoot day's strips and its day break's place.
    const byDay = new Map();
    let day = 0;
    let cur = [];
    let curBanners = [];
    sb.strips.forEach((st, i) => {
      if (st.kind === 'break') {
        day++;
        byDay.set(day, { strips: cur, banners: curBanners, at: i });
        cur = [];
        curBanners = [];
      } else if (st.kind === 'scene') cur.push(st);
      else if (st.kind === 'banner') curBanners.push(st.text);
    });
    // The schedule's strip fields (decision 102), else the user's older pick when changed.
    const oldPick = fields && fields.length && fields.join() !== 'scene,set,pages' ? fields : null;
    const f = model.calendarFields || oldPick || ['scene', 'set', 'elements', 'cast'];
    for (const d of flat) {
      const b = d.shoot_day ? byDay.get(d.shoot_day) : null;
      d.strips = b ? b.strips.map((st) => ({ strip: st.id, sheet: st.sheet, cells: f.map((x) => cellText(st, x)), tone: tone(st), elements: st.els })) : [];
      d.pages = b ? pages(b.strips.reduce((a, x) => a + x.eighths, 0)) : '';
      d.time = b ? '0:00' : '';
      d.drop_at = b ? b.at : null;
      d.scenes = b ? b.strips.length : 0;
      d.banners = b ? b.banners : [];
    }
    const weeks = [];
    for (let w = 0; w < cal.weeks.length; w++) weeks.push(flat.slice(w * 7, w * 7 + 7));
    const SHOWS = [['day_labels', 'Shoot day and day off labels'], ['pages', 'Page counts'], ['time', 'Estimated time'], ['scenes', 'Scenes per day'], ['events', 'Events'], ['strips', 'Strips'], ['banners', 'Banners'], ['red_flags', 'Red flags']];
    return { ...cal, span: Boolean(span), hidden: SHOWS.map(([k]) => k).filter((k) => (model.calendarHidden || []).includes(k)), shows: SHOWS, event_kinds: FAKE_EVENT_KINDS, event_colors: FAKE_EVENT_COLORS, weeks, board: model.shown, fields: f, week: monday ? [...names.slice(1), 'Sun'] : names, today: new Date().toISOString().slice(0, 10), prep_start: sb.prep || null, wrap_end: sb.wrap || null };
  },
  set_day_kind: ({ sub_board, date, kind }) => {
    subOf(model, sub_board);
    if (!validDate(date)) throw `The date ${JSON.stringify(date)} is not a date. Use YYYY-MM-DD.`;
    if (!['work', 'off', 'holiday', 'move'].includes(kind)) throw 'A date is working, a day off, a holiday or company travel.';
    return edit((m) => {
      const c = calOf(subOf(m, sub_board));
      delete c.marks[date];
      if (kindOf(subOf(m, sub_board), date) !== kind) c.marks[date] = kind;
    });
  },
  set_weekly_off: ({ sub_board, days }) => {
    subOf(model, sub_board);
    if (days.some((d) => !WDKEY.includes(d))) throw 'That is not a weekday.';
    if (WDKEY.every((d) => days.includes(d))) throw 'Keep at least one working day in the week.';
    return edit((m) => (calOf(subOf(m, sub_board)).weekly_off = [...days]));
  },
  save_event: ({ sub_board, index, event }) => {
    const { kind, label, name, elements: els } = event;
    if (!FAKE_EVENT_KINDS.some(([k]) => k === kind)) throw 'An event is travel, a fitting, a rehearsal, unavailable, a camera test, post-production work or other.';
    const color = event.color && String(event.color).trim() ? String(event.color).trim() : null;
    if (color && !FAKE_EVENT_COLORS.some(([k]) => k === color)) throw `An event's color is one of ${FAKE_EVENT_COLORS.map(([, n]) => n.toLowerCase()).join(', ')}.`;
    const from = typedDate(event.from);
    const to = typedDate(event.to);
    if (!from) throw notADate('From', event.from);
    if (!to) throw notADate('To', event.to);
    if (to < from) throw 'The event ends before it starts.';
    const c = calOf(subOf(model, sub_board));
    if (index !== null && !c.events[index]) throw 'That event is no longer on the calendar.';
    const ev = { kind, label: kind === 'other' ? label.trim() : '', name: name.trim(), from, to, elements: [...new Set(els)], color };
    return edit((m) => {
      const mc = calOf(subOf(m, sub_board));
      if (index === null) mc.events.push(ev);
      else mc.events[index] = ev;
    });
  },
  delete_event: ({ sub_board, index }) => {
    if (!calOf(subOf(model, sub_board)).events[index]) throw 'That event is no longer on the calendar.';
    return edit((m) => calOf(subOf(m, sub_board)).events.splice(index, 1));
  },
  // Dragging an event (src/events.rs): it keeps its length and starts on the date.
  move_event_to: ({ sub_board, index, date }) => {
    const ev = calOf(subOf(model, sub_board)).events[index];
    if (!ev) throw 'That event is no longer on the calendar.';
    const to = typedDateFake(date);
    if (!to) throw 'Drop the event on a date.';
    const days = (iso) => Date.parse(`${iso}T00:00:00Z`) / 86400000;
    const shift = days(to) - days(ev.from);
    if (!shift) return view();
    return edit((m) => {
      const e = calOf(subOf(m, sub_board)).events[index];
      e.from = to;
      e.to = isoDay(e.to, shift).toISOString().slice(0, 10);
    });
  },
  add_category: ({ name }) => {
    const n = String(name).trim();
    if (!n) throw 'Type a name for the category.';
    if (model.categories.some((c) => same(c.name, n))) throw `There is already a category called ${n}.`;
    const id = nextId++;
    return { ...edit((m) => m.categories.push({ id, name: n, auto: false, holds: false, drops: false, gap: 1, exclude: false, daily: false, props: [] })), category: id };
  },
  set_category_field: ({ category, field, value }) => {
    const c = needCat(model, category);
    if (field === 'name') {
      const n = String(value).trim();
      if (!n) throw 'Type a name for the category.';
      if (model.categories.some((x) => x.id !== c.id && same(x.name, n))) throw `There is already a category called ${n}.`;
      return edit((m) => {
        const mc = needCat(m, category);
        for (const e of m.elements) if (e.category === mc.name) e.category = n;
        mc.name = n;
      });
    }
    const v = field === 'min_drop_gap' ? gapOf(value) : bool(value);
    const key = { auto_board_id: 'auto', exclude_from_board: 'exclude', daily_quantity: 'daily', holds: 'holds', drops: 'drops', min_drop_gap: 'gap' }[field];
    if (!key) throw 'That is not a category setting.';
    return edit((m) => (needCat(m, category)[key] = v));
  },
  move_category: ({ category, to }) => {
    const c = needCat(model, category);
    if (isBuiltIn(c)) throw `${c.name} is built in, so it can't be moved or deleted.`;
    return edit((m) => {
      const built = m.categories.filter(isBuiltIn);
      const users = m.categories.filter((x) => !isBuiltIn(x));
      const from = users.findIndex((x) => x.id === category);
      const [moving] = users.splice(from, 1);
      users.splice(Math.min(to, users.length), 0, moving);
      m.categories = built.concat(users);
    });
  },
  delete_category: ({ category }) => {
    const c = needCat(model, category);
    if (isBuiltIn(c)) throw `${c.name} is built in, so it can't be moved or deleted.`;
    if (model.elements.some((e) => e.category === c.name)) throw `${c.name} still has elements. Move or delete them first.`;
    return edit((m) => {
      m.categories = m.categories.filter((x) => x.id !== category);
      // An element count of it counts every element again (decision 147).
      const layouts = [...Object.values(m.layouts || {}), ...(m.library || []).filter((l) => l.kind === 'strips').map((l) => l.columns)];
      for (const cols of layouts) for (const c of cols) if (c.category === category) c.category = null;
    });
  },
  apply_category_defaults: ({ category }) => {
    const c = needCat(model, category);
    const n = model.elements.filter((e) => e.category === c.name).length;
    const v = edit((m) => {
      for (const e of m.elements.filter((x) => x.category === c.name)) Object.assign(e, { holds: c.holds, drops: c.drops, gap: c.gap, exclude_from_board: !!c.exclude });
    });
    return { ...v, notice: n === 1 ? "Gave 1 element the category's rules." : `Gave ${n} elements the category's rules.` };
  },
  set_element_field: ({ element, field, value }) => {
    const e = needEl(model, element);
    const v = String(value).trim();
    if (field === 'name') {
      if (!v) throw 'Type a name for the element.';
      if (model.elements.some((x) => x.id !== e.id && x.category === e.category && same(x.name, v))) throw `${v} is already in this category.`;
    } else if (field === 'board_id') {
      if (v && !/^[A-Za-z0-9]{1,12}$/.test(v)) throw 'A board ID is letters and digits, at most 12, like 12 or 12A.';
      if (v && model.elements.some((x) => x.id !== e.id && x.category === e.category && x.board_id === v)) throw `Board ID ${v} is already used in this category.`;
    } else if (field === 'min_drop_gap') gapOf(v);
    else if (['locked', 'exclude_from_board', 'holds', 'drops'].includes(field)) bool(v);
    else if (field === 'notes') {
      if (value.length > 10000) throw 'Notes are at most 10000 characters.';
    } else throw 'That is not an element field.';
    return edit((m) => {
      const x = needEl(m, element);
      if (field === 'notes') x.notes = String(value).trimEnd();
      else if (field === 'name') x.name = v;
      else if (field === 'board_id') x.board_id = v;
      else if (field === 'min_drop_gap') x.gap = gapOf(v);
      else x[field] = bool(v);
    });
  },
  merge_elements: ({ survivor, others }) => {
    const keep = needEl(model, survivor);
    const gone = others.filter((o) => o !== survivor).map((o) => needEl(model, o));
    if (!gone.length) throw 'Choose at least two elements to merge.';
    if (gone.some((g) => g.category !== keep.category)) throw 'Only elements of one category can be merged.';
    const ids = gone.map((g) => g.id);
    return edit((m) => {
      for (const sc of scenesInOrder(m)) sc.els = [...new Set(sc.els.map((x) => (ids.includes(x) ? survivor : x)))];
      for (const sb of m.subs) for (const ev of sb.events || []) ev.elements = [...new Set(ev.elements.map((x) => (ids.includes(x) ? survivor : x)))];
      m.elements = m.elements.filter((x) => !ids.includes(x.id));
    });
  },
  move_element: ({ element, category }) => {
    const e = needEl(model, element);
    const c = needCat(model, category);
    if (model.elements.some((x) => x.category === c.name && same(x.name, e.name))) throw `${e.name} is already in this category.`;
    if (e.board_id && model.elements.some((x) => x.category === c.name && x.board_id === e.board_id)) throw `Board ID "${e.board_id}" is already used in this category.`;
    return edit((m) => {
      const x = needEl(m, element);
      x.category = c.name;
      x.props = [];
    });
  },
  renumber_elements: ({ category, start, order: shown }) => {
    const c = needCat(model, category);
    const t = String(start).trim();
    if (!/^\d{1,5}$/.test(t) || Number(t) < 1) throw 'Start from a whole number from 1 to 99999.';
    const usual = listed(model, c.name).map((e) => e.id);
    if (shown && (shown.length !== usual.length || new Set(shown).size !== shown.length || !shown.every((id) => usual.includes(id)))) throw 'The list changed while it was open. Try again.';
    return edit((m) => {
      const order = (shown || usual).map((id) => needEl(m, id));
      const kept = (e) => e.locked || (e.board_id && !/^\d+$/.test(e.board_id));
      const taken = new Set(order.filter((e) => kept(e) && /^\d+$/.test(e.board_id)).map((e) => Number(e.board_id)));
      let n = Number(t);
      for (const e of order) {
        if (kept(e)) continue;
        while (taken.has(n)) n++;
        e.board_id = String(n++);
      }
    });
  },
  delete_element: ({ element, unlink }) => {
    const e = needEl(model, element);
    const linked = e.links.length > 0 || model.elements.some((x) => x.links.includes(e.id));
    if (linked && !unlink) throw `${e.name} is linked to other elements. Unlink it to delete it.`;
    return edit((m) => {
      m.elements = m.elements.filter((x) => x.id !== element);
      for (const x of m.elements) x.links = x.links.filter((l) => l !== element);
      for (const sc of scenesInOrder(m)) sc.els = sc.els.filter((x) => x !== element);
      for (const sb of m.subs) for (const ev of sb.events || []) ev.elements = ev.elements.filter((x) => x !== element);
    });
  },
  set_start_date: ({ sub_board, date }) => {
    subOf(model, sub_board);
    const t = date === null ? '' : String(date).trim();
    const iso = t ? typedDate(t) : null;
    if (t && !iso) throw notADate('The start date', t);
    return edit((m) => {
      subOf(m, sub_board).start = iso;
    });
  },
};

// Merge Strips, roughly as the engine does it (src/sheet.rs, Schedule::merge_sheets):
// `sheets` comes in the order selected; the first survives, scenes joined in that order,
// pages summed, cast unioned, other I/E, set and time of day values left out. With
// `apply`, every board's copy changes.
function fakeMerge(m, sheets, apply = false) {
  if (new Set(sheets).size !== sheets.length) throw 'A sheet was named twice.';
  for (const s of sheets) needSheet(m, s);
  if (sheets.length < 2) throw 'Select two or more scenes to merge.';
  const scenes0 = scenesInOrder(m);
  const all = sheets.map((id) => scenes0.find((s) => s.sheet === id));
  const [first, ...rest] = all;
  const title = (s) => (s.n ? `Scene ${s.n}` : 'A sheet with no scene number');
  const scenes = [...new Set(all.map((s) => s.n).filter(Boolean))].join(', ');
  const eighths = all.reduce((a, s) => a + s.eighths, 0);
  const els = [...new Set(all.flatMap((s) => s.els))];
  const combined = [];
  if (scenes !== first.n) combined.push({ field: 'scenes', label: 'Scenes', before: first.n, after: scenes });
  if (eighths !== first.eighths) combined.push({ field: 'eighths', label: 'Pages', before: pages(first.eighths), after: pages(eighths) });
  const dropped = [];
  for (const [k, field, label] of [['ie', 'int_ext', 'I/E'], ['set', 'set', 'Set'], ['tod', 'time_of_day', 'Time of day']]) {
    for (const s of rest) if (s[k] && s[k] !== first[k]) dropped.push({ sheet: s.sheet, title: title(s), field, label, value: s[k] });
  }
  const name = (id) => m.elements.find((e) => e.id === id);
  const dto = {
    survivor: { sheet: first.sheet, title: title(first) },
    merged: rest.map((s) => ({ sheet: s.sheet, title: title(s) })),
    combined,
    elements_added: els.filter((e) => !first.els.includes(e)).map((e) => `${name(e).name} (${name(e).category})`),
    dropped,
  };
  if (apply) {
    const gone = new Set(rest.map((s) => s.sheet));
    for (const b of allBoards(m)) {
      for (const st of b.subs.flatMap((x) => x.strips).concat(b.boneyard)) {
        if (st.sheet === first.sheet) Object.assign(st, { n: scenes, eighths, els: [...els] });
      }
      for (const sb of b.subs) sb.strips = sb.strips.filter((x) => !gone.has(x.sheet));
      b.boneyard.splice(0, b.boneyard.length, ...b.boneyard.filter((x) => !gone.has(x.sheet)));
    }
  }
  return { dto, survivor: first.sheet };
}

function bannerMinutes(text, duration) {
  if (!String(text).trim()) throw "Type the banner's text.";
  if (String(text).trim().length > 200) throw 'A banner is at most 200 characters.';
  const d = String(duration).trim();
  if (!d) return 0;
  const m = /^(\d+):([0-5]\d)$/.exec(d);
  if (!m) throw `Duration ${JSON.stringify(d)} must look like 1:30 (hours and minutes).`;
  const minutes = Number(m[1]) * 60 + Number(m[2]);
  if (minutes > MAX_DURATION) throw tooLong('Duration', d);
  return minutes;
}

// A typed h:mm duration is a day at most (src/sheet.rs, MAX_DURATION; decision 50).
const MAX_DURATION = 24 * 60;
const hmMinutes = (v) => {
  const [h, m] = v.split(':').map(Number);
  return h * 60 + m;
};
const tooLong = (what, v) => `${what} ${JSON.stringify(v)} is longer than a day. Type 24:00 at most.`;

// ---- Find and Replace (src/find.rs): the engine's typed search, roughly. Places in board
// order: each board's sub-boards' strips (a sheet's fields the first time it is met,
// banners), each sub-board's events, then sheets met only in a Boneyard, then categories
// each followed by its elements.

const FIND_FIELDS = ['number', 'scenes', 'int_ext', 'set', 'time_of_day', 'synopsis', 'location', 'script_pages', 'script_day', 'unit', 'sequence', 'episode', 'comments'];
const FIND_LABELS = { number: 'Sheet number', scenes: 'Scenes', int_ext: 'I/E', set: 'Set', time_of_day: 'Time of day', synopsis: 'Synopsis', location: 'Location', script_pages: 'Script pages', script_day: 'Script day', unit: 'Unit', sequence: 'Sequence', episode: 'Episode', comments: 'Comments' };
const MULTI_LINE = ['synopsis', 'comments'];
/** The one-value header fields (reports::HeaderField), as view.rs sends their values. */
const HEADER_KEYS = ['set', 'location', 'unit', 'episode', 'int_ext', 'time_of_day', 'script_day', 'sequence'];
const sheetGet = (s, f) => (FIELD_KEYS[f] ? s[FIELD_KEYS[f]] : s.f[f] || '');
const sheetSet = (s, f, v) => {
  if (FIELD_KEYS[f]) s[FIELD_KEYS[f]] = v;
  else s.f[f] = v;
};
const isWord = (c) => c !== undefined && /[\p{L}\p{N}_]/u.test(c);

function findRanges(hay, q) {
  const out = [];
  const h = q.match_case ? hay : hay.toLowerCase();
  const n = q.match_case ? q.text : q.text.toLowerCase();
  if (!n) return out;
  let at = 0;
  for (;;) {
    const i = h.indexOf(n, at);
    if (i < 0) break;
    const end = i + n.length;
    const ok = !q.whole_word || !((isWord(n[0]) && isWord(hay[i - 1])) || (isWord(n[n.length - 1]) && isWord(hay[end])));
    if (ok) {
      out.push([i, end]);
      at = end;
    } else at = i + 1;
  }
  return out;
}

/** Every place the query covers, in board order: [{ key, kind, field, get, set, ... }]. */
function findPlaces(m, q) {
  const want = (f) => !q.field || q.field === f || (q.field === 'sheet' && FIND_FIELDS.includes(f));
  const out = [];
  const seen = new Set();
  const pushSheet = (s) => {
    if (seen.has(s.sheet)) return;
    seen.add(s.sheet);
    for (const f of FIND_FIELDS.filter(want)) {
      const label = s.n ? `Scene ${s.n}` : 'A sheet with no scene number';
      out.push({ key: `sheet:${s.sheet}:${f}`, kind: 'sheet', field: f, place: `${label}, ${FIND_LABELS[f]}`, sheet: s.sheet, line: !MULTI_LINE.includes(f), get: () => sheetGet(s, f), set: (v) => sheetSet(s, f, v) });
    }
  };
  let scopeSub = null;
  if (q.sub_board !== null && q.sub_board !== undefined) {
    scopeSub = q.sub_board;
    if (!m.boards.some((b) => listsOf(m, b.id).subs.some((sb) => sb.id === scopeSub))) throw 'That sub-board is no longer in the schedule.';
  }
  const many = m.boards.length > 1;
  for (const b of m.boards) {
    const lists = listsOf(m, b.id);
    for (const sb of lists.subs) {
      if (scopeSub !== null && sb.id !== scopeSub) continue;
      const where = many ? `${b.name}, ${sb.name}` : sb.name;
      for (const st of sb.strips) {
        if (st.kind === 'scene') pushSheet(st);
        else if (st.kind === 'banner' && want('banner'))
          out.push({ key: `banner:${b.id}:${sb.id}:${st.id}`, kind: 'banner', field: 'text', place: `Banner on ${where}`, board: b.id, sub_board: sb.id, strip: st.id, line: true, get: () => st.text, set: (v) => (st.text = v) });
      }
      if (want('event'))
        calOf(sb).events.forEach((ev, index) =>
          out.push({ key: `event:${b.id}:${sb.id}:${index}`, kind: 'event', field: 'name', place: `Event on ${where}, ${ev.from}`, board: b.id, sub_board: sb.id, event: index, date: ev.from, line: true, get: () => ev.name, set: (v) => (ev.name = v) }),
        );
    }
  }
  let inScope = null;
  if (scopeSub === null) {
    for (const b of m.boards) for (const st of listsOf(m, b.id).boneyard) if (st.kind === 'scene') pushSheet(st);
  } else {
    inScope = new Set();
    for (const b of m.boards)
      for (const sb of listsOf(m, b.id).subs.filter((x) => x.id === scopeSub)) {
        for (const st of sb.strips) if (st.kind === 'scene') st.els.forEach((e) => inScope.add(e));
        for (const ev of calOf(sb).events) (ev.elements || []).forEach((e) => inScope.add(e));
      }
  }
  for (const c of m.categories) {
    const els = m.elements.filter((e) => e.category === c.name && (!inScope || inScope.has(e.id)));
    if (want('category') && (!inScope || els.length))
      out.push({ key: `category:${c.id}`, kind: 'category', field: 'name', place: 'Category', category: c.id, line: true, name: true, cat: c, get: () => c.name, set: (v) => (c.name = v) });
    if (want('element'))
      for (const e of els)
        out.push({ key: `element:${e.id}`, kind: 'element', field: 'name', place: `${c.name} element`, element: e.id, category: c.id, line: true, name: true, el: e, get: () => e.name, set: (v) => (e.name = v) });
  }
  return out;
}

function hitDto(p, q) {
  const text = p.get();
  const flat = text.replace(/[\u0000-\u001f]/g, ' ');
  const first = findRanges(text, q)[0] || [0, 0];
  const from = Math.max(0, Math.min(first[0] - 20, flat.length - 80));
  const context = flat.length <= 80 ? flat : `${from > 0 ? '...' : ''}${flat.slice(from, from + 80)}${from + 80 < flat.length ? '...' : ''}`;
  const pick = (k) => (p[k] === undefined ? null : p[k]);
  return { key: p.key, kind: p.kind, field: p.field, place: p.place, context, sheet: pick('sheet'), element: pick('element'), category: pick('category'), board: pick('board'), sub_board: pick('sub_board'), strip: pick('strip'), event: pick('event'), date: pick('date') };
}

/** An advanced find's rows, checked as Rust checks them (core find::Condition::new). */
function findConditions(q) {
  return (q.conditions || []).map((c) => {
    const numeric = ['pages', 'shoot_time', 'screen_time'].includes(c.field);
    if (!numeric && !FIND_FIELDS.includes(c.field)) throw "That isn't a field Find knows.";
    const ops = ['contains', 'not_contains', 'is', 'is_not', 'starts_with', 'empty', 'not_empty', 'greater', 'less'];
    if (!ops.includes(c.op)) throw "That isn't a comparison Find knows.";
    const v = String(c.value || '').trim();
    const takes = c.op !== 'empty' && c.op !== 'not_empty';
    let n = null;
    if (!numeric && takes && !v && c.op !== 'is' && c.op !== 'is_not') throw 'Type a value for each condition, or pick is empty.';
    if (numeric && takes) {
      if (['contains', 'not_contains', 'starts_with'].includes(c.op)) throw `${c.field[0].toUpperCase()}${c.field.slice(1)} is a number: use is, is not, greater than or less than.`;
      if (c.field === 'pages') {
        const m = /^(\d+)?\s*(?:(\d+)\/8)?$/.exec(v);
        if (!v || !m) throw `${JSON.stringify(v)} is not a page length like 2 or 1 3/8.`;
        n = Number(m[1] || 0) * 8 + Number(m[2] || 0);
      } else {
        if (!/^\d+(:[0-5]\d)?$/.test(v)) throw `${JSON.stringify(v)} is not a ${c.field === 'shoot_time' ? 'shoot time like 1:30' : 'screen time like 2:05'}.`;
        n = v.includes(':') ? hmOf(v) : Number(v);
      }
    }
    return { ...c, v, n, numeric };
  });
}

function meets(s, c, matchCase) {
  if (c.numeric) {
    const have = c.field === 'pages' ? (s.eighths ? s.eighths : null) : s.f[c.field] ? hmOf(s.f[c.field]) : null;
    const want = c.n || 0;
    switch (c.op) {
      case 'empty': return have === null;
      case 'not_empty': return have !== null;
      case 'is': return (have || 0) === want;
      case 'is_not': return (have || 0) !== want;
      case 'greater': return have !== null && have > want;
      case 'less': return (have || 0) < want;
      default: return false;
    }
  }
  const raw = sheetGet(s, c.field).trim();
  const [x, v] = matchCase ? [raw, c.v] : [raw.toLowerCase(), c.v.toLowerCase()];
  switch (c.op) {
    case 'contains': return x.includes(v);
    case 'not_contains': return !x.includes(v);
    case 'is': return x === v;
    case 'is_not': return x !== v;
    case 'starts_with': return x.startsWith(v);
    case 'empty': return !x;
    case 'not_empty': return Boolean(x);
    case 'greater': return Boolean(x) && natural(raw, c.v) > 0;
    case 'less': return Boolean(x) && natural(raw, c.v) < 0;
    default: return false;
  }
}

function findText(q) {
  if (q.field && !['sheet', 'element', 'category', 'banner', 'event', ...FIND_FIELDS].includes(q.field)) throw "That isn't a field Find knows.";
  const conds = findConditions(q);
  let places = findPlaces(model, q);
  if (conds.length) {
    // Only the header fields of the sheets that meet the conditions (AND, or OR with any).
    const ok = (sheet) => {
      const s = sceneOf(model, sheet);
      const each = conds.map((c) => meets(s, c, q.match_case));
      return q.any ? each.some(Boolean) : each.every(Boolean);
    };
    places = places.filter((p) => p.kind === 'sheet' && ok(p.sheet));
    if (!q.text) {
      const first = conds.find((c) => !c.numeric);
      const seen = new Set();
      const hits = [];
      for (const p of places) {
        if (seen.has(p.sheet)) continue;
        seen.add(p.sheet);
        const at = (first && places.find((x) => x.sheet === p.sheet && x.field === first.field)) || p;
        const flat = at.get().replace(/[\u0000-\u001f]/g, ' ');
        hits.push({ ...hitDto({ ...at, get: () => '' }, { text: '' }), context: flat.slice(0, 80) });
      }
      return { count: hits.length, hits, truncated: false };
    }
  }
  if (!q.text) return { count: 0, hits: [], truncated: false };
  const hits = places.filter((p) => findRanges(p.get(), q).length).map((p) => hitDto(p, q));
  return { count: hits.length, hits: hits.slice(0, 1000), truncated: hits.length > 1000 };
}

function replaceText(q, withText, picks) {
  if (!q.text) throw 'Type what to find first.';
  if (!picks.length) throw 'Check one or more results to replace.';
  const live = new Set(findText(q).hits.map((h) => h.key));
  if (picks.some((k) => !live.has(k))) throw 'Nothing was replaced: some of the checked results no longer match, because the schedule changed. Check them again and choose Replace.';
  let changed = 0;
  let refusal = null;
  const v = edit((m) => {
    const byKey = new Map(findPlaces(m, q).map((p) => [p.key, p]));
    for (const k of new Set(picks)) {
      const p = byKey.get(k);
      const before = p.get();
      let after = '';
      let from = 0;
      for (const [s, e] of findRanges(before, q)) {
        after += before.slice(from, s) + withText;
        from = e;
      }
      after += before.slice(from);
      if (after === before) continue;
      if (/[\u0000-\u001f]/.test(withText) && (p.line || /[\u0000-\u0008\u000b-\u001f]/.test(withText))) refusal = refusal || `Nothing was replaced: ${p.place} is one line, so the replacement can't hold a line break.`;
      if (p.name) {
        after = after.trim();
        if (!after) refusal = refusal || `Nothing was replaced: ${p.kind === 'category' ? 'a category' : 'an element'}'s name can't be empty.`;
        if (after === before) continue;
        if (p.kind === 'category') for (const e of m.elements) if (e.category === p.cat.name) e.category = after;
      }
      p.set(after);
      changed++;
    }
    for (const c of m.categories) {
      const names = m.elements.filter((e) => e.category === c.name).map((e) => e.name.trim().toLowerCase());
      const dup = names.find((n, i) => names.indexOf(n) !== i);
      if (dup !== undefined) refusal = refusal || `Nothing was replaced: ${c.name} would have two elements called ${JSON.stringify(m.elements.find((e) => e.category === c.name && e.name.trim().toLowerCase() === dup).name)}.`;
    }
    const cats = m.categories.map((c) => c.name.trim().toLowerCase());
    if (cats.some((n, i) => cats.indexOf(n) !== i)) refusal = refusal || 'Nothing was replaced: there would be two categories with one name.';
  });
  if (refusal) {
    // The fake's edit always takes a step: take it back.
    model = JSON.parse(undo.pop());
    throw refusal;
  }
  if (!changed) {
    model = JSON.parse(undo.pop());
    return view('Nothing changed: the checked results already read that way.');
  }
  return { ...view(changed === 1 ? 'Replaced in 1 place. Undo takes it back.' : `Replaced in ${changed} places. Undo takes them all back.`) };
}

// ---- Conflicts (src/conflicts.rs), roughly: elements on a date in two or more sub-boards of
// the board shown (work days from day breaks and start dates), and a travel day, fitting or
// rehearsal on a date the element works on the same sub-board; event clashes apart.

/** Conflict marks (src/conflictmarks.rs), roughly: strips and dates in words. */
function fakeConflictMarks(m) {
  const found = fakeConflicts(m);
  const strips = new Map();
  const dates = new Map();
  const add = (map, k, t) => map.set(k, [...new Set([...(map.get(k) || []), t])]);
  for (const c of found.conflicts)
    for (const bk of c.bookings) {
      const others = c.bookings.filter((o) => o.sub_board !== bk.sub_board).map((o) => `${o.sub_board_name} day ${o.day}`);
      const t = `${c.name} on ${c.date} also on ${others.join(', ')}`;
      for (const sh of bk.sheets) add(strips, sh.strip, t);
      add(dates, `${bk.sub_board}|${c.date}`, t);
    }
  for (const w of found.on_work) add(dates, `${w.sub_board}|${w.date}`, `${w.name} at ${w.event} on a work day, ${w.date}`);
  return { strips, dates };
}

function fakeConflicts(m) {
  const booked = new Map(); // `${el}|${date}` -> [{ sub, day, sheets }]
  const undated = [];
  for (const sb of m.subs) {
    if (!sb.start) {
      undated.push({ id: sb.id, name: sb.name });
      continue;
    }
    let day = 1;
    let cur = [];
    for (const st of sb.strips) {
      if (st.kind === 'scene') cur.push(st);
      if (st.kind !== 'break') continue;
      const [date] = addDays(sb, day);
      for (const sc of cur)
        for (const e of sc.els) {
          const k = `${e}|${date}`;
          const list = booked.get(k) || [];
          let bk = list.find((x) => x.sub_board === sb.id);
          if (!bk) list.push((bk = { sub_board: sb.id, sub_board_name: sb.name, day, sheets: [] }));
          bk.sheets.push({ sheet: sc.sheet, scene: sc.n ? `Scene ${sc.n}` : 'A sheet with no scene number', strip: sc.id });
          booked.set(k, list);
        }
      cur = [];
      day++;
    }
  }
  const named = (id) => m.elements.find((e) => e.id === id) || { name: '', category: '' };
  const wd = (iso) => WD[new Date(`${iso}T00:00:00Z`).getUTCDay()];
  const conflicts = [];
  for (const [k, list] of booked) {
    if (list.length < 2) continue;
    const [el, date] = k.split('|');
    const e = named(Number(el));
    conflicts.push({ element: Number(el), name: e.name, category: e.category, date, weekday: wd(date), bookings: list });
  }
  const evs = new Map();
  const onWork = [];
  const KIND = { travel: 'Travel', fitting: 'Fitting', rehearsal: 'Rehearsal' };
  for (const sb of m.subs)
    calOf(sb).events.forEach((ev, index) => {
      for (let d = ev.from; d <= ev.to; d = isoDay(d, 1).toISOString().slice(0, 10))
        for (const e of ev.elements || []) {
          const k = `${e}|${d}`;
          const bk = (booked.get(k) || []).find((x) => x.sub_board === sb.id);
          if (bk && KIND[ev.kind]) {
            const who = named(e);
            onWork.push({ element: e, name: who.name, category: who.category, date: d, weekday: wd(d), sub_board: sb.id, sub_board_name: sb.name, day: bk.day, index, event: eventTitle(ev) });
          }
          evs.set(k, [...(evs.get(k) || []), { sub_board: sb.id, sub_board_name: sb.name, index, name: eventTitle(ev) }]);
        }
    });
  const clashes = [];
  for (const [k, list] of evs) {
    const bookings = booked.get(k) || [];
    if (list.length < 2 && !bookings.some((bk) => list.some((ev) => ev.sub_board !== bk.sub_board))) continue;
    const [el, date] = k.split('|');
    const e = named(Number(el));
    clashes.push({ element: Number(el), name: e.name, category: e.category, date, weekday: wd(date), events: list, bookings });
  }
  const order = (a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : a.element - b.element);
  conflicts.sort(order);
  onWork.sort(order);
  clashes.sort(order);
  return { board: m.shown, board_name: boardName(m), count: conflicts.length + onWork.length, conflicts, on_work: onWork, clash_count: clashes.length, event_clashes: clashes, undated, truncated: false };
}

// ---- Red flags (src/redflags.rs), roughly: a flagged element working on a shoot day of
// sub-board `sb`, or at a travel day, fitting or rehearsal there, on a date its flag rules
// out. Sorted by date, then the element's place, then the flag's.

function flagText(m, f) {
  if (f.dates === 'range') return f.from === f.to ? `on ${f.from}` : `${f.from} to ${f.to}`;
  return f.dates === 'not_before' ? `not before ${f.from}` : `not after ${f.from}`;
}

function flagDto(m, f) {
  const t = m.redFlagTypes.find((x) => x.id === f.red_flag_type);
  return { ...f, type_name: t ? t.name : '', text: flagText(m, f) };
}

function covers(f, iso) {
  if (f.dates === 'range') return f.from <= iso && iso <= f.to;
  return f.dates === 'not_before' ? iso < f.from : iso > f.from;
}

function fakeRedFlags(m, sb) {
  const flagged = m.elements.filter((e) => (e.red_flags || []).length);
  if (!flagged.length) return [];
  const busy = new Map(); // `${el}|${date}` -> { day, sheets, events }
  const at = (el, date) => {
    const k = `${el}|${date}`;
    if (!busy.has(k)) busy.set(k, { el, date, day: null, sheets: [], events: [] });
    return busy.get(k);
  };
  if (sb.start) {
    let day = 1;
    let cur = [];
    for (const st of sb.strips) {
      if (st.kind === 'scene') cur.push(st);
      if (st.kind !== 'break') continue;
      const [date] = addDays(sb, day);
      for (const sc of cur)
        for (const e of sc.els) {
          const b = at(e, date);
          b.day = day;
          b.sheets.push({ sheet: sc.sheet, scene: sc.n ? `Scene ${sc.n}` : 'A sheet with no scene number', strip: sc.id });
        }
      cur = [];
      day++;
    }
  }
  calOf(sb).events.forEach((ev, index) => {
    if (ev.kind === 'other') return;
    for (let d = ev.from; d <= ev.to; d = isoDay(d, 1).toISOString().slice(0, 10)) for (const e of ev.elements || []) at(e, d).events.push({ index, name: eventTitle(ev) });
  });
  const out = [];
  for (const b of busy.values()) {
    const rank = m.elements.findIndex((e) => e.id === b.el);
    const e = m.elements[rank];
    if (!e) continue;
    const cat = m.categories.find((c) => c.name === e.category);
    (e.red_flags || []).forEach((f, i) => {
      if (!covers(f, b.date)) return;
      const d = flagDto(m, f);
      out.push({
        key: [b.date, rank, i],
        date: b.date,
        weekday: WD[new Date(`${b.date}T00:00:00Z`).getUTCDay()],
        element: e.id,
        name: e.name,
        category: cat ? cat.id : 0,
        category_name: e.category,
        flag: f.id,
        type_name: d.type_name,
        red_flag_type: f.red_flag_type,
        dates: d.text,
        note: f.note,
        hard: f.hard,
        day: b.day,
        sheets: b.sheets,
        events: b.events,
        text: `${e.name}, ${d.type_name}, ${d.text}, ${f.hard ? 'hard' : 'soft'}`,
      });
    });
  }
  out.sort((a, b) => (a.key[0] < b.key[0] ? -1 : a.key[0] > b.key[0] ? 1 : a.key[1] - b.key[1] || a.key[2] - b.key[2]));
  for (const c of out) delete c.key;
  return out;
}

// The Plan menu's commands (harness/fake-plan.js).
Object.assign(
  commands,
  planFakes({
    view,
    edit,
    model: () => model,
    subOf,
    sceneStrips: (m) => new Set(allBoards(m).flatMap((b) => b.subs.flatMap((sb) => sb.strips).concat(b.boneyard)).filter((s) => s.kind === 'scene').map((s) => s.id)),
  }),
);

window.__calls = [];
const handlers = {};
/**
 * Sends an event from "Rust" to the page, as Tauri does: window.__emit('autosave-changed',
 * true) to every window (`emit`), window.__emit('file-state', {...}, 'main') to one window
 * only (`emit_to`). This page is the window labelled `main`; it hears its own events and
 * app-wide ones, never another window's.
 */
const LABEL = 'main';
window.__emit = (name, payload, to) =>
  (handlers[name] || []).forEach((h) => (to === undefined || to === h.target) && h.fn({ payload }));
window.__menu = (id) => window.__emit('menu', id, LABEL);

window.__TAURI__ = {
  core: {
    invoke: async (cmd, args) => {
      window.__calls.push([cmd, args]);
      const fn = commands[cmd];
      if (!fn) throw `fake: no command ${cmd}`;
      return fn(args || {});
    },
  },
  event: {
    listen: async (name, fn, options) => {
      (handlers[name] = handlers[name] || []).push({ fn, target: options && options.target });
      return () => {};
    },
  },
  webviewWindow: {
    getCurrentWebviewWindow: () => ({ label: LABEL }),
  },
};
