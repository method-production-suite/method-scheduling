// The Reports tab: the schedule's report list (report_list, src/reportsetup.rs), each
// report drawn as printable pages from the engine's report data (report_data,
// src/reports.rs) with its own options, a preview of exactly the pages that print, and
// Print, Save as PDF and Export. Our own plain layouts (decisions.md, entries 23 and 59).
// Every value comes from Rust; nothing here computes a count. js/report.js does the
// paging and printing.
//
// The list: reports in the schedule's order, Up and Down, Copy (a named copy with the
// same options), Delete (copies; the last of a kind stays). The options of the picked
// report sit below it, each change one undo step: name, date range, paper, header layout,
// text size, the switches that apply to its kind, and the category, element and column
// pickers (the simple report designer), the strip layout, category columns and where
// pages break (decision 100). Header Layouts… designs the headers a report picks from
// (decision 99); Production Info edits the fields they can place.

import { el } from './dom.js';
import { setRules } from './dynstyle.js';
import { layout, scalePreview, printPages, printedDate, row, td, th } from './report.js';
import { DEFAULT_LAYOUT, LAYOUT_ALIGNS, TEXT_SIZES, cellText, gridRules, labelOf, layoutHead, printShares, sceneRows, stripCell, stripGrid } from './strips.js';
import { bannerLook } from './board.js';
import { customDialog, confirmDialog, errorLine } from './modal.js';
import { dateInput, dayDate, showDate } from './dates.js';
import { loadWeekends } from './dood.js';
import { doodReportPages } from './logic.js';

/** What File > Export can write for each kind of report (src/export.rs). */
export const EXPORTABLE = { dood: 'dood', one_line: 'one_line', elements: 'elements', element_detail: 'element_detail', breakdown: 'breakdown', red_flags: 'red_flags' };

/** The options each kind uses, beyond name, paper, header and text size. */
const STRIPS = ['columns', 'layout', 'element_columns'];
const DAYS = ['shoot_days', 'day_breaks', 'total_count'];
const APPLIES = {
  one_line: ['range', 'synopsis', 'cast_names', 'page_break', 'banners', 'colors', 'icons', ...STRIPS, ...DAYS],
  shooting: ['range', 'synopsis', 'cast_names', 'banners', 'categories', 'colors', 'icons', ...STRIPS, ...DAYS],
  breakdown: ['range', 'synopsis', 'cast_names', 'page_break', 'categories', 'colors', 'icons', 'total_count', ...STRIPS],
  breakdown_day: ['range', 'synopsis', 'cast_names', 'page_break', 'categories', 'colors', 'icons', ...STRIPS, ...DAYS],
  dood: ['range', 'categories', 'elements', 'no_holds'],
  elements: ['categories', 'elements'],
  element_detail: ['range', 'synopsis', 'cast_names', 'categories', 'elements', 'colors', ...STRIPS],
  totals: ['range', 'categories'],
  calendar: ['range', 'weekdays_only', 'icons'],
  vertical: ['range'],
  multi_unit: ['range', 'synopsis', 'cast_names', 'banners', 'elements', 'colors', 'icons', ...STRIPS],
  conflicts: ['range'],
  red_flags: ['range', 'categories', 'elements'],
};

const SWITCHES = [
  ['synopsis', 'Synopsis'],
  ['banners', 'Banners'],
  ['cast_names', 'Cast names'],
  ['shoot_days', 'Shoot day lines'],
  ['day_breaks', 'Day breaks'],
  ['total_count', 'Total count'],
  ['colors', 'Strip colors'],
  ['icons', 'Red flag marks'],
  ['no_holds', 'Without holds'],
  ['weekdays_only', 'Weekdays only'],
];

const LEGEND_START = 'An event on a work day shows its code, then the work code. Shaded columns are days off; the line under the codes marks each work span.';

// Date columns per grid page, scenes per vertical page: what fits at a readable size.
const DATES_PER_PAGE = { portrait: 18, landscape: 28 };
const SCENES_PER_PAGE = { portrait: 18, landscape: 30 };


function dayText(d) {
  const when = d.date ? `, ${dayDate(d.weekday, d.date)}` : '';
  return `Day ${d.number}${when}`;
}

function endText(d) {
  const scenes = d.scenes === 1 ? '1 scene' : `${d.scenes} scenes`;
  return `End of day ${d.number}: ${scenes}, ${d.pages || '0'} pages, shoot time ${d.time || '0:00'}`;
}

/** One header item as it prints: the title larger, a space pushing the rest across, dates in the user's format. */
function headItem(i) {
  if (i.kind === 'space') return el('span', { class: 'rpt-hspace' });
  if (i.kind === 'today') return el('span', { class: 'rpt-hitem', text: printedDate() });
  if (i.kind === 'start_date') return i.text ? el('span', { class: 'rpt-hitem', text: `Starts ${showDate(i.text)}` }) : null;
  if (!i.text) return null;
  return el('span', { class: i.kind === 'title' ? 'rpt-htitle' : 'rpt-hitem', text: i.text });
}

/** The report's header as its layout designs it (decisions.md, entry 99): rows of items, its looks. Null when the layout prints none. */
export function headBlock(head) {
  if (!head || !head.include) return null;
  const rows = (head.rows || [])
    .map((r) => r.map(headItem).filter(Boolean))
    .filter((items) => items.some((n) => !n.classList.contains('rpt-hspace')))
    .map((items) => el('div', { class: 'rpt-hrow' }, ...items));
  if (!rows.length) return null;
  const looks = [`al-${head.align || 'left'}`, `hz-${head.size || 'normal'}`, head.bold ? 'bold' : '', head.italic ? 'italic' : '', head.every_page ? 'every' : ''].filter(Boolean).join(' ');
  return el('div', { class: `rpt-header ${looks}` }, ...rows);
}

/** The first page's block: the header (unless it prints atop every page), then the report's notes. */
function intro(dto, extra) {
  const notes = [];
  if (dto.range) notes.push(`Dates ${dto.range}.`);
  if (dto.filtered && dto.filtered.length) notes.push(`Left out: ${dto.filtered.join(', ')}.`);
  notes.push(...(extra || []));
  const head = dto.head && dto.head.every_page ? null : headBlock(dto.head);
  return el('div', { class: 'rpt-intro' }, head, ...notes.map((t) => el('p', { class: 'rpt-note', text: t })));
}

/** The report's last line, its scene count and pages, when asked for (decision 100). */
function totalRow(dto, n) {
  return dto.total ? [row(el('tr', { class: 'rpt-end rpt-total' }, td(dto.total, '', n)))] : [];
}

/** Splits `items` (days or blocks) where one starts a new page; each part becomes a flow. */
function byPage(items) {
  const parts = [];
  for (const it of items) {
    if (!parts.length || it.new_page) parts.push([]);
    parts[parts.length - 1].push(it);
  }
  return parts;
}

function emptyFlow(dto) {
  return [{ name: dto.name, cls: '', cols: ['c-wide'], head: null, rows: [row(el('tr', { class: 'rpt-empty' }, td(dto.empty)))], intro: intro(dto) }];
}

// ---------------------------------------------------------------------------
// Scene lines with the report's columns

/**
 * Where the report's fields sit (strips.js, stripGrid): its layout's first line as the
 * columns, each a share of the page; later lines print under them (decisions.md, entry 87).
 */
function columnsOf(dto, name) {
  const grid = stripGrid(dto.layout);
  const shares = printShares(grid.cols);
  setRules(`report-${name}`, shares.map((w, i) => `.method-report .${name} col.lc-${i} { width: ${w}%; }`).join('\n'));
  return grid;
}

/** Where the synopsis goes: nowhere extra when a field shows it, else under the set (or the first wide column). */
function underOf(grid) {
  if (grid.cells.some((g) => g.c.field === 'synopsis')) return null;
  const first = grid.cells.filter((g) => g.line === 0);
  return first.find((g) => g.c.field === 'set') || first.find((g) => g.c.width === 'wide' || g.c.width === 'widest') || first[0] || null;
}

function label(c) {
  return labelOf(c);
}

function headOf(grid, under) {
  return layoutHead(grid, (c) => (under && c === under.c ? `${label(c)} and synopsis` : label(c)));
}

/** A scene's rows: its lines (the shared cell renderer), the synopsis under one, a red flag mark, the strip's color, struck when omitted. */
function sceneRow(dto, l, grid, under) {
  const flag = dto.icons && l.flags && l.flags.length ? el('span', { class: 'rpt-flag', text: '⚑ ', title: l.flags.join('; ') }) : null;
  const cls = `${dto.colors && l.tone ? `tone-${l.tone}` : ''}${l.omitted ? ' omitted' : ''}`;
  return sceneRows(grid, Array.isArray(l.cells) ? l.cells : [], cls, {
    lead: flag,
    first: (t) => (l.omitted ? `${t} (omitted)` : t),
    sub: under ? { k: under.k, text: l.synopsis } : null,
  });
}

/** A banner's row, in its look on the board (bold, color, size, alignment; board.js). */
function bannerRow(l, n) {
  const cells = n > 1 ? [td(l.text, 'banner-text', n - 1), td(l.duration, 'num')] : [td(`${l.text} (${l.duration})`, 'banner-text')];
  return el('tr', { class: `rpt-banner${bannerLook(l)}` }, ...cells);
}

// ---------------------------------------------------------------------------
// The reports

/**
 * The one-line schedule's columns follow the board's strip layout (decisions.md, entry
 * 39) or the report's own columns. Unless a column shows the synopsis, it goes under the
 * set. With a page per day, each day starts a page.
 */
function oneLine(dto) {
  const lay = columnsOf(dto, 'one-line');
  const n = lay.cols.length;
  const under = underOf(lay);
  const cols = lay.cols.map((_, i) => `lc-${i}`);
  const dayRows = (d) => {
    const rows = dto.shoot_days ? [row(el('tr', { class: 'rpt-day' }, td(dayText(d), '', n)), true)] : [];
    for (const l of d.lines) rows.push(...(l.kind === 'banner' ? [bannerRow(l, n)] : sceneRow(dto, l, lay, under)).map((tr) => row(tr, true)));
    if (dto.day_breaks) rows.push(row(el('tr', { class: 'rpt-end' }, td(endText(d), '', n))));
    else if (rows.length) rows[rows.length - 1].keep = false;
    return rows;
  };
  const parts = byPage(dto.days);
  return parts.map((days, k) => ({
    name: parts.length > 1 ? `${dto.name}, day ${days[0].number}` : dto.name,
    cls: 'one-line',
    cols,
    head: headOf(lay, under),
    rows: [...days.flatMap(dayRows), ...(k === parts.length - 1 ? totalRow(dto, n) : [])],
    intro: k === 0 ? intro(dto) : null,
    newPage: k > 0,
  }));
}

function groupsBlock(groups) {
  return el(
    'div',
    { class: 'sh-groups' },
    ...groups.map((g) => el('div', {}, el('div', { class: 'sh-cat', text: g.category }), ...g.elements.map((e) => el('span', { class: 'sh-el', text: e })))),
  );
}

/**
 * The block reports' fields (shooting schedule, breakdown by strip): the report's layout
 * placed as the board places it (strips.js, stripGrid and gridRules), each column a share
 * of the line. `name` is the flow's class the rules are scoped to.
 */
function blockGrid(dto, name) {
  const grid = stripGrid(dto.layout && dto.layout.length ? dto.layout : DEFAULT_LAYOUT.slice(0, 5));
  const scope = `.method-report .${name} .sh-cells`;
  setRules(`report-${name}`, `${scope} { grid-template-columns: ${printShares(grid.cols).map((w) => `${w}fr`).join(' ')}; }\n${gridRules(scope, grid)}`);
  return grid;
}

/**
 * A scene's block: its fields through the shared cell renderer (custom text, alignment,
 * bold, italic, later lines), the day it is on, synopsis, location and elements.
 */
function sceneBlock(dto, l, grid, extra) {
  const cells = Array.isArray(l.cells) ? l.cells : [];
  const flag = dto.icons && l.flags && l.flags.length ? el('span', { class: 'rpt-flag', text: '⚑ ', title: l.flags.join('; ') }) : null;
  const text = (g) => {
    const t = cellText(g, cells);
    if (g.line === 0 && g.c.field === 'scene') return `Scene ${t || '-'}`;
    if (g.line === 0 && g.c.field === 'pages') return `${t || '0'} pg`;
    return t;
  };
  const shown = grid.cells.filter((g) => g.line === 0 || cellText(g, cells));
  const line = el('div', { class: 'sh-line sh-cells' }, ...shown.map((g, i) => stripCell('span', g, text(g), { extra: ` sc-${g.k}`, before: i === 0 ? flag : null })));
  return el(
    'td',
    { class: dto.colors && l.tone ? `tone-${l.tone}` : '' },
    extra ? el('div', { class: 'sh-when', text: extra }) : null,
    line,
    l.synopsis ? el('div', { class: 'sh-syn', text: l.synopsis }) : null,
    l.location ? el('div', { class: 'sh-loc', text: `Location: ${l.location}` }) : null,
    l.groups && l.groups.length ? groupsBlock(l.groups) : null,
  );
}

function shooting(dto) {
  const grid = blockGrid(dto, 'sh-flow');
  // Each day is a flow of its own, so each starts a new page.
  return dto.days.map((d, k) => {
    const rows = d.lines.map((l) => row(l.kind === 'banner' ? bannerRow(l, 1) : el('tr', {}, sceneBlock(dto, l, grid))));
    if (dto.day_breaks) rows.push(row(el('tr', { class: 'rpt-end' }, td(endText(d)))));
    if (k === dto.days.length - 1) rows.push(...totalRow(dto, 1));
    const head = dto.shoot_days ? el('tr', { class: 'rpt-day' }, el('th', { text: `Day ${d.number} of ${dto.days.length}${d.date ? `, ${dayDate(d.weekday, d.date)}` : ''}`, attrs: { scope: 'col' } })) : null;
    return { name: `${dto.name}, day ${d.number}`, cls: 'sh-flow', cols: ['c-wide'], head, rows, intro: k === 0 ? intro(dto) : null, newPage: true };
  });
}

/** The breakdown by strip: a block per sheet with its day, a page each with a page per day. */
function breakdown(dto) {
  const grid = blockGrid(dto, 'bk-flow');
  const when = (b) => (b.day ? `Day ${b.day}${b.date ? `, ${dayDate(b.weekday, b.date)}` : ''}` : 'Not scheduled');
  const parts = byPage(dto.blocks);
  return parts.map((blocks, k) => ({
    name: parts.length > 1 ? `${dto.name}, scene ${blocks[0].line.scenes || '-'}` : dto.name,
    cls: 'bk-flow',
    cols: ['c-wide'],
    head: null,
    rows: [...blocks.map((b) => row(el('tr', { class: 'bk-block' }, sceneBlock(dto, b.line, grid, when(b))))), ...(k === parts.length - 1 ? totalRow(dto, 1) : [])],
    intro: k === 0 ? intro(dto) : null,
    newPage: k > 0,
  }));
}

/** The breakdown by day: each day's scenes, then its elements merged by category. */
function breakdownDay(dto) {
  const lay = columnsOf(dto, 'bk-day');
  const n = lay.cols.length;
  const under = underOf(lay);
  const cols = lay.cols.map((_, i) => `lc-${i}`);
  return dto.days.map((d, k) => {
    const rows = dto.shoot_days ? [row(el('tr', { class: 'rpt-day' }, td(dayText(d), '', n)), true)] : [];
    for (const l of d.lines) rows.push(...sceneRow(dto, l, lay, under).map((tr) => row(tr, true)));
    if (dto.day_breaks) rows.push(row(el('tr', { class: 'rpt-end' }, td(endText(d), '', n)), true));
    rows.push(row(el('tr', { class: 'bk-merged' }, el('td', { attrs: { colspan: String(n) } }, d.groups.length ? groupsBlock(d.groups) : el('span', { class: 'muted', text: 'No elements.' })))));
    if (k === dto.days.length - 1) rows.push(...totalRow(dto, n));
    return { name: `${dto.name}, day ${d.number}`, cls: 'bk-day', cols, head: headOf(lay, under), rows, intro: k === 0 ? intro(dto, ['Each element once a day; the number in brackets is how many of the day\'s scenes list it, and x3 how many one scene needs.']) : null, newPage: d.new_page };
  });
}

const KIND = { off: 'k-off', holiday: 'k-holiday', move: 'k-move', work: '' };

function legend(dto) {
  const part = dto.dood[0] && dto.dood[0].grid.codes;
  const codes = Array.isArray(part) && part.length ? part.map(([, labelText, code]) => `${code} ${labelText}`).join(', ') : '';
  return `${codes}${codes ? '. ' : ''}${LEGEND_START}`;
}

function dood(dto, landscape) {
  const per = DATES_PER_PAGE[landscape ? 'landscape' : 'portrait'];
  // The DOOD tab's Saturdays and Sundays switches decide whether each prints (DOD-19,
  // decisions 141 and 158); the dates already start on the week's first day.
  const weekends = loadWeekends();
  const flows = [];
  const notes = [legend(dto)];
  if (dto.dood[0] && dto.dood[0].grid.no_holds) notes.push('Worked out without holds: no element holds or drops.');
  for (const part of dto.dood) {
    const g = part.grid;
    // Rust starts the dates on the week's first day, so the first one says which it is.
    const pages = doodReportPages(g.dates, per, weekends, g.dates.length && g.dates[0].weekday === 'Mon' ? 'mon' : 'sun');
    for (const at of pages) {
      const dates = at.map((i) => g.dates[i]);
      const cols = ['g-id', 'g-name', ...dates.map(() => 'g-date')];
      const head = el(
        'tr',
        {},
        th('ID'),
        th('Element'),
        ...dates.map((d, i) =>
          el(
            'th',
            { class: `g-date ${KIND[d.kind] || ''}`, attrs: { scope: 'col' } },
            el('span', { text: i === 0 || d.day === 1 ? d.month : '' }),
            el('span', { class: 'g-day', text: String(d.day) }),
            el('span', { text: d.weekday.slice(0, 2) }),
            el('span', { text: d.shoot_day ? `D${d.shoot_day}` : '' }),
          ),
        ),
      );
      const rows = g.rows.map((r) => {
        const cells = at.map((i) => {
          const span = r.span && i >= r.span[0] && i <= r.span[1] ? ' span' : '';
          return td(r.cells[i], `g-cell ${KIND[g.dates[i].kind] || ''}${span}`);
        });
        return row(el('tr', {}, td(r.board_id), td(r.name, 'g-name'), ...cells));
      });
      const range = pages.length > 1 ? `, ${showDate(dates[0].date)} to ${showDate(dates[dates.length - 1].date)}` : '';
      const heading = el('h2', { class: 'rpt-seg-title', text: `${part.category}${range}` });
      const first = flows.length === 0;
      flows.push({ name: `${dto.name}: ${part.category}`, cls: 'rpt-dood', cols, head, rows, intro: first ? el('div', {}, intro(dto, notes), heading) : heading });
    }
  }
  // The summary: every row's counts as the engine gives them, by category.
  const cols = ['e-id', 'e-name', 'e-num', 'e-num', 'e-date', 'e-date', 'e-num'];
  const head = el('tr', {}, th('ID'), th('Element'), th('Work', 'num'), th('Hold', 'num'), th('Start'), th('Finish'), th('Total', 'num'));
  const rows = [];
  for (const part of dto.dood) {
    rows.push(row(el('tr', { class: 'rpt-cat' }, td(part.category, '', 7)), true));
    for (const r of part.grid.rows) rows.push(row(el('tr', {}, td(r.board_id), td(r.name), td(r.work, 'num'), td(r.hold, 'num'), td(showDate(r.start)), td(showDate(r.finish)), td(r.total, 'num'))));
  }
  flows.push({ name: `${dto.name}: summary`, cls: '', cols, head, rows, intro: el('h2', { class: 'rpt-seg-title', text: 'Summary' }) });
  return flows;
}

function elements(dto) {
  const cols = ['e-id', 'e-name', 'e-num', 'e-num', 'e-num', 'e-date', 'e-date'];
  const head = el('tr', {}, th('ID'), th('Element'), th('Sheets', 'num'), th('Pages', 'num'), th('Days', 'num'), th('Start'), th('Finish'));
  const rows = [];
  let cat = null;
  for (const e of dto.elements) {
    if (e.category !== cat) {
      cat = e.category;
      rows.push(row(el('tr', { class: 'rpt-cat' }, td(cat, '', 7)), true));
    }
    rows.push(row(el('tr', {}, td(e.board_id), td(e.name), td(e.sheets, 'num'), td(e.pages, 'num'), td(e.days, 'num'), td(showDate(e.start)), td(showDate(e.finish)))));
    // Custom property values, on a line of their own under the element.
    if (e.properties && e.properties.length) rows.push(row(el('tr', { class: 'rpt-props' }, td(''), td(e.properties.join('; '), '', 6))));
  }
  const note = dto.start ? [] : ['Days, start and finish need a start date on the board.'];
  return [{ name: dto.name, cls: '', cols, head, rows, intro: intro(dto, note) }];
}

/** The element detail: each element's heading and work dates, then its scenes with day and date. */
function elementDetail(dto) {
  const lay = columnsOf(dto, 'el-detail');
  const n = lay.cols.length + 1;
  const cols = ['d-when', ...lay.cols.map((_, i) => `lc-${i}`)];
  const head = headOf(lay, null);
  head.prepend(th('Day'));
  const rows = [];
  for (const d of dto.details) {
    const days = d.work_dates.length;
    rows.push(row(el('tr', { class: 'rpt-cat' }, td(`${d.name} (${d.category}), ${days === 1 ? '1 work day' : `${days} work days`}`, '', n)), true));
    if (d.work_dates.length) rows.push(row(el('tr', { class: 'rpt-sub-row' }, td(`Works ${d.work_dates.map(showDate).join(', ')}`, 'rpt-sub', n)), true));
    for (const b of d.scenes) {
      const when = b.day ? `${b.day}${b.date ? `, ${dayDate(b.weekday, b.date)}` : ''}` : 'Not scheduled';
      const lines = sceneRow(dto, b.line, lay, null);
      lines.forEach((r, i) => {
        r.prepend(td(i === 0 ? when : ''));
        rows.push(row(r, i < lines.length - 1));
      });
    }
  }
  return [{ name: dto.name, cls: 'el-detail', cols, head, rows, intro: intro(dto) }];
}

function totals(dto) {
  const t = dto.totals;
  const cols = ['t-day', 't-date', ...t.categories.map(() => 't-count')];
  const head = el('tr', {}, th('Day'), th('Date'), ...t.categories.map((c) => th(c, 'num')));
  const partial = (d, i) => Boolean(d.partial && d.partial[i]);
  const rows = t.days.map((d) => row(el('tr', {}, td(d.number), td(d.date ? `${dayDate(d.weekday, d.date)}` : ''), ...d.counts.map((n, i) => td(partial(d, i) ? `${n}*` : n, 'num')))));
  const notes = ['Each count is how many different elements of that category the day uses, or for a category that uses daily quantities, their quantities added up.'];
  if (t.days.some((d) => d.counts.some((_, i) => partial(d, i)))) notes.push('* A partial total: a quantity a sheet marks not to count daily is left out.');
  if (t.left_out === 1) notes.push('1 category no shoot day uses is left out.');
  else if (t.left_out > 1) notes.push(`${t.left_out} categories no shoot day uses are left out.`);
  return [{ name: dto.name, cls: '', cols, head, rows, intro: intro(dto, notes) }];
}

const DAY_KIND = { off: 'Off', holiday: 'Holiday', move: 'Travel day' };

/** The calendar: a page per month, the weeks as rows, each date with what it holds. */
function calendar(dto) {
  const n = dto.week.length;
  return dto.months.map((m, k) => {
    const cols = dto.week.map(() => 'cal-col');
    const head = el('tr', {}, ...dto.week.map((w) => th(w)));
    const rows = m.weeks.map((w) =>
      row(
        el(
          'tr',
          { class: 'cal-week' },
          ...w.map((d) =>
            el(
              'td',
              { class: `cal-cell k-${d.kind}${d.in_month ? '' : ' outside'}` },
              el('div', { class: 'cal-top' }, el('span', { class: 'cal-n', text: String(d.day) }), el('span', { class: 'cal-what', text: d.shoot_day ? `Day ${d.shoot_day}` : DAY_KIND[d.kind] || '' })),
              d.scenes.length ? el('div', { class: 'cal-line', text: `Sc ${d.scenes.join(', ')}` }) : null,
              d.shoot_day ? el('div', { class: 'cal-line muted', text: `${d.pages || '0'} pg, ${d.time || '0:00'}` }) : null,
              ...d.events.map((e) => el('div', { class: 'cal-line cal-ev', text: e })),
              dto.icons && d.flags.length ? el('div', { class: 'cal-line rpt-flag', text: `⚑ ${d.flags.length === 1 ? d.flags[0] : `${d.flags.length} red flags`}` }) : null,
            ),
          ),
        ),
      ),
    );
    const title = el('h2', { class: 'rpt-seg-title', text: m.title });
    return { name: `${dto.name}: ${m.title}`, cls: `rpt-cal cols-${n}`, cols, head, rows, intro: k === 0 ? el('div', {}, intro(dto, ['Each shoot day shows its scenes, pages and estimated length.']), title) : title, newPage: k > 0 };
  });
}

/**
 * The vertical one-line (behavior note 23): the stripboard on its side. Each scene is a
 * column with its text turned, each day break a dark column with the day's pages and
 * time; the left of every page holds the production info and a row per cast member in
 * board ID order, so there is no cover page.
 */
function vertical(dto, landscape) {
  const v = dto.vertical;
  const per = SCENES_PER_PAGE[landscape ? 'landscape' : 'portrait'];
  // Scenes and day breaks in order: a break after each day's last scene.
  const cols = [];
  for (const x of v.columns) {
    cols.push({ kind: 'scene', x });
    if (x.end) cols.push({ kind: 'break', x });
  }
  const info = (v.production || []).map(([k, val]) => el('div', { class: 'v-info-line' }, el('span', { class: 'v-info-label', text: `${k}: ` }), val));
  const flows = [];
  const chunks = Math.max(1, Math.ceil(cols.length / per));
  for (let c = 0; c < chunks; c++) {
    const part = cols.slice(c * per, (c + 1) * per);
    const turned = (text) => el('span', { class: 'v-turn', text });
    const sceneText = (x) => [x.scene ? `Sc ${x.scene}` : 'No scene number', x.int_ext, x.set, x.time_of_day, x.pages ? `${x.pages} pg` : ''].filter(Boolean).join('  ');
    // Three short turned lines, so the dark column fits them (live note 26: one long line was cut off).
    const breakText = (x) => [`End of day ${x.day}`, x.date ? dayDate(x.weekday, x.date) : '', `${x.end.pages || '0'} pg, ${x.end.time || '0:00'}`].filter(Boolean).join('\n');
    const head = el(
      'tr',
      { class: 'v-head' },
      el('th', { class: 'v-name v-info', attrs: { scope: 'col' } }, ...info.map((n) => (c === 0 ? n : n.cloneNode(true))), el('div', { class: 'v-info-cast', text: 'Cast' })),
      ...part.map((p) =>
        p.kind === 'break'
          ? el('th', { class: 'v-col v-break', attrs: { scope: 'col' } }, turned(breakText(p.x)))
          : el('th', { class: 'v-col', title: `${sceneText(p.x)}, day ${p.x.day}${p.x.date ? `, ${dayDate(p.x.weekday, p.x.date)}` : ''}`, attrs: { scope: 'col' } }, turned(sceneText(p.x))),
      ),
    );
    const cell = (p, r) => (p.kind === 'break' ? td('', 'v-col v-break') : td(p.x.rows.includes(r) ? v.cast[r][0] || '●' : '', 'v-col mid'));
    const rows = v.cast.map(([id, name], r) => row(el('tr', {}, td(id ? `${id} ${name}` : name, 'v-name'), ...part.map((p) => cell(p, r)))));
    if (!v.cast.length) rows.push(row(el('tr', { class: 'rpt-empty' }, td('No cast member is in these scenes.', 'v-name'), ...part.map((p) => (p.kind === 'break' ? td('', 'v-col v-break') : td('', 'v-col'))))));
    const range = chunks > 1 ? ` (${c + 1} of ${chunks})` : '';
    flows.push({ name: `${dto.name}${range}`, cls: 'rpt-vertical', cols: ['v-name', ...part.map((p) => (p.kind === 'break' ? 'v-col v-break' : 'v-col'))], head, rows, intro: c === 0 ? intro(dto, ['Each column is a scene in shoot order, each dark column the end of a day; a row per cast member, in board ID order.']) : null, newPage: c > 0 });
  }
  return flows;
}

/** The multi-unit one-line: each date, then each unit's day under its name. */
function multiUnit(dto) {
  const lay = columnsOf(dto, 'multi-unit');
  const n = lay.cols.length;
  const under = underOf(lay);
  const cols = lay.cols.map((_, i) => `lc-${i}`);
  const rows = [];
  for (const d of dto.units) {
    rows.push(row(el('tr', { class: 'rpt-day' }, td(`${dayDate(d.weekday, d.date)}`, '', n)), true));
    for (const u of d.units) {
      rows.push(row(el('tr', { class: 'rpt-cat' }, td(`${u.name}, day ${u.day}: ${u.pages} pages, ${u.time}`, '', n)), true));
      for (const l of u.lines) rows.push(...(l.kind === 'banner' ? [bannerRow(l, n)] : sceneRow(dto, l, lay, under)).map((tr) => row(tr, true)));
    }
  }
  const notes = dto.undated.length ? [`Left out, no start date: ${dto.undated.join(', ')}.`] : [];
  return [{ name: dto.name, cls: 'multi-unit', cols, head: headOf(lay, under), rows, intro: intro(dto, notes) }];
}

const scenesText = (sheets) => sheets.map((x) => x.scene).join(', ');

/** The conflict report: elements on one date in two or more sub-boards, events on work days, then event clashes apart. */
function conflictsReport(dto) {
  const c = dto.conflicts;
  const flows = [];
  const cols = ['r-date', 'r-name', 'r-cat', 'r-wide'];
  const head = el('tr', {}, th('Date'), th('Element'), th('Category'), th('Booked'));
  const rows = c.conflicts.map((x) => row(el('tr', {}, td(`${dayDate(x.weekday, x.date)}`), td(x.name), td(x.category), td(x.bookings.map((b) => `${b.sub_board_name} day ${b.day}: ${scenesText(b.sheets)}`).join('; ')))));
  if (!rows.length) rows.push(row(el('tr', { class: 'rpt-empty' }, td('No element is on one date in two sub-boards.', '', 4))));
  flows.push({ name: dto.name, cls: '', cols, head, rows, intro: intro(dto) });
  if (c.on_work.length) {
    const r2 = c.on_work.map((x) => row(el('tr', {}, td(`${dayDate(x.weekday, x.date)}`), td(x.name), td(x.category), td(`${x.event} on ${x.sub_board_name} day ${x.day}`))));
    flows.push({ name: `${dto.name}: events on work days`, cls: '', cols, head: head.cloneNode(true), rows: r2, intro: el('h2', { class: 'rpt-seg-title', text: 'Events on work days' }) });
  }
  if (c.event_clashes.length) {
    const r3 = c.event_clashes.map((x) => row(el('tr', {}, td(`${dayDate(x.weekday, x.date)}`), td(x.name), td(x.category), td([...x.events.map((e) => `${e.name} (${e.sub_board_name})`), ...x.bookings.map((b) => `${b.sub_board_name} day ${b.day}`)].join('; ')))));
    flows.push({ name: `${dto.name}: informational`, cls: '', cols, head: head.cloneNode(true), rows: r3, intro: el('h2', { class: 'rpt-seg-title', text: 'Informational, not conflicts' }) });
  }
  return flows;
}

/** The red flag report: the table of every flag (the Red flags tab's), then every clash on the sub-board, by date. */
function redFlagsReport(dto) {
  const r = dto.red_flags;
  const flows = [];
  const open = (d) => (d ? showDate(d) : 'Open');
  const tcols = ['r-date', 'r-date', 'r-cat', 'r-name', 'r-type', 'r-firm', 'r-wide'];
  const thead = el('tr', {}, th('Start'), th('End'), th('Category'), th('Element'), th('Type'), th('Hard'), th('Note'));
  const trows = (r.rows || []).map((x) => row(el('tr', {}, td(open(x.start)), td(open(x.end)), td(x.category_name), td(x.board_id ? `${x.board_id} ${x.name}` : x.name), td(x.type_name), td(x.hard ? 'Hard' : 'Soft'), td(x.note))));
  if (!trows.length) trows.push(row(el('tr', { class: 'rpt-empty' }, td('No red flag meets the report\u2019s filters.', '', 7))));
  flows.push({ name: `${dto.name}: every flag`, cls: '', cols: tcols, head: thead, rows: trows, intro: el('div', {}, intro(dto), el('h2', { class: 'rpt-seg-title', text: 'Every red flag' })) });
  const cols = ['r-date', 'r-name', 'r-type', 'r-firm', 'r-dates', 'r-wide'];
  const head = el('tr', {}, th('Date'), th('Element'), th('Type'), th('Hard'), th('Flag'), th('Where'));
  const rows = r.clashes.map((x) => {
    const where = x.day ? `Day ${x.day}: ${scenesText(x.sheets)}` : x.events.map((e) => e.name).join(', ');
    return row(el('tr', {}, td(`${dayDate(x.weekday, x.date)}`), td(`${x.name} (${x.category_name})`), td(x.type_name), td(x.hard ? 'Hard' : 'Soft'), td(x.note ? `${x.dates}. ${x.note}` : x.dates), td(where)));
  });
  if (!rows.length) rows.push(row(el('tr', { class: 'rpt-empty' }, td(`No red flag clashes on ${r.sub_board_name}.`, '', 6))));
  const notes = r.undated ? ['The sub-board has no start date, so only events were checked.'] : [];
  flows.push({ name: `${dto.name}: clashes`, cls: '', cols, head, rows, intro: el('div', {}, ...(notes.length ? [el('p', { class: 'rpt-note', text: notes[0] })] : []), el('h2', { class: 'rpt-seg-title', text: `Clashes on ${r.sub_board_name}` })), newPage: false });
  return flows;
}

/** The flows of report `dto` for js/report.js. */
export function flowsFor(dto, landscape) {
  if (dto.empty) return emptyFlow(dto);
  switch (dto.report) {
    case 'one_line':
      return oneLine(dto);
    case 'shooting':
      return shooting(dto);
    case 'breakdown':
      return breakdown(dto);
    case 'breakdown_day':
      return breakdownDay(dto);
    case 'dood':
      return dood(dto, landscape);
    case 'elements':
      return elements(dto);
    case 'element_detail':
      return elementDetail(dto);
    case 'calendar':
      return calendar(dto);
    case 'vertical':
      return vertical(dto, landscape);
    case 'multi_unit':
      return multiUnit(dto);
    case 'conflicts':
      return conflictsReport(dto);
    case 'red_flags':
      return redFlagsReport(dto);
    default:
      return totals(dto);
  }
}

function layoutOpts(dto, landscape) {
  // A header layout printed atop every page goes in each page's top band (js/report.js).
  const repeat = dto.head && dto.head.include && dto.head.every_page ? () => headBlock(dto.head) : null;
  return { title: dto.title || 'Untitled', place: `${dto.board}, ${dto.sub_board}`, landscape, size: dto.text, repeat };
}

/** The print job's name, and the PDF's suggested file name. */
export function jobTitle(dto) {
  return `${dto.title || 'Untitled'} - ${dto.name}`;
}

/** What a saved PDF shows, for its subject: the report, board and sub-board, date range. */
export function jobSubject(dto) {
  const place = [dto.board, dto.sub_board].filter(Boolean).join(', ');
  return [dto.name, place, dto.range].filter(Boolean).join('; ');
}

// ---------------------------------------------------------------------------
// Pickers (the simple designer): categories, elements, columns

function checkList(id, items, chosen) {
  return el(
    'div',
    { class: 'cal-pick rp-pick', attrs: { role: 'group' } },
    ...items.map(([value, text, group]) => {
      const box = el('input', { attrs: { id: `${id}-${value}`, type: 'checkbox', checked: chosen.has(value) } });
      box.addEventListener('change', () => (box.checked ? chosen.add(value) : chosen.delete(value)));
      return el('label', { class: 'el-check', attrs: { for: `${id}-${value}` }, title: group || null }, box, el('span', { text }));
    }),
  );
}

/** Picks a subset of `items` ([value, text, group]); empty means all. Resolves with the values or null. */
function pickDialog({ title, hint, items, picked, none = 'All' }) {
  return customDialog((close) => {
    const chosen = new Set(picked);
    const filter = el('input', { attrs: { type: 'text', autocomplete: 'off', placeholder: 'Filter', 'aria-label': 'Filter' } });
    const box = el('div', {});
    const draw = () => {
      const t = filter.value.trim().toLowerCase();
      box.replaceChildren(checkList('rp-pick', items.filter(([, text, group]) => !t || text.toLowerCase().includes(t) || (group || '').toLowerCase().includes(t)), chosen));
    };
    filter.addEventListener('input', draw);
    draw();
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: title, attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: hint }),
      el('div', { class: 'el-row' }, filter, el('button', { class: 'btn', text: none, attrs: { type: 'button' }, on: { click: () => close([]) } })),
      box,
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }), el('button', { class: 'btn btn-primary', text: 'Use These', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(items.map((x) => x[0]).filter((v) => chosen.has(v))) } })),
    );
  });
}

/** A report column as the page keeps it: the field with its line, span and look (decisions.md, entry 90). */
export function reportColumn(c) {
  const o = typeof c === 'string' ? { field: c } : c || {};
  return { field: o.field, line: Number(o.line) || 1, span: Number(o.span) || 1, align: o.align || 'auto', bold: Boolean(o.bold), italic: Boolean(o.italic), size: o.size || '' };
}

/**
 * The report designer: the columns a report shows, in order, each with Show, its line
 * (a field on lines 2 to 4 prints under the first line's columns, covering as many as
 * its span: a merged cell), alignment, bold, italic, its own text size, and Up and Down.
 * Default clears them.
 */
function columnsDialog({ fields, columns }) {
  return customDialog((close) => {
    const shown = columns.map(reportColumn);
    let rows = [...shown.map((c) => ({ ...c, on: true })), ...fields.map(([f]) => f).filter((f) => !shown.some((c) => c.field === f)).map((f) => ({ ...reportColumn(f), on: false }))];
    const list = el('ul', { class: 'lay-list' });
    const nameOf = (f) => (fields.find((x) => x[0] === f) || [f, f])[1];
    const pickOf = (aria, options, value, disabled, onChange) => {
      const s = el('select', { attrs: { 'aria-label': aria, disabled } }, ...options.map(([v, l]) => el('option', { text: l, attrs: { value: String(v) } })));
      s.value = String(value);
      s.addEventListener('change', () => onChange(s.value));
      return s;
    };
    const draw = () => {
      const firstCount = Math.max(1, rows.filter((r) => r.on && r.line === 1).length);
      list.replaceChildren(
        ...rows.map((r, i) => {
          const name = nameOf(r.field);
          const off = !r.on;
          const box = el('input', { attrs: { id: `rp-col-${r.field}`, type: 'checkbox', checked: r.on } });
          box.addEventListener('change', () => {
            r.on = box.checked;
            draw();
          });
          const move = (by) => {
            const j = i + by;
            if (j < 0 || j >= rows.length) return;
            [rows[i], rows[j]] = [rows[j], rows[i]];
            draw();
          };
          const line = pickOf(`Line of ${name}`, [1, 2, 3, 4].map((n) => [n, `Line ${n}`]), r.line, off, (v) => {
            r.line = Number(v);
            draw();
          });
          const spans = Array.from({ length: firstCount }, (_, n) => [n + 1, n === 0 ? '1 col' : `${n + 1} cols`]);
          const span = pickOf(`Columns ${name} covers`, spans, Math.min(r.span, firstCount), off || r.line === 1, (v) => (r.span = Number(v)));
          const align = pickOf(`Alignment of ${name}`, LAYOUT_ALIGNS, r.align, off, (v) => (r.align = v));
          const size = pickOf(`Text size of ${name}`, [['', 'Report size'], ...TEXT_SIZES], r.size, off, (v) => (r.size = v));
          const flag = (key, text, title) => {
            const b = el('input', { attrs: { type: 'checkbox', disabled: off, 'aria-label': `${title} ${name}`, checked: r[key] } });
            b.addEventListener('change', () => (r[key] = b.checked));
            return el('label', { class: `ly-flag ly-${key}`, title: `${title} ${name}` }, b, text);
          };
          return el(
            'li',
            { class: `lay-row rp-design${off ? ' off' : ''}` },
            el('label', { class: 'el-check', attrs: { for: `rp-col-${r.field}` } }, box, el('span', { text: name })),
            line,
            span,
            align,
            flag('bold', 'B', 'Bold'),
            flag('italic', 'I', 'Italic'),
            size,
            el('button', { class: 'btn', text: 'Up', attrs: { type: 'button', 'aria-label': `Move ${name} up` }, on: { click: () => move(-1) } }),
            el('button', { class: 'btn', text: 'Down', attrs: { type: 'button', 'aria-label': `Move ${name} down` }, on: { click: () => move(1) } }),
          );
        }),
      );
    };
    draw();
    const chosen = () => rows.filter((r) => r.on).map(({ field, line, span, align, bold, italic, size }) => ({ field, line, span: line === 1 ? 1 : span, align, bold, italic, size }));
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'Columns', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: 'The scene fields this report shows, in order. A field on line 2 to 4 prints under the first line\'s columns and can cover several (a merged cell). Default follows the board\'s strip layout (one-line reports) or the report\'s usual fields.' }),
      list,
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Default', attrs: { type: 'button' }, on: { click: () => close([]) } }),
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Use These', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(chosen()) } }),
      ),
    );
  });
}

/** Production Info: the title and every production field (behavior note 19), two to a row, labels beside them. */
function productionDialog(list, save) {
  return customDialog((close) => {
    const rows = [['title', 'Title', list.title], ...(list.production || [])];
    const fields = rows.map(([key, text, value]) => ({ key, value, input: el('input', { class: 'w-l', attrs: { id: `rp-prod-${key}`, type: 'text', autocomplete: 'off', value: value || '', maxlength: '200' } }), text }));
    const problem = errorLine();
    const ok = async () => {
      try {
        for (const f of fields) if (f.input.value.trim() !== (f.value || '')) await save(f.key, f.input.value);
        close(true);
      } catch (e) {
        problem.textContent = String(e);
        problem.hidden = false;
      }
    };
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'Production info', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: 'Saved in the schedule; a report header layout places any of these fields.' }),
      problem,
      el('div', { class: 'rp-form rp-prod' }, ...fields.map((f) => el('label', { class: 'bd-field beside', attrs: { for: `rp-prod-${f.key}` } }, el('span', { text: f.text }), f.input))),
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }), el('button', { class: 'btn btn-primary', text: 'Save', attrs: { type: 'button' }, on: { click: ok } })),
    );
  });
}

/**
 * Header Layouts (decisions.md, entry 99; behavior note 19): the schedule's named header
 * layouts. Pick one to edit its name, whether it prints, first page only, bold, italic,
 * alignment and text size, and up to three rows of fields, each added from the list (or
 * custom text), moved left or right and removed. Save keeps the one shown (one undo step);
 * New, Copy, Delete, Up and Down change the list. `getList()` is the latest report_list.
 */
function headersDialog(getList, actions, startId) {
  return customDialog((close) => {
    let current = startId;
    let draft = null;
    const problem = errorLine();
    const picker = el('select', { attrs: { id: 'hd-pick', 'aria-label': 'Header layout' } });
    const nameIn = el('input', { class: 'w-l', attrs: { id: 'hd-name', type: 'text', autocomplete: 'off', maxlength: '200' } });
    const box = (id, text) => {
      const b = el('input', { attrs: { id, type: 'checkbox' } });
      return { b, node: el('label', { class: 'el-check', attrs: { for: id } }, b, el('span', { text })) };
    };
    const include = box('hd-include', 'Include header');
    const first = box('hd-first', 'First page only');
    const bold = box('hd-bold', 'Bold');
    const italic = box('hd-italic', 'Italic');
    const sel = (id, label, opts) => el('select', { attrs: { id, 'aria-label': label } }, ...opts.map(([v, t]) => el('option', { text: t, attrs: { value: v } })));
    const align = sel('hd-align', 'Alignment', [['left', 'Left'], ['center', 'Center'], ['right', 'Right']]);
    const size = sel('hd-size', 'Text size', TEXT_SIZES);
    const rowsBox = el('div', { class: 'hd-rows' });
    const items = () => getList().header_items || [];
    const labelOfItem = (i) => (typeof i === 'string' ? (items().find((x) => x[0] === i) || [i, i])[1] : `"${i.text}"`);

    const load = () => {
      const list = getList().headers || [];
      const h = list.find((x) => x.id === current) || list[0];
      current = h ? h.id : null;
      draft = h ? JSON.parse(JSON.stringify(h)) : null;
      while (draft && draft.rows.length < 3) draft.rows.push([]);
      picker.replaceChildren(...list.map((x) => el('option', { text: x.name, attrs: { value: String(x.id) } })));
      picker.value = String(current);
      draw();
    };
    const draw = () => {
      if (!draft) return;
      nameIn.value = draft.name;
      include.b.checked = draft.include;
      first.b.checked = draft.first_page_only;
      bold.b.checked = draft.bold;
      italic.b.checked = draft.italic;
      align.value = draft.align;
      size.value = draft.size;
      rowsBox.replaceChildren(
        ...draft.rows.map((r, ri) => {
          const chips = r.map((it, k) => {
            const name = labelOfItem(it);
            const move = (by) => {
              const j = k + by;
              if (j < 0 || j >= r.length) return;
              [r[k], r[j]] = [r[j], r[k]];
              draw();
            };
            return el(
              'span',
              { class: 'hd-chip' },
              el('span', { text: name }),
              el('button', { class: 'btn btn-mini', text: '<', attrs: { type: 'button', 'aria-label': `Move ${name} left` }, on: { click: () => move(-1) } }),
              el('button', { class: 'btn btn-mini', text: '>', attrs: { type: 'button', 'aria-label': `Move ${name} right` }, on: { click: () => move(1) } }),
              el('button', { class: 'btn btn-mini', text: '×', attrs: { type: 'button', 'aria-label': `Remove ${name}` }, on: { click: () => { r.splice(k, 1); draw(); } } }),
            );
          });
          const add = el('select', { attrs: { 'aria-label': `Field to add to row ${ri + 1}` } }, el('option', { text: 'Add field…', attrs: { value: '' } }), el('option', { text: 'Custom text', attrs: { value: ':text' } }), ...items().map(([k, t]) => el('option', { text: t, attrs: { value: k } })));
          const text = el('input', { class: 'w-m', attrs: { type: 'text', maxlength: '100', placeholder: 'Custom text', 'aria-label': `Custom text for row ${ri + 1}`, hidden: true } });
          const addBtn = el('button', { class: 'btn', text: 'Add', attrs: { type: 'button', hidden: true } });
          add.addEventListener('change', () => {
            if (add.value === ':text') {
              text.hidden = false;
              addBtn.hidden = false;
              text.focus();
              return;
            }
            if (add.value) {
              r.push(add.value);
              draw();
            }
          });
          const addText = () => {
            if (!text.value.trim()) return;
            r.push({ text: text.value.trim() });
            draw();
          };
          addBtn.addEventListener('click', addText);
          text.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              addText();
            }
          });
          return el('div', { class: 'hd-row' }, el('span', { class: 'el-label', text: `Row ${ri + 1}` }), el('div', { class: 'hd-chips' }, ...chips, add, text, addBtn));
        }),
      );
    };
    const collect = () => ({ ...draft, name: nameIn.value, include: include.b.checked, first_page_only: first.b.checked, bold: bold.b.checked, italic: italic.b.checked, align: align.value, size: size.value, rows: draft.rows.filter((r, i) => r.length || draft.rows.slice(i + 1).some((x) => x.length)) });
    for (const [b, key] of [[include.b, 'include'], [first.b, 'first_page_only'], [bold.b, 'bold'], [italic.b, 'italic']]) b.addEventListener('change', () => (draft[key] = b.checked));
    align.addEventListener('change', () => (draft.align = align.value));
    size.addEventListener('change', () => (draft.size = size.value));
    nameIn.addEventListener('input', () => (draft.name = nameIn.value));
    const run = async (fn, next) => {
      try {
        await fn();
        problem.hidden = true;
        if (next !== undefined) current = typeof next === 'function' ? next() : next;
        load();
      } catch (e) {
        problem.textContent = String(e);
        problem.hidden = false;
      }
    };
    const lastId = () => Math.max(...(getList().headers || []).map((h) => h.id));
    const idx = () => (getList().headers || []).findIndex((h) => h.id === current);
    picker.addEventListener('change', () => {
      current = Number(picker.value);
      load();
    });
    const btn = (text, title, click) => el('button', { class: 'btn', text, attrs: { type: 'button', title }, on: { click } });
    load();
    return el(
      'div',
      { class: 'modal modal-wider', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'modal-title' } },
      el('h2', { class: 'modal-title', text: 'Header layouts', attrs: { id: 'modal-title' } }),
      el('p', { class: 'modal-message', text: 'Named report headers kept in the schedule. A report picks one in its options; a space pushes what follows to the far side of its row.' }),
      problem,
      el(
        'div',
        { class: 'rp-row hd-top' },
        el('label', { class: 'el-label', attrs: { for: 'hd-pick' }, text: 'Layout' }),
        picker,
        btn('New', 'A new header layout with the title only', () => run(() => actions.saveHeader({ id: 0, name: `Header ${(getList().headers || []).length + 1}`, include: true, first_page_only: true, bold: false, italic: false, align: 'left', size: 'normal', rows: [['title']] }), lastId)),
        btn('Copy', 'A copy of this layout', () => run(() => actions.saveHeader({ ...collect(), id: 0, name: `${draft.name} copy` }), lastId)),
        btn('Delete', 'Reports that picked it print with the first layout', () => run(() => actions.deleteHeader(current), null)),
        btn('Up', 'Move it up the list', () => idx() > 0 && run(() => actions.moveHeader(current, idx() - 1))),
        btn('Down', 'Move it down the list', () => run(() => actions.moveHeader(current, idx() + 1))),
      ),
      el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'hd-name' }, text: 'Name' }), nameIn),
      el('div', { class: 'rp-row hd-looks' }, include.node, first.node, bold.node, italic.node, el('label', { class: 'el-label', attrs: { for: 'hd-align' }, text: 'Align' }), align, el('label', { class: 'el-label', attrs: { for: 'hd-size' }, text: 'Size' }), size),
      rowsBox,
      el('div', { class: 'modal-actions' }, el('button', { class: 'btn', text: 'Close', attrs: { type: 'button' }, on: { click: () => close(null) } }), el('button', { class: 'btn btn-primary', text: 'Save Layout', attrs: { type: 'button', id: 'hd-save' }, on: { click: () => run(() => actions.saveHeader(collect())) } })),
    );
  });
}

// ---------------------------------------------------------------------------
// The tab

/**
 * Builds the tab in `host`. actions (each returns a promise): list() resolves with
 * report_list; show(index) loads a report's data and calls render; save(index, name,
 * options), duplicate(index), move(index, to), remove(index), production(field, value)
 * change the schedule; print(pdf), exportFile(kind), exportBudget(); say(text, isError).
 * Returns { render(dto), setList(list), current(), kind(), print(pdf), landscape() }.
 */
export function createReports(host, actions) {
  let dto = null;
  let list = null;
  let picked = 0;

  const items = el('ul', { class: 'el-cats rp-list', attrs: { 'aria-label': 'Reports' } });
  const up = el('button', { class: 'btn', text: 'Up', attrs: { type: 'button', title: 'Move the report up the list' }, on: { click: () => moveBy(-1) } });
  const down = el('button', { class: 'btn', text: 'Down', attrs: { type: 'button', title: 'Move the report down the list' }, on: { click: () => moveBy(1) } });
  const copy = el('button', { class: 'btn', text: 'Copy', attrs: { type: 'button', title: 'A named copy of this report with its options' }, on: { click: () => duplicate() } });
  const del = el('button', { class: 'btn', text: 'Delete', attrs: { type: 'button' }, on: { click: () => remove() } });

  // ---- Options
  const name = el('input', { class: 'rp-name', attrs: { id: 'rp-name', type: 'text', autocomplete: 'off', maxlength: '200' } });
  const from = dateInput({ class: 'w-s', id: 'rp-from', 'aria-label': 'From' });
  const to = dateInput({ class: 'w-s', id: 'rp-to', 'aria-label': 'To' });
  const sel = (id, opts) => el('select', { attrs: { id } }, ...opts.map(([v, t]) => el('option', { text: t, attrs: { value: v } })));
  const paper = sel('rp-orientation', [['portrait', 'Portrait'], ['landscape', 'Landscape']]);
  const header = sel('rp-header', []);
  const pageBreak = sel('rp-page-break', [['none', 'None'], ['day', 'Each day'], ['week', 'Each week']]);
  const stripLayout = sel('rp-layout', []);
  const size = sel('rp-text', [['small', 'Small'], ['normal', 'Normal'], ['large', 'Large']]);
  const switches = SWITCHES.map(([key, text]) => {
    const box = el('input', { attrs: { id: `rp-${key}`, type: 'checkbox' } });
    box.addEventListener('change', () => saveWith({ [key]: box.checked }));
    return { key, box, node: el('label', { class: 'el-check', attrs: { for: `rp-${key}` } }, box, el('span', { text })) };
  });
  const catBtn = el('button', { class: 'btn rp-picker', attrs: { type: 'button', id: 'rp-categories' }, on: { click: () => pickCategories() } });
  const elBtn = el('button', { class: 'btn rp-picker', attrs: { type: 'button', id: 'rp-elements' }, on: { click: () => pickElements() } });
  const colBtn = el('button', { class: 'btn rp-picker', attrs: { type: 'button', id: 'rp-columns' }, on: { click: () => pickColumns() } });
  const elColBtn = el('button', { class: 'btn rp-picker', attrs: { type: 'button', id: 'rp-element-columns' }, on: { click: () => pickElementColumns() } });
  const pickRow = (text, btn, key) => ({ key, node: el('div', { class: 'rp-row' }, el('span', { class: 'el-label', text }), btn) });
  const pickers = [
    pickRow('Categories', catBtn, 'categories'),
    pickRow('Elements', elBtn, 'elements'),
    { key: 'layout', node: el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'rp-layout' }, text: 'Layout' }), stripLayout) },
    pickRow('Columns', colBtn, 'columns'),
    pickRow('Category cols', elColBtn, 'element_columns'),
    { key: 'page_break', node: el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'rp-page-break' }, text: 'New page' }), pageBreak) },
  ];
  const rangeRow = el('div', { class: 'rp-row', data: { opt: 'range' } }, el('label', { class: 'el-label', attrs: { for: 'rp-from' }, text: 'From' }), from, el('label', { class: 'el-label', attrs: { for: 'rp-to' }, text: 'to' }), to);
  name.addEventListener('change', () => saveWith({}, name.value));
  for (const f of [from, to]) f.addEventListener('change', () => saveWith({ from: from.value, to: to.value }));
  paper.addEventListener('change', () => saveWith({ landscape: paper.value === 'landscape' }));
  header.addEventListener('change', () => saveWith({ header: Number(header.value) }));
  pageBreak.addEventListener('change', () => saveWith({ page_break: pageBreak.value }));
  stripLayout.addEventListener('change', () => saveWith({ layout: stripLayout.value ? Number(stripLayout.value) : null }));
  size.addEventListener('change', () => saveWith({ text: size.value }));

  const options = el(
    'section',
    { class: 'rp-options', attrs: { 'aria-label': 'Report options' } },
    el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'rp-name' }, text: 'Name' }), name),
    rangeRow,
    el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'rp-orientation' }, text: 'Paper' }), paper, el('label', { class: 'el-label', attrs: { for: 'rp-text' }, text: 'Text' }), size),
    el('div', { class: 'rp-row' }, el('label', { class: 'el-label', attrs: { for: 'rp-header' }, text: 'Header' }), header, el('button', { class: 'btn', text: 'Layouts…', attrs: { type: 'button', id: 'rp-headers', title: 'Design the header layouts' }, on: { click: () => list && headersDialog(() => list, actions, item() ? item().options.header : null) } })),
    el('div', { class: 'rp-switches' }, ...switches.map((s) => s.node)),
    ...pickers.map((p) => p.node),
  );
  const prodBtn = el('button', { class: 'btn', text: 'Production Info…', attrs: { type: 'button', id: 'rp-production' }, on: { click: () => list && productionDialog(list, actions.production) } });
  const printBtn = el('button', { class: 'btn btn-primary', text: 'Print…', attrs: { type: 'button', id: 'rp-print' }, on: { click: () => actions.print(false) } });
  const pdfBtn = el('button', { class: 'btn', text: 'Save as PDF…', attrs: { type: 'button', id: 'rp-pdf' }, on: { click: () => actions.print(true) } });
  const exportBtn = el('button', { class: 'btn', text: 'Export…', attrs: { type: 'button', id: 'rp-export' }, on: { click: () => actions.exportFile(kind()) } });
  const budgetBtn = el('button', { class: 'btn', text: 'Export for Budget…', attrs: { type: 'button', id: 'rp-budget' }, on: { click: () => actions.exportBudget() } });
  const count = el('p', { class: 'bd-hint', attrs: { id: 'rp-count' } });
  const error = errorLine();
  const side = el(
    'div',
    { class: 'el-side rp-side' },
    el('div', { class: 'rp-head' }, el('h2', { text: 'Reports' }), count),
    items,
    el('div', { class: 'rp-actions' }, up, down, copy, del),
    error,
    options,
    el('div', { class: 'rp-actions' }, prodBtn),
    el('div', { class: 'rp-actions' }, printBtn, pdfBtn),
    el('div', { class: 'rp-actions' }, exportBtn, budgetBtn),
  );
  const preview = el('div', { class: 'rp-preview rpt-preview', attrs: { id: 'rp-preview', tabindex: '0', 'aria-label': 'Print preview' } });
  host.replaceChildren(el('div', { class: 'rp-tab' }, side, preview));

  const item = () => (list ? list.reports[picked] || null : null);
  function kind() {
    const it = item();
    return it ? it.kind : 'one_line';
  }
  function landscape() {
    const it = item();
    return it ? it.landscape === true : false;
  }

  async function attempt(run) {
    try {
      await run();
      error.hidden = true;
      return true;
    } catch (e) {
      error.textContent = String(e);
      error.hidden = false;
      return false;
    }
  }

  /** Saves the picked report with `changes` to its options (and a new name). */
  function saveWith(changes, newName) {
    const it = item();
    if (!it) return Promise.resolve(false);
    const opts = { ...it.options, ...changes };
    return attempt(() => actions.save(picked, newName === undefined ? it.name : newName, opts)).then((ok) => {
      if (!ok) drawOptions();
      return ok;
    });
  }

  /** Runs a list edit with `next` picked, so the redraw after it shows that report; puts the old pick back if refused. */
  async function listEdit(next, run) {
    const was = picked;
    picked = next;
    if (!(await attempt(run))) {
      picked = was;
      drawList();
      drawOptions();
    }
  }

  function moveBy(by) {
    const from = picked;
    const to = from + by;
    if (!list || to < 0 || to >= list.reports.length) return;
    listEdit(to, () => actions.move(from, to));
  }

  function duplicate() {
    const from = picked;
    listEdit(from + 1, () => actions.duplicate(from));
  }

  async function remove() {
    const it = item();
    if (!it) return;
    const ok = await confirmDialog({ title: 'Delete this report?', message: `${it.name} leaves the report list. Undo brings it back.`, confirmText: 'Delete Report', danger: true });
    const from = picked;
    if (ok) listEdit(Math.max(0, from - 1), () => actions.remove(from));
  }

  async function pickCategories() {
    const it = item();
    if (!it) return;
    const out = await pickDialog({ title: 'Categories', hint: 'Only the categories ticked show in this report. All shows every one.', items: list.categories.map((c) => [c.id, c.name, '']), picked: it.options.categories });
    if (out) saveWith({ categories: out });
  }

  async function pickElements() {
    const it = item();
    if (!it) return;
    const out = await pickDialog({ title: 'Elements', hint: 'Only the elements ticked show in this report. All shows every one.', items: list.elements.map((e) => [e.id, e.board_id ? `${e.board_id} ${e.name}` : e.name, e.category]), picked: it.options.elements });
    if (out) saveWith({ elements: out });
  }

  async function pickElementColumns() {
    const it = item();
    if (!it) return;
    const out = await pickDialog({ title: 'Category columns', hint: 'A column for each category ticked, after the scene fields, listing each scene\'s elements of it. None adds no columns.', items: list.categories.map((c) => [c.id, c.name, '']), picked: it.options.element_columns || [], none: 'None' });
    if (out) saveWith({ element_columns: out });
  }

  async function pickColumns() {
    const it = item();
    if (!it) return;
    // With no columns of its own, the report shows its default ones: start from those.
    const shown = it.options.columns.length ? it.options.columns : dto && dto.index === picked && Array.isArray(dto.layout) ? dto.layout.map((c) => ({ ...c, size: c.size || '' })) : [];
    const out = await columnsDialog({ fields: list.fields, columns: shown });
    if (out) saveWith({ columns: out });
  }

  function drawList() {
    items.replaceChildren(
      ...list.reports.map((r) =>
        el(
          'li',
          {},
          el('button', {
            class: `el-cat${r.index === picked ? ' current' : ''}`,
            text: r.name,
            title: r.name === r.kind_label ? null : r.kind_label,
            attrs: { type: 'button', 'aria-pressed': r.index === picked ? 'true' : 'false' },
            data: { report: r.kind, index: r.index },
            on: { click: () => pick(r.index) },
          }),
        ),
      ),
    );
    up.disabled = picked <= 0;
    down.disabled = picked >= list.reports.length - 1;
    const it = item();
    del.disabled = !it || list.reports.filter((r) => r.kind === it.kind).length < 2;
    del.title = del.disabled ? 'The last report of its kind stays' : 'Delete this copy';
  }

  function drawOptions() {
    const it = item();
    if (!it) return;
    const o = it.options;
    const applies = new Set(APPLIES[it.kind] || []);
    if (document.activeElement !== name) name.value = it.name;
    if (document.activeElement !== from) from.value = showDate(o.from, true);
    if (document.activeElement !== to) to.value = showDate(o.to, true);
    rangeRow.hidden = !applies.has('range');
    paper.value = it.landscape ? 'landscape' : 'portrait';
    header.replaceChildren(...(list.headers || []).map((h) => el('option', { text: h.name, attrs: { value: String(h.id) } })));
    // A pick whose layout is gone prints with the first one (decision 99).
    header.value = (list.headers || []).some((h) => h.id === o.header) ? String(o.header) : String(((list.headers || [])[0] || {}).id);
    pageBreak.value = o.page_break || 'none';
    stripLayout.replaceChildren(el('option', { text: 'Own columns', attrs: { value: '' } }), ...(list.strip_layouts || []).map((l) => el('option', { text: l.name, attrs: { value: String(l.id) } })));
    stripLayout.value = o.layout === null || o.layout === undefined ? '' : String(o.layout);
    size.value = o.text;
    for (const s of switches) {
      s.box.checked = o[s.key] === true;
      s.node.hidden = !applies.has(s.key);
    }
    for (const p of pickers) p.node.hidden = !applies.has(p.key);
    const catNames = o.categories.map((id) => (list.categories.find((c) => c.id === id) || {}).name).filter(Boolean);
    catBtn.textContent = catNames.length ? `${catNames.length} of ${list.categories.length}…` : 'All…';
    catBtn.title = catNames.join(', ');
    elBtn.textContent = o.elements.length ? `${o.elements.length} picked…` : 'All…';
    colBtn.textContent = o.columns.length ? `${o.columns.length} fields…` : 'Default…';
    const ec = (o.element_columns || []).map((id) => (list.categories.find((c) => c.id === id) || {}).name).filter(Boolean);
    elColBtn.textContent = ec.length ? `${ec.length} picked…` : 'None…';
    elColBtn.title = ec.join(', ');
    colBtn.title = o.columns.map((c) => reportColumn(c).field).map((f) => (list.fields.find((x) => x[0] === f) || [f, f])[1]).join(', ');
    exportBtn.disabled = !EXPORTABLE[it.kind];
    exportBtn.title = EXPORTABLE[it.kind] ? 'Export this report as a spreadsheet (.xlsx) or CSV' : 'This report prints; File > Export has the spreadsheets';
  }

  function pick(index) {
    picked = index;
    drawList();
    drawOptions();
    actions.show(index);
  }

  function rescale() {
    const w = (preview.clientWidth || 800) - 32;
    scalePreview(w, landscape());
  }

  function drawPreview() {
    if (!dto) return;
    rescale();
    const flat = dto.landscape === true;
    const { pages } = layout(preview, flowsFor(dto, flat), layoutOpts(dto, flat));
    count.textContent = pages === 1 ? '1 page' : `${pages} pages`;
  }

  window.addEventListener('resize', () => {
    if (dto && !host.closest('[hidden]')) rescale();
  });

  return {
    /** The report list (report_list): keeps the picked place, within the list. */
    setList(next) {
      list = next;
      picked = Math.min(picked, Math.max(0, list.reports.length - 1));
      drawList();
      drawOptions();
    },
    render(next) {
      dto = next;
      drawPreview();
    },
    current: () => picked,
    kind,
    landscape,
    /** Prints the report shown (with `pdf`, into a PDF file). Resolves with { done, file }. */
    print(pdf) {
      if (!dto) return Promise.resolve({ done: false, file: null });
      const flat = dto.landscape === true;
      return printPages((root) => layout(root, flowsFor(dto, flat), layoutOpts(dto, flat)), { landscape: flat, pdf, title: jobTitle(dto), subject: jobSubject(dto) });
    },
    /** Picks the first report of `kind` (the Calendar tab's Print). */
    pickKind(k) {
      if (!list) return false;
      const r = list.reports.find((x) => x.kind === k);
      if (!r) return false;
      picked = r.index;
      drawList();
      drawOptions();
      return true;
    },
  };
}
