//! The Plan menu: the features beyond the reference app (decisions.md, IMP-04 and BYD-01
//! to BYD-09). The engine does the work (`method_schedule_core::{plan, revise, timeline,
//! solar, rest, drift, optimize, callsheet}`); this reads what the page typed, turns the
//! results into what ui/js/plan.js draws, and makes every change one undo step through
//! `Session::edit`.
//!
//! Security: script and save paths come only from native dialogs shown here. A picked
//! revision is kept here by token until it is applied or replaced; the page never names
//! a file. Typed values (times, degrees, offsets, hours) are read strictly in Rust.

use crate::view::sub_ref;
use method_schedule_core::callsheet::call_sheet;
use method_schedule_core::drift::{drift, WorkDay};
use method_schedule_core::fountain::ImportedScript;
use method_schedule_core::model::{BoardId, CategoryId, ElementId, StripId, SubBoardId};
use method_schedule_core::optimize::{apply as apply_order_to, suggest, Score};
use method_schedule_core::plan::{
    self as p, clock_text, degrees_text, hours_text, offset_text, parse_clock,
    parse_daylight_saving, parse_degrees, parse_hours, parse_offset, GeoLocation,
};
use method_schedule_core::rest::rest_report;
use method_schedule_core::revise::{revise, revision_text};
use method_schedule_core::solar::Sun;
use method_schedule_core::timeline::{
    past_wrap_in, push_past_wrap as push_strips, scene_name, timelines, EntryKind,
};
use method_schedule_core::Schedule;
use serde::Serialize;

pub fn ids(s: &Schedule, sub: u32) -> Result<(BoardId, SubBoardId), String> {
    let (b, sb) = sub_ref(s, sub)?;
    Ok((b.id, sb.id))
}

pub fn upper(e: impl ToString) -> String {
    let t = e.to_string();
    let mut c = t.chars();
    match c.next() {
        Some(f) => {
            let mut out: String = f.to_uppercase().chain(c).collect();
            if !out.ends_with('.') {
                out.push('.');
            }
            out
        }
        None => String::new(),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RevisionDto {
    pub token: u64,
    pub file: String,
    pub matched: usize,
    pub changed: usize,
    pub added: usize,
    pub omitted: usize,
    /// The report, one line each.
    pub lines: Vec<String>,
}

/// What a revision would do to `s`, worked out on a copy.
pub fn revision_preview(
    s: &Schedule,
    script: &ImportedScript,
    tags: bool,
) -> Result<(usize, usize, usize, usize, Vec<String>), String> {
    let mut copy = s.clone();
    let cast = copy.cast_or_add();
    let r = revise(&mut copy, script, cast, tags).map_err(upper)?;
    let lines = revision_text(&copy, &r)
        .lines()
        .map(str::to_string)
        .collect();
    Ok((
        r.matched.len(),
        r.changed().count(),
        r.added.len(),
        r.omitted.len(),
        lines,
    ))
}

// ---------------------------------------------------------------------------
// The timeline and call times

#[derive(Clone, Debug, Serialize)]
pub struct EntryDto {
    pub strip: u32,
    pub kind: &'static str,
    pub text: String,
    pub start: String,
    pub end: String,
    pub untimed: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DayDto {
    pub number: u32,
    pub date: Option<String>,
    /// The day break that closes the day: call times are kept by it.
    pub day: u32,
    pub call: String,
    pub wrap: String,
    pub length: String,
    pub sun: String,
    pub entries: Vec<EntryDto>,
    pub warnings: Vec<String>,
    /// What runs past the wrap, in words (decision 192); empty when nothing does.
    pub past_wrap: String,
    /// The strips that would move to the next day, as `push_past_wrap` wants them back;
    /// empty when nothing runs past or this is the last day.
    pub push: Vec<u32>,
    /// The day they would move to.
    pub next: Option<u32>,
    /// Who works the day (cast, and any category with a turnaround), with their own call
    /// and wrap where they have one (decision 231).
    pub people: Vec<PersonDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonDto {
    pub element: u32,
    pub name: String,
    /// Their own call and wrap as clock times; empty when they keep the company's.
    pub call: String,
    pub wrap: String,
}

/// The day's people in scene order: cast and any element whose category has a turnaround,
/// then anyone with a call of their own whose scenes have since moved.
fn people(s: &Schedule, t: &method_schedule_core::timeline::DayTimeline) -> Vec<PersonDto> {
    let cast = s.cast();
    let mut els: Vec<ElementId> = Vec::new();
    for e in &t.entries {
        let EntryKind::Scene(id) = e.kind else {
            continue;
        };
        for el in s.sheet(id).map_or(&[][..], |h| &h.elements[..]) {
            let shown = s.element(*el).is_some_and(|x| {
                Some(x.category) == cast || s.plan.turnaround(x.category).is_some()
            });
            if shown && !els.contains(el) {
                els.push(*el);
            }
        }
    }
    for x in s.plan.person_calls.iter().filter(|x| x.day == t.day) {
        if !els.contains(&x.element) {
            els.push(x.element);
        }
    }
    els.into_iter()
        .filter_map(|el| {
            let name = s.element(el)?.name.clone();
            let own = s.plan.person_call(t.day, el);
            Some(PersonDto {
                element: el.0,
                name,
                call: own
                    .and_then(|x| x.call)
                    .map_or(String::new(), |c| clock_text(c as i32)),
                wrap: own
                    .and_then(|x| x.wrap)
                    .map_or(String::new(), |w| t.time_text(w as i32)),
            })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct TimelineDto {
    pub sub_board: u32,
    pub sub_board_name: String,
    pub long_day: String,
    pub days: Vec<DayDto>,
    /// The day holding the strip asked about (its index in `days`), else 0.
    pub focus: usize,
}

fn sun_text(sun: Option<&method_schedule_core::timeline::DaySun>) -> String {
    match sun {
        None => String::new(),
        Some(x) => match x.sun {
            Sun::Times { rise, set } => format!(
                "Sunrise {}, sunset {} at {}",
                clock_text(rise),
                clock_text(set),
                x.location
            ),
            Sun::AlwaysUp => format!("The sun stays up at {}", x.location),
            Sun::AlwaysDown => format!("The sun does not rise at {}", x.location),
        },
    }
}

pub fn timeline_view(s: &Schedule, sub: u32, strip: Option<u32>) -> Result<TimelineDto, String> {
    let (b, sb) = ids(s, sub)?;
    let name = sub_ref(s, sub)?.1.name.clone();
    let mut focus = 0;
    let mut days = Vec::new();
    let all = timelines(s, b, sb).map_err(upper)?;
    for (i, t) in all.iter().enumerate() {
        if strip.is_some_and(|x| t.day.0 == x || t.entries.iter().any(|e| e.strip.0 == x)) {
            focus = i;
        }
        let past = past_wrap_in(s, b, sb, &all, t.number).map_err(upper)?;
        let push = if past.next.is_some() {
            past.strips.iter().map(|x| x.0).collect()
        } else {
            Vec::new()
        };
        days.push(DayDto {
            number: t.number,
            date: t.date.map(|d| d.to_string()),
            day: t.day.0,
            call: t.call.map_or(String::new(), |c| clock_text(c as i32)),
            wrap: if t.call.is_some() {
                t.time_text(t.wrap)
            } else {
                String::new()
            },
            length: hours_text(t.length),
            sun: sun_text(t.sun.as_ref()),
            entries: t
                .entries
                .iter()
                .map(|e| {
                    let (kind, text) = match &e.kind {
                        EntryKind::Scene(id) => (
                            "scene",
                            s.sheet(*id).map_or(String::new(), |h| {
                                format!(
                                    "{} {} {} {}",
                                    scene_name(h),
                                    h.int_ext,
                                    h.set,
                                    h.time_of_day
                                )
                            }),
                        ),
                        EntryKind::Banner(text) => ("banner", text.clone()),
                    };
                    EntryDto {
                        strip: e.strip.0,
                        kind,
                        text,
                        start: t.time_text(e.start),
                        end: t.time_text(e.end),
                        untimed: e.untimed,
                        warnings: e.warnings.clone(),
                    }
                })
                .collect(),
            warnings: t.warnings.clone(),
            past_wrap: if past.strips.is_empty() {
                String::new()
            } else {
                past.text()
            },
            push,
            next: past.next,
            people: people(s, t),
        });
    }
    Ok(TimelineDto {
        sub_board: sub,
        sub_board_name: name,
        long_day: hours_text(s.plan.long_day()),
        days,
        focus,
    })
}

/// Moves what runs past shoot day `day`'s wrap to the start of the next day (decision
/// 192), refused when it is no longer the `strips` previewed. Returns how many moved.
pub fn push_past(s: &mut Schedule, sub: u32, day: u32, strips: &[u32]) -> Result<usize, String> {
    let (b, sb) = ids(s, sub)?;
    let seen: Vec<StripId> = strips.iter().copied().map(StripId).collect();
    push_strips(s, b, sb, day, &seen).map_err(upper)
}

/// Sets the call time of the day closed by `day` (a day break), or clears it when empty.
pub fn set_call(s: &mut Schedule, day: u32, call: &str) -> Result<(), String> {
    let call = call.trim();
    let minutes = if call.is_empty() {
        None
    } else {
        Some(parse_clock(call).map_err(upper)?)
    };
    p::set_call_time(s, StripId(day), minutes).map_err(upper)
}

/// Sets `element`'s own call and wrap on the day `day` closes (decision 231), as typed;
/// empty keeps the company's. A wrap at or before the call is read as after midnight.
pub fn set_own_call(
    s: &mut Schedule,
    day: u32,
    element: u32,
    call: &str,
    wrap: &str,
) -> Result<(), String> {
    let clock = |x: &str| {
        let x = x.trim();
        if x.is_empty() {
            Ok(None)
        } else {
            parse_clock(x).map(Some).map_err(upper)
        }
    };
    let call = clock(call)?;
    let day = StripId(day);
    let wrap = clock(wrap)?.map(|w| match call.or(s.plan.call_time(day)) {
        Some(c) if w <= c => w + 24 * 60,
        _ => w,
    });
    p::set_person_call(s, day, ElementId(element), call, wrap).map_err(upper)
}

/// Records what was shot of `sheet` on the day `day` closes (M5): `eighths` as typed
/// (`3/8`, `1 2/8`; empty for none) and whether that day finished it.
pub fn set_scene_shot(
    s: &mut Schedule,
    sheet: u32,
    day: u32,
    eighths: &str,
    complete: bool,
) -> Result<(), String> {
    let e = eighths.trim();
    let e = if e.is_empty() {
        method_schedule_core::Eighths::ZERO
    } else {
        e.parse::<method_schedule_core::Eighths>()
            .map_err(|_| format!("{e:?} isn't a page length, such as 3/8 or 1 2/8."))?
    };
    p::record_shot(
        s,
        method_schedule_core::model::SheetId(sheet),
        StripId(day),
        e,
        complete,
    )
    .map_err(upper)
}

/// The long day limit as typed (`12`, `10:30`); empty for the default.
pub fn set_long(s: &mut Schedule, value: &str) -> Result<(), String> {
    let v = value.trim();
    let m = if v.is_empty() {
        None
    } else {
        Some(parse_hours(v).map_err(upper)?)
    };
    p::set_long_day(s, m).map_err(upper)
}

// ---------------------------------------------------------------------------
// Locations

#[derive(Clone, Debug, Serialize)]
pub struct LocationDto {
    pub name: String,
    pub sheets: usize,
    pub latitude: String,
    pub longitude: String,
    pub utc_offset: String,
    /// The daylight saving rule's name (`us_canada`, `eu`, `australia`, `new_zealand`, `israel`, `egypt`, `chile`), empty for
    /// none (decisions 193 and 230).
    pub daylight_saving: String,
}

pub fn locations_view(s: &Schedule) -> Vec<LocationDto> {
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
    names
        .into_iter()
        .map(|n| {
            let g = s.plan.location(&n);
            LocationDto {
                sheets: s
                    .sheets
                    .iter()
                    .filter(|h| h.location.trim().eq_ignore_ascii_case(&n))
                    .count(),
                latitude: g.map_or(String::new(), |g| degrees_text(g.latitude)),
                longitude: g.map_or(String::new(), |g| degrees_text(g.longitude)),
                utc_offset: g.map_or(String::new(), |g| offset_text(g.utc_offset)),
                daylight_saving: g
                    .and_then(|g| g.daylight_saving)
                    .map_or(String::new(), |r| r.name().to_string()),
                name: n,
            }
        })
        .collect()
}

/// Places `name` on the map from typed degrees and offset and a daylight saving rule
/// (`us_canada`, `eu`, `australia`, `new_zealand`, `israel`, `egypt`, `chile`, empty for none); both degrees empty takes it off. An empty offset
/// is the longitude's hour.
pub fn place_location(
    s: &mut Schedule,
    name: &str,
    latitude: &str,
    longitude: &str,
    utc_offset: &str,
    daylight_saving: &str,
) -> Result<(), String> {
    if latitude.trim().is_empty() && longitude.trim().is_empty() {
        s.plan.remove_location(name);
        return Ok(());
    }
    if latitude.trim().is_empty() || longitude.trim().is_empty() {
        return Err("Type both a latitude and a longitude, or neither.".to_string());
    }
    let lat = parse_degrees(latitude, 90).map_err(upper)?;
    let lon = parse_degrees(longitude, 180).map_err(upper)?;
    let offset = if utc_offset.trim().is_empty() {
        ((i64::from(lon) * 60 / 15 / 1_000_000) as i32 / 60) * 60
    } else {
        parse_offset(utc_offset).map_err(upper)?
    };
    let daylight_saving = parse_daylight_saving(daylight_saving).map_err(upper)?;
    s.plan
        .set_location(GeoLocation {
            name: name.to_string(),
            latitude: lat,
            longitude: lon,
            utc_offset: offset,
            daylight_saving,
        })
        .map_err(upper)
}

// ---------------------------------------------------------------------------
// Turnarounds and rest

#[derive(Clone, Debug, Serialize)]
pub struct TurnaroundDto {
    pub category: u32,
    pub name: String,
    pub minimum: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RestDto {
    pub turnarounds: Vec<TurnaroundDto>,
    pub short: Vec<String>,
    pub unchecked: usize,
}

pub fn rest_view(s: &Schedule, board: u32) -> Result<RestDto, String> {
    let b = crate::view::board_ref(s, board)?.id;
    let r = rest_report(s, b).map_err(upper)?;
    Ok(RestDto {
        turnarounds: s
            .categories
            .iter()
            .map(|c| TurnaroundDto {
                category: c.id.0,
                name: c.name.clone(),
                minimum: s.plan.turnaround(c.id).map_or(String::new(), hours_text),
            })
            .collect(),
        short: r.short.into_iter().map(|x| x.text).collect(),
        unchecked: r.unchecked,
    })
}

pub fn set_minimum(s: &mut Schedule, category: u32, value: &str) -> Result<(), String> {
    let v = value.trim();
    let m = if v.is_empty() {
        None
    } else {
        Some(parse_hours(v).map_err(upper)?)
    };
    p::set_turnaround(s, CategoryId(category), m).map_err(upper)
}

// ---------------------------------------------------------------------------
// Locks and drift

#[derive(Clone, Debug, Serialize)]
pub struct LockDto {
    pub name: String,
    pub made: String,
    pub scenes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActorDto {
    pub name: String,
    pub board_id: String,
    pub before: usize,
    pub after: usize,
    pub added: String,
    pub dropped: String,
    pub moved: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DriftDto {
    pub lock: String,
    pub actors: Vec<ActorDto>,
    /// Actors with no change, not listed.
    pub unchanged: usize,
}

pub fn drift_view(s: &Schedule, name: &str) -> Result<DriftDto, String> {
    let d = drift(s, name).map_err(upper)?;
    let days = |v: &[WorkDay]| v.iter().map(|w| w.text(s)).collect::<Vec<_>>().join(", ");
    let unchanged = d.actors.iter().filter(|a| a.is_empty()).count();
    Ok(DriftDto {
        lock: d.lock.clone(),
        actors: d
            .changed()
            .map(|a| ActorDto {
                name: a.name.clone(),
                board_id: a.board_id.clone().unwrap_or_default(),
                before: a.before,
                after: a.after,
                added: days(&a.added),
                dropped: days(&a.dropped),
                moved: a
                    .moved
                    .iter()
                    .map(|m| {
                        let scene = s.sheet(m.sheet).map_or_else(
                            || "A scene no longer in the schedule".to_string(),
                            scene_name,
                        );
                        format!("{scene}: {} to {}", m.from.text(s), m.to.text(s))
                    })
                    .collect(),
            })
            .collect(),
        unchanged,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct ScoreRow {
    pub label: &'static str,
    pub before: u32,
    pub after: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuggestionDto {
    pub sub_board: u32,
    pub sub_board_name: String,
    /// Every strip of the sub-board in the suggested order: Apply sends it back.
    pub order: Vec<u32>,
    pub moved: usize,
    pub pinned: usize,
    pub groups: usize,
    pub rows: Vec<ScoreRow>,
}

fn rows(a: &Score, b: &Score) -> Vec<ScoreRow> {
    vec![
        ScoreRow {
            label: "Shoot days",
            before: a.shoot_days,
            after: b.shoot_days,
        },
        ScoreRow {
            label: "Cast work days",
            before: a.cast_days,
            after: b.cast_days,
        },
        ScoreRow {
            label: "Location changes in a day",
            before: a.location_changes,
            after: b.location_changes,
        },
        ScoreRow {
            label: "Red flag clashes",
            before: a.red_flag_clashes,
            after: b.red_flag_clashes,
        },
    ]
}

pub fn suggestion_view(s: &Schedule, sub: u32) -> Result<SuggestionDto, String> {
    let (b, sb) = ids(s, sub)?;
    let sug = suggest(s, b, sb).map_err(upper)?;
    Ok(SuggestionDto {
        sub_board: sub,
        sub_board_name: sub_ref(s, sub)?.1.name.clone(),
        order: sug.order.iter().map(|x| x.0).collect(),
        moved: sug.moved,
        pinned: sug.pinned,
        groups: sug.groups,
        rows: rows(&sug.before, &sug.after),
    })
}

/// Puts the previewed order on the board, refused when it is no longer the suggestion
/// (the board changed since the preview).
pub fn apply_order(s: &mut Schedule, sub: u32, order: &[u32]) -> Result<usize, String> {
    let (b, sb) = ids(s, sub)?;
    let sug = suggest(s, b, sb).map_err(upper)?;
    if sug.order.iter().map(|x| x.0).collect::<Vec<_>>() != order {
        return Err("The board changed since the preview; suggest the order again.".to_string());
    }
    apply_order_to(s, &sug).map_err(upper)?;
    Ok(sug.moved)
}

// ---------------------------------------------------------------------------
// Call sheet data

/// The call sheet file's bytes for shoot day `day` of a sub-board: JSON or CSV.
pub fn call_sheet_bytes(s: &Schedule, sub: u32, day: u32, csv: bool) -> Result<Vec<u8>, String> {
    let (b, sb) = ids(s, sub)?;
    let c = call_sheet(s, b, sb, day).map_err(upper)?;
    Ok(if csv {
        method_schedule_format::callsheet::to_csv(&c)
    } else {
        method_schedule_format::callsheet::to_json(&c).into_bytes()
    })
}
