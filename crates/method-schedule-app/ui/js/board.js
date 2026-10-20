// The Board tab: the board shown, its sub-boards as day groups of strips, then the
// Boneyard. Rust sends what to draw (src/view.rs); this file lays it out, keeps the
// selection marks, drags strips, and works out where a strip goes when it moves one place.
//
// Fast at 1,500 strips: every line in a sub-board's strip list (day header, strip, day
// break, caption) is a whole number of fixed-height board lines (LINE_PX): one, or a
// scene strip its layout's lines (up to four) and a two-line banner two. So where a line
// sits is arithmetic on a running sum (lineTops) and a binary search (lineAt). With many lines
// only those in view, plus a margin, are drawn; the space above and below them in each list
// is two spacer divs sized through a constructed stylesheet (dynstyle.js), as the CSP
// allows no inline styles. The same approach as the day out of days grid (dood.js).
// Selection and keys work on the data, so a row that isn't drawn yet can be selected: the
// board scrolls to it and draws it.
//
// Dragging works on the same arithmetic: the line under the pointer comes from its list's
// top and LINE_PX, so a drop can land on a line that wasn't drawn when the drag began.
// Near the top or bottom of the scroller the board scrolls by itself and draws the lines
// coming into view. The drop indicator is a class on the line it touches; the label that
// follows the pointer is moved through a constructed stylesheet, numbers only.

import { el } from './dom.js';
import { dateInput, dayDate, showDate } from './dates.js';
import { setRules, dynamicStyles } from './dynstyle.js';
import { DEFAULT_LAYOUT, boardTracks, cellClass, cellText, gridRules, labelOf, stripCell, stripGrid } from './strips.js';
import { colorClass, paintStrips } from './colors.js';
import { customDialog } from './modal.js';
import { STRIP_MARK, bannerFontClass, bannerHex, bannerHexRules, dayLead } from './logic.js';

const BONEYARD = 'boneyard';
/** Must match `.strips > .line` in styles.css. */
const LINE_PX = 22;
/** Lines drawn at once before the board draws only what is in view. */
const ALL_LINES_UP_TO = 120;
/** Extra lines drawn above and below the visible ones in each list. */
const OVERSCAN = 15;
/** How far the pointer moves before a press becomes a drag. */
const DRAG_START_PX = 5;
/** The band at the scroller's top and bottom edges that scrolls while dragging. */
const EDGE_PX = 48;
/** Most pixels scrolled a tick, at the very edge, and the tick. A timer rather than
 * requestAnimationFrame, so it keeps going where frames are paused (a hidden test pane). */
const EDGE_SPEED = 24;
const EDGE_TICK_MS = 16;

/** What automatic day breaks limit a day by, and the last limit typed for each, kept across redraws. */
const BREAK_BY = [
  ['pages', 'Pages', 'Pages per day, as 5 or 4 4/8'],
  ['time', 'Shoot time', 'Shoot time per day, as h:mm'],
  ['screen_time', 'Screen time', 'Screen time per day, as m:ss'],
];
let breakBy = 'pages';
const breakLimit = { pages: '5', time: '10:00', screen_time: '5:00' };

/** The strips cut to the app's clipboard on the board drawn (src/clipboard.rs): marked until the paste. */
let cutMarks = new Set();

/**
 * What the Find panel's results name on the board drawn (decision 130): sheet ids and
 * banner strip ids, marked until the search changes or the panel closes.
 */
let foundMarks = { sheets: new Set(), strips: new Set() };
const NOTHING_FOUND = { sheets: new Set(), strips: new Set() };

/** Whether a strip the view sent is among the Find panel's results. */
export function isFound(strip, found) {
  return strip.kind === 'scene' ? found.sheets.has(strip.sheet) : strip.kind === 'banner' && found.strips.has(strip.id);
}

/**
 * The prefix of the board being drawn's element ids: '' for the Board tab, another for
 * the board beside it (split screen, decisions.md, entry 80). Each board keeps its own
 * cut marks, palette, layout and day look; createBoard puts them in these module values
 * before it draws, so two boards never mix.
 */
let prefix = '';

// ---------------------------------------------------------------------------
// What the board shows (decisions.md, entry 64): day headings, day breaks, the totals on
// them and banners can each be hidden. A per-viewer convenience, remembered in this
// browser (localStorage, wrapped in try/catch), not schedule data; printing the board
// follows it.

const SHOW_KEY = 'method-schedule.board-show';
const SHOW_DEFAULT = { heads: true, breaks: true, totals: true, banners: true };

/** What the board shows now: { heads, breaks, totals, banners }. */
export function boardShow() {
  try {
    const v = JSON.parse(window.localStorage.getItem(SHOW_KEY) || 'null');
    if (v && typeof v === 'object') {
      const out = { ...SHOW_DEFAULT };
      for (const k of Object.keys(SHOW_DEFAULT)) if (typeof v[k] === 'boolean') out[k] = v[k];
      return out;
    }
  } catch {
    // Storage blocked or unreadable: show everything.
  }
  return { ...SHOW_DEFAULT };
}

function saveShow(show) {
  try {
    window.localStorage.setItem(SHOW_KEY, JSON.stringify(show));
  } catch {
    // Not remembered; the choice still applies until the page reloads.
  }
}

let showNow = boardShow();

/** Board > Show on Board: four switches. Resolves true when changed. */
export function boardShowDialog() {
  return customDialog((close) => {
    const boxes = [
      ['heads', 'Day headings (day number and date)'],
      ['breaks', 'Day breaks'],
      ['totals', "Totals on day breaks (scenes, pages, time)"],
      ['banners', 'Banners'],
    ].map(([key, label]) => {
      const box = el('input', { attrs: { type: 'checkbox', id: `show-${key}`, checked: showNow[key] } });
      return { key, box, node: el('label', { class: 'bd-check', attrs: { for: `show-${key}` } }, box, label) };
    });
    const go = () => {
      const next = { ...showNow };
      for (const b of boxes) next[b.key] = b.box.checked;
      showNow = next;
      saveShow(next);
      close(true);
    };
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'show-title' } },
      el('h2', { class: 'modal-title', text: 'Show on the Board', attrs: { id: 'show-title' } }),
      el('p', { class: 'modal-message', text: 'Hidden lines stay in the schedule and in the reports; this only changes what the board and its printout show, on this computer.' }),
      el('div', { class: 'show-boxes' }, ...boxes.map((b) => b.node)),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Show', attrs: { type: 'button', 'data-autofocus': true }, on: { click: go } }),
      ),
    );
  });
}

/** What the board's day breaks show (src/view.rs, DayLookDto; decisions.md, entry 67). */
let lookNow = { shoot_day_strip: false, screen_time: false, sunrise: false, sunset: false, quantity: false, text: '' };

/** The extras a day break shows besides its totals: screen time, sunrise, sunset, quantities, text. */
export function dayExtras(day, look) {
  const parts = [];
  // The pages its scenes still have to shoot, once anything is reported shot (M5).
  if (day.left) parts.push(day.left);
  if (look.screen_time && day.screen_time) parts.push(`Screen ${day.screen_time}`);
  if (look.sunrise && day.sunrise) parts.push(`Sunrise ${day.sunrise}`);
  if (look.sunset && day.sunset) parts.push(`Sunset ${day.sunset}`);
  // The day's element quantities (decisions.md, entry 87): `Background 45 · Animals 3`.
  if (look.quantity && day.quantity) parts.push(day.quantity);
  if (look.text) parts.push(look.text);
  return parts.join(' · ');
}

/** The shoot day strip's text: `Shoot day 3 of 12 · Wednesday, March 3, 2027`, then its layout's text. */
export function shootDayText(day, of, text = '') {
  return `Shoot day ${day.number} of ${of} · ${day.full_date || 'No date'}${text ? ` · ${text}` : ''}`;
}

/** The shoot day layout's text the board picked (decisions.md, entry 78), from the last draw. */
let shootTextNow = '';

/** The banner palette's names (model::BannerColor): each a class in styles.css. */
const BANNER_COLOR = new Set(['yellow', 'orange', 'red', 'pink', 'purple', 'blue', 'teal', 'green', 'gray']);

/** Banner colors by value seen so far, each a `bh-rrggbb` class in a constructed sheet (decision 154). */
const bannerHexes = new Set();

/** The class for a banner color by value, its rule written the first time it is seen. */
export function bannerHexClass(c) {
  const hex = bannerHex(c);
  if (!hex) return '';
  if (!bannerHexes.has(hex)) {
    bannerHexes.add(hex);
    setRules('banner-hex', bannerHexRules([...bannerHexes]));
  }
  return ` bh-${hex.slice(1)}`;
}
/** A banner's text size steps and alignments besides the usual (decisions.md, entry 87): classes. */
const BANNER_SIZE = new Set(['small', 'large']);
const BANNER_ALIGN = new Set(['center', 'right']);

/** A banner's look as classes, from fixed names only (decisions.md, entries 65, 87 and 187). */
export function bannerLook(strip) {
  return (
    `${strip.bold ? ' bold' : ''}${BANNER_COLOR.has(strip.banner_color) ? ` bc-${strip.banner_color}` : bannerHexClass(strip.banner_color)}${strip.tall ? ' tall' : ''}` +
    `${BANNER_SIZE.has(strip.banner_size) ? ` ts-${strip.banner_size}` : ''}${BANNER_ALIGN.has(strip.banner_align) ? ` al-${strip.banner_align}` : ''}${bannerFontClass(strip.banner_font)}`
  );
}

/**
 * Where each line of a list starts, in board lines from the list's top: `tops[i]` for line
 * `i`, and `tops[n]` the list's height. A line is `h` board lines high (one when not
 * given): a scene strip its layout's lines, a two-line banner two (decisions.md, entry 87).
 */
export function lineTops(lines) {
  const tops = new Int32Array(lines.length + 1);
  for (let i = 0; i < lines.length; i++) tops[i + 1] = tops[i] + (lines[i].h || 1);
  return tops;
}

/** The line that board line `u` falls in (clamped to the list): a binary search of `tops`. */
export function lineAt(tops, u) {
  const n = tops.length - 1;
  if (n <= 0 || u <= 0) return 0;
  if (u >= tops[n]) return n - 1;
  let lo = 0;
  let hi = n - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (tops[mid] <= u) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** The user's strip colors in use on the board drawn: `bg|ink` -> class index (colors.js). */
let palette = new Map();

/** Where the board's strip layout puts each field (strips.js, stripGrid), from the last draw. */
let gridNow = stripGrid(DEFAULT_LAYOUT);

/** A strip's list: a sub-board id, or BONEYARD. */
function listKey(subId) {
  return subId === null ? BONEYARD : String(subId);
}

/** How many board lines a strip takes: a scene its layout's lines, a two-line banner two. */
function heightOf(strip) {
  if (strip.kind === 'scene') return gridNow.lines;
  return strip.kind === 'banner' && strip.tall ? 2 : 1;
}

function option(id, cls, selected, pos, size, ...children) {
  return el(
    'div',
    {
      class: `line row ${cls}${selected ? ' selected' : ''}${cutMarks.has(id) ? ' cut' : ''}`,
      attrs: {
        role: 'option',
        'aria-selected': selected ? 'true' : 'false',
        'aria-posinset': String(pos),
        'aria-setsize': String(size),
        id: `${prefix}strip-${id}`,
      },
      data: { id },
    },
    ...children,
  );
}

function stripRow(strip, selected, pos, size) {
  if (strip.kind === 'break') {
    // A day break parked in the Boneyard: no day, no date (decisions.md, entry 57).
    return option(strip.id, 'day-break parked', selected, pos, size, el('span', { text: 'Day break, parked' }), el('span', { class: 'totals', text: 'no day' }));
  }
  if (strip.kind === 'banner') {
    return option(
      strip.id,
      `banner${bannerLook(strip)}${strip.tall ? ' h2' : ''}${isFound(strip, foundMarks) ? ' found' : ''}`,
      selected,
      pos,
      size,
      el('span', { class: 'banner-text', text: strip.text, title: strip.text }),
      el('span', { class: 'banner-time', text: strip.duration && strip.duration !== '0:00' ? strip.duration : '' }),
    );
  }
  const flags = strip.red_flags || [];
  const clash = strip.conflicts || [];
  // The first clashing type's color paints the mark (decisions.md, entry 84).
  const flagType = flags.length && strip.red_flag_types && strip.red_flag_types.length ? ` rft-${Number(strip.red_flag_types[0])}` : '';
  const lines = gridNow.lines;
  const first = gridNow.cells.find((g) => g.line === 0);
  // An omitted scene (a script revision left it out, decisions.md, entry 69): struck and
  // dimmed, and said in its title.
  const row = option(
    strip.id,
    `scene ${strip.tone}${colorClass(strip, palette)}${flags.length ? ` flagged${flagType}` : ''}${clash.length ? ' conflicted' : ''}${strip.omitted ? ' omitted' : ''}${strip.shot ? ` shot-${strip.shot}` : ''}${isFound(strip, foundMarks) ? ' found' : ''}${lines > 1 ? ` h${lines}` : ''}`,
    selected,
    pos,
    size,
    // Every field in its place on the strip's lines (strips.js, the shared cell renderer).
    // The first line's first field is set apart, as the scene cell is in the mark (decision 206).
    // The conflict and red flag marks lead it, whatever it shows, and a part
    // of a scene spanning days says which (decisions.md, entry 76).
    ...gridNow.cells.map((g) => {
      const lead = g === first;
      const part = lead && strip.parts > 1 ? el('span', { class: 'part', text: ` ${strip.part}/${strip.parts}`, title: `Part ${strip.part} of ${strip.parts}: the scene spans days` }) : null;
      return stripCell('span', g, cellText(g, strip.cells), { extra: ` sc-${g.k}${lead ? ' lead' : ''}`, before: lead ? [clash.length ? conflictMark(clash) : null, flags.length ? redFlagMark(flags, flagType) : null] : null, after: part });
    }),
  );
  if (strip.omitted) row.title = `Scene ${strip.scene || '(no number)'}, omitted: the last script revision no longer has it`;
  // What was shot of it (M5): marked, dimmed once finished, and said in its title.
  else if (strip.shot_text) row.title = `Scene ${strip.scene || '(no number)'}: ${strip.shot_text}`;
  return row;
}

/**
 * The red flag mark on a strip whose day a flag rules out (src/redflags.rs): its
 * accessible name and title give each clash's element, type, dates and firmness.
 */
function redFlagMark(flags, typeClass = '') {
  const text = flags.length === 1 ? `Red flag: ${flags[0]}` : `${flags.length} red flags: ${flags.join('; ')}`;
  return el('span', { class: `rf-mark${typeClass}`, text: STRIP_MARK.flag, title: text, attrs: { role: 'img', 'aria-label': text } });
}

/** The conflict mark on a strip (decisions.md, entry 83): its title says each conflict. */
export function conflictMark(list) {
  const text = list.length === 1 ? `Conflict: ${list[0]}` : `${list.length} conflicts: ${list.join('; ')}`;
  return el('span', { class: 'cf-mark', text: STRIP_MARK.conflict, title: text, attrs: { role: 'img', 'aria-label': text } });
}

function breakRow(day, selected, pos, size) {
  const scenes = day.scenes === 1 ? '1 scene' : `${day.scenes} scenes`;
  return option(
    day.end.id,
    'day-break',
    selected,
    pos,
    size,
    el('span', { text: `End of day ${day.number}` }),
    el('span', { class: 'extras', text: dayExtras(day, lookNow) }),
    showNow.totals ? el('span', { class: 'totals', text: `${scenes} · ${day.pages} pages · ${day.time}` }) : null,
  );
}

function dayHead(day) {
  const when = day.date ? dayDate(day.weekday, day.date) : 'No date';
  return el('div', { class: 'line day-head' }, el('strong', { text: `Day ${day.number}` }), el('span', { text: when }));
}

/** The column headings: the first line's fields; the later lines' in the title. */
function columns() {
  const later = gridNow.cells.filter((g) => g.line > 0);
  return el(
    'div',
    { class: 'columns', attrs: { 'aria-hidden': 'true' }, title: later.length ? `Lines 2 to ${gridNow.lines}: ${later.map((g) => labelOf(g.c)).join(', ')}` : null },
    ...gridNow.cols.map((c) => el('span', { class: cellClass(c), text: c.field === 'text' ? c.text || '' : c.field === 'spacer' ? '' : labelOf(c) })),
  );
}

/**
 * Builds the board in `host`, which scrolls inside `scroller`. `actions` has
 * onPick(id, { range, toggle }), onOpen(row), onStart(subId, date),
 * onAutoBreaks(subId, by, limit), onRemoveBreaks(subId), dragSet(id) (the rows a drag from row `id` carries: the
 * selection when `id` is in it, else just `id`, which it selects) and
 * onDrop(rows, { sub_board, index }). Returns
 * { render(board, selectedId, picked, cut), select(id, picked), update(), setFound(found), dragging() }.
 */
export function createBoard(host, scroller, actions, opts = {}) {
  /** This board's drawing state, swapped into the module values before each draw. */
  const own = { prefix: opts.prefix || '', cut: new Set(), found: NOTHING_FOUND, palette: new Map(), grid: stripGrid(DEFAULT_LAYOUT), look: lookNow, shoot: '' };
  const use = () => {
    prefix = own.prefix;
    foundMarks = own.found;
    shootTextNow = own.shoot;
    cutMarks = own.cut;
    palette = own.palette;
    gridNow = own.grid;
    lookNow = own.look;
  };
  /**
   * Each strip list: { key, sub, len, node, lines, drawn: { first, last } }. A line is
   * { t, at, ... }: `at` is the strip index a drop just above it lands on.
   */
  let lists = [];
  /** Lists by key, with the empty ones too: key -> { sub, len, list (or null), empty }. */
  let byKey = new Map();
  /** The selectable rows in screen order: [{ id, kind, list, index, sheet, strip }]. */
  let rows = [];
  /** Row id -> { l (list), line, pos (place in rows) }. */
  let where = new Map();
  /** The focused row, and every selected row. */
  let selected = null;
  let picked = new Set();
  let virtual = false;
  /** The drop being shown: { key, l, line, edge, refused } (l and line null for an empty list). */
  let drop = null;

  // One listener each for the whole board instead of one per row.
  const rowOf = (e) => {
    const node = e.target.closest('.row[data-id]');
    return node && host.contains(node) ? Number(node.dataset.id) : null;
  };
  let swallowClick = false;
  host.addEventListener('click', (e) => {
    if (swallowClick) {
      swallowClick = false;
      return;
    }
    const id = rowOf(e);
    if (id !== null) actions.onPick(id, { range: e.shiftKey, toggle: e.metaKey || e.ctrlKey });
  });
  // A scene opens its breakdown sheet on a double-click; a banner opens its editor.
  host.addEventListener('dblclick', (e) => {
    if (e.shiftKey || e.metaKey || e.ctrlKey) return;
    const w = where.get(rowOf(e));
    if (w) actions.onOpen(rows[w.pos]);
  });

  function lineNode(line, l, k) {
    use();
    const size = rows.length;
    let node;
    switch (line.t) {
      case 'day':
        node = dayHead(line.day);
        break;
      case 'break':
        node = breakRow(line.day, picked.has(line.day.end.id), line.pos + 1, size);
        break;
      case 'strip':
        node = stripRow(line.st, picked.has(line.st.id), line.pos + 1, size);
        break;
      case 'shoot':
        node = el('div', { class: 'line shoot-day' }, el('strong', { text: shootDayText(line.day, line.of, shootTextNow) }));
        break;
      default:
        node = el('div', { class: 'line caption', text: line.text });
    }
    node.dataset.line = String(k);
    if (drop && drop.l === l && drop.line === k) node.classList.add(dropClass(drop));
    return node;
  }

  const pad = (cls) => el('div', { class: `pad ${cls}`, attrs: { 'aria-hidden': 'true' } });

  function padRules() {
    let css = '';
    lists.forEach((l, i) => {
      const { first, last } = l.drawn;
      const above = l.tops[first];
      const below = l.tops[l.lines.length] - l.tops[last + 1];
      css += `#${own.prefix}bl-${i} > .pad-top { height: ${above * LINE_PX}px; } #${own.prefix}bl-${i} > .pad-bottom { height: ${below * LINE_PX}px; }\n`;
    });
    setRules(`${own.prefix}board-pad`, css);
  }

  /** Draws the lines now in view in each list (all of them when the board is small). */
  function update(force = false) {
    if (!virtual || !host.getClientRects().length) return;
    use();
    const view = scroller.getBoundingClientRect();
    const height = scroller.clientHeight || 600;
    const changed = [];
    for (const l of lists) {
      const n = l.lines.length;
      const top = l.node.getBoundingClientRect().top + l.node.clientTop - view.top;
      let first = Math.max(0, lineAt(l.tops, Math.floor(-top / LINE_PX)) - OVERSCAN);
      let last = Math.min(n - 1, lineAt(l.tops, Math.ceil((height - top) / LINE_PX)) + OVERSCAN);
      if (first > last) [first, last] = [0, -1];
      if (!force && first === l.drawn.first && last === l.drawn.last) continue;
      l.drawn = { first, last };
      changed.push(l);
    }
    if (!changed.length) return;
    padRules();
    for (const l of changed) {
      const li = lists.indexOf(l);
      const nodes = [];
      for (let i = l.drawn.first; i <= l.drawn.last; i++) nodes.push(lineNode(l.lines[i], li, i));
      l.node.replaceChildren(pad('pad-top'), ...nodes, pad('pad-bottom'));
    }
  }

  // Browsers send at most one scroll event a frame, and update does nothing until the
  // lines in view change.
  scroller.addEventListener('scroll', () => {
    update();
    if (drag && drag.active) aim();
  });
  window.addEventListener('resize', () => update());

  /** Scrolls the scroller just enough to show row `id`, then draws what is in view. */
  function reveal(id) {
    const w = where.get(id);
    if (!w || !host.getClientRects().length) return;
    const view = scroller.getBoundingClientRect();
    const l = lists[w.l];
    const y = l.node.getBoundingClientRect().top + l.node.clientTop - view.top + scroller.scrollTop + l.tops[w.line] * LINE_PX;
    const tall = (l.tops[w.line + 1] - l.tops[w.line]) * LINE_PX;
    const height = scroller.clientHeight;
    if (y < scroller.scrollTop) scroller.scrollTop = Math.floor(y);
    else if (y + tall > scroller.scrollTop + height) scroller.scrollTop = Math.ceil(y + tall - height);
    update();
  }

  function mark(id, on) {
    const node = id === null ? null : document.getElementById(`${own.prefix}strip-${id}`);
    if (!node || !host.contains(node)) return;
    node.classList.toggle('selected', on);
    node.setAttribute('aria-selected', on ? 'true' : 'false');
  }

  /**
   * Focuses row `id` (or nothing with null) and selects the rows in `chosen` (the focused
   * row alone when not given), drawing the focused row and scrolling it into view.
   */
  function select(id, chosen) {
    const next = new Set(chosen || (id === null ? [] : [id]));
    for (const old of picked) if (!next.has(old)) mark(old, false);
    picked = new Set([...next].filter((x) => where.has(x)));
    selected = where.has(id) ? id : null;
    if (selected !== null) reveal(selected);
    for (const x of picked) mark(x, true);
    host.setAttribute('aria-activedescendant', selected === null ? '' : `${own.prefix}strip-${selected}`);
    host.setAttribute('aria-multiselectable', 'true');
  }

  /**
   * Draws `board` with row `selectedId` focused, the rows in `chosen` selected and those
   * in `cut` marked as cut, and
   * returns the selectable rows in screen order: [{ id, kind, list, index, sheet, strip }],
   * where `list` is a sub-board id or null for the Boneyard, `sheet` a scene's sheet id
   * and `strip` what the view sent. Rows that aren't drawn are listed too.
   */
  function render(board, selectedId, chosen, cut) {
    use();
    cutMarks = own.cut = new Set(cut || []);
    gridNow = own.grid = stripGrid(board.layout);
    // The columns' widths (fixed words only, from strips.js), the strip's lines and where
    // each field sits on them (numbers only): CSS can't take them from the page.
    setRules(
      `${own.prefix}board-layout`,
      `#${host.id} .row.scene, #${host.id} .columns { grid-template-columns: ${boardTracks(gridNow.cols)}; }\n` +
        `#${host.id} .row.scene { grid-template-rows: repeat(${gridNow.lines}, minmax(0, 1fr)); }\n${gridRules(`#${host.id} .row.scene`, gridNow)}`,
    );
    // The layout's text size step (decisions.md, entry 87): a class on the board.
    for (const t of ['small', 'large']) host.classList.toggle(`ts-${t}`, board.layout_size === t);
    // The user's strip colors (decisions.md, entry 60): every color in use, painted once,
    // so lines drawn later while scrolling find their class.
    palette = own.palette = new Map();
    for (const sb of board.sub_boards) for (const st of sb.days.flatMap((d) => d.strips).concat(sb.unscheduled)) colorClass(st, palette);
    for (const st of board.boneyard) colorClass(st, palette);
    paintStrips(palette, board.colors, host.id === 'board' ? {} : { scope: `#${host.id}`, name: `strip-colors-${host.id}` });
    lookNow = own.look = board.day_look || lookNow;
    shootTextNow = own.shoot = board.shoot_text || '';
    rows = [];
    where = new Map();
    lists = [];
    byKey = new Map();
    selected = selectedId;
    picked = new Set(chosen || (selectedId === null ? [] : [selectedId]));
    drop = null;
    const nodes = [];

    /** Adds a list of lines to `node`, recording where each selectable row is. */
    const addList = (key, sub, len, lines) => {
      const node = el('div', { class: 'strips', attrs: { id: `${own.prefix}bl-${lists.length}` } });
      const l = { key, sub, len, node, lines, tops: lineTops(lines), drawn: { first: 0, last: -1 } };
      lists.push(l);
      byKey.set(key, { sub, len, list: l, empty: null });
      return node;
    };
    const emptyList = (key, sub, text) => {
      const empty = el('div', { class: 'empty', text });
      byKey.set(key, { sub, len: 0, list: null, empty });
      return el('div', { class: 'strips' }, empty);
    };
    const addRow = (lines, row, line) => {
      line.pos = rows.length;
      // A scene takes its layout's lines and a two-line banner two (decisions.md,
      // entries 65, 78 and 87): one line of the list, that many board lines high.
      if (line.t === 'strip') line.h = heightOf(line.st);
      where.set(row.id, { l: lists.length, line: lines.length, pos: rows.length });
      rows.push(row);
      lines.push(line);
    };
    const stripRowData = (st, list) => ({ id: st.id, kind: st.kind, list, index: st.index, sheet: st.sheet ?? null, strip: st });

    // Hidden lines (boardShow) are left out of the lines and the selectable rows; a drop
    // still lands by strip index, so it goes where the neighbors shown say.
    const shownStrip = (st) => showNow.banners || st.kind !== 'banner';
    for (const sb of board.sub_boards) {
      const lines = [];
      let at = 0;
      for (const day of sb.days) {
        // The day's first line: the shoot day strip when the board shows it, in place of
        // our day heading (decisions.md, entries 67 and 148), else the heading.
        const lead = dayLead(lookNow, showNow);
        if (lead === 'shoot') lines.push({ t: 'shoot', day, of: sb.days.length, at });
        else if (lead === 'day') lines.push({ t: 'day', day, at });
        for (const st of day.strips) if (shownStrip(st)) addRow(lines, stripRowData(st, sb.id), { t: 'strip', st, at: st.index });
        if (showNow.breaks) addRow(lines, { id: day.end.id, kind: 'break', list: sb.id, index: day.end.index, sheet: null, strip: day.end }, { t: 'break', day, at: day.end.index });
        at = day.end.index + 1;
      }
      if (sb.unscheduled.length) {
        lines.push({ t: 'caption', text: sb.days.length ? 'After the last day break: not scheduled' : 'No day breaks yet: not scheduled', at });
        for (const st of sb.unscheduled) if (shownStrip(st)) addRow(lines, stripRowData(st, sb.id), { t: 'strip', st, at: st.index });
      }
      const key = listKey(sb.id);
      const strips = lines.length
        ? addList(key, sb.id, sb.len, lines)
        : emptyList(key, sb.id, 'No strips on this sub-board. Drag scenes here from the Boneyard, or move them with Option or Alt and the arrow keys.');

      // A text field, not a date control (decision 49); Rust reads the typed date.
      const start = dateInput({ class: 'start-date', value: showDate(sb.start, true), 'aria-label': `Start date of ${sb.name}` });
      start.addEventListener('change', () => actions.onStart(sb.id, start.value));
      start.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') start.blur();
      });
      // Automatic day breaks: by pages, shoot time or screen time (decisions.md, entry 56).
      const hint = () => BREAK_BY.find((x) => x[0] === breakBy)[2];
      const limit = el('input', {
        class: 'short',
        attrs: { type: 'text', value: breakLimit[breakBy], 'aria-label': hint(), title: hint() },
        on: { input: (e) => (breakLimit[breakBy] = e.target.value) },
      });
      const by = el(
        'select',
        {
          attrs: { 'aria-label': 'Break days by' },
          on: {
            change: (e) => {
              breakBy = e.target.value;
              limit.value = breakLimit[breakBy];
              limit.title = hint();
              limit.setAttribute('aria-label', hint());
            },
          },
        },
        ...BREAK_BY.map(([v, label]) => el('option', { text: label, attrs: { value: v, ...(v === breakBy ? { selected: '' } : {}) } })),
      );
      const runBreaks = () => actions.onAutoBreaks(sb.id, breakBy, limit.value);
      limit.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') runBreaks();
      });
      const shootDays = sb.days.length === 1 ? '1 shoot day' : `${sb.days.length} shoot days`;
      nodes.push(
        el(
          'section',
          { class: 'sub-board', attrs: { 'aria-label': sb.name }, data: { list: key } },
          el(
            'div',
            { class: 'sub-head' },
            el('h2', { text: `${board.name}: ${sb.name}` }),
            el('span', { class: 'field', text: shootDays }),
            el('label', { class: 'field' }, 'Starts', start),
            el(
              'span',
              { class: 'field' },
              el('label', { class: 'field' }, 'Break by', by),
              limit,
              el('button', { class: 'btn', text: 'Break', attrs: { type: 'button', title: 'Replace the day breaks: a day closes once it reaches the limit' }, on: { click: runBreaks } }),
              el('button', {
                class: 'btn',
                text: 'Remove Breaks',
                attrs: { type: 'button', title: 'Remove every day break on this sub-board', disabled: !sb.days.length },
                on: { click: () => actions.onRemoveBreaks(sb.id) },
              }),
            ),
          ),
          columns(),
          strips,
        ),
      );
    }

    const yardLines = [];
    for (const st of board.boneyard) addRow(yardLines, stripRowData(st, null), { t: 'strip', st, at: st.index });
    const yard = yardLines.length ? addList(BONEYARD, null, board.boneyard.length, yardLines) : emptyList(BONEYARD, null, 'The Boneyard is empty.');
    nodes.push(
      el(
        'section',
        { class: 'sub-board', attrs: { 'aria-label': 'Boneyard' }, data: { list: BONEYARD } },
        el('div', { class: 'sub-head' }, el('h2', { text: 'Boneyard' }), el('span', { class: 'field', text: 'Unscheduled scenes and parked day breaks' })),
        columns(),
        yard,
      ),
    );

    const total = lists.reduce((a, l) => a + l.lines.length, 0);
    virtual = Boolean(dynamicStyles) && total > ALL_LINES_UP_TO;
    if (virtual) {
      // Every list starts as one spacer of its full height, so the page keeps its length
      // (and the scroll position) until update() draws the lines in view.
      padRules();
      for (const l of lists) l.node.append(pad('pad-top'), pad('pad-bottom'));
      const keep = scroller.scrollTop;
      host.replaceChildren(...nodes);
      scroller.scrollTop = keep;
      update(true);
    } else {
      setRules(`${own.prefix}board-pad`, '');
      lists.forEach((l, li) => {
        l.drawn = { first: 0, last: l.lines.length - 1 };
        l.node.append(...l.lines.map((line, k) => lineNode(line, li, k)));
      });
      host.replaceChildren(...nodes);
    }
    if (!where.has(selected)) selected = null;
    picked = new Set([...picked].filter((x) => where.has(x)));
    host.setAttribute('aria-activedescendant', selected === null ? '' : `${own.prefix}strip-${selected}`);
    host.setAttribute('aria-multiselectable', 'true');
    return rows;
  }

  // -------------------------------------------------------------------------
  // Dragging

  /** The press or drag under way: { id, x, y, active, rows, refused, lastX, lastY, frame }. */
  let drag = null;

  const dropClass = (d) => `drop-${d.edge}${d.refused ? ' drop-refused' : ''}`;

  function dropNode(d) {
    if (!d) return null;
    if (d.l === null) return byKey.get(d.key)?.empty || null;
    return lists[d.l].node.querySelector(`:scope > [data-line="${d.line}"]`);
  }

  function showDrop(next) {
    const same = drop && next && drop.key === next.key && drop.l === next.l && drop.line === next.line && drop.edge === next.edge && drop.refused === next.refused;
    if (same) return;
    const old = dropNode(drop);
    if (old) old.classList.remove('drop-before', 'drop-after', 'drop-into', 'drop-refused');
    drop = next;
    const node = dropNode(drop);
    if (node) for (const c of dropClass(drop).split(' ')) node.classList.add(c);
  }

  /** Where a drop at the pointer lands: { key, sub, index, l, line, edge }, or null. */
  function target(x, y) {
    const hit = document.elementFromPoint(x, y);
    const section = hit && hit.closest ? hit.closest('section.sub-board[data-list]') : null;
    if (!section || !host.contains(section)) return null;
    const entry = byKey.get(section.dataset.list);
    if (!entry) return null;
    const key = section.dataset.list;
    if (!entry.list) return { key, sub: entry.sub, index: 0, l: null, line: null, edge: 'into' };
    const l = entry.list;
    const li = lists.indexOf(l);
    const rect = l.node.getBoundingClientRect();
    const top = rect.top + l.node.clientTop;
    const n = l.lines.length;
    // In board lines from the list's top; a line may be several high (lineTops).
    const raw = (y - top) / LINE_PX;
    if (raw < 0) return { key, sub: l.sub, index: l.lines[0].at, l: li, line: 0, edge: lineEdge(l.lines[0], 'before') };
    if (raw >= l.tops[n]) return { key, sub: l.sub, index: l.len, l: li, line: n - 1, edge: 'after' };
    const k = lineAt(l.tops, Math.floor(raw));
    const line = l.lines[k];
    const lower = (raw - l.tops[k]) / (l.tops[k + 1] - l.tops[k]) >= 0.5;
    // A day header, shoot day strip or caption: a drop lands below it, as the first strip of its run.
    if (heads(line)) return { key, sub: l.sub, index: line.at, l: li, line: k, edge: 'after' };
    return lower ? { key, sub: l.sub, index: line.at + 1, l: li, line: k, edge: 'after' } : { key, sub: l.sub, index: line.at, l: li, line: k, edge: 'before' };
  }

  /** Lines that head a run of strips (not strips or breaks) take a drop below them. */
  function heads(line) {
    return line.t === 'day' || line.t === 'shoot' || line.t === 'caption';
  }
  function lineEdge(line, edge) {
    return heads(line) ? 'after' : edge;
  }

  /** Works out the drop at the last pointer place and shows it. */
  function aim() {
    if (!drag || !drag.active) return;
    const t = target(drag.lastX, drag.lastY);
    drag.to = t;
    if (!t) {
      showDrop(null);
      return;
    }
    // Banners never go in the Boneyard; scenes and day breaks (parked) do.
    t.refused = t.sub === null && drag.rows.some((r) => r.kind === 'banner');
    showDrop(t);
  }

  function ghostAt(x, y) {
    setRules('board-ghost', `#drag-ghost { transform: translate(${Math.round(x) + 14}px, ${Math.round(y) + 10}px); }`);
  }

  /** Scrolls the board while the pointer rests near the scroller's top or bottom edge. */
  function edgeScroll() {
    if (!drag || !drag.active) return;
    const r = scroller.getBoundingClientRect();
    let by = 0;
    if (drag.lastY < r.top + EDGE_PX) by = -Math.ceil(((r.top + EDGE_PX - drag.lastY) / EDGE_PX) * EDGE_SPEED);
    else if (drag.lastY > r.bottom - EDGE_PX) by = Math.ceil(((drag.lastY - (r.bottom - EDGE_PX)) / EDGE_PX) * EDGE_SPEED);
    if (by) {
      const before = scroller.scrollTop;
      scroller.scrollTop = before + Math.max(-EDGE_SPEED, Math.min(EDGE_SPEED, by));
      if (scroller.scrollTop !== before) {
        update();
        aim();
      }
    }
    drag.frame = setTimeout(edgeScroll, EDGE_TICK_MS);
  }

  function begin() {
    drag.rows = actions.dragSet(drag.id);
    if (!drag.rows.length) {
      drag = null;
      return;
    }
    drag.active = true;
    host.classList.add('dragging');
    const first = drag.rows[0];
    const what =
      drag.rows.length > 1
        ? `${drag.rows.length} strips`
        : first.kind === 'scene'
          ? `Scene ${first.strip.scene || '(no number)'}`
          : first.kind === 'banner'
            ? 'Banner'
            : 'Day break';
    ghostAt(drag.lastX, drag.lastY);
    document.body.append(el('div', { class: 'drag-ghost', text: what, attrs: { id: 'drag-ghost', 'aria-hidden': 'true' } }));
    aim();
    drag.frame = setTimeout(edgeScroll, EDGE_TICK_MS);
  }

  function end(commit) {
    if (!drag) return;
    const d = drag;
    drag = null;
    if (d.frame) clearTimeout(d.frame);
    if (!d.active) return;
    swallowClick = true;
    setTimeout(() => (swallowClick = false), 0);
    host.classList.remove('dragging');
    const ghost = document.getElementById('drag-ghost');
    if (ghost) ghost.remove();
    setRules('board-ghost', '');
    const to = d.to;
    showDrop(null);
    if (commit && to) actions.onDrop(d.rows, { sub_board: to.sub, index: to.index, refused: to.refused });
  }

  host.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
    const id = rowOf(e);
    if (id === null) return;
    drag = { id, x: e.clientX, y: e.clientY, lastX: e.clientX, lastY: e.clientY, active: false, rows: [], to: null, frame: 0, pointer: e.pointerId };
  });
  host.addEventListener('pointermove', (e) => {
    if (!drag || e.pointerId !== drag.pointer) return;
    drag.lastX = e.clientX;
    drag.lastY = e.clientY;
    if (!drag.active) {
      if (Math.abs(e.clientX - drag.x) + Math.abs(e.clientY - drag.y) < DRAG_START_PX) return;
      try {
        host.setPointerCapture(e.pointerId);
      } catch {
        // The pointer may already be gone; the drag still works while it is over the board.
      }
      begin();
      if (!drag) return;
    }
    e.preventDefault();
    ghostAt(e.clientX, e.clientY);
    aim();
  });
  host.addEventListener('pointerup', (e) => {
    if (drag && e.pointerId === drag.pointer) end(true);
  });
  host.addEventListener('pointercancel', () => end(false));
  host.addEventListener('lostpointercapture', () => {
    if (drag && drag.active) end(false);
  });
  // Escape puts the strips back where they were.
  document.addEventListener(
    'keydown',
    (e) => {
      if (drag && drag.active && e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        end(false);
      }
    },
    true,
  );

  /**
   * Marks the strips the Find panel's results name: `found` is { sheets, strips } (sets of
   * sheet ids and banner strip ids), or null for none. Lines drawn later take it too.
   */
  function setFound(found) {
    own.found = found || NOTHING_FOUND;
    for (const r of rows) {
      const node = document.getElementById(`${own.prefix}strip-${r.id}`);
      if (node && host.contains(node)) node.classList.toggle('found', isFound(r.strip, own.found));
    }
  }

  return { render, select, update, setFound, dragging: () => Boolean(drag && drag.active) };
}

/** Each list's length, keyed by listKey: sub-boards in order, then the Boneyard. */
function listOrder(board) {
  const order = board.sub_boards.map((sb) => ({ key: listKey(sb.id), sub: sb.id, len: sb.len }));
  order.push({ key: BONEYARD, sub: null, len: board.boneyard.length });
  return order;
}

/**
 * Where a strip goes when it moves one place up (`delta` -1) or down (+1), as the
 * arguments move_strips takes ({ sub_board, index }, counting positions before the move),
 * or null when it is already at the very top or bottom. At the edge of a list it moves to
 * the end of the list above, or the start of the list below.
 */
export function oneStep(board, row, delta) {
  const order = listOrder(board);
  const at = order.findIndex((l) => l.key === listKey(row.list));
  if (at < 0) return null;
  const list = order[at];
  if (delta < 0) {
    if (row.index > 0) return { sub_board: list.sub, index: row.index - 1 };
    const prev = order[at - 1];
    return prev ? { sub_board: prev.sub, index: prev.len } : null;
  }
  if (row.index < list.len - 1) return { sub_board: list.sub, index: row.index + 2 };
  const next = order[at + 1];
  return next ? { sub_board: next.sub, index: 0 } : null;
}

/**
 * Where a selection goes when it moves one place: a block of rows (in screen order) moves
 * as its top row does going up and as its bottom row does going down, so a contiguous
 * selection keeps together and steps over one neighbor. Rows apart from each other gather
 * there.
 */
export function blockStep(board, rows, delta) {
  if (!rows.length) return null;
  return oneStep(board, delta < 0 ? rows[0] : rows[rows.length - 1], delta);
}
