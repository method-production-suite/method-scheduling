//! The budget exchange file: what a budget needs from a schedule, as small documented JSON
//! (docs/EXCHANGE.md). Shoot days by unit, set and location, and per element the days it
//! works, holds, travels and so on, each count kept apart. Version 1, frozen on
//! 2026-10-15: changes from here are additive only.

use std::collections::BTreeMap;

use serde::Serialize;

use method_schedule_core::days::days;
use method_schedule_core::dood::{dood, Cell};
use method_schedule_core::model::{BoardId, Error, Schedule, StripKind, SubBoardId};
use method_schedule_core::{Date, Weekday};

/// The `format` field of an exchange file.
pub const EXCHANGE_FORMAT: &str = "method-schedule-days";
/// The exchange version this build writes. Version 0 was the draft; a reader takes both.
pub const EXCHANGE_VERSION: u32 = 1;

#[derive(Serialize)]
struct Exchange {
    format: &'static str,
    version: u32,
    schedule: String,
    board: String,
    sub_board: String,
    /// The date the file was written, when the caller gives one.
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<String>,
    /// A hash of the whole schedule as saved (its canonical file text). It changes whenever
    /// the schedule does, so a budget can tell the link is stale.
    source_hash: String,
    /// A hash of only what the file reports, `shoot_days` and `elements` (see
    /// [`days_hash`]), so a budget can tell a change in days from an unrelated edit.
    days_hash: String,
    shoot_days: ShootDays,
    elements: Vec<ElementDays>,
}

#[derive(Serialize)]
struct ShootDays {
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    total: u32,
    /// Shoot days on which each value appears on at least one scheduled sheet. A day can
    /// count under several values, so these don't add up to the total.
    by_unit: BTreeMap<String, u32>,
    by_set: BTreeMap<String, u32>,
    by_location: BTreeMap<String, u32>,
    /// The Monday of each calendar week with shoot days, to the number of shoot days in it.
    by_week: BTreeMap<String, u32>,
    /// The date of every shoot day, in order, so a reader can group them any way it likes.
    dates: Vec<String>,
    /// The sub-board's prep start and wrap end, when the schedule records them (optional,
    /// additive: docs/EXCHANGE.md).
    #[serde(skip_serializing_if = "Option::is_none")]
    prep_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wrap_end: Option<String>,
}

#[derive(Serialize)]
struct ElementDays {
    id: u32,
    /// Left out rather than written as `""`; a schedule that passes its checks always has
    /// one.
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    /// Left out when the element has none, never `""`.
    #[serde(skip_serializing_if = "Option::is_none")]
    board_id: Option<String>,
    name: String,
    /// First and last strip day, left out when the element has no strip day on this
    /// sub-board (it only has counted events there).
    #[serde(skip_serializing_if = "Option::is_none")]
    first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<String>,
    work: u32,
    hold: u32,
    travel: u32,
    fitting: u32,
    rehearsal: u32,
    holiday: u32,
    company_move: u32,
    /// Everything above added up: work, hold and every counted kind. The same as the
    /// reference product's summary total (worked plus held).
    total: u32,
    holds: bool,
    drops: bool,
    min_drop_gap: u32,
}

/// Writes the exchange file for one sub-board as pretty JSON with a trailing newline.
/// `generated` is the date to stamp, if any; the engine has no clock.
pub fn to_string(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
    generated: Option<Date>,
) -> Result<String, Error> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let start = sb
        .start
        .ok_or_else(|| Error::Broken(format!("sub-board {:?} has no start date", sb.name)))?;

    let mut by_unit = BTreeMap::new();
    let mut by_set = BTreeMap::new();
    let mut by_location = BTreeMap::new();
    let mut by_week = BTreeMap::new();
    let mut dates = Vec::new();
    let all = days(s, board, sub)?;
    let mut end = None;
    for day in &all.days {
        end = sb.calendar.date_of_day(start, day.number);
        if let Some(d) = end {
            dates.push(d.to_string());
            let monday = d.add_days(
                -(Weekday::ALL
                    .iter()
                    .position(|w| *w == d.weekday())
                    .unwrap_or(0) as i32),
            );
            *by_week.entry(monday.to_string()).or_insert(0) += 1;
        }
        let mut units = std::collections::BTreeSet::new();
        let mut sets = std::collections::BTreeSet::new();
        let mut locations = std::collections::BTreeSet::new();
        for id in &day.strips {
            let Some(StripKind::Sheet(sheet)) =
                sb.strips.iter().find(|x| x.id == *id).map(|x| &x.kind)
            else {
                continue;
            };
            let Some(h) = s.sheet(*sheet) else { continue };
            for (set, v) in [
                (&mut units, &h.unit),
                (&mut sets, &h.set),
                (&mut locations, &h.location),
            ] {
                if !v.trim().is_empty() {
                    set.insert(v.trim().to_string());
                }
            }
        }
        for (map, vals) in [
            (&mut by_unit, units),
            (&mut by_set, sets),
            (&mut by_location, locations),
        ] {
            for v in vals {
                *map.entry(v).or_insert(0) += 1;
            }
        }
    }

    let mut elements = Vec::new();
    for row in dood(s, board, sub)? {
        let e = s
            .element(row.element)
            .ok_or(Error::NoSuchElement(row.element))?;
        let mut d = ElementDays {
            id: e.id.0,
            category: s
                .category(e.category)
                .map(|c| c.name.clone())
                .filter(|n| !n.trim().is_empty()),
            board_id: e.board_id.clone().filter(|x| !x.trim().is_empty()),
            name: e.name.clone(),
            first: row.start.map(|x| x.to_string()),
            last: row.finish.map(|x| x.to_string()),
            work: 0,
            hold: 0,
            travel: 0,
            fitting: 0,
            rehearsal: 0,
            holiday: 0,
            company_move: 0,
            total: 0,
            holds: e.rules.holds,
            drops: e.rules.drops,
            min_drop_gap: e.rules.min_drop_gap,
        };
        for (_, cell) in &row.cells {
            match cell {
                // An event on a work day counts once, as work (behavior note 17, A4).
                Cell::Work { .. } | Cell::EventOnWork { .. } => d.work += 1,
                Cell::Hold => d.hold += 1,
                Cell::Event { code: "TR", .. } => d.travel += 1,
                Cell::Event { code: "FT", .. } => d.fitting += 1,
                Cell::Event { code: "R", .. } => d.rehearsal += 1,
                Cell::Event { .. } => {}
                Cell::Holiday => d.holiday += 1,
                Cell::Move => d.company_move += 1,
                Cell::Idle => {}
            }
        }
        d.total = d.work + d.hold + d.travel + d.fitting + d.rehearsal + d.holiday + d.company_move;
        debug_assert_eq!(d.total, row.worked + row.held);
        elements.push(d);
    }

    let shoot_days = ShootDays {
        start: Some(start.to_string()),
        end: end.map(|d| d.to_string()),
        total: all.days.len() as u32,
        by_unit,
        by_set,
        by_location,
        by_week,
        dates,
        prep_start: sb.calendar.prep_start.map(|d| d.to_string()),
        wrap_end: sb.calendar.wrap_end.map(|d| d.to_string()),
    };
    let file = Exchange {
        format: EXCHANGE_FORMAT,
        version: EXCHANGE_VERSION,
        schedule: s.title.clone(),
        board: b.name.clone(),
        sub_board: sb.name.clone(),
        generated: generated.map(|d| d.to_string()),
        source_hash: fnv1a64(crate::to_string(s).as_bytes()),
        days_hash: days_hash(&shoot_days, &elements),
        shoot_days,
        elements,
    };
    let mut text = serde_json::to_string_pretty(&file).expect("exchange files always serialize");
    text.push('\n');
    Ok(text)
}

/// What `days_hash` covers: the file's own `shoot_days` and `elements`, as compact JSON
/// (`{"shoot_days":{..},"elements":[..]}`, no whitespace outside strings), the same values
/// in the same order as the file writes them. Its fields are the ones version 1 has: a
/// field added later inside either is left out here, so an unchanged schedule keeps its
/// hash across writer versions (docs/EXCHANGE.md).
#[derive(Serialize)]
struct DaysHashed<'a> {
    shoot_days: &'a ShootDays,
    elements: &'a [ElementDays],
}

fn days_hash(shoot_days: &ShootDays, elements: &[ElementDays]) -> String {
    let bytes = serde_json::to_vec(&DaysHashed {
        shoot_days,
        elements,
    })
    .expect("exchange files always serialize");
    fnv1a64(&bytes)
}

/// FNV-1a, 64 bits, as 16 hex digits: enough to notice a change, not a security hash.
/// Also the revision `method-schedule mcp` reports for a file's bytes (docs/MCP.md).
pub fn fnv1a64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Today's date on the computer's own clock, in its time zone: the date an exchange file
/// is stamped with. A schedule's dates are local dates, so a file made late in the
/// evening carries that evening's date, not the next day's in UTC. Falls back to UTC
/// when the time zone can't be read. Not on WebAssembly, where the standard clock
/// panics: a browser host passes its own date, so a call there doesn't compile.
#[cfg(not(target_arch = "wasm32"))]
pub fn local_today() -> Option<Date> {
    local::today().or_else(utc_today)
}

#[cfg(not(target_arch = "wasm32"))]
fn utc_today() -> Option<Date> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(Date((secs / 86_400) as i32))
}

/// The C library's `localtime_r`, which reads the time zone (`TZ`, then the system's).
#[cfg(unix)]
mod local {
    use std::os::raw::{c_char, c_int, c_long};

    use method_schedule_core::Date;

    /// `struct tm` as macOS, Linux and the BSDs lay it out, with room to spare at the end.
    #[repr(C)]
    struct Tm {
        sec: c_int,
        min: c_int,
        hour: c_int,
        mday: c_int,
        mon: c_int,
        year: c_int,
        wday: c_int,
        yday: c_int,
        isdst: c_int,
        gmtoff: c_long,
        zone: *const c_char,
        spare: [u8; 64],
    }

    extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub fn today() -> Option<Date> {
        let mut tm = Tm {
            sec: 0,
            min: 0,
            hour: 0,
            mday: 0,
            mon: 0,
            year: 0,
            wday: 0,
            yday: 0,
            isdst: 0,
            gmtoff: 0,
            zone: std::ptr::null(),
            spare: [0; 64],
        };
        // SAFETY: `time` with a null pointer only returns the time; `localtime_r` writes
        // into `tm`, which is ours and at least as large as the C library's `struct tm`.
        let ok = unsafe {
            let now = time(std::ptr::null_mut());
            !localtime_r(&now, &mut tm).is_null()
        };
        if !ok {
            return None;
        }
        Date::from_ymd(
            tm.year + 1900,
            u32::try_from(tm.mon + 1).ok()?,
            u32::try_from(tm.mday).ok()?,
        )
    }
}

/// Windows' `GetLocalTime`, from kernel32, which every program links.
#[cfg(windows)]
mod local {
    use method_schedule_core::Date;

    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(out: *mut SystemTime);
    }

    pub fn today() -> Option<Date> {
        let mut st = SystemTime::default();
        // SAFETY: `GetLocalTime` fills the struct it is given and nothing else.
        unsafe { GetLocalTime(&mut st) };
        Date::from_ymd(i32::from(st.year), u32::from(st.month), u32::from(st.day))
    }
}

#[cfg(not(any(unix, windows)))]
mod local {
    pub fn today() -> Option<method_schedule_core::Date> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::model::{DoodRules, Event, EventKind};
    use method_schedule_core::sample::sample;

    #[test]
    fn counts_each_kind_apart_and_matches_the_dood_total() {
        let mut s = sample(200);
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let cast = s.category_named("Cast").unwrap();
        let p = s.element_named(cast, "PLAYER 00").unwrap();
        s.element_mut(p).unwrap().rules = DoodRules {
            holds: true,
            drops: false,
            min_drop_gap: 1,
        };
        let start = s.boards[0].sub_boards[0].start.unwrap();
        s.boards[0].sub_boards[0].calendar.events.push(Event {
            name: "Wardrobe".into(),
            kind: EventKind::Fitting,
            from: start.add_days(-3),
            to: start.add_days(-3),
            elements: vec![p],
            color: None,
        });
        let text = to_string(&s, b, sb, Date::from_ymd(2026, 10, 11)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["format"], "method-schedule-days");
        assert_eq!(v["generated"], "2026-10-11");
        assert!(v["shoot_days"]["total"].as_u64().unwrap() > 0);
        let rows = dood(&s, b, sb).unwrap();
        for e in v["elements"].as_array().unwrap() {
            let row = rows
                .iter()
                .find(|r| r.element.0 as u64 == e["id"].as_u64().unwrap())
                .unwrap();
            assert_eq!(
                e["total"].as_u64().unwrap() as u32,
                row.worked + row.held,
                "{e}"
            );
        }
        let lead = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "PLAYER 00")
            .unwrap();
        assert_eq!(lead["fitting"], 1);
        assert!(lead["hold"].as_u64().unwrap() > 0);
        assert!(lead["holiday"].as_u64().unwrap() >= 1, "{lead}");
    }

    /// The file is what EXCHANGE.md documents, shared with Method Budget: every field its
    /// three tables list, in their order, and nothing else, so a change has to go through
    /// the spec (additive only after version 1). Its example has the same shape too.
    #[test]
    fn writes_exactly_the_fields_exchange_md_lists_in_its_order() {
        let doc = include_str!("../../../docs/EXCHANGE.md");
        let table = |heading: &str| -> Vec<String> {
            let part = doc.split(heading).nth(1).expect(heading);
            part.split("\n\n")
                .find(|p| p.starts_with("| Field"))
                .expect("a field table")
                .lines()
                .skip(2)
                .map(|l| l.split('`').nth(1).expect("a `field`").to_string())
                .collect()
        };
        let (top, days, elements) = (
            table("## Top level"),
            table("## `shoot_days`"),
            table("## Elements"),
        );
        // Every optional field present: prep and wrap, a generated date, a board ID.
        let mut s = sample(30);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        s.boards[0].sub_boards[0].calendar.prep_start = Date::from_ymd(2026, 12, 1);
        s.boards[0].sub_boards[0].calendar.wrap_end = Date::from_ymd(2027, 6, 30);
        let text = to_string(&s, b, sb, Date::from_ymd(2026, 10, 11)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys = |o: &serde_json::Value| -> Vec<String> {
            let mut k: Vec<String> = o.as_object().unwrap().keys().cloned().collect();
            // In the order written (serde_json's map sorts them).
            k.sort_by_key(|key| text.find(&format!("\"{key}\":")).unwrap());
            k
        };
        assert_eq!(keys(&v), top, "top level");
        // The order inside `shoot_days`: positions after its own key.
        let at = text.find("\"shoot_days\":").unwrap();
        let mut d: Vec<String> = v["shoot_days"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        d.sort_by_key(|key| text[at..].find(&format!("\"{key}\":")).unwrap());
        assert_eq!(d, days, "shoot_days");
        let lead = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e.get("board_id").is_some())
            .expect("an element with a board ID");
        let at = text.find("\"elements\":").unwrap();
        let mut e: Vec<String> = lead.as_object().unwrap().keys().cloned().collect();
        e.sort_by_key(|key| text[at..].find(&format!("\"{key}\":")).unwrap());
        assert_eq!(e, elements, "elements");
        // The example reads as JSON with the documented fields (prep and wrap left out).
        let example = doc
            .split("## Example\n\n```json\n")
            .nth(1)
            .and_then(|t| t.split("```").next())
            .unwrap();
        let ex: serde_json::Value = serde_json::from_str(example).unwrap();
        let set = |o: &serde_json::Value| -> std::collections::BTreeSet<String> {
            o.as_object().unwrap().keys().cloned().collect()
        };
        assert_eq!(set(&ex), top.iter().cloned().collect());
        let optional = ["prep_start", "wrap_end"];
        assert_eq!(
            set(&ex["shoot_days"]),
            days.iter()
                .filter(|k| !optional.contains(&k.as_str()))
                .cloned()
                .collect()
        );
        assert_eq!(set(&ex["elements"][0]), elements.iter().cloned().collect());
    }

    #[test]
    fn prep_and_wrap_are_written_only_when_set() {
        let mut s = sample(20);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let text = to_string(&s, b, sb, None).unwrap();
        assert!(!text.contains("prep_start") && !text.contains("wrap_end"));
        s.boards[0].sub_boards[0].calendar.prep_start = Date::from_ymd(2026, 12, 1);
        s.boards[0].sub_boards[0].calendar.wrap_end = Date::from_ymd(2027, 6, 30);
        let v: serde_json::Value =
            serde_json::from_str(&to_string(&s, b, sb, None).unwrap()).unwrap();
        assert_eq!(v["shoot_days"]["prep_start"], "2026-12-01");
        assert_eq!(v["shoot_days"]["wrap_end"], "2027-06-30");
    }

    #[test]
    fn hash_changes_with_the_schedule_and_weeks_add_up() {
        let mut s = sample(60);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let read = |s: &Schedule| -> serde_json::Value {
            serde_json::from_str(&to_string(s, b, sb, None).unwrap()).unwrap()
        };
        let a = read(&s);
        assert_eq!(a["source_hash"], read(&s)["source_hash"]);
        s.sheets[0].synopsis = "A change.".into();
        assert_ne!(a["source_hash"], read(&s)["source_hash"]);
        // A change to the view alone changes it too (EXCHANGE.md): the hash is over the
        // whole file, kept so for the budget side.
        let before = read(&s)["source_hash"].clone();
        s.boards[0].day_look.sunset = true;
        assert_ne!(before, read(&s)["source_hash"]);
        let weeks: u64 = a["shoot_days"]["by_week"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum();
        assert_eq!(weeks, a["shoot_days"]["total"].as_u64().unwrap());
        assert!(a["shoot_days"]["by_week"]
            .as_object()
            .unwrap()
            .contains_key("2027-03-01"));
        assert_eq!(fnv1a64(b""), "cbf29ce484222325");
    }

    /// Version 1: an element with no strip day on the sub-board (only a counted event)
    /// has no `first` or `last`, and a missing board ID is left out, never `""`.
    #[test]
    fn leaves_out_what_an_element_lacks_rather_than_write_empty_values() {
        let mut s = sample(30);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let props = s.category_named("Props").unwrap();
        let tank = s.add_element(props, "WATER TANK").unwrap();
        s.element_mut(tank).unwrap().board_id = Some("  ".into());
        let start = s.boards[0].sub_boards[0].start.unwrap();
        s.boards[0].sub_boards[0].calendar.events.push(Event {
            name: "Tank test".into(),
            kind: EventKind::Fitting,
            from: start.add_days(-2),
            to: start.add_days(-2),
            elements: vec![tank],
            color: None,
        });
        let text = to_string(&s, b, sb, None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], 1);
        let els = v["elements"].as_array().unwrap();
        let t = els.iter().find(|e| e["name"] == "WATER TANK").unwrap();
        assert_eq!(t["fitting"], 1);
        assert_eq!(t["total"], 1);
        for key in ["first", "last", "board_id"] {
            assert!(t.get(key).is_none(), "{key}: {t}");
        }
        assert_eq!(t["category"], "Props");
        // Every element on a strip day has both dates; every category is named.
        for e in els.iter().filter(|e| e["work"].as_u64().unwrap() > 0) {
            assert!(e["first"].is_string() && e["last"].is_string(), "{e}");
        }
        assert!(els.iter().all(|e| e["category"].is_string()));
        assert!(!text.contains(": \"\""), "no empty strings");
    }

    #[test]
    fn days_hash_moves_with_the_days_and_not_with_other_edits() {
        let mut s = sample(60);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let read = |s: &Schedule, day: Option<Date>| -> serde_json::Value {
            serde_json::from_str(&to_string(s, b, sb, day).unwrap()).unwrap()
        };
        let a = read(&s, None);
        let hash = a["days_hash"].as_str().unwrap().to_string();
        assert_eq!(hash.len(), 16);
        // Unrelated edits: a synopsis, the view, the stamped date, the schedule's title.
        s.sheets[0].synopsis = "A change.".into();
        s.boards[0].day_look.sunset = true;
        s.title = "Another title".into();
        let c = read(&s, Date::from_ymd(2026, 10, 15));
        assert_ne!(a["source_hash"], c["source_hash"]);
        assert_eq!(hash, c["days_hash"]);
        // A real change: the sub-board starts a week later, so every date moves.
        let start = s.boards[0].sub_boards[0].start.unwrap();
        s.boards[0].sub_boards[0].start = Some(start.add_days(7));
        assert_ne!(hash, read(&s, None)["days_hash"]);
        // And a count: an element's hold switch.
        s.boards[0].sub_boards[0].start = Some(start);
        assert_eq!(hash, read(&s, None)["days_hash"]);
        let cast = s.category_named("Cast").unwrap();
        let p = s.element_named(cast, "PLAYER 00").unwrap();
        let holds = s.element(p).unwrap().rules.holds;
        s.element_mut(p).unwrap().rules.holds = !holds;
        assert_ne!(hash, read(&s, None)["days_hash"]);
    }

    /// EXCHANGE.md's definition, worked from the file's own text: its `shoot_days` and
    /// `elements` with the whitespace outside strings removed, in a two-key object.
    #[test]
    fn days_hash_is_the_compact_shoot_days_and_elements() {
        let mut s = sample(40);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        s.boards[0].sub_boards[0].calendar.prep_start = Date::from_ymd(2026, 12, 1);
        let text = to_string(&s, b, sb, Date::from_ymd(2026, 10, 15)).unwrap();
        let mut compact = String::new();
        let (mut quoted, mut escaped) = (false, false);
        for c in text.chars() {
            if quoted {
                compact.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    quoted = false;
                }
            } else if !c.is_whitespace() {
                quoted = c == '"';
                compact.push(c);
            }
        }
        let at = compact.find("\"shoot_days\":").unwrap();
        let hashed = format!("{{{}", &compact[at..]);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["days_hash"], fnv1a64(hashed.as_bytes()));
    }

    #[test]
    fn lists_every_shoot_date_in_order() {
        let s = sample(60);
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let v: serde_json::Value =
            serde_json::from_str(&to_string(&s, b, sb, None).unwrap()).unwrap();
        let d = &v["shoot_days"];
        let dates: Vec<Date> = d["dates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().parse().unwrap())
            .collect();
        assert_eq!(dates.len() as u64, d["total"].as_u64().unwrap());
        assert!(dates.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(dates[0].to_string(), d["start"]);
        assert_eq!(dates.last().unwrap().to_string(), d["end"]);
        // Regrouped by Monday, they give `by_week`.
        let mut weeks = BTreeMap::new();
        for x in &dates {
            let back = Weekday::ALL.iter().position(|w| *w == x.weekday()).unwrap();
            *weeks
                .entry(x.add_days(-(back as i32)).to_string())
                .or_insert(0u64) += 1;
        }
        let by_week: BTreeMap<String, u64> = d["by_week"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, n)| (k.clone(), n.as_u64().unwrap()))
            .collect();
        assert_eq!(weeks, by_week);
    }

    #[test]
    fn needs_a_start_date() {
        let s = Schedule::new("ZTEST");
        assert!(to_string(&s, s.boards[0].id, s.boards[0].sub_boards[0].id, None).is_err());
    }

    #[test]
    fn today_is_within_a_day_of_utc() {
        let local = super::local_today().unwrap();
        let utc = super::utc_today().unwrap();
        assert!((local.0 - utc.0).abs() <= 1, "{local:?} {utc:?}");
    }
}
