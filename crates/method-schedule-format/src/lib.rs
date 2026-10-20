//! The native schedule file: open, documented, diff-friendly JSON (docs/FORMAT.md).
//!
//! `to_string` and `from_str` are the reference implementation. The JSON shape lives in
//! the private `File*` structs below: their field order is the key order in the file,
//! defaults are left out, and unknown fields are errors. Every number is an integer.

pub mod callsheet;
pub mod exchange;
pub mod export;
#[cfg(test)]
mod fuzz_tests;
pub mod pdf_info;
mod plan;
mod setup;
#[cfg(test)]
mod strict_tests;

use serde::{Deserialize, Serialize};

use method_schedule_core::extensions::Extensions;
use method_schedule_core::layouts::{
    LayoutBody, LayoutId, LayoutKind, LayoutUse, Library, SavedLayout,
};
use method_schedule_core::model::{
    Align, Banner, BannerColor, BannerFont, BannerStyle, Board, BoardId, Calendar, Category,
    CategoryId, DayLook, DoodRules, Element, ElementId, Event, EventKind, Minutes, PropertyValue,
    Quantity, RedFlag, RedFlagDates, RedFlagId, RedFlagType, RedFlagTypeId, Rgb, Seconds, Sheet,
    SheetId, Strip, StripColorRule, StripColors, StripColumn, StripField, StripId, StripKind,
    StripLayout, StripWidth, SubBoard, SubBoardId, TextSize, CAST_NAME,
};
use method_schedule_core::{Date, Eighths, Schedule, Weekday};

/// The `format` field every schedule file carries.
pub const FORMAT: &str = "method-schedule";

/// The file version this build writes. Before version 1 ships, changes may break files;
/// after, additive changes only.
pub const VERSION: u32 = 0;

#[derive(Debug)]
pub enum FormatError {
    Json(serde_json::Error),
    NotASchedule,
    TooNew(u32),
    BadValue(String),
    /// The file parsed but doesn't hang together (`Schedule::check`).
    Invalid(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Json(e) => write!(f, "{e}"),
            FormatError::NotASchedule => f.write_str("not a schedule file"),
            FormatError::TooNew(v) => {
                write!(
                    f,
                    "file version {v} is newer than this build reads ({VERSION})"
                )
            }
            FormatError::BadValue(why) => f.write_str(why),
            FormatError::Invalid(why) => write!(f, "schedule is inconsistent: {why}"),
        }
    }
}

impl std::error::Error for FormatError {}

/// Writes a schedule as pretty JSON with a trailing newline. The same schedule always
/// gives the same bytes.
pub fn to_string(s: &Schedule) -> String {
    write(s, false)
}

/// Writes `s` as a template ([`Schedule::to_template`]): a schedule file with no sheets,
/// strips, events or red flags, marked `"template": true` (decision 77).
pub fn to_template_string(s: &Schedule) -> String {
    write(&s.to_template(), true)
}

/// Writes `s`, a changed template, marked as a template and as it is: refused when it
/// holds what a template can't ([`Schedule::template_problem`]), so a write to a template
/// keeps it one (decision 77).
pub fn to_template_string_as_is(s: &Schedule) -> Result<String, FormatError> {
    match s.template_problem() {
        Some(why) => Err(FormatError::BadValue(why.to_string())),
        None => Ok(write(s, true)),
    }
}

fn write(s: &Schedule, template: bool) -> String {
    let file = FileTop {
        format: FORMAT.to_string(),
        version: VERSION,
        template: template.then_some(true),
        schedule: out_schedule(s),
        extensions: ext_out(s.extensions.file.as_deref()),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("schedules always serialize");
    text.push('\n');
    text
}

/// Whether the file text is marked as a template. Text that isn't a schedule file reads
/// as not a template; [`from_str`] says why.
pub fn is_template(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("template").and_then(|t| t.as_bool()))
        .unwrap_or(false)
}

/// Reads a schedule file. Strict: unknown fields, wrong types, bad dates and a schedule
/// that fails `Schedule::check` are all errors. A template reads as the schedule it
/// holds ([`is_template`] tells them apart).
pub fn from_str(text: &str) -> Result<Schedule, FormatError> {
    let (file, given): (FileTop, _) = head(text, FORMAT, || FormatError::NotASchedule)?;
    if file.template == Some(false) {
        return Err(default_given("template", "false"));
    }
    let mut file = file;
    let extensions = take_extensions(&mut file)?;
    let mut s = in_schedule(file.schedule)?;
    s.extensions = extensions;
    s.check().map_err(|e| FormatError::Invalid(e.to_string()))?;
    // Every key the writer leaves out at its default, written out at it (or as null), is
    // refused: the file would not write back byte for byte (FORMAT.md, "Defaults").
    let written = serde_json::to_value(out_schedule(&s)).map_err(FormatError::Json)?;
    if let Some(given) = given.get("schedule") {
        as_written("schedule", given, &written)?;
    }
    if given
        .get("extensions")
        .is_some_and(serde_json::Value::is_null)
    {
        return Err(default_given("extensions", "null"));
    }
    if file.template == Some(true) {
        if let Some(why) = s.template_problem() {
            return Err(FormatError::BadValue(why.to_string()));
        }
    }
    Ok(s)
}

/// Reads a file of our own JSON whose `format` must be `format`: identity first, so a
/// stranger file gets a plain answer (`stranger`), then the version, then the shape `T`.
/// Returns the shape and the file's JSON as given, for [`as_written`].
fn head<T: serde::de::DeserializeOwned>(
    text: &str,
    format: &str,
    stranger: fn() -> FormatError,
) -> Result<(T, serde_json::Value), FormatError> {
    let head: serde_json::Value = serde_json::from_str(text).map_err(FormatError::Json)?;
    if head.get("format").and_then(|v| v.as_str()) != Some(format) {
        return Err(stranger());
    }
    if let Some(v) = head.get("version").and_then(|v| v.as_u64()) {
        if v > u64::from(VERSION) {
            return Err(FormatError::TooNew(v as u32));
        }
    }
    // The JSON value keeps the last of a key given twice, so the text is read once more
    // for that: such a file would not write back as it is (decision 173).
    let mut de = serde_json::Deserializer::from_str(text);
    OnceEach::deserialize(&mut de).map_err(FormatError::Json)?;
    let shape = T::deserialize(&head).map_err(FormatError::Json)?;
    Ok((shape, head))
}

/// Any JSON value, read only to refuse an object that gives a key twice.
struct OnceEach;

impl<'de> Deserialize<'de> for OnceEach {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<OnceEach, D::Error> {
        d.deserialize_any(OnceEach)
    }
}

impl<'de> serde::de::Visitor<'de> for OnceEach {
    type Value = OnceEach;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_i64<E>(self, _: i64) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_u64<E>(self, _: u64) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_f64<E>(self, _: f64) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_str<E>(self, _: &str) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_unit<E>(self) -> Result<OnceEach, E> {
        Ok(OnceEach)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<OnceEach, A::Error> {
        while a.next_element::<OnceEach>()?.is_some() {}
        Ok(OnceEach)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<OnceEach, A::Error> {
        let mut seen = std::collections::HashSet::new();
        while let Some(k) = a.next_key::<String>()? {
            if !seen.insert(k.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "key {k:?} is given twice"
                )));
            }
            a.next_value::<OnceEach>()?;
        }
        Ok(OnceEach)
    }
}

/// Refuses a file whose JSON (`given`, at `path`) is not what the writer writes for what
/// it read (`written`): a key written as `null`, a key at its default (which the writer
/// leaves out), or a value the writer writes another way. Files the app wrote are always
/// as written; this catches hand edits and other writers.
pub(crate) fn as_written(
    path: &str,
    given: &serde_json::Value,
    written: &serde_json::Value,
) -> Result<(), FormatError> {
    use serde_json::Value;
    let bad = |why: String| Err(FormatError::BadValue(why));
    match (given, written) {
        (Value::Object(g), Value::Object(w)) => {
            for (k, v) in g {
                let at = format!("{path}.{k}");
                match w.get(k) {
                    Some(x) => as_written(&at, v, x)?,
                    None if v.is_null() => return bad(format!("{at} is null; leave the key out")),
                    None => {
                        return Err(default_given(&at, &short(v)));
                    }
                }
            }
            if let Some(k) = w.keys().find(|k| !g.contains_key(*k)) {
                return bad(format!(
                    "{path}.{k} is missing; it reads as {}",
                    short(&w[k])
                ));
            }
            Ok(())
        }
        (Value::Array(g), Value::Array(w)) if g.len() == w.len() => {
            for (i, (a, b)) in g.iter().zip(w).enumerate() {
                as_written(&format!("{path}[{i}]"), a, b)?;
            }
            Ok(())
        }
        _ if given == written => Ok(()),
        _ => bad(format!(
            "{path} is {}, which writes back as {}; write it that way",
            short(given),
            short(written)
        )),
    }
}

/// A JSON value in at most 60 characters, for a message.
fn short(v: &serde_json::Value) -> String {
    let t = v.to_string();
    if t.chars().count() <= 60 {
        t
    } else {
        format!("{}...", t.chars().take(57).collect::<String>())
    }
}

// ---- the file's shape

/// A key written with its default value: refused, since the writer leaves it out and the
/// file would not write back byte for byte (FORMAT.md, "Defaults").
pub(crate) fn default_given(key: &str, value: &str) -> FormatError {
    FormatError::BadValue(format!("{key} {value} is the default; leave the key out"))
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// An `extensions` object (decision 224): any JSON object but an empty one.
type Extension = serde_json::Map<String, serde_json::Value>;

/// The `extensions` object kept as `text` (the core's Extensions holds each as compact
/// JSON, always an object: [`take_extensions`] made it).
fn ext_out(text: Option<&str>) -> Option<Extension> {
    text.and_then(|t| serde_json::from_str(t).ok())
}

/// One `extensions` object as kept: refused when empty (the writer leaves it out).
fn ext_in(e: Option<Extension>) -> Result<Option<String>, FormatError> {
    match e {
        None => Ok(None),
        Some(m) if m.is_empty() => Err(default_given("extensions", "{}")),
        Some(m) => serde_json::to_string(&m)
            .map(Some)
            .map_err(FormatError::Json),
    }
}

/// Takes every `extensions` object out of the file, by the object it was on.
fn take_extensions(f: &mut FileTop) -> Result<Extensions, FormatError> {
    let mut x = Extensions {
        file: ext_in(f.extensions.take())?,
        schedule: ext_in(f.schedule.extensions.take())?,
        objects: Default::default(),
    };
    let mut keep = |id: u32, e: &mut Option<Extension>| -> Result<(), FormatError> {
        if let Some(text) = ext_in(e.take())? {
            x.objects.insert(id, text);
        }
        Ok(())
    };
    let s = &mut f.schedule;
    for c in &mut s.categories {
        keep(c.id, &mut c.extensions)?;
    }
    for t in &mut s.red_flag_types {
        keep(t.id, &mut t.extensions)?;
    }
    for e in &mut s.elements {
        keep(e.id, &mut e.extensions)?;
    }
    for h in &mut s.sheets {
        keep(h.id, &mut h.extensions)?;
    }
    for b in &mut s.boards {
        keep(b.id, &mut b.extensions)?;
        for sb in &mut b.sub_boards {
            keep(sb.id, &mut sb.extensions)?;
        }
    }
    Ok(x)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileTop {
    format: String,
    version: u32,
    /// A template (File > Save as Template): `true`, left out on a schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    template: Option<bool>,
    schedule: FileSchedule,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSchedule {
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    production: Option<setup::FileProduction>,
    next_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    categories: Vec<FileCategory>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    red_flag_types: Vec<FileRedFlagType>,
    /// Left out when the user set no colors (decision 60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strip_colors: Option<FileStripColors>,
    /// The Breakdown tab's header field order; left out at the default (decision 77).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sheet_fields: Vec<String>,
    /// The header fields the Breakdown tab hides, and widths set (decision 101).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sheet_hidden: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    sheet_widths: std::collections::BTreeMap<String, String>,
    /// How many shown fields each of the Breakdown tab's rows holds (decision 186).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sheet_rows: Vec<u8>,
    /// What a calendar strip shows; left out at the default (decision 102).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar_fields: Option<Vec<String>>,
    /// What the Calendar hides (decision 111); left out while it shows everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar_hidden: Option<Vec<String>>,
    /// The layout library (decision 78), left out while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layouts: Vec<FileLayout>,
    /// The strip layout the one-line report picked; none: the board's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    one_line_layout: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    elements: Vec<FileElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sheets: Vec<FileSheet>,
    boards: Vec<FileBoard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reports: Vec<setup::FileReport>,
    /// The report header layouts, left out while they are the three the list starts with
    /// (decision 99).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    header_layouts: Vec<setup::FileHeader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dood_codes: Option<setup::FileDoodCodes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan: Option<plan::FilePlan>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileStripColors {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<FileColorRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scene_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    banner_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    break_text: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileColorRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    int_ext: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    time_of_day: String,
    color: String,
    /// The pair's own text color (decision 108); left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

fn out_colors(c: &StripColors) -> Option<FileStripColors> {
    if c.is_default() {
        return None;
    }
    Some(FileStripColors {
        rules: c
            .rules
            .iter()
            .map(|r| FileColorRule {
                int_ext: r.int_ext.clone(),
                time_of_day: r.time_of_day.clone(),
                color: r.color.hex(),
                text: r.text.map(Rgb::hex),
            })
            .collect(),
        scene_text: c.scene_text.map(Rgb::hex),
        banner_text: c.banner_text.map(Rgb::hex),
        break_text: c.break_text.map(Rgb::hex),
    })
}

fn rgb(text: &str) -> Result<Rgb, FormatError> {
    Rgb::parse(text)
        .ok_or_else(|| FormatError::BadValue(format!("{text:?} is not a #rrggbb color")))
}

fn in_colors(f: Option<FileStripColors>) -> Result<StripColors, FormatError> {
    let Some(f) = f else {
        return Ok(StripColors::default());
    };
    let opt = |x: Option<String>| x.as_deref().map(rgb).transpose();
    Ok(StripColors {
        rules: f
            .rules
            .into_iter()
            .map(|r| {
                Ok(StripColorRule {
                    color: rgb(&r.color)?,
                    text: opt(r.text)?,
                    int_ext: r.int_ext,
                    time_of_day: r.time_of_day,
                })
            })
            .collect::<Result<_, FormatError>>()?,
        scene_text: opt(f.scene_text)?,
        banner_text: opt(f.banner_text)?,
        break_text: opt(f.break_text)?,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRules {
    #[serde(default, skip_serializing_if = "is_false")]
    holds: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    drops: bool,
    /// Left out at the default of 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_drop_gap: Option<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileCategory {
    id: u32,
    name: String,
    #[serde(default, skip_serializing_if = "is_false")]
    auto_board_id: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    exclude_from_board: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    defaults: Option<FileRules>,
    /// The built-in cast category. Written only once it has another name; without it,
    /// the category named `Cast` (ignoring case) is the cast.
    #[serde(default, skip_serializing_if = "is_false")]
    cast: bool,
    /// "Use in daily quantity": quantities add up in the daily totals.
    #[serde(default, skip_serializing_if = "is_false")]
    daily_quantity: bool,
    /// Custom element property names, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    properties: Vec<String>,
    /// The properties whose values take several lines, by name (decision 119).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    multi_line: Vec<String>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileElement {
    id: u32,
    category: u32,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    board_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    board_id_locked: bool,
    /// Left off the board on its own (behavior note 21). Omitted when it matches its
    /// category's `exclude_from_board`, the default a new element gets (decision 120): so
    /// an older file, where a category's switch left every one of its elements off,
    /// reads with the same meaning and writes back byte for byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exclude_from_board: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rules: Option<FileRules>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    links: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    red_flags: Vec<FileRedFlag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    properties: Vec<FileProperty>,
    /// Image paths, relative to the schedule file's folder or absolute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileProperty {
    name: String,
    value: String,
}

pub(crate) fn is_true(b: &bool) -> bool {
    *b
}

pub(crate) fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileQuantity {
    element: u32,
    count: u32,
    /// Left out when on (the default).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    daily: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRedFlagType {
    id: u32,
    name: String,
    /// `#rrggbb`; left out when the type has no color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    /// A built-in type's event kind key (`travel`, `camera_test`, `dubbing`); left out
    /// for the schedule's own types (decision 113).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    builtin: Option<String>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

/// A red flag: `from` and `to` together, or `not_before`, or `not_after`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRedFlag {
    id: u32,
    /// A red flag type id.
    #[serde(rename = "type")]
    kind: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    not_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    not_after: Option<String>,
    /// Left out when empty; `""` written out is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    /// Left out when soft; `false` written out is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hard: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSheet {
    id: u32,
    /// The sheet number, as typed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    scenes: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    int_ext: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    set: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    time_of_day: String,
    /// Minutes after midnight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sunrise: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sunset: Option<u32>,
    /// Page length in eighths.
    #[serde(default, skip_serializing_if = "is_zero")]
    eighths: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shoot_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    synopsis: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    location: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    script_pages: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    script_day: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    screen_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    unit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    sequence: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    episode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    comments: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    elements: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    quantities: Vec<FileQuantity>,
    /// Image paths, relative to the schedule file's folder or absolute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileBoard {
    id: u32,
    name: String,
    /// Left out when it is the default layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strip_layout: Option<Vec<FileStripColumn>>,
    /// The strip layout's text size step (decision 87), left out at `normal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strip_size: Option<String>,
    /// The strip layout's red flag and conflict marks (decision 136): `false` hides them,
    /// left out while they show.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    strip_red_flags: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    strip_conflicts: bool,
    /// Left out when the day breaks show their totals only (decision 67).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    day_look: Option<FileDayLook>,
    /// The named layouts it picked (decision 78), left out when it uses its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    uses: Option<FileUses>,
    sub_boards: Vec<FileSubBoard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    boneyard: Vec<FileStrip>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDayLook {
    #[serde(default, skip_serializing_if = "is_false")]
    shoot_day_strip: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    screen_time: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    sunrise: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    sunset: bool,
    /// The day's element quantities (decision 87).
    #[serde(default, skip_serializing_if = "is_false")]
    quantity: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
}

/// A day look, left out while its switches are all off and it has no text.
fn out_day_look(l: &DayLook) -> Option<FileDayLook> {
    (!l.is_default()).then(|| FileDayLook {
        shoot_day_strip: l.shoot_day_strip,
        screen_time: l.screen_time,
        sunrise: l.sunrise,
        sunset: l.sunset,
        quantity: l.quantity,
        text: l.text.clone(),
    })
}

fn in_day_look(f: Option<FileDayLook>) -> Result<DayLook, FormatError> {
    let Some(f) = f else {
        return Ok(DayLook::default());
    };
    let l = DayLook {
        shoot_day_strip: f.shoot_day_strip,
        screen_time: f.screen_time,
        sunrise: f.sunrise,
        sunset: f.sunset,
        quantity: f.quantity,
        text: f.text,
    };
    if l.is_default() {
        return Err(default_given("a day look", "{}"));
    }
    Ok(l)
}

/// One field of a strip layout; everything but `field` left out at its default.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileStripColumn {
    field: String,
    /// Left out at the field's default width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<String>,
    /// On the strip's second line (decision 78); left out on the others.
    #[serde(default, skip_serializing_if = "is_false")]
    second_line: bool,
    /// On the third or fourth line (decision 87): `3` or `4`. The second line is
    /// `second_line`, so files from before read and write back unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line: Option<u8>,
    /// How many of the first line's columns it covers, left out at 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<u8>,
    /// `left`, `center` or `right`, left out at the field's usual way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    align: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    italic: bool,
    /// A custom text field's text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
    /// The category an `element_count` counts, by id in a schedule (decision 147); left
    /// out for every element. On `category_elements`, the category it lists (decision
    /// 194).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<u32>,
    /// The same by name, in a layout file only: ids are one schedule's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category_name: Option<String>,
    /// A `category_elements` column's elements by board ID rather than by name (decision
    /// 194).
    #[serde(default, skip_serializing_if = "is_false")]
    board_ids: bool,
    /// Line styles (decision 185): a line after the cell, a rule under it.
    #[serde(default, skip_serializing_if = "is_false")]
    rule_after: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    rule_under: bool,
}

fn out_column(c: &StripColumn) -> FileStripColumn {
    FileStripColumn {
        field: c.field.name().to_string(),
        width: (c.width != c.field.default_width()).then(|| c.width.name().to_string()),
        second_line: c.line == 1,
        line: (c.line > 1).then_some(c.line + 1),
        span: (c.span != 1).then_some(c.span),
        align: out_align(c.align),
        bold: c.bold,
        italic: c.italic,
        text: c.text.clone(),
        category: c.category.map(|x| x.0),
        category_name: None,
        board_ids: c.board_ids,
        rule_after: c.rule_after,
        rule_under: c.rule_under,
    }
}

fn in_column(c: FileStripColumn) -> Result<StripColumn, FormatError> {
    let field = StripField::from_name(&c.field).ok_or_else(|| {
        FormatError::BadValue(format!("{:?} is not a strip layout field", c.field))
    })?;
    let width = match c.width.as_deref() {
        None => field.default_width(),
        Some(w) => StripWidth::from_name(w)
            .filter(|x| *x != field.default_width())
            .ok_or_else(|| match StripWidth::from_name(w) {
                Some(_) => default_given(&format!("{} width", c.field), w),
                None => FormatError::BadValue(format!("{w:?} is not a strip column width")),
            })?,
    };
    if c.span == Some(1) {
        return Err(default_given("span", "1"));
    }
    let line = match (c.second_line, c.line) {
        (false, None) => 0,
        (true, None) => 1,
        (false, Some(n @ 3..=4)) => n - 1,
        (_, Some(n)) => {
            return Err(FormatError::BadValue(format!(
                "{n} is not a strip line (3 or 4; the second is second_line)"
            )))
        }
    };
    // A layout file's names are taken out before this (`layouts_from_str`).
    if c.category_name.is_some() {
        return Err(FormatError::BadValue(
            "a schedule names a strip column's category by id (category)".to_string(),
        ));
    }
    Ok(StripColumn {
        field,
        width,
        line,
        span: c.span.unwrap_or(1),
        align: in_align(c.align.as_deref())?,
        bold: c.bold,
        italic: c.italic,
        text: c.text,
        category: c.category.map(CategoryId),
        board_ids: c.board_ids,
        rule_after: c.rule_after,
        rule_under: c.rule_under,
    })
}

/// A text size step's name, left out at `normal`.
fn out_size(t: TextSize) -> Option<String> {
    (t != TextSize::Normal).then(|| t.name().to_string())
}

pub(crate) fn in_size(t: Option<&str>) -> Result<TextSize, FormatError> {
    match t {
        None => Ok(TextSize::Normal),
        Some(t) => match TextSize::from_name(t) {
            Some(TextSize::Normal) => Err(default_given("a text size", t)),
            Some(x) => Ok(x),
            None => Err(FormatError::BadValue(format!("{t:?} is not a text size"))),
        },
    }
}

/// A named layout (decision 78), in a schedule (with its `id`) or a layout file (without).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLayout {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    name: String,
    /// `strips`, `day_breaks` or `shoot_day`.
    kind: String,
    /// A strip layout's columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    columns: Option<Vec<FileStripColumn>>,
    /// A strip layout's text size step (decision 87), left out at `normal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<String>,
    /// A strip layout's red flag and conflict marks (decision 136): `false` hides them,
    /// left out while they show.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    red_flags: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    conflicts: bool,
    /// A day break or shoot day layout's switches and text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    look: Option<FileDayLook>,
}

/// The named layouts a board picked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileUses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strips: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    day_breaks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shoot_day: Option<u32>,
}

fn out_columns(l: &StripLayout) -> Vec<FileStripColumn> {
    l.columns.iter().map(out_column).collect()
}

fn out_saved(l: &SavedLayout, with_id: bool) -> FileLayout {
    let (columns, size, look) = match &l.body {
        LayoutBody::Strips(x) => (Some(out_columns(x)), out_size(x.size), None),
        LayoutBody::DayBreaks(d) | LayoutBody::ShootDay(d) => (None, None, out_day_look(d)),
    };
    let marks = match &l.body {
        LayoutBody::Strips(x) => (!x.hide_red_flags, !x.hide_conflicts),
        _ => (true, true),
    };
    FileLayout {
        id: with_id.then_some(l.id.0),
        name: l.name.clone(),
        kind: l.body.kind().name().to_string(),
        columns,
        size,
        red_flags: marks.0,
        conflicts: marks.1,
        look,
    }
}

fn in_saved(f: FileLayout) -> Result<(Option<u32>, String, LayoutBody), FormatError> {
    let kind = LayoutKind::from_name(&f.kind)
        .ok_or_else(|| FormatError::BadValue(format!("{:?} is not a layout type", f.kind)))?;
    let other = |key: &str| {
        FormatError::BadValue(format!(
            "layout {:?}: a {} layout has no {key}",
            f.name, f.kind
        ))
    };
    let body = match kind {
        LayoutKind::Strips => {
            if f.look.is_some() {
                return Err(other("look"));
            }
            let columns = f.columns.ok_or_else(|| {
                FormatError::BadValue(format!("strip layout {:?} has no columns", f.name))
            })?;
            let mut x = in_layout(Some(columns), f.size.as_deref())?;
            x.hide_red_flags = !f.red_flags;
            x.hide_conflicts = !f.conflicts;
            LayoutBody::Strips(x)
        }
        _ if f.columns.is_some() => return Err(other("columns")),
        _ if f.size.is_some() => return Err(other("size")),
        _ if !f.red_flags => return Err(other("red_flags")),
        _ if !f.conflicts => return Err(other("conflicts")),
        LayoutKind::DayBreaks => LayoutBody::DayBreaks(in_day_look(f.look)?),
        LayoutKind::ShootDay => LayoutBody::ShootDay(in_day_look(f.look)?),
    };
    Ok((f.id, f.name, body))
}

fn out_uses(u: &LayoutUse) -> Option<FileUses> {
    (!u.is_default()).then(|| FileUses {
        strips: u.strips.map(|x| x.0),
        day_breaks: u.day_breaks.map(|x| x.0),
        shoot_day: u.shoot_day.map(|x| x.0),
    })
}

fn in_uses(f: Option<FileUses>) -> Result<LayoutUse, FormatError> {
    let Some(f) = f else {
        return Ok(LayoutUse::default());
    };
    let u = LayoutUse {
        strips: f.strips.map(LayoutId),
        day_breaks: f.day_breaks.map(LayoutId),
        shoot_day: f.shoot_day.map(LayoutId),
    };
    if u.is_default() {
        return Err(default_given("uses", "{}"));
    }
    Ok(u)
}

/// The `format` field every layout file carries (decision 78).
pub const LAYOUTS_FORMAT: &str = "method-schedule-layouts";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLayoutsTop {
    format: String,
    version: u32,
    layouts: Vec<FileLayout>,
}

/// Writes named layouts as a layout file (our own JSON, decision 78): no ids, so a file
/// can be loaded into any schedule. An element count's category, and a category's
/// elements' (decision 194), goes by its name in `s` (`category_name`, decision 147), as
/// category ids are one schedule's.
pub fn layouts_to_string(s: &Schedule, layouts: &[SavedLayout]) -> String {
    let names: Vec<Vec<Option<String>>> = layouts
        .iter()
        .map(|l| match &l.body {
            LayoutBody::Strips(x) => x
                .columns
                .iter()
                .map(|c| {
                    c.category
                        .and_then(|id| s.category(id))
                        .map(|c| c.name.clone())
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    let top = FileLayoutsTop {
        format: LAYOUTS_FORMAT.to_string(),
        version: VERSION,
        layouts: layouts
            .iter()
            .zip(&names)
            .map(|(l, names)| named_columns(out_saved(l, false), names))
            .collect(),
    };
    let mut text = serde_json::to_string_pretty(&top).expect("layouts always serialize");
    text.push('\n');
    text
}

/// A written layout's columns with their element counts' categories by name.
fn named_columns(mut f: FileLayout, names: &[Option<String>]) -> FileLayout {
    if let Some(columns) = &mut f.columns {
        for (c, name) in columns.iter_mut().zip(names) {
            c.category = None;
            c.category_name = name.clone();
        }
    }
    f
}

/// Reads a layout file to load into `s`: each layout's name and body, checked. Strict, as
/// schedules are. An element count's category (or a category's elements', decision 194)
/// is found in `s` by name, ignoring case (decision 147); a name `s` lacks leaves that
/// count counting every element, and that list empty.
pub fn layouts_from_str(
    s: &Schedule,
    text: &str,
) -> Result<Vec<(String, LayoutBody)>, FormatError> {
    let (top, given): (FileLayoutsTop, _) = head(text, LAYOUTS_FORMAT, || {
        FormatError::BadValue("not a layout file".to_string())
    })?;
    let read = top
        .layouts
        .into_iter()
        .map(|mut f| {
            // The names come out first; the rest reads as a schedule's layout does.
            let mut names: Vec<Option<String>> = Vec::new();
            for c in f.columns.iter_mut().flatten() {
                if c.category.is_some() {
                    return Err(FormatError::BadValue(format!(
                        "layout {:?}: a layout file names an element count's category by \
                         name (category_name), not id",
                        f.name
                    )));
                }
                if let Some(n) = &c.category_name {
                    if !StripField::from_name(&c.field).is_some_and(StripField::picks_category) {
                        return Err(FormatError::BadValue(format!(
                            "layout {:?}: only an element_count or category_elements picks a \
                             category",
                            f.name
                        )));
                    }
                    if n.trim().is_empty() || n.trim() != n {
                        return Err(FormatError::BadValue(format!(
                            "layout {:?}: {n:?} is not a category name",
                            f.name
                        )));
                    }
                }
                names.push(c.category_name.take());
            }
            let (id, name, mut body) = in_saved(f)?;
            if id.is_some() {
                return Err(FormatError::BadValue(format!(
                    "layout {name:?}: a layout file has no ids; each schedule gives its own"
                )));
            }
            if let Some(why) = body.problem() {
                return Err(FormatError::Invalid(format!("layout {name:?}: {why}")));
            }
            if let LayoutBody::Strips(x) = &mut body {
                for (c, n) in x.columns.iter_mut().zip(&names) {
                    c.category = n.as_deref().and_then(|n| s.category_named(n));
                }
            }
            Ok((name, body, names))
        })
        .collect::<Result<Vec<_>, FormatError>>()?;
    // Written back with the names as given, so a name `s` lacks still reads as written.
    let written: Vec<FileLayout> = read
        .iter()
        .map(|(name, body, names)| {
            named_columns(
                out_saved(
                    &SavedLayout {
                        id: LayoutId(0),
                        name: name.clone(),
                        body: body.clone(),
                    },
                    false,
                ),
                names,
            )
        })
        .collect();
    if let Some(given) = given.get("layouts") {
        let written = serde_json::to_value(written).map_err(FormatError::Json)?;
        as_written("layouts", given, &written)?;
    }
    Ok(read.into_iter().map(|(n, b, _)| (n, b)).collect())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSubBoard {
    id: u32,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar: Option<FileCalendar>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    strips: Vec<FileStrip>,
    /// Additions from other apps, kept and written back untouched (decision 224).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extensions: Option<Extension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileCalendar {
    /// Left out when it is Saturday and Sunday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weekly_off: Option<Vec<String>>,
    /// Left out when none; `[]` written out is refused (a key since decision 21).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    work_days: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    off_days: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    holidays: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    moves: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    events: Vec<FileEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prep_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrap_end: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEvent {
    /// `travel`, `fitting`, `rehearsal` or `other`.
    kind: String,
    /// For `other`: what kind it is.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    from: String,
    to: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    elements: Vec<u32>,
    /// One of the ten event colors by key (decision 109); left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum FileStrip {
    Sheet {
        id: u32,
        sheet: u32,
    },
    Banner {
        id: u32,
        text: String,
        #[serde(default, skip_serializing_if = "is_zero")]
        minutes: u32,
        /// The banner's look (decision 65), each left out at its default.
        #[serde(default, skip_serializing_if = "is_false")]
        bold: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        tall: bool,
        /// Its text size step and alignment (decision 87): `small` or `large`, `center`
        /// or `right`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        align: Option<String>,
        /// Its font family (decision 187): `serif`, `sans` or `mono`; left out for the
        /// board's own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
    },
    DayBreak {
        id: u32,
    },
}

// ---- writing

fn out_rules(r: &DoodRules) -> Option<FileRules> {
    (*r != DoodRules::default()).then(|| FileRules {
        holds: r.holds,
        drops: r.drops,
        min_drop_gap: (r.min_drop_gap != 1).then_some(r.min_drop_gap),
    })
}

fn out_dates(ds: &[Date]) -> Vec<String> {
    let mut v: Vec<Date> = ds.to_vec();
    v.sort();
    v.dedup();
    v.iter().map(Date::to_string).collect()
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

fn out_calendar(c: &Calendar) -> Option<FileCalendar> {
    if *c == Calendar::default() {
        return None;
    }
    let mut off: Vec<Weekday> = c.weekly_off.clone();
    off.sort_by_key(|w| Weekday::ALL.iter().position(|x| x == w));
    off.dedup();
    let weekly_off = (off != Calendar::default().weekly_off)
        .then(|| off.iter().map(|w| weekday_name(*w).to_string()).collect());
    Some(FileCalendar {
        weekly_off,
        work_days: (!c.work_days.is_empty()).then(|| out_dates(&c.work_days)),
        off_days: out_dates(&c.off_days),
        holidays: out_dates(&c.holidays),
        moves: out_dates(&c.moves),
        events: c
            .events
            .iter()
            .map(|e| {
                let label = match &e.kind {
                    EventKind::Other(l) => l.clone(),
                    _ => String::new(),
                };
                let kind = e.kind.key();
                FileEvent {
                    kind: kind.to_string(),
                    label,
                    name: e.name.clone(),
                    from: e.from.to_string(),
                    to: e.to.to_string(),
                    elements: e.elements.iter().map(|x| x.0).collect(),
                    color: e
                        .color
                        .and_then(|c| method_schedule_core::model::EVENT_COLORS.get(usize::from(c)))
                        .map(|(k, _, _)| k.to_string()),
                }
            })
            .collect(),
        prep_start: c.prep_start.map(|d| d.to_string()),
        wrap_end: c.wrap_end.map(|d| d.to_string()),
    })
}

fn out_red_flag(f: &RedFlag) -> FileRedFlag {
    let mut out = FileRedFlag {
        id: f.id.0,
        kind: f.kind.0,
        from: None,
        to: None,
        not_before: None,
        not_after: None,
        note: (!f.note.is_empty()).then(|| f.note.clone()),
        hard: f.hard.then_some(true),
    };
    match f.dates {
        RedFlagDates::Range { from, to } => {
            out.from = Some(from.to_string());
            out.to = Some(to.to_string());
        }
        RedFlagDates::NotBefore(d) => out.not_before = Some(d.to_string()),
        RedFlagDates::NotAfter(d) => out.not_after = Some(d.to_string()),
    }
    out
}

/// A board's own strip layout: its columns (left out when they are the default ones)
/// and its text size.
fn out_layout(l: &StripLayout) -> (Option<Vec<FileStripColumn>>, Option<String>) {
    (
        (!l.default_columns()).then(|| out_columns(l)),
        out_size(l.size),
    )
}

/// An alignment's name, left out at `Auto`.
pub(crate) fn out_align(a: Align) -> Option<String> {
    (a != Align::Auto).then(|| a.name().to_string())
}

pub(crate) fn in_align(a: Option<&str>) -> Result<Align, FormatError> {
    match a {
        None => Ok(Align::Auto),
        Some(a) => match Align::from_name(a) {
            Some(Align::Auto) => Err(default_given("an alignment", a)),
            Some(x) => Ok(x),
            None => Err(FormatError::BadValue(format!("{a:?} is not an alignment"))),
        },
    }
}

fn out_strip(st: &Strip) -> FileStrip {
    match &st.kind {
        StripKind::Sheet(id) => FileStrip::Sheet {
            id: st.id.0,
            sheet: id.0,
        },
        StripKind::Banner(b) => FileStrip::Banner {
            id: st.id.0,
            text: b.text.clone(),
            minutes: b.duration.0,
            bold: b.style.bold,
            color: b.style.color.map(BannerColor::text),
            tall: b.style.tall,
            size: out_size(b.style.size),
            align: out_align(b.style.align),
            font: (b.style.font != BannerFont::Usual).then(|| b.style.font.name().to_string()),
        },
        StripKind::DayBreak => FileStrip::DayBreak { id: st.id.0 },
    }
}

fn out_schedule(s: &Schedule) -> FileSchedule {
    FileSchedule {
        title: s.title.clone(),
        production: setup::out_production(&s.production),
        next_id: s.next_id,
        strip_colors: out_colors(&s.strip_colors),
        sheet_fields: s.sheet_fields.iter().map(|k| k.to_string()).collect(),
        calendar_fields: (!s.calendar_fields.is_empty()).then(|| {
            s.calendar_fields
                .iter()
                .map(|f| f.name().to_string())
                .collect()
        }),
        calendar_hidden: (!s.calendar_hidden.is_empty()).then(|| {
            s.calendar_hidden
                .iter()
                .map(|c| c.key().to_string())
                .collect()
        }),
        sheet_hidden: s.sheet_looks.hidden.iter().map(|k| k.to_string()).collect(),
        sheet_widths: s
            .sheet_looks
            .widths
            .iter()
            .map(|(k, w)| (k.to_string(), w.name().to_string()))
            .collect(),
        sheet_rows: s.sheet_looks.rows.clone(),
        layouts: s
            .layouts
            .layouts
            .iter()
            .map(|l| out_saved(l, true))
            .collect(),
        one_line_layout: s.layouts.one_line.map(|x| x.0),
        categories: s
            .categories
            .iter()
            .map(|c| FileCategory {
                id: c.id.0,
                name: c.name.clone(),
                auto_board_id: c.auto_board_id,
                exclude_from_board: c.exclude_from_board,
                defaults: out_rules(&c.defaults),
                cast: c.cast && !c.name.trim().eq_ignore_ascii_case(CAST_NAME),
                daily_quantity: c.daily_quantity,
                properties: c.properties.clone(),
                multi_line: c.multi_line.clone(),
                extensions: ext_out(s.extensions.of(c.id.0)),
            })
            .collect(),
        red_flag_types: s
            .red_flag_types
            .iter()
            .map(|t| FileRedFlagType {
                id: t.id.0,
                name: t.name.clone(),
                color: t.color.map(Rgb::hex),
                builtin: t.builtin.as_ref().map(|k| k.key().to_string()),
                extensions: ext_out(s.extensions.of(t.id.0)),
            })
            .collect(),
        elements: s
            .elements
            .iter()
            .map(|e| FileElement {
                id: e.id.0,
                category: e.category.0,
                name: e.name.clone(),
                board_id: e.board_id.clone(),
                board_id_locked: e.board_id_locked,
                exclude_from_board: {
                    let default = s.category(e.category).is_some_and(|c| c.exclude_from_board);
                    (e.exclude_from_board != default).then_some(e.exclude_from_board)
                },
                rules: out_rules(&e.rules),
                notes: e.notes.clone(),
                links: e.links.iter().map(|x| x.0).collect(),
                red_flags: e.red_flags.iter().map(out_red_flag).collect(),
                properties: e
                    .properties
                    .iter()
                    .map(|p| FileProperty {
                        name: p.name.clone(),
                        value: p.value.clone(),
                    })
                    .collect(),
                images: e.images.clone(),
                extensions: ext_out(s.extensions.of(e.id.0)),
            })
            .collect(),
        sheets: s
            .sheets
            .iter()
            .map(|h| FileSheet {
                id: h.id.0,
                number: h.number.clone(),
                scenes: h.scenes.clone(),
                int_ext: h.int_ext.clone(),
                set: h.set.clone(),
                time_of_day: h.time_of_day.clone(),
                sunrise: h.sunrise,
                sunset: h.sunset,
                eighths: h.pages.0,
                shoot_minutes: h.shoot_time.map(|m| m.0),
                synopsis: h.synopsis.clone(),
                location: h.location.clone(),
                script_pages: h.script_pages.clone(),
                script_day: h.script_day.clone(),
                screen_seconds: h.screen_time.map(|x| x.0),
                unit: h.unit.clone(),
                sequence: h.sequence.clone(),
                episode: h.episode.clone(),
                comments: h.comments.clone(),
                elements: h.elements.iter().map(|x| x.0).collect(),
                quantities: h
                    .quantities
                    .iter()
                    .map(|q| FileQuantity {
                        element: q.element.0,
                        count: q.count,
                        daily: q.daily,
                    })
                    .collect(),
                images: h.images.clone(),
                extensions: ext_out(s.extensions.of(h.id.0)),
            })
            .collect(),
        boards: s
            .boards
            .iter()
            .map(|b| {
                let (strip_layout, strip_size) = out_layout(&b.strip_layout);
                FileBoard {
                    id: b.id.0,
                    name: b.name.clone(),
                    strip_layout,
                    strip_size,
                    strip_red_flags: !b.strip_layout.hide_red_flags,
                    strip_conflicts: !b.strip_layout.hide_conflicts,
                    day_look: out_day_look(&b.day_look),
                    uses: out_uses(&b.uses),
                    sub_boards: b
                        .sub_boards
                        .iter()
                        .map(|sb| FileSubBoard {
                            id: sb.id.0,
                            name: sb.name.clone(),
                            start: sb.start.map(|d| d.to_string()),
                            calendar: out_calendar(&sb.calendar),
                            strips: sb.strips.iter().map(out_strip).collect(),
                            extensions: ext_out(s.extensions.of(sb.id.0)),
                        })
                        .collect(),
                    boneyard: b.boneyard.iter().map(out_strip).collect(),
                    extensions: ext_out(s.extensions.of(b.id.0)),
                }
            })
            .collect(),
        reports: setup::out_reports(&s.reports),
        header_layouts: s.header_layouts.iter().map(setup::out_header).collect(),
        dood_codes: setup::out_codes(&s.dood_codes),
        plan: plan::out_plan(&s.plan),
        extensions: ext_out(s.extensions.schedule.as_deref()),
    }
}

// ---- reading

pub(crate) fn date(s: &str) -> Result<Date, FormatError> {
    s.parse()
        .map_err(|_| FormatError::BadValue(format!("{s:?} is not a YYYY-MM-DD date")))
}

fn dates(v: &[String]) -> Result<Vec<Date>, FormatError> {
    v.iter().map(|s| date(s)).collect()
}

fn minute_of_day(m: Option<u32>, what: &str) -> Result<Option<u32>, FormatError> {
    match m {
        Some(m) if m >= 24 * 60 => Err(FormatError::BadValue(format!(
            "{what} {m} is past midnight"
        ))),
        _ => Ok(m),
    }
}

fn in_rules(r: Option<FileRules>) -> DoodRules {
    r.map_or_else(DoodRules::default, |r| DoodRules {
        holds: r.holds,
        drops: r.drops,
        min_drop_gap: r.min_drop_gap.unwrap_or(1),
    })
}

fn in_red_flag(f: FileRedFlag) -> Result<RedFlag, FormatError> {
    let dates = match (&f.from, &f.to, &f.not_before, &f.not_after) {
        (Some(a), Some(b), None, None) => {
            let (from, to) = (date(a)?, date(b)?);
            if to < from {
                return Err(FormatError::BadValue(format!(
                    "red flag {} ends ({to}) before it starts ({from})",
                    f.id
                )));
            }
            RedFlagDates::Range { from, to }
        }
        (None, None, Some(d), None) => RedFlagDates::NotBefore(date(d)?),
        (None, None, None, Some(d)) => RedFlagDates::NotAfter(date(d)?),
        _ => {
            return Err(FormatError::BadValue(format!(
                "red flag {} needs from and to, or not_before, or not_after",
                f.id
            )))
        }
    };
    if f.note.as_deref() == Some("") {
        return Err(default_given("a red flag's note", "\"\""));
    }
    if f.hard == Some(false) {
        return Err(default_given("a red flag's hard", "false"));
    }
    Ok(RedFlag {
        id: RedFlagId(f.id),
        kind: RedFlagTypeId(f.kind),
        dates,
        note: f.note.unwrap_or_default(),
        hard: f.hard.unwrap_or(false),
    })
}

fn in_layout(
    l: Option<Vec<FileStripColumn>>,
    size: Option<&str>,
) -> Result<StripLayout, FormatError> {
    let columns = match l {
        None => StripLayout::default().columns,
        Some(columns) => columns
            .into_iter()
            .map(in_column)
            .collect::<Result<Vec<_>, FormatError>>()?,
    };
    Ok(StripLayout {
        size: in_size(size)?,
        ..StripLayout::of(columns)
    })
}

fn in_strip(f: FileStrip) -> Result<Strip, FormatError> {
    Ok(match f {
        FileStrip::Sheet { id, sheet } => Strip {
            id: StripId(id),
            kind: StripKind::Sheet(SheetId(sheet)),
        },
        FileStrip::Banner {
            id,
            text,
            minutes,
            bold,
            color,
            tall,
            size,
            align,
            font,
        } => Strip {
            id: StripId(id),
            kind: StripKind::Banner(Banner {
                text,
                duration: Minutes(minutes),
                style: BannerStyle {
                    bold,
                    color: match color {
                        None => None,
                        Some(c) => Some(BannerColor::from_text(&c).ok_or_else(|| {
                            FormatError::BadValue(format!(
                                "{c:?} is not a banner color (a palette name or a lowercase #rrggbb)"
                            ))
                        })?),
                    },
                    tall,
                    size: in_size(size.as_deref())?,
                    align: in_align(align.as_deref())?,
                    font: match font.as_deref() {
                        None => BannerFont::Usual,
                        Some(f) => BannerFont::from_name(f)
                            .filter(|x| *x != BannerFont::Usual)
                            .ok_or_else(|| {
                                FormatError::BadValue(format!(
                                    "{f:?} is not a banner font (serif, sans or mono)"
                                ))
                            })?,
                    },
                },
            }),
        },
        FileStrip::DayBreak { id } => Strip {
            id: StripId(id),
            kind: StripKind::DayBreak,
        },
    })
}

fn in_calendar(c: Option<FileCalendar>) -> Result<Calendar, FormatError> {
    let Some(c) = c else {
        return Ok(Calendar::default());
    };
    let weekly_off = match c.weekly_off {
        None => Calendar::default().weekly_off,
        Some(names) => names
            .iter()
            .map(|n| {
                Weekday::ALL
                    .iter()
                    .copied()
                    .find(|w| weekday_name(*w) == n)
                    .ok_or_else(|| {
                        FormatError::BadValue(format!("{n:?} is not a weekday (mon to sun)"))
                    })
            })
            .collect::<Result<_, _>>()?,
    };
    let events = c
        .events
        .into_iter()
        .map(|e| {
            let Some(kind) = EventKind::from_key(&e.kind, &e.label) else {
                return Err(FormatError::BadValue(format!(
                    "{:?} is not an event kind",
                    e.kind
                )));
            };
            if !e.label.is_empty() && !matches!(kind, EventKind::Other(_)) {
                return Err(FormatError::BadValue(format!(
                    "a {} event has no label",
                    e.kind
                )));
            }
            let color = match e.color.as_deref() {
                None => None,
                Some(k) => Some(method_schedule_core::model::event_color(k).ok_or_else(|| {
                    FormatError::BadValue(format!("{k:?} is not an event color"))
                })?),
            };
            let (from, to) = (date(&e.from)?, date(&e.to)?);
            if to < from {
                return Err(FormatError::BadValue(format!(
                    "event ends ({to}) before it starts ({from})"
                )));
            }
            // As every edit holds it (decision 21): no longer than MAX_EVENT_DAYS.
            if to.0 - from.0 >= method_schedule_core::model::MAX_EVENT_DAYS {
                return Err(FormatError::BadValue(format!(
                    "event {from} to {to} is longer than {} days",
                    method_schedule_core::model::MAX_EVENT_DAYS
                )));
            }
            Ok(Event {
                name: e.name,
                kind,
                from,
                to,
                elements: e.elements.into_iter().map(ElementId).collect(),
                color,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(Calendar {
        weekly_off,
        work_days: match &c.work_days {
            Some(list) if list.is_empty() => return Err(default_given("work_days", "[]")),
            Some(list) => dates(list)?,
            None => Vec::new(),
        },
        off_days: dates(&c.off_days)?,
        holidays: dates(&c.holidays)?,
        moves: dates(&c.moves)?,
        events,
        prep_start: c.prep_start.as_deref().map(date).transpose()?,
        wrap_end: c.wrap_end.as_deref().map(date).transpose()?,
    })
}

fn in_schedule(f: FileSchedule) -> Result<Schedule, FormatError> {
    let strip_colors = in_colors(f.strip_colors)?;
    let layouts = Library {
        layouts: f
            .layouts
            .into_iter()
            .map(|l| {
                let (id, name, body) = in_saved(l)?;
                let id =
                    id.ok_or_else(|| FormatError::BadValue(format!("layout {name:?} has no id")))?;
                Ok(SavedLayout {
                    id: LayoutId(id),
                    name,
                    body,
                })
            })
            .collect::<Result<_, FormatError>>()?,
        one_line: f.one_line_layout.map(LayoutId),
    };
    let marked = f.categories.iter().any(|c| c.cast);
    let mut categories: Vec<Category> = f
        .categories
        .into_iter()
        .map(|c| Category {
            id: CategoryId(c.id),
            name: c.name,
            auto_board_id: c.auto_board_id,
            exclude_from_board: c.exclude_from_board,
            defaults: in_rules(c.defaults),
            cast: c.cast,
            daily_quantity: c.daily_quantity,
            properties: c.properties,
            multi_line: c.multi_line,
        })
        .collect();
    if !marked {
        if let Some(c) = categories
            .iter_mut()
            .find(|c| c.name.trim().eq_ignore_ascii_case(CAST_NAME))
        {
            c.cast = true;
        }
    }
    let red_flag_types =
        f.red_flag_types
            .into_iter()
            .map(|t| {
                Ok(RedFlagType {
                    id: RedFlagTypeId(t.id),
                    builtin: t
                        .builtin
                        .as_deref()
                        .map(|k| match EventKind::from_key(k, "") {
                            Some(EventKind::Other(_)) | None => Err(FormatError::BadValue(
                                format!("red flag type {:?}: no built-in type {k:?}", t.name),
                            )),
                            Some(kind) => Ok(kind),
                        })
                        .transpose()?,
                    name: t.name,
                    color: t.color.as_deref().map(rgb).transpose()?,
                })
            })
            .collect::<Result<_, FormatError>>()?;
    // An element without its own setting takes its category's (decision 120).
    let category_excluded: std::collections::HashMap<u32, bool> = categories
        .iter()
        .map(|c: &Category| (c.id.0, c.exclude_from_board))
        .collect();
    let elements = f
        .elements
        .into_iter()
        .map(|e| {
            Ok(Element {
                id: ElementId(e.id),
                category: CategoryId(e.category),
                name: e.name,
                board_id: e.board_id,
                board_id_locked: e.board_id_locked,
                exclude_from_board: e.exclude_from_board.unwrap_or_else(|| {
                    category_excluded.get(&e.category).copied().unwrap_or(false)
                }),
                rules: in_rules(e.rules),
                notes: e.notes,
                links: e.links.into_iter().map(ElementId).collect(),
                red_flags: e
                    .red_flags
                    .into_iter()
                    .map(in_red_flag)
                    .collect::<Result<_, _>>()?,
                properties: e
                    .properties
                    .into_iter()
                    .map(|p| PropertyValue {
                        name: p.name,
                        value: p.value,
                    })
                    .collect(),
                images: e.images,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    let sheets = f
        .sheets
        .into_iter()
        .map(|h| {
            Ok(Sheet {
                id: SheetId(h.id),
                number: h.number,
                scenes: h.scenes,
                int_ext: h.int_ext,
                set: h.set,
                time_of_day: h.time_of_day,
                sunrise: minute_of_day(h.sunrise, "sunrise")?,
                sunset: minute_of_day(h.sunset, "sunset")?,
                pages: Eighths(h.eighths),
                shoot_time: h.shoot_minutes.map(Minutes),
                synopsis: h.synopsis,
                location: h.location,
                script_pages: h.script_pages,
                script_day: h.script_day,
                screen_time: h.screen_seconds.map(Seconds),
                unit: h.unit,
                sequence: h.sequence,
                episode: h.episode,
                comments: h.comments,
                elements: h.elements.into_iter().map(ElementId).collect(),
                quantities: h
                    .quantities
                    .into_iter()
                    .map(|q| Quantity {
                        element: ElementId(q.element),
                        count: q.count,
                        daily: q.daily,
                    })
                    .collect(),
                images: h.images,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    for (key, list) in [
        ("calendar_fields", &f.calendar_fields),
        ("calendar_hidden", &f.calendar_hidden),
    ] {
        if list.as_ref().is_some_and(Vec::is_empty) {
            return Err(default_given(key, "[]"));
        }
    }
    let calendar_fields = f
        .calendar_fields
        .iter()
        .flatten()
        .map(|n| {
            StripField::from_name(n)
                .ok_or_else(|| FormatError::BadValue(format!("{n:?} is not a field")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if calendar_fields.as_slice() == method_schedule_core::model::DEFAULT_CALENDAR_FIELDS {
        return Err(default_given(
            "calendar_fields",
            "scene, set, elements, cast",
        ));
    }
    let mut calendar_hidden = Vec::new();
    for k in f.calendar_hidden.iter().flatten() {
        let c = method_schedule_core::model::CalendarShow::from_key(k).ok_or_else(|| {
            FormatError::BadValue(format!("{k:?} is not something the calendar shows"))
        })?;
        if calendar_hidden.last().is_some_and(|l| *l >= c) {
            return Err(FormatError::BadValue(
                "calendar_hidden is written in its order, each once".to_string(),
            ));
        }
        calendar_hidden.push(c);
    }
    let sheet_looks = method_schedule_core::model::SheetLooks::read(
        &f.sheet_hidden,
        &f.sheet_widths.into_iter().collect::<Vec<_>>(),
    )
    .and_then(|l| l.with_rows(&f.sheet_rows))
    .map_err(|e| FormatError::BadValue(e.to_string()))?;
    let sheet_fields = f
        .sheet_fields
        .iter()
        .map(|k| {
            method_schedule_core::model::sheet_field_key(k)
                .ok_or_else(|| FormatError::BadValue(format!("{k:?} is not a sheet field")))
        })
        .collect::<Result<_, _>>()?;
    let boards = f
        .boards
        .into_iter()
        .map(|b| {
            Ok(Board {
                id: BoardId(b.id),
                name: b.name,
                strip_layout: StripLayout {
                    hide_red_flags: !b.strip_red_flags,
                    hide_conflicts: !b.strip_conflicts,
                    ..in_layout(b.strip_layout, b.strip_size.as_deref())?
                },
                day_look: in_day_look(b.day_look)?,
                uses: in_uses(b.uses)?,
                sub_boards: b
                    .sub_boards
                    .into_iter()
                    .map(|sb| {
                        Ok(SubBoard {
                            id: SubBoardId(sb.id),
                            name: sb.name,
                            start: sb.start.as_deref().map(date).transpose()?,
                            calendar: in_calendar(sb.calendar)?,
                            strips: sb
                                .strips
                                .into_iter()
                                .map(in_strip)
                                .collect::<Result<_, FormatError>>()?,
                        })
                    })
                    .collect::<Result<_, FormatError>>()?,
                boneyard: b
                    .boneyard
                    .into_iter()
                    .map(in_strip)
                    .collect::<Result<_, FormatError>>()?,
            })
        })
        .collect::<Result<_, FormatError>>()?;
    Ok(Schedule {
        title: f.title,
        categories,
        elements,
        sheets,
        boards,
        red_flag_types,
        production: setup::in_production(f.production),
        reports: setup::in_reports(f.reports)?,
        header_layouts: f
            .header_layouts
            .into_iter()
            .map(setup::in_header)
            .collect::<Result<_, _>>()?,
        dood_codes: setup::in_codes(f.dood_codes),
        strip_colors,
        layouts,
        plan: plan::in_plan(f.plan)?,
        sheet_fields,
        sheet_looks,
        calendar_fields,
        calendar_hidden,
        next_id: f.next_id,
        extensions: Extensions::default(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use method_schedule_core::days::{auto_day_breaks, Limit};
    use method_schedule_core::fountain;
    use method_schedule_core::model::{Place, DEFAULT_CATEGORIES};
    use method_schedule_core::setup::ReportColumn;

    pub(crate) fn sample() -> Schedule {
        let mut s = Schedule::new("ZTEST Harbor Light");
        let script = fountain::parse(include_str!(
            "../../../fixtures/scripts/harbor-light.fountain"
        ));
        let cast = s.category_named("Cast").unwrap();
        s.import_script(&script, cast).unwrap();
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let ids: Vec<_> = s.boards[0].boneyard.iter().skip(1).map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            StripKind::Banner(Banner {
                text: "Company move to the harbor".into(),
                duration: Minutes(45),
                ..Default::default()
            }),
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        auto_day_breaks(&mut s, b, sb, Limit::Pages(Eighths(3))).unwrap();
        let tove = s.element_named(cast, "TOVE").unwrap();
        s.element_mut(tove).unwrap().rules = DoodRules {
            holds: true,
            drops: true,
            min_drop_gap: 10,
        };
        s.set_board_id(tove, Some("1")).unwrap();
        let sub = &mut s.boards[0].sub_boards[0];
        sub.start = "2027-03-01".parse().ok();
        sub.calendar.holidays.push("2027-03-03".parse().unwrap());
        // Saturday 2027-03-06 back to working.
        sub.calendar.work_days.push("2027-03-06".parse().unwrap());
        sub.calendar.events.push(Event {
            name: "Wardrobe".into(),
            kind: EventKind::Fitting,
            from: "2027-02-25".parse().unwrap(),
            to: "2027-02-26".parse().unwrap(),
            elements: vec![tove],
            color: None,
        });
        let away = s.add_red_flag_type("Unavailable").unwrap();
        let job = s.add_red_flag_type("Second job").unwrap();
        for (kind, dates, note, hard) in [
            (
                away,
                RedFlagDates::Range {
                    from: "2027-03-04".parse().unwrap(),
                    to: "2027-03-05".parse().unwrap(),
                },
                "Wedding",
                true,
            ),
            (
                job,
                RedFlagDates::NotBefore("2027-03-02".parse().unwrap()),
                "",
                false,
            ),
            (
                job,
                RedFlagDates::NotAfter("2027-03-20".parse().unwrap()),
                "",
                false,
            ),
        ] {
            s.set_red_flag(
                tove,
                RedFlag {
                    id: RedFlagId(0),
                    kind,
                    dates,
                    note: note.into(),
                    hard,
                },
            )
            .unwrap();
        }
        s.sheets[0].shoot_time = Some(Minutes(90));
        s.sheets[0].sunrise = Some(6 * 60 + 41);
        s.set_strip_layout(
            b,
            StripLayout::of(vec![
                StripColumn::of(StripField::Scene),
                StripColumn {
                    width: StripWidth::Wide,
                    ..StripColumn::of(StripField::Synopsis)
                },
                StripColumn::of(StripField::Cast),
            ]),
        )
        .unwrap();
        s.duplicate_board(b, "Rain plan").unwrap();
        s.check().unwrap();
        s
    }

    #[test]
    fn the_cast_category_keeps_being_the_cast_when_renamed() {
        let mut s = sample();
        let cast = s.cast().unwrap();
        // Named Cast: no marker, so older files read and write back unchanged.
        let text = to_string(&s);
        assert!(!text.contains("\"cast\": true"), "{text}");
        assert_eq!(from_str(&text).unwrap().cast(), Some(cast));
        // Renamed (behavior note 17, A10): the marker keeps it the cast, even beside a
        // user category that took the old name.
        s.rename_category(cast, "Actors").unwrap();
        let other = s.add_category("Cast");
        let text = to_string(&s);
        assert!(text.contains("\"cast\": true"), "{text}");
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.cast(), Some(cast));
        assert!(!back.category(other).unwrap().cast);
        assert_eq!(to_string(&back), text);
    }

    #[test]
    fn round_trips_exactly() {
        let s = sample();
        let text = to_string(&s);
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
    }

    #[test]
    fn a_category_left_off_the_board_reads_its_elements_as_off_and_writes_back() {
        // Decision 120: the category's switch is a default for new elements, and an
        // element's own setting decides. An older file with the category's switch on and
        // no element keys means every element is off; it writes back byte for byte.
        let mut s = sample();
        let cast = s.cast().unwrap();
        s.category_mut(cast).unwrap().exclude_from_board = true;
        for e in s.elements.iter_mut().filter(|e| e.category == cast) {
            e.exclude_from_board = true;
        }
        let text = to_string(&s);
        assert_eq!(
            text.matches("\"exclude_from_board\"").count(),
            1,
            "the category's only"
        );
        let back = from_str(&text).unwrap();
        assert!(back
            .elements
            .iter()
            .filter(|e| e.category == cast)
            .all(|e| e.exclude_from_board));
        assert_eq!(to_string(&back), text);
        // One element put back on the board says so with false.
        let one = s.elements.iter().find(|e| e.category == cast).unwrap().id;
        s.element_mut(one).unwrap().exclude_from_board = false;
        let text = to_string(&s);
        assert!(text.contains("\"exclude_from_board\": false"));
        let back = from_str(&text).unwrap();
        assert!(!back.element(one).unwrap().exclude_from_board);
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
    }

    #[test]
    fn the_layout_library_round_trips_and_layout_files_load_anywhere() {
        // Decision 78: left out while empty; ids in the schedule, none in a layout file.
        assert!(!to_string(&sample()).contains("layouts"));
        let mut s = sample();
        let b = s.boards[0].id;
        let mut two = StripLayout::default();
        two.columns.push(StripColumn {
            width: StripWidth::Widest,
            ..StripColumn::on_line(StripField::Synopsis, 1, 1)
        });
        let wide = s.add_layout("Two lines", LayoutBody::Strips(two)).unwrap();
        let night = s
            .add_layout(
                "Night",
                LayoutBody::DayBreaks(DayLook {
                    sunset: true,
                    ..DayLook::default()
                }),
            )
            .unwrap();
        let shoot = s
            .add_layout("Plain", LayoutBody::ShootDay(DayLook::default()))
            .unwrap();
        s.use_layout(b, LayoutKind::Strips, Some(wide)).unwrap();
        s.use_layout(b, LayoutKind::DayBreaks, Some(night)).unwrap();
        s.use_layout(b, LayoutKind::ShootDay, Some(shoot)).unwrap();
        s.use_one_line_layout(Some(wide)).unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"second_line\": true"), "{text}");
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // A pick of a missing layout is refused.
        let bad = text.replacen(
            &format!("\"shoot_day\": {}", shoot.0),
            "\"shoot_day\": 999999",
            1,
        );
        assert!(from_str(&bad).is_err());
        // A layout file: no ids; loads into another schedule under free names.
        let file = layouts_to_string(&s, &s.layouts.layouts);
        assert!(!file.contains("\"id\""), "{file}");
        let read = layouts_from_str(&s, &file).unwrap();
        assert_eq!(read.len(), 3);
        let mut other = Schedule::new("ZTEST other");
        other.import_layouts(read.clone()).unwrap();
        other.import_layouts(read).unwrap();
        assert_eq!(other.layouts.layouts[3].name, "Two lines 2");
        other.check().unwrap();
        assert!(
            layouts_from_str(&s, &text).is_err(),
            "a schedule is not a layout file"
        );
    }

    #[test]
    fn explicit_defaults_and_keys_of_another_layout_kind_are_refused() {
        use method_schedule_core::plan::GeoLocation;
        use serde_json::{json, Value};
        let mut s = sample();
        let b = s.boards[0].id;
        let strips = s
            .add_layout("Wide", LayoutBody::Strips(StripLayout::default()))
            .unwrap();
        let shoot = s
            .add_layout("Plain", LayoutBody::ShootDay(DayLook::default()))
            .unwrap();
        s.add_layout(
            "Night",
            LayoutBody::DayBreaks(DayLook {
                sunset: true,
                ..DayLook::default()
            }),
        )
        .unwrap();
        s.use_layout(b, LayoutKind::Strips, Some(strips)).unwrap();
        s.use_layout(b, LayoutKind::ShootDay, Some(shoot)).unwrap();
        let mut r = s.report_list()[0].clone();
        r.options.synopsis = false;
        s.set_report(0, r).unwrap();
        s.plan
            .set_location(GeoLocation {
                name: "Pier".into(),
                latitude: 1,
                longitude: 2,
                utc_offset: 0,
                daylight_saving: None,
            })
            .unwrap();
        let text = to_string(&s);
        // A shoot day layout with every switch off writes no look, and reads back so.
        assert!(!text.contains("\"look\": {}"), "{text}");
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
        let good: Value = serde_json::from_str(&text).unwrap();
        let refused = |change: &dyn Fn(&mut Value)| {
            let mut v = good.clone();
            change(&mut v);
            let changed = serde_json::to_string_pretty(&v).unwrap();
            from_str(&changed).unwrap_err().to_string()
        };
        fn named<'a>(v: &'a mut Value, name: &str) -> &'a mut Value {
            let list = v["schedule"]["layouts"].as_array_mut().unwrap();
            list.iter_mut().find(|l| l["name"] == name).unwrap()
        }
        type Change = Box<dyn Fn(&mut Value)>;
        let cases: Vec<(&str, Change)> = vec![
            (
                "has no look",
                Box::new(|v| named(v, "Wide")["look"] = json!({"sunset": true})),
            ),
            (
                "has no columns",
                Box::new(|v| named(v, "Night")["columns"] = json!([{"field": "scene"}])),
            ),
            (
                "has no size",
                Box::new(|v| named(v, "Plain")["size"] = json!("large")),
            ),
            (
                "is the default",
                Box::new(|v| named(v, "Plain")["look"] = json!({})),
            ),
            (
                "is the default",
                Box::new(|v| named(v, "Wide")["columns"] = json!([{"field": "scene", "span": 1}])),
            ),
            (
                "is the default",
                Box::new(|v| named(v, "Wide")["size"] = json!("normal")),
            ),
            (
                "is the default",
                Box::new(|v| v["schedule"]["boards"][0]["day_look"] = json!({})),
            ),
            (
                "is the default",
                Box::new(|v| v["schedule"]["boards"][0]["uses"] = json!({})),
            ),
            (
                "is the default",
                Box::new(|v| v["schedule"]["plan"]["locations"] = json!([])),
            ),
            (
                "trimmed",
                Box::new(|v| v["schedule"]["plan"]["locations"][0]["name"] = json!(" Pier")),
            ),
            (
                "is the default",
                Box::new(|v| v["schedule"]["reports"][0]["text"] = json!("normal")),
            ),
            (
                "is the default",
                Box::new(|v| v["schedule"]["reports"][0]["header"] = json!("full")),
            ),
        ];
        for (want, change) in &cases {
            let e = refused(change.as_ref());
            assert!(e.contains(want), "{want}: {e}");
        }
        // A report's orientation written as its kind's default.
        let kind = good["schedule"]["reports"][0]["kind"].as_str().unwrap();
        let landscape = setup_kind_landscape(kind);
        let e = refused(&|v: &mut Value| {
            v["schedule"]["reports"][0]["orientation"] =
                json!(if landscape { "landscape" } else { "portrait" })
        });
        assert!(e.contains("is the default"), "{e}");
        // The top level `template: false` too.
        let e = from_str(&text.replacen("{\n", "{\n  \"template\": false,\n", 1))
            .unwrap_err()
            .to_string();
        assert!(e.contains("is the default"), "{e}");
        // A layout file has no ids.
        let file = layouts_to_string(&s, &s.layouts.layouts);
        let with_id = file.replacen("\"name\"", "\"id\": 4,\n      \"name\"", 1);
        let e = layouts_from_str(&s, &with_id).unwrap_err().to_string();
        assert!(e.contains("no ids"), "{e}");
    }

    fn setup_kind_landscape(kind: &str) -> bool {
        method_schedule_core::setup::ReportKind::from_name(kind)
            .unwrap()
            .landscape_by_default()
    }

    #[test]
    fn the_strip_designer_round_trips_and_its_keys_are_left_out_at_their_defaults() {
        // Decision 87: four lines (the second still `second_line`), spans, alignment,
        // bold, italic, custom text and a text size per layout.
        let plain = to_string(&sample());
        for key in [
            "\"span\"",
            "\"align\"",
            "\"italic\"",
            "strip_size",
            "\"line\"",
            "rule_after",
            "rule_under",
        ] {
            assert!(!plain.contains(key), "{key}");
        }
        let mut s = sample();
        let b = s.boards[0].id;
        let layout = StripLayout {
            columns: vec![
                StripColumn {
                    bold: true,
                    align: Align::Center,
                    ..StripColumn::of(StripField::Scene)
                },
                StripColumn {
                    rule_after: true,
                    ..StripColumn::of(StripField::Set)
                },
                StripColumn::of(StripField::Spacer),
                StripColumn {
                    italic: true,
                    rule_under: true,
                    ..StripColumn::on_line(StripField::Synopsis, 1, 2)
                },
                StripColumn {
                    text: "Call 07:00".into(),
                    align: Align::Right,
                    ..StripColumn::on_line(StripField::Text, 3, 3)
                },
            ],
            size: TextSize::Large,
            ..StripLayout::default()
        };
        s.set_strip_layout(b, layout.clone()).unwrap();
        s.add_layout("Four lines", LayoutBody::Strips(layout))
            .unwrap();
        let text = to_string(&s);
        for key in [
            "\"strip_size\": \"large\"",
            "\"size\": \"large\"",
            "\"second_line\": true",
            "\"line\": 4",
            "\"span\": 3",
            "\"align\": \"center\"",
            "\"italic\": true",
            "\"text\": \"Call 07:00\"",
            "\"rule_after\": true",
            "\"rule_under\": true",
        ] {
            assert!(text.contains(key), "{key}: {text}");
        }
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        let file = layouts_to_string(&s, &s.layouts.layouts);
        assert_eq!(
            layouts_from_str(&s, &file).unwrap()[0].1,
            s.layouts.layouts[0].body
        );
        // Refused: line 2 spelled as `line`, a fifth line, an unknown alignment or size,
        // a span on the first line (the consistency check).
        for (from, to) in [
            ("\"line\": 4", "\"line\": 2"),
            ("\"line\": 4", "\"line\": 5"),
            ("\"align\": \"center\"", "\"align\": \"middle\""),
            ("\"align\": \"center\"", "\"align\": \"auto\""),
            ("\"strip_size\": \"large\"", "\"strip_size\": \"huge\""),
            ("\"strip_size\": \"large\"", "\"strip_size\": \"normal\""),
        ] {
            assert!(from_str(&text.replacen(from, to, 1)).is_err(), "{to}");
        }
        let first = text.replacen(
            "\"bold\": true",
            "\"bold\": true,\n            \"span\": 2",
            1,
        );
        assert!(from_str(&first).is_err());
        // Only the size changed: the columns stay out, the size goes in.
        let mut sized = sample();
        sized.boards[0].strip_layout.size = TextSize::Small;
        let text = to_string(&sized);
        assert!(text.contains("\"strip_size\": \"small\""));
        assert_eq!(from_str(&text).unwrap(), sized);
    }

    #[test]
    fn a_boards_day_look_round_trips_and_is_left_out_at_its_default() {
        // Decision 67.
        assert!(!to_string(&sample()).contains("day_look"));
        let mut s = sample();
        let b = s.boards[0].id;
        s.set_day_look(
            b,
            DayLook {
                shoot_day_strip: true,
                sunset: true,
                quantity: true,
                text: "Wrap by 19:00".into(),
                ..DayLook::default()
            },
        )
        .unwrap();
        let text = to_string(&s);
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        assert!(!text.contains("\"sunrise\": false"));
        // The day's quantities (decision 87).
        assert!(text.contains("\"quantity\": true"));
    }

    #[test]
    fn banner_styles_round_trip_and_unknown_colors_are_refused() {
        // Decision 65: each style field left out at its default.
        assert!(!to_string(&sample()).contains("\"tall\""));
        let mut s = sample();
        for st in s.boards[0].sub_boards[0].strips.iter_mut() {
            if let StripKind::Banner(b) = &mut st.kind {
                b.style = BannerStyle {
                    bold: true,
                    color: Some(BannerColor::Teal),
                    tall: true,
                    size: TextSize::Large,
                    align: Align::Center,
                    font: BannerFont::Mono,
                };
            }
        }
        let text = to_string(&s);
        assert!(text.contains("\"color\": \"teal\""), "{text}");
        // Its font family (decision 187); the usual one is left out, unknown ones refused.
        assert!(text.contains("\"font\": \"mono\""), "{text}");
        for bad in ["\"comic\"", "\"usual\""] {
            let t = text.replacen("\"mono\"", bad, 1);
            assert!(
                matches!(from_str(&t), Err(FormatError::BadValue(_))),
                "{bad}"
            );
        }
        // Its size and alignment (decision 87).
        assert!(text.contains("\"size\": \"large\""), "{text}");
        assert!(text.contains("\"align\": \"center\""), "{text}");
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        let bad = text.replacen("\"teal\"", "\"mauve\"", 1);
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        // A color by value (decision 154): lowercase #rrggbb, written back as read; an
        // uppercase one is refused, as it could not be.
        let hex = text.replacen("\"teal\"", "\"#3fb8a8\"", 1);
        let read = from_str(&hex).unwrap();
        assert_eq!(to_string(&read), hex);
        let upper = text.replacen("\"teal\"", "\"#3FB8A8\"", 1);
        assert!(matches!(from_str(&upper), Err(FormatError::BadValue(_))));
    }

    #[test]
    fn strip_colors_round_trip_and_files_without_them_read_unchanged() {
        // Decision 60: left out when none are set, so older files write back byte for byte.
        let plain = to_string(&sample());
        assert!(!plain.contains("strip_colors"));
        let mut s = sample();
        s.set_strip_colors(StripColors {
            rules: vec![
                StripColorRule {
                    int_ext: "EXT".into(),
                    time_of_day: "NIGHT".into(),
                    color: Rgb(0x1f3524),
                    text: Some(Rgb(0xffeeaa)),
                },
                StripColorRule {
                    int_ext: String::new(),
                    time_of_day: "DUSK".into(),
                    color: Rgb(0xffcc88),
                    text: None,
                },
            ],
            scene_text: None,
            banner_text: Some(Rgb(0x6a1b9a)),
            break_text: None,
        })
        .unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"color\": \"#1f3524\""), "{text}");
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        let bad = text.replacen("#1f3524", "#1f35", 1);
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let twice = text.replacen("\"DUSK\"", "\"night\"", 1).replacen(
            "\"color\": \"#ffcc88\"",
            "\"int_ext\": \"ext\",\n\"color\": \"#ffcc88\"",
            1,
        );
        assert!(matches!(from_str(&twice), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn a_day_break_parked_in_the_boneyard_round_trips_and_a_banner_there_is_refused() {
        // Decision 57: day breaks may sit in the Boneyard, banners may not.
        let mut s = sample();
        let b = s.boards[0].id;
        let brk = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .find(|x| x.kind == StripKind::DayBreak)
            .unwrap()
            .id;
        s.move_strips(
            b,
            &[brk],
            Place {
                sub_board: None,
                index: 0,
            },
        )
        .unwrap();
        let text = to_string(&s);
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        let mut banner = s.clone();
        let st = banner.boards[0].boneyard.remove(0);
        banner.boards[0].boneyard.insert(
            0,
            Strip {
                id: st.id,
                kind: StripKind::Banner(Banner {
                    text: "Move".into(),
                    duration: Minutes(0),
                    ..Default::default()
                }),
            },
        );
        assert!(from_str(&to_string(&banner)).is_err());
    }

    #[test]
    fn a_scene_spanning_days_round_trips_and_a_part_in_the_boneyard_is_refused() {
        // Decision 76: a scene in parts is its sheet's strip twice on one board, on a
        // sub-board; no new key, and the strict reader refuses a part in the Boneyard.
        let mut s = sample();
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let sheet = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .find_map(|x| match x.kind {
                StripKind::Sheet(h) => Some(h),
                _ => None,
            })
            .unwrap();
        let end = s.boards[0].sub_boards[0].strips.len();
        s.add_part(
            b,
            sheet,
            Place {
                sub_board: Some(sb),
                index: end,
            },
        )
        .unwrap();
        let text = to_string(&s);
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        let mut yard = s.clone();
        let part = yard.boards[0].sub_boards[0].strips.pop().unwrap();
        yard.boards[0].boneyard.push(part);
        assert!(from_str(&to_string(&yard)).is_err());
    }

    #[test]
    fn writes_the_documented_shape() {
        let text = to_string(&sample());
        assert!(text.starts_with(
            "{\n  \"format\": \"method-schedule\",\n  \"version\": 0,\n  \"schedule\": {\n    \"title\""
        ));
        assert!(text.ends_with("}\n"));
        assert!(text.contains("\"kind\": \"day_break\""));
        assert!(text.contains("\"eighths\": "));
        assert!(text.contains("\"min_drop_gap\": 10"));
        assert!(text.contains("\"red_flag_types\": [\n"));
        assert!(text.contains("\"not_before\": \"2027-03-02\""));
        assert!(text.contains("\"hard\": true"));
        assert!(!text.contains("\"hard\": false"));
        assert!(!text.contains("\"note\": \"\""));
        // Defaults are left out.
        assert!(!text.contains("\"holds\": false"));
        assert!(!text.contains("weekly_off"));
        assert!(text.contains("\"work_days\": [\n"));
        assert!(!text.contains("\"synopsis\":"));
        // The strip layout: after the board's name, widths left out at the default.
        assert!(text.contains(
            "\"name\": \"Board 1\",\n        \"strip_layout\": [\n          {\n            \"field\": \"scene\"\n          },"
        ));
        assert!(text.contains("\"field\": \"synopsis\",\n            \"width\": \"wide\""));
        assert!(!text.contains("\"width\": \"narrow\""));
        // No floats anywhere.
        for line in text.lines() {
            if let Some((_, v)) = line.split_once(": ") {
                let v = v.trim_end_matches(',');
                if v.parse::<f64>().is_ok() {
                    assert!(v.parse::<u64>().is_ok(), "non-integer number: {line}");
                }
            }
        }
    }

    #[test]
    fn rejects_strangers_and_mistakes() {
        let good = to_string(&sample());
        assert!(matches!(
            from_str("{\"format\": \"other\"}"),
            Err(FormatError::NotASchedule)
        ));
        assert!(matches!(from_str("[]"), Err(FormatError::NotASchedule)));
        assert!(matches!(from_str("not json"), Err(FormatError::Json(_))));
        let newer = good.replacen("\"version\": 0", "\"version\": 9", 1);
        assert!(matches!(from_str(&newer), Err(FormatError::TooNew(9))));
        let unknown = good.replacen("\"title\"", "\"colour\": 1,\n    \"title\"", 1);
        assert!(matches!(from_str(&unknown), Err(FormatError::Json(_))));
        let bad_date = good.replacen("2027-03-03", "2027-02-30", 1);
        assert!(matches!(from_str(&bad_date), Err(FormatError::BadValue(_))));
        let float = good.replacen("\"eighths\": 2", "\"eighths\": 2.5", 1);
        assert!(from_str(&float).is_err());
        // Red flags: one way of giving dates, in order, of a type that exists.
        let both = good.replacen(
            "\"not_before\": ",
            "\"from\": \"2027-03-01\",\n\"not_before\": ",
            1,
        );
        assert!(matches!(from_str(&both), Err(FormatError::BadValue(_))));
        let half = good.replacen("\"to\": \"2027-03-05\"", "\"not_after\": \"2027-03-05\"", 1);
        assert!(matches!(from_str(&half), Err(FormatError::BadValue(_))));
        let backwards = good.replacen("\"to\": \"2027-03-05\"", "\"to\": \"2027-03-01\"", 1);
        assert!(matches!(
            from_str(&backwards),
            Err(FormatError::BadValue(_))
        ));
        let no_type = good.replacen("\"type\": ", "\"type\": 9999", 1);
        assert!(matches!(from_str(&no_type), Err(FormatError::Invalid(_))));
        // A strip layout: known fields and widths, at least one field, each once.
        let field = good.replacen("\"field\": \"synopsis\"", "\"field\": \"colour\"", 1);
        assert!(matches!(from_str(&field), Err(FormatError::BadValue(_))));
        let width = good.replacen("\"width\": \"wide\"", "\"width\": \"huge\"", 1);
        assert!(matches!(from_str(&width), Err(FormatError::BadValue(_))));
        let twice = good.replacen("\"field\": \"synopsis\"", "\"field\": \"scene\"", 1);
        assert!(matches!(from_str(&twice), Err(FormatError::Invalid(_))));
        let at = good.find("\"strip_layout\": [").unwrap();
        let end = at + good[at..].find("\n        ],\n").unwrap() + "\n        ],\n".len();
        let empty = format!("{}\"strip_layout\": [],\n{}", &good[..at], &good[end..]);
        assert!(matches!(from_str(&empty), Err(FormatError::Invalid(_))));
        let gone = format!("{}{}", &good[..at], &good[end..].trim_start());
        let back = from_str(&gone).unwrap();
        assert!(
            back.boards[0].strip_layout.is_default(),
            "left out: the default"
        );
        // A strip pointing at a sheet that doesn't exist fails the consistency check.
        let dangling = good.replacen("\"sheet\": ", "\"sheet\": 9999", 1);
        assert!(from_str(&dangling).is_err());
    }

    #[test]
    fn the_example_in_format_md_reads_and_writes_back_unchanged() {
        let doc = include_str!("../../../docs/FORMAT.md");
        let start = doc.find("## Example").unwrap();
        let block = &doc[start..];
        let json = &block[block.find("```json\n").unwrap() + 8..];
        let json = &json[..json.find("```").unwrap()];
        let s = from_str(json).unwrap();
        assert_eq!(to_string(&s), json);
    }

    #[test]
    fn files_without_red_flags_read_unchanged() {
        // A file from before red flags (no `red_flag_types`, no `red_flags`) reads, and
        // writes back byte for byte.
        let mut s = sample();
        s.red_flag_types.clear();
        for e in &mut s.elements {
            e.red_flags.clear();
        }
        let text = to_string(&s);
        assert!(!text.contains("red_flag"));
        let back = from_str(&text).unwrap();
        assert!(back.red_flag_types.is_empty());
        assert_eq!(to_string(&back), text);
    }

    /// The calendar's and the red flags' keys written at their defaults are refused, as
    /// FORMAT.md's rule has it for every key added since the first version; so is a
    /// calendar with a date of two kinds (decision 21).
    #[test]
    fn calendar_and_red_flag_defaults_written_out_are_refused() {
        use serde_json::{json, Value};
        let s = sample();
        let text = to_string(&s);
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
        let good: Value = serde_json::from_str(&text).unwrap();
        let refused = |change: &dyn Fn(&mut Value)| {
            let mut v = good.clone();
            change(&mut v);
            from_str(&serde_json::to_string_pretty(&v).unwrap())
                .unwrap_err()
                .to_string()
        };
        fn cal(v: &mut Value) -> &mut Value {
            &mut v["schedule"]["boards"][0]["sub_boards"][0]["calendar"]
        }
        fn flag(v: &mut Value) -> &mut Value {
            let els = v["schedule"]["elements"].as_array_mut().unwrap();
            let e = els
                .iter_mut()
                .find(|e| e.get("red_flags").is_some())
                .unwrap();
            &mut e["red_flags"][0]
        }
        let e = refused(&|v| flag(v)["hard"] = json!(false));
        assert!(e.contains("hard false is the default"), "{e}");
        let e = refused(&|v| flag(v)["note"] = json!(""));
        assert!(e.contains("note \"\" is the default"), "{e}");
        let e = refused(&|v| cal(v)["work_days"] = json!([]));
        assert!(e.contains("work_days [] is the default"), "{e}");
        let e = refused(&|v| v["schedule"]["calendar_hidden"] = json!([]));
        assert!(e.contains("calendar_hidden [] is the default"), "{e}");
        let e = refused(&|v| v["schedule"]["calendar_fields"] = json!([]));
        assert!(e.contains("calendar_fields [] is the default"), "{e}");
        let e = refused(&|v| {
            let day = cal(v)["work_days"][0].clone();
            cal(v)["holidays"] = json!([day]);
        });
        assert!(
            e.contains("is in both its dates back to working and its holidays"),
            "{e}"
        );
    }

    #[test]
    fn an_element_left_off_the_board_round_trips_and_stays_out_when_off() {
        // Off (every older file): no key on any element, and the file writes back byte
        // for byte.
        let mut s = sample();
        let text = to_string(&s);
        let keys = text.matches("\"exclude_from_board\"").count();
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
        // On: one key more, read back as written.
        s.elements[0].exclude_from_board = true;
        let text = to_string(&s);
        assert_eq!(text.matches("\"exclude_from_board\"").count(), keys + 1);
        let back = from_str(&text).unwrap();
        assert!(back.elements[0].exclude_from_board);
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
    }

    #[test]
    fn files_without_strip_layouts_read_unchanged() {
        // A file from before strip layouts reads with the default and writes back byte
        // for byte; so does one whose layout is the default.
        let mut s = sample();
        for b in &mut s.boards {
            b.strip_layout = StripLayout::default();
        }
        let text = to_string(&s);
        assert!(!text.contains("strip_layout"));
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
    }

    #[test]
    fn days_and_scenes_as_shot_round_trip_and_are_checked() {
        use method_schedule_core::plan::{DayShot, SceneShot};
        use method_schedule_core::Eighths;
        let mut s = sample();
        let breaks: Vec<_> = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .filter(|x| x.kind == StripKind::DayBreak)
            .map(|x| x.id)
            .collect();
        let (d1, sheet) = (breaks[0], s.sheets[0].id);
        // A scene started one day and finished the next, from a production report.
        s.plan.days_shot.push(DayShot {
            day: d1,
            date: "2027-03-02".parse().ok(),
            call: Some(420),
            wrap: Some(1500),
        });
        s.plan.scenes_shot.push(SceneShot {
            sheet,
            day: d1,
            eighths: Eighths(5),
            complete: false,
        });
        let d2 = *breaks.get(1).unwrap_or(&d1);
        if d2 != d1 {
            s.plan.scenes_shot.push(SceneShot {
                sheet,
                day: d2,
                eighths: Eighths(3),
                complete: true,
            });
        }
        s.check().unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"days_shot\""), "{text}");
        assert!(text.contains("\"wrap\": 1500"), "past midnight is fine");
        assert!(!text.contains("\"complete\": false"), "false is left out");
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        assert_eq!(
            back.plan.eighths_shot(sheet),
            Eighths(if d2 != d1 { 8 } else { 5 })
        );
        // Refused: a wrap before the call or past two days, a call past 23:59, false
        // written, an empty day, nothing shot and not finished, the same sheet twice on a day.
        for (from, to) in [
            ("\"wrap\": 1500", "\"wrap\": 400"),
            ("\"wrap\": 1500", "\"wrap\": 2881"),
            ("\"call\": 420", "\"call\": 1440"),
            (
                "\"eighths\": 5",
                "\"eighths\": 5,\n          \"complete\": false",
            ),
            ("\"eighths\": 5", "\"eighths\": 0"),
            ("\"date\": \"2027-03-02\"", "\"date\": \"2027-02-30\""),
        ] {
            assert!(text.contains(from), "{from}");
            let bad = text.replacen(from, to, 1);
            assert!(from_str(&bad).is_err(), "{to}");
        }
        let mut twice = s.clone();
        twice.plan.scenes_shot.push(s.plan.scenes_shot[0]);
        assert!(twice.check().is_err());
        assert!(from_str(&to_string(&twice)).is_err());
        // A file without them is untouched (the sample has none).
        let plain = sample();
        assert_eq!(
            to_string(&from_str(&to_string(&plain)).unwrap()),
            to_string(&plain)
        );
    }

    #[test]
    fn plan_round_trips_and_stays_out_while_empty() {
        use method_schedule_core::plan::{set_call_time, set_turnaround, GeoLocation};
        let mut s = sample();
        let plain = to_string(&s);
        assert!(!plain.contains("\"plan\""));
        let day = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .find(|x| x.kind == StripKind::DayBreak)
            .unwrap()
            .id;
        set_call_time(&mut s, day, Some(390)).unwrap();
        let cast = s.cast().unwrap();
        set_turnaround(&mut s, cast, Some(720)).unwrap();
        s.plan.long_day = Some(600);
        s.plan
            .set_location(GeoLocation {
                name: "Pier".into(),
                latitude: 34_052_200,
                longitude: -118_243_700,
                utc_offset: -420,
                daylight_saving: None,
            })
            .unwrap();
        let b = s.boards[0].id;
        method_schedule_core::drift::lock(&mut s, b, "White", "2027-02-01".parse().ok()).unwrap();
        s.plan.omitted.push(s.sheets[0].id);
        let first = s.boards[0].sub_boards[0].strips[0].id;
        s.plan.pinned.push(first);
        s.check().unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"latitude_microdeg\": 34052200"));
        assert!(text.contains("\"utc_offset_minutes\": -420"));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // Out of range values are refused.
        let bad = text.replace("\"minutes\": 390", "\"minutes\": 1500");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let bad = text.replace("34052200", "95000000");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        // A daylight saving rule (decision 193): the standard offset and the rule's name,
        // left out for none; an unknown name, another spelling or "none" written is
        // refused.
        assert!(!text.contains("daylight_saving"));
        let mut dst = s.clone();
        dst.plan.locations[0].utc_offset = -480;
        dst.plan.locations[0].daylight_saving =
            Some(method_schedule_core::solar::DaylightSaving::UsCanada);
        let dst_text = to_string(&dst);
        let at = |k: &str| dst_text.find(k).unwrap();
        assert!(at("\"utc_offset_minutes\": -480,") < at("\"daylight_saving\": \"us_canada\""));
        assert_eq!(from_str(&dst_text).unwrap(), dst);
        for wrong in ["\"US_CANADA\"", "\"none\"", "\"eu_summer\"", "1"] {
            let bad = dst_text.replace("\"us_canada\"", wrong);
            assert!(from_str(&bad).is_err(), "{wrong}");
        }
        let eu = dst_text.replace("\"us_canada\"", "\"eu\"");
        assert_eq!(
            from_str(&eu).unwrap().plan.locations[0].daylight_saving,
            Some(method_schedule_core::solar::DaylightSaving::Eu)
        );
        // The southern rules (decision 220) read and write by their names.
        for rule in [
            method_schedule_core::solar::DaylightSaving::Australia,
            method_schedule_core::solar::DaylightSaving::NewZealand,
        ] {
            let mut south = dst.clone();
            south.plan.locations[0].daylight_saving = Some(rule);
            let text = to_string(&south);
            assert!(text.contains(&format!("\"daylight_saving\": \"{}\"", rule.name())));
            assert_eq!(from_str(&text).unwrap(), south);
        }
    }

    #[test]
    fn calendar_strip_fields_round_trip_and_stay_out_at_the_default() {
        let mut s = sample();
        assert!(!to_string(&s).contains("calendar_fields"));
        // What the Calendar hides (decision 111): additive, in its order.
        assert!(!to_string(&s).contains("calendar_hidden"));
        let mut hid = s.clone();
        hid.set_calendar_hidden(&[
            method_schedule_core::model::CalendarShow::RedFlags,
            method_schedule_core::model::CalendarShow::Pages,
        ]);
        let hid_text = to_string(&hid);
        assert!(
            hid_text
                .contains("\"calendar_hidden\": [\n      \"pages\",\n      \"red_flags\"\n    ]"),
            "{hid_text}"
        );
        assert_eq!(from_str(&hid_text).unwrap(), hid);
        let backwards = hid_text.replacen(
            "\"pages\",\n      \"red_flags\"",
            "\"red_flags\",\n      \"pages\"",
            1,
        );
        assert!(from_str(&backwards).is_err());
        let odd = hid_text.replacen("\"pages\",", "\"weather\",", 1);
        assert!(from_str(&odd).is_err());
        s.set_calendar_fields(vec![StripField::Scene, StripField::Elements])
            .unwrap();
        let text = to_string(&s);
        assert!(
            text.contains("\"calendar_fields\": [\n      \"scene\",\n      \"elements\"\n    ]")
        );
        assert_eq!(from_str(&text).unwrap(), s);
        assert!(from_str(&text.replace("\"elements\"\n", "\"spacer\"\n")).is_err());
        assert!(from_str(&text.replace("\"elements\"\n", "\"scene\"\n")).is_err());
        s.set_calendar_fields(method_schedule_core::model::DEFAULT_CALENDAR_FIELDS.to_vec())
            .unwrap();
        assert!(s.calendar_fields.is_empty());
    }

    #[test]
    fn hidden_sheet_fields_and_widths_round_trip() {
        use method_schedule_core::model::{FieldWidth, SheetLooks};
        let mut s = sample();
        let plain = to_string(&s);
        assert!(!plain.contains("sheet_hidden") && !plain.contains("sheet_widths"));
        let k = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        s.sheet_looks = SheetLooks::read(
            &k(&["sunset", "unit"]),
            &[
                ("synopsis".into(), "wide".into()),
                ("set".into(), "medium".into()),
            ],
        )
        .unwrap();
        assert_eq!(s.sheet_looks.hidden, ["unit", "sunset"]);
        assert_eq!(s.sheet_looks.width("set"), Some(FieldWidth::Medium));
        let text = to_string(&s);
        assert!(text.contains("\"sheet_hidden\": [\n      \"unit\",\n      \"sunset\"\n    ]"));
        assert!(text.contains(
            "\"sheet_widths\": {\n      \"set\": \"medium\",\n      \"synopsis\": \"wide\"\n    }"
        ));
        // A hidden field's values stay on the sheets.
        assert_eq!(from_str(&text).unwrap(), s);
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
        assert!(from_str(&text.replace("\"wide\"", "\"huge\"")).is_err());
        assert!(from_str(&text.replace("\"sunset\"\n", "\"budget\"\n")).is_err());
        // Rows of the user's own lengths (decision 186): left out at the usual rows,
        // a row of none or more fields than a sheet has refused.
        assert!(!text.contains("sheet_rows"));
        s.sheet_looks = s.sheet_looks.clone().with_rows(&[3, 3, 3]).unwrap();
        let text = to_string(&s);
        assert!(
            text.contains("\"sheet_rows\": [\n      3,\n      3,\n      3\n    ]"),
            "{text}"
        );
        assert_eq!(from_str(&text).unwrap(), s);
        for bad in ["3,\n      0", "3,\n      30"] {
            let t = text.replacen("3,\n      3", bad, 1);
            assert!(from_str(&t).is_err(), "{bad}");
        }
    }

    #[test]
    fn header_layouts_and_report_strip_options_round_trip() {
        use method_schedule_core::headers::{HeaderAlign, HeaderDesign, HeaderId, HeaderItem};
        use method_schedule_core::layouts::LayoutBody;
        use method_schedule_core::setup::PageBreak;
        let mut s = sample();
        let plain = to_string(&s);
        for key in [
            "header_layouts",
            "page_break",
            "shoot_days",
            "element_columns",
        ] {
            assert!(!plain.contains(key), "{key}");
        }
        let mut d = HeaderDesign::of(
            HeaderId(0),
            "Top",
            vec![
                vec![HeaderItem::Title, HeaderItem::Space, HeaderItem::Today],
                vec![
                    HeaderItem::Text("Draft".into()),
                    HeaderItem::Production("episode"),
                ],
            ],
        );
        d.first_page_only = false;
        d.italic = true;
        d.align = HeaderAlign::Right;
        let id = s.add_header(d).unwrap();
        let lay = s
            .add_layout("Wide", LayoutBody::Strips(Default::default()))
            .unwrap();
        let cast = s.cast().unwrap();
        let mut r = s.report_list()[0].clone();
        r.options.header = id;
        r.options.page_break = PageBreak::Week;
        r.options.shoot_days = false;
        r.options.day_breaks = false;
        r.options.total_count = true;
        r.options.element_columns = vec![cast];
        r.options.layout = Some(lay);
        s.set_report(0, r).unwrap();
        s.check().unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"header\": 4"), "{text}");
        assert!(text.contains("\"page_break\": \"week\""));
        assert!(text.contains("\"every_page\": true"));
        assert!(text.contains("\"text\": \"Draft\""));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // A pick of one of the first three is written by its old name; a number is refused.
        let bad = text.replace("\"header\": 4", "\"header\": 2");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let bad = text.replace("\"page_break\": \"week\"", "\"page_break\": \"day\"");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let bad = text.replace("\"episode\"", "\"writer\"");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        // Deleting the picked layout leaves the report on its own columns.
        s.delete_layout(lay).unwrap();
        assert_eq!(s.report_list()[0].options.layout, None);
        s.check().unwrap();
    }

    #[test]
    fn the_full_production_info_round_trips_and_stays_out_when_empty() {
        let mut s = sample();
        let plain = to_string(&s);
        assert!(!plain.contains("\"production\""));
        for (i, f) in method_schedule_core::setup::Production::FIELDS
            .iter()
            .enumerate()
        {
            s.production.set(f, &format!(" Value {i} ")).unwrap();
        }
        let text = to_string(&s);
        assert!(text.contains("\"unit_production_manager\": \"Value 3\""));
        assert!(text.contains("\"custom_4\": \"Value 15\""));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        s.production = Default::default();
        s.production.set("episode", "104").unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"production\": {\n      \"episode\": \"104\"\n    }"));
        assert!(from_str(&text.replace("\"episode\"", "\"writer\"")).is_err());
    }

    #[test]
    fn report_setups_production_codes_and_prep_round_trip() {
        use method_schedule_core::headers::HeaderId;
        use method_schedule_core::setup::{PageBreak, ReportKind, TextSize};
        let mut s = sample();
        let plain = to_string(&s);
        for key in [
            "production",
            "reports",
            "dood_codes",
            "prep_start",
            "wrap_end",
        ] {
            assert!(!plain.contains(key), "{key}");
        }
        s.production.company = "ZTEST Pictures".into();
        s.production.director = "A. Director".into();
        let cast = s.cast().unwrap();
        let mut r = s.report_list()[0].clone();
        r.name = "Cast one-line".into();
        r.options.from = "2027-03-02".parse().ok();
        r.options.landscape = Some(true);
        r.options.synopsis = false;
        r.options.cast_names = true;
        r.options.page_break = PageBreak::Day;
        r.options.banners = false;
        r.options.categories = vec![cast];
        r.options.colors = true;
        r.options.icons = false;
        r.options.header = HeaderId::COMPACT;
        r.options.text = TextSize::Small;
        r.options.columns = vec![
            ReportColumn::of(StripField::Scene),
            ReportColumn::of(StripField::Set),
        ];
        s.set_report(0, r).unwrap();
        let mut d = s.report_list()[4].clone();
        assert_eq!(d.kind, ReportKind::Dood);
        d.options.no_holds = true;
        d.options.landscape = Some(true); // the kind's default: not written
        s.set_report(4, d).unwrap();
        s.dood_codes.hold = "HD".into();
        s.boards[0].sub_boards[0].calendar.prep_start = "2027-01-04".parse().ok();
        s.boards[0].sub_boards[0].calendar.wrap_end = "2027-04-30".parse().ok();
        s.check().unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"production\": {\n      \"company\": \"ZTEST Pictures\""));
        assert!(text.contains("\"orientation\": \"landscape\""));
        assert_eq!(text.matches("\"orientation\"").count(), 1);
        assert!(text.contains("\"dood_codes\": {\n      \"hold\": \"HD\"\n    }"));
        assert!(text.contains("\"prep_start\": \"2027-01-04\""));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // Strangers are refused.
        let bad = text.replace("\"kind\": \"dood\"", "\"kind\": \"topsheet\"");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let bad = text.replace("\"hold\": \"HD\"", "\"hold\": \"W\"");
        assert!(matches!(from_str(&bad), Err(FormatError::Invalid(_))));
        let bad = text.replace(
            "\"wrap_end\": \"2027-04-30\"",
            "\"wrap_end\": \"2026-04-30\"",
        );
        assert!(matches!(from_str(&bad), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn report_designer_columns_round_trip_and_plain_ones_stay_names() {
        // Decision 90: a plain column is its field's name, as before; one with a line, a
        // merged cell, a look or a size is an object.
        let mut s = sample();
        let mut r = s.report_list()[0].clone();
        r.options.columns = vec![
            ReportColumn::of(StripField::Scene),
            ReportColumn {
                bold: true,
                size: Some(TextSize::Large),
                ..ReportColumn::of(StripField::Set)
            },
            ReportColumn::of(StripField::Pages),
            ReportColumn {
                line: 1,
                span: 2,
                italic: true,
                align: Align::Center,
                ..ReportColumn::of(StripField::Synopsis)
            },
        ];
        s.set_report(0, r).unwrap();
        s.check().unwrap();
        let text = to_string(&s);
        assert!(
            text.contains("\"columns\": [\n          \"scene\",\n          {"),
            "{text}"
        );
        assert!(text.contains("\"size\": \"large\""));
        assert!(text.contains("\"span\": 2"));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // An object that sets nothing but its field, a span of 1 and a later line too
        // wide are refused.
        let only = text.replacen("\"scene\",", "{\"field\": \"scene\"},", 1);
        assert!(from_str(&only)
            .unwrap_err()
            .to_string()
            .contains("write it as the name"));
        let one = text.replacen("\"span\": 2", "\"span\": 1", 1);
        assert!(from_str(&one)
            .unwrap_err()
            .to_string()
            .contains("is the default"));
        let wide = text.replacen("\"span\": 2", "\"span\": 4", 1);
        assert!(matches!(from_str(&wide), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn properties_images_quantities_links_and_event_kinds_round_trip() {
        // Decisions 62 and 63: every new key is left out at its default, so a file without them
        // reads and writes back byte for byte, and one with them round trips exactly.
        let plain = sample();
        let before = to_string(&plain);
        for key in [
            "daily_quantity",
            "\"properties\"",
            "multi_line",
            "\"images\"",
            "\"quantities\"",
            "\"daily\"",
            "camera_test",
            "unavailable",
        ] {
            assert!(!before.contains(key), "{key}");
        }
        assert_eq!(to_string(&from_str(&before).unwrap()), before);

        let mut s = plain;
        let cast = s.cast().unwrap();
        let crowd = s.add_category("Background");
        s.category_mut(crowd).unwrap().daily_quantity = true;
        s.add_property(cast, "Agent").unwrap();
        s.add_property(cast, "Height").unwrap();
        s.set_property_multi_line(cast, 1, true).unwrap();
        let tove = s.element_named(cast, "TOVE").unwrap();
        s.set_property_value(tove, "height", "1.85 m").unwrap();
        s.element_mut(tove).unwrap().images = vec!["images/tove.jpg".into()];
        let walkers = s.add_element(crowd, "Dock workers").unwrap();
        s.set_links(tove, &[walkers], false).unwrap();
        let sheet = s.sheets[0].id;
        s.tag_with_links(sheet, tove).unwrap();
        s.set_quantity(sheet, walkers, 12, false).unwrap();
        for kind in [EventKind::Unavailable, EventKind::CameraTest] {
            s.boards[0].sub_boards[0].calendar.events.push(Event {
                name: String::new(),
                kind,
                from: "2027-03-02".parse().unwrap(),
                to: "2027-03-02".parse().unwrap(),
                elements: vec![tove],
                color: None,
            });
        }
        s.check().unwrap();
        let text = to_string(&s);
        for key in [
            "\"daily_quantity\": true",
            "\"properties\": [\n          \"Agent\",\n          \"Height\"\n        ]",
            "\"multi_line\": [\n          \"Height\"\n        ]",
            "\"name\": \"Height\",\n            \"value\": \"1.85 m\"",
            "\"images\": [\n          \"images/tove.jpg\"",
            "\"count\": 12,\n            \"daily\": false",
            "\"kind\": \"unavailable\"",
            "\"kind\": \"camera_test\"",
        ] {
            assert!(text.contains(key), "{key}\n{text}");
        }
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);
        // Strict: a value for a property the category lacks, a quantity for an element
        // not on the sheet, a label on a kind that has none.
        let stray = text.replacen("\"name\": \"Height\",\n", "\"name\": \"Weight\",\n", 1);
        assert!(matches!(from_str(&stray), Err(FormatError::Invalid(_))));
        let zero = text.replacen("\"count\": 12", "\"count\": 0", 1);
        assert!(matches!(from_str(&zero), Err(FormatError::Invalid(_))));
        let label = text.replacen(
            "\"kind\": \"camera_test\",",
            "\"kind\": \"camera_test\",\n\"label\": \"x\",",
            1,
        );
        assert!(matches!(from_str(&label), Err(FormatError::BadValue(_))));
        // Post-production kinds and event colors (decision 109): additive. A file without
        // them was written above byte for byte; with them it round-trips too.
        assert!(!text.contains("\"color\""));
        let mut post = s.clone();
        post.boards[0].sub_boards[0].calendar.events.push(Event {
            name: "Grade".into(),
            kind: EventKind::Post(method_schedule_core::model::PostKind::PictureEditing),
            from: "2027-06-01".parse().unwrap(),
            to: "2027-06-10".parse().unwrap(),
            elements: vec![tove, walkers],
            color: Some(9),
        });
        post.check().unwrap();
        let post_text = to_string(&post);
        assert!(
            post_text.contains("\"kind\": \"picture_editing\""),
            "{post_text}"
        );
        assert!(post_text.contains("\"color\": \"brown\""), "{post_text}");
        assert_eq!(from_str(&post_text).unwrap(), post);
        assert_eq!(to_string(&from_str(&post_text).unwrap()), post_text);
        let mauve = post_text.replacen("\"color\": \"brown\"", "\"color\": \"mauve\"", 1);
        assert!(matches!(from_str(&mauve), Err(FormatError::BadValue(_))));
        let dance = post_text.replacen("\"picture_editing\"", "\"dance\"", 1);
        assert!(from_str(&dance).is_err());
    }

    #[test]
    fn a_new_schedule_has_our_default_categories() {
        let s = Schedule::with_default_categories("ZTEST");
        let names: Vec<&str> = s.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names[0], "Cast");
        assert_eq!(names.len(), 1 + DEFAULT_CATEGORIES.len());
        let text = to_string(&s);
        assert_eq!(from_str(&text).unwrap(), s);
        assert_eq!(to_string(&from_str(&text).unwrap()), text);
    }

    #[test]
    fn empty_schedule_is_small() {
        let text = to_string(&Schedule::new("ZTEST"));
        assert_eq!(from_str(&text).unwrap(), Schedule::new("ZTEST"));
        assert!(text.lines().count() <= 30, "{text}");
    }

    /// Decision 113: a built-in red flag type keeps its event kind key; files without the
    /// key read and write back byte for byte; an unknown or `other` key is refused.
    #[test]
    fn built_in_red_flag_types_round_trip() {
        let s = Schedule::with_default_categories("ZTEST Built-in types");
        let text = to_string(&s);
        assert!(text.contains("\"builtin\": \"travel\""));
        assert!(text.contains("\"builtin\": \"music_recording\""));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);

        let mut old = Schedule::new("ZTEST Older");
        old.add_red_flag_type("Second job").unwrap();
        let plain = to_string(&old);
        assert!(!plain.contains("builtin"));
        assert_eq!(to_string(&from_str(&plain).unwrap()), plain);

        for bad in ["other", "picnic"] {
            let t = text.replacen(
                "\"builtin\": \"travel\"",
                &format!("\"builtin\": \"{bad}\""),
                1,
            );
            assert!(from_str(&t).is_err(), "{bad}");
        }
    }

    #[test]
    fn sheet_numbers_images_field_order_flag_colors_and_templates_round_trip() {
        // Decision 77: each new key is left out at its default, so files without them
        // read and write back byte for byte.
        let mut s = sample();
        let plain = to_string(&s);
        for key in [
            "\"number\"",
            "\"sheet_fields\"",
            "\"color\"",
            "\"template\"",
        ] {
            assert!(!plain.contains(key), "{key}");
        }
        assert_eq!(to_string(&from_str(&plain).unwrap()), plain);
        assert!(!is_template(&plain));

        let unavailable = s.add_red_flag_type("Unavailable").unwrap();
        s.red_flag_types[0].color = Some(Rgb(0x3366cc));
        s.sheets[0].number = "12A".into();
        s.sheets[0].images = vec!["images/kitchen.jpg".into()];
        s.set_sheet_field_order(&["scenes".into(), "number".into()])
            .unwrap();
        let cast = s.cast().unwrap();
        let kit = s.add_element(cast, "ZTEST KIT").unwrap();
        s.set_red_flag(
            kit,
            method_schedule_core::model::RedFlag {
                id: RedFlagId(0),
                kind: unavailable,
                dates: RedFlagDates::NotAfter("2027-03-10".parse().unwrap()),
                note: String::new(),
                hard: true,
            },
        )
        .unwrap();
        s.check().unwrap();
        let text = to_string(&s);
        assert!(text.contains("\"color\": \"#3366cc\""));
        assert!(text.contains("\"number\": \"12A\""));
        assert!(text.contains("\"images\": [\n          \"images/kitchen.jpg\""));
        assert!(text.contains("\"sheet_fields\": [\n      \"scenes\",\n      \"number\","));
        let back = from_str(&text).unwrap();
        assert_eq!(back, s);
        assert_eq!(to_string(&back), text);

        // A template: no sheets, strips, events or red flags; marked as one.
        let t = to_template_string(&s);
        assert!(is_template(&t));
        assert!(t.contains("\"template\": true"));
        let back = from_str(&t).unwrap();
        assert!(back.sheets.is_empty());
        assert!(back.boards.iter().all(|b| b.boneyard.is_empty()
            && b.sub_boards
                .iter()
                .all(|sb| sb.strips.is_empty() && sb.calendar.events.is_empty())));
        assert!(back.elements.iter().all(|e| e.red_flags.is_empty()));
        assert_eq!(back.categories, s.categories);
        // The template gets the built-in types it lacked (decision 113): Unavailable,
        // already there, becomes the built-in one and keeps its id and color.
        assert_eq!(back.red_flag_types[0].id, s.red_flag_types[0].id);
        assert_eq!(back.red_flag_types[0].color, s.red_flag_types[0].color);
        assert_eq!(
            back.red_flag_types[0].builtin,
            Some(method_schedule_core::model::EventKind::Unavailable)
        );
        assert_eq!(back.missing_standard_red_flag_types(), 0);
        assert!(t.contains("\"builtin\": \"camera_test\""));
        assert_eq!(back.sheet_fields, s.sheet_fields);
        assert_eq!(to_template_string(&back), t);
        assert_eq!(to_template_string_as_is(&back).unwrap(), t);
        assert!(to_template_string_as_is(&s).is_err());

        // A template holding sheets, strips, events or red flags is refused, and so is
        // `"template": false`, the default written out.
        let marked = to_string(&s).replacen("{\n", "{\n  \"template\": true,\n", 1);
        let e = from_str(&marked).unwrap_err().to_string();
        assert!(e.contains("a template holds no sheets"), "{e}");
        let unmarked = plain.replacen("{\n", "{\n  \"template\": false,\n", 1);
        let e = from_str(&unmarked).unwrap_err().to_string();
        assert!(e.contains("is the default"), "{e}");

        // Strangers are refused.
        let bad = text.replace("\"#3366cc\"", "\"blue\"");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
        let bad = text.replace("      \"number\",\n", "      \"page_count\",\n");
        assert!(matches!(from_str(&bad), Err(FormatError::BadValue(_))));
    }

    /// Decision 89: a sheet's numbers past their bounds are refused on reading.
    #[test]
    fn a_sheet_past_its_bounds_is_refused() {
        let mut s = sample();
        s.sheets[0].pages = Eighths(999 * 8);
        s.sheets[0].shoot_time = Some(Minutes(24 * 60));
        let text = to_string(&s);
        assert_eq!(from_str(&text).unwrap(), s);
        let at = |n: u32| format!("\"eighths\": {n}");
        let bad = text.replacen(&at(999 * 8), &at(999 * 8 + 1), 1);
        assert!(matches!(from_str(&bad), Err(FormatError::Invalid(_))));
        let at = |n: u32| format!("\"shoot_minutes\": {n}");
        let bad = text.replacen(&at(24 * 60), &at(24 * 60 + 1), 1);
        assert!(matches!(from_str(&bad), Err(FormatError::Invalid(_))));
    }
}
