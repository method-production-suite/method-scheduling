//! Red flags in the app (decisions.md, entries 33 and 36): the red flag types (add, rename,
//! reorder, delete), each element's flags (add, change, delete), and the clashes
//! `method_schedule_core::redflags::red_flags` finds, shaped for the page: marks on the
//! board's strips, the calendar's dates and the day out of days cells, and the Red flags
//! tab's list.
//!
//! Every edit is one of the engine's functions, run through `Session::edit` as one undo
//! step, with what the page sent read strictly (dates as `YYYY-MM-DD`) and the engine's
//! refusals said as plain sentences.
//!
//! Cost: the board's marks go with every view, so they are worked out only when some
//! element has a red flag, and then one `red_flags` call per sub-board of the board shown.
//! The tab's full list, with names, scenes and strips, is built only when the tab asks.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use method_schedule_core::model::{
    Board, ElementId, Error, RedFlag, RedFlagDates, RedFlagId, RedFlagTypeId, StripKind, SubBoard,
    MAX_RED_FLAGS_AT_ONCE,
};
use method_schedule_core::redflags::{red_flags, Clash};
use method_schedule_core::{Date, Schedule};

use crate::board::EditResult;
use crate::conflicts::SheetRefDto;
use crate::sheet::{MAX_NAME, MAX_TEXT};
use crate::view::{sub_ref, weekday};

/// Most clashes the Red flags tab lists; the count says how many in all.
pub const MAX_LISTED: usize = 1000;

/// The date forms a flag takes, as the page sends them.
pub const DATE_FORMS: &[&str] = &["range", "not_before", "not_after"];

// ---------------------------------------------------------------------------
// What the page sends

/// A red flag as the page sends it: `id` null (or 0) for a new flag; `dates` is `range`
/// (with `from` and `to`), `not_before` or `not_after` (with `from` only); dates are
/// `YYYY-MM-DD`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedFlagInput {
    #[serde(default)]
    pub id: Option<u32>,
    pub red_flag_type: u32,
    pub dates: String,
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub hard: bool,
}

/// The engine's red flag refusals, as the page says them.
pub fn say(e: Error) -> String {
    match e {
        Error::NoSuchRedFlagType(_) => "That red flag type is no longer in the schedule.".into(),
        Error::NoSuchRedFlag(_) => "That red flag is no longer in the schedule.".into(),
        Error::DuplicateName(n) => format!("There is already a red flag type called {n}."),
        Error::InUse(n) => format!(
            "{n} is used by red flags, so it can't be deleted. Change or delete those flags first."
        ),
        Error::EmptyName => "Type a name for the red flag type.".into(),
        Error::BuiltInType(n) => format!(
            "{n} is a standard type, so it can't be deleted. It can be renamed and recolored."
        ),
        other => crate::elements::say(other),
    }
}

/// A date as typed in the app's date fields (`crate::calendar::typed_date`).
fn date(what: &str, value: &str) -> EditResult<Date> {
    let t = value.trim();
    if t.is_empty() {
        return Err(format!("Type the {what} date, as 3/1/2027 or 2027-03-01."));
    }
    crate::calendar::parse_date(&format!("The {what} date"), t)
}

/// The dates a flag rules out, read strictly.
pub fn parse_dates(form: &str, from: &str, to: &str) -> EditResult<RedFlagDates> {
    match form {
        "range" => {
            let a = date("first", from)?;
            let b = date("last", to)?;
            if b < a {
                return Err(
                    "The last date is before the first. A red flag can't end before it starts."
                        .into(),
                );
            }
            Ok(RedFlagDates::Range { from: a, to: b })
        }
        "not_before" => Ok(RedFlagDates::NotBefore(date("not before", from)?)),
        "not_after" => Ok(RedFlagDates::NotAfter(date("not after", from)?)),
        _ => Err("A red flag's dates are a range, not before a date, or not after one.".into()),
    }
}

fn type_id(s: &Schedule, id: u32) -> EditResult<RedFlagTypeId> {
    s.red_flag_type(RedFlagTypeId(id))
        .map(|t| t.id)
        .ok_or_else(|| say(Error::NoSuchRedFlagType(RedFlagTypeId(id))))
}

fn type_name(value: &str) -> EditResult<String> {
    let t = value.trim();
    if t.is_empty() {
        return Err(say(Error::EmptyName));
    }
    if t.chars().count() > MAX_NAME {
        return Err(format!("A name is at most {MAX_NAME} characters."));
    }
    if t.chars().any(char::is_control) {
        return Err("A red flag type's name is one line.".into());
    }
    Ok(t.to_string())
}

// ---------------------------------------------------------------------------
// The edits (Session::edit makes each one undo step)

/// Adds a red flag to `element`, or changes flag `input.id` (which must be on it).
/// Returns the flag's id.
pub fn set_red_flag(s: &mut Schedule, element: u32, input: &RedFlagInput) -> EditResult<u32> {
    let el = s
        .element(ElementId(element))
        .map(|e| e.id)
        .ok_or_else(|| crate::elements::say(Error::NoSuchElement(ElementId(element))))?;
    let kind = type_id(s, input.red_flag_type)?;
    let dates = parse_dates(&input.dates, &input.from, &input.to)?;
    let note = input.note.trim();
    if note.chars().count() > MAX_TEXT {
        return Err(format!("A note is at most {MAX_TEXT} characters."));
    }
    let id = RedFlagId(input.id.unwrap_or(0));
    if id.0 != 0 {
        match s.red_flag(id) {
            None => return Err(say(Error::NoSuchRedFlag(id))),
            Some((on, _)) if on != el => {
                return Err("That red flag belongs to another element.".into())
            }
            Some(_) => {}
        }
    }
    s.set_red_flag(
        el,
        RedFlag {
            id,
            kind,
            dates,
            note: note.to_string(),
            hard: input.hard,
        },
    )
    .map(|id| id.0)
    .map_err(say)
}

/// Deletes red flag `flag`.
pub fn delete_red_flag(s: &mut Schedule, flag: u32) -> EditResult {
    s.delete_red_flag(RedFlagId(flag)).map_err(say)
}

/// Adds a red flag type at the end of the list; a name already used (ignoring case) is
/// refused. Returns its id.
pub fn add_red_flag_type(s: &mut Schedule, name: &str) -> EditResult<u32> {
    let name = type_name(name)?;
    if s.red_flag_type_named(&name).is_some() {
        return Err(say(Error::DuplicateName(name)));
    }
    s.add_red_flag_type(&name).map(|id| id.0).map_err(say)
}

pub fn rename_red_flag_type(s: &mut Schedule, kind: u32, name: &str) -> EditResult {
    let id = type_id(s, kind)?;
    let name = type_name(name)?;
    s.rename_red_flag_type(id, &name).map_err(say)
}

/// Moves a red flag type to place `to` in the list (from 0).
pub fn move_red_flag_type(s: &mut Schedule, kind: u32, to: usize) -> EditResult {
    let id = type_id(s, kind)?;
    s.move_red_flag_type(id, to).map_err(say)
}

/// Deletes a red flag type no flag uses; one in use, or a standard one, is refused with
/// a sentence.
pub fn delete_red_flag_type(s: &mut Schedule, kind: u32) -> EditResult {
    let id = type_id(s, kind)?;
    s.delete_red_flag_type(id).map_err(say)
}

/// Red Flag Types > Add the Standard Types (decision 113): the built-in types an older
/// schedule lacks, in one step. Returns how many were added or marked.
pub fn add_standard_types(s: &mut Schedule) -> EditResult<usize> {
    if s.missing_standard_red_flag_types() == 0 {
        return Err("The schedule already has every standard type.".into());
    }
    Ok(s.standard_red_flag_types())
}

/// Most elements and dates one Add on the Red flags tab takes.
pub const MAX_PICKED: usize = 1000;

/// Flags for several elements and dates at once, as the Red flags tab sends them
/// (behavior note 23): `dates` is `picked` (each of `picked`, runs of following dates
/// joined into one range), `range` (`from` to `to`), `not_before` or `not_after` (`from`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddFlagsInput {
    pub elements: Vec<u32>,
    pub red_flag_type: u32,
    pub dates: String,
    #[serde(default)]
    pub picked: Vec<String>,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub hard: bool,
}

/// Adds one flag per element and date (run of dates), one undo step. Returns how many.
pub fn add_red_flags(s: &mut Schedule, input: &AddFlagsInput) -> EditResult<usize> {
    if input.elements.is_empty() {
        return Err("Check at least one element to flag.".into());
    }
    if input.elements.len() > MAX_PICKED || input.picked.len() > MAX_PICKED {
        return Err(format!(
            "At most {MAX_PICKED} elements and {MAX_PICKED} dates at once."
        ));
    }
    let mut elements = Vec::with_capacity(input.elements.len());
    for &e in &input.elements {
        let id = s
            .element(ElementId(e))
            .map(|x| x.id)
            .ok_or_else(|| crate::elements::say(Error::NoSuchElement(ElementId(e))))?;
        if !elements.contains(&id) {
            elements.push(id);
        }
    }
    let kind = type_id(s, input.red_flag_type)?;
    let dates = if input.dates == "picked" {
        let mut picked = Vec::with_capacity(input.picked.len());
        for d in &input.picked {
            picked.push(date("picked", d)?);
        }
        if picked.is_empty() {
            return Err("Pick at least one date on the calendar.".into());
        }
        method_schedule_core::redflags::runs(&picked)
    } else {
        let to = if input.dates == "range" && input.to.trim().is_empty() {
            &input.from
        } else {
            &input.to
        };
        vec![parse_dates(&input.dates, &input.from, to)?]
    };
    let note = input.note.trim();
    if note.chars().count() > MAX_TEXT {
        return Err(format!("A note is at most {MAX_TEXT} characters."));
    }
    if elements.len().saturating_mul(dates.len()) > MAX_RED_FLAGS_AT_ONCE {
        return Err(format!(
            "That would add {} red flags. Add at most {MAX_RED_FLAGS_AT_ONCE} at once: check fewer elements or dates.",
            elements.len() * dates.len()
        ));
    }
    s.add_red_flags(&elements, kind, &dates, note, input.hard)
        .map(|ids| ids.len())
        .map_err(say)
}

/// Deletes the checked flags on the Red flags tab, one undo step.
pub fn delete_red_flags(s: &mut Schedule, flags: &[u32]) -> EditResult {
    if flags.is_empty() {
        return Err("Check the red flags to delete first.".into());
    }
    let ids: Vec<RedFlagId> = flags.iter().map(|&f| RedFlagId(f)).collect();
    s.delete_red_flags(&ids).map_err(say)
}

// ---------------------------------------------------------------------------
// What the page draws

/// A red flag type, with how many flags use it.
#[derive(Clone, Debug, Serialize)]
pub struct RedFlagTypeDto {
    pub id: u32,
    pub name: String,
    pub uses: usize,
    /// `#rrggbb`, the color its marks are drawn in (decisions.md, entry 84); none: red.
    pub color: Option<String>,
    /// A standard type's event kind key (`travel`), from the event kinds' word list
    /// (decision 113); none for the schedule's own types.
    pub builtin: Option<&'static str>,
    /// For a standard type whose event kind counts: the day out of days code an event of
    /// that kind shows (`TR`). The flag itself never counts.
    pub event_code: Option<&'static str>,
}

/// One flag on an element, for the Elements tab's editor.
#[derive(Clone, Debug, Serialize)]
pub struct RedFlagDto {
    pub id: u32,
    pub red_flag_type: u32,
    pub type_name: String,
    /// `range`, `not_before` or `not_after`.
    pub dates: &'static str,
    /// `YYYY-MM-DD`; `to` is empty unless `dates` is `range`.
    pub from: String,
    pub to: String,
    /// The dates in words: `2027-03-02 to 2027-03-04`, `on 2027-03-02`, `not before
    /// 2027-03-10`.
    pub text: String,
    pub note: String,
    pub hard: bool,
}

/// Every red flag type in order, with its use count (the view carries it).
pub fn types(s: &Schedule) -> Vec<RedFlagTypeDto> {
    let mut uses: HashMap<RedFlagTypeId, usize> = HashMap::new();
    for f in s.elements.iter().flat_map(|e| &e.red_flags) {
        *uses.entry(f.kind).or_default() += 1;
    }
    s.red_flag_types
        .iter()
        .map(|t| RedFlagTypeDto {
            id: t.id.0,
            name: t.name.clone(),
            uses: uses.get(&t.id).copied().unwrap_or(0),
            color: t.color.map(|c| c.hex()),
            builtin: t.builtin.as_ref().map(|k| k.key()),
            event_code: t.builtin.as_ref().and_then(|k| k.code()),
        })
        .collect()
}

fn type_label(s: &Schedule, kind: RedFlagTypeId) -> String {
    s.red_flag_type(kind)
        .map(|t| t.name.clone())
        .unwrap_or_default()
}

/// An element's flags, in the order added.
pub fn flags_of(s: &Schedule, element: ElementId) -> Vec<RedFlagDto> {
    let Some(e) = s.element(element) else {
        return Vec::new();
    };
    e.red_flags
        .iter()
        .map(|f| {
            let (dates, from, to) = match f.dates {
                RedFlagDates::Range { from, to } => ("range", from.to_string(), to.to_string()),
                RedFlagDates::NotBefore(d) => ("not_before", d.to_string(), String::new()),
                RedFlagDates::NotAfter(d) => ("not_after", d.to_string(), String::new()),
            };
            RedFlagDto {
                id: f.id.0,
                red_flag_type: f.kind.0,
                type_name: type_label(s, f.kind),
                dates,
                from,
                to,
                text: f.dates.text(),
                note: f.note.clone(),
                hard: f.hard,
            }
        })
        .collect()
}

/// True when some element has a red flag: without one there is nothing to work out.
pub fn any(s: &Schedule) -> bool {
    s.elements.iter().any(|e| !e.red_flags.is_empty())
}

/// One clash in words: `ADA, Unavailable, 2027-03-02 to 2027-03-03, hard`.
pub fn clash_text(s: &Schedule, c: &Clash) -> String {
    let name = s.element(c.element).map_or("", |e| e.name.as_str());
    let dates = s
        .red_flag(c.flag)
        .map(|(_, f)| f.dates.text())
        .unwrap_or_default();
    let firm = if c.hard { "hard" } else { "soft" };
    format!("{name}, {}, {dates}, {firm}", type_label(s, c.kind))
}

/// The red flag marks on board `b`: each strip that clashes, with what it clashes with,
/// and how many clashes each sub-board has.
#[derive(Debug, Default)]
pub struct BoardMarks {
    pub strips: HashMap<u32, Vec<String>>,
    pub counts: HashMap<u32, usize>,
    /// The red flag types clashing on each strip, in the order met (their colors paint
    /// the mark; decisions.md, entry 84).
    pub kinds: HashMap<u32, Vec<u32>>,
}

/// The marks for the board view. Nothing is worked out when no element has a flag.
pub fn board_marks(s: &Schedule, b: &Board) -> BoardMarks {
    let mut out = BoardMarks::default();
    if !any(s) {
        return out;
    }
    for sb in &b.sub_boards {
        let Ok(found) = red_flags(s, b.id, sb.id) else {
            continue;
        };
        out.counts.insert(sb.id.0, found.clashes.len());
        if found.clashes.is_empty() {
            continue;
        }
        let strip_of = strips_by_sheet(sb);
        // One flag clashes on many dates: say it once.
        let mut said: HashMap<RedFlagId, String> = HashMap::new();
        for c in &found.clashes {
            if c.sheets.is_empty() {
                continue;
            }
            let text = said
                .entry(c.flag)
                .or_insert_with(|| clash_text(s, c))
                .clone();
            for sheet in &c.sheets {
                if let Some(&strip) = strip_of.get(&sheet.0) {
                    let list = out.strips.entry(strip).or_default();
                    if !list.contains(&text) {
                        list.push(text.clone());
                    }
                    let kinds = out.kinds.entry(strip).or_default();
                    if !kinds.contains(&c.kind.0) {
                        kinds.push(c.kind.0);
                    }
                }
            }
        }
    }
    out
}

fn strips_by_sheet(sb: &SubBoard) -> HashMap<u32, u32> {
    sb.strips
        .iter()
        .filter_map(|st| match st.kind {
            StripKind::Sheet(sheet) => Some((sheet.0, st.id.0)),
            _ => None,
        })
        .collect()
}

/// The clashes on sub-board `sb` of `b` by date, in words (the Calendar tab's marks).
pub fn by_date(s: &Schedule, b: &Board, sb: &SubBoard) -> HashMap<Date, Vec<String>> {
    let mut out: HashMap<Date, Vec<String>> = HashMap::new();
    if !any(s) {
        return out;
    }
    if let Ok(found) = red_flags(s, b.id, sb.id) {
        for c in &found.clashes {
            out.entry(c.date).or_default().push(clash_text(s, c));
        }
    }
    out
}

/// The red flag types clashing on each date of sub-board `sb` (the Calendar's marks take
/// the first one's color).
pub fn kinds_by_date(s: &Schedule, b: &Board, sb: &SubBoard) -> HashMap<Date, Vec<u32>> {
    let mut out: HashMap<Date, Vec<u32>> = HashMap::new();
    if !any(s) {
        return out;
    }
    if let Ok(found) = red_flags(s, b.id, sb.id) {
        for c in &found.clashes {
            let v = out.entry(c.date).or_default();
            if !v.contains(&c.kind.0) {
                v.push(c.kind.0);
            }
        }
    }
    out
}

/// The clashes of each element on sub-board `sb`: how many, for the Elements table's
/// mark (decisions.md, entry 84).
pub fn count_by_element(s: &Schedule, b: &Board, sb: &SubBoard) -> HashMap<ElementId, usize> {
    let mut out = HashMap::new();
    if !any(s) {
        return out;
    }
    if let Ok(found) = red_flags(s, b.id, sb.id) {
        for c in &found.clashes {
            *out.entry(c.element).or_default() += 1;
        }
    }
    out
}

/// The clashes on a sheet's shoot date on sub-board `sb`, in words, with their types: the
/// Breakdown tab's mark (decisions.md, entry 84).
pub fn on_sheet(
    s: &Schedule,
    b: &Board,
    sb: &SubBoard,
    sheet: method_schedule_core::model::SheetId,
) -> (Vec<String>, Vec<u32>) {
    let (mut texts, mut kinds) = (Vec::new(), Vec::new());
    if !any(s) {
        return (texts, kinds);
    }
    if let Ok(found) = red_flags(s, b.id, sb.id) {
        for c in found.clashes.iter().filter(|c| c.sheets.contains(&sheet)) {
            let t = clash_text(s, c);
            if !texts.contains(&t) {
                texts.push(t);
            }
            if !kinds.contains(&c.kind.0) {
                kinds.push(c.kind.0);
            }
        }
    }
    (texts, kinds)
}

/// The clashes on sub-board `sb` of `b` by element and date, in words (the day out of
/// days cells).
pub fn by_element_date(
    s: &Schedule,
    b: &Board,
    sb: &SubBoard,
) -> HashMap<(ElementId, Date), Vec<String>> {
    let mut out: HashMap<(ElementId, Date), Vec<String>> = HashMap::new();
    if !any(s) {
        return out;
    }
    if let Ok(found) = red_flags(s, b.id, sb.id) {
        for c in &found.clashes {
            out.entry((c.element, c.date))
                .or_default()
                .push(clash_text(s, c));
        }
    }
    out
}

/// The Red flags tab: every clash on one sub-board, in date order.
#[derive(Clone, Debug, Serialize)]
pub struct RedFlagsDto {
    pub board: u32,
    pub board_name: String,
    pub sub_board: u32,
    pub sub_board_name: String,
    /// The sub-board's start date, where the small calendar opens (live note 26).
    pub start: Option<String>,
    /// Every clash on the sub-board.
    pub count: usize,
    pub clashes: Vec<ClashDto>,
    /// The sub-board's strips have no dates (no start date), so only events were checked.
    pub undated: bool,
    /// How many red flags there are in the schedule (none: the tab says how to add one).
    pub flags: usize,
    pub truncated: bool,
    /// Every red flag in the schedule, for the tab's table (behavior note 23), by start
    /// date (open starts first), then element; at most [`MAX_LISTED`].
    pub rows: Vec<FlagRowDto>,
    /// The categories and their elements, for the tab's element picker.
    pub categories: Vec<PickCategoryDto>,
}

/// One red flag in the Red flags tab's table: the dates it rules out.
#[derive(Clone, Debug, Serialize)]
pub struct FlagRowDto {
    pub id: u32,
    /// `YYYY-MM-DD`; none for an open start (not before a date).
    pub start: Option<String>,
    /// `YYYY-MM-DD`; none for an open end (not after a date).
    pub end: Option<String>,
    pub element: u32,
    pub name: String,
    pub board_id: String,
    pub category: u32,
    pub category_name: String,
    pub red_flag_type: u32,
    pub type_name: String,
    pub note: String,
    pub hard: bool,
    /// The dates in words: `2027-03-02 to 2027-03-04`, `not before 2027-03-10`.
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PickCategoryDto {
    pub id: u32,
    pub name: String,
    pub elements: Vec<PickElementDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PickElementDto {
    pub id: u32,
    pub name: String,
    pub board_id: String,
}

/// Every red flag as a table row (dates as they rule out: an open start or end is none),
/// by start date, open starts first, then the element's place, then the flag's.
pub fn flag_rows(s: &Schedule) -> Vec<FlagRowDto> {
    let mut rows: Vec<(Option<Date>, usize, usize, FlagRowDto)> = Vec::new();
    for (place, e) in s.elements.iter().enumerate() {
        let (category, category_name) = s
            .category(e.category)
            .map_or((0, String::new()), |c| (c.id.0, c.name.clone()));
        for (k, f) in e.red_flags.iter().enumerate() {
            let (start, end) = match f.dates {
                RedFlagDates::Range { from, to } => (Some(from), Some(to)),
                RedFlagDates::NotBefore(d) => (None, Some(d.add_days(-1))),
                RedFlagDates::NotAfter(d) => (Some(d.add_days(1)), None),
            };
            rows.push((
                start,
                place,
                k,
                FlagRowDto {
                    id: f.id.0,
                    start: start.map(|d| d.to_string()),
                    end: end.map(|d| d.to_string()),
                    element: e.id.0,
                    name: e.name.clone(),
                    board_id: e.board_id.clone().unwrap_or_default(),
                    category,
                    category_name: category_name.clone(),
                    red_flag_type: f.kind.0,
                    type_name: type_label(s, f.kind),
                    note: f.note.clone(),
                    hard: f.hard,
                    text: f.dates.text(),
                },
            ));
        }
    }
    rows.sort_by_key(|r| (r.0, r.1, r.2));
    rows.into_iter().take(MAX_LISTED).map(|r| r.3).collect()
}

/// The categories in order with their elements in order, for the element picker.
pub fn pick_categories(s: &Schedule) -> Vec<PickCategoryDto> {
    s.categories
        .iter()
        .map(|c| PickCategoryDto {
            id: c.id.0,
            name: c.name.clone(),
            elements: s
                .elements
                .iter()
                .filter(|e| e.category == c.id)
                .map(|e| PickElementDto {
                    id: e.id.0,
                    name: e.name.clone(),
                    board_id: e.board_id.clone().unwrap_or_default(),
                })
                .collect(),
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct ClashDto {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub weekday: &'static str,
    pub element: u32,
    pub name: String,
    pub category: u32,
    pub category_name: String,
    pub flag: u32,
    /// The flag's type, whose color the row's mark takes.
    pub red_flag_type: u32,
    pub type_name: String,
    /// The flag's dates in words.
    pub dates: String,
    pub note: String,
    pub hard: bool,
    /// The shoot day, when the element works that date.
    pub day: Option<u32>,
    pub sheets: Vec<SheetRefDto>,
    /// The counted events that date: their place in the sub-board's calendar and name.
    pub events: Vec<EventRefDto>,
    /// The same clash in words, for titles.
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRefDto {
    pub index: usize,
    pub name: String,
}

/// The Red flags tab for sub-board `sub`.
pub fn red_flags_view(s: &Schedule, sub: u32) -> Result<RedFlagsDto, String> {
    let (b, sb) = sub_ref(s, sub)?;
    let found = red_flags(s, b.id, sb.id).map_err(say)?;
    let strip_of = strips_by_sheet(sb);
    let clashes = found
        .clashes
        .iter()
        .take(MAX_LISTED)
        .map(|c| {
            let e = s.element(c.element);
            let (category, category_name) = e
                .and_then(|e| s.category(e.category))
                .map_or((0, String::new()), |c| (c.id.0, c.name.clone()));
            let flag = s.red_flag(c.flag).map(|(_, f)| f);
            ClashDto {
                date: c.date.to_string(),
                weekday: weekday(c.date),
                element: c.element.0,
                name: e.map(|e| e.name.clone()).unwrap_or_default(),
                category,
                category_name,
                flag: c.flag.0,
                red_flag_type: c.kind.0,
                type_name: type_label(s, c.kind),
                dates: flag.map(|f| f.dates.text()).unwrap_or_default(),
                note: flag.map(|f| f.note.clone()).unwrap_or_default(),
                hard: c.hard,
                day: c.day,
                sheets: c
                    .sheets
                    .iter()
                    .map(|&sheet| {
                        let scenes = s.sheet(sheet).map_or("", |h| h.scenes.trim());
                        SheetRefDto {
                            sheet: sheet.0,
                            scene: if scenes.is_empty() {
                                "A sheet with no scene number".to_string()
                            } else {
                                format!("Scene {scenes}")
                            },
                            strip: strip_of.get(&sheet.0).copied(),
                        }
                    })
                    .collect(),
                events: c
                    .events
                    .iter()
                    .map(|&index| EventRefDto {
                        index,
                        name: sb
                            .calendar
                            .events
                            .get(index)
                            .map(|ev| ev.title())
                            .unwrap_or_default(),
                    })
                    .collect(),
                text: clash_text(s, c),
            }
        })
        .collect();
    Ok(RedFlagsDto {
        board: b.id.0,
        board_name: b.name.clone(),
        sub_board: sb.id.0,
        sub_board_name: sb.name.clone(),
        start: sb.start.map(|d| d.to_string()),
        count: found.clashes.len(),
        clashes,
        undated: found.undated,
        flags: s.elements.iter().map(|e| e.red_flags.len()).sum(),
        truncated: found.clashes.len() > MAX_LISTED,
        rows: flag_rows(s),
        categories: pick_categories(s),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::{Event, EventKind, Place, Sheet, StripId, SubBoardId};

    fn d(x: &str) -> Date {
        x.parse().unwrap()
    }

    /// Main from Mon 2027-03-01, one sheet a day: day 1 scene 1 (ADA), day 2 scene 2 (ADA,
    /// BEN), day 3 scene 3 (BEN). Returns the session, Main, ADA and BEN.
    fn session() -> (Session, SubBoardId, u32, u32) {
        let mut s = Schedule::new("ZTEST Red flags app");
        let cast = s.category_named("Cast").unwrap();
        let ada = s.add_element(cast, "ADA").unwrap();
        let ben = s.add_element(cast, "BEN").unwrap();
        let b = s.boards[0].id;
        let main = s.boards[0].sub_boards[0].id;
        for (i, elements) in [vec![ada], vec![ada, ben], vec![ben]]
            .into_iter()
            .enumerate()
        {
            let sheet = s.add_sheet(Sheet {
                scenes: (i + 1).to_string(),
                elements,
                ..Sheet::default()
            });
            let strip: StripId = s.boards[0]
                .boneyard
                .iter()
                .find(|x| x.kind == StripKind::Sheet(sheet))
                .unwrap()
                .id;
            let end = s.boards[0].sub_boards[0].strips.len();
            let at = |index| Place {
                sub_board: Some(main),
                index,
            };
            s.move_strips(b, &[strip], at(end)).unwrap();
            s.insert_strip(b, StripKind::DayBreak, at(end + 1)).unwrap();
        }
        s.boards[0].sub_boards[0].start = Some(d("2027-03-01"));
        s.check().unwrap();
        let mut session = Session::new();
        session
            .edit(|x| {
                *x = s.clone();
                Ok(())
            })
            .unwrap();
        (session, main, ada.0, ben.0)
    }

    fn input(kind: u32, dates: &str, from: &str, to: &str, hard: bool) -> RedFlagInput {
        RedFlagInput {
            id: None,
            red_flag_type: kind,
            dates: dates.into(),
            from: from.into(),
            to: to.into(),
            note: String::new(),
            hard,
        }
    }

    #[test]
    fn dates_are_read_strictly() {
        // Typed month first with slashes too (decision 49).
        assert_eq!(
            parse_dates("not_before", "3/10/2027", ""),
            Ok(RedFlagDates::NotBefore(d("2027-03-10")))
        );
        assert_eq!(
            parse_dates("range", "2027-03-02", " 2027-03-04 "),
            Ok(RedFlagDates::Range {
                from: d("2027-03-02"),
                to: d("2027-03-04")
            })
        );
        assert_eq!(
            parse_dates("not_before", "2027-03-10", ""),
            Ok(RedFlagDates::NotBefore(d("2027-03-10")))
        );
        assert_eq!(
            parse_dates("not_after", "2027-03-10", "ignored"),
            Ok(RedFlagDates::NotAfter(d("2027-03-10")))
        );
        for (form, from, to) in [
            ("range", "2027-03-04", "2027-03-02"),
            ("range", "2027-03-02", ""),
            ("range", "2027-3-2", "2027-03-04"),
            ("range", "2027-02-30", "2027-03-04"),
            ("not_before", "10/03/27", ""),
            ("not_after", "", ""),
            ("between", "2027-03-02", "2027-03-04"),
        ] {
            let e = parse_dates(form, from, to).unwrap_err();
            assert!(
                e.ends_with('.') && !e.contains('!') && !e.contains('\u{2014}'),
                "{e}"
            );
        }
    }

    #[test]
    fn types_and_flags_are_one_undo_step_each() {
        let (mut s, _, ada, ben) = session();
        let away = s.edit(|x| add_red_flag_type(x, " Unavailable ")).unwrap();
        let job = s.edit(|x| add_red_flag_type(x, "Second job")).unwrap();
        let err = s.edit(|x| add_red_flag_type(x, "unavailable")).unwrap_err();
        assert_eq!(err, "There is already a red flag type called unavailable.");
        assert!(s.edit(|x| add_red_flag_type(x, "  ")).is_err());
        s.edit(|x| rename_red_flag_type(x, job, "Other job"))
            .unwrap();
        assert!(s
            .edit(|x| rename_red_flag_type(x, job, "UNAVAILABLE"))
            .is_err());
        s.edit(|x| move_red_flag_type(x, job, 0)).unwrap();
        let names: Vec<&str> = s
            .schedule()
            .red_flag_types
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names, ["Other job", "Unavailable"]);

        let flag = s
            .edit(|x| {
                set_red_flag(
                    x,
                    ada,
                    &input(away, "range", "2027-03-02", "2027-03-03", true),
                )
            })
            .unwrap();
        assert_eq!(types(s.schedule())[1].uses, 1);
        // Change it in place: note and soft.
        let mut change = input(away, "not_after", "2027-03-01", "", false);
        change.id = Some(flag);
        change.note = " Wedding ".into();
        s.edit(|x| set_red_flag(x, ada, &change)).unwrap();
        let shown = flags_of(s.schedule(), ElementId(ada));
        assert_eq!(shown.len(), 1);
        assert_eq!(
            (
                shown[0].dates,
                shown[0].from.as_str(),
                shown[0].note.as_str(),
                shown[0].hard
            ),
            ("not_after", "2027-03-01", "Wedding", false)
        );
        assert_eq!(shown[0].text, "not after 2027-03-01");
        // Another element can't take it; a missing type or flag is refused.
        assert_eq!(
            s.edit(|x| set_red_flag(x, ben, &change)).unwrap_err(),
            "That red flag belongs to another element."
        );
        assert!(s
            .edit(|x| set_red_flag(
                x,
                ada,
                &input(999_999, "not_after", "2027-03-01", "", false)
            ))
            .is_err());
        // A type in use stays, with a sentence.
        let err = s.edit(|x| delete_red_flag_type(x, away)).unwrap_err();
        assert_eq!(
            err,
            "Unavailable is used by red flags, so it can't be deleted. Change or delete those flags first."
        );
        let before = s.schedule().clone();
        s.edit(|x| delete_red_flag(x, flag)).unwrap();
        assert!(s.schedule().red_flag(RedFlagId(flag)).is_none());
        s.undo();
        assert_eq!(s.schedule(), &before);
        s.edit(|x| delete_red_flag(x, flag)).unwrap();
        s.edit(|x| delete_red_flag_type(x, away)).unwrap();
        assert!(s.edit(|x| delete_red_flag(x, flag)).is_err());
        // Each change was one step: undo walks back through them.
        let steps = (0..20).take_while(|_| s.undo()).count();
        assert_eq!(steps, 9);
    }

    #[test]
    fn clashes_mark_strips_dates_cells_and_the_tab() {
        let (mut s, main, ada, ben) = session();
        let away = s.edit(|x| add_red_flag_type(x, "Unavailable")).unwrap();
        // Nothing to mark yet.
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].red_flags, 0);
        assert!(v.board.sub_boards[0]
            .days
            .iter()
            .flat_map(|d| &d.strips)
            .all(|st| st.red_flags.is_empty()));
        // ADA away Tue Mar 2 (hard): day 2, scene 2. BEN not before Mar 3 (soft): day 2 too.
        s.edit(|x| {
            set_red_flag(
                x,
                ada,
                &input(away, "range", "2027-03-02", "2027-03-02", true),
            )
        })
        .unwrap();
        s.edit(|x| set_red_flag(x, ben, &input(away, "not_before", "2027-03-03", "", false)))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let sb = &v.board.sub_boards[0];
        assert_eq!(sb.red_flags, 2);
        let day2 = &sb.days[1].strips[0];
        assert_eq!(day2.scene, "2");
        assert_eq!(
            day2.red_flags,
            [
                "ADA, Unavailable, on 2027-03-02, hard",
                "BEN, Unavailable, not before 2027-03-03, soft"
            ]
        );
        assert!(sb.days[0].strips[0].red_flags.is_empty());
        assert!(sb.days[2].strips[0].red_flags.is_empty());
        // BRD-14 (decision 136): the strip designer's switch hides the marks on strips,
        // one undo step; the tab's count stays.
        let b = v.board.id;
        let cols: Vec<crate::board::LayoutColumnInput> = serde_json::from_str(
            r#"[{"field":"scene","width":"narrow"},{"field":"cast","width":"wide"}]"#,
        )
        .unwrap();
        let hide = crate::board::MarksInput {
            hide_red_flags: true,
            hide_conflicts: true,
        };
        s.edit(|x| crate::board::set_strip_layout_with(x, b, &cols, "", hide))
            .unwrap();
        let hidden = crate::view::view(&s, None).unwrap();
        assert!(hidden.board.layout_marks.hide_red_flags);
        let sbh = &hidden.board.sub_boards[0];
        assert_eq!(sbh.red_flags, 2);
        assert!(sbh
            .days
            .iter()
            .flat_map(|d| &d.strips)
            .all(|st| st.red_flags.is_empty()
                && st.red_flag_types.is_empty()
                && st.conflicts.is_empty()));
        s.undo();
        let back = crate::view::view(&s, None).unwrap();
        assert_eq!(
            back.board.sub_boards[0].days[1].strips[0].red_flags.len(),
            2
        );

        // The calendar marks Mar 2 only.
        let cal = crate::view::calendar_view(s.schedule(), main.0, Some(2027), Some(3)).unwrap();
        let marked: Vec<&str> = cal
            .weeks
            .iter()
            .flatten()
            .filter(|x| !x.red_flags.is_empty())
            .map(|x| x.date.as_str())
            .collect();
        assert_eq!(marked, ["2027-03-02"]);

        // The day out of days marks ADA's and BEN's Mar 2 cells.
        let grid = crate::view::dood_grid(s.schedule(), main.0, None).unwrap();
        let at = grid
            .dates
            .iter()
            .position(|x| x.date == "2027-03-02")
            .unwrap();
        for r in &grid.rows {
            let cells: Vec<usize> = r.red_flags.iter().map(|f| f.at).collect();
            assert_eq!(cells, [at], "{}", r.name);
        }

        // The tab: date order, then the element's place; each scene links to its strip.
        let tab = red_flags_view(s.schedule(), main.0).unwrap();
        assert_eq!((tab.count, tab.flags, tab.undated), (2, 2, false));
        // The small calendar opens on the start month (live note 26).
        assert!(tab.start.is_some());
        let c = &tab.clashes[0];
        assert_eq!(
            (c.name.as_str(), c.date.as_str(), c.weekday),
            ("ADA", "2027-03-02", "Tue")
        );
        assert_eq!(
            (c.type_name.as_str(), c.hard, c.day),
            ("Unavailable", true, Some(2))
        );
        assert_eq!(c.sheets[0].scene, "Scene 2");
        assert_eq!(c.sheets[0].strip, Some(day2.id));
        assert_eq!(tab.clashes[1].name, "BEN");
        assert!(!tab.clashes[1].hard);

        // A fitting for ADA on Fri Mar 5 (no shoot day) clashes too, as an event.
        s.edit(|x| {
            let b = x.boards[0].id;
            x.set_event(
                b,
                main,
                None,
                Event {
                    // No name: the clash calls it by its kind.
                    name: String::new(),
                    kind: EventKind::Fitting,
                    from: d("2027-03-05"),
                    to: d("2027-03-05"),
                    elements: vec![ElementId(ada)],
                    color: None,
                },
            )
            .map(|_| ())
            .map_err(say)
        })
        .unwrap();
        s.edit(|x| set_red_flag(x, ada, &input(away, "not_after", "2027-03-04", "", true)))
            .unwrap();
        let tab = red_flags_view(s.schedule(), main.0).unwrap();
        let last = tab.clashes.last().unwrap();
        assert_eq!((last.date.as_str(), last.day), ("2027-03-05", None));
        assert!(last.sheets.is_empty());
        assert_eq!(last.events[0].name, "Fitting");
        // A missing sub-board is refused with a sentence.
        assert_eq!(
            red_flags_view(s.schedule(), 999_999).unwrap_err(),
            "That sub-board is no longer in the schedule."
        );
    }

    /// Behavior note 23: the Red flags tab adds flags for several elements and dates in
    /// one undo step, lists every flag, deletes the checked ones; Red Flag Types adds the
    /// standard types an older schedule lacks and never deletes one.
    #[test]
    fn several_at_once_the_table_and_the_standard_types() {
        let (mut session, main, ada, ben) = session();
        assert_eq!(session.schedule().missing_standard_red_flag_types(), 16);
        session.edit(add_standard_types).unwrap();
        assert!(session.edit(add_standard_types).is_err());
        let s = session.schedule();
        let t = types(s);
        assert_eq!(t.len(), 16);
        assert_eq!(
            (t[0].name.as_str(), t[0].builtin, t[0].event_code),
            ("Travel", Some("travel"), Some("TR"))
        );
        assert_eq!(t[3].event_code, None);
        let unavailable = t[3].id;
        let e = session
            .edit(|x| delete_red_flag_type(x, unavailable))
            .unwrap_err();
        assert!(e.contains("standard type"), "{e}");

        let add = |dates: &str, picked: &[&str], from: &str| AddFlagsInput {
            elements: vec![ada, ben, ada],
            red_flag_type: unavailable,
            dates: dates.into(),
            picked: picked.iter().map(|x| x.to_string()).collect(),
            from: from.into(),
            to: String::new(),
            note: " Second job ".into(),
            hard: true,
        };
        let picked = add("picked", &["3/3/2027", "2027-03-02", "2027-03-09"], "");
        session.edit(|x| add_red_flags(x, &picked)).unwrap();
        // One undo step takes all four away.
        assert!(session.undo());
        assert!(flag_rows(session.schedule()).is_empty());
        session.edit(|x| add_red_flags(x, &picked)).unwrap();
        session
            .edit(|x| add_red_flags(x, &add("not_before", &[], "2027-03-02")))
            .unwrap();
        // A range with no last date is that one date.
        session
            .edit(|x| add_red_flags(x, &add("range", &[], "2027-03-20")))
            .unwrap();
        let s = session.schedule();
        let rows = flag_rows(s);
        assert_eq!(rows.len(), 8);
        // Open starts first, then by date, then element.
        assert_eq!(
            (rows[0].start.clone(), rows[0].end.as_deref()),
            (None, Some("2027-03-01"))
        );
        assert_eq!(rows[0].name, "ADA");
        assert_eq!(rows[1].name, "BEN");
        assert_eq!(rows[2].start.as_deref(), Some("2027-03-02"));
        assert_eq!(rows[2].end.as_deref(), Some("2027-03-03"));
        assert_eq!(rows[2].note, "Second job");
        assert_eq!(rows[7].start.as_deref(), Some("2027-03-20"));
        assert_eq!(rows[7].end.as_deref(), Some("2027-03-20"));
        // Eleven elements on a thousand dates a day apart would be 11,000 flags: refused
        // with a sentence, nothing added.
        let mut wide = s.clone();
        let cast = wide.cast().unwrap();
        let crowd: Vec<u32> = (0..11)
            .map(|i| wide.add_element(cast, &format!("EXTRA {i}")).unwrap().0)
            .collect();
        let start: Date = "2027-03-01".parse().unwrap();
        let every_other: Vec<String> = (0..1000)
            .map(|i| start.add_days(i * 2).to_string())
            .collect();
        let before = wide.clone();
        let e = add_red_flags(
            &mut wide,
            &AddFlagsInput {
                elements: crowd,
                picked: every_other,
                ..add("picked", &[], "")
            },
        )
        .unwrap_err();
        assert!(e.starts_with("That would add 11000 red flags."), "{e}");
        assert_eq!(wide, before);
        let view = red_flags_view(s, main.0).unwrap();
        assert_eq!(view.rows.len(), 8);
        assert_eq!(view.categories[0].elements.len(), 2);

        for bad in [
            add("picked", &[], ""),
            add("picked", &["2027-02-30"], ""),
            add("range", &[], ""),
            AddFlagsInput {
                elements: vec![],
                ..add("picked", &["2027-03-02"], "")
            },
            AddFlagsInput {
                elements: vec![9999],
                ..add("picked", &["2027-03-02"], "")
            },
        ] {
            assert!(session.edit(|x| add_red_flags(x, &bad)).is_err());
        }

        let gone: Vec<u32> = rows[..2].iter().map(|r| r.id).collect();
        session.edit(|x| delete_red_flags(x, &gone)).unwrap();
        assert_eq!(flag_rows(session.schedule()).len(), 6);
        assert!(session.edit(|x| delete_red_flags(x, &[])).is_err());
        assert!(session.edit(|x| delete_red_flags(x, &gone)).is_err());
        // Delete Checked is one undo step: both come back, and redo takes both again.
        assert!(session.undo());
        assert_eq!(flag_rows(session.schedule()).len(), 8);
        assert!(session.redo());
        assert_eq!(flag_rows(session.schedule()).len(), 6);
    }
}
