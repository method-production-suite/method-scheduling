// Stand-ins for the Plan menu's commands (src/plan.rs), so its dialogs can be drawn and
// clicked in a browser. The plan lives in the fake's model (`model().plan`), so each change
// is one undo step as in Rust; what people type is read by ports of the engine's readers
// (method_schedule_core::plan), with the same messages. `ctx`: { view(notice), edit(fn),
// model(), subOf(m, id), sceneStrips(m) } from fake-tauri.js.

const q = (t) => JSON.stringify(t);
/** As src/plan.rs `upper`: the first letter capitalized and a full stop at the end. */
const upper = (t) => {
  const s = String(t);
  if (!s) return s;
  const out = s[0].toUpperCase() + s.slice(1);
  return out.endsWith('.') ? out : `${out}.`;
};
const sameName = (a, b) => String(a).trim().toLowerCase() === String(b).trim().toLowerCase();

/** Degrees as typed, compass letters too; millionths (plan::parse_degrees). */
export function parseDegrees(text, max) {
  const t = String(text).trim();
  const bad = () => `${q(t)} is not a number of degrees such as 34.0522 or -118.2437`;
  const last = t.slice(-1).toUpperCase();
  let body = t;
  let sign = 1;
  if (((last === 'N' || last === 'S') && max === 90) || ((last === 'E' || last === 'W') && max === 180)) {
    body = t.slice(0, -1);
    sign = last === 'S' || last === 'W' ? -1 : 1;
  }
  body = body.trim();
  const neg = body.startsWith('-');
  const digits = neg ? body.slice(1) : body.startsWith('+') ? body.slice(1) : body;
  if (neg && sign < 0) throw bad();
  const dot = digits.indexOf('.');
  const whole = dot < 0 ? digits : digits.slice(0, dot);
  const frac = dot < 0 ? '' : digits.slice(dot + 1);
  if ((whole === '' && frac === '') || !/^\d*$/.test(whole) || !/^\d*$/.test(frac) || whole.length > 3) throw bad();
  if (frac.length > 6) throw `${q(t)} has more than six decimals`;
  const v = Number(whole || '0') * 1000000 + Number(frac.padEnd(6, '0'));
  if (v > max * 1000000) throw max === 90 ? `${q(t)} is past 90 degrees` : `${q(t)} is past 180 degrees`;
  return neg || sign < 0 ? -v : v;
}

/** Millionths of a degree as text (plan::degrees_text). */
export function degreesText(v) {
  const sign = v < 0 ? '-' : '';
  const a = Math.abs(v);
  const frac = String(a % 1000000).padStart(6, '0').replace(/0+$/, '');
  return frac ? `${sign}${Math.floor(a / 1000000)}.${frac}` : `${sign}${Math.floor(a / 1000000)}`;
}

/** A UTC offset as typed; minutes (plan::parse_offset). */
export function parseOffset(text) {
  const t = String(text).trim();
  const bad = () => `${q(t)} is not a UTC offset such as -8, +5:30 or -08:00`;
  let body = t;
  for (const p of ['UTC', 'utc', 'GMT']) {
    if (body.startsWith(p)) {
      body = body.slice(p.length);
      break;
    }
  }
  body = body.trim();
  if (!body) return 0;
  const neg = body[0] === '-';
  const rest = body[0] === '-' || body[0] === '+' ? body.slice(1) : body;
  const colon = rest.indexOf(':');
  const h = colon < 0 ? rest : rest.slice(0, colon);
  const m = colon < 0 ? '0' : rest.slice(colon + 1);
  const ok = (s) => /^\d{1,2}$/.test(s);
  if (!ok(h) || !ok(m)) throw bad();
  const v = Number(h) * 60 + Number(m);
  if (Number(m) >= 60 || v > 14 * 60) throw bad();
  return neg ? -v : v;
}

/** Minutes ahead of UTC as text, `-04:00` (plan::offset_text). */
export function offsetText(minutes) {
  const a = Math.abs(minutes);
  return `${minutes < 0 ? '-' : '+'}${String(Math.floor(a / 60)).padStart(2, '0')}:${String(a % 60).padStart(2, '0')}`;
}

/** A clock time as typed; minutes after midnight (plan::parse_clock). */
function parseClock(text) {
  const t = String(text).trim();
  const bad = () => `${q(t)} is not a time such as 6:30 or 18:45`;
  const m = /^(\d{1,2}):(\d{2})$/.exec(t);
  if (!m || Number(m[1]) > 23 || Number(m[2]) > 59) throw bad();
  return Number(m[1]) * 60 + Number(m[2]);
}

const clockText = (minutes) => {
  const m = ((minutes % 1440) + 1440) % 1440;
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
};

/** A length of time as typed, `12` or `10:30`; minutes (plan::parse_hours). */
export function parseHours(text) {
  const t = String(text).trim();
  const bad = () => `${q(t)} is not a length of time such as 12 or 10:30`;
  const colon = t.indexOf(':');
  const h = colon < 0 ? t : t.slice(0, colon);
  const m = colon < 0 ? '00' : t.slice(colon + 1);
  if (!/^\d{1,3}$/.test(h) || !/^\d{2}$/.test(m) || Number(m) > 59) throw bad();
  return Number(h) * 60 + Number(m);
}

/** Minutes as `h:mm` (plan::hours_text). */
export const hoursText = (minutes) => `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}`;

const DEFAULT_LONG_DAY = 12 * 60;

/** A daylight saving rule as typed; its name or null (plan::parse_daylight_saving). */
export function parseDaylightSaving(text) {
  const t = String(text ?? '').trim();
  if (!t || t.toLowerCase() === 'none') return null;
  const name = ['us_canada', 'eu', 'australia', 'new_zealand', 'israel', 'egypt', 'chile'].find((r) => r === t.toLowerCase());
  if (!name) throw `${q(t)} is not a daylight saving rule: none, us_canada, eu, australia, new_zealand, israel, egypt or chile`;
  return name;
}

/** As src/plan.rs place_location: both degrees or neither, an empty offset from the longitude. */
export function placeLocation(plan, name, latitude, longitude, utcOffset, daylightSaving = '') {
  const lat = String(latitude).trim();
  const lon = String(longitude).trim();
  if (!lat && !lon) {
    plan.locations = plan.locations.filter((l) => !sameName(l.name, name));
    return;
  }
  if (!lat || !lon) throw 'Type both a latitude and a longitude, or neither.';
  let la;
  let lo;
  let offset;
  let dst;
  try {
    la = parseDegrees(lat, 90);
    lo = parseDegrees(lon, 180);
    // The longitude's hour, as Rust's integer division has it (toward zero).
    offset = String(utcOffset).trim() ? parseOffset(utcOffset) : Math.trunc(Math.trunc((lo * 4) / 1000000) / 60) * 60;
    dst = parseDaylightSaving(daylightSaving);
  } catch (e) {
    throw upper(e);
  }
  const n = String(name).trim();
  if (!n) throw "A name can't be empty.";
  const loc = { name: n, latitude: la, longitude: lo, utc_offset: offset, daylight_saving: dst };
  const at = plan.locations.findIndex((l) => sameName(l.name, n));
  if (at < 0) plan.locations.push(loc);
  else plan.locations[at] = loc;
}

/** The names a sheet's Location field holds in the fake, standing in for the sheets'. */
const SHEET_LOCATIONS = [
  ['Harbor', 3],
  ['Net loft', 2],
  ['Boatyard', 1],
];

export function planFakes(ctx) {
  const { view, edit, model, subOf, sceneStrips } = ctx;
  const planOf = (m) =>
    (m.plan = m.plan || {
      calls: {},
      longDay: null,
      locations: [{ name: 'Harbor', latitude: 41500000, longitude: -70600000, utc_offset: -240 }],
      turnarounds: {},
      locks: [],
      pinned: [],
    });
  const longDayOf = (m) => planOf(m).longDay ?? DEFAULT_LONG_DAY;
  /** An edit with the engine's message, as Rust's `map_err(upper)`. */
  const planEdit = (fn, notice = null) => {
    const v = edit((m) => fn(m, planOf(m)));
    v.notice = notice;
    return v;
  };
  const read = (fn) => {
    try {
      return fn();
    } catch (e) {
      throw upper(e);
    }
  };

  // The pending revision (src/plan.rs PENDING): one at a time, gone once applied.
  let nextToken = 1;
  let pending = null;

  /** The timeline of a sub-board from the fake's own days: an hour a strip from the call. */
  function timeline(sub, strip) {
    const m = model();
    subOf(m, sub);
    const sb = view().board.sub_boards.find((x) => x.id === sub);
    const plan = planOf(m);
    const limit = longDayOf(m);
    const harbor = plan.locations.find((l) => sameName(l.name, 'Harbor'));
    let focus = 0;
    const days = sb.days.map((d, i) => {
      const id = d.end.id;
      const call = plan.calls[id] ?? null;
      const at = (mins) => (call === null ? `+${hoursText(mins)}` : clockText(call + mins));
      let t = 0;
      const entries = d.strips
        .filter((x) => x.kind === 'scene' || x.kind === 'banner')
        .map((x) => {
          const e = { strip: x.id, kind: x.kind, text: x.kind === 'scene' ? `Sc ${x.scene} ${x.int_ext} ${x.set} ${x.time_of_day}` : x.text, start: at(t), end: at(t + 60), untimed: false, warnings: [] };
          t += 60;
          return e;
        });
      if (strip !== null && strip !== undefined && (id === strip || entries.some((e) => e.strip === strip))) focus = i;
      // As timeline::past_wrap: from the first strip after the day's first that ends past
      // the call plus the long day limit (decision 192).
      const from = entries.findIndex((e, k) => k > 0 && (k + 1) * 60 > limit);
      const moving = from < 0 ? [] : entries.slice(from);
      const next = i + 1 < sb.days.length ? sb.days[i + 1].number : null;
      const by = call === null ? `+${hoursText(limit)}` : clockText(call + limit);
      const what = moving.length === 1 ? '1 strip runs' : `${moving.length} strips run`;
      const names = moving.map((e) => (e.kind === 'scene' ? e.text.split(' ').slice(0, 2).join(' ') : e.text)).join(', ');
      const pastWrap = !moving.length
        ? ''
        : next === null
          ? `${what} past ${by} on day ${d.number}, the last shoot day: add a day break after it first.`
          : `${what} past ${by} on day ${d.number}: ${names}. They would move to the start of day ${next}.`;
      return {
        number: d.number,
        date: d.date,
        day: id,
        call: call === null ? '' : clockText(call),
        wrap: call === null ? '' : clockText(call + t),
        length: hoursText(t),
        sun: i === 0 && harbor ? 'Sunrise 06:13, sunset 17:41 at Harbor' : '',
        entries,
        warnings: t > limit ? [`Day ${d.number} runs ${hoursText(t)}, past the ${hoursText(limit)} long day limit.`] : [],
        past_wrap: pastWrap,
        push: next === null ? [] : moving.map((e) => e.strip),
        next,
        // As plan.rs people (decision 231): fixed cast, with their own times where set.
        people: [
          [1, 'KIT'],
          [4, 'HARBORMASTER LUND'],
        ].map(([element, name]) => {
          const own = (plan.own || {})[`${element}:${id}`] || {};
          return { element, name, call: own.call ?? '', wrap: own.wrap ?? '' };
        }),
      };
    });
    return { sub_board: sub, sub_board_name: sb.name, long_day: hoursText(limit), days, focus };
  }

  /**
   * The fake's suggestion: scenes not pinned regrouped by set, every other strip in place.
   * Stands in for optimize::suggest; Apply refuses an order that is no longer it.
   */
  function suggestion(m, sub) {
    const sb = subOf(m, sub);
    const pins = new Set(planOf(m).pinned);
    const free = (s) => s.kind === 'scene' && !pins.has(s.id);
    const movable = sb.strips.filter(free);
    const sorted = [...movable].sort((a, b) => a.set.localeCompare(b.set));
    let k = 0;
    const order = sb.strips.map((s) => (free(s) ? sorted[k++] : s));
    return {
      order: order.map((s) => s.id),
      moved: order.filter((s, i) => s.id !== sb.strips[i].id).length,
      pinned: sb.strips.filter((s) => pins.has(s.id)).length,
      groups: new Set(movable.map((s) => s.set)).size,
      name: sb.name,
    };
  }

  return {
    import_script_tags: () => view('Added 5 scenes to the Boneyard, with 7 production tags.'),
    revise_pick: ({ tags }) => {
      nextToken += 1;
      pending = nextToken;
      return {
        token: pending,
        file: tags ? 'harbor-light-blue.fdx' : 'harbor-light-blue.fountain',
        matched: 4,
        changed: 2,
        added: 1,
        omitted: 1,
        lines: ['4 scenes matched (2 changed), 1 new, 1 omitted.', 'Changed Scene 2A, EXT. PIER (matched by heading): scenes "2" -> "2A"', 'New Scene 6, INT. ATTIC in the Boneyard', 'Omitted Scene 3A, INT. CAR (kept, marked omitted)'],
      };
    },
    revise_apply: ({ token }) => {
      // Taken whatever the token: a stale one leaves nothing waiting.
      const was = pending;
      pending = null;
      if (was === null || was !== token) throw 'That revision is no longer waiting; pick the script again.';
      return view('Revision merged: 4 scenes matched (2 changed), 1 new, 1 omitted. Undo takes it back.');
    },
    timeline_data: ({ sub_board, strip }) => timeline(sub_board, strip),
    set_call_time: ({ day, call }) => {
      const t = String(call).trim();
      const minutes = t ? read(() => parseClock(t)) : null;
      return planEdit((m, plan) => {
        if (minutes === null) delete plan.calls[day];
        else plan.calls[day] = minutes;
      });
    },
    // As plan.rs set_own_call (decision 231): kept as typed once read.
    set_person_call: ({ day, element, call, wrap }) => {
      if (typeof day !== 'number' || typeof element !== 'number') throw 'fake: set_person_call needs day and element';
      const c = String(call).trim();
      const w = String(wrap).trim();
      if (c) read(() => parseClock(c));
      if (w) read(() => parseClock(w));
      return planEdit((m, plan) => {
        plan.own = plan.own || {};
        if (!c && !w) delete plan.own[`${element}:${day}`];
        else plan.own[`${element}:${day}`] = { call: c && clockText(parseClock(c)), wrap: w && clockText(parseClock(w)) };
      });
    },
    // As plan::record_shot (M5): kept for the page only; the fake's board doesn't mark it.
    set_shot: ({ sheet, day, eighths, complete }) => {
      if (typeof sheet !== 'number' || typeof day !== 'number' || typeof complete !== 'boolean') throw 'fake: set_shot needs sheet, day and complete';
      return planEdit((m, plan) => {
        plan.shot = plan.shot || {};
        plan.shot[`${sheet}:${day}`] = { eighths: String(eighths), complete };
      });
    },
    // As timeline::push_past_wrap: the previewed strips, whole and in order, go just after
    // the day's break; refused when they are no longer what would move.
    push_past_wrap: ({ sub_board, day, strips }) => {
      const t = timeline(sub_board, null);
      const d = t.days.find((x) => x.number === day);
      if (!d) throw `There is no shoot day ${day}.`;
      if (d.push.length !== strips.length || d.push.some((id, i) => id !== strips[i])) throw 'The day changed since the preview; look at what runs past wrap again.';
      if (!d.push.length) throw d.past_wrap || `Nothing on day ${day} runs past the wrap.`;
      const what = strips.length === 1 ? '1 strip' : `${strips.length} strips`;
      return planEdit((m) => {
        const sb = subOf(m, sub_board);
        const moving = new Set(strips);
        const kept = sb.strips.filter((x) => !moving.has(x.id));
        const at = kept.findIndex((x) => x.id === d.day) + 1;
        kept.splice(at, 0, ...strips.map((id) => sb.strips.find((x) => x.id === id)));
        sb.strips = kept;
      }, `Moved ${what} to the start of day ${day + 1}. Undo puts them back.`);
    },
    set_long_day: ({ value }) => {
      const t = String(value).trim();
      const minutes = t ? read(() => parseHours(t)) : null;
      if (minutes !== null && (minutes === 0 || minutes > 24 * 60)) throw 'A long day limit is more than 0:00 and 24:00 at most.';
      return planEdit((m, plan) => {
        plan.longDay = minutes === DEFAULT_LONG_DAY ? null : minutes;
      });
    },
    locations_data: () => {
      const plan = planOf(model());
      const names = SHEET_LOCATIONS.map(([n]) => n);
      for (const l of plan.locations) if (!names.some((x) => sameName(x, l.name))) names.push(l.name);
      return names.map((name) => {
        const g = plan.locations.find((l) => sameName(l.name, name));
        const s = SHEET_LOCATIONS.find(([n]) => sameName(n, name));
        return {
          name,
          sheets: s ? s[1] : 0,
          latitude: g ? degreesText(g.latitude) : '',
          longitude: g ? degreesText(g.longitude) : '',
          utc_offset: g ? offsetText(g.utc_offset) : '',
          daylight_saving: g && g.daylight_saving ? g.daylight_saving : '',
        };
      });
    },
    set_location: ({ name, latitude, longitude, utc_offset, daylight_saving }) => {
      return planEdit((m, plan) => placeLocation(plan, name, latitude, longitude, utc_offset, daylight_saving));
    },
    rest_data: () => {
      const m = model();
      const plan = planOf(m);
      const cast = m.categories.find((c) => c.cast);
      const castRest = cast ? plan.turnarounds[cast.id] : undefined;
      return {
        turnarounds: m.categories.map((c) => ({ category: c.id, name: c.name, minimum: plan.turnarounds[c.id] ? hoursText(plan.turnarounds[c.id]) : '' })),
        short: castRest && castRest > 10 * 60 ? [`KIT rests 10:00 between 2027-03-01 (wrap 20:00) and 2027-03-02 (call 06:00), under the ${hoursText(castRest)} turnaround.`] : [],
        unchecked: 2,
      };
    },
    set_turnaround: ({ category, value }) => {
      const t = String(value).trim();
      const minutes = t ? read(() => parseHours(t)) : null;
      if (!model().categories.some((c) => c.id === category)) throw `No category ${category}.`;
      if (minutes !== null && (minutes === 0 || minutes > 48 * 60)) throw 'A turnaround is more than 0:00 and 48:00 at most.';
      return planEdit((m, plan) => {
        if (minutes === null) delete plan.turnarounds[category];
        else plan.turnarounds[category] = minutes;
      });
    },
    locks_data: () => planOf(model()).locks.map((name) => ({ name, made: '2027-02-20', scenes: 5 })),
    lock_schedule: ({ name }) => {
      const n = String(name).trim();
      if ([...String(name)].length > 200) throw "A lock's name is 200 characters at most.";
      if (!n) throw "A name can't be empty.";
      if (planOf(model()).locks.some((x) => sameName(x, n))) throw `There is already a lock called ${q(n)}.`;
      return planEdit((m, plan) => plan.locks.push(n), `Locked the schedule as ${q(n)}.`);
    },
    delete_lock: ({ name }) => {
      if (!planOf(model()).locks.some((x) => sameName(x, name))) throw `There is no lock called ${q(name)}.`;
      return planEdit((m, plan) => {
        plan.locks = plan.locks.filter((x) => !sameName(x, name));
      });
    },
    // As plan::export_drift: the Save dialog's outcome, a file named for the lock.
    export_drift: ({ name, format }) => {
      if (!['xlsx', 'csv'].includes(format)) throw "That export format isn't one the app knows.";
      return { done: true, file: `Harbor - Drift since ${name}.${format}` };
    },
    drift_data: ({ name }) => ({
      lock: name,
      actors: [{ name: 'KIT', board_id: '1', before: 3, after: 3, added: '2027-03-03', dropped: '2027-03-02', moved: ['Sc 3: 2027-03-02 to 2027-03-03'] }],
      unchanged: 2,
    }),
    // As plan::set_pinned: scene strips only, and only the ones that change are counted.
    pin_strips: ({ strips, pinned }) => {
      const scenes = sceneStrips(model());
      if (strips.some((id) => !scenes.has(id))) throw 'Only scene strips can be pinned.';
      let n = 0;
      const v = planEdit((m, plan) => {
        for (const id of strips) {
          const has = plan.pinned.includes(id);
          if (pinned && !has) {
            plan.pinned.push(id);
            n += 1;
          } else if (!pinned && has) {
            plan.pinned = plan.pinned.filter((x) => x !== id);
            n += 1;
          }
        }
      });
      v.notice = `${pinned ? 'Pinned' : 'Unpinned'} ${n === 1 ? '1 strip' : `${n} strips`}.`;
      return v;
    },
    suggest_order: ({ sub_board }) => {
      const s = suggestion(model(), sub_board);
      return {
        sub_board,
        sub_board_name: s.name,
        order: s.order,
        moved: s.moved,
        pinned: s.pinned,
        groups: s.groups,
        rows: [
          { label: 'Shoot days', before: 3, after: 3 },
          { label: 'Cast work days', before: 6, after: 4 },
          { label: 'Location changes in a day', before: 3, after: 1 },
          { label: 'Red flag clashes', before: 0, after: 0 },
        ],
      };
    },
    apply_suggested_order: ({ sub_board, order }) => {
      const s = suggestion(model(), sub_board);
      if (s.order.length !== order.length || s.order.some((id, i) => id !== order[i])) throw 'The board changed since the preview; suggest the order again.';
      const what = s.moved === 1 ? '1 scene' : `${s.moved} scenes`;
      return planEdit((m) => {
        const sb = subOf(m, sub_board);
        const byId = new Map(sb.strips.map((x) => [x.id, x]));
        sb.strips = order.map((id) => byId.get(id));
      }, `Moved ${what} into the suggested order. Undo puts them back.`);
    },
    export_call_sheet: ({ day, format }) => ({ done: true, file: `Harbor Light - Call Sheet Day ${day}.${format}` }),
    // Print Call Sheet's data, in the JSON export's shape (docs/CALLSHEET.md). Invented and
    // fixed: enough for the page's layout, not worked out from the fake board.
    call_sheet_data: ({ sub_board, day }) => {
      const sb = view().board.sub_boards.find((x) => x.id === sub_board);
      if (!sb || day < 1 || day > sb.days.length) throw `Day ${day} isn't a shoot day of this sub-board.`;
      const sc = (scenes, int_ext, set, time_of_day, pages, cast, start, end, synopsis) => ({ scenes, int_ext, set, time_of_day, pages, cast, start, end, synopsis, location: 'Pell Marine, harbor wall' });
      return {
        format: 'method-schedule-callsheet', version: 1, title: 'Harbor Light', company: 'Gull Point Pictures', director: 'Ines Varga', producer: 'Tom Achebe',
        board: 'Board 1', sub_board: sb.name, day, days: sb.days.length, date: '2026-10-20', call: '07:00', call_minutes: 420, wrap: '15:40', wrap_minutes: 940, length_minutes: 520, eighths: 49, pages: '6 1/8',
        sun: { state: 'times', location: 'Harbor wall, Pell Marine', sunrise: '07:15', sunrise_minutes: 435, sunset: '18:19', sunset_minutes: 1099 },
        scenes: [
          sc('3', 'EXT', 'HARBOR WALL', 'DAY', '2/8', ['1', '4'], '07:00', '07:20', 'Kit spots the ferry turning back.'),
          sc('5', 'EXT', 'HARBOR WALL', 'DAY', '1 2/8', ['1', '4', '6'], '07:20', '08:50', 'Lund warns Kit off the logbook.'),
          sc('9', 'EXT', 'HARBOR WALL', 'DUSK', '3/8', ['1'], '08:50', '09:20', 'Kit alone as the light goes.'),
        ],
        cast: [
          { element: 1, board_id: '1', name: 'KIT', scenes: ['3', '5', '9'], first: '07:00', last: '09:20', call: '06:30', wrap: '15:40', own_call: true },
          { element: 4, board_id: '4', name: 'HARBORMASTER LUND', scenes: ['3', '5'], first: '07:00', last: '08:50', call: '07:00', wrap: '15:40', own_call: false },
        ],
        elements: [{ category: 5, name: 'Hand props', elements: [{ element: 9, board_id: '', name: 'Logbook', scenes: ['5'] }] }],
        notes: ['Company move to Gull Point, 09:20, 30 minutes.', 'Scene 9 is exterior dusk: shoot before sunset at 18:19.'],
        advance: day < sb.days.length
          ? [{ day: day + 1, date: '2026-10-21', call: '07:00', call_minutes: 420, eighths: 11, pages: '1 3/8', scenes: [sc('3A', 'INT/EXT', "KIT'S TRUCK", 'DAY', '1/8', ['1'], null, null, ''), sc('7', 'INT', 'NET LOFT', 'NIGHT', '1 2/8', ['1', '4'], null, null, '')] }]
          : [],
      };
    },
    shoot_day_count: ({ sub_board }) => {
      const sb = view().board.sub_boards.find((x) => x.id === sub_board);
      return sb ? sb.days.length : 0;
    },
  };
}
