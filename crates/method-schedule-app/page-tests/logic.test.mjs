// The page's own rules (ui/js/logic.js), run by Node's built-in test runner with no npm:
//   node --test crates/method-schedule-app/page-tests/
// logic.js has no imports, so it loads as a module from its text whatever Node makes of
// a `.js` file outside a package (decisions.md, entry 137).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source = readFileSync(new URL('../ui/js/logic.js', import.meta.url), 'utf8');
const L = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

test('logic.js stands alone: no imports, no page globals', () => {
  assert.doesNotMatch(source, /^\s*import\b/m);
  for (const g of ['document', 'window', 'localStorage', 'navigator']) {
    assert.doesNotMatch(source, new RegExp(`\\b${g}\\.`), g);
  }
});

test('typed dates: dashed or slashed, real days only', () => {
  assert.equal(L.typedDate('2027-03-01'), '2027-03-01');
  assert.equal(L.typedDate(' 3/1/2027 '), '2027-03-01');
  assert.equal(L.typedDate('02/29/2028'), '2028-02-29');
  for (const bad of ['2027-02-29', '13/1/2027', '3/32/2027', '2027-3-1', 'March 1', '', null]) assert.equal(L.typedDate(bad), null, String(bad));
});

test('RFL-05: a click replaces the pick, Command adds or removes, Shift adds a run', () => {
  let s = { picked: new Set(), anchor: null };
  const click = (date, mods) => (s = L.pickDate(s.picked, s.anchor, date, mods));
  const sorted = () => [...s.picked].sort();
  click('2027-03-04');
  assert.deepEqual([...s.picked], ['2027-03-04']);
  click('2027-03-09');
  assert.deepEqual([...s.picked], ['2027-03-09'], 'a plain click replaces');
  click('2027-03-11', { add: true });
  assert.deepEqual(sorted(), ['2027-03-09', '2027-03-11'], 'Command adds');
  click('2027-03-09', { add: true });
  assert.deepEqual([...s.picked], ['2027-03-11'], 'Command on a picked date removes it');
  assert.equal(s.anchor, '2027-03-11', 'a removing Command-click leaves the anchor');
  // Shift with nothing clicked yet picks that date; across a month end.
  s = L.pickDate(new Set(), null, '2027-02-27', { range: true });
  assert.deepEqual([...s.picked], ['2027-02-27']);
  s = L.pickDate(s.picked, s.anchor, '2027-03-02', { range: true });
  assert.equal(s.picked.size, 4);
  // The picked set given in is never changed.
  const before = new Set(['2027-01-01']);
  L.pickDate(before, '2027-01-01', '2027-01-05', { add: true });
  L.pickDate(before, '2027-01-01', '2027-01-05', { range: true });
  assert.deepEqual([...before], ['2027-01-01']);
});

test('E-E17: the reference app\'s own runs (behavior note 27)', () => {
  let s = { picked: new Set(), anchor: null };
  const click = (day, mods) => (s = L.pickDate(s.picked, s.anchor, `2027-03-${String(day).padStart(2, '0')}`, mods));
  const days = () => [...s.picked].sort().map((d) => Number(d.slice(8)));
  // Click 5, Shift-click 9: 5 to 9.
  click(5);
  click(9, { range: true });
  assert.deepEqual(days(), [5, 6, 7, 8, 9]);
  // Command-click 7 removes it and leaves the anchor at 9, the Shift-click's end.
  click(7, { add: true });
  assert.deepEqual(days(), [5, 6, 8, 9]);
  // Shift-click 15 adds 10 to 15 and keeps the rest; 7 stays off.
  click(15, { range: true });
  assert.deepEqual(days(), [5, 6, 8, 9, 10, 11, 12, 13, 14, 15]);
  // Click 22, Command-click 18, Shift-click 20: 18, 19, 20 and 22.
  click(22);
  assert.deepEqual(days(), [22], 'a plain click picks that date alone');
  click(18, { add: true });
  click(20, { range: true });
  assert.deepEqual(days(), [18, 19, 20, 22]);
  // Shift backwards from the last end, and Command-Shift works the same.
  click(16, { add: true, range: true });
  assert.deepEqual(days(), [16, 17, 18, 19, 20, 22]);
  // Shift-click on a lone date adds it without Command.
  click(1);
  click(3, { range: true });
  click(28, { add: true });
  click(30, { range: true });
  assert.deepEqual(days(), [1, 2, 3, 28, 29, 30]);
});

test('RFL-05: Command on the Mac, Ctrl elsewhere', () => {
  assert.deepEqual(L.pickMods({ metaKey: true, ctrlKey: false, shiftKey: false }, true), { add: true, range: false });
  assert.deepEqual(L.pickMods({ metaKey: false, ctrlKey: true, shiftKey: false }, true), { add: false, range: false });
  assert.deepEqual(L.pickMods({ metaKey: false, ctrlKey: true, shiftKey: true }, false), { add: true, range: true });
  assert.deepEqual(L.pickMods({ metaKey: true, ctrlKey: false, shiftKey: false }, false), { add: false, range: false });
});

test('BRK-18: Quick add\'s Category;Name', () => {
  assert.deepEqual(L.semicolonCreate('Props;Lantern'), { category: 'Props', name: 'Lantern' });
  assert.deepEqual(L.semicolonCreate(' Set Dressing ; Old lamp; brass '), { category: 'Set Dressing', name: 'Old lamp; brass' });
  for (const t of ['Lantern', ';Lantern', 'Props;', ' ; ']) assert.equal(L.semicolonCreate(t), null, t);
});

test('BRK-11: Hide empty hides empty boxes, never the one being typed in', () => {
  assert.equal(L.boxHidden(true, 0, false), true);
  assert.equal(L.boxHidden(true, 0, true), false);
  assert.equal(L.boxHidden(true, 2, false), false);
  assert.equal(L.boxHidden(false, 0, false), false);
});

test('ELM-16: column sort up, down, then the engine\'s order; blanks last; ties kept', () => {
  let s = { key: null, down: false };
  s = L.nextSort(s.key, s.down, 'name');
  assert.deepEqual(s, { key: 'name', down: false });
  s = L.nextSort(s.key, s.down, 'name');
  assert.deepEqual(s, { key: 'name', down: true });
  s = L.nextSort(s.key, s.down, 'name');
  assert.deepEqual(s, { key: null, down: false });
  s = L.nextSort('name', true, 'days');
  assert.deepEqual(s, { key: 'days', down: false });
  const rows = [
    { name: 'ada', board_id: '10', days: 3, pages: '1 3/8' },
    { name: 'Bo', board_id: '2', days: null, pages: '7/8' },
    { name: 'cy', board_id: '', days: 3, pages: '2' },
    { name: 'Al', board_id: '1', days: 1, pages: '' },
  ];
  const names = (key, down) => L.sortRows(rows, key, down).map((r) => r.name);
  assert.deepEqual(names(null, false), ['ada', 'Bo', 'cy', 'Al']);
  assert.deepEqual(names('name', false), ['ada', 'Al', 'Bo', 'cy'], 'any case');
  assert.deepEqual(names('board_id', false), ['Al', 'Bo', 'ada', 'cy'], '2 before 10, blank last');
  assert.deepEqual(names('board_id', true), ['ada', 'Bo', 'Al', 'cy'], 'blank last going down too');
  assert.deepEqual(names('days', false), ['Al', 'ada', 'cy', 'Bo'], 'ties in the given order');
  assert.deepEqual(names('pages', true), ['cy', 'ada', 'Bo', 'Al']);
  assert.equal(L.eighthsOf('1 3/8'), 11);
  assert.equal(L.eighthsOf('5/8'), 5);
  assert.equal(L.eighthsOf('x'), 0);
});

test('CAL-05: Go To takes a typed date or says why not', () => {
  assert.deepEqual(L.goToDate('3/9/2027'), { date: '2027-03-09' });
  assert.deepEqual(L.goToDate('   '), {});
  assert.match(L.goToDate('soon').error, /"soon" is not a date/);
});

test('CAL-07, E-C11: the filter checks several categories and elements', () => {
  const elements = [
    { id: 1, category: 'Cast' },
    { id: 2, category: 'Cast' },
    { id: 3, category: 'Props' },
    { id: 4, category: 'Vehicles' },
  ];
  const f = (categories, ids) => ({ categories, elements: ids });
  const shown = (strip, filter) => L.stripShown({ elements: strip }, filter, elements);
  // Nothing checked (All) shows every strip, even one with no elements.
  assert.equal(shown([2], f([], [])), true);
  assert.equal(shown([], f([], [])), true);
  // Categories alone: any element of a checked category.
  assert.equal(shown([2], f(['Cast'], [])), true);
  assert.equal(shown([2], f(['Props'], [])), false);
  assert.equal(shown([3], f(['Cast', 'Props'], [])), true);
  assert.equal(shown([], f(['Cast'], [])), false);
  // Elements checked: any of them, whatever else the strip holds.
  assert.equal(shown([2], f([], [1])), false);
  assert.equal(shown([2], f([], [2])), true);
  assert.equal(shown([1, 3], f([], [2, 3])), true);
  assert.equal(shown([4], f(['Cast'], [1, 2])), false);
  assert.equal(shown([2, 4], f(['Cast'], [2])), true);
});

test('CAL-07, E-C11: the filter\'s lists hold the categories in use', () => {
  const elements = [
    { id: 1, category: 'Cast' },
    { id: 2, category: 'Cast' },
    { id: 3, category: 'Props' },
  ];
  const all = L.calendarFilter(elements, { categories: [], elements: [] });
  assert.deepEqual(all.categories, ['Cast', 'Props']);
  assert.deepEqual(all.groups.map((g) => [g.category, g.elements.map((e) => e.id)]), [['Cast', [1, 2]], ['Props', [3]]], 'All lists every element under its category');
  const props = L.calendarFilter(elements, { categories: ['Props', 'Gone'], elements: [1, 3, 9] });
  assert.deepEqual(props.groups.map((g) => g.category), ['Props'], 'only the checked categories\' elements');
  assert.deepEqual(props.filter, { categories: ['Props'], elements: [3] }, 'checks no longer listed are dropped');
  const two = L.calendarFilter(elements, { categories: ['Props', 'Cast'], elements: [] });
  assert.deepEqual(two.groups.map((g) => g.category), ['Cast', 'Props'], 'in category order');
});

test('CAL-06: a calendar drag moves a strip to a shoot day, an event to any date', () => {
  const day = { drop: '7', date: '2027-03-04' };
  const off = { drop: null, date: '2027-03-06' };
  assert.deepEqual(L.calendarDrop({ kind: 'strip', id: 12 }, day), { strip: 12, at: 7 });
  assert.equal(L.calendarDrop({ kind: 'strip', id: 12 }, off), null, 'a strip needs a shoot day');
  assert.deepEqual(L.calendarDrop({ kind: 'event', id: 2 }, off), { event: 2, date: '2027-03-06' });
  assert.deepEqual(L.calendarDrop({ kind: 'event', id: 0 }, day), { event: 0, date: '2027-03-04' });
  assert.equal(L.calendarDrop({ kind: 'event', id: 2 }, null), null, 'off the month');
  assert.equal(L.calendarDrop(null, day), null);
  assert.equal(L.dragStarted(10, 10, 12, 11), false);
  assert.equal(L.dragStarted(10, 10, 13, 12), true);
});

test('E-B12: a new banner goes by the bottommost selected strip', () => {
  const shown = { id: 10, len: 6 };
  const rows = [
    { list: 10, index: 1 },
    { list: 10, index: 4 },
  ];
  assert.deepEqual(L.bannerPlace(rows, 'above', shown), { sub: 10, index: 4 }, 'Above: above the bottommost');
  assert.deepEqual(L.bannerPlace(rows, 'below', shown), { sub: 10, index: 5 }, 'Below: below the bottommost');
  assert.deepEqual(L.bannerPlace([{ list: 11, index: 0 }], 'above', shown), { sub: 11, index: 0 }, 'on the strip\'s own sub-board');
  assert.deepEqual(L.bannerPlace([], 'above', shown), { sub: 10, index: 0 });
  assert.deepEqual(L.bannerPlace([], 'below', shown), { sub: 10, index: 6 });
  assert.deepEqual(L.bannerPlace([{ list: 10, index: 2 }, { list: null, index: 0 }], 'above', shown), { boneyard: true });
});

test('E-D25: the DOOD sort clears on leaving the tab or another sub-board', () => {
  const keys = ['hold:desc'];
  assert.deepEqual(L.doodSortAfter(keys, { tab: 'dood', sub: 1 }, { tab: 'board', sub: 1 }), [], 'leaving the tab');
  assert.deepEqual(L.doodSortAfter(keys, { tab: 'dood', sub: 1 }, { tab: 'dood', sub: 2 }), [], 'another sub-board');
  assert.deepEqual(L.doodSortAfter(keys, { tab: 'dood', sub: 1 }, { tab: 'dood', sub: 1 }), keys, 'the same tab again');
  assert.deepEqual(L.doodSortAfter(keys, { tab: 'board', sub: 1 }, { tab: 'calendar', sub: 1 }), keys, 'other tabs');
});

test('KEY-03: the strip menu follows the selection', () => {
  const subBoards = [
    { id: 10, name: 'Main' },
    { id: 11, name: 'Second Unit' },
  ];
  const scene = { kind: 'scene', list: 10, id: 1 };
  const scene2 = { kind: 'scene', list: 10, id: 2 };
  const banner = { kind: 'banner', list: 10, id: 3 };
  const brk = { kind: 'break', list: 10, id: 4 };
  const yard = { kind: 'scene', list: null, id: 5 };
  const menu = (rows, focus, clip = null) => {
    const out = {};
    const walk = (items) => {
      for (const m of items) {
        if (m === 'sep') continue;
        out[m.id] = m;
        if (m.items) walk(m.items);
      }
    };
    walk(L.stripMenu({ rows, focus, clip, subBoards }));
    return out;
  };
  let m = menu([scene], scene);
  assert.equal(m.open.label, 'Edit Sheet');
  assert.equal(m.paste.disabled, true, 'Paste off with nothing cut or copied');
  assert.equal(m.merge.disabled, true, 'merging needs two scenes');
  assert.equal(m.continue.disabled, false);
  assert.equal(m['move:10'], undefined, 'not to the list it is on');
  assert.equal(m['move:11'].disabled, false);
  assert.equal(m['move:yard'].disabled, false);
  assert.equal(m.break.disabled, false);
  m = menu([scene, scene2], scene, 'strips');
  assert.equal(m.merge.disabled, false);
  assert.equal(m.paste.disabled, false);
  m = menu([banner], banner);
  assert.equal(m.open.label, 'Edit Banner…');
  assert.equal(m['move:yard'].disabled, true, 'banners never go to the Boneyard');
  assert.equal(m.duplicate.disabled, true);
  m = menu([brk], brk);
  assert.equal(m.open.disabled, true);
  m = menu([yard], yard);
  assert.equal(m.continue.disabled, true, 'not from the Boneyard');
  assert.equal(m.shot.disabled, true, 'shot on a shoot day, never in the Boneyard (M5)');
  assert.equal(m.break.disabled, true);
  assert.equal(m.banner.disabled, true);
  assert.equal(m['move:yard'].disabled, true);
  m = menu([], null);
  for (const k of ['cut', 'copy', 'delete', 'move', 'open']) assert.equal(m[k].disabled, true, k);
  const order = L.stripMenu({ rows: [scene], focus: scene, clip: null, subBoards }).map((x) => (x === 'sep' ? '-' : x.id));
  assert.deepEqual(order, ['open', '-', 'cut', 'copy', 'paste', 'delete', '-', 'merge', 'duplicate', 'continue', 'shot', 'move', '-', 'break', 'banner']);
});

test('KEY-04: Delete asks for scenes, banners and parts, never for day breaks alone', () => {
  const brk = { kind: 'break', strip: {} };
  const scene = { kind: 'scene', strip: { scene: '12', parts: 1 } };
  const part = { kind: 'scene', strip: { scene: '7', parts: 2, part: 2 } };
  const banner = { kind: 'banner', strip: {} };
  assert.equal(L.deleteQuestion([brk]), null);
  assert.equal(L.deleteQuestion([brk, brk]), null);
  let q = L.deleteQuestion([scene]);
  assert.equal(q.title, 'Delete this scene?');
  assert.match(q.message, /^Delete scene 12\? Its sheet leaves every board/);
  assert.match(q.message, /Undo brings it back\.$/);
  q = L.deleteQuestion([part]);
  assert.equal(q.title, 'Delete this part?');
  assert.match(q.message, /^Delete part 2 of scene 7\? Undo/);
  q = L.deleteQuestion([banner]);
  assert.equal(q.title, 'Delete this banner?');
  q = L.deleteQuestion([scene, scene, banner, brk]);
  assert.equal(q.title, 'Delete these strips?');
  assert.match(q.message, /^Delete 2 scenes, 1 banner and 1 day break\? Their sheets leave/);
});

test('KEY-05: the shortcut sheet\'s filter matches names, keys and groups', () => {
  const groups = [
    { title: 'File', rows: [{ label: 'Save', keys: '⌘S' }, { label: 'Open…', keys: '⌘O' }] },
    { title: 'On the board', rows: [{ label: 'Delete', keys: '⌫' }] },
  ];
  assert.equal(L.filterShortcuts(groups, '').length, 2);
  assert.deepEqual(L.filterShortcuts(groups, 'SAVE').map((g) => g.rows.map((r) => r.label)), [['Save']]);
  assert.deepEqual(L.filterShortcuts(groups, '⌘o')[0].rows.map((r) => r.label), ['Open…']);
  assert.equal(L.filterShortcuts(groups, 'board')[0].title, 'On the board');
  assert.deepEqual(L.filterShortcuts(groups, 'nothing like it'), []);
});

test('Find and Replace: the advanced rows\' words fit the panel', () => {
  assert.deepEqual(L.FIND_JOINS.map(([k]) => k), ['and', 'or']);
  for (const [, label] of [...L.FIND_JOINS, ...L.FIND_OPS]) assert.ok(label.length <= 16, label);
  for (const [, label] of L.FIND_JOINS) assert.doesNotMatch(label, /[()]/, 'no (AND) to be cut off');
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  assert.match(css, /\.fp-cond \{ display: grid;/, 'a condition takes two lines');
  assert.doesNotMatch(css, /\.fp-cond select \{[^}]*max-width/, 'no select squeezed to a fixed width');
});

test('Find and Replace: the replace box\'s hint fits beside its button, the meaning in a tooltip', () => {
  // The box takes what the 22em panel leaves beside Replace: about twenty characters.
  assert.ok(L.FIND_REPLACE_HINT.placeholder.length <= 16, L.FIND_REPLACE_HINT.placeholder);
  assert.match(L.FIND_REPLACE_HINT.title, /empty/);
  const find = readFileSync(new URL('../ui/js/find.js', import.meta.url), 'utf8');
  assert.match(find, /placeholder: FIND_REPLACE_HINT\.placeholder, title: FIND_REPLACE_HINT\.title/);
});

test('KEY-05: Escape closes a dialog unless something in it took the key', () => {
  assert.equal(L.dialogEscape({ key: 'Escape', defaultPrevented: false }), true);
  assert.equal(L.dialogEscape({ key: 'Escape', defaultPrevented: true }), false, 'a list or menu closed first');
  assert.equal(L.dialogEscape({ key: 'Escape', defaultPrevented: false, isComposing: true }), false);
  assert.equal(L.dialogEscape({ key: 'Enter', defaultPrevented: false }), false);
  assert.equal(L.dialogEscape(null), false);
});

test('calendar check box lists: Escape closes an open one first, from anywhere, and only it', () => {
  assert.equal(L.dropEscape({ key: 'Escape', defaultPrevented: false }, true), true);
  assert.equal(L.dropEscape({ key: 'Escape', defaultPrevented: false }, false), false, 'closed: the key goes on');
  assert.equal(L.dropEscape({ key: 'Enter', defaultPrevented: false }, true), false);
  const cal = readFileSync(new URL('../ui/js/calendar.js', import.meta.url), 'utf8');
  // The document listens while a list is open, in the capture phase, and the key stops there.
  assert.match(cal, /document\.addEventListener\('keydown', onKey, true\)/);
  assert.match(cal, /document\.removeEventListener\('keydown', onKey, true\)/);
  assert.match(cal, /dropEscape\(e, !list\.hidden\)\) return;\s*e\.preventDefault\(\);\s*e\.stopPropagation\(\);/);
});

test('KEY-05: text fields offer no saved values; search boxes no spelling either', () => {
  assert.deepEqual(L.quietField('input', { type: 'search' }), { autocomplete: 'off', spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' });
  assert.deepEqual(L.quietField('input', {}), { autocomplete: 'off' }, 'no type is text');
  assert.deepEqual(L.quietField('textarea', { spellcheck: 'true' }), { autocomplete: 'off' });
  assert.deepEqual(L.quietField('input', { type: 'search', autocomplete: 'on', spellcheck: 'true' }), { autocorrect: 'off', autocapitalize: 'off' }, 'what a field sets stays');
  assert.deepEqual(L.quietField('input', { type: 'checkbox' }), {});
  assert.deepEqual(L.quietField('select', {}), {});
});

test('SPL: View items open a pane, the same one again closes it', () => {
  assert.deepEqual(L.splitToggle('calendar', 'board', null), { kind: 'calendar', showBoard: false });
  assert.deepEqual(L.splitToggle('calendar', 'board', 'calendar'), { kind: null, showBoard: false });
  assert.deepEqual(L.splitToggle('images', 'board', 'calendar'), { kind: 'images', showBoard: false });
  assert.deepEqual(L.splitToggle('board', 'dood', 'board'), { kind: 'board', showBoard: true }, 'from another tab it opens on the Board tab');
  assert.deepEqual(L.splitToggle('single', 'board', 'board'), { kind: null, showBoard: false });
  assert.deepEqual(L.breakdownImagesToggle('breakdown', 'images'), { on: false, showBreakdown: false });
  assert.deepEqual(L.breakdownImagesToggle('board', 'images'), { on: true, showBreakdown: true });
  assert.equal(L.paneFor('board', 'calendar', true), 'calendar');
  assert.equal(L.paneFor('breakdown', 'calendar', true), 'images');
  assert.equal(L.paneFor('breakdown', 'calendar', false), null);
  assert.equal(L.paneFor('dood', 'calendar', true), null);
  assert.equal(L.paneFor('board', 'nonsense', false), null);
});

test('SPL: the pane\'s width leaves the board its room, and stored state is checked', () => {
  assert.equal(L.paneWidth(520, 1200), 520);
  assert.equal(L.paneWidth(1000, 1200), 880, 'the board keeps 320 px');
  assert.equal(L.paneWidth(100, 1200), 280);
  assert.equal(L.paneWidth(520, 400), 280, 'never under the least');
  assert.equal(L.paneWidth(520, 861 - 274), 280, 'the Find panel taking room');
  assert.deepEqual(L.readSplit('{"kind":"board","width":610.6,"bdImages":true}'), { kind: 'board', width: 611, bdImages: true });
  assert.deepEqual(L.readSplit('{"kind":"x","width":"wide"}'), { kind: null, width: 520, bdImages: false });
  assert.deepEqual(L.readSplit('not json'), { kind: null, width: 520, bdImages: false });
  assert.deepEqual(L.readSplit(null), { kind: null, width: 520, bdImages: false });
});

test('DOD-10, DOD-19: weekends and the vertical grid', () => {
  assert.equal(L.isWeekend({ weekday: 'Sat' }), true);
  assert.equal(L.isWeekend({ weekday: 'Mon' }), false);
  assert.equal(L.weekendsOn(null), true);
  assert.equal(L.weekendsOn('1'), true);
  assert.equal(L.weekendsOn('0'), false);
  const dto = {
    dates: [
      { date: '2027-03-05', weekday: 'Fri', kind: 'work', shoot_day: 5 },
      { date: '2027-03-06', weekday: 'Sat', kind: 'off', shoot_day: null },
    ],
    rows: [
      { board_id: '1', name: 'TOVE', cells: ['W', ''], work: 1, hold: 0, total: 1 },
      { board_id: null, name: 'Dog', cells: ['SW', 'H'], work: 1, hold: 1, total: 2 },
    ],
  };
  const v = L.verticalGrid(dto);
  assert.deepEqual(v.heads, ['1', 'Dog']);
  assert.deepEqual(v.rows.map((r) => [r.day, r.weekend, r.cells]), [
    ['D5', false, ['W', 'SW']],
    ['', true, ['', 'H']],
  ]);
  assert.deepEqual(v.sums, [
    { label: 'Work', cells: ['1', '1'] },
    { label: 'Hold', cells: ['0', '1'] },
    { label: 'Total', cells: ['1', '2'] },
  ]);
});

test('DOD-19: the DOOD report pages by whole weeks and follows the Weekends box', () => {
  // Sunday 2027-02-28 to Saturday 2027-03-20: three weeks.
  const names = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const dates = Array.from({ length: 21 }, (_, i) => ({ weekday: names[i % 7] }));
  // Shown: two whole weeks (14 columns) fit in 18, then the third.
  assert.deepEqual(L.doodReportPages(dates, 18, true, 'sun').map((p) => [p[0], p.length]), [[0, 14], [14, 7]]);
  // Hidden: Friday then Monday, and three five-day weeks fit in 18.
  const hidden = L.doodReportPages(dates, 18, false, 'sun');
  assert.equal(hidden.length, 1);
  assert.deepEqual(hidden[0].slice(0, 6), [1, 2, 3, 4, 5, 8]);
  assert.equal(hidden[0].length, 15);
  // Weeks from Monday: the Sunday ends the first week.
  assert.deepEqual(L.doodReportPages(dates.slice(1), 7, true, 'mon').map((p) => p.length), [7, 7, 6]);
  // A page too narrow for a week still takes one.
  assert.equal(L.doodReportPages(dates, 5, true, 'sun').length, 3);
  assert.deepEqual(L.doodReportPages([], 18, true, 'sun'), []);
});

test('BRD-18: the element count picks one category, found by its name only', () => {
  const cats = [
    { id: 1, name: 'Cast Members' },
    { id: 4, name: 'Props' },
    { id: 9, name: 'Set Dressing' },
  ];
  assert.deepEqual(L.countCategories(cats, '').map((c) => c.id), [1, 4, 9]);
  assert.deepEqual(L.countCategories(cats, ' S').map((c) => c.id), [1, 4, 9]);
  assert.deepEqual(L.countCategories(cats, 'PROP').map((c) => c.id), [4]);
  assert.deepEqual(L.countCategories(cats, 'dress').map((c) => c.id), [9]);
  assert.deepEqual(L.countCategories(cats, 'TOVE'), [], 'an element name finds nothing');
  assert.equal(L.countSay(cats, 4), 'Counts Props');
  assert.equal(L.countSay(cats, null), 'Counts every element');
  assert.equal(L.countSay(cats, 77), 'Counts every element', 'a category since deleted');
});

test('Entry 194: a category elements field says what it lists', () => {
  const cats = [
    { id: 1, name: 'Cast Members' },
    { id: 4, name: 'Props' },
  ];
  assert.equal(L.listSay(cats, 4, false), 'Lists Props by name');
  assert.equal(L.listSay(cats, 4, true), 'Lists Props by board ID');
  assert.equal(L.listSay(cats, null, true), 'Pick a category: until then the field is empty');
  assert.equal(L.listSay(cats, 77, false), 'Pick a category: until then the field is empty', 'a category since deleted');
});

test('BRK-02, E-L12: header values are offered as typed, ignoring case', () => {
  const values = ['BOATHOUSE', 'NET LOFT', 'ROOM 2', 'STOREROOM'];
  assert.deepEqual(L.headerMatches(values, 'room 2'), ['ROOM 2']);
  assert.deepEqual(L.headerMatches(values, 'ro'), ['ROOM 2', 'STOREROOM'], 'starting with it first');
  assert.deepEqual(L.headerMatches(values, 'ROOM 2'), [], 'spelled exactly: nothing to pick');
  assert.deepEqual(L.headerMatches(values, '  '), []);
  assert.deepEqual(L.headerMatches(values, 'o', 2), ['BOATHOUSE', 'NET LOFT']);
  assert.deepEqual(L.headerMatches(undefined, 'x'), []);
});

test('BRK-13, BRK-18: Quick add tags an exact name anywhere, asks for a new one', () => {
  const cats = ['Cast', 'ZGEAR', 'Props'];
  const els = [
    { id: 1, name: 'TOVE', category: 'Cast' },
    { id: 7, name: 'Ztorch', category: 'Props' },
    { id: 5, name: 'ZTORCH', category: 'ZGEAR' },
    { id: 9, name: 'ZTORCH LAMP', category: 'ZGEAR' },
  ];
  assert.deepEqual(L.quickAddPlan('  ', els, new Set(), cats), { kind: 'none' });
  assert.deepEqual(L.quickAddPlan('ztorch', els, new Set(), cats), { kind: 'tag', element: els[2] }, 'ignoring case, the first in category order');
  assert.deepEqual(L.quickAddPlan('ZTORCH', els, new Set([5]), cats), { kind: 'tag', element: els[1] }, 'the one not on the sheet');
  assert.deepEqual(L.quickAddPlan('ztorch', els, new Set([5, 7]), cats), { kind: 'on', element: els[2] });
  assert.deepEqual(L.quickAddPlan('ZTOR', els, new Set(), cats), { kind: 'ask', name: 'ZTOR' }, 'a start is not a match');
  assert.deepEqual(L.quickAddPlan(' Lantern ', els, new Set(), cats), { kind: 'ask', name: 'Lantern' });
  assert.deepEqual(L.quickAddPlan('Props;Lantern', els, new Set(), cats), { kind: 'create', category: 'Props', name: 'Lantern' }, 'the semicolon form never asks');
});

test('BRK-13: the category picker starts on the New in category', () => {
  assert.deepEqual(L.categoryChoices(['Cast', 'Props'], 'Props'), { choices: ['Cast', 'Props'], at: 1 });
  assert.deepEqual(L.categoryChoices(['Cast', 'Props'], ' props '), { choices: ['Cast', 'Props'], at: 1 }, 'ignoring case');
  assert.deepEqual(L.categoryChoices(['Cast', 'Props'], 'Weather'), { choices: ['Weather', 'Cast', 'Props'], at: 0 }, 'a new one typed in New in comes first');
  assert.deepEqual(L.categoryChoices(['Cast', 'Props'], ''), { choices: ['Cast', 'Props'], at: 0 });
});

test('DOD-19: Saturdays and Sundays switch apart, the old Weekends box read as both', () => {
  assert.deepEqual(L.weekendDays(null, null, null), { sat: true, sun: true });
  assert.deepEqual(L.weekendDays('0', null, null), { sat: false, sun: false }, 'the old box stands for both');
  assert.deepEqual(L.weekendDays('0', '1', null), { sat: true, sun: false }, 'a day set on its own wins');
  assert.deepEqual(L.weekendDays('1', null, '0'), { sat: true, sun: false });
  const sat = { weekday: 'Sat' };
  const sun = { weekday: 'Sun' };
  const mon = { weekday: 'Mon' };
  assert.deepEqual([sat, sun, mon].map((d) => L.dayShown(d, { sat: false, sun: true })), [false, true, true]);
  assert.deepEqual([sat, sun, mon].map((d) => L.dayShown(d, false)), [false, false, true]);
  // The report: Sundays hidden, Saturdays shown, weeks from Sunday: six columns a week.
  const names = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const dates = Array.from({ length: 21 }, (_, i) => ({ weekday: names[i % 7] }));
  const pages = L.doodReportPages(dates, 18, { sat: true, sun: false }, 'sun');
  assert.deepEqual(pages.map((p) => p.length), [18], 'three six-day weeks fit in 18');
  assert.deepEqual(pages[0].slice(0, 7), [1, 2, 3, 4, 5, 6, 8], 'Saturday prints, Sunday is left out');
  const satOff = L.doodReportPages(dates, 18, { sat: false, sun: true }, 'sun');
  assert.equal(satOff[0].includes(6), false);
  assert.equal(satOff[0].includes(7), true);
  assert.deepEqual(L.doodReportPages(dates, 18, { sat: true, sun: true }, 'sun'), L.doodReportPages(dates, 18, true, 'sun'));
});

test('DBK-03: the shoot day strip heads the day in place of our day heading', () => {
  assert.equal(L.dayLead({ shoot_day_strip: true }, { heads: true }), 'shoot');
  assert.equal(L.dayLead({ shoot_day_strip: true }, { heads: false }), 'shoot', 'a board setting, not a Show on Board line');
  assert.equal(L.dayLead({ shoot_day_strip: false }, { heads: true }), 'day');
  assert.equal(L.dayLead({}, { heads: false }), null);
  assert.equal(L.dayLead(null, { heads: true }), 'day');
});

test('strip marks: plain text forms on badges of fixed colors', () => {
  assert.equal(L.STRIP_MARK.conflict, '⚠︎');
  assert.equal(L.STRIP_MARK.flag, '⚑︎');
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  // Fixed colors, not the theme's, so the badge reads the same on any strip color.
  assert.match(css, /\.row \.cf-mark \{ background: #[0-9a-f]{6}; color: #fff;/);
  assert.match(css, /\.row \.rf-mark \{ background: #fff;/);
});

test('tooltips: none on the board or the sheet as a whole, one short line on the step buttons', () => {
  const page = (f) => readFileSync(new URL(`../ui/js/${f}`, import.meta.url), 'utf8');
  const main = page('main.js');
  assert.doesNotMatch(main, /\$\('(board|breakdown)'\)\.title\s*=/);
  assert.doesNotMatch(main, /\.title\s*=\s*SHORTCUTS/);
  const bd = page('breakdown.js');
  assert.match(bd, /title: withShortcut\('Previous sheet', 'sheet\.previous'\)/);
  assert.match(bd, /title: withShortcut\('Next sheet', 'sheet\.next'\)/);
});

test('Boards dialog: a name goes once, and a redraw waits for the click', () => {
  assert.equal(L.nameToSend('Main', null, ' Second '), 'Second');
  assert.equal(L.nameToSend('Main', null, 'Main '), null, 'unchanged');
  assert.equal(L.nameToSend('Main', 'Second', 'Second'), null, 'Return sent it; the change after does not');
  assert.equal(L.nameToSend('Main', 'Second', 'Third'), 'Third');
  const gate = L.pressGate();
  const done = [];
  gate.run(() => done.push('now'));
  assert.deepEqual(done, ['now']);
  gate.press();
  gate.run(() => done.push('first'));
  gate.run(() => done.push('last'));
  assert.deepEqual(done, ['now'], 'held while pressed');
  done.push('click');
  gate.release();
  assert.deepEqual(done, ['now', 'click', 'last'], 'after the click, the last redraw only');
  gate.release();
  assert.equal(done.length, 3);
});

test('dialogs with editable rows redraw after the click, and send a name once', () => {
  const page = (f) => readFileSync(new URL(`../ui/js/${f}`, import.meta.url), 'utf8');
  // Every dialog gets the press gate from customDialog (decision 170).
  const modal = page('modal.js');
  assert.match(modal, /const box = build\(close, gate\.run\);/);
  assert.match(modal, /backdrop\.addEventListener\('pointerdown', \(\) => \{\s*gate\.press\(\);/);
  // Boards and Sub-boards, Layouts, Red Flag Types and the Plan dialogs redraw through it.
  assert.match(page('boards.js'), /later\(\(\) => draw\(keepFocus\)\)/);
  assert.match(page('layouts.js'), /later\(draw\);/);
  assert.match(page('redflags.js'), /later\(\(\) => draw\(focusKey\)\)/);
  const plan = page('plan.js');
  for (const f of ['later(draw);', 'later(drawReport);', 'later(() => {']) assert.ok(plan.includes(f), f);
  assert.doesNotMatch(plan, /customDialog\(\(close\) => \{\s*const error = errorLine\('pl-error'\);\s*const (rows|report|listBox)/, 'the editable Plan dialogs take later');
  // A name in Layouts and Red Flag Types goes once (decision 151's nameToSend).
  assert.match(page('layouts.js'), /nameToSend\(l\.name, sent, name\.value\)/);
  assert.match(page('redflags.js'), /nameToSend\(t\.name, sent, name\.value\)/);
});

test('banner colors: about fifty swatches, checked hex, black or white text', () => {
  assert.equal(L.BANNER_PALETTE.length + 9, 49);
  assert.equal(new Set(L.BANNER_PALETTE).size, L.BANNER_PALETTE.length, 'no swatch twice');
  for (const c of L.BANNER_PALETTE) assert.equal(L.bannerHex(c), c, c);
  for (const bad of ['teal', '#3FB8A8', '#3fb8a', 'red; x', null, '']) assert.equal(L.bannerHex(bad), null, String(bad));
  assert.equal(L.inkFor('#ffffff'), '#000000');
  assert.equal(L.inkFor('#1f5fbf'), '#ffffff');
  assert.equal(L.inkFor('#808080'), '#000000', 'half way up is black, as Rgb::ink');
  const css = L.bannerHexRules(['#3fb8a8', 'x} body {color:red', '#3fb8a8']);
  assert.match(css, /\.row\.banner\.bh-3fb8a8, \.bn-swatch\.bh-3fb8a8 \{ background-color: #3fb8a8; color: #000000; \}/);
  assert.doesNotMatch(css, /body/);
});

test('Undo of New Sheet or Duplicate Sheet shows the sheet shown before', () => {
  const made = [
    { made: 40, back: 5 },
    { made: 41, back: 40 },
  ];
  const have = (ids) => (id) => ids.includes(id);
  assert.equal(L.sheetBack(41, made, have([5, 25, 40])), 40, 'one undone');
  assert.equal(L.sheetBack(40, made, have([5, 25])), 5, 'then the next');
  assert.equal(L.sheetBack(41, made, have([5, 25])), 5, 'both undone at once');
  assert.equal(L.sheetBack(41, made, have([5, 25, 40, 41])), 41, 'redone: still there');
  assert.equal(L.sheetBack(25, made, have([5])), 25, 'a sheet no New Sheet made is left alone');
  assert.equal(L.sheetBack(7, [{ made: 7, back: null }], have([])), 7);
});

test('Find marks: scenes on every board, a banner on its own board only', () => {
  const hits = [
    { kind: 'sheet', sheet: 4 },
    { kind: 'banner', board: 1, strip: 90 },
    { kind: 'element', element: 7 },
  ];
  const main = L.foundOn(hits, 1);
  assert.deepEqual([...main.sheets], [4]);
  assert.deepEqual([...main.strips], [90]);
  const beside = L.foundOn(hits, 2);
  assert.deepEqual([...beside.sheets], [4], 'the board beside marks the same scenes');
  assert.equal(beside.strips.size, 0, "another board's banner is not marked");
  assert.equal(L.foundOn([{ kind: 'banner', board: 1, strip: 90 }], 2), null);
  assert.equal(L.foundOn([], 1), null);
  assert.equal(L.foundOn(null, 1), null);
});

test('strip line styles: a line after, a rule under, both or none', () => {
  assert.equal(L.ruleClass({ rule_after: true }), ' rl-after');
  assert.equal(L.ruleClass({ rule_under: true }), ' rl-under');
  assert.equal(L.ruleClass({ rule_after: true, rule_under: true }), ' rl-after rl-under');
  assert.equal(L.ruleClass({}), '');
  assert.equal(L.ruleClass(null), '');
});

test('outside changes: Later first, the default last and primary, every choice said', () => {
  const both = { name: 'film.mtsc', changed_at: '2026-10-14 09:30 UTC', dirty: true, choices: ['merge', 'keep_mine', 'load_theirs'], default: 'merge', confirm_load: true, problem: null };
  assert.deepEqual(
    L.outsideButtons(both).map((b) => [b.choice, b.label, b.primary]),
    [
      ['later', 'Later', false],
      ['keep_mine', 'Keep Mine', false],
      ['load_theirs', 'Load Theirs', false],
      ['merge', 'Merge', true],
    ],
  );
  const clean = { ...both, dirty: false, choices: ['load_theirs', 'keep_mine'], default: 'load_theirs', confirm_load: false };
  assert.deepEqual(
    L.outsideButtons(clean).map((b) => b.choice),
    ['later', 'keep_mine', 'load_theirs'],
  );
  const said = L.outsideMessage(both);
  assert.match(said[0], /film\.mtsc was changed outside Method Schedule \(last changed 2026-10-14 09:30 UTC\), and this window has unsaved changes too\./);
  assert.ok(said.some((l) => l.startsWith('Merge ')) && said.some((l) => l.includes('Undo brings them back')));
  assert.match(L.outsideMessage(clean)[0], /no unsaved changes/);
  assert.ok(!L.outsideMessage(clean).some((l) => l.startsWith('Merge ')), 'only what is offered');
  const bad = { ...clean, choices: ['keep_mine'], default: 'keep_mine', problem: "The version on disk can't be read as a schedule." };
  assert.deepEqual(L.outsideButtons(bad).map((b) => b.choice), ['later', 'keep_mine']);
  assert.equal(L.outsideMessage(bad)[1], bad.problem);
  for (const l of [...said, ...L.outsideMessage(clean)]) assert.ok(!l.includes('!') && !l.includes(String.fromCharCode(0x2014)), l);
});

test('merge bases: the list value and the base asked for, both ways', () => {
  for (const v of ['auto', 'disk', 'last_read', 'film 2026-10-11 14.03.00.000 UTC.mtsc']) {
    assert.equal(L.baseValueOf(L.baseChoiceOf(v)), v);
  }
  assert.deepEqual(L.baseChoiceOf('last_read'), { kind: 'last_read' });
  assert.deepEqual(L.baseChoiceOf('x.mtsc'), { kind: 'backup', name: 'x.mtsc' });
});

test('Revert to Backup: each backup with how far it is from now', () => {
  const b = (changes) => ({ name: 'n', label: 'Backup from 2026-10-11 14:03 UTC', changes });
  assert.equal(L.backupLine(b(3)), 'Backup from 2026-10-11 14:03 UTC, 3 changes from now');
  assert.equal(L.backupLine(b(1)), 'Backup from 2026-10-11 14:03 UTC, 1 change from now');
  assert.equal(L.backupLine(b(0)), 'Backup from 2026-10-11 14:03 UTC, the same as now');
  assert.equal(L.backupLine(b(null)), "Backup from 2026-10-11 14:03 UTC, can't be read");
});

test('breakdown designer: rows of the usual or own lengths, dragging between rows', () => {
  const keys = 'abcdefghijklmnopqr'.split('');
  assert.deepEqual(L.sheetRows(keys, []).map((r) => r.length), [6, 5, 7], 'six, five, the rest');
  assert.deepEqual(L.sheetRows(keys, [4, 4]).map((r) => r.length), [4, 4, 10]);
  assert.deepEqual(L.sheetRows(keys.slice(0, 5), [4, 4]).map((r) => r.length), [4, 1], 'fewer fields than the rows hold');
  assert.deepEqual(L.sheetRows([], [3]), []);
  const rows = [['a', 'b', 'c'], ['d', 'e']];
  assert.deepEqual(L.gridMove(rows, 'a', 1, 1), [['b', 'c'], ['d', 'a', 'e']], 'to another row');
  assert.deepEqual(L.gridMove(rows, 'a', 0, 3), [['b', 'c', 'a'], ['d', 'e']], 'later in its own row');
  assert.deepEqual(L.gridMove(rows, 'c', 0, 0), [['c', 'a', 'b'], ['d', 'e']]);
  assert.deepEqual(L.gridMove(rows, 'e', 2, 0), [['a', 'b', 'c'], ['d'], ['e']], 'a new row');
  assert.deepEqual(L.gridMove([['a'], ['b']], 'a', 1, 0), [['a', 'b']], 'an emptied row goes');
  assert.deepEqual(L.gridMove(rows, 'z', 0, 0), rows);
  const d = L.gridDesign([['c', 'a'], ['d']], ['a', 'b', 'c', 'd'], new Set(['b']));
  assert.deepEqual(d, { keys: ['c', 'b', 'a', 'd'], rows: [2, 1] }, 'a hidden field keeps its place');
});

test('banner fonts: a few safe families, classes from the fixed names only', () => {
  assert.deepEqual(L.BANNER_FONTS.map((f) => f[0]), ['usual', 'serif', 'sans', 'mono']);
  assert.equal(L.bannerFontClass('serif'), ' bf-serif');
  assert.equal(L.bannerFontClass('mono'), ' bf-mono');
  for (const none of ['usual', '', null, undefined, 'Comic Sans', 'serif; color:red']) assert.equal(L.bannerFontClass(none), '', String(none));
});

test('the printed board is as dense as the screen: a line of text in the same height', () => {
  const css = readFileSync(new URL('../ui/print.css', import.meta.url), 'utf8');
  const board = readFileSync(new URL('../ui/js/board.js', import.meta.url), 'utf8');
  const screen = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  const linePx = Number(/const LINE_PX = (\d+);/.exec(board)[1]);
  const textPx = Number(/html, body \{[^}]*font: (\d+)px/.exec(screen)[1]);
  const textPt = Number(/\.method-report \{[^}]*font-size: ([\d.]+)pt/.exec(css)[1]);
  const m = /tr\.bp td \{ padding-top: ([\d.]+)pt; padding-bottom: ([\d.]+)pt; line-height: ([\d.]+)pt; border-bottom: ([\d.]+)pt/.exec(css);
  assert.ok(m, 'the printed board line rule');
  const line = m.slice(1).map(Number).reduce((a, b) => a + b, 0);
  const want = (textPt * linePx) / textPx;
  assert.ok(Math.abs(line - want) < 0.1, `${line}pt, want ${want}pt`);
  const tall = Number(/tr\.bp\.banner\.tall \{ height: ([\d.]+)pt; \}/.exec(css)[1]);
  assert.ok(Math.abs(tall - 2 * want) < 0.1, `two-line banner ${tall}pt`);
});

test('Fountain lines per page: a whole number, 40 to 70 (decision 195)', () => {
  assert.deepEqual(L.FOUNTAIN_LINES, { min: 40, max: 70, usual: 56 });
  assert.equal(L.fountainLines('56'), 56);
  assert.equal(L.fountainLines(' 40 '), 40);
  assert.equal(L.fountainLines('70'), 70);
  for (const bad of ['39', '71', '55.5', '-56', '5 6', '', 'fifty', null, undefined, '0056']) assert.equal(L.fountainLines(bad), null, String(bad));
});

test('the user guide reads as headings, paragraphs and lists, never HTML (decision 199)', () => {
  const md = ['# Guide', '', '## Getting started', 'One line', 'joined.', '', '- **File > New** an', '  empty one.', '- `B` <b>not html</b>', 'After the list.', '', '### Small'].join('\n');
  assert.deepEqual(L.guideBlocks(md), [
    { kind: 'h1', text: 'Guide' },
    { kind: 'h2', text: 'Getting started' },
    { kind: 'p', text: 'One line joined.' },
    { kind: 'ul', items: ['**File > New** an empty one.', '`B` <b>not html</b>'] },
    { kind: 'p', text: 'After the list.' },
    { kind: 'h3', text: 'Small' },
  ]);
  const runs = (t) => L.guideRuns(t).map((r) => [r.text, r.bold ? 'b' : r.code ? 'c' : r.em ? 'i' : '']);
  assert.deepEqual(runs('Choose **Help > Open Sample** (`⌘/`), *Saltmarsh Road*.'), [
    ['Choose ', ''],
    ['Help > Open Sample', 'b'],
    [' (', ''],
    ['⌘/', 'c'],
    ['), ', ''],
    ['Saltmarsh Road', 'i'],
    ['.', ''],
  ]);
  assert.deepEqual(runs('See [MERGE.md](MERGE.md) and 2 * 3'), [['See MERGE.md and 2 * 3', '']]);
  assert.deepEqual(L.guideBlocks(''), []);
  // The real guide: no em dashes, no exclamation marks, every section a heading.
  const guide = readFileSync(new URL('../../../docs/GUIDE.md', import.meta.url), 'utf8');
  assert.doesNotMatch(guide, /[\u2014!]/);
  const blocks = L.guideBlocks(guide);
  assert.equal(blocks[0].kind, 'h1');
  assert.ok(blocks.filter((b) => b.kind === 'h2').length >= 12);
});

test('the strip\'s first cell is set apart, as the scene cell is in the mark (decision 206)', () => {
  const board = readFileSync(new URL('../ui/js/board.js', import.meta.url), 'utf8');
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  assert.match(board, /extra: ` sc-\$\{g\.k\}\$\{lead \? ' lead' : ''\}`/, 'only the first line\'s first field gets the class');
  // Out to the strip's edge and as tall as a board line, so it fills its row.
  assert.match(css, /\.row\.scene > \.lead \{[^}]*margin-left: -8px;[^}]*line-height: 22px;/);
  assert.match(css, /--lead-cell: rgba\(/, 'a tint the strip color shows through');
});

test('the printed call sheet says dates, lengths, times and the sun plainly (decision 207)', () => {
  assert.equal(L.callSheetDate('2026-10-20'), 'Tuesday, Oct 20, 2026');
  assert.equal(L.callSheetDate('2027-01-01'), 'Friday, Jan 1, 2027');
  assert.equal(L.callSheetDate(null), null);
  assert.equal(L.callSheetDate('10/20/2026'), null);
  assert.equal(L.lengthText(520), '8:40');
  assert.equal(L.lengthText(5), '0:05');
  assert.equal(L.spanText('07:00', '07:20'), '07:00–07:20');
  assert.equal(L.spanText('', ''), '', 'no call time, no clock times');
  assert.equal(L.sunText({ state: 'times', sunrise: '07:15', sunset: '18:19' }), '07:15 · 18:19');
  assert.equal(L.sunText({ state: 'always_up' }), 'Sun up all day');
  assert.equal(L.sunText({ state: 'unknown' }), '');
  assert.equal(L.sunText(null), '');
  // Ink only, so it copies cleanly: no background fills in its print rules.
  const css = readFileSync(new URL('../ui/print.css', import.meta.url), 'utf8');
  const cs = css.slice(css.indexOf('/* The printed call sheet'));
  assert.ok(cs.length > 100);
  assert.doesNotMatch(cs, /background/);
});

test('looks: Paper by default, Graphite and Ledger by choice, one layout (decision 208)', () => {
  assert.deepEqual(L.THEMES.map(([v]) => v), ['paper', 'graphite', 'ledger']);
  assert.equal(L.themeAttr('paper'), null, 'Paper is the stylesheet as written');
  assert.equal(L.themeAttr('graphite'), 'graphite');
  assert.equal(L.themeAttr('ledger'), 'ledger');
  for (const none of ['', null, undefined, 'neon', 'Graphite']) assert.equal(L.themeAttr(none), null, String(none));
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  for (const t of ['graphite', 'ledger']) assert.match(css, new RegExp(`:root\\[data-theme="${t}"\\] \\{\\n  color-scheme: (dark|light);`), t);
  // No look changes a strip line's height: the board draws only the lines in view by it.
  const looks = css.slice(css.indexOf('/* Looks (decision 208)'));
  assert.doesNotMatch(looks, /\.line[^{]*\{[^}]*height:/);
});

test('updates: an automatic check is silent, one the user asked for answers (decision 213)', () => {
  const st = (o) => ({ phase: 'idle', version: null, manual: false, said: null, automatic: true, available: true, ...o });
  assert.equal(L.updateMessage(st({ phase: 'checking' })), null);
  assert.equal(L.updateMessage(st({ phase: 'downloading', version: '0.2.0' })), null);
  assert.equal(L.updateMessage(st({ phase: 'ready', version: '0.2.0' })), null, 'the footer shows it, the status line stays quiet');
  assert.deepEqual(L.updateMessage(st({ phase: 'checking', manual: true })), { text: 'Checking for updates…', error: false });
  assert.deepEqual(L.updateMessage(st({ phase: 'downloading', manual: true, version: '0.2.0' })), { text: 'Downloading Method Schedule 0.2.0…', error: false });
  assert.deepEqual(L.updateMessage(st({ said: 'current' })), { text: 'Method Schedule is up to date.', error: false });
  assert.equal(L.updateMessage(st({ said: 'failed' })).error, true);
  assert.match(L.updateMessage(st({ said: 'unavailable', available: false })).text, /methodschedule\.com/);
  assert.equal(L.updateMessage(st({ said: 'install-failed', phase: 'ready' })).error, true);
  assert.equal(L.updateMessage(st({ phase: 'installing' })).text, 'Installing the update…');
  assert.equal(L.updateMessage(null), null);
});


test('screen block 6: the lead cell keeps | and _ rules, primary buttons stay readable', () => {
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  // No box-shadow on the lead cell: it would hide a field's .rl-after and .rl-under rules.
  for (const m of css.matchAll(/([^{}]*\.lead[^{}]*)\{([^}]*)\}/g)) assert.doesNotMatch(m[2], /box-shadow/, m[1].trim());
  assert.match(css, /\.btn\.btn-primary:disabled \{[^}]*opacity: 1;/, 'a disabled primary button is readable');
  assert.match(css, /:root\[data-theme="ledger"\] \.btn:not\(\.btn-primary\)/, "Ledger's white buttons skip primary ones");
  assert.match(css, /input\.date-field\.w-s \{ width: 8\.25em; \}/, 'a short date field fits its hint');
});

test('decision 219: fenced blocks kept as written for the notices; the board fills the window', () => {
  const md = '## License texts\n\n### MIT\n\n```text\nMIT License\n\n# not a heading\n- not a list\n```\n\nAfter.';
  const b = L.guideBlocks(md);
  assert.deepEqual(b.map((x) => x.kind), ['h2', 'h3', 'pre', 'p']);
  assert.equal(b[2].text, 'MIT License\n\n# not a heading\n- not a list');
  assert.equal(b[3].text, 'After.');
  // An unclosed fence keeps the rest as written rather than dropping it.
  assert.equal(L.guideBlocks('```\nkept').at(-1).text, 'kept');
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  assert.match(css, /\n\.board \{ outline: none; \}/, 'no width cap on the board');
});

// WCAG contrast for the Graphite and Ledger looks (issue #5). Paper's colors are checked by
// `scripts/make-brand.py --contrast`; these two are read from styles.css the same way, so a look
// that darkens or lightens one of its pairs fails here.
function rgb(hex) {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  assert.ok(m, `expected a #rrggbb color, got ${hex}`);
  return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16) / 255);
}

// WCAG 2.1 relative luminance and contrast ratio (https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio).
function luminance([r, g, b]) {
  const [R, G, B] = [r, g, b].map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * R + 0.7152 * G + 0.0722 * B;
}

function contrast(fg, bg) {
  const [a, b] = [luminance(rgb(fg)), luminance(rgb(bg))].sort((x, y) => y - x);
  return (a + 0.05) / (b + 0.05);
}

test('looks: Graphite and Ledger keep text on its surfaces at 4.5:1 or better', () => {
  const css = readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
  const pairs = [
    ['--text', '--bg'],
    ['--text', '--surface'],
    ['--muted', '--surface'],
    ['--break-text', '--break-bg'],
  ];
  for (const theme of ['graphite', 'ledger']) {
    const block = css.slice(css.indexOf(`:root[data-theme="${theme}"] {`));
    const vars = new Map(
      [...block.slice(0, block.indexOf('\n}')).matchAll(/(--[\w-]+):\s*(#[0-9a-f]{3,8})\s*;/gi)].map((m) => [
        m[1],
        m[2],
      ]),
    );
    for (const [fg, bg] of pairs) {
      assert.ok(vars.has(fg), `${theme} defines ${fg}`);
      assert.ok(vars.has(bg), `${theme} defines ${bg}`);
      const ratio = contrast(vars.get(fg), vars.get(bg));
      assert.ok(ratio >= 4.5, `${theme} ${fg} on ${bg} is ${ratio.toFixed(2)}:1, needs 4.5:1`);
    }
  }
});
