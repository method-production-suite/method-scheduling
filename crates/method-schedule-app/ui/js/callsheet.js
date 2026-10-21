// The printed call sheet (decision 207): one shoot day's call sheet data (docs/CALLSHEET.md,
// the `call_sheet_data` command) laid out into Letter pages by report.js, so a long day
// breaks between rows and every page has its header and "Page X of Y". The first page opens
// with the day: the production, the shoot day out of how many, the date, the call and wrap,
// the day's length and pages, sunrise and sunset. Then the scenes in shooting order with
// their clock times, the cast with when they are on set, the other elements by category,
// and the notes. Only what the schedule holds: no crew, weather or map (CALLSHEET.md, "Not
// included").
//
// Text goes in through textContent only (dom.js); no inline styles (print.css, .cs-*).

import { el } from './dom.js';
import { row, td, th } from './report.js';
import { callSheetDate, lengthText, spanText, sunText } from './logic.js';

const fact = (label, value) => el('div', { class: 'cs-fact' }, el('span', { class: 'cs-label', text: label }), el('span', { class: 'cs-value', text: value || 'Not set' }));

/** The first page's opening: who, which day, when. */
function opening(d) {
  const people = [
    ['Director', d.director],
    ['Producer', d.producer],
    ['Board', [d.board, d.sub_board].filter(Boolean).join(' · ')],
  ].filter(([, v]) => v);
  const date = callSheetDate(d.date);
  const sceneCount = (d.scenes || []).length;
  return el(
    'div',
    { class: 'cs-open' },
    el(
      'div',
      { class: 'cs-top' },
      el(
        'div',
        { class: 'cs-who' },
        el('div', { class: 'cs-title', text: d.title || 'Untitled' }),
        d.company ? el('div', { class: 'cs-company', text: d.company }) : null,
        people.length ? el('div', { class: 'cs-people' }, ...people.flatMap(([k, v]) => [el('span', { class: 'cs-label', text: k }), el('span', { text: v })])) : null,
      ),
      el(
        'div',
        { class: 'cs-day' },
        el('span', { class: 'cs-label', text: 'Shoot day' }),
        el('span', { class: 'cs-big' }, el('span', { text: String(d.day) }), el('span', { class: 'cs-of', text: ` of ${d.days}` })),
        el('span', { class: 'cs-date', text: date || 'No date set' }),
      ),
      el(
        'div',
        { class: 'cs-call' },
        el('span', { class: 'cs-label', text: 'Call' }),
        el('span', { class: 'cs-big', text: d.call || '--:--' }),
        el('span', { class: 'cs-wrap', text: d.wrap ? `Wrap about ${d.wrap}` : 'No call time set' }),
      ),
    ),
    el(
      'div',
      { class: 'cs-facts' },
      fact('Length', lengthText(d.length_minutes)),
      fact('Pages', d.pages),
      fact('Scenes', String(sceneCount)),
      fact('Sunrise · Sunset', sunText(d.sun)),
      fact('Sun at', d.sun && d.sun.location ? d.sun.location : ''),
    ),
    el('div', { class: 'rpt-seg-title', text: 'Scenes' }),
  );
}

const title = (text) => el('div', { class: 'rpt-seg-title', text });

/** The set, then the synopsis and the location under it. */
function setCell(s) {
  return el(
    'td',
    { class: 'cs-set' },
    el('strong', { text: s.set || '' }),
    s.synopsis ? el('span', { class: 'cs-sub', text: s.synopsis }) : null,
    s.location ? el('span', { class: 'cs-loc', text: s.location }) : null,
  );
}

/** The report.js flows for a call sheet. */
export function callSheetFlows(d) {
  const scenes = d.scenes || [];
  const cast = d.cast || [];
  const cats = (d.elements || []).filter((c) => (c.elements || []).length);
  const notes = d.notes || [];
  const advance = (d.advance || []).filter((a) => (a.scenes || []).length);
  const flows = [
    {
      // The page header's center: "Call sheet" where the sheet starts, the section after.
      name: 'Call sheet',
      cls: 'cs-scenes',
      cols: ['cs-c-time', 'cs-c-sc', 'cs-c-ie', 'cs-c-set', 'cs-c-dn', 'cs-c-pg', 'cs-c-cast'],
      head: el('tr', {}, th('Time'), th('Scene'), th('I/E'), th('Set and synopsis'), th('D/N'), th('Pages', 'num'), th('Cast')),
      intro: opening(d),
      rows: scenes.length
        ? scenes.map((s) => row(el('tr', {}, td(spanText(s.start, s.end), 'muted'), td(s.scenes, 'fb'), td(s.int_ext), setCell(s), td(s.time_of_day), td(s.pages, 'num'), td((s.cast || []).join(', ')))))
        : [row(el('tr', { class: 'rpt-empty' }, td('No scenes on this day.', '', 7)))],
    },
  ];
  if (cast.length) {
    flows.push({
      name: 'Cast',
      cls: 'cs-cast',
      cols: ['cs-c-id', 'cs-c-name', 'cs-c-scenes', 'cs-c-call', 'cs-c-on'],
      head: el('tr', {}, th('ID'), th('Name'), th('Scenes'), th('Call'), th('On set')),
      intro: title('Cast'),
      // Their call and wrap, bold when their own (entry 231).
      rows: cast.map((c) => row(el('tr', {}, td(c.board_id, 'fb'), td(c.name), td((c.scenes || []).join(', '), 'muted'), td(spanText(c.call, c.wrap), c.own_call ? 'fb' : ''), td(spanText(c.first, c.last))))),
    });
  }
  if (cats.length) {
    flows.push({
      name: 'Elements',
      cls: 'cs-elements',
      cols: ['cs-c-cat', 'cs-c-els'],
      head: null,
      intro: title('Elements'),
      rows: cats.map((c) =>
        row(
          el(
            'tr',
            {},
            td(c.name, 'cs-cat'),
            td(
              c.elements
                .map((e) => {
                  const name = e.board_id ? `${e.board_id}. ${e.name}` : e.name;
                  return (e.scenes || []).length ? `${name} (${e.scenes.join(', ')})` : name;
                })
                .join(' · '),
            ),
          ),
        ),
      ),
    });
  }
  if (notes.length) {
    flows.push({
      name: 'Notes',
      cls: 'cs-notes',
      cols: ['c-wide'],
      head: null,
      intro: title('Notes'),
      rows: notes.map((n) => row(el('tr', {}, td(n)))),
    });
  }
  // The next shoot days in brief (entry 232): a line per day, then its scenes.
  if (advance.length) {
    flows.push({
      name: 'Advance',
      cls: 'cs-advance',
      cols: ['cs-c-sc', 'cs-c-adv-ie', 'cs-c-adv-set', 'cs-c-dn', 'cs-c-pg', 'cs-c-cast'],
      head: el('tr', {}, th('Scene'), th('I/E'), th('Set'), th('D/N'), th('Pages', 'num'), th('Cast')),
      intro: title('Advance schedule'),
      rows: advance.flatMap((a) => [
        row(el('tr', {}, td([`Day ${a.day}`, callSheetDate(a.date), a.call ? `Call ${a.call}` : '', `${a.pages} pages`].filter(Boolean).join(' · '), 'cs-adv-day', 6))),
        ...a.scenes.map((s) => row(el('tr', {}, td(s.scenes, 'fb'), td(s.int_ext), td(s.set), td(s.time_of_day), td(s.pages, 'num'), td((s.cast || []).join(', '))))),
      ]),
    });
  }
  return flows;
}

/** report.js layout options: the title in the header band, the day in the footer. */
export function callSheetOpts(d) {
  const date = callSheetDate(d.date);
  return { title: d.title || 'Untitled', place: [`Call sheet, day ${d.day} of ${d.days}`, date].filter(Boolean).join(' · ') };
}

/** The print job's title, the PDF's default file name: "Harbor Light - Call Sheet Day 2". */
export function callSheetJob(d) {
  return `${d.title || 'Untitled'} - Call Sheet Day ${d.day}`;
}
