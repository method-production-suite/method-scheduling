// Printing the board (decisions.md, entry 61): the board shown as a stripboard on
// Letter pages, through the same paging and print path as the reports (report.js,
// src/reports.rs): each sub-board's days with their strips in the strips' colors and the
// board's strip layout as columns, day breaks with their totals, then (if asked) the
// Boneyard. All strips, or only the selected ones. A strip's cells come from the same
// renderer the board uses (strips.js): its later lines print as rows of their own under
// it, a field covering several columns as a cell spanning them (entry 87).

import { el } from './dom.js';
import { dayDate } from './dates.js';
import { setRules } from './dynstyle.js';
import { customDialog } from './modal.js';
import { layout, printPages, row, td } from './report.js';
import { draftMark, layoutHead, printShares, sceneRows, stripGrid } from './strips.js';
import { colorClass, paintStrips } from './colors.js';
import { bannerLook, dayExtras, shootDayText } from './board.js';
import { dayLead } from './logic.js';


/** The options last chosen, kept for the next print. */
const last = { only: false, boneyard: false, landscape: false };

/**
 * The board's flows for report.js: one per sub-board (and the Boneyard when asked).
 * `picked` (a Set of strip ids) limits them to the selected strips when `only`.
 */
export function boardFlows(board, { only = false, picked = new Set(), boneyard = false, show = { heads: true, breaks: true, totals: true, banners: true } } = {}) {
  const grid = stripGrid(board.layout);
  const n = grid.cols.length;
  const shares = printShares(grid.cols);
  setRules('board-print', shares.map((w, i) => `.method-report col.bc-${i} { width: ${w}%; }`).join('\n'));
  const cols = grid.cols.map((_, i) => `bc-${i}`);
  const head = () => layoutHead(grid);
  const palette = new Map();
  const want = (id) => !only || picked.has(id);
  // What the board shows (board.js, boardShow) the printout shows too.
  const shownStrip = (st) => (show.banners || st.kind !== 'banner') && want(st.id);
  const size = board.layout_size === 'small' || board.layout_size === 'large' ? ` ts-${board.layout_size}` : '';

  // A strip's rows: a banner or a parked day break one, a scene its lines (kept together).
  const pushStrip = (rows, st, keep) => {
    if (st.kind === 'banner') {
      const cells = n > 1 ? [td(st.text, 'banner-text', n - 1), td(st.duration && st.duration !== '0:00' ? st.duration : '', 'num')] : [td(st.text, 'banner-text')];
      rows.push(row(el('tr', { class: `bp banner${bannerLook(st)}` }, ...cells), keep));
      return;
    }
    if (st.kind === 'break') {
      rows.push(row(el('tr', { class: 'bp day-break parked' }, td('Day break, parked', '', n)), keep));
      return;
    }
    const cls = `bp scene ${st.tone || ''}${colorClass(st, palette)}${st.omitted ? ' omitted' : ''}${size}`;
    const lines = sceneRows(grid, st.cells, cls, { lead: st.draft ? [draftMark(st)] : null, first: (t) => `${t}${st.parts > 1 ? ` (${st.part}/${st.parts})` : ''}${st.omitted ? ' (omitted)' : ''}` });
    lines.forEach((tr, i) => rows.push(row(tr, keep || i < lines.length - 1)));
  };
  const flows = [];
  for (const sb of board.sub_boards) {
    const rows = [];
    for (const d of sb.days) {
      const strips = d.strips.filter(shownStrip);
      const end = want(d.end.id);
      if (!strips.length && !end) continue;
      // The board's day look (decisions.md, entries 67 and 148): the shoot day strip heads
      // the day in place of the day heading, as on the board; extras go on the break.
      const look = board.day_look || {};
      const lead = dayLead(look, show);
      if (lead === 'shoot') rows.push(row(el('tr', { class: 'rpt-day bp-shoot' }, td(shootDayText(d, sb.days.length, board.shoot_text || ''), '', n)), true));
      else if (lead === 'day') rows.push(row(el('tr', { class: 'rpt-day' }, td(`Day ${d.number}${d.date ? `, ${dayDate(d.weekday, d.date)}` : ''}`, '', n)), true));
      strips.forEach((st) => pushStrip(rows, st, true));
      const scenes = d.scenes === 1 ? '1 scene' : `${d.scenes} scenes`;
      const totals = show.totals ? `: ${scenes}, ${d.pages || '0'} pages, ${d.time || '0:00'}` : '';
      const extras = dayExtras(d, look);
      if (show.breaks) rows.push(row(el('tr', { class: 'bp day-break' }, td(`End of day ${d.number}${totals}${extras ? ` · ${extras}` : ''}`, '', n))));
    }
    const tail = sb.unscheduled.filter(shownStrip);
    if (tail.length) {
      rows.push(row(el('tr', { class: 'rpt-day' }, td(sb.days.length ? 'After the last day break: not scheduled' : 'Not scheduled', '', n)), true));
      tail.forEach((st) => pushStrip(rows, st, false));
    }
    if (!rows.length) {
      if (only) continue;
      rows.push(row(el('tr', { class: 'rpt-empty' }, td('No strips on this sub-board.', '', n))));
    }
    flows.push({ name: `${board.name}: ${sb.name}`, cls: '', cols, head: head(), rows, intro: el('h2', { class: 'bp-title', text: `${sb.name}${sb.start ? `, starts ${sb.start}` : ''}` }) });
  }
  if (boneyard) {
    const yard = board.boneyard.filter(shownStrip);
    const yardRows = [];
    yard.forEach((st) => pushStrip(yardRows, st, false));
    if (yard.length) flows.push({ name: `${board.name}: Boneyard`, cls: '', cols, head: head(), rows: yardRows, intro: el('h2', { class: 'bp-title', text: 'Boneyard' }) });
  }
  // The user's strip colors, for the printed rows (colors.js checks every value).
  paintStrips(palette, board.colors, { scope: '#print-root', row: 'tr.bp', cells: true });
  return flows;
}

/**
 * Asks what to print (all strips or the selection, the Boneyard, the paper's way), then
 * prints the board or saves it as a PDF. Resolves with printPages' outcome, or null when
 * cancelled.
 */
export async function printBoard({ board, picked, title, pdf, show }) {
  const opts = await customDialog((close) => {
    const only = el('input', { attrs: { type: 'checkbox', checked: last.only && picked.size > 0, disabled: !picked.size, id: 'bp-only' } });
    const yard = el('input', { attrs: { type: 'checkbox', checked: last.boneyard, id: 'bp-yard' } });
    const way = el(
      'select',
      { attrs: { id: 'bp-way', 'aria-label': 'Paper' } },
      el('option', { text: 'Portrait', attrs: { value: 'portrait', selected: !last.landscape } }),
      el('option', { text: 'Landscape', attrs: { value: 'landscape', selected: last.landscape } }),
    );
    const go = () => close({ only: only.checked, boneyard: yard.checked, landscape: way.value === 'landscape' });
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'bp-head' } },
      el('h2', { class: 'modal-title', text: pdf ? 'Save the Board as PDF' : 'Print the Board', attrs: { id: 'bp-head' } }),
      el('p', { class: 'modal-message', text: `${board.name}: every sub-board's days and strips in the strips' colors, with the board's strip layout as columns.` }),
      el('label', { class: 'bd-check', attrs: { for: 'bp-only' } }, only, picked.size ? `Selected strips only (${picked.size})` : 'Selected strips only (none selected)'),
      el('label', { class: 'bd-check', attrs: { for: 'bp-yard' } }, yard, 'Include the Boneyard'),
      el('label', { class: 'bd-field beside', attrs: { for: 'bp-way' } }, el('span', { text: 'Paper' }), way),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: pdf ? 'Save as PDF…' : 'Print…', attrs: { type: 'button', 'data-autofocus': true }, on: { click: go } }),
      ),
    );
  });
  if (!opts) return null;
  Object.assign(last, opts);
  const flows = boardFlows(board, { only: opts.only, picked, boneyard: opts.boneyard, show });
  return printPages((root) => layout(root, flows, { title, place: board.name, landscape: opts.landscape }), {
    landscape: opts.landscape,
    pdf,
    title: `${title} - ${board.name}`,
    subject: `Strip board; ${board.name}`,
  });
}
