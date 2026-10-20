// File > Export: pick a report and a format, then Rust shows the Save dialog and writes the
// file (export_report, src/export.rs). File > Export for Budget writes the budget exchange
// file the same way (export_budget). The page names the report and the format, never a
// path.

import { el } from './dom.js';
import { customDialog } from './modal.js';
import { invoke } from './api.js';

const REPORTS = [
  ['dood', 'Day out of days summary', 'Every element the sub-board schedules: work, hold, start, finish and total.'],
  ['one_line', 'One-line schedule', 'As the report\'s strip layout: a row per strip in board order, day breaks with their totals.'],
  ['one_line_fixed', 'One-line, fixed columns', 'A row per scene or banner with its day and date, in the stable columns other tools read.'],
  ['elements', 'Elements', 'Every element with its sheets, pages, days, start and finish.'],
  ['dood_grid', 'Day out of days grid', 'Every element of every category with its code on each date, then the summary.'],
  ['element_detail', 'Element detail', 'A row per element and scene it is in, with the day and date.'],
  ['breakdown', 'Breakdown', 'A row per scene in board order, with a column per category of its elements.'],
  ['breakdown_day', 'Breakdown by day', 'A row per shoot day and element, with how many scenes list it and the day\'s quantity.'],
  ['board', 'Board', 'Every strip of the board: each sub-board with its day breaks, then the Boneyard.'],
  ['totals', 'Daily category totals', 'A row per shoot day with how many of each category it uses, then the totals.'],
  ['red_flags', 'Red flags', 'Every red flag in the schedule: its first and last dates, element, type, firmness and note.'],
  ['shot', 'Shot against planned', 'A row per shoot day with pages planned, shot and still to shoot, then a sheet with a row per scene.'],
];

const FORMATS = [
  ['xlsx', 'Spreadsheet (.xlsx)'],
  ['csv', 'CSV (.csv)'],
];

let last = { report: 'one_line', format: 'xlsx' };

function radios(name, options, value) {
  return options.map(([v, label, hint]) =>
    el(
      'label',
      { class: 'ex-option' },
      el('input', { attrs: { type: 'radio', name, value: v, checked: v === value } }),
      el('span', { class: 'ex-text' }, el('span', { text: label }), hint ? el('span', { class: 'ex-hint', text: hint }) : null),
    ),
  );
}

/** Asks which report and format. Resolves with { report, format }, or null when cancelled. */
function ask(report, where) {
  const start = { ...last, report: REPORTS.some((r) => r[0] === report) ? report : last.report };
  return customDialog((close) => {
    const form = el('form', { class: 'ex-form', attrs: { 'aria-labelledby': 'ex-title' } });
    const value = (name) => {
      const c = form.querySelector(`input[name="${name}"]:checked`);
      return c ? c.value : null;
    };
    form.append(
      el('fieldset', { class: 'ex-group' }, el('legend', { text: 'Report' }), ...radios('ex-report', REPORTS, start.report)),
      el('fieldset', { class: 'ex-group' }, el('legend', { text: 'Format' }), ...radios('ex-format', FORMATS, start.format)),
    );
    const done = () => close({ report: value('ex-report'), format: value('ex-format') });
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      done();
    });
    const cancel = el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } });
    const ok = el('button', { class: 'btn btn-primary', text: 'Export…', attrs: { type: 'button', id: 'ex-ok', 'data-autofocus': true }, on: { click: done } });
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'ex-title' } },
      el('h2', { class: 'modal-title', text: 'Export', attrs: { id: 'ex-title' } }),
      el('p', { class: 'modal-message', text: `From ${where}. Next, choose where to save it.` }),
      form,
      el('div', { class: 'modal-actions' }, cancel, ok),
    );
  });
}

/**
 * File > Export: asks for the report (starting at `report` when it can be exported) and
 * the format, then exports from sub-board `sub` (`where` names it, as "Board 1, Main").
 * `say(text, isError)` reports how it went.
 */
export async function exportReport(report, say, sub, where) {
  const choice = await ask(report, where);
  if (!choice || !choice.report || !choice.format) return;
  last = choice;
  try {
    const out = await invoke('export_report', { sub_board: sub, report: choice.report, format: choice.format });
    if (out && out.done) say(`Exported ${out.file}.`);
  } catch (e) {
    say(e, true);
  }
}

/** File > Export for Budget, from sub-board `sub`. */
export async function exportBudget(say, sub) {
  try {
    const out = await invoke('export_budget', { sub_board: sub });
    if (out && out.done) say(`Saved ${out.file} for the budget.`);
  } catch (e) {
    say(e, true);
  }
}
