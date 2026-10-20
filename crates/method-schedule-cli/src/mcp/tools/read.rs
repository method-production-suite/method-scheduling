//! Tools that read: listing schedules, summaries, sheets, elements, shoot days, the day
//! out of days, the one-line schedule, the budget exchange file, the file check, what
//! changed between two schedules, why a day out of days count is what it is, find,
//! conflicts between sub-boards, and red flag clashes.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use method_schedule_core::conflicts::{conflicts as board_conflicts, Booking};
use method_schedule_core::days::days as shoot_days;
use method_schedule_core::diff::{diff as diff_schedules, Subject};
use method_schedule_core::dood::Gap;
use method_schedule_core::dood::{dood as dood_rows, summary};
use method_schedule_core::explain::explain as explain_count;
use method_schedule_core::find::find as find_places;
use method_schedule_core::model::{
    DayKind, Element, ElementId, EventKind, Sheet, Strip, StripKind, SubBoard,
};
use method_schedule_core::redflags::red_flags as sub_board_red_flags;
use method_schedule_core::reports::{one_line as one_line_report, OneLine};
use method_schedule_core::{Date, Eighths, Schedule};
use serde_json::{json, Value};

use super::super::files::{self, load, load_from};
use super::super::limits;
use super::super::paths::{READ_SCHEDULE_EXTS, SCHEDULE_EXTS};
use super::super::schema::{boolean, integer, name, object, one_of, path};
use super::{
    board_prop, date_json, date_prop, id_prop, location_json, path_prop, pick, pick_board, quoted,
    red_flag_json, search_props, search_query, sub_board_prop, Args, Ctx, Tool, ToolError, ToolOut,
    ToolResult,
};

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "list_schedules",
            title: "List schedules",
            description: "The schedule files under the server's roots (or one folder inside \
                          them): every .mtsc file, and .json files that are schedules. \
                          Hidden folders and symlinks are skipped. Returns each file's path \
                          (to pass to the other tools) and size in bytes.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        (
                            "folder",
                            path("A folder inside a root. Default: every root."),
                        ),
                        (
                            "recursive",
                            boolean("Look in subfolders too (8 deep at most). Default true."),
                        ),
                    ],
                    &[],
                )
            },
            run: list_schedules,
        },
        Tool {
            name: "read_schedule",
            title: "Read a schedule",
            description: "A schedule at a glance, and its revision (which every change \
                          needs): title, counts, categories, each board with its Boneyard \
                          and sub-boards (start date, shoot days, strips scheduled and not), \
                          and the sheets in creation order (500 at a time by default).",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        (
                            "offset",
                            integer("Skip this many sheets. Default 0.", 0, i64::from(u32::MAX)),
                        ),
                        (
                            "limit",
                            integer(
                                "Most sheets to list. Default 500.",
                                0,
                                limits::MAX_PAGE as i64,
                            ),
                        ),
                    ],
                    &["path"],
                )
            },
            run: read_schedule,
        },
        Tool {
            name: "get_sheet",
            title: "Get a breakdown sheet",
            description: "One breakdown sheet: every header field, its elements (id, \
                          category, name, board ID) and where its strip is on each board \
                          (sub-board or Boneyard, position, strip id, shoot day).",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![path_prop(), ("sheet", id_prop("The sheet id."))],
                    &["path", "sheet"],
                )
            },
            run: get_sheet,
        },
        Tool {
            name: "list_elements",
            title: "List elements",
            description: "The categories (with their default hold and drop rules) and the \
                          elements: id, category, name, board ID, lock, rules, notes, links \
                          and how many sheets each is on.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        ("category", name("Only this category, by name.")),
                    ],
                    &["path"],
                )
            },
            run: list_elements,
        },
        Tool {
            name: "days",
            title: "Shoot days",
            description: "A sub-board's shoot days in board order: number, date (when the \
                          sub-board has a start date), pages in eighths, minutes, screen \
                          seconds, the day break closing it, and each strip (sheet strips \
                          with their scene, set and length; banners with their text). Also \
                          the strips after the last day break, the board's Boneyard and the \
                          sub-board's calendar (days off, holidays, travel days, events). \
                          Strip ids here are what move_strips and delete_day_break take.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop(), board_prop(), sub_board_prop()], &["path"]),
            run: days,
        },
        Tool {
            name: "dood",
            title: "Day out of days",
            description: "The day out of days of a sub-board (it needs a start date): per \
                          element, the code on every date from its first to its last day \
                          (SW, W, WF, SWF, H, TR, FT, R, /, M, and empty for a day that \
                          counts nowhere; an event on a work day shows the event's code and \
                          then the whole work code, FTW or FTPW, and carries conflict: true, \
                          unless the element is left off the board; codes in the usual \
                          letters, whatever the schedule's own), and the summary: \
                          work, hold, start, finish and total (work plus hold). With header \
                          (set, location, unit, episode, int_ext, time_of_day, script_day or \
                          sequence), one row per value that header field takes on the sheets \
                          instead.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        board_prop(),
                        sub_board_prop(),
                        ("category", name("Only this category, by name.")),
                        (
                            "header",
                            one_of(
                                "A header field as a category: a row per value it takes.",
                                &[
                                    "set",
                                    "location",
                                    "unit",
                                    "episode",
                                    "int_ext",
                                    "time_of_day",
                                    "script_day",
                                    "sequence",
                                ],
                            ),
                        ),
                    ],
                    &["path"],
                )
            },
            run: dood,
        },
        Tool {
            name: "one_line",
            title: "One-line schedule",
            description: "The one-line schedule of a sub-board: each shoot day with its date \
                          and its strips in board order, scenes with their cast by board ID \
                          (or name), banners with their minutes, and the day's pages and \
                          minutes. With `columns`, also the one-line export's first sheet as \
                          `table`: `layout` follows the report's strip layout as the \
                          reference app's spreadsheet export does, `fixed` has the stable day to text \
                          columns.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "columns",
                            one_of(
                                "Add `table`, the one-line export's first sheet with these \
                                 columns.",
                                &["layout", "fixed"],
                            ),
                        ),
                    ],
                    &["path"],
                )
            },
            run: one_line,
        },
        Tool {
            name: "budget_exchange",
            title: "Budget exchange file",
            description: "What a budget needs from a sub-board, as the documented budget \
                          exchange JSON (docs/EXCHANGE.md): shoot days by unit, set, \
                          location and week, and each element's work, hold, travel, \
                          fitting, rehearsal, holiday and company move days, kept apart. \
                          The sub-board needs a start date.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "generated",
                            date_prop("The date to stamp the file with; left out when not given."),
                        ),
                    ],
                    &["path"],
                )
            },
            run: budget_exchange,
        },
        Tool {
            name: "check",
            title: "Check a schedule",
            description: "Whether the file reads as a schedule and hangs together (the strict \
                          reader and Schedule::check, as method-schedule check does): ok, and the \
                          problem when it doesn't; whether the file is in canonical form; \
                          its revision.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop()], &["path"]),
            run: check,
        },
        Tool {
            name: "diff",
            title: "What changed",
            description: "What changed from one schedule (path, the older) to another (other, \
                          the newer), matched by id, never by position: the title, \
                          categories, elements and sheets added, removed or changed field by \
                          field, boards and sub-boards, calendars (start, weekly days off, \
                          marked dates, events) and strips added, removed, changed or moved \
                          (each strip that changed place, not the whole order). Each change \
                          has its kind, what it is about, its fields before and after, and \
                          one plain sentence.",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        (
                            "path",
                            path("The older schedule: relative to the first root, or absolute inside a root."),
                        ),
                        (
                            "other",
                            path("The newer schedule, the same way."),
                        ),
                    ],
                    &["path", "other"],
                )
            },
            run: diff,
        },
        Tool {
            name: "explain",
            title: "Explain a day out of days count",
            description: "Why one element's day out of days totals on a sub-board are what \
                          they are (it needs a start date): work, hold and total; what work \
                          is made of (shoot days, events, holidays, company travel days); \
                          every gap between work days with its length against the minimum \
                          and whether it was dropped or held; and every date of the row with \
                          its code, where it counts and why, its shoot day, sheets and \
                          events. Name the element by id, or by name (and category when the \
                          name is in more than one).",
            mutating: false,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        ("element", id_prop("The element id.")),
                        (
                            "name",
                            name("The element's name, ignoring case (instead of element)."),
                        ),
                        ("category", name("The category of a named element.")),
                        board_prop(),
                        sub_board_prop(),
                    ],
                    &["path"],
                )
            },
            run: explain,
        },
        Tool {
            name: "find",
            title: "Find",
            description: "Searches a schedule's text: sheet header fields, element names, \
                          category names, banner text and event names (or one of them), \
                          ignoring case unless match_case, optionally whole words, in the \
                          whole schedule or one sub-board. Returns each place in board \
                          order, numbered from 1 (the numbers replace takes), with its kind, \
                          field, ids, whole text and a short context line.",
            mutating: false,
            destructive: false,
            schema: || {
                let mut props = vec![path_prop()];
                props.extend(search_props());
                props.push((
                    "limit",
                    integer(
                        "Most results to return. Default 500.",
                        1,
                        limits::MAX_PAGE as i64,
                    ),
                ));
                object(props, &["path", "text"])
            },
            run: find,
        },
        Tool {
            name: "conflicts",
            title: "Conflicts",
            description: "Elements scheduled on the same date in two or more sub-boards of \
                          one board (other boards are never compared): each element and date \
                          with the sub-boards, shoot days and sheets involved. Dates come from \
                          each sub-board's start date, calendar and day breaks; sub-boards \
                          without a start date are listed as not compared. Also conflicts: \
                          a travel day, fitting or rehearsal on a date the element works on \
                          the same sub-board (events on work days). Apart and informational \
                          only: event clashes, an element with two events on a date, or an \
                          event in one sub-board and work in another.",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop(), board_prop()], &["path"]),
            run: conflicts,
        },
        Tool {
            name: "red_flags",
            title: "Red flags",
            description: "Red flags: dates an element can't (hard) or would rather not \
                          (soft) work. Returns the schedule's red flag types, every red flag \
                          (element, type, dates as from and to, or not_before, or not_after, \
                          note, hard), and the clashes on one sub-board: each date an element \
                          works (strips above a day break, dated from the start date and \
                          calendar) or has a counted event (travel, fitting, rehearsal) that \
                          one of its flags rules out, with the shoot day, sheets and events. \
                          Warnings only; ordered by date. Without a start date only events \
                          are checked (undated is true).",
            mutating: false,
            destructive: false,
            schema: || object(vec![path_prop(), board_prop(), sub_board_prop()], &["path"]),
            run: red_flags,
        },
    ]
}

// ---- listing

fn list_schedules(ctx: &Ctx, a: &Args) -> ToolResult {
    let starts: Vec<PathBuf> = match a.str("folder") {
        Some(f) => vec![ctx.roots.folder(Some(f))?],
        None => ctx.roots.dirs().to_vec(),
    };
    let recursive = a.opt_bool("recursive").unwrap_or(true);
    let mut found = Vec::new();
    let mut truncated = false;
    for start in &starts {
        walk(start, 0, recursive, &mut found, &mut truncated);
    }
    found.sort();
    found.dedup();
    let files: Vec<Value> = found
        .iter()
        .map(|(p, len)| json!({"path": ctx.roots.show(p), "bytes": len}))
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} schedule file(s){}.",
            files.len(),
            if truncated {
                " (the list stops there)"
            } else {
                ""
            }
        ),
        data: json!({"files": files, "truncated": truncated}),
    })
}

/// Collects schedule files under `dir`, skipping hidden names and symlinks.
fn walk(dir: &Path, depth: usize, recursive: bool, out: &mut Vec<(PathBuf, u64)>, full: &mut bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= limits::MAX_LISTED_FILES {
            *full = true;
            return;
        }
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        // `DirEntry::file_type` does not follow symlinks, so a link is neither.
        let Ok(t) = e.file_type() else { continue };
        let p = e.path();
        if t.is_dir() {
            if recursive && depth + 1 < limits::MAX_LIST_DEPTH {
                walk(&p, depth + 1, recursive, out, full);
            }
        } else if t.is_file() && looks_like_schedule(&p) {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((p, len));
        }
    }
}

/// A `.mtsc` file, or a `.json` file whose first bytes name the schedule format.
fn looks_like_schedule(p: &Path) -> bool {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("mtsc" | "mtst") => true,
        Some(e) if SCHEDULE_EXTS.contains(&e) => {
            let mut head = Vec::new();
            std::fs::File::open(p)
                .and_then(|f| f.take(256).read_to_end(&mut head))
                .is_ok()
                && String::from_utf8_lossy(&head).contains("\"format\": \"method-schedule\"")
        }
        _ => false,
    }
}

// ---- JSON for the model's parts

fn pages(e: Eighths) -> Value {
    json!(e.to_string())
}

/// A sheet's header fields (everything but its elements' details).
pub fn sheet_json(h: &Sheet) -> Value {
    json!({
        "id": h.id.0,
        "number": h.number,
        "scenes": h.scenes,
        "int_ext": h.int_ext,
        "set": h.set,
        "time_of_day": h.time_of_day,
        "sunrise": h.sunrise,
        "sunset": h.sunset,
        "eighths": h.pages.0,
        "pages": pages(h.pages),
        "shoot_minutes": h.shoot_time.map(|m| m.0),
        "synopsis": h.synopsis,
        "location": h.location,
        "script_pages": h.script_pages,
        "script_day": h.script_day,
        "screen_seconds": h.screen_time.map(|s| s.0),
        "unit": h.unit,
        "sequence": h.sequence,
        "episode": h.episode,
        "comments": h.comments,
        "elements": h.elements.iter().map(|e| e.0).collect::<Vec<_>>(),
        "quantities": h.quantities.iter().map(|q| json!({
            "element": q.element.0,
            "count": q.count,
            "daily": q.daily,
        })).collect::<Vec<_>>(),
        "images": h.images,
    })
}

/// A sheet in a list: enough to recognize it.
fn sheet_brief(h: &Sheet) -> Value {
    json!({
        "id": h.id.0,
        "scenes": h.scenes,
        "int_ext": h.int_ext,
        "set": h.set,
        "time_of_day": h.time_of_day,
        "eighths": h.pages.0,
        "pages": pages(h.pages),
        "elements": h.elements.len(),
    })
}

fn element_json(s: &Schedule, e: &Element) -> Value {
    json!({
        "id": e.id.0,
        "category": e.category.0,
        "category_name": s.category(e.category).map_or("", |c| c.name.as_str()),
        "name": e.name,
        "board_id": e.board_id,
        "board_id_locked": e.board_id_locked,
        "exclude_from_board": e.exclude_from_board,
        "excluded": s.element_excluded(e),
        "rules": {"holds": e.rules.holds, "drops": e.rules.drops, "min_drop_gap": e.rules.min_drop_gap},
        "notes": e.notes,
        "properties": e.properties.iter().map(|p| json!({"name": p.name, "value": p.value})).collect::<Vec<_>>(),
        "links": e.links.iter().map(|l| l.0).collect::<Vec<_>>(),
        "images": e.images,
        "sheets": s.sheets.iter().filter(|h| h.elements.contains(&e.id)).count(),
    })
}

/// A strip: its id and kind, and for a sheet strip the sheet's heading and length.
fn strip_json(s: &Schedule, st: &Strip) -> Value {
    match &st.kind {
        StripKind::Sheet(id) => {
            let h = s.sheet(*id);
            json!({
                "strip": st.id.0,
                "kind": "sheet",
                "sheet": id.0,
                "scenes": h.map_or("", |h| h.scenes.as_str()),
                "int_ext": h.map_or("", |h| h.int_ext.as_str()),
                "set": h.map_or("", |h| h.set.as_str()),
                "time_of_day": h.map_or("", |h| h.time_of_day.as_str()),
                "eighths": h.map_or(0, |h| h.pages.0),
            })
        }
        StripKind::Banner(b) => json!({
            "strip": st.id.0,
            "kind": "banner",
            "text": b.text,
            "minutes": b.duration.0,
            "style": crate::banners::style_json(&b.style),
        }),
        StripKind::DayBreak => json!({"strip": st.id.0, "kind": "day_break"}),
    }
}

fn calendar_json(sb: &SubBoard) -> Value {
    let c = &sb.calendar;
    let dates = |v: &[Date]| v.iter().map(|d| d.to_string()).collect::<Vec<_>>();
    let events: Vec<Value> = c
        .events
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let kind = ev.kind.key();
            let label = match &ev.kind {
                EventKind::Other(l) => l.as_str(),
                _ => "",
            };
            json!({
                "index": i,
                "kind": kind,
                "label": label,
                "name": ev.name,
                "from": ev.from.to_string(),
                "to": ev.to.to_string(),
                "elements": ev.elements.iter().map(|e| e.0).collect::<Vec<_>>(),
                "color": ev
                    .color
                    .and_then(|c| method_schedule_core::model::EVENT_COLORS.get(usize::from(c)))
                    .map(|(k, _, _)| *k),
                "counts": ev.kind.code().is_some(),
            })
        })
        .collect();
    json!({
        "weekly_off": c.weekly_off.iter().map(|w| format!("{w:?}").to_ascii_lowercase()).collect::<Vec<_>>(),
        "work_days": dates(&c.work_days),
        "off_days": dates(&c.off_days),
        "holidays": dates(&c.holidays),
        "moves": dates(&c.moves),
        "events": events,
    })
}

/// The kind of date `d` is on a calendar, as the tools name it.
pub fn day_kind_name(k: DayKind) -> &'static str {
    match k {
        DayKind::Work => "work",
        DayKind::Off => "off",
        DayKind::Holiday => "holiday",
        DayKind::Move => "move",
    }
}

// ---- the tools

fn read_schedule(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let offset = a.u64("offset").unwrap_or(0) as usize;
    let limit = a.u64("limit").unwrap_or(500) as usize;
    let total: Eighths = s.sheets.iter().map(|h| h.pages).sum();
    let categories: Vec<Value> = s
        .categories
        .iter()
        .map(|c| {
            json!({
                "id": c.id.0,
                "name": c.name,
                "elements": s.elements.iter().filter(|e| e.category == c.id).count(),
            })
        })
        .collect();
    let mut boards = Vec::new();
    let mut day_count = 0;
    for b in &s.boards {
        let mut subs = Vec::new();
        for sb in &b.sub_boards {
            let d = shoot_days(s, b.id, sb.id)?;
            if b.id == s.boards[0].id && sb.id == b.sub_boards[0].id {
                day_count = d.days.len();
            }
            let end = match (sb.start, d.days.last()) {
                (Some(st), Some(last)) => sb.calendar.date_of_day(st, last.number),
                _ => None,
            };
            subs.push(json!({
                "id": sb.id.0,
                "name": sb.name,
                "start": date_json(sb.start),
                "end": date_json(end),
                "shoot_days": d.days.len(),
                "strips": sb.strips.len(),
                "unscheduled": d.unscheduled.len(),
                "events": sb.calendar.events.len(),
            }));
        }
        boards.push(json!({
            "id": b.id.0,
            "name": b.name,
            "boneyard": b.boneyard.len(),
            "sub_boards": subs,
        }));
    }
    let sheets: Vec<Value> = s
        .sheets
        .iter()
        .skip(offset)
        .take(limit)
        .map(sheet_brief)
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} {}: {} sheets, {total} pages, {} elements; {day_count} shoot days on the \
             first sub-board. Revision {}.",
            l.shown,
            quoted(&s.title),
            s.sheets.len(),
            s.elements.len(),
            l.revision
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "canonical": l.canonical,
            "template": l.template,
            "title": s.title,
            "counts": {
                "categories": s.categories.len(),
                "elements": s.elements.len(),
                "sheets": s.sheets.len(),
                "boards": s.boards.len(),
            },
            "eighths": total.0,
            "pages": pages(total),
            "categories": categories,
            "boards": boards,
            "sheets_offset": offset,
            "sheets": sheets,
        }),
    })
}

fn get_sheet(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let id = method_schedule_core::model::SheetId(a.req_u32("sheet")?);
    let h = s
        .sheet(id)
        .ok_or_else(|| ToolError::not_found(format!("no sheet {}", id.0)))?;
    let elements: Vec<Value> = h
        .elements
        .iter()
        .filter_map(|e| s.element(*e))
        .map(|e| {
            json!({
                "id": e.id.0,
                "category": s.category(e.category).map_or("", |c| c.name.as_str()),
                "name": e.name,
                "board_id": e.board_id,
            })
        })
        .collect();
    let mut places = Vec::new();
    for b in &s.boards {
        let Some(p) = s.sheet_place(b.id, id) else {
            continue;
        };
        let (strip, day) = match p.sub_board {
            Some(sub) => {
                let sb = b.sub_boards.iter().find(|x| x.id == sub);
                let strip = sb.and_then(|sb| sb.strips.get(p.index)).map(|st| st.id);
                let d = shoot_days(s, b.id, sub)?;
                let day = strip.and_then(|st| {
                    d.days
                        .iter()
                        .find(|day| day.strips.contains(&st))
                        .map(|day| day.number)
                });
                (strip, day)
            }
            None => (b.boneyard.get(p.index).map(|st| st.id), None),
        };
        places.push(json!({
            "board": b.id.0,
            "sub_board": p.sub_board.map(|x| x.0),
            "index": p.index,
            "strip": strip.map(|x| x.0),
            "day": day,
        }));
    }
    let mut data = sheet_json(h);
    data["element_details"] = json!(elements);
    data["places"] = json!(places);
    data["path"] = json!(l.shown);
    data["revision"] = json!(l.revision);
    Ok(ToolOut {
        summary: format!(
            "Sheet {}: scene {} {} {} {}, {} pages, {} element(s).",
            id.0,
            quoted(&h.scenes),
            quoted(&h.int_ext),
            quoted(&h.set),
            quoted(&h.time_of_day),
            h.pages,
            h.elements.len()
        ),
        data,
    })
}

fn list_elements(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let only = match a.str("category") {
        Some(n) => Some(
            s.category_named(n)
                .ok_or_else(|| ToolError::not_found(format!("no category {}", quoted(n))))?,
        ),
        None => None,
    };
    let categories: Vec<Value> = s
        .categories
        .iter()
        .filter(|c| only.is_none_or(|o| o == c.id))
        .map(|c| {
            json!({
                "id": c.id.0,
                "name": c.name,
                "auto_board_id": c.auto_board_id,
                "exclude_from_board": c.exclude_from_board,
                "daily_quantity": c.daily_quantity,
                "properties": c.properties,
                "multi_line": c.multi_line,
                "defaults": {"holds": c.defaults.holds, "drops": c.defaults.drops, "min_drop_gap": c.defaults.min_drop_gap},
            })
        })
        .collect();
    let elements: Vec<Value> = s
        .elements
        .iter()
        .filter(|e| only.is_none_or(|o| o == e.category))
        .map(|e| element_json(s, e))
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} element(s) in {} categor{}.",
            elements.len(),
            categories.len(),
            if categories.len() == 1 { "y" } else { "ies" }
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "categories": categories,
            "elements": elements,
        }),
    })
}

fn days(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (bid, sid) = pick(s, a)?;
    let b = s.board(bid).expect("picked");
    let sb = b.sub_boards.iter().find(|x| x.id == sid).expect("picked");
    let d = shoot_days(s, bid, sid)?;
    let find = |id| sb.strips.iter().find(|st| st.id == id);
    let list: Vec<Value> = d
        .days
        .iter()
        .map(|day| {
            let date = sb.start.and_then(|st| sb.calendar.date_of_day(st, day.number));
            json!({
                "number": day.number,
                "date": date_json(date),
                "eighths": day.pages.0,
                "pages": pages(day.pages),
                "minutes": day.time.0,
                "screen_seconds": day.screen_time.0,
                "day_break": day.break_id.0,
                "strips": day.strips.iter().filter_map(|id| find(*id)).map(|st| strip_json(s, st)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let unscheduled: Vec<Value> = d
        .unscheduled
        .iter()
        .filter_map(|id| find(*id))
        .map(|st| strip_json(s, st))
        .collect();
    let boneyard: Vec<Value> = b.boneyard.iter().map(|st| strip_json(s, st)).collect();
    Ok(ToolOut {
        summary: format!(
            "{} shoot day(s) on {} of {}{}; {} strip(s) after the last day break, {} in the \
             Boneyard.",
            list.len(),
            quoted(&sb.name),
            quoted(&b.name),
            sb.start.map_or(" (no start date)".to_string(), |d| format!(
                ", starting {d}"
            )),
            unscheduled.len(),
            boneyard.len()
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "board": bid.0,
            "sub_board": sid.0,
            "start": date_json(sb.start),
            "days": list,
            "unscheduled": unscheduled,
            "boneyard": boneyard,
            "calendar": calendar_json(sb),
        }),
    })
}

fn dood(ctx: &Ctx, a: &Args) -> ToolResult {
    use method_schedule_core::reports::{header_category, HeaderField};
    let l = load(ctx, a)?;
    let (bid, sid) = pick(&l.schedule, a)?;
    // A header field works as a category made from the sheets (inventory section 4).
    let hc = a
        .str("header")
        .and_then(HeaderField::from_key)
        .map(|f| header_category(&l.schedule, f));
    let s = hc.as_ref().map_or(&l.schedule, |h| &h.schedule);
    let only = match (&hc, a.str("category")) {
        (Some(h), _) => Some(h.category),
        (None, Some(n)) => Some(
            s.category_named(n)
                .ok_or_else(|| ToolError::not_found(format!("no category {}", quoted(n))))?,
        ),
        (None, None) => None,
    };
    let rows: Vec<_> = dood_rows(s, bid, sid)?
        .into_iter()
        .filter(|r| only.is_none_or(|c| s.element(r.element).is_some_and(|e| e.category == c)))
        .collect();
    let who = |id| {
        s.element(id).map_or(
            (Value::Null, String::new(), String::new()),
            |e: &Element| {
                (
                    json!(e.board_id),
                    e.name.clone(),
                    s.category(e.category)
                        .map_or(String::new(), |c| c.name.clone()),
                )
            },
        )
    };
    let out_rows: Vec<Value> = rows
        .iter()
        .map(|r| {
            let (board_id, name, category) = who(r.element);
            json!({
                "element": r.element.0,
                "category": category,
                "board_id": board_id,
                "name": name,
                "start": date_json(r.start),
                "finish": date_json(r.finish),
                "worked": r.worked,
                "held": r.held,
                "cells": r.cells.iter().map(|(d, c)| {
                    let mut cell = json!({"date": d.to_string(), "code": c.code()});
                    // An event on a work day: flagged, and a conflict (behavior note 17),
                    // unless the element is left off the board (decision 120).
                    if c.is_flagged() && !s.excluded(r.element) {
                        cell["conflict"] = json!(true);
                    }
                    cell
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    let out_summary: Vec<Value> = summary(&rows)
        .iter()
        .map(|r| {
            let (board_id, name, category) = who(r.element);
            json!({
                "element": r.element.0,
                "category": category,
                "board_id": board_id,
                "name": name,
                "work": r.work,
                "hold": r.hold,
                "start": date_json(r.start),
                "finish": date_json(r.finish),
                "total": r.total,
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!("Day out of days: {} element(s).", out_rows.len()),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "board": bid.0,
            "sub_board": sid.0,
            "rows": out_rows,
            "summary": out_summary,
        }),
    })
}

fn one_line(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (bid, sid) = pick(s, a)?;
    let report = one_line_report(s, bid, sid, s.cast())?;
    // The element columns of the one-line export: the categories the schedule's one-line
    // report shows, each with the scene's elements and their quantities on it.
    let o = method_schedule_format::export::one_line_options(s);
    let elements = |sheet: method_schedule_core::model::SheetId| -> Vec<Value> {
        let Some(h) = s.sheet(sheet) else {
            return Vec::new();
        };
        method_schedule_core::report_data::elements_by_category(s, &h.elements)
            .into_iter()
            .filter(|(c, _)| o.shows_category(*c))
            .map(|(c, els)| {
                json!({
                    "category": s.category(c).map_or("", |x| x.name.as_str()),
                    "elements": els.iter().filter_map(|e| s.element(*e)).map(|e| json!({
                        "element": e.id.0,
                        "board_id": e.board_id,
                        "name": e.name,
                        "quantity": h.quantity(e.id).0,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect()
    };
    let out: Vec<Value> = report
        .iter()
        .map(|day| {
            let lines: Vec<Value> = day
                .lines
                .iter()
                .map(|line| match line {
                    OneLine::Scene {
                        sheet,
                        scenes,
                        int_ext,
                        set,
                        time_of_day,
                        pages: p,
                        synopsis,
                        cast,
                    } => {
                        let mut line = json!({
                            "kind": "scene",
                            "scenes": scenes,
                            "int_ext": int_ext,
                            "set": set,
                            "time_of_day": time_of_day,
                            "eighths": p.0,
                            "pages": pages(*p),
                            "synopsis": synopsis,
                            "cast": cast,
                            "elements": elements(*sheet),
                        });
                        // A scene a script revision left out: listed, counting nowhere
                        // (decision 91).
                        if s.plan.is_omitted(*sheet) {
                            line["omitted"] = json!(true);
                        }
                        line
                    }
                    OneLine::Banner {
                        text,
                        duration,
                        style,
                    } => json!({
                        "kind": "banner",
                        "text": text,
                        "minutes": duration.0,
                        "style": crate::banners::style_json(style),
                    }),
                })
                .collect();
            json!({
                "number": day.number,
                "date": date_json(day.date),
                "eighths": day.pages.0,
                "pages": pages(day.pages),
                "minutes": day.time.0,
                "lines": lines,
            })
        })
        .collect();
    let mut data = json!({
        "path": l.shown,
        "revision": l.revision,
        "board": bid.0,
        "sub_board": sid.0,
        "days": out,
    });
    if let Some(c) = a.str("columns") {
        use method_schedule_format::export::{one_line as one_line_sheet, Cell, OneLineColumns};
        let columns = OneLineColumns::from_name(c)
            .ok_or_else(|| ToolError::invalid("`columns` is layout or fixed"))?;
        let wb = one_line_sheet(s, bid, sid, s.cast(), columns)
            .map_err(|e| ToolError::invalid(e.to_string()))?;
        let sheet = &wb.sheets[0];
        let cell = |c: &Cell| -> Value {
            match c {
                Cell::Empty => Value::Null,
                Cell::Text(t) => json!(t),
                Cell::Int(n) => json!(n),
                Cell::Date(d) => json!(d.to_string()),
                Cell::Formula(f) => json!(f.value()),
            }
        };
        data["table"] = json!({
            "columns": sheet.columns,
            "rows": sheet.rows.iter().map(|r| r.iter().map(cell).collect::<Vec<_>>()).collect::<Vec<_>>(),
        });
    }
    Ok(ToolOut {
        summary: format!("One-line schedule: {} shoot day(s).", out.len()),
        data,
    })
}

fn budget_exchange(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (bid, sid) = pick(s, a)?;
    let generated = a.date("generated")?;
    let text = method_schedule_format::exchange::to_string(s, bid, sid, generated)?;
    let exchange: Value =
        serde_json::from_str(&text).map_err(|e| ToolError::io(format!("exchange file: {e}")))?;
    Ok(ToolOut {
        summary: format!(
            "Budget exchange for board {}: {} shoot day(s), {} element(s).",
            bid.0,
            exchange["shoot_days"]["total"],
            exchange["elements"].as_array().map_or(0, Vec::len)
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "exchange": exchange,
        }),
    })
}

fn check(ctx: &Ctx, a: &Args) -> ToolResult {
    let typed = a.req("path")?;
    let real = ctx.roots.existing(typed, READ_SCHEDULE_EXTS)?;
    let shown = ctx.roots.show(&real);
    let bytes = files::read_file(&real, &shown)?;
    let revision = files::revision(&bytes);
    let (ok, problem, canonical) = match files::parse(&bytes, &shown) {
        Ok(s) => (true, Value::Null, files::is_canonical(&s, &bytes)),
        Err(e) => (false, json!(e.message), false),
    };
    Ok(ToolOut {
        summary: if ok {
            format!(
                "{shown}: ok{}.",
                if canonical {
                    ""
                } else {
                    " (not in canonical form)"
                }
            )
        } else {
            format!("{shown}: {}", problem.as_str().unwrap_or_default())
        },
        data: json!({
            "path": shown,
            "revision": revision,
            "ok": ok,
            "problem": problem,
            "canonical": canonical,
        }),
    })
}

// ---- versions and explanations

fn subject_json(subject: &Subject) -> Value {
    match *subject {
        Subject::Title => json!({"kind": "title"}),
        Subject::Setup => json!({"kind": "setup"}),
        Subject::StripColors => json!({"kind": "strip_colors"}),
        Subject::Layouts => json!({"kind": "layouts"}),
        Subject::Category(c) => json!({"kind": "category", "category": c.0}),
        Subject::RedFlagType(t) => json!({"kind": "red_flag_type", "red_flag_type": t.0}),
        Subject::Element(e) => json!({"kind": "element", "element": e.0}),
        Subject::RedFlag(e, f) => json!({"kind": "red_flag", "element": e.0, "red_flag": f.0}),
        Subject::Sheet(s) => json!({"kind": "sheet", "sheet": s.0}),
        Subject::Board(b) => json!({"kind": "board", "board": b.0}),
        Subject::SubBoard(b, s) => json!({"kind": "sub_board", "board": b.0, "sub_board": s.0}),
        Subject::Calendar(b, s) => json!({"kind": "calendar", "board": b.0, "sub_board": s.0}),
        Subject::Strip(b, s) => json!({"kind": "strip", "board": b.0, "strip": s.0}),
    }
}

fn diff(ctx: &Ctx, a: &Args) -> ToolResult {
    let old = load(ctx, a)?;
    let new = load_from(ctx, a, "other")?;
    let d = diff_schedules(&old.schedule, &new.schedule);
    let changes: Vec<Value> = d
        .changes
        .iter()
        .map(|c| {
            json!({
                "kind": c.kind.key(),
                "subject": subject_json(&c.subject),
                "fields": c.fields.iter().map(|f| json!({"field": f.field, "before": f.before, "after": f.after})).collect::<Vec<_>>(),
                "text": c.text,
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} change(s) from {} to {}.",
            changes.len(),
            old.shown,
            new.shown
        ),
        data: json!({
            "path": old.shown,
            "revision": old.revision,
            "other": new.shown,
            "other_revision": new.revision,
            "changes": changes,
        }),
    })
}

fn explain(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (bid, sid) = pick(s, a)?;
    let id = match (a.u32("element"), a.str("name")) {
        (Some(id), _) => {
            let id = ElementId(id);
            s.element(id)
                .ok_or_else(|| ToolError::not_found(format!("no element {}", id.0)))?;
            id
        }
        (None, Some(n)) => {
            let cats: Vec<_> = match a.str("category") {
                Some(c) => vec![s
                    .category_named(c)
                    .ok_or_else(|| ToolError::not_found(format!("no category {}", quoted(c))))?],
                None => s.categories.iter().map(|c| c.id).collect(),
            };
            let found: Vec<ElementId> =
                cats.iter().filter_map(|c| s.element_named(*c, n)).collect();
            match found[..] {
                [] => return Err(ToolError::not_found(format!("no element {}", quoted(n)))),
                [id] => id,
                _ => {
                    return Err(ToolError::invalid(format!(
                        "{} is in more than one category; give category",
                        quoted(n)
                    )))
                }
            }
        }
        (None, None) => return Err(ToolError::invalid("give element (an id) or name")),
    };
    let x = explain_count(s, bid, sid, id)?;
    let el = s.element(id).expect("checked");
    let gap = |g: &Gap| json!({"after": g.after.to_string(), "before": g.before.to_string(), "days": g.days});
    Ok(ToolOut {
        summary: x.summary.first().cloned().unwrap_or_default(),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "board": bid.0,
            "sub_board": sid.0,
            "element": id.0,
            "name": el.name,
            "category": s.category(el.category).map_or(String::new(), |c| c.name.clone()),
            "rules": {"holds": x.rules.holds, "drops": x.rules.drops, "min_drop_gap": x.rules.min_drop_gap},
            "start": date_json(x.start),
            "finish": date_json(x.finish),
            "work": x.work,
            "hold": x.hold,
            "total": x.total,
            "work_days": x.work_days,
            "event_days": x.event_days,
            "holidays": x.holidays,
            "moves": x.moves,
            "drop": x.drop.as_ref().map_or(Value::Null, gap),
            "gaps": x.gaps.iter().map(|g| {
                let mut v = gap(&g.gap);
                v["outcome"] = json!(g.outcome.key());
                v["text"] = json!(g.text);
                v
            }).collect::<Vec<_>>(),
            "days": x.days.iter().map(|d| json!({
                "date": d.date.to_string(),
                "code": d.code,
                "counts": d.counted.key(),
                "reason": d.reason.key(),
                "shoot_day": d.shoot_day,
                "sheets": d.sheets.iter().map(|s| s.0).collect::<Vec<_>>(),
                "events": d.events,
                "text": d.text,
            })).collect::<Vec<_>>(),
            "summary": x.summary,
        }),
    })
}

fn find(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let q = search_query(s, a)?;
    let hits = find_places(s, &q)?;
    let limit = a.u64("limit").unwrap_or(500) as usize;
    let results: Vec<Value> = hits
        .iter()
        .take(limit)
        .enumerate()
        .map(|(i, h)| {
            let mut v = location_json(&h.location);
            v["number"] = json!(i + 1);
            v["text"] = json!(h.text);
            v["context"] = json!(h.context);
            v
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} place(s) match {} in {}{}.",
            hits.len(),
            quoted(&q.text),
            l.shown,
            if hits.len() > limit {
                format!(", the first {limit} listed")
            } else {
                String::new()
            }
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "count": hits.len(),
            "truncated": hits.len() > limit,
            "results": results,
        }),
    })
}

fn conflicts(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let bid = pick_board(s, a)?;
    let c = board_conflicts(s, bid)?;
    let b = s.board(bid).expect("picked");
    let sub_name = |id| {
        b.sub_boards
            .iter()
            .find(|x| x.id == id)
            .map_or(String::new(), |x| x.name.clone())
    };
    let element = |id: ElementId| {
        let e = s.element(id);
        json!({
            "id": id.0,
            "name": e.map_or(String::new(), |e| e.name.clone()),
            "category": e.and_then(|e| s.category(e.category)).map_or(String::new(), |c| c.name.clone()),
            "board_id": e.and_then(|e| e.board_id.clone()),
        })
    };
    let booking = |bk: &Booking| {
        json!({
            "sub_board": bk.sub_board.0,
            "sub_board_name": sub_name(bk.sub_board),
            "day": bk.day,
            "sheets": bk.sheets.iter().map(|h| json!({
                "id": h.0,
                "scenes": s.sheet(*h).map_or(String::new(), |x| x.scenes.clone()),
            })).collect::<Vec<_>>(),
        })
    };
    let list: Vec<Value> = c
        .conflicts
        .iter()
        .map(|x| {
            json!({
                "date": x.date.to_string(),
                "element": element(x.element),
                "bookings": x.bookings.iter().map(booking).collect::<Vec<_>>(),
            })
        })
        .collect();
    let on_work: Vec<Value> = c
        .event_on_work
        .iter()
        .map(|x| {
            json!({
                "date": x.date.to_string(),
                "element": element(x.element),
                "sub_board": x.sub_board.0,
                "sub_board_name": sub_name(x.sub_board),
                "day": x.day,
                "event": {"index": x.event, "name": x.name},
            })
        })
        .collect();
    let clashes: Vec<Value> = c
        .event_clashes
        .iter()
        .map(|x| {
            json!({
                "date": x.date.to_string(),
                "element": element(x.element),
                "events": x.events.iter().map(|e| json!({
                    "sub_board": e.sub_board.0,
                    "sub_board_name": sub_name(e.sub_board),
                    "index": e.index,
                    "name": e.name,
                })).collect::<Vec<_>>(),
                "bookings": x.bookings.iter().map(booking).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} conflict(s) on {} ({} element(s) on a date in two sub-boards, {} event(s) on \
             work days); {} event clash(es), informational only.",
            c.count(),
            quoted(&b.name),
            list.len(),
            on_work.len(),
            clashes.len()
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "board": bid.0,
            "count": c.count(),
            "conflicts": list,
            "event_on_work": on_work,
            "event_clashes": clashes,
            "event_clashes_note": "Informational, not conflicts: two or more events for an \
                                   element on one date, or an event in one sub-board and work \
                                   in another. An event on a work day in the same sub-board \
                                   is a conflict, listed under event_on_work.",
            "not_compared": c.undated.iter().map(|id| json!({
                "sub_board": id.0,
                "name": sub_name(*id),
                "why": "no start date, or no working day in its week",
            })).collect::<Vec<_>>(),
        }),
    })
}

fn red_flags(ctx: &Ctx, a: &Args) -> ToolResult {
    let l = load(ctx, a)?;
    let s = &l.schedule;
    let (bid, sid) = pick(s, a)?;
    let r = sub_board_red_flags(s, bid, sid)?;
    let b = s.board(bid).expect("picked");
    let sb = b.sub_boards.iter().find(|x| x.id == sid).expect("picked");
    let flags: Vec<Value> = s
        .elements
        .iter()
        .flat_map(|e| e.red_flags.iter().map(move |f| red_flag_json(s, e.id, f)))
        .collect();
    let clashes: Vec<Value> = r
        .clashes
        .iter()
        .map(|c| {
            json!({
                "date": c.date.to_string(),
                "element": c.element.0,
                "element_name": s.element(c.element).map_or(String::new(), |e| e.name.clone()),
                "red_flag": c.flag.0,
                "type": c.kind.0,
                "type_name": s.red_flag_type(c.kind).map_or(String::new(), |t| t.name.clone()),
                "hard": c.hard,
                "day": c.day,
                "sheets": c.sheets.iter().map(|h| json!({
                    "id": h.0,
                    "scenes": s.sheet(*h).map_or(String::new(), |x| x.scenes.clone()),
                })).collect::<Vec<_>>(),
                "events": c.events.iter().map(|i| json!({
                    "index": i,
                    "name": sb.calendar.events.get(*i).map_or(String::new(), |e| e.name.clone()),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(ToolOut {
        summary: format!(
            "{} red flag clash(es) on {} (board {}); {} red flag(s) in the schedule.",
            clashes.len(),
            quoted(&sb.name),
            quoted(&b.name),
            flags.len()
        ),
        data: json!({
            "path": l.shown,
            "revision": l.revision,
            "board": bid.0,
            "sub_board": sid.0,
            "types": s.red_flag_types.iter().map(|t| json!({"id": t.id.0, "name": t.name, "color": t.color.map(|c| c.hex()), "builtin": t.builtin.as_ref().map(|k| k.key())})).collect::<Vec<_>>(),
            "red_flags": flags,
            "clashes": clashes,
            "undated": r.undated,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_kinds_have_names() {
        assert_eq!(day_kind_name(DayKind::Move), "move");
        let mut s = Schedule::new("x");
        let sh = s.add_sheet(Sheet::default());
        assert_eq!(sheet_json(s.sheet(sh).unwrap())["eighths"], 0);
    }
}
