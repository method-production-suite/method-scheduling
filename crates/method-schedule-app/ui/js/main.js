// The page: the top bar, the Board, Breakdown, Elements, Day out of days, Calendar, Reports,
// Conflicts and Red flags tabs, the Find panel, keys and menu commands (File > Export and Print, What
// Changed and Merge Changes From, Edit > Find, Replace and Explain This Count among them).
// Rust owns the schedule; every change is a command, and the page draws the view it gets
// back (src/app.rs, src/view.rs).

import { invoke, listen, connected } from './api.js';
import { setDateFormat } from './dates.js';
import { $, el, isTextField } from './dom.js';
import { matchShortcut, menuRunsAppKeys, shortcutById, withShortcut, keysFor, keyText } from './shortcuts.js';
import { createBoard, blockStep, boardShow, boardShowDialog, isFound } from './board.js';
import { createBoardsBar, manageBoards, bannerDialog, sortDialog, freeName, dayLookDialog } from './boards.js';
import { layoutsDialog } from './layouts.js';
import { createDood } from './dood.js';
import { initSafety } from './safety.js';
import { initUpdates } from './updates.js';
import { initOutside } from './outside.js';
import { isModalOpen, confirmDialog, askText, customDialog } from './modal.js';
import { createBreakdown, fieldOrderDialog } from './breakdown.js';
import { createSplit } from './split.js';
import { createBoard2 } from './board2.js';
import { createImages } from './images.js';
import { shortcutSheet } from './keysheet.js';
import { guideSheet, noticesSheet } from './guide.js';
import { settingsDialog } from './settings.js';
import { createElements } from './elements.js';
import { createCalendar } from './calendar.js';
import { createReports, EXPORTABLE } from './reports.js';
import { exportReport, exportBudget } from './export.js';
import { whatChanged, mergeChanges } from './diffmerge.js';
import { createFind } from './find.js';
import { createConflicts } from './conflicts.js';
import { createRedFlags, manageTypes, paintFlagTypes, flagDialog } from './redflags.js';
import { mergeDialog, layoutDialog, layoutKey } from './strips.js';
import { planCommands } from './plan.js';
import { stripColorsDialog, combosOf } from './colors.js';
import { printBoard } from './boardprint.js';
import { openMenu, isMenuOpen } from './ctxmenu.js';
import { loadHost, isWeb } from './host.js';
import { bannerPlace, breakdownImagesToggle, deleteQuestion, doodSortAfter, foundOn, sheetBack, splitToggle, stripMenu, themeAttr } from './logic.js';

const state = {
  view: null,
  tab: 'board',
  /** The focused strip's id, kept across redraws. */
  selected: null,
  /** Every selected strip's id (the focused one among them), and where a Shift range starts. */
  picked: new Set(),
  anchor: null,
  /** The sub-board the Elements, Day out of days, Calendar and Reports tabs show. */
  subBoard: null,
  /** The selectable rows in screen order, from the last draw. */
  rows: [],
  /** The Breakdown tab's sheet, and its place in board order (where to look if it goes). */
  sheet: null,
  sheetIndex: null,
  /** The board's selection changed since the Breakdown tab last showed a sheet: the tab follows it (decision 51). */
  followBoard: false,
  /** The Elements tab's category. */
  category: null,
  /** The Day out of days tab's category (null: Cast), or the header field it shows instead. */
  doodCategory: null,
  doodHeader: null,
  /** The header field the Elements tab shows instead of a category (set, location, unit, episode, I/E, time of day, script day, sequence). */
  header: null,
  /** The Calendar tab's month, [year, month] (null: the start date's month), and its sub-board. */
  calMonth: null,
  calSub: null,
  /** Where the board was scrolled to when another tab opened. */
  boardTop: 0,
};

const TABS = ['board', 'breakdown', 'elements', 'dood', 'calendar', 'reports', 'conflicts', 'redflags'];

// ---------------------------------------------------------------------------
// Status line

let statusTimer = 0;

/**
 * Shows `text` on the status line, or clears it. The line keeps its height either way
 * (styles.css, `.status-line`), so nothing below it moves (decision 48).
 */
function say(text, isError = false) {
  const s = $('status');
  clearTimeout(statusTimer);
  const quiet = () => {
    s.textContent = '';
    s.title = '';
    s.classList.remove('error');
    s.classList.add('quiet');
  };
  if (!text) {
    quiet();
    return;
  }
  s.textContent = String(text);
  s.title = String(text);
  s.classList.toggle('error', isError);
  s.classList.remove('quiet');
  if (!isError) statusTimer = setTimeout(quiet, 6000);
}

// ---------------------------------------------------------------------------
// Drawing

/** Changes made outside the app (outside.js), set up with the commands below. */
let outside = null;

function fileState(file) {
  $('doc-file').textContent = file.name;
  $('doc-dirty').hidden = !file.dirty;
  // Stays up while autosave is paused because the file changed on disk.
  const conflict = $('conflict');
  const was = Boolean(state.view && state.view.file && state.view.file.conflict);
  conflict.textContent = file.conflict
    ? file.dirty
      ? `${file.name} changed on disk after it was opened here, so autosave is not writing into it. Your changes are kept in a recovery copy. Save settles it: keep yours, load the version on disk or merge the two.`
      : `${file.name} changed on disk after it was opened here, so autosave is not writing into it. Save settles it: keep yours or load the version on disk.`
    : '';
  conflict.hidden = !file.conflict;
  if (state.view) state.view.file = file;
  // Autosave just found the change: ask now when this window is in front (decisions.md,
  // entry 179); otherwise the check runs when it gets focus.
  if (file.conflict && !was && document.hasFocus() && outside) outside.check(false);
}

/** The sub-board shown: the one picked while it is on the board shown, else its first. */
function syncSubBoard() {
  const subs = state.view.board.sub_boards;
  if (!subs.some((sb) => sb.id === state.subBoard)) state.subBoard = subs.length ? subs[0].id : null;
}

/** "Board 1, Main": the board shown and the sub-board picked, for messages. */
function whereText() {
  const b = state.view.board;
  const sb = b.sub_boards.find((x) => x.id === state.subBoard);
  return sb ? `${b.name}, ${sb.name}` : b.name;
}

function show(view) {
  if (!view) return;
  state.view = view;
  if (view.sub_board !== null && view.sub_board !== undefined) state.subBoard = view.sub_board;
  syncSubBoard();
  boardsBar.render(view, state.subBoard);
  if (view.strip !== null && view.strip !== undefined) {
    state.followBoard = true;
    state.selected = view.strip;
    state.picked = new Set([view.strip]);
    state.anchor = view.strip;
  } else if (Array.isArray(view.strips) && view.strips.length) {
    // A duplicate or a paste: every new strip selected, the last one focused.
    state.followBoard = true;
    state.selected = view.strips[view.strips.length - 1];
    state.picked = new Set(view.strips);
    state.anchor = view.strips[0];
  }
  $('doc-title').textContent = view.title;
  fileState(view.file);
  safety.setToggle(view.autosave);
  $('btn-undo').disabled = !view.can_undo;
  $('btn-redo').disabled = !view.can_redo;
  const b = view.board;
  const scenes = b.scenes === 1 ? '1 scene' : `${b.scenes} scenes`;
  $('footer').textContent = `${scenes}, ${b.pages} pages, ${b.boneyard.filter((x) => x.kind === 'scene').length} in the Boneyard`;
  drawBoard();
  if (view.notice) say(view.notice);
  if (view.sheet !== null && view.sheet !== undefined) state.sheet = view.sheet;
  // Undo of New Sheet or Duplicate Sheet: back to the sheet shown before it, not the one
  // that took its place in the order (decisions.md, entry 152).
  const back = sheetBack(state.sheet, sheetsMade, (id) => sheetOnBoard(view.board, id));
  if (back !== state.sheet) {
    state.sheet = back;
    state.sheetIndex = null;
  }
  if (view.category !== null && view.category !== undefined) state.category = view.category;
  conflictsBadge(view.conflicts);
  redFlagsBadge();
  paintFlagTypes(view.red_flag_types);
  refreshTab();
  find.refresh();
}

/** Redraws the open tab after a change (the board is always drawn, and the pane beside it). */
function refreshTab() {
  if (state.tab === 'board') refreshSplit();
  else if (state.tab === 'dood') refreshDood();
  else if (state.tab === 'breakdown') sheetShown = refreshSheet();
  else if (state.tab === 'elements') elementsShown = refreshElements();
  else if (state.tab === 'calendar') calendarShown = refreshCalendar();
  else if (state.tab === 'reports') reportShown = refreshReport();
  else if (state.tab === 'conflicts') refreshConflicts();
  else if (state.tab === 'redflags') redFlagsShown = refreshRedFlags();
}

// ---------------------------------------------------------------------------
// The Conflicts tab (conflicts.js)

/** The tab's count of conflicts on the board shown; hidden at none. */
function conflictsBadge(n) {
  const badge = $('conflicts-badge');
  const count = Number.isSafeInteger(n) && n > 0 ? n : 0;
  badge.textContent = count ? String(count) : '';
  badge.hidden = !count;
  badge.title = count === 1 ? '1 conflict on this board' : `${count} conflicts on this board`;
}

async function refreshConflicts() {
  try {
    conflicts.render(await invoke('conflicts_view', { board: state.view.board.id }));
  } catch (e) {
    say(e, true);
  }
}

/** Picks sub-board `sub` of the board shown for the per-sub-board tabs. */
function pickSubBoard(sub) {
  if (sub === null || sub === undefined || sub === state.subBoard) return;
  if (!state.view.board.sub_boards.some((sb) => sb.id === sub)) return;
  state.subBoard = sub;
  boardsBar.render(state.view, state.subBoard);
  redFlagsBadge();
  // The pane beside the board follows, so its edits go to the sub-board it shows.
  if (state.tab === 'board') refreshSplit();
}

// ---------------------------------------------------------------------------
// Red flags (redflags.js; src/redflags.rs)

/**
 * The Red flags tab's count: the clashes on the sub-board picked in the board bar, which
 * the view carries for each sub-board of the board shown. Hidden at none.
 */
function redFlagsBadge() {
  const badge = $('redflags-badge');
  const sb = state.view && state.view.board.sub_boards.find((x) => x.id === state.subBoard);
  const n = sb ? sb.red_flags : 0;
  const count = Number.isSafeInteger(n) && n > 0 ? n : 0;
  badge.textContent = count ? String(count) : '';
  badge.hidden = !count;
  badge.title = sb ? (count === 1 ? `1 red flag clash on ${sb.name}` : `${count} red flag clashes on ${sb.name}`) : '';
}

/** Settles once the Red flags tab has drawn the latest list. */
let redFlagsShown = Promise.resolve();

async function refreshRedFlags() {
  try {
    redFlags.render(await invoke('red_flags_view', { sub_board: state.subBoard }));
  } catch (e) {
    say(e, true);
  }
}

/** Edit > Red Flag Types: the dialog, kept in step with the view after each change. */
function openRedFlagTypes() {
  return manageTypes({
    types: () => (state.view ? state.view.red_flag_types : []),
    missing: () => (state.view ? state.view.standard_types_missing || 0 : 0),
    standard: () => command(invoke('add_standard_red_flag_types')),
    add: (name) => command(invoke('add_red_flag_type', { name })),
    rename: (id, name) => command(invoke('rename_red_flag_type', { red_flag_type: id, name })),
    move: (id, to) => command(invoke('move_red_flag_type', { red_flag_type: id, to })),
    remove: (id) => command(invoke('delete_red_flag_type', { red_flag_type: id })),
    color: (id, color) => command(invoke('set_red_flag_type_color', { red_flag_type: id, color })),
  });
}

/** Shows element `element` of `category` on the Elements tab, with its red flags. */
async function goElement(category, element) {
  state.category = category;
  showTab('elements');
  await elementsShown;
  elements.select(element, false);
}

const redFlags = createRedFlags($('redflags'), {
  goStrip: (sub, strip) => goStrip(sub, strip),
  goDate: (sub, date) => goDate(sub, date),
  goElements: (category, element) => goElement(category, element),
  manageTypes: () => openRedFlagTypes(),
  types: () => (state.view ? state.view.red_flag_types : []),
  weekStart: () => prefs.week_start,
  addFlags: (flags) => command(invoke('add_red_flags', { flags })),
  deleteFlags: (flags) => command(invoke('delete_red_flags', { flags })),
  exportTable: () => exportReport('red_flags', say, state.subBoard, whereText()),
  print: (pdf) => printRedFlags(pdf),
});

/** Shows strip `strip` of the board shown on the Board tab, selected. */
function goStrip(sub, strip) {
  pickSubBoard(sub);
  showTab('board');
  select(strip);
}

/** Shows `date` (YYYY-MM-DD) on sub-board `sub`'s calendar, picked. */
async function goDate(sub, date, focus = true) {
  pickSubBoard(sub);
  const [y, m] = String(date).split('-').map(Number);
  state.calSub = state.subBoard;
  state.calMonth = [y, m];
  showTab('calendar');
  await calendarShown;
  await calendar.goTo(date, focus);
}

/** Shows sub-board `sub` on the Board tab with focus on its start date. */
function goStart(sub) {
  pickSubBoard(sub);
  showTab('board');
  const input = document.querySelector(`.sub-board[data-list="${Number(sub)}"] input.start-date`);
  if (input) {
    input.scrollIntoView({ block: 'nearest' });
    input.focus();
  }
}

const conflicts = createConflicts($('conflicts'), {
  goStrip: (sub, strip) => goStrip(sub, strip),
  goDate: (sub, date) => goDate(sub, date),
  goStart: (sub) => goStart(sub),
});

// ---------------------------------------------------------------------------
// Find and Replace (find.js)

/** Goes to a Find result; with `keepFocus` the search box keeps focus. */
async function goToHit(h, { keepFocus }) {
  try {
    if (h.board !== null && h.board !== undefined && h.board !== state.view.board.id) await command(invoke('show_board', { board: h.board }));
    if (h.kind === 'sheet') {
      if (state.tab === 'breakdown') breakdown.settle();
      state.sheet = h.sheet;
      state.sheetIndex = null;
      state.followBoard = false;
      showTab('breakdown');
      await sheetShown;
      breakdown.mark(h.field, !keepFocus);
    } else if (h.kind === 'element' || h.kind === 'category') {
      state.category = h.category;
      showTab('elements');
      await elementsShown;
      if (h.kind === 'element') elements.select(h.element, !keepFocus);
      else elements.focusCategory(!keepFocus);
    } else if (h.kind === 'banner' || h.kind === 'strip') {
      goStrip(h.sub_board, h.strip);
    } else if (h.kind === 'event' && h.date) {
      await goDate(h.sub_board, h.date, !keepFocus);
    }
  } catch (e) {
    say(e, true);
  }
}

/**
 * The Find panel's results on the board shown (decision 130): the scenes' sheets and the
 * banners' strips, marked on the board until the search changes or the panel closes.
 */
let foundNow = null;
/** The results last marked, for the board beside the board (decision 184). */
let foundHits = [];

function foundOf(hits) {
  return foundOn(hits, state.view ? state.view.board.id : null);
}

function markFound(hits) {
  foundHits = hits;
  board2.markFound();
  const next = foundOf(hits);
  const key = (f) => (f ? `${[...f.sheets].join(',')}|${[...f.strips].join(',')}` : '');
  if (key(next) === key(foundNow)) return;
  foundNow = next;
  board.setFound(foundNow);
}

/** Select on Board: every strip of the board shown that the results name, on the Board tab. */
function selectFound(hits) {
  const f = foundOf(hits);
  const ids = f ? state.rows.filter((r) => isFound(r.strip, f)).map((r) => r.id) : [];
  if (!ids.length) {
    say('No strip on this board is in the results.');
    return;
  }
  showTab('board');
  state.followBoard = true;
  state.selected = ids[0];
  state.anchor = ids[0];
  state.picked = new Set(ids);
  board.select(ids[0], state.picked);
  onBoardSelection();
  say(ids.length === 1 ? '1 strip selected.' : `${ids.length} strips selected.`);
}

const find = createFind($('findpanel'), {
  search: (query) => invoke('find_text', { query }),
  replace: async (query, withText, picks) => {
    const view = await invoke('replace_text', { query, with: withText, picks });
    const notice = view ? view.notice : null;
    if (view) {
      view.notice = null;
      say('');
      show(view);
    }
    return notice;
  },
  goTo: (hit, how) => goToHit(hit, how),
  found: (hits) => markFound(hits),
  selectOnBoard: (hits) => selectFound(hits),
  subBoard: () => {
    const sb = state.view && state.view.board.sub_boards.find((x) => x.id === state.subBoard);
    return sb ? { id: sb.id, name: sb.name } : null;
  },
  closed: () => {
    if (state.tab === 'board') $('board').focus({ preventScroll: true });
  },
});

// ---------------------------------------------------------------------------
// The Reports tab (reports.js), printing and exports (export.js)

/**
 * The user's display preferences (settings.json through `preferences`; decision 59): the
 * week start, the date format, what a calendar strip shows and Fountain lines per page
 * (decision 195). Never in the schedule.
 */
const prefs = { week_start: 'sun', date_format: 'iso', calendar_fields: ['scene', 'set', 'pages'], fountain_lines: 56, theme: 'paper' };

/** The look (decision 208): a data-theme on <html> that styles.css answers; none for Paper. */
function applyTheme(theme) {
  const t = themeAttr(theme);
  if (t) document.documentElement.dataset.theme = t;
  else delete document.documentElement.dataset.theme;
}

async function loadPrefs() {
  try {
    Object.assign(prefs, await invoke('preferences'));
  } catch {
    // The defaults stand.
  }
  setDateFormat(prefs.date_format);
  applyTheme(prefs.theme);
}

/** Saves preferences (`changes` merged in), then redraws the open tab. */
async function savePrefs(changes) {
  const next = await invoke('set_preferences', { prefs: { ...prefs, ...changes } });
  Object.assign(prefs, next);
  setDateFormat(prefs.date_format);
  applyTheme(prefs.theme);
  redrawForPrefs();
  return next;
}

/** The board (its day headings and start dates) and the open tab, in the preferences now. */
function redrawForPrefs() {
  if (state.view) drawBoard();
  refreshTab();
}

/**
 * Preferences saved in any window (every window hears `prefs-changed`): read them again
 * and redraw, unless this window has them already (it saved them).
 */
function prefsChanged(next) {
  if (next && typeof next === 'object' && JSON.stringify({ ...prefs, ...next }) === JSON.stringify(prefs)) return;
  loadPrefs().then(redrawForPrefs);
}

/** Settles once the Reports tab has drawn the latest data. */
let reportShown = Promise.resolve();

async function refreshReport() {
  try {
    reports.setList(await invoke('report_list'));
    const dto = await invoke('report_data', { sub_board: state.subBoard, index: reports.current(), week_start: prefs.week_start });
    reports.render(dto);
  } catch (e) {
    say(e, true);
  }
}

const reports = createReports($('reports'), {
  show: () => {
    reportShown = refreshReport();
    return reportShown;
  },
  save: (index, name, options) => command(invoke('save_report', { index, name, options }), () => reportShown),
  duplicate: (index) => command(invoke('duplicate_report', { index }), () => reportShown),
  move: (index, to) => command(invoke('move_report', { index, to }), () => reportShown),
  remove: (index) => command(invoke('delete_report', { index }), () => reportShown),
  production: (field, value) => command(invoke('set_production', { field, value }), () => reportShown),
  saveHeader: (design) => command(invoke('save_header_layout', { design }), () => reportShown),
  deleteHeader: (id) => command(invoke('delete_header_layout', { id }), () => reportShown),
  moveHeader: (id, to) => command(invoke('move_header_layout', { id, to }), () => reportShown),
  print: (pdf) => printReport(pdf),
  exportFile: (kind) => exportReport(EXPORTABLE[kind] || null, say, state.subBoard, whereText()),
  exportBudget: () => exportBudget(say, state.subBoard),
});

/** The Calendar tab's Print and Save as PDF: the Calendar report of the list, on the Reports tab. */
async function printCalendar(pdf) {
  try {
    reports.setList(await invoke('report_list'));
    reports.pickKind('calendar');
  } catch (e) {
    say(e, true);
    return;
  }
  await printReport(pdf);
}

/** The Red flags tab's Print and Save as PDF: the Red flags report of the list (every flag, then the clashes). */
async function printRedFlags(pdf) {
  try {
    reports.setList(await invoke('report_list'));
    if (!reports.pickKind('red_flags')) {
      say('The report list has no Red flags report. Add one on the Reports tab, then print.', true);
      return;
    }
  } catch (e) {
    say(e, true);
    return;
  }
  await printReport(pdf);
}

/** File > Print and Save as PDF: the Reports tab's report, laid out fresh from the schedule. */
let printing = false;
/** Print or Save as PDF of the board shown (decisions.md, entry 61), as reports print. */
async function printBoardNow(pdf) {
  if (printing || !state.view) return;
  printing = true;
  try {
    onBoardTab();
    const out = await printBoard({ board: state.view.board, picked: new Set(state.picked), title: state.view.title || 'Untitled', pdf, show: boardShow() });
    if (pdf && out && out.done && out.file) say(`Saved ${out.file}.`);
  } catch (e) {
    say(e, true);
  } finally {
    printing = false;
  }
}

/** File > Print and Save as PDF: the board on the Board tab, else the Reports tab's report. */
async function printReport(pdf) {
  if (state.tab === 'board') return printBoardNow(pdf);
  if (printing) return;
  printing = true;
  try {
    if (state.tab !== 'reports') showTab('reports');
    else reportShown = refreshReport();
    await reportShown;
    const out = await reports.print(pdf);
    if (pdf && out.done && out.file) say(`Saved ${out.file}.`);
  } catch (e) {
    say(e, true);
  } finally {
    printing = false;
  }
}

// ---------------------------------------------------------------------------
// The Calendar tab (calendar.js)

/** Settles once the Calendar tab has drawn the latest state. */
let calendarShown = Promise.resolve();

// The Calendar's weeks view (decision 111) is a per-viewer convenience, remembered in this
// browser; the month view is the default.
const CAL_SPAN_KEY = 'method-schedule.calendar.weeks';
state.calSpan = (() => {
  try {
    return window.localStorage.getItem(CAL_SPAN_KEY) === 'true';
  } catch {
    return false;
  }
})();

function setCalendarSpan(on) {
  state.calSpan = Boolean(on);
  try {
    window.localStorage.setItem(CAL_SPAN_KEY, String(state.calSpan));
  } catch {
    // Not remembered; the view still applies until the page reloads.
  }
  calendarShown = refreshCalendar();
  return calendarShown;
}

/**
 * The Calendar's Actions menu, Add Red Flag…: pick the element, then the red flag dialog
 * opens with the date filled in (src/redflags.rs, `set_red_flag`; one undo step).
 */
async function calendarRedFlag(date) {
  const dto = await invoke('calendar_view', { sub_board: state.subBoard, year: null, month: null, week_start: prefs.week_start, fields: prefs.calendar_fields, span: false });
  const els = dto.elements || [];
  if (!els.length) {
    say('There are no elements yet. Add them on a breakdown sheet, then flag their dates.');
    return;
  }
  const picked = await customDialog((close) => {
    const pick = el('select', { attrs: { id: 'cal-rf-element', 'data-autofocus': true } }, ...els.map((e) => el('option', { text: `${e.category}: ${e.board_id ? `${e.board_id} ${e.name}` : e.name}`, attrs: { value: String(e.id) } })));
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'cal-rf-title' } },
      el('h2', { class: 'modal-title', text: 'Add a red flag', attrs: { id: 'cal-rf-title' } }),
      el('div', { class: 'el-row' }, el('label', { class: 'bd-field', attrs: { for: 'cal-rf-element' } }, el('span', { text: 'Element' }), pick)),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Next…', attrs: { type: 'button' }, on: { click: () => close(els.find((e) => String(e.id) === pick.value) || null) } }),
      ),
    );
  });
  if (!picked) return;
  await flagDialog({
    element: picked.name,
    flag: null,
    from: date,
    types: state.view ? state.view.red_flag_types : [],
    submit: (flag) => command(invoke('set_red_flag', { element: picked.id, flag }), () => calendarShown),
    manage: () => openRedFlagTypes(),
  });
}

async function refreshCalendar() {
  try {
    // Another sub-board starts at its own start date's month.
    if (state.calSub !== state.subBoard) state.calMonth = null;
    state.calSub = state.subBoard;
    const [year, month] = state.calMonth || [null, null];
    const dto = await invoke('calendar_view', { sub_board: state.subBoard, year, month, week_start: prefs.week_start, fields: prefs.calendar_fields, span: state.calSpan });
    state.calMonth = [dto.year, dto.month];
    calendar.render(dto);
  } catch (e) {
    say(e, true);
  }
}

/**
 * New Event from an element (Elements tab) or a sheet (Breakdown tab): the event editor
 * opens over the tab, with those elements picked and `date` (or the sub-board's start)
 * as its dates, on the sub-board the tabs show. Saving adds it to that sub-board's
 * calendar as one undo step (inventory section 8).
 */
async function newEventFor(ids, date) {
  try {
    await refreshCalendar();
    await calendar.newEvent(ids, date);
  } catch (e) {
    say(e, true);
  }
}

const calendar = createCalendar($('calendar'), {
  month: (year, month) => {
    state.calMonth = [year, month];
    calendarShown = refreshCalendar();
    return calendarShown;
  },
  setDayKind: (date, kind) => command(invoke('set_day_kind', { sub_board: state.subBoard, date, kind }), () => calendarShown),
  setWeeklyOff: (days) => command(invoke('set_weekly_off', { sub_board: state.subBoard, days }), () => calendarShown),
  saveEvent: (index, event) => command(invoke('save_event', { sub_board: state.subBoard, index, event }), () => calendarShown),
  deleteEvent: (index) => command(invoke('delete_event', { sub_board: state.subBoard, index }), () => calendarShown),
  moveEvent: (index, date) => command(invoke('move_event_to', { sub_board: state.subBoard, index, date }), () => calendarShown),
  moveStrip: (board, strip, index) => command(invoke('move_strips', { board, strips: [strip], sub_board: state.subBoard, index }), () => calendarShown),
  setPrepWrap: (prep, wrap) => command(invoke('set_prep_wrap', { sub_board: state.subBoard, prep_start: prep, wrap_end: wrap }), () => calendarShown),
  setFields: (fields) => command(invoke('set_calendar_fields', { fields }), () => calendarShown),
  setHidden: (hidden) => command(invoke('set_calendar_hidden', { hidden }), () => calendarShown),
  setSpan: (on) => setCalendarSpan(on),
  addRedFlag: (date) => calendarRedFlag(date),
  setStart: (date) => command(invoke('set_start_date', { sub_board: state.subBoard, date }), () => calendarShown),
  prefs: () => prefs,
  setPrefs: (changes) => savePrefs(changes),
  settings: () => run('app.settings', 'button'),
  print: (pdf) => printCalendar(pdf),
});

const board = createBoard($('board'), document.querySelector('.panels'), {
  onPick: (id, how) => pick(id, how),
  onOpen: (row) => openRow(row),
  onStart: (sub, date) => apply(invoke('set_start_date', { sub_board: sub, date: date || null })),
  onAutoBreaks: (sub, by, limit) => apply(invoke('auto_day_breaks', { sub_board: sub, by, limit })),
  onRemoveBreaks: (sub) => removeBreaks(sub),
  dragSet: (id) => dragSet(id),
  onDrop: (rows, to) => dropRows(rows, to),
});

/** The strips cut to the app's clipboard on the board shown, for their marks. */
function cutStrips() {
  const c = state.view.clipboard;
  return c && c.kind === 'strips' && c.cut && c.board === state.view.board.id ? c.strips : [];
}

function drawBoard() {
  const prev = state.rows.findIndex((r) => r.id === state.selected);
  state.rows = board.render(state.view.board, state.selected, state.picked, cutStrips());
  const ids = new Set(state.rows.map((r) => r.id));
  state.picked = new Set([...state.picked].filter((id) => ids.has(id)));
  if (state.anchor !== null && !ids.has(state.anchor)) state.anchor = null;
  if (state.selected !== null && !ids.has(state.selected)) {
    // The focused strip is gone (a deleted day break): select what is now in its place.
    const r = state.rows[Math.min(Math.max(prev, 0), state.rows.length - 1)];
    state.selected = r ? r.id : null;
    state.picked = new Set(r ? [r.id] : []);
    state.anchor = state.selected;
    board.select(state.selected, state.picked);
  }
}

/** Selects one row by data: the board scrolls to it and draws it if it isn't drawn yet. */
function select(id) {
  state.followBoard = true;
  state.selected = id;
  state.picked = new Set(id === null ? [] : [id]);
  state.anchor = id;
  board.select(id, state.picked);
  $('board').focus({ preventScroll: true });
  onBoardSelection();
}

const rowAt = (id) => state.rows.findIndex((r) => r.id === id);

/**
 * The rows from the anchor to `id`, in the order they count as selected: the anchor
 * first, then each row toward `id`. `state.picked` keeps the order strips were selected
 * in (a Set keeps insertion order), which Merge Strips follows (decision 46).
 */
function rangeTo(id) {
  const a = rowAt(state.anchor);
  const b = rowAt(id);
  if (a < 0 || b < 0) return new Set([id]);
  const run = state.rows.slice(Math.min(a, b), Math.max(a, b) + 1).map((r) => r.id);
  return new Set(a <= b ? run : run.reverse());
}

/** A click on a row: Shift selects the run from the anchor, Command or Ctrl adds or drops one. */
function pick(id, how = {}) {
  state.followBoard = true;
  if (how.range && state.anchor !== null) {
    state.picked = rangeTo(id);
  } else if (how.toggle) {
    const next = new Set(state.picked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    state.picked = next;
    state.anchor = id;
  } else {
    select(id);
    return;
  }
  state.selected = id;
  board.select(id, state.picked);
  $('board').focus({ preventScroll: true });
  onBoardSelection();
}

/** The selected rows in screen order. */
function pickedRows() {
  return state.rows.filter((r) => state.picked.has(r.id));
}

/** The selected rows in the order they were selected: the first click, then each added one. */
function pickedInSelectionOrder() {
  const byId = new Map(state.rows.map((r) => [r.id, r]));
  return [...state.picked].map((id) => byId.get(id)).filter(Boolean);
}

/** Shift+Up and Shift+Down: grow or shrink the selection from the anchor. */
function extend(delta) {
  if (!state.rows.length) return;
  if (state.anchor === null) state.anchor = state.selected;
  const i = rowAt(state.selected);
  if (i < 0) {
    step(delta);
    return;
  }
  const next = state.rows[Math.min(Math.max(i + delta, 0), state.rows.length - 1)];
  state.picked = rangeTo(next.id);
  state.selected = next.id;
  board.select(next.id, state.picked);
}

/** What a drag from row `id` carries: the selection when `id` is in it, else `id` alone. */
function dragSet(id) {
  if (!state.picked.has(id)) select(id);
  return pickedRows();
}

function dropRows(rows, to) {
  if (to.refused) {
    say('Banners go on a sub-board, not in the Boneyard.', true);
    return;
  }
  const strips = rows.map((r) => r.id);
  apply(invoke('move_strips', { board: state.view.board.id, strips, sub_board: to.sub_board, index: to.index })).then(() =>
    board.select(state.selected, state.picked),
  );
}

/**
 * The Day out of days tab's session view (decision 59): its sort keys and the without-holds
 * switch, never saved. Another sub-board, or coming back to the tab, starts unsorted, as
 * the reference app clears a sort on both (doodSortAfter, behavior note 27).
 */
const doodView = { sub: null, sort: [], noHolds: false };

async function refreshDood() {
  try {
    doodView.sort = doodSortAfter(doodView.sort, { tab: 'dood', sub: doodView.sub }, { tab: 'dood', sub: state.subBoard });
    doodView.sub = state.subBoard;
    dood.setSort(doodView.sort);
    const dto = await invoke('dood_grid', { sub_board: state.subBoard, category: state.doodCategory, header: state.doodHeader, sort: doodView.sort, no_holds: doodView.noHolds });
    state.doodCategory = dto.category;
    state.doodHeader = dto.header;
    dood.render(dto);
  } catch (e) {
    say(e, true);
  }
}

const dood = createDood($('dood'), {
  pickCategory: (id) => {
    state.doodCategory = id;
    state.doodHeader = null;
    refreshDood();
  },
  pickHeader: (key) => {
    state.doodHeader = key;
    refreshDood();
  },
  explain: (element) => invoke('explain_count', { sub_board: state.subBoard, element }),
  say,
  sort: (keys) => {
    doodView.sort = keys;
    refreshDood();
  },
  noHolds: (on) => {
    doodView.noHolds = on;
    refreshDood();
  },
  setRule: (element, field, on) => command(invoke('set_element_field', { element, field, value: on ? 'true' : 'false' })),
  setCodes: (codes) => command(invoke('set_dood_codes', { codes })),
});

/** Edit > Explain This Count: the selected row of the Day out of days grid. */
function explainCount() {
  if (state.tab !== 'dood') {
    showTab('dood');
    say('Select a row in the grid (click an element name), then choose Explain This Count again.');
    return;
  }
  dood.explainSelected();
}

function showTab(tab) {
  if (!TABS.includes(tab)) return;
  if ((state.tab === 'breakdown' || splitShown('breakdown')) && tab !== 'breakdown') breakdown.settle();
  // The tabs share one scroller: keep the board's place while another tab is open.
  const scroller = document.querySelector('.panels');
  const back = state.tab !== 'board' && tab === 'board';
  if (state.tab === 'board' && tab !== 'board') state.boardTop = scroller.scrollTop;
  doodView.sort = doodSortAfter(doodView.sort, { tab: state.tab, sub: doodView.sub }, { tab, sub: doodView.sub });
  state.tab = tab;
  for (const name of TABS) {
    $(`tab-${name}`).setAttribute('aria-selected', name === tab ? 'true' : 'false');
    // A panel beside the board stays shown until it goes back among the tabs.
    if (!split.holds($(`panel-${name}`))) $(`panel-${name}`).hidden = name !== tab;
  }
  const moved = split.place(tab);
  if (tab === 'board') {
    if (back) scroller.scrollTop = state.boardTop;
    board.update(true);
    $('board').focus({ preventScroll: true });
    if (moved || split.shown()) refreshSplit(moved);
  } else refreshTab();
}

// ---------------------------------------------------------------------------
// Split screen (split.js; decisions.md, entry 80): a pane beside the board

const splitShown = (kind) => split.shown() === kind;

/** The sheet of the strip focused on the board, or null. */
function selectedSheet() {
  const r = selectedRow();
  return r && r.kind === 'scene' && r.sheet !== null && r.sheet !== undefined ? r.sheet : null;
}

/** The breakdown beside the board follows the board's selection (a scene's sheet). */
function followSheet() {
  const sheet = selectedSheet();
  if (sheet === null || sheet === state.sheet) return false;
  breakdown.settle();
  state.sheet = sheet;
  state.sheetIndex = null;
  return true;
}

/** After the board's selection changes: the pane beside it shows the new scene. */
function onBoardSelection() {
  if (splitShown('breakdown')) {
    if (followSheet()) sheetShown = refreshSheet();
  } else if (splitShown('images')) refreshImages();
}

/** Draws the pane beside the board; `follow` first moves the breakdown to the selection. */
function refreshSplit(follow = false) {
  const k = split.shown();
  if (k === 'breakdown') {
    if (follow) followSheet();
    sheetShown = refreshSheet();
  } else if (k === 'calendar') calendarShown = refreshCalendar();
  else if (k === 'board') board2.refresh().catch((e) => say(e, true));
  else if (k === 'images') refreshImages();
}

/** View > Images Beside Breakdown (again: Single View): the sheet shown's images beside the Breakdown tab. */
function imagesBesideBreakdown() {
  const { on, showBreakdown: show } = breakdownImagesToggle(state.tab, split.shown());
  if (show) showBreakdown();
  split.setBreakdownImages(on);
}

/** View > Breakdown, Board, Calendar or Images Beside Board (again: Single View), and Single View. */
function splitTo(kind) {
  const go = splitToggle(kind || 'single', state.tab, split.shown());
  if (go.showBoard) showTab('board');
  split.setKind(go.kind);
}

const split = createSplit({
  changed: (kind) => {
    if (kind) refreshSplit(true);
    board.update(true);
    if (!kind && state.tab === 'board') $('board').focus({ preventScroll: true });
  },
});

const board2 = createBoard2({
  view: () => state.view,
  load: (id) => invoke('board_view', { board: id }),
  move: (boardId, strips, sub, index) => apply(invoke('move_strips', { board: boardId, strips, sub_board: sub, index })),
  setStart: (sub, date) => apply(invoke('set_start_date', { sub_board: sub, date: date || null })),
  autoBreaks: (sub, by, limit) => apply(invoke('auto_day_breaks', { sub_board: sub, by, limit })),
  removeBreaks: (sub) => apply(invoke('remove_day_breaks', { sub_board: sub })),
  openSheet: (sheet) => openSheet(sheet),
  found: (boardId) => foundOn(foundHits, boardId),
});

const images = createImages($('images-pane'), {
  sheetImage: (sheet, at) => invoke('sheet_image', { sheet, at }),
  elementImage: (element, at) => invoke('element_image', { element, at }),
});

/** The images pane: the focused scene's sheet, its images and its elements'. */
async function refreshImages() {
  // Beside the breakdown, the sheet it shows; beside the board, the focused scene's.
  const sheet = state.tab === 'breakdown' ? state.sheet : selectedSheet();
  if (sheet === null || !state.view) {
    images.render(null);
    return;
  }
  try {
    const dto = await invoke('get_sheet', { board: state.view.board.id, sheet, index: null });
    images.render(dto && dto.id === sheet ? dto : null);
  } catch (e) {
    say(e, true);
  }
}

// ---------------------------------------------------------------------------
// The Elements tab (elements.js)

/** Settles once the Elements tab has drawn the latest state. */
let elementsShown = Promise.resolve();

async function refreshElements() {
  try {
    const dto = await invoke('elements_view', { sub_board: state.subBoard, category: state.category, header: state.header });
    if (dto.category !== null) state.category = dto.category;
    state.header = dto.header;
    elements.render(dto);
  } catch (e) {
    say(e, true);
  }
}

const elements = createElements($('elements'), {
  show: (category) => {
    state.category = category;
    state.header = null;
    elementsShown = refreshElements();
    return elementsShown;
  },
  showHeader: (key) => {
    state.header = key;
    elementsShown = refreshElements();
    return elementsShown;
  },
  addProperty: (category, name) => command(invoke('add_property', { category, name }), () => elementsShown),
  renameProperty: (category, at, name) => command(invoke('rename_property', { category, at, name }), () => elementsShown),
  moveProperty: (category, at, to) => command(invoke('move_property', { category, at, to }), () => elementsShown),
  deleteProperty: (category, at) => command(invoke('delete_property', { category, at }), () => elementsShown),
  setPropertyLines: (category, at, multiLine) => command(invoke('set_property_multi_line', { category, at, multi_line: multiLine }), () => elementsShown),
  setProperty: (element, name, value) => command(invoke('set_property_value', { element, name, value }), () => elementsShown),
  setLinks: (element, links, retroactive) => command(invoke('set_links', { element, links, retroactive }), () => elementsShown),
  addImage: (element) => command(invoke('add_element_image', { element }), () => elementsShown),
  removeImage: (element, at) => command(invoke('remove_element_image', { element, at }), () => elementsShown),
  image: (element, at) => invoke('element_image', { element, at }),
  days: (element) => invoke('element_days', { sub_board: state.subBoard, element }),
  newEvent: (ids) => newEventFor(ids, null),
  addCategory: (name) => command(invoke('add_category', { name }), () => elementsShown),
  setCategory: (category, field, value) => command(invoke('set_category_field', { category, field, value }), () => elementsShown),
  moveCategory: (category, to) => command(invoke('move_category', { category, to }), () => elementsShown),
  deleteCategory: (category) => command(invoke('delete_category', { category }), () => elementsShown),
  applyDefaults: (category) => command(invoke('apply_category_defaults', { category }), () => elementsShown),
  setElement: (element, field, value) => command(invoke('set_element_field', { element, field, value }), () => elementsShown),
  merge: (survivor, others) => command(invoke('merge_elements', { survivor, others }), () => elementsShown),
  moveElement: (element, category) => command(invoke('move_element', { element, category }), () => elementsShown),
  renumber: (category, start, order) => command(invoke('renumber_elements', { category, start, order }), () => elementsShown),
  deleteElement: (element, unlink) => command(invoke('delete_element', { element, unlink }), () => elementsShown),
  types: () => (state.view ? state.view.red_flag_types : []),
  manageTypes: () => openRedFlagTypes(),
  setFlag: (element, flag) => command(invoke('set_red_flag', { element, flag }), () => elementsShown),
  deleteFlag: (flag) => command(invoke('delete_red_flag', { flag }), () => elementsShown),
});

// ---------------------------------------------------------------------------
// The Breakdown tab (breakdown.js)

/** Settles once the Breakdown tab has drawn the latest sheet. */
let sheetShown = Promise.resolve();

async function refreshSheet() {
  try {
    const dto = await invoke('get_sheet', { board: state.view.board.id, sheet: state.sheet, index: state.sheetIndex, order: state.sheetOrder });
    state.sheet = dto ? dto.id : null;
    state.sheetIndex = dto ? dto.index : null;
    breakdown.render(dto);
    if (state.tab === 'breakdown' && splitShown('images')) refreshImages();
  } catch (e) {
    say(e, true);
  }
}

function openSheet(id) {
  state.sheet = id;
  state.sheetIndex = null;
  state.followBoard = false;
  showTab('breakdown');
}

/**
 * The Breakdown tab by its tab, the View menu or a Breakdown menu command. From the Board
 * tab, or from another tab when the board's selection changed since the Breakdown tab
 * last showed a sheet, it shows the focused strip's sheet when that is a scene, as a
 * double-click would; otherwise the sheet shown last (decision 51).
 */
function showBreakdown() {
  if (state.tab === 'board' || (state.tab !== 'breakdown' && state.followBoard)) {
    const r = selectedRow();
    if (r && r.kind === 'scene' && r.sheet !== null && r.sheet !== undefined) {
      state.sheet = r.sheet;
      state.sheetIndex = null;
    }
  }
  state.followBoard = false;
  showTab('breakdown');
}

function stepSheetTo(id) {
  breakdown.settle();
  state.sheet = id;
  sheetShown = refreshSheet();
}

function stepSheet(delta) {
  const d = breakdown.current();
  const id = d ? (delta < 0 ? d.prev : d.next) : null;
  if (id !== null) stepSheetTo(id);
}

/** First Sheet and Last Sheet, in the stepping order. */
function stepSheetEnd(which) {
  const d = breakdown.current();
  const id = d ? d[which] : null;
  if (id !== null && id !== undefined && id !== d.id) stepSheetTo(id);
}

/** Where the stepping order is remembered in this browser (decisions.md, entry 81). */
const ORDER_KEY = 'method-schedule.breakdown.order';

function loadOrder() {
  try {
    const v = window.localStorage.getItem(ORDER_KEY);
    return v === 'number' || v === 'scene' ? v : 'board';
  } catch {
    return 'board';
  }
}

state.sheetOrder = loadOrder();

/** Previous and Next step by board order, sheet number or scene number. */
function setSheetOrder(order) {
  state.sheetOrder = order === 'number' || order === 'scene' ? order : 'board';
  try {
    window.localStorage.setItem(ORDER_KEY, state.sheetOrder);
  } catch {
    // Not remembered; the order still applies until the page reloads.
  }
  state.sheetIndex = null;
  sheetShown = refreshSheet();
}

/**
 * Breakdown > Go to Scene and Go to Sheet Number: the sheet whose scenes (or sheet number)
 * are exactly what is typed, found by the advanced find (decisions.md, entry 79).
 */
async function goByNumber(field) {
  const what = field === 'number' ? 'sheet number' : 'scene';
  const typed = await askText({
    title: field === 'number' ? 'Go to Sheet Number' : 'Go to Scene',
    label: field === 'number' ? 'Sheet number' : 'Scene',
    confirmText: 'Go',
  });
  if (typed === null || !typed.trim()) return;
  try {
    const found = await invoke('find_text', { query: { text: '', conditions: [{ field, op: 'is', value: typed.trim() }] } });
    const hit = found.hits.find((h) => h.kind === 'sheet');
    if (!hit) {
      say(`No sheet has ${what} ${typed.trim()}.`);
      return;
    }
    if (found.count > 1) say(`${found.count} sheets have ${what} ${typed.trim()}; showing the first.`);
    if (splitShown('breakdown')) stepSheetTo(hit.sheet);
    else openSheet(hit.sheet);
  } catch (e) {
    say(e, true);
  }
}

/** Breakdown > Breakdown Designer: the sheet's fields' order, which are hidden, their widths and rows, in the schedule. */
async function fieldOrder() {
  const d = await onBreakdownTab();
  const out = await fieldOrderDialog(d && d.field_order ? d.field_order : [], (d && d.hidden_fields) || [], (d && d.field_widths) || [], (d && d.field_rows) || []);
  if (out) apply(invoke('set_sheet_field_order', { fields: out.keys, hidden: out.hidden, widths: out.widths, rows: out.rows }));
}

/** Edit > Linked Elements and Add Red Flag: for the element picked on the Elements tab. */
async function onElement(what) {
  if (state.tab !== 'elements') {
    showTab('elements');
    await elementsShown;
  }
  const done = what === 'links' ? elements.editLinks() : elements.addRedFlag();
  if (!done) say('Pick an element in the list on the Elements tab first, then choose it again.');
}

/** The sheets New Sheet and Duplicate Sheet made, each with the sheet shown before it (sheetBack). */
const sheetsMade = [];

function noteMade(before) {
  if (state.sheet === null || state.sheet === before) return;
  sheetsMade.push({ made: state.sheet, back: before });
  if (sheetsMade.length > 100) sheetsMade.shift();
}

/** Whether sheet `id` has a strip on `board` (every sheet has one on every board). */
function sheetOnBoard(board, id) {
  const has = (list) => list.some((st) => st.sheet === id);
  return has(board.boneyard) || board.sub_boards.some((sb) => has(sb.unscheduled) || sb.days.some((d) => has(d.strips)));
}

async function newSheet(after) {
  breakdown.settle();
  const before = state.sheet;
  try {
    await command(invoke('new_sheet', { board: state.view.board.id, after }));
    noteMade(before);
    await sheetShown;
    breakdown.focusFirst();
  } catch (e) {
    say(e, true);
  }
}

/** Breakdown > Duplicate Sheet: the copy goes right after the sheet and is shown next. */
async function duplicateSheet(dto) {
  if (!dto) {
    say('There is no sheet to duplicate yet.');
    return;
  }
  breakdown.settle();
  const before = state.sheet;
  try {
    await command(invoke('duplicate_sheets', { board: state.view.board.id, sheets: [dto.id] }));
    noteMade(before);
    await sheetShown;
  } catch (e) {
    say(e, true);
  }
}

/** The Breakdown menu's commands show the Breakdown tab first (or use the breakdown beside the board). */
async function onBreakdownTab() {
  if (state.tab !== 'breakdown' && !splitShown('breakdown')) showBreakdown();
  await sheetShown;
  return breakdown.current();
}

async function deleteSheet(dto) {
  if (!dto) {
    say('There is no sheet to delete.');
    return;
  }
  const which = dto.scenes ? `Scene ${dto.scenes}` : 'This sheet';
  const ok = await confirmDialog({
    title: 'Delete this sheet?',
    message: `${which} and its strip leave every board. Its elements stay in the schedule, and Undo brings the sheet back.`,
    confirmText: 'Delete Sheet',
    danger: true,
  });
  if (ok) apply(invoke('delete_sheet', { board: state.view.board.id, sheet: dto.id, order: dto.order || 'board' }));
}

/** Enter or a double-click: a scene opens its breakdown sheet, a banner its editor. */
function openRow(row) {
  if (!row) return;
  if (row.kind === 'banner') editBanner(row);
  else if (row.sheet !== null && row.sheet !== undefined) openSheet(row.sheet);
}

function openSelectedSheet() {
  openRow(selectedRow());
}

const breakdown = createBreakdown($('breakdown'), {
  field: (sheet, field, value) => command(invoke('set_sheet_field', { sheet, field, value })),
  pages: (sheet, pages, eighths) => command(invoke('set_sheet_pages', { sheet, pages, eighths })),
  tag: (sheet, element) => command(invoke('tag_element', { sheet, element })),
  create: (sheet, category, name) => command(invoke('create_element', { sheet, category, name })),
  untag: (sheet, element) => command(invoke('untag_element', { sheet, element })),
  step: (id) => stepSheetTo(id),
  newSheet: (after) => newSheet(after),
  duplicateSheet: (dto) => duplicateSheet(dto),
  deleteSheet: (dto) => deleteSheet(dto),
  goStrip: (sub, strip) => goStrip(sub, strip),
  clipboard: () => (state.view ? state.view.clipboard : null),
  quantity: (sheet, element, count, daily) => command(invoke('set_quantity', { sheet, element, count, daily })),
  newEvent: (dto) => newEventFor(dto.groups.flatMap((g) => g.elements.map((e) => e.id)), dto.date || null),
  order: (order) => setSheetOrder(order),
  image: (sheet, at) => invoke('sheet_image', { sheet, at }),
  addImage: (sheet) => command(invoke('add_sheet_image', { sheet })),
  removeImage: (sheet, at) => command(invoke('remove_sheet_image', { sheet, at })),
});

// ---------------------------------------------------------------------------
// Commands

/**
 * Shows the view a Rust command returns (null when a dialog was cancelled), or its error.
 * Every command is named in the call to invoke itself, never computed, so a Rust test can
 * compare what the page calls with what the window is granted (src/app.rs).
 */
async function apply(pending) {
  try {
    const view = await pending;
    if (view) {
      if (!view.notice) say('');
      show(view);
    }
  } catch (e) {
    say(e, true);
  }
}

function selectedRow() {
  return state.rows.find((r) => r.id === state.selected) || null;
}

function step(delta) {
  if (!state.rows.length) return;
  const i = state.rows.findIndex((r) => r.id === state.selected);
  const next = i < 0 ? (delta > 0 ? 0 : state.rows.length - 1) : Math.min(Math.max(i + delta, 0), state.rows.length - 1);
  select(state.rows[next].id);
}

/** Option-Up and Option-Down: the selection moves one place, as a block. */
function move(delta) {
  const rows = pickedRows();
  if (!rows.length) return;
  const to = blockStep(state.view.board, rows, delta);
  if (!to) return;
  const strips = rows.map((r) => r.id);
  apply(invoke('move_strips', { board: state.view.board.id, strips, sub_board: to.sub_board, index: to.index })).then(() =>
    board.select(state.selected, state.picked),
  );
}

/** The sub-board shown on the Board tab's terms: its id and length. */
function shownSub() {
  const subs = state.view.board.sub_boards;
  return subs.find((sb) => sb.id === state.subBoard) || subs[0];
}

/**
 * Insert Day Break Below (the B key, Board menu) or Above (Board menu, context menu): below
 * the bottommost selected strip or above the topmost; with nothing
 * selected, at the end or the start of the sub-board shown.
 */
function insertBreak(where = 'below') {
  onBoardTab();
  const rows = pickedRows();
  const row = rows.length ? (where === 'above' ? rows[0] : rows[rows.length - 1]) : null;
  if (row && row.list === null) {
    say('Day breaks go on a sub-board, not in the Boneyard.', true);
    return;
  }
  const sub = row ? row.list : shownSub().id;
  const index = row ? (where === 'above' ? row.index : row.index + 1) : where === 'above' ? 0 : shownSub().len;
  apply(invoke('insert_day_break', { sub_board: sub, index })).then(() => board.select(state.selected, state.picked));
}

/** Continue Scene on Next Day (decisions.md, entry 76): a part of the focused scene below the next day break. */
function continueScene() {
  onBoardTab();
  const row = selectedRow();
  if (!row || row.kind !== 'scene') {
    say('Select a scene to continue on the next day.');
    return;
  }
  if (row.list === null) {
    say('Schedule the scene on a sub-board before it spans days.', true);
    return;
  }
  apply(invoke('continue_scene', { board: state.view.board.id, strip: row.id })).then(() => board.select(state.selected, state.picked));
}

/** Remove All Day Breaks: on `sub`, else the selection's sub-board, else the picked one. One undo step. */
function removeBreaks(sub = null) {
  onBoardTab();
  if (sub === null) {
    const row = selectedRow();
    sub = row && row.list !== null ? row.list : shownSub().id;
  }
  apply(invoke('remove_day_breaks', { sub_board: sub }));
}

/**
 * Delete or Backspace: the selected strips. Day breaks go without asking (Undo brings
 * them back); banners and scenes ask first, and a scene's sheet leaves every board
 * (decisions.md, entry 54).
 */
async function deleteSelected() {
  const rows = pickedRows();
  if (!rows.length) return;
  // A part of a scene spanning days goes alone; its sheet stays (decisions.md, entry 76).
  const ask = deleteQuestion(rows);
  if (ask) {
    const ok = await confirmDialog({ ...ask, confirmText: 'Delete', danger: true });
    if (!ok) return;
  }
  apply(invoke('delete_strips', { board: state.view.board.id, strips: rows.map((r) => r.id) }));
}

// ---------------------------------------------------------------------------
// Cut, copy and paste (src/clipboard.rs; decisions.md, entry 38)
//
// The Edit menu's Cut, Copy and Paste are the system's own items: in a text field they
// do what they always do. Elsewhere the page answers the DOM's cut, copy and paste
// events for the area that has focus: the board's strips, or the elements chosen on the
// Breakdown tab. What is cut or copied goes to the app's own clipboard in Rust; plain
// text goes to the system clipboard through the event's clipboardData, which needs no
// permission. The before* events say the area has something to act on, which is what
// WebKit asks before it enables the menu items.

/** The area cut, copy and paste act on now: 'board', 'elements' or null (the browser's own). */
function clipArea() {
  if (isModalOpen() || board.dragging()) return null;
  const a = document.activeElement;
  if (isTextField(a)) return null;
  const nowhere = !a || a === document.body;
  // The breakdown beside the board: its elements, as on the Breakdown tab.
  if (splitShown('breakdown') && split.holds(a)) return 'elements';
  if (state.tab === 'board' && (nowhere || $('board').contains(a))) return 'board';
  // On the Breakdown tab: the element list, or anywhere on the tab but a field (so a
  // paste works after stepping to another sheet with Next).
  if (state.tab === 'breakdown' && (nowhere || breakdown.elementsFocused() || $('panel-breakdown').contains(a))) return 'elements';
  return null;
}

/** True when text is selected outside the board: then copying it is the browser's job. */
function textSelected() {
  const sel = window.getSelection ? window.getSelection() : null;
  return Boolean(sel && !sel.isCollapsed && String(sel).trim());
}

/** Whether `what` (copy, cut, paste) has something to act on in the focused area. */
function canClip(what) {
  const area = clipArea();
  if (!area || (what !== 'paste' && textSelected())) return false;
  const clip = state.view && state.view.clipboard;
  if (area === 'board') return what === 'paste' ? Boolean(clip && clip.kind === 'strips') : state.picked.size > 0;
  if (what === 'paste') return Boolean(clip && clip.kind === 'elements' && breakdown.current());
  return breakdown.pickedElements().length > 0;
}

/** A strip as one line of plain text: a scene's strip layout cells separated by tabs. */
function stripText(row) {
  const s = row.strip;
  if (row.kind === 'banner') return [s.text, s.duration].join('\t');
  if (row.kind === 'break') return 'End of day';
  if (Array.isArray(s.cells)) return s.cells.map((c) => c.replace(/[\t\n]+/g, ' ')).join('\t');
  return [s.scene, s.int_ext, s.set, s.time_of_day, s.pages, (s.cast || []).join(', ')].join('\t');
}

/** Set when the page answered a cut, copy or paste event (the context menu checks it). */
let clipHandled = false;

function onClipboard(e) {
  const what = e.type;
  if (!canClip(what)) return;
  e.preventDefault();
  clipHandled = true;
  const area = clipArea();
  if (what === 'paste') {
    if (area === 'board') pasteStrips();
    else pasteElements();
    return;
  }
  const cut = what === 'cut';
  if (area === 'board') {
    copyStrips(cut, e.clipboardData);
  } else {
    const d = breakdown.current();
    const picked = breakdown.pickedElements();
    if (e.clipboardData) e.clipboardData.setData('text/plain', picked.map((x) => x.name).join('\n'));
    apply(invoke('copy_elements', { sheet: d.id, elements: picked.map((x) => x.id), cut }));
  }
}

/** Cut or Copy of the selected strips to the app's clipboard, their text to `data` when given. */
function copyStrips(cut, data) {
  const rows = pickedRows();
  if (!rows.length) return;
  if (data) data.setData('text/plain', rows.map(stripText).join('\n'));
  apply(invoke('copy_strips', { board: state.view.board.id, strips: rows.map((r) => r.id), cut })).then(() => board.select(state.selected, state.picked));
}

/**
 * The context menu's Cut and Copy: the same as the Edit menu's. The click is a user
 * gesture, so the browser's own copy command runs and sends the event above, which also
 * puts the strips' text on the system clipboard; where it doesn't, the strips go to the
 * app's clipboard without the text.
 */
function clipFromMenu(what) {
  $('board').focus({ preventScroll: true });
  if (what === 'paste') {
    pasteStrips();
    return;
  }
  clipHandled = false;
  try {
    document.execCommand(what);
  } catch {
    // Not supported here: the fallback below does the work.
  }
  if (!clipHandled) copyStrips(what === 'cut', null);
}

/** Paste below the bottommost selected strip, or at the end of the picked sub-board. */
function pasteStrips() {
  const rows = pickedRows();
  let sub;
  let index;
  if (rows.length) {
    const row = rows[rows.length - 1];
    sub = row.list;
    index = row.index + 1;
  } else {
    sub = shownSub().id;
    index = shownSub().len;
  }
  apply(invoke('paste_strips', { board: state.view.board.id, sub_board: sub, index })).then(() => {
    board.select(state.selected, state.picked);
    $('board').focus({ preventScroll: true });
  });
}

function pasteElements() {
  const d = breakdown.current();
  if (!d) return;
  apply(invoke('paste_elements', { sheet: d.id }));
}

document.addEventListener('copy', onClipboard);
document.addEventListener('cut', onClipboard);
document.addEventListener('paste', onClipboard);
for (const name of ['beforecopy', 'beforecut', 'beforepaste']) {
  document.addEventListener(name, (e) => {
    if (canClip(name.slice(6))) e.preventDefault();
  });
}

// ---------------------------------------------------------------------------
// Banners, sorting, boards and sub-boards (boards.js)

/** Board menu commands work on the Board tab; from another tab they show it first. */
function onBoardTab() {
  if (state.tab !== 'board') showTab('board');
}

/**
 * A command a dialog or a tab waits for: shows the view (its notice on the status line),
 * or rejects with the sentence for the caller to show. `after`, when given, returns what
 * to wait for once the view is shown (the tab's redraw, as `() => elementsShown`).
 */
async function command(pending, after = null) {
  const view = await pending;
  if (view) {
    if (!view.notice) say('');
    show(view);
    if (after) await after();
  }
  return view;
}

/** Insert Banner Above or Below the selection's bottommost strip (on its sub-board). */
function insertBanner(where) {
  onBoardTab();
  const place = bannerPlace(pickedRows(), where, shownSub());
  if (place.boneyard) {
    say('Banners go on a sub-board, not in the Boneyard.', true);
    return;
  }
  const { sub, index } = place;
  bannerDialog({
    title: where === 'above' ? 'Insert Banner Above' : 'Insert Banner Below',
    confirmText: 'Insert Banner',
    submit: ({ text, duration, style }) => command(invoke('insert_banner', { sub_board: sub, index, text, duration, style })),
  }).then(() => board.select(state.selected, state.picked));
}

function editBanner(row) {
  onBoardTab();
  const target = row && row.kind === 'banner' ? row : pickedRows().find((r) => r.kind === 'banner');
  if (!target) {
    say('Select a banner first.');
    return;
  }
  const boardId = state.view.board.id;
  bannerDialog({
    title: 'Edit Banner',
    text: target.strip.text,
    duration: target.strip.duration,
    style: { bold: target.strip.bold, color: target.strip.banner_color || null, tall: target.strip.tall, size: target.strip.banner_size || '', align: target.strip.banner_align || '', font: target.strip.banner_font || 'usual' },
    confirmText: 'Save',
    submit: ({ text, duration, style }) => command(invoke('set_banner', { board: boardId, strip: target.id, text, duration, style })),
    remove: () => command(invoke('delete_strips', { board: boardId, strips: [target.id] })),
  }).then(() => board.select(state.selected, state.picked));
}

/** Sort Strips: the list holding the focused strip (a sub-board or the Boneyard), else the sub-board shown. */
function sortStrips() {
  onBoardTab();
  const row = selectedRow();
  const sub = row ? row.list : shownSub().id;
  const b = state.view.board;
  const name = sub === null ? 'the Boneyard' : b.sub_boards.find((x) => x.id === sub).name;
  sortDialog({
    name,
    submit: (keys) => command(invoke('sort_strips', { board: b.id, sub_board: sub, keys })),
  }).then((done) => {
    if (done) say(sub === null ? 'Sorted the Boneyard.' : `Sorted ${name}. Break the days again when you are ready.`);
  });
}

const sceneSheets = (rows) => rows.filter((r) => r.kind === 'scene' && r.sheet !== null && r.sheet !== undefined).map((r) => r.sheet);

/** The selected scenes' sheets, in screen order. */
function pickedSheets() {
  return sceneSheets(pickedRows());
}

/** Board > Duplicate Strip: each selected scene copied right after its strip, the copies selected. */
function duplicateStrips() {
  onBoardTab();
  const sheets = pickedSheets();
  if (!sheets.length) {
    say('Select a scene to duplicate. Banners and day breaks are not duplicated.');
    return;
  }
  apply(invoke('duplicate_sheets', { board: state.view.board.id, sheets })).then(() => board.select(state.selected, state.picked));
}

/**
 * Board > Merge Strips: the engine's preview of the merge, then the merge as one undo step.
 * The sheets go in the order they were selected: the first selected survives and the
 * scenes join in that order, as in the reference app (behavior note 18, decision 46).
 */
async function mergeStrips() {
  onBoardTab();
  const sheets = sceneSheets(pickedInSelectionOrder());
  if (sheets.length < 2) {
    say('Select two or more scenes to merge (Shift-click or Command-click them).');
    return;
  }
  const boardId = state.view.board.id;
  try {
    const dto = await invoke('merge_sheets_preview', { board: boardId, sheets });
    const done = await mergeDialog(dto, () => command(invoke('merge_sheets', { board: boardId, sheets })));
    if (done) board.select(state.selected, state.picked);
  } catch (e) {
    say(e, true);
  }
}

/** Board > Strip Layout: the fields the board shown's strips show, where and how (decisions.md, entries 39, 78 and 87). */
async function stripLayout() {
  onBoardTab();
  const b = state.view.board;
  const library = (state.view.layouts || []).filter((l) => l.kind === 'strips');
  const uses = b.uses ? b.uses.strips : null;
  const own = b.own_layout || b.layout || [];
  const ownSize = b.own_layout_size || 'normal';
  const ownMarks = b.own_layout_marks || {};
  // What an element count can count (decision 147): a category.
  let categories = [];
  try {
    categories = ((await invoke('elements_view', { sub_board: shownSub().id, category: null, header: null })).categories || []).map((c) => ({ id: c.id, name: c.name }));
  } catch {
    // The picker stays empty; the element count counts every element.
  }
  layoutDialog({
    board: b.name,
    own,
    ownSize,
    ownMarks,
    uses,
    library,
    categories,
    oneLine: state.view.one_line_layout ?? null,
    // Each change is its own command and undo step (decisions.md, entry 78).
    save: async ({ target, columns, size, marks, pick, oneLine }) => {
      const named = library.find((l) => l.id === target);
      const before = target === 'own' ? layoutKey(own, ownSize, ownMarks) : layoutKey(named ? named.columns : [], named ? named.size : 'normal', named ? named.marks : null);
      if (before !== layoutKey(columns, size, marks)) {
        if (target === 'own') await command(invoke('set_strip_layout', { board: b.id, columns, size, marks }));
        else await command(invoke('save_layout', { id: target, layout: { columns, size, marks } }));
      }
      if (pick !== uses) await command(invoke('use_layout', { board: b.id, kind: 'strips', id: pick }));
      if (oneLine !== (state.view.one_line_layout ?? null)) await command(invoke('use_one_line_layout', { id: oneLine }));
    },
    saveAs: (name, columns, size, marks) => command(invoke('add_layout', { name, kind: 'strips', layout: { columns, size, marks }, board: b.id })),
    remove: (id) => command(invoke('delete_layout', { id })),
  }).then((done) => {
    if (done) board.select(state.selected, state.picked);
  });
}

/** Board > Day Breaks: the board's own day look or named layouts (decisions.md, entries 67 and 78). */
function dayLooks() {
  onBoardTab();
  const b = state.view.board;
  const lib = state.view.layouts || [];
  const uses = b.uses || {};
  const own = b.own_day_look || b.day_look || {};
  const dayLayouts = lib.filter((l) => l.kind === 'day_breaks');
  const shootLayouts = lib.filter((l) => l.kind === 'shoot_day');
  const same = (x, y) => ['screen_time', 'sunrise', 'sunset', 'quantity'].every((k) => Boolean(x[k]) === Boolean(y[k])) && (x.text || '') === (y.text || '').trim();
  dayLookDialog({
    board: b.name,
    own,
    uses,
    dayLayouts,
    shootLayouts,
    // Each change is its own command and undo step.
    save: async ({ dayTarget, look, shoot, shootText }) => {
      const ownShoot = shoot === 'own' || (typeof shoot === 'number' && Boolean(own.shoot_day_strip));
      const ownNext = dayTarget === 'own' ? { ...look, shoot_day_strip: ownShoot } : { ...own, shoot_day_strip: ownShoot };
      if (!same(own, ownNext) || Boolean(own.shoot_day_strip) !== ownNext.shoot_day_strip) await command(invoke('set_day_look', { board: b.id, look: ownNext }));
      if (dayTarget !== 'own') {
        const named = dayLayouts.find((l) => l.id === dayTarget);
        if (named && !same(named.look || {}, look)) await command(invoke('save_layout', { id: dayTarget, layout: { look } }));
      }
      const dayPick = dayTarget === 'own' ? null : dayTarget;
      if (dayPick !== (uses.day_breaks ?? null)) await command(invoke('use_layout', { board: b.id, kind: 'day_breaks', id: dayPick }));
      const shootPick = typeof shoot === 'number' ? shoot : null;
      if (shootPick !== (uses.shoot_day ?? null)) await command(invoke('use_layout', { board: b.id, kind: 'shoot_day', id: shootPick }));
      if (shootPick !== null) {
        const named = shootLayouts.find((l) => l.id === shootPick);
        if (named && ((named.look && named.look.text) || '') !== shootText.trim()) {
          await command(invoke('save_layout', { id: shootPick, layout: { look: { screen_time: false, sunrise: false, sunset: false, quantity: false, text: shootText } } }));
        }
      }
    },
    saveAs: (kind, name, look) => command(invoke('add_layout', { name, kind, layout: { look }, board: b.id })),
  }).then((done) => {
    if (done) board.select(state.selected, state.picked);
  });
}

/** Board > Layouts: the named layouts (decisions.md, entry 78). */
function manageLayouts() {
  onBoardTab();
  layoutsDialog({
    layouts: () => state.view.layouts || [],
    rename: (id, name) => command(invoke('rename_layout', { id, name })),
    move: (id, to) => command(invoke('move_layout', { id, to })),
    remove: (id) => command(invoke('delete_layout', { id })),
    exportFile: async (ids) => {
      const out = await invoke('export_layouts', { ids });
      if (out && out.done && out.file) say(`Saved ${out.file}.`);
      return out;
    },
    importFile: () => apply(invoke('import_layouts')),
  }).then(() => board.select(state.selected, state.picked));
}

/** Board > Strip Colors: colors by I/E and time of day, and text colors per strip type. */
function stripColors() {
  onBoardTab();
  const b = state.view.board;
  stripColorsDialog({
    colors: b.colors || { rules: [] },
    combos: combosOf(b),
    submit: (c) => command(invoke('set_strip_colors', { rules: c.rules, scene_text: c.scene_text, banner_text: c.banner_text, break_text: c.break_text })),
  }).then((done) => {
    if (done) board.select(state.selected, state.picked);
  });
}

function showBoard(id) {
  return command(invoke('show_board', { board: id }));
}

const boardsBar = createBoardsBar($('boardbar'), {
  showBoard: (id) => showBoard(id).catch((e) => say(e, true)),
  pickSub: (id) => {
    state.subBoard = id;
    redFlagsBadge();
    // On the Board tab, the pane beside it (its calendar, say) shows the sub-board picked.
    refreshTab();
    find.refresh();
  },
  manage: () => openBoards(null),
});

function openBoards(focus) {
  manageBoards(
    {
      view: () => state.view,
      showBoard,
      addBoard: (name) => command(invoke('add_board', { name })),
      duplicateBoard: (id, name) => command(invoke('duplicate_board', { board: id, name })),
      renameBoard: (id, name) => command(invoke('rename_board', { board: id, name })),
      moveBoard: (id, to) => command(invoke('move_board', { board: id, to })),
      deleteBoard: (id) => command(invoke('delete_board', { board: id })),
      addSub: (boardId, name) => command(invoke('add_sub_board', { board: boardId, name })),
      renameSub: (id, name) => command(invoke('rename_sub_board', { sub_board: id, name })),
      moveSub: (id, to) => command(invoke('move_sub_board', { sub_board: id, to })),
      deleteSub: (id) => command(invoke('delete_sub_board', { sub_board: id })),
    },
    focus,
  );
}

/** Board > Duplicate Board: a copy of the board shown, named after it, shown next. */
function duplicateShown() {
  const b = state.view.board;
  const name = freeName(`${b.name} copy`, state.view.boards.map((x) => x.name));
  command(invoke('duplicate_board', { board: b.id, name }))
    .then((v) => v && say(`Duplicated ${b.name} as ${name}.`))
    .catch((e) => say(e, true));
}

// ---------------------------------------------------------------------------
// The strips' context menu (ctxmenu.js; decision 53)
//
// A right-click on a strip (on a sub-board or in the Boneyard) selects it first unless it
// is already selected, then opens the menu; Shift+F10 or the context menu key opens it on
// the selection. Each item runs what the menu bar and the Edit menu run.

/**
 * Move To: the selection to the top of sub-board `sub` (null: the Boneyard), and the
 * selection to the strip that followed it where it was (the one before it at the end of
 * its list), as the reference app (behavior note 24; decision 129).
 */
function moveToPlan(rows, picked, sub) {
  const last = rows.findIndex((r) => r.id === picked[picked.length - 1].id);
  const first = rows.findIndex((r) => r.id === picked[0].id);
  const from = picked[0].list;
  const stays = (r) => r.list === from && !picked.some((p) => p.id === r.id);
  const after = rows.slice(last + 1).find(stays);
  const before = rows.slice(0, Math.max(first, 0)).reverse().find(stays);
  const next = after || before || null;
  return { index: 0, sub_board: sub, next: next ? next.id : null };
}

function moveTo(sub) {
  const rows = pickedRows();
  if (!rows.length) return;
  const b = state.view.board;
  const plan = moveToPlan(state.rows, rows, sub);
  apply(invoke('move_strips', { board: b.id, strips: rows.map((r) => r.id), sub_board: sub, index: plan.index })).then(() => {
    if (plan.next !== null && state.rows.some((r) => r.id === plan.next)) select(plan.next);
    else board.select(state.selected, state.picked);
  });
}

/** The shortcut an item shows: the menu bar's for `id`, else the Edit menu's `spec`. */
function keyFor(id, spec) {
  const k = id ? keysFor(shortcutById(id)) : [spec];
  return k.length ? keyText(k[0]) : null;
}

function stripMenuItems() {
  const rows = pickedRows();
  const clip = state.view.clipboard;
  const model = stripMenu({ rows, focus: selectedRow(), clip: clip ? clip.kind : null, subBoards: state.view.board.sub_boards });
  /** What each item runs, and its keys (logic.js names the items). */
  const does = {
    open: ['board.open_sheet'],
    cut: [null, 'mod+x', () => clipFromMenu('cut')],
    copy: [null, 'mod+c', () => clipFromMenu('copy')],
    paste: [null, 'mod+v', () => clipFromMenu('paste')],
    delete: ['board.delete_break'],
    merge: ['board.merge', null, null, false],
    duplicate: ['board.duplicate', null, null, false],
    continue: ['board.continue_scene', null, null, false],
    shot: [null, null, () => shotDialog(), false],
    break_above: ['board.break_above'],
    break_below: ['board.break_below'],
    banner_above: ['board.banner_above'],
    banner_below: ['board.banner_below'],
  };
  const item = (m) => {
    if (m === 'sep') return m;
    if (m.id.startsWith('move:')) {
      const to = m.id === 'move:yard' ? null : Number(m.id.slice(5));
      return { label: m.label, run: () => moveTo(to), disabled: m.disabled };
    }
    if (m.items) return { label: m.label, items: m.items.map(item), disabled: m.disabled };
    const [id, keys, own, keyed = true] = does[m.id];
    const key = keyed ? keyFor(id, keys || undefined) : undefined;
    return { label: m.label, key, run: own || (() => run(id, 'context')), disabled: m.disabled };
  };
  return model.map(item);
}

/** The day break closing the day `stripId` is planned on, on the board shown; null off a day. */
function dayBreakOf(stripId) {
  for (const sb of state.view.board.sub_boards) {
    for (const d of sb.days) if (d.strips.some((x) => x.id === stripId)) return d.end.id;
  }
  return null;
}

/**
 * The strip menu's Shot…: what was shot of the scene on the day it is planned on (M5), its
 * page length and whether that finished it. Empty and not finished takes the day's entry away.
 */
async function shotDialog() {
  const focus = selectedRow();
  if (!focus || focus.kind !== 'scene' || !focus.strip.sheet) return;
  const day = dayBreakOf(focus.strip.id);
  if (day === null) return say('A scene is shot on a shoot day: schedule it first.', true);
  const answer = await customDialog((close) => {
    const pages = el('input', { attrs: { type: 'text', id: 'shot-pages', value: focus.strip.pages || '', 'aria-label': 'Pages shot' } });
    const done = el('input', { attrs: { type: 'checkbox', id: 'shot-done', checked: focus.strip.shot !== 'part' } });
    const go = () => close({ eighths: pages.value, complete: done.checked });
    pages.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') go();
    });
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'shot-title' } },
      el('h2', { class: 'modal-title', text: `Scene ${focus.strip.scene || ''} shot`.replace('  ', ' '), attrs: { id: 'shot-title' } }),
      el('p', { class: 'modal-message', text: `On the day it is planned. It is ${focus.strip.pages || 'no'} pages long${focus.strip.shot_text ? `; ${focus.strip.shot_text.toLowerCase()} so far` : ''}. Leave the pages empty to take this day's back.` }),
      el('label', { class: 'bd-field', attrs: { for: 'shot-pages' } }, 'Pages shot (such as 3/8 or 1 2/8)', pages),
      el('label', { class: 'bd-check', attrs: { for: 'shot-done' } }, done, 'This day finished the scene'),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Save', attrs: { type: 'button' }, on: { click: go } }),
      ),
    );
  });
  if (!answer) return;
  await command(invoke('set_shot', { sheet: focus.strip.sheet, day, eighths: answer.eighths, complete: answer.complete }));
}

function openStripMenu(x, y) {
  if (isModalOpen() || board.dragging() || !state.view) return;
  openMenu(x, y, stripMenuItems(), { label: 'Strip', back: $('board') });
}

$('board').addEventListener('contextmenu', (e) => {
  const node = e.target.closest('.row[data-id]');
  if (isMenuOpen()) {
    e.preventDefault();
    return;
  }
  if (!node) {
    // The context menu key or Shift+F10 sent by the browser itself: the selection's menu.
    if (e.target === $('board') && state.picked.size) {
      e.preventDefault();
      openKeyMenu();
    }
    return;
  }
  e.preventDefault();
  const id = Number(node.dataset.id);
  if (!state.picked.has(id)) select(id);
  else {
    // Keeps the selection; the strip clicked becomes the focused one.
    state.selected = id;
    board.select(id, state.picked);
    $('board').focus({ preventScroll: true });
  }
  openStripMenu(e.clientX, e.clientY);
});

/** Opens the menu below the focused strip, or at the board's corner. */
function openKeyMenu() {
  const node = state.selected === null ? null : document.getElementById(`strip-${state.selected}`);
  const r = (node || $('board')).getBoundingClientRect();
  openStripMenu(r.left + 24, node ? r.bottom : r.top);
}

$('board').addEventListener('keydown', (e) => {
  if ((e.key === 'F10' && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) || e.key === 'ContextMenu') {
    e.preventDefault();
    e.stopPropagation();
    openKeyMenu();
  }
});

const safety = initSafety({ apply, say, fileState });
const updates = initUpdates({ say });
outside = initOutside({ apply, say, show });

const COMMANDS = {
  'file.new': () => apply(invoke('new_schedule')),
  'file.open': () => apply(invoke('open_schedule')),
  // Save checks for a change made outside the app first; settling it takes the save's
  // place (decisions.md, entry 179).
  'file.save': async () => {
    if (await outside.check(true)) return;
    await apply(invoke('save_schedule', { save_as: false }));
  },
  'file.save_as': () => apply(invoke('save_schedule', { save_as: true })),
  'file.import_script': () => apply(invoke('import_script')),
  'file.export': () => exportReport(state.tab === 'reports' ? EXPORTABLE[reports.kind()] || null : null, say, state.subBoard, whereText()),
  'file.export_budget': () => exportBudget(say, state.subBoard),
  'file.print': () => printReport(false),
  'file.save_pdf': () => printReport(true),
  'file.what_changed': () => whatChanged('file', say, null, (h) => goToHit(h, { keepFocus: false })),
  'file.what_changed_saved': () => whatChanged('saved', say, null, (h) => goToHit(h, { keepFocus: false })),
  'file.merge': () => mergeChanges(say, show),
  'file.revert_backup': () => outside.revertToBackup(),
  'edit.undo': () => apply(invoke('undo')),
  'edit.redo': () => apply(invoke('redo')),
  'edit.explain': () => explainCount(),
  'edit.find': () => find.open({ replace: false }),
  'edit.replace': () => find.open({ replace: true }),
  'edit.find_next': () => find.step(1),
  'edit.find_previous': () => find.step(-1),
  'edit.red_flag_types': () => openRedFlagTypes(),
  'view.board': () => showTab('board'),
  'view.breakdown': () => showBreakdown(),
  'view.elements': () => showTab('elements'),
  'view.dood': () => showTab('dood'),
  'view.calendar': () => showTab('calendar'),
  'view.reports': () => showTab('reports'),
  'view.conflicts': () => showTab('conflicts'),
  'view.red_flags': () => showTab('redflags'),
  'board.previous': () => step(-1),
  'board.next': () => step(1),
  'board.extend_up': () => extend(-1),
  'board.extend_down': () => extend(1),
  'board.move_up': () => move(-1),
  'board.move_down': () => move(1),
  'board.insert_break': () => insertBreak('below'),
  'board.break_below': () => insertBreak('below'),
  'board.break_above': () => insertBreak('above'),
  'board.continue_scene': () => continueScene(),
  'board.remove_breaks': () => removeBreaks(),
  'board.delete_break': () => deleteSelected(),
  'board.open_sheet': () => openSelectedSheet(),
  'board.banner_below': () => insertBanner('below'),
  'board.banner_above': () => insertBanner('above'),
  'board.edit_banner': () => editBanner(null),
  'board.duplicate': () => duplicateStrips(),
  'board.merge': () => mergeStrips(),
  'board.sort': () => sortStrips(),
  'board.layout': () => stripLayout(),
  'board.colors': () => stripColors(),
  'board.day_look': () => dayLooks(),
  'board.layouts': () => manageLayouts(),
  'board.show': () => {
    onBoardTab();
    boardShowDialog().then((done) => {
      if (done) show(state.view);
    });
  },
  'board.print': () => printBoardNow(false),
  'board.save_pdf': () => printBoardNow(true),
  'boards.manage': () => openBoards(null),
  'boards.new_board': () => openBoards('board'),
  'boards.duplicate': () => duplicateShown(),
  'boards.new_sub': () => openBoards('sub'),
  'breakdown.new_sheet': () => onBreakdownTab().then((d) => newSheet(d ? d.id : null)),
  'breakdown.duplicate_sheet': () => onBreakdownTab().then((d) => duplicateSheet(d)),
  'breakdown.delete_sheet': () => onBreakdownTab().then((d) => deleteSheet(d)),
  'breakdown.go_scene': () => goByNumber('scenes'),
  'breakdown.go_sheet': () => goByNumber('number'),
  'breakdown.field_order': () => fieldOrder(),
  'sheet.previous': () => stepSheet(-1),
  'sheet.next': () => stepSheet(1),
  'sheet.first': () => stepSheetEnd('first'),
  'sheet.last': () => stepSheetEnd('last'),
  'file.close': () => invoke('close_window').catch((e) => say(e, true)),
  'file.new_window': () => invoke('new_window').catch((e) => say(e, true)),
  'file.open_window': () => invoke('open_in_new_window').catch((e) => say(e, true)),
  'edit.links': () => onElement('links'),
  'edit.add_red_flag': () => onElement('flag'),
  'view.split_breakdown': () => splitTo('breakdown'),
  'view.split_board': () => splitTo('board'),
  'view.split_calendar': () => splitTo('calendar'),
  'view.split_images': () => splitTo('images'),
  'view.split_breakdown_images': () => imagesBesideBreakdown(),
  'view.single': () => (state.tab === 'breakdown' ? split.close() : splitTo(null)),
  'help.shortcuts': () => shortcutSheet(),
  'help.guide': () => invoke('user_guide').then((text) => guideSheet(text)).catch((e) => say(e, true)),
  'help.whats_new': () =>
    invoke('whats_new')
      .then((w) => (w.notes ? guideSheet(w.notes) : say(`There are no release notes for version ${w.version} yet.`)))
      .catch((e) => say(e, true)),
  'help.report_bug': () => invoke('open_link', { which: 'bug' }).then(() => say('Opened a new bug report in your browser. Please use an invented schedule, never a real one.')).catch((e) => say(e, true)),
  'help.licenses': () => invoke('notices').then((n) => noticesSheet(n)).catch((e) => say(e, true)),
  'help.suite': () => invoke('open_link', { which: 'suite' }).catch((e) => say(e, true)),
  'help.open_sample': () => apply(invoke('open_sample')),
  'file.new_from_template': () => apply(invoke('new_from_template')),
  'file.save_template': () =>
    invoke('save_template')
      .then((out) => out && out.done && say(`Saved the template ${out.file}. File > New from Template starts a schedule from it.`))
      .catch((e) => say(e, true)),
  'app.settings': () => settingsDialog({ prefs: { ...prefs }, save: (changes) => savePrefs(changes) }).then((done) => done && say('Settings saved for this computer.')),
};

// The Plan menu (plan.js): its dialogs name the selection's sub-board, else the one shown.
Object.assign(
  COMMANDS,
  planCommands({
    say,
    apply,
    show,
    board: () => state.view.board.id,
    sub: () => {
      const row = selectedRow();
      return row && row.list !== null && row.list !== undefined ? row.list : shownSub().id;
    },
    selected: () => state.selected,
    scenes: () => pickedRows().filter((r) => r.kind === 'scene').map((r) => r.id),
  }),
);

/** App keys that work while typing in a field (Windows and Linux; macOS's menu runs them). */
const FIELD_KEYS = new Set(['edit.find', 'edit.replace', 'edit.find_next', 'edit.find_previous']);

// A key and its menu item can both arrive for one press. The second one within this
// window is dropped.
const REPEAT_MS = 400;
let lastRun = { id: '', via: '', at: 0 };

function run(id, via) {
  const fn = COMMANDS[id];
  if (!fn) return false;
  const now = performance.now();
  if (lastRun.id === id && lastRun.via !== via && now - lastRun.at < REPEAT_MS) {
    lastRun = { id: '', via: '', at: 0 };
    return true;
  }
  lastRun = { id, via, at: now };
  fn();
  return true;
}

/**
 * Whether a key pressed on `node` is for the board: on the board itself (a strip), or with
 * focus nowhere. Not on a button (Enter clicks it), a field, the Find panel, the pane
 * beside the board or the context menu, which are all outside it.
 */
function onBoardFocus(node) {
  if (!node || node === document.body || node === document.documentElement) return true;
  if (isTextField(node) || node.tagName === 'BUTTON') return false;
  return $('board').contains(node);
}

function onKey(e) {
  // A dialog takes its own keys (Escape, Tab, Enter on its buttons); a drag takes Escape;
  // the context menu takes its own and nothing else runs under it.
  if (isModalOpen() || board.dragging() || isMenuOpen()) return;
  const sc = matchShortcut(e);
  if (!sc) return;
  if (sc.scope === 'app') {
    // macOS: the menu item's key equivalent runs it (src/menu.rs); a browser has no such
    // menu, so the page runs it.
    if (menuRunsAppKeys && !isWeb()) return;
    // Typing keeps its own Undo and Redo; the Board menu's keys wait for the board. Find's
    // keys work from a field too.
    if (isTextField(e.target) && !FIELD_KEYS.has(sc.id) && (sc.id.startsWith('edit.') || sc.id.startsWith('board.'))) return;
  } else if (sc.scope === 'sheet') {
    // Sheet keys on the Breakdown tab, in its fields too, and in the breakdown beside the board.
    if (state.tab !== 'breakdown' && !(splitShown('breakdown') && split.holds(e.target))) return;
  } else {
    // Board keys only on the Board tab, and only with focus on the board (onBoardFocus).
    if (state.tab !== 'board' || !onBoardFocus(e.target)) return;
  }
  if (run(sc.id, 'key')) e.preventDefault();
}

const RECENT = 'file.recent.';

function onMenu(id) {
  if (typeof id !== 'string' || isModalOpen()) return;
  if (id.startsWith(RECENT)) {
    // File > Open Recent: the file's place in the list (src/safety_app.rs).
    const index = Number(id.slice(RECENT.length));
    if (Number.isInteger(index) && index >= 0) safety.openRecent(index);
    return;
  }
  const sc = shortcutById(id);
  if (sc && sc.scope === 'board' && state.tab !== 'board') return;
  if (sc && sc.scope === 'sheet' && state.tab !== 'breakdown') return;
  run(id, 'menu');
}

// ---------------------------------------------------------------------------
// Start

for (const [btn, id, label] of [
  ['btn-new', 'file.new', 'New schedule'],
  ['btn-open', 'file.open', 'Open a schedule'],
  ['btn-save', 'file.save', 'Save'],
  ['btn-save-as', 'file.save_as', 'Save as a new file'],
  ['btn-import', 'file.import_script', 'Import a script (Fountain, .fdx or .mscenes) into the Boneyard'],
  ['btn-undo', 'edit.undo', 'Undo'],
  ['btn-redo', 'edit.redo', 'Redo'],
]) {
  const b = $(btn);
  b.title = withShortcut(label, id);
  b.addEventListener('click', () => run(id, 'button'));
}
$('tab-board').addEventListener('click', () => showTab('board'));
$('tab-breakdown').addEventListener('click', () => showBreakdown());
$('tab-elements').addEventListener('click', () => showTab('elements'));
$('tab-dood').addEventListener('click', () => showTab('dood'));
$('tab-calendar').addEventListener('click', () => showTab('calendar'));
$('tab-reports').addEventListener('click', () => showTab('reports'));
$('tab-conflicts').addEventListener('click', () => showTab('conflicts'));
$('tab-redflags').addEventListener('click', () => showTab('redflags'));
// No tooltip on the board or the sheet as a whole: it covered the strips wherever the
// pointer rested, and its children took it. Their keys are in the Help shortcut sheet
// (decisions.md, entry 150).
document.addEventListener('keydown', onKey);
listen('menu', onMenu);
listen('prefs-changed', prefsChanged);
// Strips cut or copied in another window can be pasted here; elements copied there empty
// this window's clipboard (src/docs.rs, decisions.md entry 183).
listen('clipboard-changed', (clip) => {
  if (!state.view) return;
  state.view.clipboard = clip && typeof clip === 'object' ? clip : null;
  if (state.tab === 'board') drawBoard();
});

// The pane beside the board the viewer chose last time (split.js remembers it).
split.place(state.tab);

/**
 * In a browser: the menus as buttons at the start of the top bar, each opening the
 * page's own menu (ctxmenu.js) with the host's table, and no Autosave switch.
 */
async function webMode() {
  $('autosave').closest('label').hidden = true;
  const menus = await invoke('menu_table');
  const bar = el('div', { class: 'group menubar', attrs: { role: 'menubar', 'aria-label': 'Menus' } });
  for (const m of menus) {
    const b = el('button', { class: 'btn', text: m.title, attrs: { type: 'button', role: 'menuitem', 'aria-haspopup': 'menu' } });
    b.addEventListener('click', () => {
      const r = b.getBoundingClientRect();
      const items = m.items.map((i) => {
        if (!i) return 'sep';
        const k = keysFor(shortcutById(i.id));
        return { label: i.label, key: k.length ? keyText(k[0]) : '', run: () => onMenu(i.id) };
      });
      openMenu(r.left, r.bottom, items, { label: m.title, back: b });
    });
    bar.append(b);
  }
  const top = document.querySelector('.topbar');
  top.prepend(el('span', { class: 'sep', attrs: { 'aria-hidden': 'true' } }));
  top.prepend(bar);
}

if (connected) {
  // The host, the first view, then a file the system asked to open and schedules to
  // recover (the desktop's).
  loadHost(invoke)
    .then(() => (isWeb() ? webMode().catch((e) => say(e, true)) : null))
    .then(() => loadPrefs())
    .then(() => apply(invoke('get_view')))
    .then(() => {
      if (isWeb()) return;
      safety.start();
      updates.start();
    });
} else {
  say('This page is not connected to the app. Run it with cargo run -p method-schedule-app.', true);
}
