// Settings… (mod+,; decisions.md, entry 79): the user's display preferences in one
// place, kept in settings.json on this computer, never in the schedule (decision 59).
// The look (decision 208), the week start, the date format and Fountain lines per page
// (decision 195); the
// calendar's strip fields stay on the Calendar tab, beside what they change.

import { el } from './dom.js';
import { customDialog, errorLine } from './modal.js';
import { FOUNTAIN_LINES, THEMES, fountainLines } from './logic.js';

const WEEK = [
  ['sun', 'Sunday'],
  ['mon', 'Monday'],
];
const FORMATS = [
  ['iso', '2027-03-01'],
  ['us', '3/1/2027'],
  ['long', 'Mon 1 Mar 2027'],
];

/**
 * Shows the settings. `prefs` is the page's current preferences; `save(changes)` stores
 * them and rejects with a sentence. Resolves true when saved.
 */
export function settingsDialog({ prefs, save }) {
  return customDialog((close) => {
    const pick = (id, label, options, value) => {
      const s = el('select', { attrs: { id } }, ...options.map(([v, t]) => el('option', { text: t, attrs: { value: v } })));
      s.value = value;
      return [s, el('label', { class: 'field', attrs: { for: id } }, label, s)];
    };
    const [look, lookRow] = pick('set-theme', 'Look', THEMES.map(([v, name]) => [v, name]), prefs.theme || 'paper');
    const lookHint = el('p', { class: 'set-hint', text: '' });
    const hint = () => {
      const t = THEMES.find(([v]) => v === look.value);
      lookHint.textContent = t ? t[2] : '';
    };
    look.addEventListener('change', hint);
    hint();
    const [week, weekRow] = pick('set-week', 'Weeks start on', WEEK, prefs.week_start || 'sun');
    const [fmt, fmtRow] = pick('set-date', 'Dates show as', FORMATS, prefs.date_format || 'iso');
    const lines = el('input', { class: 'w-xs', attrs: { id: 'set-lines', type: 'text', inputmode: 'numeric', autocomplete: 'off', value: String(prefs.fountain_lines || FOUNTAIN_LINES.usual), title: `Lines on a page when a Fountain script is imported, ${FOUNTAIN_LINES.min} to ${FOUNTAIN_LINES.max}; at 56 an eighth is 7 lines. An .fdx or .mscenes file keeps its own lengths.` } });
    const linesRow = el('label', { class: 'field', attrs: { for: 'set-lines' } }, 'Fountain lines per page', lines);
    const error = errorLine();
    const go = async () => {
      const n = fountainLines(lines.value);
      if (n === null) {
        error.textContent = `Fountain lines per page are ${FOUNTAIN_LINES.min} to ${FOUNTAIN_LINES.max}.`;
        error.hidden = false;
        lines.focus();
        return;
      }
      try {
        await save({ theme: look.value, week_start: week.value, date_format: fmt.value, fountain_lines: n });
        close(true);
      } catch (e) {
        error.textContent = String(e);
        error.hidden = false;
      }
    };
    return el(
      'div',
      { class: 'modal', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'set-title' } },
      el('h2', { class: 'modal-title', text: 'Settings', attrs: { id: 'set-title' } }),
      el('p', { class: 'modal-message', text: 'For this computer only; the schedule file is not changed. Dates are still typed as 3/1/2027 or 2027-03-01.' }),
      error,
      el('div', { class: 'el-row' }, lookRow, lookHint),
      el('div', { class: 'el-row' }, weekRow, fmtRow, linesRow),
      el(
        'div',
        { class: 'modal-actions' },
        el('button', { class: 'btn', text: 'Cancel', attrs: { type: 'button' }, on: { click: () => close(null) } }),
        el('button', { class: 'btn btn-primary', text: 'Save', attrs: { type: 'button', 'data-autofocus': true }, on: { click: go } }),
      ),
    );
  });
}
