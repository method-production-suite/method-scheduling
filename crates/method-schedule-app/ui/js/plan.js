// The Plan menu (src/plan.rs; decisions.md, IMP-04 and BYD-01 to BYD-09): the features
// beyond the reference app, each a compact dialog or a command.
//
// - Import Script with Tags: an .fdx file's production tags as elements.
// - Import Script Revision: pick a new draft (in Rust), read the report, merge as one
//   undo step.
// - Day Timeline: each shoot day from its call time, with sunrise, sunset and warnings;
//   the call time and the long day limit are typed here.
// - Locations: latitude, longitude, UTC offset and daylight saving rule beside each sheet
//   location.
// - Turnarounds and Rest: a minimum per category and the rests shorter than it.
// - Schedule Locks: lock, see each actor's drift, delete.
// - Pin Strips, Unpin Strips: the selected scenes stay put in a suggested order.
// - Suggest Strip Order: before and after counts, Apply as one undo step.
// - Export Call Sheet Data: a day, JSON or CSV, a Save dialog in Rust.
//
// Rust reads every typed value. Schedule text goes in through textContent only; no
// inline styles (the CSP).

import { invoke } from './api.js';
import { el } from './dom.js';
import { showDate } from './dates.js';
import { customDialog, errorLine } from './modal.js';
import { layout, printPages } from './report.js';
import { callSheetFlows, callSheetJob, callSheetOpts } from './callsheet.js';

let uid = 0;
const nextId = (prefix) => `${prefix}-${++uid}`;

/** A label beside its field, the app's dense form row. */
function field(label, input) {
  const id = input.id || nextId('pl-f');
  input.id = id;
  return el('div', { class: 'pl-field' }, el('label', { text: label, attrs: { for: id } }), input);
}

const textInput = (value, cls = 'pl-short') => {
  const i = el('input', { class: cls, attrs: { type: 'text', spellcheck: 'false', autocomplete: 'off' } });
  i.value = value || '';
  return i;
};

const button = (text, onClick, cls = 'btn') => el('button', { class: cls, text, attrs: { type: 'button' }, on: { click: onClick } });

function closeButton(close) {
  return el('button', { class: 'btn btn-primary', text: 'Close', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(null) } });
}

function box(title, idBase, ...children) {
  const tid = nextId(idBase);
  return el(
    'div',
    { class: 'modal modal-wider pl-dialog', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': tid } },
    el('h2', { class: 'modal-title', text: title, attrs: { id: tid } }),
    ...children,
  );
}

function table(heads, rows) {
  return el(
    'table',
    { class: 'pl-table' },
    el('thead', {}, el('tr', {}, ...heads.map((h) => el('th', { text: h })))),
    el('tbody', {}, ...rows),
  );
}

/**
 * The Plan menu's commands for main.js. `ctx`: say(text, isError), apply(promise of a
 * view), show(view), board() (the board shown), sub() (the selection's sub-board, else
 * the one shown), selected() (the focused strip or null), scenes() (the selected scene
 * strips).
 */
export function planCommands(ctx) {
  /** Runs an edit inside a dialog: the view goes to the page, errors to `error`. */
  async function edit(pending, error) {
    try {
      const view = await pending;
      if (view) ctx.show(view);
      error.textContent = '';
      error.hidden = true;
      return true;
    } catch (e) {
      error.textContent = String(e);
      error.hidden = false;
      return false;
    }
  }

  async function reviseScript() {
    const tags = await customDialog((close) => {
      const check = el('input', { attrs: { type: 'checkbox', id: nextId('pl-tags') } });
      return box(
        'Import Script Revision',
        'pl-rev',
        el('p', { class: 'modal-message', text: 'Scenes are matched by number, then by heading, and keep their tags and places. New scenes go to the Boneyard; scenes the draft no longer has are marked omitted, not deleted. You see what changes before anything does.' }),
        el('label', { class: 'pl-check' }, check, ' Also read production tags (Final Draft files only)'),
        el('div', { class: 'modal-actions' }, button('Cancel', () => close(null)), el('button', { class: 'btn btn-primary', text: 'Choose Script', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(check.checked) } })),
      );
    });
    if (tags === null) return;
    let dto;
    try {
      dto = await invoke('revise_pick', { tags });
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    if (!dto) return;
    const go = await customDialog((close) =>
      box(
        'Merge This Revision?',
        'pl-revp',
        el('p', { class: 'modal-message', text: `${dto.file}: ${dto.matched} matched (${dto.changed} changed), ${dto.added} new, ${dto.omitted} omitted.` }),
        el('ul', { class: 'pl-lines' }, ...dto.lines.slice(1).map((l) => el('li', { text: l }))),
        el('div', { class: 'modal-actions' }, button('Cancel', () => close(null)), el('button', { class: 'btn btn-primary', text: 'Merge Revision', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(true) } })),
      ),
    );
    if (go) ctx.apply(invoke('revise_apply', { token: dto.token }));
  }

  async function timeline() {
    const sub = ctx.sub();
    let dto;
    try {
      dto = await invoke('timeline_data', { sub_board: sub, strip: ctx.selected() });
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    if (!dto.days.length) {
      ctx.say('This sub-board has no shoot days yet: add a day break first.');
      return;
    }
    let at = dto.focus;
    await customDialog((close, later) => {
      const body = el('div', { class: 'pl-body' });
      const error = errorLine('pl-error');
      const pick = el('select', { attrs: { id: nextId('pl-day') } });
      const call = textInput('');
      const longDay = textInput(dto.long_day);
      async function reload() {
        dto = await invoke('timeline_data', { sub_board: sub, strip: null });
        later(draw);
      }
      function draw() {
        const d = dto.days[Math.min(at, dto.days.length - 1)];
        pick.replaceChildren(...dto.days.map((x, i) => el('option', { text: `Day ${x.number}${x.date ? ` ${showDate(x.date)}` : ''}`, attrs: { value: String(i) } })));
        pick.value = String(at);
        call.value = d.call;
        longDay.value = dto.long_day;
        const head = [
          `Wrap ${d.wrap || 'needs a call time'}`,
          `${d.length} long`,
          d.sun || (d.date ? 'No location has coordinates, so sunrise and sunset are not known' : 'No date: set a start date for sunrise and sunset'),
        ].join(' · ');
        const rows = d.entries.map((e) =>
          el(
            'tr',
            { class: e.warnings.length ? 'pl-warn' : e.kind === 'banner' ? 'pl-banner' : '' },
            el('td', { class: 'n', text: e.start }),
            el('td', { class: 'n', text: e.end }),
            el('td', { text: e.text }),
            el('td', { text: e.untimed ? 'No shoot time' : e.warnings.join(' ') }),
          ),
        );
        // What runs past wrap, listed before the move; the move is one undo step (entry 192).
        const past = d.past_wrap
          ? el('p', { class: 'pl-head' }, d.past_wrap, ...(d.push.length ? [' ', button(`Move to Day ${d.next}`, () => push(d), 'btn btn-mini')] : []))
          : '';
        body.replaceChildren(
          el('p', { class: 'pl-head', text: head }),
          table(['Start', 'End', 'Strip', 'Note'], rows),
          d.warnings.length ? el('ul', { class: 'pl-lines pl-warnings' }, ...d.warnings.map((w) => el('li', { text: w }))) : '',
          past,
        );
      }
      pick.addEventListener('change', () => {
        at = Number(pick.value);
        draw();
      });
      const setCall = async () => {
        const d = dto.days[at];
        if (await edit(invoke('set_call_time', { day: d.day, call: call.value }), error)) await reload();
      };
      const push = async (d) => {
        if (await edit(invoke('push_past_wrap', { sub_board: sub, day: d.number, strips: d.push }), error)) await reload();
      };
      const setLong = async () => {
        if (await edit(invoke('set_long_day', { value: longDay.value }), error)) await reload();
      };
      call.addEventListener('keydown', (e) => e.key === 'Enter' && setCall());
      longDay.addEventListener('keydown', (e) => e.key === 'Enter' && setLong());
      draw();
      return box(
        `Day Timeline: ${dto.sub_board_name}`,
        'pl-tl',
        el(
          'div',
          { class: 'pl-bar' },
          field('Day', pick),
          field('Call', call),
          button('Set Call', setCall),
          field('Long day after', longDay),
          button('Set', setLong),
        ),
        error,
        body,
        el('div', { class: 'modal-actions' }, closeButton(close)),
      );
    });
  }

  async function locations() {
    let list;
    try {
      list = await invoke('locations_data');
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    if (!list.length) {
      ctx.say('No sheet has a location yet: type one on the Breakdown tab first.');
      return;
    }
    await customDialog((close, later) => {
      const error = errorLine('pl-error');
      const rows = list.map((l) => {
        const lat = textInput(l.latitude);
        const lon = textInput(l.longitude);
        const off = textInput(l.utc_offset);
        // One compact select for the daylight saving rule (entry 193).
        const dst = el(
          'select',
          { attrs: { 'aria-label': `Daylight saving at ${l.name}` } },
          el('option', { text: 'None', attrs: { value: '' } }),
          el('option', { text: 'US and Canada', attrs: { value: 'us_canada' } }),
          el('option', { text: 'EU', attrs: { value: 'eu' } }),
          el('option', { text: 'Australia (south-east)', attrs: { value: 'australia' } }),
          el('option', { text: 'New Zealand', attrs: { value: 'new_zealand' } }),
        );
        dst.value = l.daylight_saving;
        lat.setAttribute('aria-label', `Latitude of ${l.name}`);
        lon.setAttribute('aria-label', `Longitude of ${l.name}`);
        off.setAttribute('aria-label', `UTC offset of ${l.name}`);
        const set = async () => {
          // A refusal keeps what was typed, so the mistake can be fixed in place.
          if (!(await edit(invoke('set_location', { name: l.name, latitude: lat.value, longitude: lon.value, utc_offset: off.value, daylight_saving: dst.value }), error))) return;
          try {
            const back = (await invoke('locations_data')).find((x) => x.name === l.name);
            // As the engine reads them, once a click in progress is done (entry 170).
            if (back) {
              later(() => {
                lat.value = back.latitude;
                lon.value = back.longitude;
                off.value = back.utc_offset;
                dst.value = back.daylight_saving;
              });
            }
          } catch {
            // The row keeps what was typed.
          }
        };
        for (const i of [lat, lon, off]) i.addEventListener('keydown', (e) => e.key === 'Enter' && set());
        // A rule picked for a place already on the map is set at once.
        dst.addEventListener('change', () => lat.value.trim() && lon.value.trim() && set());
        return el('tr', {}, el('td', { text: l.name }), el('td', { class: 'n', text: String(l.sheets) }), el('td', {}, lat), el('td', {}, lon), el('td', {}, off), el('td', {}, dst), el('td', {}, button('Set', set, 'btn btn-mini')));
      });
      return box(
        'Locations',
        'pl-loc',
        el('p', { class: 'modal-message', text: 'Degrees as 34.0522 and -118.2437 (or 118.2437W); the UTC offset as -7 or +5:30 (empty: from the longitude): the standard one with a daylight saving rule, which adds an hour in summer, else the one on the shoot dates. Clear both degrees to take a location off the map.' }),
        error,
        table(['Location', 'Sheets', 'Latitude', 'Longitude', 'UTC offset', 'Daylight saving', ''], rows),
        el('div', { class: 'modal-actions' }, closeButton(close)),
      );
    });
  }

  async function rest() {
    const board = ctx.board();
    let dto;
    try {
      dto = await invoke('rest_data', { board });
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    await customDialog((close, later) => {
      const error = errorLine('pl-error');
      const report = el('div', { class: 'pl-body' });
      function drawReport() {
        const lines = dto.short.length ? dto.short : [dto.turnarounds.some((t) => t.minimum) ? 'Every rest meets its turnaround.' : 'No category has a turnaround yet.'];
        report.replaceChildren(
          el('ul', { class: `pl-lines${dto.short.length ? ' pl-warnings' : ''}` }, ...lines.map((l) => el('li', { text: l }))),
          dto.unchecked ? el('p', { class: 'modal-message', text: `${dto.unchecked} work days were not checked: they need a date and a call time.` }) : '',
        );
      }
      const rows = dto.turnarounds.map((t) => {
        const input = textInput(t.minimum);
        input.setAttribute('aria-label', `Turnaround for ${t.name}`);
        const set = async () => {
          if (await edit(invoke('set_turnaround', { category: t.category, value: input.value }), error)) {
            dto = await invoke('rest_data', { board });
            later(drawReport);
          }
        };
        input.addEventListener('keydown', (e) => e.key === 'Enter' && set());
        return el('tr', {}, el('td', { text: t.name }), el('td', {}, input), el('td', {}, button('Set', set, 'btn btn-mini')));
      });
      drawReport();
      return box(
        'Turnarounds and Rest',
        'pl-rest',
        el('p', { class: 'modal-message', text: 'The shortest rest from one day’s wrap to the next day’s call, as 12 or 10:30 (empty: none). Checked across the board from each day’s call time and wrap. Warnings only.' }),
        error,
        el('div', { class: 'pl-split' }, table(['Category', 'Minimum', ''], rows), report),
        el('div', { class: 'modal-actions' }, closeButton(close)),
      );
    });
  }

  async function locks() {
    const board = ctx.board();
    let list;
    try {
      list = await invoke('locks_data', { board });
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    await customDialog((close, later) => {
      const error = errorLine('pl-error');
      const listBox = el('div', {});
      const driftBox = el('div', { class: 'pl-body' });
      const name = textInput('', 'pl-name');
      async function refresh() {
        list = await invoke('locks_data', { board });
        later(draw);
      }
      async function showDrift(lockName) {
        try {
          const d = await invoke('drift_data', { name: lockName });
          error.textContent = '';
          error.hidden = true;
          const rows = d.actors.map((a) =>
            el(
              'tr',
              {},
              el('td', { text: a.board_id }),
              el('td', { text: a.name }),
              el('td', { class: 'n', text: `${a.before} → ${a.after}` }),
              el('td', { text: a.added }),
              el('td', { text: a.dropped }),
              el('td', { text: a.moved.join('; ') }),
            ),
          );
          // The drift as a spreadsheet (BYD-05), where a Save dialog says.
          const exportAs = async (format) => {
            try {
              const out = await invoke('export_drift', { name: lockName, format });
              if (out && out.done) ctx.say(`Exported ${out.file}.`);
            } catch (e) {
              error.textContent = String(e);
              error.hidden = false;
            }
          };
          driftBox.replaceChildren(
            el('h3', { class: 'pl-sub', text: `Drift since ${d.lock}` }),
            el('div', { class: 'pl-bar' }, button('Export Excel…', () => exportAs('xlsx'), 'btn btn-mini'), ' ', button('Export CSV…', () => exportAs('csv'), 'btn btn-mini')),
            rows.length ? table(['ID', 'Actor', 'Days', 'Added', 'Dropped', 'Moved'], rows) : el('p', { class: 'modal-message', text: 'No actor’s days changed since the lock.' }),
            d.unchanged && rows.length ? el('p', { class: 'modal-message', text: `${d.unchanged} more actors are unchanged.` }) : '',
          );
        } catch (e) {
          error.textContent = String(e);
          error.hidden = false;
        }
      }
      function draw() {
        const rows = list.map((l) =>
          el(
            'tr',
            {},
            el('td', { text: l.name }),
            el('td', { text: l.made }),
            el('td', { class: 'n', text: String(l.scenes) }),
            el('td', {}, button('Drift', () => showDrift(l.name), 'btn btn-mini'), ' ', button('Delete', async () => {
              if (await edit(invoke('delete_lock', { name: l.name }), error)) {
                driftBox.replaceChildren();
                await refresh();
              }
            }, 'btn btn-mini')),
          ),
        );
        listBox.replaceChildren(rows.length ? table(['Lock', 'Made', 'Scenes', ''], rows) : el('p', { class: 'modal-message', text: 'No locks yet.' }));
      }
      const lockNow = async () => {
        if (await edit(invoke('lock_schedule', { board, name: name.value }), error)) {
          name.value = '';
          await refresh();
        }
      };
      name.addEventListener('keydown', (e) => e.key === 'Enter' && lockNow());
      draw();
      return box(
        'Schedule Locks',
        'pl-lock',
        el('p', { class: 'modal-message', text: 'A lock keeps where every scheduled scene of this board is now. Drift shows, actor by actor, the work days added, dropped and moved since.' }),
        el('div', { class: 'pl-bar' }, field('Name', name), button('Lock', lockNow)),
        error,
        listBox,
        driftBox,
        el('div', { class: 'modal-actions' }, closeButton(close)),
      );
    });
  }

  function pin(pinned) {
    const strips = ctx.scenes();
    if (!strips.length) {
      ctx.say('Select one or more scene strips on the Board first.');
      return;
    }
    ctx.apply(invoke('pin_strips', { strips, pinned }));
  }

  async function suggestOrder() {
    const sub = ctx.sub();
    let dto;
    try {
      dto = await invoke('suggest_order', { sub_board: sub });
    } catch (e) {
      ctx.say(e, true);
      return;
    }
    const go = await customDialog((close) =>
      box(
        `Suggest Strip Order: ${dto.sub_board_name}`,
        'pl-sug',
        el('p', { class: 'modal-message', text: 'Scheduled scenes grouped by location, then set, then shared cast. Day breaks, banners and pinned strips stay where they are, and dates a scene’s element is red-flagged on are avoided where possible. Nothing changes until Apply; Undo takes it back.' }),
        el('p', { class: 'pl-head', text: `${dto.moved} scenes would move, ${dto.pinned} stay put (pinned, omitted or spanning days), ${dto.groups} location and set groups.` }),
        table(
          ['', 'Now', 'Suggested'],
          dto.rows.map((r) => el('tr', { class: r.after < r.before ? 'pl-better' : r.after > r.before ? 'pl-warn' : '' }, el('td', { text: r.label }), el('td', { class: 'n', text: String(r.before) }), el('td', { class: 'n', text: String(r.after) }))),
        ),
        el('div', { class: 'modal-actions' }, button('Cancel', () => close(null)), el('button', { class: 'btn btn-primary', text: 'Apply Order', attrs: { type: 'button', 'data-autofocus': true, disabled: dto.moved === 0 }, on: { click: () => close(true) } })),
      ),
    );
    if (go) ctx.apply(invoke('apply_suggested_order', { sub_board: sub, order: dto.order }));
  }

  /** The sub-board's shoot day count and the day to offer first (the selected strip's), or null. */
  async function dayChoice() {
    const sub = ctx.sub();
    let days;
    try {
      days = await invoke('shoot_day_count', { sub_board: sub });
    } catch (e) {
      ctx.say(e, true);
      return null;
    }
    if (!days) {
      ctx.say('This sub-board has no shoot days yet: add a day break first.');
      return null;
    }
    let first = 1;
    try {
      const t = await invoke('timeline_data', { sub_board: sub, strip: ctx.selected() });
      first = t.days[t.focus] ? t.days[t.focus].number : 1;
    } catch {
      // Day 1.
    }
    const pick = el('select', {}, ...Array.from({ length: days }, (_, i) => el('option', { text: `Day ${i + 1}`, attrs: { value: String(i + 1) } })));
    pick.value = String(first);
    return { sub, pick };
  }

  /** Print Call Sheet: a shoot day's call sheet laid out on Letter pages (decision 207). */
  async function printCallSheet() {
    const choice = await dayChoice();
    if (!choice) return;
    const { sub, pick } = choice;
    const how = await customDialog((close) =>
      box(
        'Print Call Sheet',
        'pl-pcs',
        el('p', { class: 'modal-message', text: 'The day, its call and wrap, sunrise and sunset, the scenes with their times, the cast with when they are on set, the other elements and the notes.' }),
        el('div', { class: 'pl-bar' }, field('Day', pick)),
        el(
          'div',
          { class: 'modal-actions' },
          button('Cancel', () => close(null)),
          button('Save as PDF…', () => close('pdf')),
          el('button', { class: 'btn btn-primary', text: 'Print…', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close('print') } }),
        ),
      ),
    );
    if (!how) return;
    try {
      const data = await invoke('call_sheet_data', { sub_board: sub, day: Number(pick.value) });
      const r = await printPages((root) => layout(root, callSheetFlows(data), callSheetOpts(data)), { pdf: how === 'pdf', title: callSheetJob(data), subject: 'Call sheet' });
      if (r && r.file) ctx.say(`Saved ${r.file}.`);
    } catch (e) {
      ctx.say(e, true);
    }
  }

  async function callSheet() {
    const choice = await dayChoice();
    if (!choice) return;
    const { sub, pick: pickDay } = choice;
    const format = el('select', {}, el('option', { text: 'JSON', attrs: { value: 'json' } }), el('option', { text: 'CSV', attrs: { value: 'csv' } }));
    const go = await customDialog((close) =>
      box(
        'Export Call Sheet Data',
        'pl-cs',
        el('p', { class: 'modal-message', text: 'The day’s scenes with clock times, the cast with board IDs, other elements by category, sunrise and sunset, and notes, for your own call sheet template.' }),
        el('div', { class: 'pl-bar' }, field('Day', pickDay), field('Format', format)),
        el('div', { class: 'modal-actions' }, button('Cancel', () => close(null)), el('button', { class: 'btn btn-primary', text: 'Export', attrs: { type: 'button', 'data-autofocus': true }, on: { click: () => close(true) } })),
      ),
    );
    if (!go) return;
    try {
      const r = await invoke('export_call_sheet', { sub_board: sub, day: Number(pickDay.value), format: format.value });
      if (r && r.done) ctx.say(`Wrote ${r.file}.`);
    } catch (e) {
      ctx.say(e, true);
    }
  }

  return {
    'plan.import_tags': () => ctx.apply(invoke('import_script_tags')),
    'plan.revise': () => reviseScript(),
    'plan.timeline': () => timeline(),
    'plan.print_call_sheet': () => printCallSheet(),
    'plan.call_sheet': () => callSheet(),
    'plan.locations': () => locations(),
    'plan.rest': () => rest(),
    'plan.locks': () => locks(),
    'plan.pin': () => pin(true),
    'plan.unpin': () => pin(false),
    'plan.suggest': () => suggestOrder(),
  };
}
