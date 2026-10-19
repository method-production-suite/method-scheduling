//! The breakdown sheet edits the Breakdown tab offers, as plain functions on a
//! [`Schedule`], and the strict readers for what is typed into its fields. Each edit
//! validates what the page sent; `Session::edit` makes it one undo step. Integers only:
//! pages are eighths, times are whole minutes or seconds (decisions.md, entry 3).

use serde::Serialize;

use method_schedule_core::model::{
    BoardId, CategoryId, ElementId, Error, Minutes, Place, Seconds, Sheet, SheetId, SheetOrder,
    StripKind,
};
use method_schedule_core::reports::{set_header_value, HeaderField};
use method_schedule_core::{Eighths, Schedule};

use crate::board::{board_id, EditResult};

/// The longest text a header field takes, in characters.
pub const MAX_TEXT: usize = 10_000;
/// The longest element or category name, in characters.
pub const MAX_NAME: usize = 200;

/// The header fields the page may set with [`set_field`], by the names it sends.
pub const FIELDS: &[&str] = &[
    "number",
    "scenes",
    "int_ext",
    "set",
    "time_of_day",
    "sunrise",
    "sunset",
    "shoot_time",
    "screen_time",
    "synopsis",
    "location",
    "script_pages",
    "script_day",
    "unit",
    "sequence",
    "episode",
    "comments",
];

// ---------------------------------------------------------------------------
// Reading what was typed. Each is strict: digits and one colon, nothing else.

fn digits(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 9 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `a:bb` with `bb` two digits below 60: the two parts. `None` for anything else.
fn colon_pair(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once(':')?;
    if b.len() != 2 {
        return None;
    }
    let (a, b) = (digits(a)?, digits(b)?);
    (b < 60).then_some((a, b))
}

/// The longest duration typed as `h:mm` (a shoot time, a banner's duration): 24:00
/// (decision 50). A merged sheet can still add up to more.
pub const MAX_DURATION: Minutes = Minutes(24 * 60);

/// Why a typed `h:mm` duration was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationRefusal {
    /// Not `h:mm`.
    Form,
    /// Past [`MAX_DURATION`].
    TooLong,
}

/// Decimal hours, `1.5` or `1.25` (or `.5`), in whole minutes: hundredths of an hour
/// times 60 over 100, half a minute rounding up (decisions.md, entry 81). One or two
/// digits after the point; `None` for anything else, `1` among them (a bare number is
/// never a duration).
pub fn decimal_hours(s: &str) -> Option<u32> {
    let (h, f) = s.split_once('.')?;
    if f.is_empty() || f.len() > 2 || !f.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours = if h.is_empty() { 0 } else { digits(h)? };
    let hundredths: u32 = if f.len() == 1 {
        f.parse::<u32>().ok()? * 10
    } else {
        f.parse().ok()?
    };
    let total = hours.checked_mul(100)?.checked_add(hundredths)?;
    total.checked_mul(60)?.checked_add(50).map(|x| x / 100)
}

/// Minutes as decimal hours with two places (`1:30` is `1.50`): hundredths rounded half
/// up. The Breakdown tab shows a shoot time this way when the decimal display is on.
pub fn show_decimal_hours(m: Minutes) -> String {
    let hundredths = (u64::from(m.0) * 100 + 30) / 60;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// A duration as `h:mm` (`1:30`) or decimal hours (`1.5`), in whole minutes, at most
/// [`MAX_DURATION`]. Empty is `None`.
pub fn parse_duration(s: &str) -> Result<Option<Minutes>, DurationRefusal> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if let Some(total) = decimal_hours(s) {
        return if total <= MAX_DURATION.0 {
            Ok(Some(Minutes(total)))
        } else {
            Err(DurationRefusal::TooLong)
        };
    }
    let (h, m) = colon_pair(s).ok_or(DurationRefusal::Form)?;
    match h.checked_mul(60).and_then(|x| x.checked_add(m)) {
        Some(total) if total <= MAX_DURATION.0 => Ok(Some(Minutes(total))),
        _ => Err(DurationRefusal::TooLong),
    }
}

/// The sentence for a refused duration: `what` names the field (`Shoot time`).
pub fn duration_say(what: &str, s: &str, why: DurationRefusal) -> String {
    let s = s.trim();
    match why {
        DurationRefusal::Form => {
            format!("{what} {s:?} must look like 1:30 (hours and minutes) or 1.5 (hours).")
        }
        DurationRefusal::TooLong => {
            format!("{what} {s:?} is longer than a day. Type 24:00 at most.")
        }
    }
}

/// Shoot time as `h:mm` (`1:30`), in whole minutes, at most 24:00. Empty clears it.
pub fn parse_shoot_time(s: &str) -> Result<Option<Minutes>, String> {
    parse_duration(s).map_err(|why| duration_say("Shoot time", s, why))
}

/// Screen time as `m:ss` (`2:05`), in whole seconds. Empty clears it.
pub fn parse_screen_time(s: &str) -> Result<Option<Seconds>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    colon_pair(s)
        .and_then(|(m, sec)| m.checked_mul(60)?.checked_add(sec))
        .map(|x| Some(Seconds(x)))
        .ok_or_else(|| format!("Screen time {s:?} must look like 2:05 (minutes and seconds)."))
}

/// A time of day as 24-hour `hh:mm` (`06:45`, `6:45`, `18:20`), in minutes after
/// midnight. Empty clears it. `what` names the field in the error.
pub fn parse_clock(what: &str, s: &str) -> Result<Option<u32>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    colon_pair(s)
        .filter(|(h, _)| *h < 24 && s.split(':').next().is_some_and(|h| h.len() <= 2))
        .map(|(h, m)| Some(h * 60 + m))
        .ok_or_else(|| format!("{what} {s:?} must be a 24-hour time like 06:45."))
}

/// A page length typed as whole pages plus eighths, each a whole number (either may be
/// empty for 0). Eighths of 8 or more roll into pages.
pub fn parse_pages(pages: &str, eighths: &str) -> Result<Eighths, String> {
    let read = |s: &str, what: &str| -> Result<u32, String> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(0);
        }
        digits(s).ok_or_else(|| format!("{what} {s:?} must be a whole number."))
    };
    let p = read(pages, "Pages")?;
    let e = read(eighths, "Eighths")?;
    Eighths::new(p, e).ok_or_else(|| "That page length is too large.".to_string())
}

/// `h:mm`, for the page.
pub fn show_minutes(m: Minutes) -> String {
    format!("{}:{:02}", m.0 / 60, m.0 % 60)
}

/// `m:ss`, for the page.
pub fn show_seconds(s: Seconds) -> String {
    format!("{}:{:02}", s.0 / 60, s.0 % 60)
}

/// `hh:mm`, for the page.
pub fn show_clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn text(what: &str, value: &str, max: usize) -> Result<String, String> {
    let t = value.trim();
    if t.chars().count() > max {
        return Err(format!("{what} is longer than {max} characters."));
    }
    Ok(t.to_string())
}

// ---------------------------------------------------------------------------
// Edits

fn sheet_id(s: &Schedule, id: u32) -> EditResult<SheetId> {
    s.sheet(SheetId(id))
        .map(|x| x.id)
        .ok_or_else(|| "That sheet is no longer in the schedule.".to_string())
}

fn sheet_mut(s: &mut Schedule, id: u32) -> EditResult<&mut Sheet> {
    s.sheet_mut(SheetId(id))
        .ok_or_else(|| "That sheet is no longer in the schedule.".to_string())
}

/// Sets one header field, read strictly: times as `h:mm`, `m:ss` or 24-hour `hh:mm`, text
/// trimmed. Page lengths go through [`set_pages`].
///
/// A one-value field (set, location, I/E and the rest of `reports::HeaderField`) takes the
/// spelling another sheet already has when it matches ignoring case (decisions.md, entry
/// 155).
pub fn set_field(s: &mut Schedule, sheet: u32, field: &str, value: &str) -> EditResult {
    if let Some(f) = HeaderField::from_key(field) {
        let v = text("That field", value, MAX_TEXT)?;
        sheet_id(s, sheet)?;
        set_header_value(s, SheetId(sheet), f, &v);
        return Ok(());
    }
    let h = sheet_mut(s, sheet)?;
    match field {
        "sunrise" => h.sunrise = parse_clock("Sunrise", value)?,
        "sunset" => h.sunset = parse_clock("Sunset", value)?,
        "shoot_time" => h.shoot_time = parse_shoot_time(value)?,
        "screen_time" => h.screen_time = parse_screen_time(value)?,
        _ => {
            let slot = match field {
                "number" => &mut h.number,
                "scenes" => &mut h.scenes,
                "int_ext" => &mut h.int_ext,
                "set" => &mut h.set,
                "time_of_day" => &mut h.time_of_day,
                "synopsis" => &mut h.synopsis,
                "location" => &mut h.location,
                "script_pages" => &mut h.script_pages,
                "script_day" => &mut h.script_day,
                "unit" => &mut h.unit,
                "sequence" => &mut h.sequence,
                "episode" => &mut h.episode,
                "comments" => &mut h.comments,
                _ => return Err("That is not a sheet field.".to_string()),
            };
            *slot = text("That field", value, MAX_TEXT)?;
        }
    }
    Ok(())
}

/// Sets the page length from whole pages and eighths (`parse_pages`).
pub fn set_pages(s: &mut Schedule, sheet: u32, pages: &str, eighths: &str) -> EditResult {
    let length = parse_pages(pages, eighths)?;
    sheet_mut(s, sheet)?.pages = length;
    Ok(())
}

/// Puts an existing element on a sheet (once), with the elements it links to (behavior
/// note 04).
pub fn tag_element(s: &mut Schedule, sheet: u32, element: u32) -> EditResult {
    let sheet = sheet_id(s, sheet)?;
    s.element(ElementId(element))
        .ok_or("That element is no longer in the schedule.")?;
    s.tag_with_links(sheet, ElementId(element))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The largest quantity of one element on one sheet.
pub use method_schedule_core::model::MAX_QUANTITY;

/// Sets how many of an element on a sheet the sheet needs (typed as a whole number, 1 to
/// [`MAX_QUANTITY`]) and whether that counts in the day's total for its category.
pub fn set_quantity(
    s: &mut Schedule,
    sheet: u32,
    element: u32,
    count: &str,
    daily: bool,
) -> EditResult {
    let sheet = sheet_id(s, sheet)?;
    let t = count.trim();
    let n = if t.is_empty() {
        1
    } else {
        digits(t)
            .filter(|n| (1..=MAX_QUANTITY).contains(n))
            .ok_or_else(|| format!("A quantity is a whole number from 1 to {MAX_QUANTITY}."))?
    };
    s.set_quantity(sheet, ElementId(element), n, daily)
        .map_err(crate::elements::say)
}

/// Puts the element called `name` in `category` on a sheet: the one already there, or a
/// new one. A category that doesn't exist yet is added. Returns the element's id.
pub fn create_element(s: &mut Schedule, sheet: u32, category: &str, name: &str) -> EditResult<u32> {
    let sheet = sheet_id(s, sheet)?;
    let name = text("The name", name, MAX_NAME)?;
    let category = text("The category", category, MAX_NAME)?;
    if name.is_empty() {
        return Err("Type a name for the element.".to_string());
    }
    if category.is_empty() {
        return Err("Choose a category for the new element.".to_string());
    }
    let cat: CategoryId = s.add_category(&category);
    let el = match s.element_named(cat, &name) {
        Some(el) => el,
        None => s.add_element(cat, &name).map_err(|e| e.to_string())?,
    };
    s.tag_with_links(sheet, el).map_err(|e| e.to_string())?;
    Ok(el.0)
}

/// Takes an element off a sheet. The element stays in the schedule.
pub fn untag_element(s: &mut Schedule, sheet: u32, element: u32) -> EditResult {
    let sheet = sheet_id(s, sheet)?;
    s.untag(sheet, ElementId(element))
        .map_err(|e| e.to_string())
}

/// Adds an empty sheet after sheet `after` when that one is on a sub-board of `board` (the
/// board shown); otherwise (in the Boneyard, or no sheet given) at the end of the Boneyard.
/// Every other board gets it at the end of its Boneyard. Returns the new sheet's id.
pub fn new_sheet(s: &mut Schedule, board: u32, after: Option<u32>) -> EditResult<u32> {
    let board = board_id(s, board)?;
    let at = after
        .and_then(|a| s.sheet_place(board, SheetId(a)))
        .filter(|p| p.sub_board.is_some())
        .map(|p| Place {
            sub_board: p.sub_board,
            index: p.index + 1,
        })
        .unwrap_or(Place {
            sub_board: None,
            index: usize::MAX,
        });
    s.add_sheet_at(Sheet::default(), board, at)
        .map(|id| id.0)
        .map_err(|e| e.to_string())
}

/// Deletes a sheet and its strip on every board. Returns the sheet to show next: the one
/// after it in `board`'s order as the Breakdown tab steps (`by`: board order, sheet number
/// or scene number), else the one before, else none.
pub fn delete_sheet(
    s: &mut Schedule,
    board: u32,
    sheet: u32,
    by: SheetOrder,
) -> EditResult<Option<u32>> {
    let id = sheet_id(s, sheet)?;
    let order = s.sheets_in_order(board_id(s, board)?, by);
    let at = order.iter().position(|x| *x == id);
    let next = at.and_then(|i| {
        order
            .get(i + 1)
            .or_else(|| i.checked_sub(1).map(|j| &order[j]))
    });
    s.delete_sheet(id).map_err(|e| e.to_string())?;
    Ok(next.map(|x| x.0))
}

// ---------------------------------------------------------------------------
// Merging and duplicating sheets (Board > Merge Strips, Duplicate Strip; Breakdown >
// Duplicate Sheet; decisions.md, entries 35 and 37)

/// A sheet named for the page: its id and `Scene 12` (or a plain phrase without a number).
#[derive(Clone, Debug, Serialize)]
pub struct SheetRefDto {
    pub sheet: u32,
    pub title: String,
}

/// A field a merge changed on the surviving sheet, before and after, as the Breakdown tab
/// shows it.
#[derive(Clone, Debug, Serialize)]
pub struct CombinedDto {
    /// The engine's field name (`scenes`, `eighths`...).
    pub field: &'static str,
    /// The Breakdown tab's label (`Scenes`, `Pages`...).
    pub label: &'static str,
    pub before: String,
    pub after: String,
}

/// A value a merge leaves out: another sheet's value for a field the survivor keeps.
#[derive(Clone, Debug, Serialize)]
pub struct DroppedDto {
    pub sheet: u32,
    pub title: String,
    pub field: &'static str,
    pub label: &'static str,
    pub value: String,
}

/// What Merge Strips would do, worked out on a copy of the schedule: the engine's
/// `SheetMerge` with names and labels.
#[derive(Clone, Debug, Serialize)]
pub struct SheetMergeDto {
    pub survivor: SheetRefDto,
    /// The sheets merged in and deleted, in the order they were selected.
    pub merged: Vec<SheetRefDto>,
    pub combined: Vec<CombinedDto>,
    /// Elements the survivor gains, as `NAME (Category)`.
    pub elements_added: Vec<String>,
    pub dropped: Vec<DroppedDto>,
}

/// The Breakdown tab's label for one of the file's sheet field names.
pub fn field_label(field: &str) -> &'static str {
    match field {
        "scenes" => "Scenes",
        "int_ext" => "I/E",
        "set" => "Set",
        "time_of_day" => "Time of day",
        "sunrise" => "Sunrise",
        "sunset" => "Sunset",
        "eighths" => "Pages",
        "shoot_minutes" => "Shoot time",
        "synopsis" => "Synopsis",
        "location" => "Location",
        "script_pages" => "Script pages",
        "script_day" => "Script day",
        "screen_seconds" => "Screen time",
        "unit" => "Unit",
        "sequence" => "Sequence",
        "episode" => "Episode",
        "comments" => "Comments",
        "number" => "Sheet number",
        "images" => "Images",
        _ => "A field",
    }
}

/// `Scene 12`, or `A sheet with no scene number`.
pub fn sheet_title(h: &Sheet) -> String {
    if h.scenes.trim().is_empty() {
        "A sheet with no scene number".to_string()
    } else {
        format!("Scene {}", h.scenes.trim())
    }
}

/// The sheets the page named, checked: each once, each in the schedule, in `board`'s
/// order (each sub-board's strips in turn, then the Boneyard).
fn sheets_in_order(s: &Schedule, board: BoardId, sheets: &[u32]) -> EditResult<Vec<SheetId>> {
    let mut wanted = Vec::with_capacity(sheets.len());
    for &id in sheets {
        let id = sheet_id(s, id)?;
        if wanted.contains(&id) {
            return Err("A sheet was named twice.".to_string());
        }
        wanted.push(id);
    }
    let order = s.sheets_in_board_order(board);
    wanted.sort_by_key(|id| order.iter().position(|x| x == id).unwrap_or(usize::MAX));
    Ok(wanted)
}

/// The survivor and the others for Merge Strips. `sheets` comes in the order the strips
/// were selected: the first selected keeps its sheet and the rest join in that order, as
/// in the reference app (behavior note 18, engine check E-S18, decision 46). `board` is
/// only checked: the merge is the same on every board.
fn merge_parts(s: &Schedule, board: u32, sheets: &[u32]) -> EditResult<(SheetId, Vec<SheetId>)> {
    board_id(s, board)?;
    let mut wanted = Vec::with_capacity(sheets.len());
    for &id in sheets {
        let id = sheet_id(s, id)?;
        if wanted.contains(&id) {
            return Err("A sheet was named twice.".to_string());
        }
        wanted.push(id);
    }
    if wanted.len() < 2 {
        return Err("Select two or more scenes to merge.".to_string());
    }
    Ok((wanted[0], wanted[1..].to_vec()))
}

fn merge_say(e: Error) -> String {
    match e {
        Error::NoSuchSheet(_) => "That sheet is no longer in the schedule.".to_string(),
        Error::NoSuchBoard(_) => "That board is no longer in the schedule.".to_string(),
        Error::Broken(why) if why.contains("too large") => {
            "The merged page count or time would be too large.".to_string()
        }
        other => crate::elements::say(other),
    }
}

/// What merging `sheets` on `board` would do, without changing the schedule.
pub fn merge_preview(s: &Schedule, board: u32, sheets: &[u32]) -> Result<SheetMergeDto, String> {
    let (survivor, others) = merge_parts(s, board, sheets)?;
    let mut copy = s.clone();
    let r = copy.merge_sheets(survivor, &others).map_err(merge_say)?;
    let title = |id: SheetId| s.sheet(id).map(sheet_title).unwrap_or_default();
    let sheet_ref = |id: SheetId| SheetRefDto {
        sheet: id.0,
        title: title(id),
    };
    Ok(SheetMergeDto {
        survivor: sheet_ref(r.survivor),
        merged: r.merged.iter().map(|id| sheet_ref(*id)).collect(),
        combined: r
            .combined
            .iter()
            .map(|c| CombinedDto {
                field: c.field,
                label: field_label(c.field),
                before: c.before.clone(),
                after: c.after.clone(),
            })
            .collect(),
        elements_added: r
            .elements_added
            .iter()
            .filter_map(|id| s.element(*id))
            .map(|e| {
                let cat = s.category(e.category).map_or("", |c| c.name.as_str());
                format!("{} ({cat})", e.name)
            })
            .collect(),
        dropped: r
            .dropped
            .iter()
            .map(|d| DroppedDto {
                sheet: d.sheet.0,
                title: title(d.sheet),
                field: d.field,
                label: field_label(d.field),
                value: d.value.clone(),
            })
            .collect(),
    })
}

/// Merges `sheets`, given in the order they were selected, into the first of them (Board >
/// Merge Strips). Returns the surviving sheet's id.
pub fn merge_sheets(s: &mut Schedule, board: u32, sheets: &[u32]) -> EditResult<u32> {
    let (survivor, others) = merge_parts(s, board, sheets)?;
    s.merge_sheets(survivor, &others).map_err(merge_say)?;
    Ok(survivor.0)
}

/// Copies each of `sheets`, every field and element, to a new sheet whose strip goes right
/// after the original on every board where that is on a sub-board, else at the end of the
/// Boneyard (`Schedule::duplicate_sheet`). Returns the copies' ids in `board`'s order.
pub fn duplicate_sheets(s: &mut Schedule, board: u32, sheets: &[u32]) -> EditResult<Vec<u32>> {
    let board = board_id(s, board)?;
    let ordered = sheets_in_order(s, board, sheets)?;
    if ordered.is_empty() {
        return Err("Select a scene to duplicate.".to_string());
    }
    let mut out = Vec::with_capacity(ordered.len());
    for id in ordered {
        out.push(s.duplicate_sheet(id).map_err(merge_say)?.0);
    }
    Ok(out)
}

/// The strip of `sheet` on `board`, if both are there.
pub fn strip_of(s: &Schedule, board: BoardId, sheet: u32) -> Option<u32> {
    let b = s.board(board)?;
    b.sub_boards
        .iter()
        .flat_map(|x| &x.strips)
        .chain(&b.boneyard)
        .find(|st| st.kind == StripKind::Sheet(SheetId(sheet)))
        .map(|st| st.id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::session::{read_script, Session};
    use std::path::Path;

    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        let mut s = Session::new();
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s
    }

    #[test]
    fn times_and_pages_are_read_strictly() {
        assert_eq!(parse_shoot_time("1:30"), Ok(Some(Minutes(90))));
        assert_eq!(parse_shoot_time(" 0:05 "), Ok(Some(Minutes(5))));
        assert_eq!(parse_shoot_time("12:00"), Ok(Some(Minutes(720))));
        assert_eq!(
            parse_shoot_time("24:00"),
            Ok(Some(Minutes(1440))),
            "a whole day"
        );
        assert_eq!(parse_shoot_time(""), Ok(None));
        // Past a day is refused, with its own sentence (decision 50).
        for long in ["24:01", "25:00", "900:45", "99999999:00"] {
            assert_eq!(
                parse_shoot_time(long),
                Err(format!(
                    "Shoot time {long:?} is longer than a day. Type 24:00 at most."
                )),
            );
        }
        for bad in [
            "90", "1:3", "1:60", "1.", "1.505", "1:30:00", "-1:00", ":30", "1:3a", "1,5", "1 :30",
            "1.5.0", "a.5",
        ] {
            assert!(parse_shoot_time(bad).is_err(), "{bad}");
        }
        // Decimal hours (decision 81): hundredths of an hour, half a minute up.
        for (typed, minutes) in [
            ("1.5", 90),
            ("1.25", 75),
            (".5", 30),
            ("0.01", 1),
            ("2.33", 140),
            ("24.0", 1440),
        ] {
            assert_eq!(
                parse_shoot_time(typed),
                Ok(Some(Minutes(minutes))),
                "{typed}"
            );
        }
        assert!(parse_shoot_time("24.01")
            .unwrap_err()
            .contains("longer than a day"));
        assert_eq!(show_decimal_hours(Minutes(90)), "1.50");
        assert_eq!(show_decimal_hours(Minutes(140)), "2.33");
        assert_eq!(show_decimal_hours(Minutes(1)), "0.02");
        assert_eq!(show_decimal_hours(Minutes(0)), "0.00");
        assert_eq!(parse_screen_time("2:05"), Ok(Some(Seconds(125))));
        assert!(parse_screen_time("2:5").is_err());
        assert!(parse_screen_time("125").is_err());
        assert_eq!(parse_clock("Sunrise", "06:45"), Ok(Some(405)));
        assert_eq!(parse_clock("Sunrise", "6:45"), Ok(Some(405)));
        assert_eq!(parse_clock("Sunset", "23:59"), Ok(Some(1439)));
        for bad in ["24:00", "006:45", "6", "6:4", "6.45", "6:45 pm"] {
            let err = parse_clock("Sunset", bad).unwrap_err();
            assert!(err.starts_with("Sunset "), "{err}");
        }
        assert_eq!(parse_pages("1", "3"), Ok(Eighths(11)));
        assert_eq!(parse_pages("", "11"), Ok(Eighths(11)), "eighths roll over");
        assert_eq!(parse_pages("2", ""), Ok(Eighths(16)));
        assert_eq!(parse_pages("", ""), Ok(Eighths(0)));
        for (p, e) in [
            ("1.5", ""),
            ("", "3/8"),
            ("-1", "0"),
            ("1", "x"),
            ("99999999999", ""),
        ] {
            assert!(parse_pages(p, e).is_err(), "{p} {e}");
        }
        assert!(parse_pages("536870911", "8").is_err(), "overflow");
        assert_eq!(show_minutes(Minutes(90)), "1:30");
        assert_eq!(show_seconds(Seconds(125)), "2:05");
        assert_eq!(show_clock(405), "06:45");
    }

    #[test]
    fn errors_follow_the_voice() {
        let errors = [
            parse_shoot_time("x").unwrap_err(),
            parse_screen_time("x").unwrap_err(),
            parse_clock("Sunrise", "x").unwrap_err(),
            parse_pages("x", "").unwrap_err(),
            parse_pages("", "x").unwrap_err(),
        ];
        for e in errors {
            assert!(!e.contains('!') && !e.contains('\u{2014}'), "{e}");
            assert!(e.ends_with('.'), "{e}");
        }
    }

    #[test]
    fn a_header_value_takes_the_spelling_another_sheet_has() {
        // Behavior note 25 (E-L12): `room 2` in the Set used the existing `ROOM 2`.
        let mut s = harbor();
        let (a, b) = (s.schedule().sheets[0].id.0, s.schedule().sheets[1].id.0);
        s.edit(|x| set_field(x, a, "set", "ROOM 2")).unwrap();
        s.edit(|x| set_field(x, b, "set", "room 2")).unwrap();
        s.edit(|x| set_field(x, b, "unit", "second")).unwrap();
        s.edit(|x| set_field(x, a, "unit", "Second")).unwrap();
        s.edit(|x| set_field(x, b, "synopsis", "ROOM 2")).unwrap();
        s.edit(|x| set_field(x, a, "synopsis", "room 2")).unwrap();
        let (ha, hb) = (&s.schedule().sheets[0], &s.schedule().sheets[1]);
        assert_eq!(hb.set, "ROOM 2");
        assert_eq!(ha.unit, "second");
        assert_eq!(ha.synopsis, "room 2", "only the one-value fields");
    }

    #[test]
    fn every_field_is_one_undo_step_and_bad_input_changes_nothing() {
        let mut s = harbor();
        let id = s.schedule().sheets[0].id.0;
        for (field, value) in [
            ("scenes", " 1A "),
            ("int_ext", "EXT"),
            ("set", "BOATHOUSE"),
            ("time_of_day", "DUSK"),
            ("sunrise", "06:45"),
            ("sunset", "19:10"),
            ("shoot_time", "2:15"),
            ("screen_time", "1:05"),
            ("synopsis", "Tove finds the lamp."),
            ("location", "Pier 4"),
            ("script_pages", "1-2"),
            ("script_day", "D1"),
            ("unit", "Main"),
            ("sequence", "A"),
            ("episode", "101"),
            ("comments", "Rain bars"),
        ] {
            s.edit(|x| set_field(x, id, field, value)).unwrap();
        }
        s.edit(|x| set_pages(x, id, "1", "10")).unwrap();
        let h = &s.schedule().sheets[0];
        assert_eq!(h.scenes, "1A");
        assert_eq!((h.sunrise, h.sunset), (Some(405), Some(1150)));
        assert_eq!(h.shoot_time, Some(Minutes(135)));
        assert_eq!(h.screen_time, Some(Seconds(65)));
        assert_eq!(h.pages, Eighths(18));
        assert_eq!(h.comments, "Rain bars");
        // Each one undoes on its own.
        s.undo();
        assert_eq!(s.schedule().sheets[0].pages, Eighths(2));
        s.undo();
        assert_eq!(s.schedule().sheets[0].comments, "");

        let before = s.schedule().clone();
        for (field, value) in [
            ("shoot_time", "90"),
            ("sunrise", "25:00"),
            ("nope", "x"),
            ("scenes", &"x".repeat(MAX_TEXT + 1)),
        ] {
            assert!(
                s.edit(|x| set_field(x, id, field, value)).is_err(),
                "{field}"
            );
        }
        assert!(s.edit(|x| set_pages(x, id, "1/2", "")).is_err());
        assert!(s.edit(|x| set_field(x, 999_999, "set", "X")).is_err());
        assert_eq!(s.schedule(), &before);
        // Clearing a time.
        s.edit(|x| set_field(x, id, "shoot_time", "")).unwrap();
        assert_eq!(s.schedule().sheets[0].shoot_time, None);
        // The file round-trips what was typed.
        let text = method_schedule_format::to_string(s.schedule());
        assert_eq!(
            &method_schedule_format::from_str(&text).unwrap(),
            s.schedule()
        );
    }

    #[test]
    fn quick_add_tags_creates_and_removes_elements() {
        let mut s = harbor();
        let id = s.schedule().sheets[0].id.0;
        let on = |s: &Session| s.schedule().sheets[0].elements.len();
        let start = on(&s);
        // A new prop in a new category.
        let lamp = s
            .edit(|x| create_element(x, id, "Props", "Storm lamp"))
            .unwrap();
        assert_eq!(on(&s), start + 1);
        assert!(s.schedule().category_named("props").is_some());
        // The same name again (any case) tags the one already there: no step at all.
        let again = s
            .edit(|x| create_element(x, id, "PROPS", " storm LAMP "))
            .unwrap();
        assert_eq!(again, lamp);
        assert_eq!(on(&s), start + 1);
        // An element from elsewhere, by id.
        let other = s.schedule().sheets[2].id.0;
        s.edit(|x| tag_element(x, other, lamp)).unwrap();
        assert!(s.schedule().sheets[2].elements.contains(&ElementId(lamp)));
        // Removing it from one sheet leaves the element and the other sheet alone.
        s.edit(|x| untag_element(x, id, lamp)).unwrap();
        assert_eq!(on(&s), start);
        assert!(s.schedule().element(ElementId(lamp)).is_some());
        assert!(s.schedule().sheets[2].elements.contains(&ElementId(lamp)));
        // Refusals.
        assert!(s.edit(|x| create_element(x, id, "Props", "  ")).is_err());
        assert!(s.edit(|x| create_element(x, id, "", "Oar")).is_err());
        assert!(s.edit(|x| tag_element(x, id, 999_999)).is_err());
        assert!(s
            .edit(|x| create_element(x, 999_999, "Props", "Oar"))
            .is_err());
    }

    #[test]
    fn new_sheets_go_after_the_current_one_or_to_the_boneyard() {
        let mut s = harbor();
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        let ids: Vec<u32> = s.schedule().boards[0]
            .boneyard
            .iter()
            .map(|x| x.id.0)
            .collect();
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids[..2], Some(sb), 0))
            .unwrap();
        let order = |s: &Session| -> Vec<u32> {
            let b = s.schedule().boards[0].id;
            s.schedule()
                .sheets_in_board_order(b)
                .iter()
                .map(|x| x.0)
                .collect()
        };
        let first = order(&s)[0];
        let before = order(&s);
        let new = s
            .edit(|x| new_sheet(x, x.boards[0].id.0, Some(first)))
            .unwrap();
        let after = order(&s);
        assert_eq!(
            after[1], new,
            "right after the current sheet on its sub-board"
        );
        assert_eq!(after.len(), before.len() + 1);
        // From the Boneyard (or with no sheet), at the end of the Boneyard.
        let last = *before.last().unwrap();
        let tail = s
            .edit(|x| new_sheet(x, x.boards[0].id.0, Some(last)))
            .unwrap();
        assert_eq!(*order(&s).last().unwrap(), tail);
        let none = s.edit(|x| new_sheet(x, x.boards[0].id.0, None)).unwrap();
        assert_eq!(*order(&s).last().unwrap(), none);
        // One undo step each.
        s.undo();
        s.undo();
        s.undo();
        assert_eq!(order(&s), before);
    }

    #[test]
    fn deleting_a_sheet_says_which_to_show_next() {
        let mut s = harbor();
        let b = s.schedule().boards[0].id;
        let order = s.schedule().sheets_in_board_order(b);
        let n = order.len();
        let next = s
            .edit(|x| delete_sheet(x, x.boards[0].id.0, order[1].0, SheetOrder::Board))
            .unwrap();
        assert_eq!(next, Some(order[2].0));
        let next = s
            .edit(|x| delete_sheet(x, x.boards[0].id.0, order[n - 1].0, SheetOrder::Board))
            .unwrap();
        assert_eq!(next, Some(order[n - 2].0), "the last goes back one");
        assert!(
            s.edit(|x| delete_sheet(x, x.boards[0].id.0, order[1].0, SheetOrder::Board))
                .is_err(),
            "gone"
        );
        s.undo();
        s.undo();
        assert_eq!(s.schedule().sheets_in_board_order(b), order);
        // The only sheet: nothing to show next.
        let mut one = Session::new();
        let only = one.edit(|x| new_sheet(x, x.boards[0].id.0, None)).unwrap();
        assert_eq!(
            one.edit(|x| delete_sheet(x, x.boards[0].id.0, only, SheetOrder::Board))
                .unwrap(),
            None
        );
        // Stepping by scene number, the next is the next scene number, wherever its strip.
        let mut s = harbor();
        let b = s.schedule().boards[0].id;
        let first = s.schedule().sheets_in_board_order(b)[0].0;
        s.edit(|x| set_field(x, first, "scenes", "2A")).unwrap();
        let by_scene = s.schedule().sheets_in_order(b, SheetOrder::Scene);
        assert_ne!(by_scene, s.schedule().sheets_in_board_order(b));
        let next = s
            .edit(|x| delete_sheet(x, x.boards[0].id.0, by_scene[0].0, SheetOrder::Scene))
            .unwrap();
        assert_eq!(next, Some(by_scene[1].0));
    }

    /// The sheet with scene number `n` in `s`.
    fn scene_id(s: &Schedule, n: &str) -> u32 {
        s.sheets.iter().find(|h| h.scenes == n).unwrap().id.0
    }

    #[test]
    fn merge_strips_previews_then_merges_into_the_first_selected_as_one_step() {
        let mut s = harbor();
        let b = s.schedule().boards[0].id.0;
        let (one, three, four) = (
            scene_id(s.schedule(), "1"),
            scene_id(s.schedule(), "3"),
            scene_id(s.schedule(), "4"),
        );
        let before = s.schedule().clone();
        // Selected 4, then 3, then 1 (behavior note 18): scene 4 survives, though scene 1
        // is topmost on the board, and the scenes join in that order.
        let p = merge_preview(s.schedule(), b, &[four, three, one]).unwrap();
        assert_eq!(s.schedule(), &before, "a preview changes nothing");
        assert_eq!(p.survivor.title, "Scene 4");
        let merged: Vec<&str> = p.merged.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(merged, ["Scene 3", "Scene 1"]);
        let scenes = p.combined.iter().find(|c| c.field == "scenes").unwrap();
        assert_eq!((scenes.label, scenes.after.as_str()), ("Scenes", "4, 3, 1"));
        assert!(p.combined.iter().any(|c| c.label == "Pages"));
        assert_eq!(
            p.elements_added,
            ["HARBORMASTER LUND (Cast)", "TOVE (Cast)"]
        );
        // Scene 4's own values win; the others' that differ are left out, said so.
        let dropped: Vec<(&str, &str, &str)> = p
            .dropped
            .iter()
            .map(|d| (d.label, d.title.as_str(), d.value.as_str()))
            .collect();
        assert_eq!(
            dropped,
            [
                ("I/E", "Scene 3", "EXT"),
                ("Set", "Scene 3", "HARBOR WALL"),
                ("Time of day", "Scene 1", "NIGHT"),
            ]
        );
        let four_strip = strip_of(s.schedule(), BoardId(b), four);

        let survivor = s.edit(|x| merge_sheets(x, b, &[four, three, one])).unwrap();
        assert_eq!(survivor, four);
        s.schedule().check().unwrap();
        assert_eq!(s.schedule().sheets.len(), 3);
        let h = s.schedule().sheet(SheetId(four)).unwrap();
        assert_eq!(h.scenes, "4, 3, 1");
        // Scene 4 keeps its strip and place.
        assert_eq!(strip_of(s.schedule(), BoardId(b), four), four_strip);
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");

        // Refused, nothing changed.
        for bad in [vec![one], vec![one, one], vec![one, 999_999], vec![]] {
            let err = s.edit(|x| merge_sheets(x, b, &bad)).unwrap_err();
            assert!(err.ends_with('.') && !err.contains('!'), "{err}");
            assert!(merge_preview(s.schedule(), b, &bad).is_err());
        }
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn duplicate_places_each_copy_after_its_strip_as_one_step() {
        let mut s = harbor();
        let b = s.schedule().boards[0].id;
        let sb = s.schedule().boards[0].sub_boards[0].id;
        let two = scene_id(s.schedule(), "2");
        let four = scene_id(s.schedule(), "4");
        // Scene 2 on Main, the rest in the Boneyard.
        let strip = strip_of(s.schedule(), b, two).unwrap();
        s.edit(|x| crate::board::move_strips(x, b.0, &[strip], Some(sb.0), 0))
            .unwrap();
        let before = s.schedule().clone();
        let copies = s.edit(|x| duplicate_sheets(x, b.0, &[four, two])).unwrap();
        assert_eq!(copies.len(), 2);
        s.schedule().check().unwrap();
        let x = s.schedule();
        // In board order: scene 2's copy first, right after it on Main; scene 4's at the
        // Boneyard's end.
        assert_eq!(x.sheet(SheetId(copies[0])).unwrap().scenes, "2");
        assert_eq!(x.sheet(SheetId(copies[1])).unwrap().scenes, "4");
        let main: Vec<u32> = x.boards[0].sub_boards[0]
            .strips
            .iter()
            .map(|st| st.id.0)
            .collect();
        assert_eq!(main[1], strip_of(x, b, copies[0]).unwrap());
        assert_eq!(
            x.boards[0].boneyard.last().unwrap().id.0,
            strip_of(x, b, copies[1]).unwrap()
        );
        let (a, c) = (
            x.sheet(SheetId(two)).unwrap(),
            x.sheet(SheetId(copies[0])).unwrap(),
        );
        assert_eq!(
            (&a.set, &a.elements),
            (&c.set, &c.elements),
            "every field copied"
        );
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");
        assert!(s.edit(|x| duplicate_sheets(x, b.0, &[])).is_err());
        assert!(s.edit(|x| duplicate_sheets(x, b.0, &[999_999])).is_err());
    }
}
