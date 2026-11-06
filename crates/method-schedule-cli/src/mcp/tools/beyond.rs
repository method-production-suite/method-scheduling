//! The tools for the features beyond the reference app (decisions.md, IMP-04 and BYD-01
//! to BYD-09): script revisions, the running timeline and call times, call sheet data,
//! locations for sunrise and sunset, turnarounds and rest checks, schedule locks and
//! drift, and the strip order suggestion.

use method_schedule_core::callsheet::call_sheet as day_call_sheet;
use method_schedule_core::days::days as shoot_days;
use method_schedule_core::drift::{drift as lock_drift, lock, unlock, WorkDay};
use method_schedule_core::model::{CategoryId, StripId};
use method_schedule_core::optimize::{apply as apply_order, suggest, Score};
use method_schedule_core::plan::{
    self, clock_text, degrees_text, offset_text, parse_clock, parse_daylight_saving, parse_degrees,
    parse_hours, parse_offset, GeoLocation, OrderWeights,
};
use method_schedule_core::rest::rest_report;
use method_schedule_core::revise::{revise, revision_text, Revision};
use method_schedule_core::solar::Sun;
use method_schedule_core::timeline::{
    past_wrap, past_wrap_in, push_past_wrap, timelines, EntryKind,
};
use method_schedule_core::{fdx, fountain, Schedule};
use serde_json::{json, Map, Value};

use super::super::files::{self, edit, load, Edited};
use super::super::paths::SCRIPT_EXTS;
use super::super::schema::{array, boolean, integer, name, nullable, object, one_of, path};
use super::{
    board_prop, id_prop, obj, path_prop, pick, pick_board, quoted, revision_prop, sub_board_prop,
    Args, Ctx, Tool, ToolError, ToolOut, ToolResult,
};

fn day_prop() -> (&'static str, Value) {
    ("day", integer("A shoot day number, from 1.", 1, 100_000))
}

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "timeline",
            title: "Running timeline",
            description: "Each shoot day of a sub-board from its call time: every scene and \
                          banner with its start and end (minutes after midnight, and HH:MM; \
                          minutes after the call when the day has no call time), the wrap, \
                          the day's length, sunrise and sunset at the first scene whose \
                          location is on the map, and warnings (a long day, exterior day \
                          scenes after sunset or before sunrise, scenes with no shoot time).",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![path_prop(), board_prop(), sub_board_prop(), day_prop()],
                    &["path"],
                )
            },
            run: timeline,
        },
        Tool {
            name: "call_sheet",
            title: "Call sheet data",
            description: "One shoot day's call sheet data (docs/CALLSHEET.md): the day's \
                          scenes in order with clock times, the cast with board IDs, other \
                          elements by category, sunrise and sunset, and notes (banners, \
                          events, warnings). The same JSON the command line writes.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![path_prop(), board_prop(), sub_board_prop(), day_prop()],
                    &["path", "day"],
                )
            },
            run: call_sheet,
        },
        Tool {
            name: "rest_report",
            title: "Rest checks",
            description: "Rests shorter than their category's turnaround minimum, across \
                          every sub-board of a board: the element, the two dates, the \
                          wrap and next call, and the rest in minutes. Warnings only. Days \
                          with no date or no call time are counted as unchecked.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop(), board_prop()], &["path"]),
            run: rest,
        },
        Tool {
            name: "drift",
            title: "Drift since a lock",
            description: "Each actor's work days added, dropped and moved since a schedule \
                          lock was made, with day counts then and now. Lists the locks when \
                          `lock` is left out.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        ("lock", name("The lock's name. Default: list the locks.")),
                    ],
                    &["path"],
                )
            },
            run: drift,
        },
        Tool {
            name: "suggest_order",
            title: "Suggest a strip order",
            description: "Works out a strip order for a sub-board without changing it: \
                          scheduled scenes grouped by location, then set, then shared cast; \
                          day breaks, banners and pinned strips stay put; red-flagged dates \
                          avoided where possible. Returns the order and the before and after \
                          counts (shoot days, cast work days, location changes, red flag \
                          clashes). apply_order puts it on the board.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop(), board_prop(), sub_board_prop()], &["path"]),
            run: suggest_order,
        },
        Tool {
            name: "locations",
            title: "Locations on the map",
            description: "Every sheet location, how many sheets use it, and its latitude, \
                          longitude and UTC offset when it is on the map.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop()], &["path"]),
            run: locations,
        },
        Tool {
            name: "revise_script",
            title: "Merge a script revision",
            description: "Takes a revised draft (.fountain or .fdx, inside the roots) into \
                          the schedule: scenes matched to sheets by scene number, then by \
                          heading, keep their ids, tags and board places and take the \
                          draft's number, I/E, set, time of day and length; new speaking \
                          characters (and production tags, with tags true, .fdx only) are \
                          added; scenes with no match become new sheets in the Boneyard; \
                          sheets the draft lacks are marked omitted, not deleted. Returns \
                          what changed.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("script", path("The revised script (.fountain or .fdx).")),
                        (
                            "tags",
                            boolean("Also read production tags (.fdx). Default false."),
                        ),
                        super::lines_per_page_prop(),
                    ],
                    &["path", "revision", "script"],
                )
            },
            run: revise_script,
        },
        Tool {
            name: "set_call_time",
            title: "Set a call time",
            description: "Sets (HH:MM, 24-hour) or clears (null) the call time of a shoot \
                          day of a sub-board. The timeline, call sheet and rest checks use \
                          it.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        day_prop(),
                        (
                            "call",
                            nullable(name("The call time, such as 06:30, or null.")),
                        ),
                    ],
                    &["path", "revision", "day", "call"],
                )
            },
            run: set_call_time,
        },
        Tool {
            name: "set_location",
            title: "Place a location on the map",
            description: "Gives a sheet location name a latitude and longitude (decimal \
                          degrees, north and east positive, or with N, S, E, W) and a UTC \
                          offset (such as -7 or +05:30) for sunrise and sunset, and a \
                          daylight saving rule (with one, the offset is the standard one); or \
                          takes it off the map with remove true.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("name", name("The location, as the sheets write it.")),
                        ("latitude", name("Degrees, such as 34.0522.")),
                        ("longitude", name("Degrees, such as -118.2437.")),
                        (
                            "utc_offset",
                            name("The UTC offset on the shoot dates, or the standard one with a daylight saving rule. Default: from the longitude."),
                        ),
                        (
                            "daylight_saving",
                            one_of(
                                "When the clocks go an hour ahead: us_canada (second Sunday of March to first Sunday of November), eu (last Sunday of March to last Sunday of October), australia (Australia's south-east: first Sunday of October to first Sunday of April; Queensland, the Northern Territory and Western Australia have none), new_zealand (last Sunday of September to first Sunday of April), israel (the Friday before the last Sunday of March to the last Sunday of October), egypt (last Friday of April to the end of the last Thursday of October), chile (mainland: the Sunday after the first Saturday of September to the Sunday after the first Saturday of April) or none. Default: the location's rule, else none.",
                                &[
                                    "none",
                                    "us_canada",
                                    "eu",
                                    "australia",
                                    "new_zealand",
                                    "israel",
                                    "egypt",
                                    "chile",
                                ],
                            ),
                        ),
                        (
                            "offsets",
                            name("Dates with a UTC offset of their own, such as 2027-02-07 to 2027-03-14 +00:00, several split by semicolons; empty for none. Default: as they are."),
                        ),
                        ("remove", boolean("Take it off the map instead.")),
                    ],
                    &["path", "revision", "name"],
                )
            },
            run: set_location,
        },
        Tool {
            name: "set_crew_call",
            title: "Set a department's call",
            description: "Sets a crew department's call against the company's on every shoot \
                          day (-1:30, 0, +0:15), or removes it with null.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("department", name("The department, such as Camera.")),
                        (
                            "offset",
                            nullable(name("From the call, such as -1:30, or null.")),
                        ),
                    ],
                    &["path", "revision", "department", "offset"],
                )
            },
            run: set_crew_call,
        },
        Tool {
            name: "set_day_crew_call",
            title: "Set a department's call for one day",
            description: "Sets a department's call on one shoot day only: in place of its \
                          every-day call, a department called that day alone, or off to leave \
                          an every-day department out of it. Null follows the every-day call \
                          again.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        day_prop(),
                        ("department", name("The department, such as Stunts.")),
                        (
                            "offset",
                            nullable(name("From the call, such as +0:45, off, or null.")),
                        ),
                    ],
                    &["path", "revision", "day", "department", "offset"],
                )
            },
            run: set_day_crew_call,
        },
        Tool {
            name: "set_location_guide",
            title: "Set how to get to a location",
            description: "Sets a location's address, directions and map picture for the call \
                          sheet. Each part left out stays as it is; an empty text or a null map \
                          clears it. The map is a path to a picture file, kept beside the \
                          schedule; nothing is fetched.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("name", name("The location, as the sheets write it.")),
                        ("address", name("The address.")),
                        (
                            "directions",
                            name("The directions; may hold several lines."),
                        ),
                        (
                            "map",
                            nullable(name("A picture file's path, or null for none.")),
                        ),
                    ],
                    &["path", "revision", "name"],
                )
            },
            run: set_location_guide,
        },
        Tool {
            name: "set_order_weights",
            title: "Set what a suggested order counts",
            description: "Sets what each count costs when suggest_order compares orders, each \
                          0 to 1000. Each one left out stays as it is; reset true goes back to \
                          the defaults (4, 3, 1, 20, 5).",
            mutating: true,
            destructive: false,
            schema: || {
                let w = |d: &str| integer(d, 0, 1000);
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("cast_day", w("An actor's day.")),
                        ("location_change", w("A location change in a day.")),
                        ("day_night_switch", w("A day and night switch in a day.")),
                        ("red_flag_clash", w("A red flag clash.")),
                        ("long_day_hour", w("An hour past the long day limit.")),
                        ("reset", boolean("Go back to the defaults instead.")),
                    ],
                    &["path", "revision"],
                )
            },
            run: set_order_weights,
        },
        Tool {
            name: "rename_draft",
            title: "Rename a script draft's color",
            description: "Renames a script draft's color. Unique without regard to case.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("color", name("The draft's color now, such as Blue.")),
                        ("name", name("The new name.")),
                    ],
                    &["path", "revision", "color", "name"],
                )
            },
            run: rename_draft,
        },
        Tool {
            name: "set_turnaround",
            title: "Set a turnaround minimum",
            description: "Sets (hours, or h:mm) or clears (null) the shortest rest between \
                          wrap and next call for the elements of a category, and the long \
                          day limit (long_day, h:mm; null for 12:00).",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("category", name("The category's name.")),
                        ("minimum", nullable(name("Such as 12 or 10:30, or null."))),
                        ("long_day", nullable(name("Such as 12 or 13:00, or null."))),
                    ],
                    &["path", "revision"],
                )
            },
            run: set_turnaround,
        },
        Tool {
            name: "lock_schedule",
            title: "Lock or unlock the schedule",
            description: "Saves where every scheduled scene of a board is now, under a name, \
                          to report drift against later; or deletes the lock with remove \
                          true.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        ("name", name("The lock's name.")),
                        ("remove", boolean("Delete the lock instead.")),
                    ],
                    &["path", "revision", "name"],
                )
            },
            run: lock_schedule,
        },
        Tool {
            name: "pin_strips",
            title: "Pin strips",
            description: "Pins scene strips (or unpins them, with pinned false) so a \
                          suggested order leaves them where they are.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        (
                            "strips",
                            array("Strip ids.", id_prop("A strip id."), 10_000),
                        ),
                        ("pinned", boolean("Pin (true, the default) or unpin.")),
                    ],
                    &["path", "revision", "strips"],
                )
            },
            run: pin_strips,
        },
        Tool {
            name: "push_past_wrap",
            title: "Push what runs past wrap",
            description: "Moves what runs past a shoot day's wrap (its call plus the long day \
                          limit; timeline's past_wrap lists it) to the start of the next \
                          shoot day on the same sub-board: whole strips in board order, the \
                          day's first strip staying, day breaks and call times in place. \
                          Refused on the last shoot day, when nothing runs past, or when \
                          strips is given and is no longer what would move.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        day_prop(),
                        (
                            "strips",
                            array(
                                "The strips timeline listed in past_wrap. Default: what runs past now.",
                                id_prop("A strip id."),
                                100_000,
                            ),
                        ),
                    ],
                    &["path", "revision", "day"],
                )
            },
            run: push_past,
        },
        Tool {
            name: "apply_order",
            title: "Apply the suggested order",
            description: "Works out suggest_order for a sub-board again and puts it on the \
                          board. Returns the before and after counts.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![path_prop(), revision_prop(), board_prop(), sub_board_prop()],
                    &["path", "revision"],
                )
            },
            run: apply_suggested,
        },
    ]
}

fn done<T>(e: &Edited<T>, what: &str, mut extra: Map<String, Value>) -> ToolOut {
    extra.insert("path".into(), json!(e.shown));
    extra.insert("changed".into(), json!(e.changed));
    extra.insert("revision".into(), json!(e.revision));
    let summary = if e.changed {
        format!("{what} in {}. Revision {}.", e.shown, e.revision)
    } else {
        format!("{what}: nothing changed in {}; nothing written.", e.shown)
    };
    ToolOut {
        summary,
        data: Value::Object(extra),
    }
}

fn clock(m: Option<i32>) -> Value {
    m.map_or(Value::Null, |m| json!(clock_text(m)))
}

fn sun_json(sun: Option<&Sun>) -> Value {
    match sun {
        Some(Sun::Times { rise, set }) => json!({
            "state": "times",
            "sunrise_minutes": rise,
            "sunrise": clock_text(*rise),
            "sunset_minutes": set,
            "sunset": clock_text(*set),
        }),
        Some(Sun::AlwaysUp) => json!({"state": "always_up"}),
        Some(Sun::AlwaysDown) => json!({"state": "always_down"}),
        None => json!({"state": "unknown"}),
    }
}

fn timeline(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (b, sb) = pick(s, a)?;
    let only = a.u32("day");
    let mut out = Vec::new();
    let mut warnings = 0;
    let all = timelines(s, b, sb)?;
    for t in &all {
        if only.is_some_and(|n| n != t.number) {
            continue;
        }
        warnings += t.warnings.len();
        let timed = t.call.is_some();
        let past = past_wrap_in(s, b, sb, &all, t.number)?;
        out.push(json!({
            "day": t.number,
            "date": t.date.map(|d| d.to_string()),
            "call_minutes": t.call,
            "call": clock(t.call.map(|c| c as i32)),
            "wrap_minutes": t.wrap,
            "wrap": if timed { clock(Some(t.wrap)) } else { Value::Null },
            "length_minutes": t.length,
            "sun_location": t.sun.as_ref().map(|x| x.location.clone()),
            "sun": sun_json(t.sun.as_ref().map(|x| &x.sun)),
            "entries": t.entries.iter().map(|e| {
                let (kind, sheet, text) = match &e.kind {
                    EntryKind::Scene(id) => ("scene", Some(id.0), s.sheet(*id).map_or(String::new(), method_schedule_core::timeline::scene_name)),
                    EntryKind::Banner(t) => ("banner", None, t.clone()),
                };
                json!({
                    "strip": e.strip.0,
                    "kind": kind,
                    "sheet": sheet,
                    "text": text,
                    "start_minutes": e.start,
                    "end_minutes": e.end,
                    "start": t.time_text(e.start),
                    "end": t.time_text(e.end),
                    "untimed": e.untimed,
                    "warnings": e.warnings,
                })
            }).collect::<Vec<_>>(),
            "warnings": t.warnings,
            "past_wrap": {
                "wrap_by_minutes": past.wrap_by,
                "wrap_by": t.time_text(past.wrap_by),
                "strips": past.strips.iter().map(|x| x.0).collect::<Vec<_>>(),
                "names": past.names,
                "next_day": past.next,
                "text": past.text(),
            },
        }));
    }
    if let Some(n) = only {
        if out.is_empty() {
            return Err(ToolError::not_found(format!("there is no shoot day {n}")));
        }
    }
    Ok(ToolOut {
        summary: format!("{} shoot day(s), {warnings} warning(s).", out.len()),
        data: json!({"path": l.shown, "revision": l.revision, "days": out}),
    })
}

fn call_sheet(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (b, sb) = pick(s, a)?;
    let day = a.req_u32("day")?;
    let c = day_call_sheet(s, b, sb, day).map_err(|e| ToolError::not_found(e.to_string()))?;
    let data: Value = serde_json::from_str(&method_schedule_format::callsheet::to_json(&c))
        .map_err(|e| ToolError::io(e.to_string()))?;
    Ok(ToolOut {
        summary: format!(
            "Day {} of {}: {} scene(s), {} cast.",
            c.day,
            c.days,
            c.scenes.len(),
            c.cast.len()
        ),
        data,
    })
}

fn rest(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let b = pick_board(s, a)?;
    let r = rest_report(s, b)?;
    let short: Vec<Value> = r
        .short
        .iter()
        .map(|x| {
            json!({
                "element": x.element.0,
                "name": s.element(x.element).map_or(String::new(), |e| e.name.clone()),
                "category": x.category.0,
                "from": x.from.date.to_string(),
                "wrap": clock_text(x.from.wrap),
                "to": x.to.date.to_string(),
                "call": clock_text(x.to.call),
                "rest_minutes": x.rest,
                "minimum_minutes": x.minimum,
                "text": x.text,
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} short rest(s); {} work day(s) unchecked.",
            short.len(),
            r.unchecked
        ),
        data: json!({"short": short, "unchecked": r.unchecked, "turnarounds": s.plan.turnarounds.iter().map(|t| json!({
            "category": t.category.0,
            "name": s.category(t.category).map_or(String::new(), |c| c.name.clone()),
            "minutes": t.minutes,
        })).collect::<Vec<_>>()}),
    })
}

fn work_day_json(s: &Schedule, w: &WorkDay) -> Value {
    json!(w.text(s))
}

fn drift(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let Some(name) = a.str("lock") else {
        let locks: Vec<Value> = s
            .plan
            .locks
            .iter()
            .map(|k| {
                json!({
                    "name": k.name,
                    "board": k.board.0,
                    "made": k.made.map(|d| d.to_string()),
                    "scenes": k.scenes.len(),
                })
            })
            .collect();
        return Ok(ToolOut {
            summary: format!("{} lock(s).", locks.len()),
            data: json!({"locks": locks}),
        });
    };
    let d = lock_drift(s, name).map_err(|e| ToolError::not_found(e.to_string()))?;
    let actors: Vec<Value> = d
        .actors
        .iter()
        .map(|x| {
            json!({
                "element": x.element.0,
                "name": x.name,
                "board_id": x.board_id,
                "before": x.before,
                "after": x.after,
                "added": x.added.iter().map(|w| work_day_json(s, w)).collect::<Vec<_>>(),
                "dropped": x.dropped.iter().map(|w| work_day_json(s, w)).collect::<Vec<_>>(),
                "moved": x.moved.iter().map(|m| json!({
                    "sheet": m.sheet.0,
                    "from": work_day_json(s, &m.from),
                    "to": work_day_json(s, &m.to),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let changed = d.changed().count();
    Ok(ToolOut {
        summary: format!(
            "{changed} actor(s) drifted since the lock {}.",
            quoted(&d.lock)
        ),
        data: json!({"lock": d.lock, "made": d.made.map(|x| x.to_string()), "actors": actors}),
    })
}

fn score_json(x: &Score, w: &OrderWeights) -> Value {
    json!({
        "shoot_days": x.shoot_days,
        "cast_days": x.cast_days,
        "location_changes": x.location_changes,
        "day_night_switches": x.day_night_switches,
        "red_flag_clashes": x.red_flag_clashes,
        "long_day_hours": x.long_day_hours,
        "cost": x.cost(w),
    })
}

fn suggest_order(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (b, sb) = pick(s, a)?;
    let sug = suggest(s, b, sb)?;
    Ok(ToolOut {
        summary: format!(
            "{} scene(s) would move ({}); cost {} -> {}.",
            sug.moved,
            sug.rule,
            sug.before.cost(&sug.weights),
            sug.after.cost(&sug.weights)
        ),
        data: json!({
            "order": sug.order.iter().map(|x| x.0).collect::<Vec<_>>(),
            "moved": sug.moved,
            "pinned": sug.pinned,
            "groups": sug.groups,
            "before": score_json(&sug.before, &sug.weights),
            "after": score_json(&sug.after, &sug.weights),
            "rule": sug.rule,
            "tried": sug.tried.iter().map(|(r, c)| json!({"rule": r, "cost": c})).collect::<Vec<_>>(),
        }),
    })
}

fn locations(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let mut names: Vec<String> = Vec::new();
    for h in &s.sheets {
        let n = h.location.trim();
        if !n.is_empty() && !names.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            names.push(n.to_string());
        }
    }
    for g in &s.plan.locations {
        if !names.iter().any(|x| x.eq_ignore_ascii_case(&g.name)) {
            names.push(g.name.clone());
        }
    }
    for g in &s.plan.location_guides {
        if !names.iter().any(|x| x.eq_ignore_ascii_case(&g.name)) {
            names.push(g.name.clone());
        }
    }
    let list: Vec<Value> = names
        .iter()
        .map(|n| {
            let g = s.plan.location(n);
            json!({
                "name": n,
                "sheets": s.sheets.iter().filter(|h| h.location.trim().eq_ignore_ascii_case(n)).count(),
                "latitude": g.map(|g| degrees_text(g.latitude)),
                "longitude": g.map(|g| degrees_text(g.longitude)),
                "utc_offset": g.map(|g| offset_text(g.utc_offset)),
                "daylight_saving": g.map(|g| g.daylight_saving.map_or("none", |r| r.name())),
                "offsets": g.map(|g| plan::offset_periods_text(&g.periods)),
                "address": s.plan.location_guide(n).map(|x| x.address.clone()),
                "directions": s.plan.location_guide(n).map(|x| x.directions.clone()),
                "map": s.plan.location_guide(n).and_then(|x| x.map.clone()),
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} location(s), {} on the map.",
            list.len(),
            s.plan.locations.len()
        ),
        data: json!({"locations": list}),
    })
}

fn revision_json(s: &Schedule, r: &Revision) -> Value {
    let els = |v: &[method_schedule_core::model::ElementId]| {
        v.iter()
            .filter_map(|e| s.element(*e))
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
    };
    json!({
        "matched": r.matched.len(),
        "changed": r.changed().map(|m| json!({
            "sheet": m.sheet.0,
            "by_heading": m.by_heading,
            "fields": m.fields.iter().map(|f| json!({"field": f.field, "before": f.before, "after": f.after})).collect::<Vec<_>>(),
            "added": els(&m.tagged),
            "no_longer_speaking": els(&m.silent),
            "restored": m.restored,
        })).collect::<Vec<_>>(),
        "added": r.added.iter().map(|x| x.0).collect::<Vec<_>>(),
        "omitted": r.omitted.iter().map(|x| x.0).collect::<Vec<_>>(),
        "draft": r.draft,
        "text": revision_text(s, r),
    })
}

fn revise_script(ctx: &Ctx, a: &Args) -> ToolResult {
    let typed = a.req("script")?;
    let real = ctx.roots.existing(typed, SCRIPT_EXTS)?;
    let shown = ctx.roots.show(&real);
    let bytes = files::read_file(&real, &shown)?;
    let text = files::text(&bytes, &shown)?;
    let tags = a.opt_bool("tags").unwrap_or(false);
    let is_fdx = real
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fdx"));
    let script = match (is_fdx, tags) {
        (true, true) => fdx::parse_with_tags(&text),
        (true, false) => fdx::parse(&text),
        (false, true) => {
            return Err(ToolError::invalid(
                "production tags come only from .fdx files",
            ))
        }
        (false, false) => Ok(fountain::parse_with(&text, super::lines_per_page(a))),
    }
    .map_err(|e| ToolError::invalid(format!("{shown}: {e}")))?;
    let e = edit(ctx, a, |s| {
        let cast = s.cast_or_add();
        let r = revise(
            s,
            &script,
            cast,
            tags,
            method_schedule_format::exchange::local_today(),
        )?;
        Ok(revision_json(s, &r))
    })?;
    let line = e.out["text"]
        .as_str()
        .and_then(|t| t.lines().next())
        .unwrap_or("")
        .to_string();
    let Value::Object(m) = e.out.clone() else {
        unreachable!()
    };
    Ok(done(&e, &format!("Revised from {shown}: {line}"), m))
}

fn set_call_time(ctx: &Ctx, a: &Args) -> ToolResult {
    let n = a.req_u32("day")?;
    let call = a
        .str("call")
        .map(parse_clock)
        .transpose()
        .map_err(ToolError::invalid)?;
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let day = shoot_days(s, b, sb)?
            .days
            .iter()
            .find(|d| d.number == n)
            .map(|d| d.break_id)
            .ok_or_else(|| ToolError::not_found(format!("there is no shoot day {n}")))?;
        plan::set_call_time(s, day, call)?;
        Ok(())
    })?;
    let what = match call {
        Some(c) => format!("Day {n} calls at {}", clock_text(c as i32)),
        None => format!("Day {n} has no call time"),
    };
    Ok(done(
        &e,
        &what,
        obj(vec![
            ("day", json!(n)),
            ("call", clock(call.map(|c| c as i32))),
        ]),
    ))
}

/// A crew offset as given, or null for none.
fn crew_offset_arg(a: &Args) -> Result<Option<i32>, ToolError> {
    a.str("offset")
        .map(plan::parse_crew_offset)
        .transpose()
        .map_err(ToolError::invalid)
}

fn set_crew_call(ctx: &Ctx, a: &Args) -> ToolResult {
    let department = a.req("department")?.to_string();
    let offset = crew_offset_arg(a)?;
    let e = edit(ctx, a, |s| {
        plan::set_crew_call(s, &department, offset)?;
        Ok(())
    })?;
    let what = offset.map_or("no call of its own".to_string(), plan::crew_offset_words);
    Ok(done(
        &e,
        &format!("{department}: {what}"),
        obj(vec![
            ("department", json!(department)),
            ("offset", json!(offset)),
        ]),
    ))
}

fn set_day_crew_call(ctx: &Ctx, a: &Args) -> ToolResult {
    let n = a.req_u32("day")?;
    let department = a.req("department")?.to_string();
    let off = a
        .str("offset")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("off"));
    let offset = if off { None } else { crew_offset_arg(a)? };
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let day = shoot_days(s, b, sb)?
            .days
            .iter()
            .find(|d| d.number == n)
            .map(|d| d.break_id)
            .ok_or_else(|| ToolError::not_found(format!("there is no shoot day {n}")))?;
        if off {
            plan::set_day_crew_off(s, day, &department)?;
        } else {
            plan::set_day_crew_call(s, day, &department, offset)?;
        }
        Ok(())
    })?;
    let what = if off {
        "not called".to_string()
    } else {
        offset.map_or(
            "follows the every-day call".to_string(),
            plan::crew_offset_words,
        )
    };
    Ok(done(
        &e,
        &format!("{department} on day {n}: {what}"),
        obj(vec![
            ("day", json!(n)),
            ("department", json!(department)),
            ("offset", json!(offset)),
            ("off", json!(off)),
        ]),
    ))
}

fn set_location_guide(ctx: &Ctx, a: &Args) -> ToolResult {
    let name = a.req("name")?.to_string();
    let e = edit(ctx, a, |s| {
        let old = s.plan.location_guide(&name).cloned().unwrap_or_default();
        // A map left out stays; given as null, it goes.
        let map = if a.has("map") {
            a.str("map").map(str::to_string)
        } else {
            old.map
        };
        s.plan.set_location_guide(plan::LocationGuide {
            name: name.clone(),
            address: a.str("address").map_or(old.address, str::to_string),
            directions: a.str("directions").map_or(old.directions, str::to_string),
            map,
        })?;
        Ok(s.plan.location_guide(&name).cloned())
    })?;
    let g = e.out.clone();
    Ok(done(
        &e,
        &format!("Directions to {} set", quoted(&name)),
        obj(vec![
            ("address", json!(g.as_ref().map(|x| x.address.clone()))),
            (
                "directions",
                json!(g.as_ref().map(|x| x.directions.clone())),
            ),
            ("map", json!(g.and_then(|x| x.map))),
        ]),
    ))
}

fn set_order_weights(ctx: &Ctx, a: &Args) -> ToolResult {
    let e = edit(ctx, a, |s| {
        let w = if a.opt_bool("reset").unwrap_or(false) {
            None
        } else {
            let was = s.plan.order_weights();
            Some(OrderWeights {
                cast_day: a.u32("cast_day").unwrap_or(was.cast_day),
                location_change: a.u32("location_change").unwrap_or(was.location_change),
                day_night_switch: a.u32("day_night_switch").unwrap_or(was.day_night_switch),
                red_flag_clash: a.u32("red_flag_clash").unwrap_or(was.red_flag_clash),
                long_day_hour: a.u32("long_day_hour").unwrap_or(was.long_day_hour),
            })
        };
        s.plan.set_order_weights(w)?;
        Ok(s.plan.order_weights())
    })?;
    let w = e.out;
    Ok(done(
        &e,
        "Strip order weights set",
        obj(vec![
            ("cast_day", json!(w.cast_day)),
            ("location_change", json!(w.location_change)),
            ("day_night_switch", json!(w.day_night_switch)),
            ("red_flag_clash", json!(w.red_flag_clash)),
            ("long_day_hour", json!(w.long_day_hour)),
        ]),
    ))
}

fn rename_draft(ctx: &Ctx, a: &Args) -> ToolResult {
    let color = a.req("color")?.to_string();
    let name = a.req("name")?.to_string();
    let e = edit(ctx, a, |s| {
        s.plan.rename_draft(&color, &name)?;
        Ok(())
    })?;
    Ok(done(
        &e,
        &format!("The {color} draft is now {}", name.trim()),
        obj(vec![("color", json!(name.trim()))]),
    ))
}

fn set_location(ctx: &Ctx, a: &Args) -> ToolResult {
    let name = a.req("name")?.to_string();
    if a.opt_bool("remove").unwrap_or(false) {
        let e = edit(ctx, a, |s| {
            if s.plan.remove_location(&name) {
                Ok(())
            } else {
                Err(ToolError::not_found(format!(
                    "{} is not on the map",
                    quoted(&name)
                )))
            }
        })?;
        return Ok(done(
            &e,
            &format!("{} taken off the map", quoted(&name)),
            Map::new(),
        ));
    }
    let lat = parse_degrees(a.req("latitude")?, 90).map_err(ToolError::invalid)?;
    let lon = parse_degrees(a.req("longitude")?, 180).map_err(ToolError::invalid)?;
    let offset = match a.str("utc_offset") {
        Some(t) => parse_offset(t).map_err(ToolError::invalid)?,
        None => ((lon as i64 * 60 / 15 / 1_000_000) as i32 / 60) * 60,
    };
    let rule = a
        .str("daylight_saving")
        .map(parse_daylight_saving)
        .transpose()
        .map_err(ToolError::invalid)?;
    let typed_periods = a
        .str("offsets")
        .map(plan::parse_offset_periods)
        .transpose()
        .map_err(ToolError::invalid)?;
    let e = edit(ctx, a, |s| {
        // Left out, the location keeps its rule (decision 193).
        let daylight_saving = match rule {
            Some(r) => r,
            None => s.plan.location(&name).and_then(|l| l.daylight_saving),
        };
        // Periods of its own offset (decision 244) stay unless given.
        let periods = match &typed_periods {
            Some(p) => p.clone(),
            None => s
                .plan
                .location(&name)
                .map_or_else(Vec::new, |l| l.periods.clone()),
        };
        s.plan.set_location(GeoLocation {
            name: name.clone(),
            latitude: lat,
            longitude: lon,
            utc_offset: offset,
            daylight_saving,
            periods,
        })?;
        Ok(daylight_saving)
    })?;
    let rule = e.out;
    Ok(done(
        &e,
        &format!("{} placed on the map", quoted(&name)),
        obj(vec![
            ("latitude", json!(degrees_text(lat))),
            ("longitude", json!(degrees_text(lon))),
            ("utc_offset", json!(offset_text(offset))),
            ("daylight_saving", json!(rule.map_or("none", |r| r.name()))),
        ]),
    ))
}

fn set_turnaround(ctx: &Ctx, a: &Args) -> ToolResult {
    let hours = |k: &str| {
        a.str(k)
            .map(parse_hours)
            .transpose()
            .map_err(ToolError::invalid)
    };
    let (minimum, long_day) = (hours("minimum")?, hours("long_day")?);
    let category = a.str("category").map(str::to_string);
    if category.is_some() != a.has("minimum") {
        return Err(ToolError::invalid(
            "give category and minimum together (minimum null clears it)",
        ));
    }
    let e = edit(ctx, a, |s| {
        if let Some(c) = &category {
            let id: CategoryId = s
                .category_named(c)
                .ok_or_else(|| ToolError::not_found(format!("no category {}", quoted(c))))?;
            plan::set_turnaround(s, id, minimum)?;
        }
        if a.has("long_day") {
            plan::set_long_day(s, long_day)?;
        }
        Ok(s.plan.long_day())
    })?;
    let out = e.out;
    Ok(done(
        &e,
        "Turnaround set",
        obj(vec![("long_day_minutes", json!(out))]),
    ))
}

fn lock_schedule(ctx: &Ctx, a: &Args) -> ToolResult {
    let name = a.req("name")?.to_string();
    let remove = a.opt_bool("remove").unwrap_or(false);
    let e = edit(ctx, a, |s| {
        if remove {
            unlock(s, &name).map_err(|e| ToolError::not_found(e.to_string()))?;
            return Ok(0);
        }
        let b = pick_board(s, a)?;
        lock(s, b, &name, method_schedule_format::exchange::local_today())?;
        Ok(s.plan.lock(&name).map_or(0, |l| l.scenes.len()))
    })?;
    let what = if remove {
        format!("Lock {} deleted", quoted(&name))
    } else {
        format!("Locked {} scene(s) as {}", e.out, quoted(&name))
    };
    let n = e.out;
    Ok(done(&e, &what, obj(vec![("scenes", json!(n))])))
}

fn pin_strips(ctx: &Ctx, a: &Args) -> ToolResult {
    let ids: Vec<StripId> = a.u32s("strips").into_iter().map(StripId).collect();
    let on = a.opt_bool("pinned").unwrap_or(true);
    let e = edit(ctx, a, |s| Ok(plan::set_pinned(s, &ids, on)?))?;
    let n = e.out;
    Ok(done(
        &e,
        &format!("{} {n} strip(s)", if on { "Pinned" } else { "Unpinned" }),
        obj(vec![("count", json!(n))]),
    ))
}

fn push_past(ctx: &Ctx, a: &Args) -> ToolResult {
    let n = a.req_u32("day")?;
    let given = a.has("strips");
    let seen: Vec<StripId> = a.u32s("strips").into_iter().map(StripId).collect();
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let p = past_wrap(s, b, sb, n).map_err(|e| ToolError::not_found(e.to_string()))?;
        let seen = if given {
            seen.clone()
        } else {
            p.strips.clone()
        };
        push_past_wrap(s, b, sb, n, &seen)?;
        Ok(p)
    })?;
    let p = e.out.clone();
    Ok(done(
        &e,
        &format!(
            "Moved {} strip(s) from day {n} to the start of day {}",
            p.strips.len(),
            n + 1
        ),
        obj(vec![
            (
                "moved",
                json!(p.strips.iter().map(|x| x.0).collect::<Vec<_>>()),
            ),
            ("names", json!(p.names)),
            ("to_day", json!(n + 1)),
        ]),
    ))
}

fn apply_suggested(ctx: &Ctx, a: &Args) -> ToolResult {
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let sug = suggest(s, b, sb)?;
        apply_order(s, &sug)?;
        Ok((sug.before, sug.after, sug.moved, sug.weights))
    })?;
    let (before, after, moved, weights) = e.out;
    Ok(done(
        &e,
        &format!("Moved {moved} scene(s)"),
        obj(vec![
            ("moved", json!(moved)),
            ("before", score_json(&before, &weights)),
            ("after", score_json(&after, &weights)),
        ]),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::Fixture;
    use serde_json::json;

    #[test]
    fn the_plan_setters_on_a_schedule() {
        let mut f = Fixture::new("plan-setters", false);
        let p = "film.mtsc";
        let rev = f.rev(p);
        f.ok(
            "auto_day_breaks",
            json!({"path": p, "revision": rev, "by": "eighths", "limit": 24}),
        );
        let rev = f.rev(p);
        let r = f.ok(
            "set_crew_call",
            json!({"path": p, "revision": rev, "department": "Camera", "offset": "-1:30"}),
        );
        assert_eq!(r["offset"], -90);
        let rev = f.rev(p);
        f.ok(
            "set_day_crew_call",
            json!({"path": p, "revision": rev, "day": 1, "department": "Stunts", "offset": "+0:45"}),
        );
        let rev = f.rev(p);
        let c = f.ok("call_sheet", json!({"path": p, "day": 1}));
        let crew = c["sheet"]["crew"]
            .as_array()
            .or(c["crew"].as_array())
            .unwrap();
        assert_eq!(crew.len(), 2, "{c}");
        f.ok(
            "set_location",
            json!({"path": p, "revision": rev, "name": "Pier", "latitude": "34.05", "longitude": "-118.24",
                   "offsets": "2027-02-07 to 2027-03-14 +00:00"}),
        );
        let rev = f.rev(p);
        let g = f.ok(
            "set_location_guide",
            json!({"path": p, "revision": rev, "name": "Pier", "address": "1 Harbor Road", "map": "images/pier.png"}),
        );
        assert_eq!(g["map"], "images/pier.png");
        // A part left out stays; a null map goes.
        let rev = f.rev(p);
        let g = f.ok(
            "set_location_guide",
            json!({"path": p, "revision": rev, "name": "Pier", "directions": "Left at the gate.", "map": null}),
        );
        assert_eq!(g["address"], "1 Harbor Road");
        assert!(g["map"].is_null());
        let l = f.ok("locations", json!({"path": p}));
        let pier = l["locations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == "Pier")
            .unwrap();
        assert_eq!(pier["directions"], "Left at the gate.");
        assert!(pier["offsets"].as_str().unwrap().contains("2027-02-07"));
        let rev = f.rev(p);
        let w = f.ok(
            "set_order_weights",
            json!({"path": p, "revision": rev, "cast_day": 9}),
        );
        assert_eq!(
            (w["cast_day"].as_u64(), w["red_flag_clash"].as_u64()),
            (Some(9), Some(20))
        );
        assert_eq!(
            f.ok("suggest_order", json!({"path": p}))["before"]["long_day_hours"],
            0
        );
        let rev = f.rev(p);
        let w = f.ok(
            "set_order_weights",
            json!({"path": p, "revision": rev, "reset": true}),
        );
        assert_eq!(w["cast_day"], 4);
    }

    #[test]
    fn the_beyond_tools_on_a_schedule() {
        let mut f = Fixture::new("beyond", false);
        let p = "film.mtsc";
        let rev = f.rev(p);
        f.ok(
            "auto_day_breaks",
            json!({"path": p, "revision": rev, "by": "eighths", "limit": 24}),
        );
        let rev = f.rev(p);
        let r = f.ok(
            "set_call_time",
            json!({"path": p, "revision": rev, "day": 1, "call": "07:00"}),
        );
        assert_eq!(r["call"], "07:00");
        let t = f.ok("timeline", json!({"path": p, "day": 1}));
        assert_eq!(t["days"][0]["call"], "07:00");
        assert_eq!(t["days"][0]["past_wrap"]["strips"], json!([]));
        // Seven hours a scene on day 1: past its 19:00 wrap, then pushed (decision 192).
        let entries = t["days"][0]["entries"].as_array().unwrap().clone();
        assert!(entries.len() >= 2, "{entries:?}");
        for x in &entries {
            let rev = f.rev(p);
            f.ok(
                "set_sheet_fields",
                json!({"path": p, "revision": rev, "sheet": x["sheet"], "fields": {"shoot_minutes": 420}}),
            );
        }
        // Day 1 is the last day: a day break at the end gives it a next.
        let t = f.ok("timeline", json!({"path": p, "day": 1}));
        assert_eq!(t["days"][0]["past_wrap"]["next_day"], json!(null));
        let rev = f.rev(p);
        f.ok(
            "insert_day_break",
            json!({"path": p, "revision": rev, "index": 100_000}),
        );
        let t = f.ok("timeline", json!({"path": p, "day": 1}));
        let past = t["days"][0]["past_wrap"].clone();
        assert_eq!(past["wrap_by"], "19:00");
        assert_eq!(past["next_day"], 2);
        let moving = past["strips"].as_array().unwrap().clone();
        assert_eq!(moving.len(), entries.len() - 1);
        assert_eq!(moving[0], entries[1]["strip"]);
        let rev = f.rev(p);
        let (kind, _) = f
            .tool(
                "push_past_wrap",
                json!({"path": p, "revision": rev, "day": 1, "strips": [entries[0]["strip"]]}),
            )
            .unwrap_err();
        assert_eq!(kind, "refused");
        let r = f.ok(
            "push_past_wrap",
            json!({"path": p, "revision": rev, "day": 1, "strips": moving}),
        );
        assert_eq!(r["moved"], json!(moving));
        let t = f.ok("timeline", json!({"path": p, "day": 2}));
        assert_eq!(t["days"][0]["entries"][0]["strip"], moving[0]);
        let rev = f.rev(p);
        f.ok(
            "set_location",
            json!({"path": p, "revision": rev, "name": "Harbor", "latitude": "41.5", "longitude": "-70.6"}),
        );
        let l = f.ok("locations", json!({"path": p}));
        assert!(l["locations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "Harbor"
                && x["utc_offset"] == "-04:00"
                && x["daylight_saving"] == "none"));
        // A daylight saving rule (decision 193), kept when left out.
        let rev = f.rev(p);
        let r = f.ok(
            "set_location",
            json!({"path": p, "revision": rev, "name": "Harbor", "latitude": "41.5", "longitude": "-70.6", "utc_offset": "-5", "daylight_saving": "us_canada"}),
        );
        assert_eq!(r["daylight_saving"], "us_canada");
        let r = f.ok(
            "set_location",
            json!({"path": p, "revision": r["revision"], "name": "Harbor", "latitude": "41.5", "longitude": "-70.6", "utc_offset": "-5"}),
        );
        assert_eq!(r["daylight_saving"], "us_canada");
        let l = f.ok("locations", json!({"path": p}));
        assert!(l["locations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "Harbor" && x["daylight_saving"] == "us_canada"));
        let c = f.ok("call_sheet", json!({"path": p, "day": 1}));
        assert_eq!(c["format"], "method-schedule-callsheet");
        assert_eq!(c["call"], "07:00");
        let rev = f.rev(p);
        f.ok(
            "set_turnaround",
            json!({"path": p, "revision": rev, "category": "Cast", "minimum": "12", "long_day": "11:00"}),
        );
        let r = f.ok("rest_report", json!({"path": p}));
        assert_eq!(r["turnarounds"][0]["minutes"], 720);
        let rev = f.rev(p);
        f.ok(
            "lock_schedule",
            json!({"path": p, "revision": rev, "name": "White"}),
        );
        assert_eq!(
            f.ok("drift", json!({"path": p}))["locks"][0]["name"],
            "White"
        );
        let d = f.ok("drift", json!({"path": p, "lock": "White"}));
        assert!(d["actors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["added"] == json!([])));
        let sug = f.ok("suggest_order", json!({"path": p}));
        assert!(sug["before"]["cast_days"].is_number());
        // A pinned strip keeps its place through the suggestion and its application.
        let days = f.ok("days", json!({"path": p}));
        let order = |d: &serde_json::Value| -> Vec<serde_json::Value> {
            d["days"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|x| {
                    x["strips"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s["strip"].clone())
                })
                .collect()
        };
        let before = order(&days);
        let last = before.last().unwrap().clone();
        let rev = f.rev(p);
        let pin = f.ok(
            "pin_strips",
            json!({"path": p, "revision": rev, "strips": [last]}),
        );
        assert_eq!(pin["count"], 1);
        assert_eq!(pin["changed"], true);
        let again = f.ok(
            "pin_strips",
            json!({"path": p, "revision": pin["revision"], "strips": [last]}),
        );
        assert_eq!(again["changed"], false, "pinning twice changes nothing");
        assert_eq!(f.ok("suggest_order", json!({"path": p}))["pinned"], 1);
        let rev = f.rev(p);
        f.ok("apply_order", json!({"path": p, "revision": rev}));
        let after = order(&f.ok("days", json!({"path": p})));
        assert_eq!(after.len(), before.len());
        let at = |v: &[serde_json::Value]| v.iter().position(|x| *x == last);
        assert_eq!(at(&after), at(&before), "the pinned strip stayed put");
        let rev = f.rev(p);
        let off = f.ok(
            "pin_strips",
            json!({"path": p, "revision": rev, "strips": [last], "pinned": false}),
        );
        assert_eq!(off["count"], 1);
        let (kind, _) = f
            .tool(
                "pin_strips",
                json!({"path": p, "revision": off["revision"], "strips": [999_999]}),
            )
            .unwrap_err();
        assert_eq!(kind, "not_found");
        let rev = f.rev(p);
        let r = f.ok(
            "revise_script",
            json!({"path": p, "revision": rev, "script": "harbor.fountain"}),
        );
        assert_eq!(r["added"], json!([]));
        assert_eq!(r["omitted"], json!([]));
        let rev = f.rev(p);
        let e = f
            .tool(
                "revise_script",
                json!({"path": p, "revision": rev, "script": "harbor.fountain", "tags": true}),
            )
            .unwrap_err();
        assert_eq!(e.0, "invalid_input");
        f.ok("check", json!({"path": p}));
        // Removing the day breaks leaves every strip unscheduled after the last break.
        let rev = f.rev(p);
        let gone = f.ok("remove_day_breaks", json!({"path": p, "revision": rev}));
        assert!(gone["removed"].as_u64().unwrap() > 0);
        assert_eq!(f.ok("days", json!({"path": p}))["days"], json!([]));
        let again = f.ok(
            "remove_day_breaks",
            json!({"path": p, "revision": gone["revision"]}),
        );
        assert_eq!(again["removed"], 0);
        assert_eq!(again["changed"], false);
        f.ok("check", json!({"path": p}));
    }
}
