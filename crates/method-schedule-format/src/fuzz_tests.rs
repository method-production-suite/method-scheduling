//! Fuzz and property tests for the file reader (decision 173), with the engine's seeded
//! generators (`method_schedule_core::testgen`, a dev-only feature; no new crate).
//!
//! - Random schedules built through the engine write and read back as they were: the
//!   schedule file, the template, the layout file; the budget exchange file (which has no
//!   reader) writes the same bytes twice and its counts add up.
//! - Thousands of seeded mutations of written files never panic the reader. A mutated
//!   file is refused, or it reads into a schedule that passes `check` and writes back as
//!   the file was: byte for byte where the mutation kept the writer's layout (a value
//!   swapped for another, a key or a list entry added or taken out), the same JSON
//!   otherwise (bytes changed anywhere).
//!
//! More rounds: `METHOD_SCHEDULE_FUZZ_ROUNDS=50`, and `METHOD_SCHEDULE_FUZZ_SEED` for
//! other cases (the weekly fuzz job sets both).

use serde_json::Value;

use method_schedule_core::layouts::{LayoutBody, LayoutId, SavedLayout};
use method_schedule_core::model::{DayLook, Seconds, StripColumn, StripField, StripKind};
use method_schedule_core::model::{StripLayout, StripWidth};
use method_schedule_core::plan::{set_call_time, GeoLocation};
use method_schedule_core::solar::DaylightSaving;
use method_schedule_core::testgen::{
    fuzz_rng, mutate_bytes, random_edit, random_schedule, rounds, Rng,
};
use method_schedule_core::{Date, Schedule};

use crate::{
    exchange, from_str, is_template, layouts_from_str, layouts_to_string, to_string,
    to_template_string, to_template_string_as_is,
};

/// Text a person might type, and text no one should, for the free text fields.
const TEXTS: &[&str] = &[
    "Plain",
    "Ünïcödé ✓",
    "quote \" and \\ backslash",
    "line\nbreak",
    "tab\there",
    "nul \u{0} and \u{7f}",
    "emoji 🎬",
    " spaced ",
    "RTL \u{202e}mark",
    "</script>",
    "=SUM(A1)",
];

fn pick<'a, T>(rng: &mut Rng, v: &'a [T]) -> &'a T {
    &v[rng.below(v.len() as u64) as usize]
}

fn text(rng: &mut Rng) -> String {
    if rng.below(20) == 0 {
        "long ".repeat(200)
    } else {
        pick(rng, TEXTS).to_string()
    }
}

/// A few more things the merge generator leaves alone: free text with odd characters,
/// sun times, screen time, named layouts, call times and a map location.
fn decorate(rng: &mut Rng, s: &mut Schedule) {
    for _ in 0..rng.below(6) {
        match rng.below(8) {
            0 => {
                let f = *pick(rng, &method_schedule_core::setup::Production::FIELDS);
                let t = text(rng);
                let _ = s.production.set(f, &t);
            }
            1 if !s.sheets.is_empty() => {
                let i = rng.below(s.sheets.len() as u64) as usize;
                let t = text(rng);
                let h = &mut s.sheets[i];
                match rng.below(4) {
                    0 => h.synopsis = t,
                    1 => h.comments = t,
                    2 => h.sunset = Some(rng.below(24 * 60) as u32),
                    _ => h.screen_time = Some(Seconds(rng.below(5000) as u32)),
                }
            }
            2 if !s.elements.is_empty() => {
                let i = rng.below(s.elements.len() as u64) as usize;
                s.elements[i].notes = text(rng);
            }
            3 => {
                let name = format!("Layout {}", rng.below(4));
                let body = match rng.below(3) {
                    0 => LayoutBody::DayBreaks(DayLook {
                        sunrise: rng.below(2) == 0,
                        text: text(rng),
                        ..DayLook::default()
                    }),
                    1 => LayoutBody::ShootDay(DayLook {
                        text: text(rng),
                        ..DayLook::default()
                    }),
                    _ => {
                        let n = 1 + rng.below(4) as usize;
                        LayoutBody::Strips(StripLayout::of(
                            StripField::ALL[..n]
                                .iter()
                                .map(|f| StripColumn {
                                    width: StripWidth::ALL[rng.below(4) as usize],
                                    ..StripColumn::of(*f)
                                })
                                .collect(),
                        ))
                    }
                };
                let _ = s.add_layout(&name, body);
            }
            4 => {
                let day = s.boards[0]
                    .sub_boards
                    .iter()
                    .flat_map(|x| &x.strips)
                    .find(|x| x.kind == StripKind::DayBreak)
                    .map(|x| x.id);
                if let Some(day) = day {
                    let _ = set_call_time(s, day, Some(rng.below(24 * 60) as u32));
                }
            }
            5 => {
                let _ = s.plan.set_location(GeoLocation {
                    name: text(rng),
                    latitude: rng.below(180_000_001) as i32 - 90_000_000,
                    longitude: rng.below(360_000_001) as i32 - 180_000_000,
                    utc_offset: rng.below(14 * 60 * 2 + 1) as i32 - 14 * 60,
                    daylight_saving: [
                        None,
                        Some(DaylightSaving::UsCanada),
                        Some(DaylightSaving::Eu),
                    ][rng.below(3) as usize],
                    periods: Vec::new(),
                });
            }
            6 => s.title = text(rng),
            _ => {
                if let Some(b) = s.boards.first_mut() {
                    b.name = text(rng);
                }
            }
        }
    }
}

/// A random schedule built through the engine: the merge tests' generator, a few random
/// edits and [`decorate`]. Passes `check`.
fn schedule(rng: &mut Rng) -> Schedule {
    let mut s = random_schedule(rng);
    for _ in 0..rng.below(6) {
        random_edit(rng, &mut s);
    }
    decorate(rng, &mut s);
    s.check().expect("a random schedule hangs together");
    s
}

/// The layouts as a layout file reads them: names and bodies.
fn named(layouts: &[SavedLayout]) -> Vec<(String, LayoutBody)> {
    layouts
        .iter()
        .map(|l| (l.name.clone(), l.body.clone()))
        .collect()
}

/// Layouts read from a file, given ids so they can be written again.
fn saved(read: Vec<(String, LayoutBody)>) -> Vec<SavedLayout> {
    read.into_iter()
        .enumerate()
        .map(|(i, (name, body))| SavedLayout {
            id: LayoutId(i as u32 + 1),
            name,
            body,
        })
        .collect()
}

#[test]
fn random_schedules_write_and_read_back() {
    let mut rng = fuzz_rng(0xf11e);
    for case in 0..rounds(150) {
        let s = schedule(&mut rng);
        // The schedule file.
        let text = to_string(&s);
        let back = from_str(&text).unwrap_or_else(|e| panic!("case {case}: {e}\n{text}"));
        assert!(back == s, "case {case}: read back differently");
        assert_eq!(to_string(&back), text, "case {case}");
        // The template: the schedule without its sheets, strips, events and red flags.
        let t = to_template_string(&s);
        assert!(is_template(&t));
        let back = from_str(&t).unwrap_or_else(|e| panic!("case {case}: {e}\n{t}"));
        assert!(
            back == s.to_template(),
            "case {case}: template read back differently"
        );
        assert_eq!(to_template_string_as_is(&back).unwrap(), t, "case {case}");
        // The layout file.
        let f = layouts_to_string(&s, &s.layouts.layouts);
        let read = layouts_from_str(&s, &f).unwrap_or_else(|e| panic!("case {case}: {e}\n{f}"));
        assert_eq!(read, named(&s.layouts.layouts), "case {case}");
        assert_eq!(layouts_to_string(&s, &saved(read)), f, "case {case}");
        // The budget exchange file, for every sub-board with a start date (one without
        // is refused).
        let date: Date = "2027-03-01".parse().unwrap();
        for b in &s.boards {
            for sb in &b.sub_boards {
                let Ok(x) = exchange::to_string(&s, b.id, sb.id, Some(date)) else {
                    assert!(sb.start.is_none(), "case {case}");
                    continue;
                };
                assert_eq!(x, exchange::to_string(&s, b.id, sb.id, Some(date)).unwrap());
                exchange_adds_up(&x, case);
            }
        }
    }
}

/// An exchange file is our own JSON with every count a whole number, and an element's
/// total is its days of every kind added up. No nulls, no empty optional strings; one date per
/// shoot day; `first` and `last` come together (docs/EXCHANGE.md, version 1).
fn exchange_adds_up(text: &str, case: u64) {
    let v: Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("case {case}: {e}"));
    assert_eq!(v["format"], exchange::EXCHANGE_FORMAT, "case {case}");
    fn null(v: &Value) -> bool {
        match v {
            Value::Null => true,
            Value::Array(a) => a.iter().any(null),
            Value::Object(o) => o.values().any(null),
            _ => false,
        }
    }
    assert!(!null(&v), "case {case}: a null");
    let total = v["shoot_days"]["total"].as_u64().unwrap();
    let dates = v["shoot_days"]["dates"].as_array().unwrap();
    assert_eq!(dates.len() as u64, total, "case {case}");
    for (_, n) in v["shoot_days"]["by_unit"].as_object().unwrap() {
        assert!(n.as_u64().unwrap() <= total, "case {case}");
    }
    for e in v["elements"].as_array().unwrap() {
        let n = |k: &str| e[k].as_u64().unwrap_or_else(|| panic!("case {case}: {k}"));
        let parts = [
            "work",
            "hold",
            "travel",
            "fitting",
            "rehearsal",
            "holiday",
            "company_move",
        ];
        let sum: u64 = parts.iter().map(|k| n(k)).sum();
        assert_eq!(n("total"), sum, "case {case}: {e}");
        assert_eq!(
            e.get("first").is_some(),
            e.get("last").is_some(),
            "case {case}"
        );
        for k in ["category", "board_id"] {
            assert_ne!(
                e.get(k).and_then(Value::as_str),
                Some(""),
                "case {case}: {k}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Mutations

/// Where the values, an object's members and an array's entries sit in a JSON text the
/// writer wrote (so known to be valid). Spans are byte ranges.
#[derive(Default)]
struct Spans {
    /// Every value, and whether it is a scalar (not an object or array).
    values: Vec<(usize, usize, bool)>,
    /// Every object member, from its key to the end of its value, and whether it is its
    /// object's only member.
    members: Vec<(usize, usize, bool)>,
    /// Every array entry, and whether it is its array's only entry.
    entries: Vec<(usize, usize, bool)>,
    /// Just after each object's `{`.
    objects: Vec<usize>,
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn scan(b: &[u8], i: &mut usize, sp: &mut Spans) {
    skip_ws(b, i);
    let start = *i;
    match b[*i] {
        b'{' => {
            *i += 1;
            sp.objects.push(*i);
            let mut members = Vec::new();
            loop {
                skip_ws(b, i);
                if b[*i] == b'}' {
                    *i += 1;
                    break;
                }
                let key = *i;
                scan(b, i, sp);
                skip_ws(b, i);
                *i += 1; // the colon
                scan(b, i, sp);
                members.push((key, *i));
                skip_ws(b, i);
                if b[*i] == b',' {
                    *i += 1;
                }
            }
            let only = members.len() == 1;
            sp.members
                .extend(members.into_iter().map(|(a, z)| (a, z, only)));
        }
        b'[' => {
            *i += 1;
            let mut entries = Vec::new();
            loop {
                skip_ws(b, i);
                if b[*i] == b']' {
                    *i += 1;
                    break;
                }
                let at = *i;
                scan(b, i, sp);
                entries.push((at, *i));
                skip_ws(b, i);
                if b[*i] == b',' {
                    *i += 1;
                }
            }
            let only = entries.len() == 1;
            sp.entries
                .extend(entries.into_iter().map(|(a, z)| (a, z, only)));
        }
        b'"' => {
            *i += 1;
            while b[*i] != b'"' {
                *i += if b[*i] == b'\\' { 2 } else { 1 };
            }
            *i += 1;
        }
        _ => {
            while *i < b.len() && !b",}] \n\r\t".contains(&b[*i]) {
                *i += 1;
            }
        }
    }
    let scalar = !matches!(b[start], b'{' | b'[');
    sp.values.push((start, *i, scalar));
}

/// The indentation of the line that `at` is on.
fn indent(t: &str, at: usize) -> &str {
    let line = t[..at].rfind('\n').map_or(0, |n| n + 1);
    &t[line..at]
}

/// Takes out the member or entry at `a..z` with its comma, keeping the writer's layout.
fn cut(t: &str, a: usize, z: usize) -> String {
    let after = t[z..].trim_start();
    if let Some(rest) = after.strip_prefix(',') {
        // Up to the next member or entry.
        let next = t.len() - rest.trim_start().len();
        format!("{}{}", &t[..a], &t[next..])
    } else {
        // The last one: take the comma before it.
        let comma = t[..a].trim_end().len() - 1;
        format!("{}{}", &t[..comma], &t[z..])
    }
}

/// Values written as the writer would write them, so a file that reads with one in place
/// must write back byte for byte.
const SCALARS: &[&str] = &[
    "null",
    "true",
    "false",
    "0",
    "1",
    "-1",
    "7",
    "255",
    "1440",
    "65535",
    "4294967295",
    "4294967296",
    "-2147483649",
    "18446744073709551615",
    "\"\"",
    "\"x\"",
    "\"Main\"",
    "\"Cast\"",
    "\"2027-03-01\"",
    "\"2027-02-30\"",
    "\"1999-13-01\"",
    "\"EXT\"",
    "\"#ff0000\"",
    "\"blue\"",
    "\"sun\"",
    "\"wide\"",
    "\"element_count\"",
    "\"0\"",
];

/// Values the writer would never write: other spellings, wrong shapes, deep nesting.
fn odd(rng: &mut Rng) -> String {
    match rng.below(12) {
        0 => "-0".into(),
        1 => "1.0".into(),
        2 => "1e3".into(),
        3 => "1e400".into(),
        4 => "\"\\u0041\"".into(),
        5 => "[]".into(),
        6 => "{}".into(),
        7 => "[1, \"x\", null]".into(),
        8 => "{\"name\": \"x\"}".into(),
        9 => format!("{}{}", "[".repeat(300), "]".repeat(300)),
        10 => format!("\"{}\"", "a".repeat(70_000)),
        _ => format!("\"{}\"", "\\u0000".repeat(50)),
    }
}

/// One mutation of a written file. Returns the text, and whether it kept the writer's
/// layout (then a file that reads must write back byte for byte).
fn mutate(rng: &mut Rng, t: &str) -> (String, bool) {
    let mut sp = Spans::default();
    scan(t.as_bytes(), &mut 0, &mut sp);
    // The whole file is the last value; it stays.
    sp.values.pop();
    let scalars: Vec<_> = sp.values.iter().filter(|v| v.2).copied().collect();
    let b = t.as_bytes();
    let splice = |a: usize, z: usize, with: &str| format!("{}{with}{}", &t[..a], &t[z..]);
    match rng.below(9) {
        // A scalar for another. A key swapped for another name keeps the layout only
        // where the key order does: inside an `extensions` object the writer sorts its
        // keys, so there the same JSON is all that can be asked.
        0 | 1 if !scalars.is_empty() => {
            let &(a, z, _) = pick(rng, &scalars);
            let key = t[z..].trim_start().starts_with(':');
            (splice(a, z, pick(rng, SCALARS)), !key)
        }
        // Any value for an odd one.
        2 if !sp.values.is_empty() => {
            let &(a, z, _) = pick(rng, &sp.values);
            (splice(a, z, &odd(rng)), false)
        }
        // A member given twice.
        3 if !sp.members.is_empty() => {
            let &(a, z, _) = pick(rng, &sp.members);
            let copy = format!("{},\n{}", &t[a..z], indent(t, a));
            (splice(a, a, &copy), true)
        }
        // A member taken out.
        4 if !sp.members.is_empty() => {
            let &(a, z, only) = pick(rng, &sp.members);
            (cut(t, a, z), !only)
        }
        // An unknown member.
        5 if !sp.objects.is_empty() => {
            let &at = pick(rng, &sp.objects);
            let pad = if b.get(at) == Some(&b'}') {
                String::new()
            } else {
                format!(
                    "\n{}",
                    indent(t, at + t[at..].len() - t[at..].trim_start().len())
                )
            };
            (splice(at, at, &format!("{pad}\"zz_unknown\": 1,")), false)
        }
        // A list entry given twice.
        6 if !sp.entries.is_empty() => {
            let &(a, z, _) = pick(rng, &sp.entries);
            let copy = format!(",\n{}{}", indent(t, a), &t[a..z]);
            (splice(z, z, &copy), true)
        }
        // A list entry taken out.
        7 if !sp.entries.is_empty() => {
            let &(a, z, only) = pick(rng, &sp.entries);
            (cut(t, a, z), !only)
        }
        // Bytes changed anywhere.
        _ => {
            let mut bytes = t.as_bytes().to_vec();
            mutate_bytes(rng, &mut bytes, JSON_SNIPPETS);
            (String::from_utf8_lossy(&bytes).into_owned(), false)
        }
    }
}

const JSON_SNIPPETS: &[&[u8]] = &[
    b"{",
    b"}",
    b"[",
    b"]",
    b",",
    b":",
    b"\"",
    b"\\",
    b"null",
    b"-1",
    b"1e999",
    b"4294967296",
    b"\"\\ud800\"",
    b"\"title\": \"x\",",
    b"\xEF\xBB\xBF",
    b"\0",
];

/// The JSON a text holds, if it is JSON.
fn json(t: &str) -> Option<Value> {
    serde_json::from_str(t).ok()
}

/// A mutated schedule or template file: refused, or read into a schedule that passes
/// `check` and writes back as the file was.
fn reads_strictly(t: &str, exact: bool, why: &str) {
    let Ok(s) = from_str(t) else {
        return;
    };
    s.check()
        .unwrap_or_else(|e| panic!("{why}: read a schedule that fails check: {e}"));
    let write = |s: &Schedule| {
        if is_template(t) {
            to_template_string_as_is(s).unwrap_or_else(|e| panic!("{why}: {e}"))
        } else {
            to_string(s)
        }
    };
    let back = write(&s);
    if exact {
        assert_eq!(back, t, "{why}: read, but writes back otherwise");
    } else if let Some(given) = json(t) {
        assert_eq!(
            json(&back),
            Some(given),
            "{why}: read, but writes back otherwise"
        );
    }
    let again = from_str(&back).unwrap_or_else(|e| panic!("{why}: its own write: {e}"));
    assert!(again == s, "{why}: its own write reads otherwise");
}

/// The files to mutate: a schedule with every key set, and random ones; each as a
/// schedule file or a template.
fn victim(rng: &mut Rng, everything: &str, everything_t: &str) -> String {
    match rng.below(4) {
        0 => everything.to_string(),
        1 => everything_t.to_string(),
        2 => to_template_string(&schedule(rng)),
        _ => to_string(&schedule(rng)),
    }
}

#[test]
fn mutated_schedule_files_are_refused_or_write_back() {
    let all = crate::strict_tests::everything();
    let (everything, everything_t) = (to_string(&all), to_template_string(&all));
    let mut rng = fuzz_rng(0xbad_f11e);
    for case in 0..rounds(800) {
        let t = victim(&mut rng, &everything, &everything_t);
        let mut text = t;
        let mut exact = true;
        // Usually one mutation, now and then a few.
        for _ in 0..1 + rng.below(3) / 2 {
            let (next, kept) = mutate(&mut rng, &text);
            exact &= kept;
            // A text that is no longer JSON can't be scanned again.
            if json(&next).is_none() {
                text = next;
                exact = false;
                break;
            }
            text = next;
        }
        reads_strictly(&text, exact, &format!("case {case}"));
    }
}

#[test]
fn mutated_layout_files_are_refused_or_write_back() {
    let all = crate::strict_tests::everything();
    let file = layouts_to_string(&all, &all.layouts.layouts);
    let mut rng = fuzz_rng(0x1a_7007);
    for case in 0..rounds(400) {
        let (s, t) = if rng.below(2) == 0 {
            (all.clone(), file.clone())
        } else {
            let s = schedule(&mut rng);
            let t = layouts_to_string(&s, &s.layouts.layouts);
            (s, t)
        };
        let (text, exact) = mutate(&mut rng, &t);
        let why = format!("case {case}");
        let Ok(read) = layouts_from_str(&s, &text) else {
            continue;
        };
        for (name, body) in &read {
            assert!(body.problem().is_none(), "{why}: {name}");
        }
        // An element count's category is found by name, ignoring case, and one the
        // schedule lacks counts every element (decision 147): a layout file loads into a
        // schedule, so what it writes back follows the schedule. Away from those names it
        // writes back as it was.
        let back = layouts_to_string(&s, &saved(read.clone()));
        if exact && !text.contains("category_name") {
            assert_eq!(back, text, "{why}: read, but writes back otherwise");
        }
        let again = layouts_from_str(&s, &back).unwrap_or_else(|e| panic!("{why}: {e}"));
        assert_eq!(again, read, "{why}: its own write reads otherwise");
    }
}

/// Decision 173: a key given twice used to read, the last one kept, so the file did not
/// write back as it was. Every object in the file, at any depth.
#[test]
fn a_key_given_twice_is_refused() {
    let s = crate::tests::sample();
    let text = to_string(&s);
    let twice = text.replacen("\"title\": ", "\"title\": \"Other\",\n    \"title\": ", 1);
    let e = from_str(&twice).unwrap_err().to_string();
    assert!(e.contains("key \"title\" is given twice"), "{e}");
    // Deeper: an element's name, and a key in a map of the writer's.
    let deep = text.replacen("\"name\": ", "\"name\": \"X\", \"name\": ", 1);
    assert!(from_str(&deep).is_err());
    let all = crate::strict_tests::everything();
    let text = to_string(&all);
    let at = text
        .find("\"sheet_widths\": {")
        .expect("everything sets widths")
        + 17;
    let widths = format!("{}\"synopsis\": \"wide\",{}", &text[..at], &text[at..]);
    let e = from_str(&widths).unwrap_err().to_string();
    assert!(e.contains("given twice"), "{e}");
    // The layout file reads the same way.
    let f = layouts_to_string(&all, &all.layouts.layouts);
    let twice = f.replacen("\"name\": ", "\"name\": \"X\", \"name\": ", 1);
    assert!(layouts_from_str(&all, &twice).is_err());
}
