//! What the page draws, and nothing more: the board shown as day groups of strips, one
//! breakdown sheet, and the per-sub-board tabs (Elements, day out of days, Calendar) for the
//! sub-board the page names. Plain serde shapes, built from the engine after each command.
//! Every string here is shown with `textContent` (docs/APP.md, "Security model").

use std::collections::BTreeMap;

use serde::Serialize;

use method_schedule_core::days::{days, days_with};
use method_schedule_core::dood::{dood_with, sort_rows, summary, Options as DoodOptions, SortKey};
use method_schedule_core::model::{Align, StripLayout, TextSize};
use method_schedule_core::model::{
    Board, BoardId, Minutes, Sheet, SheetOrder, Strip, StripKind, SubBoard,
};
use method_schedule_core::model::{ElementIndex, IdMap, SheetIndex};
use method_schedule_core::reports::{column_cell, id_order, strip_cell, CastOrder, HeaderField};
use method_schedule_core::{Date, Eighths, Schedule, Weekday};

use crate::session::Session;

/// After every command: the file, the history buttons and the board.
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub title: String,
    pub file: FileDto,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Every board, in order, for the board picker.
    pub boards: Vec<NamedDto>,
    /// The board shown (`Session::shown_board`).
    pub board: BoardDto,
    /// One line for the page to show after the command (a script import's count).
    pub notice: Option<String>,
    /// The app's Autosave switch (src/safety_app.rs).
    pub autosave: bool,
    /// The sheet a command made or moved to (a new sheet, the one after a deleted sheet):
    /// the Breakdown tab shows it.
    pub sheet: Option<u32>,
    /// The category a command made (Add Category): the Elements tab shows it.
    pub category: Option<u32>,
    /// The sub-board a command made (Add Sub-board): the page shows it.
    pub sub_board: Option<u32>,
    /// The strip a command made (a banner, a day break): the board selects it.
    pub strip: Option<u32>,
    /// The strips a command made or moved (a duplicate, a paste), in board order: the
    /// board selects them all, the last one focused.
    pub strips: Vec<u32>,
    /// How many conflicts the board shown has (src/conflicts.rs), for the Conflicts tab's
    /// badge.
    pub conflicts: usize,
    /// Every red flag type, in order, with how many flags use it (src/redflags.rs).
    pub red_flag_types: Vec<crate::redflags::RedFlagTypeDto>,
    /// How many standard red flag types the schedule lacks (decision 113): Red Flag
    /// Types offers to add them when some are missing.
    pub standard_types_missing: usize,
    /// The layout library (decisions.md, entry 78), and the strip layout the one-line
    /// report picked (null: the board's).
    pub layouts: Vec<SavedLayoutDto>,
    pub one_line_layout: Option<u32>,
    /// What Paste would paste, and what to mark as cut (src/clipboard.rs).
    pub clipboard: Option<crate::clipboard::ClipboardDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileDto {
    /// The file's name, never its folder; "Untitled" before the first save.
    pub name: String,
    pub saved_once: bool,
    pub dirty: bool,
    /// The file changed on disk outside the app, so autosave into it is paused. The page
    /// keeps a warning up for as long as this holds.
    pub conflict: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardDto {
    pub id: u32,
    pub name: String,
    /// The board's strip layout: the columns its scene strips show, in order
    /// (decisions.md, entry 39). Each strip's `cells` follow it.
    pub layout: Vec<LayoutColumnDto>,
    /// Its text size step (`small`, `normal`, `large`, decisions.md, entry 87).
    pub layout_size: &'static str,
    /// Whether the layout hides the red flag and conflict marks (decision 136).
    pub layout_marks: MarksDto,
    pub sub_boards: Vec<SubBoardDto>,
    pub boneyard: Vec<StripDto>,
    /// The strip colors the user set (decisions.md, entry 60), for the Strip Colors
    /// dialog and the text colors per strip type.
    pub colors: ColorsDto,
    /// What the board's day breaks show (decisions.md, entry 67), with the layouts it
    /// picked (entry 78) applied, and its shoot day layout's text.
    pub day_look: DayLookDto,
    pub shoot_text: String,
    /// Its own strip layout and day look, which it uses where it picked no named layout,
    /// and its picks (named layouts' ids, null: its own).
    pub own_layout: Vec<LayoutColumnDto>,
    pub own_layout_size: &'static str,
    pub own_layout_marks: MarksDto,
    pub own_day_look: DayLookDto,
    pub uses: UsesDto,
    /// Scenes on the board in all, and their pages.
    pub scenes: usize,
    pub pages: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SubBoardDto {
    pub id: u32,
    pub name: String,
    /// `YYYY-MM-DD`.
    pub start: Option<String>,
    pub days: Vec<DayDto>,
    /// Strips after the last day break: not scheduled.
    pub unscheduled: Vec<StripDto>,
    /// How many strips the sub-board holds, day breaks included (the end index).
    pub len: usize,
    /// How many red flag clashes the sub-board has (the Red flags tab's badge).
    pub red_flags: usize,
    /// The pages its scheduled scenes still have to shoot (M5), left out while nothing in
    /// the schedule is reported shot.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub left: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DayDto {
    /// The pages its scenes still have to shoot (M5), left out while nothing in the
    /// schedule is reported shot.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub left: String,
    pub number: u32,
    pub date: Option<String>,
    /// `Mon` to `Sun`, with the date.
    pub weekday: Option<String>,
    pub pages: String,
    /// Shoot time, `h:mm`.
    pub time: String,
    pub scenes: usize,
    pub strips: Vec<StripDto>,
    /// The day break that closes the day.
    pub end: StripDto,
    /// The day's screen time (`m:ss`), its earliest sunrise and latest sunset (`hh:mm`,
    /// typed on its sheets or worked out at their located places, decision 167; empty
    /// when none is known), and its full date (`Wednesday, March 3, 2027`):
    /// what the board's day look may show (decisions.md, entry 67).
    pub screen_time: String,
    pub sunrise: String,
    pub sunset: String,
    pub full_date: Option<String>,
    /// The day's element quantities (`Background 45 · Animals 3`, decisions.md, entry
    /// 87), worked out only when the board's day breaks show them.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub quantity: String,
}

/// What a board's day breaks show (decisions.md, entry 67), as the page edits it.
#[derive(Clone, Debug, Serialize)]
pub struct DayLookDto {
    pub shoot_day_strip: bool,
    pub screen_time: bool,
    pub sunrise: bool,
    pub sunset: bool,
    pub quantity: bool,
    pub text: String,
}

/// `Wednesday, March 3, 2027`.
pub fn full_date(d: Date) -> String {
    let (y, m, day) = d.ymd();
    let wd = match d.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    };
    format!(
        "{wd}, {} {day}, {y}",
        MONTHS[(m as usize).saturating_sub(1) % 12]
    )
}

/// The named layouts a board picked (decisions.md, entry 78).
#[derive(Clone, Debug, Serialize)]
pub struct UsesDto {
    pub strips: Option<u32>,
    pub day_breaks: Option<u32>,
    pub shoot_day: Option<u32>,
}

/// The strip colors as the page sends and shows them: `#rrggbb`.
#[derive(Clone, Debug, Serialize)]
pub struct ColorsDto {
    pub rules: Vec<ColorRuleDto>,
    pub scene_text: Option<String>,
    pub banner_text: Option<String>,
    pub break_text: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ColorRuleDto {
    pub int_ext: String,
    pub time_of_day: String,
    pub color: String,
    /// The pair's own text color (decision 108), or none.
    pub text: Option<String>,
}

pub fn colors_dto(c: &method_schedule_core::model::StripColors) -> ColorsDto {
    ColorsDto {
        rules: c
            .rules
            .iter()
            .map(|r| ColorRuleDto {
                int_ext: r.int_ext.clone(),
                time_of_day: r.time_of_day.clone(),
                color: r.color.hex(),
                text: r.text.map(|x| x.hex()),
            })
            .collect(),
        scene_text: c.scene_text.map(|x| x.hex()),
        banner_text: c.banner_text.map(|x| x.hex()),
        break_text: c.break_text.map(|x| x.hex()),
    }
}

/// One column of a strip layout, as the page draws it.
#[derive(Clone, Debug, Serialize)]
pub struct LayoutColumnDto {
    /// `StripField::name`: `scene`, `int_ext`, `cast`...
    pub field: &'static str,
    /// The column heading.
    pub label: String,
    /// `narrow`, `medium`, `wide` or `widest`.
    pub width: &'static str,
    /// Its line, from 1 (decisions.md, entries 78 and 87), and how many of the first
    /// line's columns it covers.
    pub line: u8,
    pub span: u8,
    /// `auto`, `left`, `center` or `right`; bold and italic; a custom text field's text.
    pub align: &'static str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// A report column's own text size (decisions.md, entry 90): `small`, `normal` or
    /// `large`; empty for the report's (and on the board).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub size: &'static str,
    /// The category an element count counts (decision 147); none: every element. The
    /// category a category's elements lists (decision 194).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<u32>,
    /// A category's elements by board ID rather than name (decision 194).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub board_ids: bool,
    /// Line styles (decision 185): a line after the cell, a rule under it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rule_after: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rule_under: bool,
}

/// A strip layout's mark switches (decision 136): `true` hides the marks.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct MarksDto {
    pub hide_red_flags: bool,
    pub hide_conflicts: bool,
}

pub fn marks_dto(l: &StripLayout) -> MarksDto {
    MarksDto {
        hide_red_flags: l.hide_red_flags,
        hide_conflicts: l.hide_conflicts,
    }
}

/// A layout's columns for the page. A category's elements is headed by its category's
/// name in `s` (decision 194).
pub fn layout_dto(s: &Schedule, l: &StripLayout) -> Vec<LayoutColumnDto> {
    l.columns
        .iter()
        .map(|c| LayoutColumnDto {
            field: c.field.name(),
            label: match c.category.and_then(|x| s.category(x)) {
                Some(cat)
                    if c.field == method_schedule_core::model::StripField::CategoryElements =>
                {
                    cat.name.clone()
                }
                _ => c.field.label().to_string(),
            },
            width: c.width.name(),
            line: c.line + 1,
            span: c.span,
            align: c.align.name(),
            bold: c.bold,
            italic: c.italic,
            text: c.text.clone(),
            size: "",
            category: c.category.map(|x| x.0),
            board_ids: c.board_ids,
            rule_after: c.rule_after,
            rule_under: c.rule_under,
        })
        .collect()
}

/// One named layout as the page lists it (decisions.md, entry 78): its id, name and type
/// (`strips`, `day_breaks`, `shoot_day`), and its columns or switches.
#[derive(Clone, Debug, Serialize)]
pub struct SavedLayoutDto {
    pub id: u32,
    pub name: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<LayoutColumnDto>,
    /// A strip layout's text size step (decisions.md, entry 87).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub size: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<DayLookDto>,
    /// A strip layout's mark switches (decision 136).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marks: Option<MarksDto>,
    /// How many boards pick it, and whether the one-line report does.
    pub boards: usize,
    pub one_line: bool,
}

pub fn look_dto(d: &method_schedule_core::model::DayLook) -> DayLookDto {
    DayLookDto {
        shoot_day_strip: d.shoot_day_strip,
        screen_time: d.screen_time,
        sunrise: d.sunrise,
        sunset: d.sunset,
        quantity: d.quantity,
        text: d.text.clone(),
    }
}

/// The layout library for the page.
pub fn layouts_dto(s: &Schedule) -> Vec<SavedLayoutDto> {
    use method_schedule_core::layouts::LayoutBody;
    s.layouts
        .layouts
        .iter()
        .map(|l| {
            let kind = l.body.kind();
            let (columns, size, look) = match &l.body {
                LayoutBody::Strips(x) => (layout_dto(s, x), x.size.name(), None),
                LayoutBody::DayBreaks(d) | LayoutBody::ShootDay(d) => {
                    (Vec::new(), "", Some(look_dto(d)))
                }
            };
            SavedLayoutDto {
                id: l.id.0,
                name: l.name.clone(),
                kind: kind.name(),
                columns,
                size,
                look,
                marks: match &l.body {
                    LayoutBody::Strips(x) => Some(marks_dto(x)),
                    _ => None,
                },
                boards: s.boards_using(l.id).count(),
                one_line: s.layouts.one_line == Some(l.id),
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct StripDto {
    pub id: u32,
    /// Position in its sub-board's (or the Boneyard's) strip list.
    pub index: usize,
    /// `scene`, `banner` or `break`.
    pub kind: &'static str,
    /// A scene's breakdown sheet.
    pub sheet: Option<u32>,
    pub scene: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    pub pages: String,
    /// Cast board IDs, or names where an element has none, in board ID order.
    pub cast: Vec<String>,
    /// A scene's text for each column of the board's strip layout, in order.
    pub cells: Vec<String>,
    /// A banner's text.
    pub text: String,
    /// A banner's duration, `h:mm` (it counts toward the day's time).
    pub duration: String,
    /// A scene spanning days (decisions.md, entry 76): which part this strip is, from 1,
    /// of how many on the board. Left out (0) for a scene with one strip.
    #[serde(skip_serializing_if = "is_zero")]
    pub part: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub parts: u32,
    /// A scene a script revision left out (decisions 69 and 91): drawn struck. Left out
    /// when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub omitted: bool,
    /// What has been shot of the scene (M5, from `plan.scenes_shot`): `part` or `done`,
    /// left out while nothing is; and in words (`Shot 3/8 of 6/8`).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub shot: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub shot_text: String,
    /// A banner's look (decisions.md, entry 65): bold, its palette color's name (empty:
    /// the board's banner color), two lines high.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_color: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tall: bool,
    /// A banner's text size step and alignment (decisions.md, entry 87): `small` or
    /// `large`, `center` or `right`; left out at the usual size and the left.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_size: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_align: &'static str,
    /// A banner's font family (decisions.md, entry 187): `serif`, `sans` or `mono`; left
    /// out for the board's own.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_font: &'static str,
    /// The strip color by craft convention: `int-day`, `ext-day`, `int-night`,
    /// `ext-night`, or empty for banners and breaks.
    pub tone: &'static str,
    /// A scene strip's color from the user's strip colors (`#rrggbb`), and the text color
    /// on it (the scene text color set, else black or white, whichever reads). Left out
    /// when no strip color matches: the strip keeps its `tone`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ink: String,
    /// The red flag clashes on a scheduled scene's day, in words (`ADA, Unavailable,
    /// 2027-03-02 to 2027-03-03, hard`). Left out when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red_flags: Vec<String>,
    /// Those clashes' red flag types, whose colors paint the mark (decisions.md, entry 84).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red_flag_types: Vec<u32>,
    /// The conflicts on it, in words (`KIT on 2027-03-02 also on Unit B day 3`;
    /// decisions.md, entry 83). Left out when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
}

/// The day out of days grid for one sub-board and one category: one
/// row per element, one column per date from the first to the last date any row covers,
/// and each row's summary at the right.
#[derive(Clone, Debug, Serialize)]
pub struct DoodGridDto {
    pub sub_board: String,
    pub start: Option<String>,
    /// Every category, for the picker.
    pub categories: Vec<NamedDto>,
    /// The category shown: the one asked for while it exists, else Cast, else the first.
    /// `None` while a header field is shown.
    pub category: Option<u32>,
    /// The header fields that work as categories, for the picker (inventory section 4).
    pub headers: Vec<HeaderDto>,
    /// The header field shown instead of a category, if any: one row per value.
    pub header: Option<&'static str>,
    /// Every element of the category shown is left off the board, so no cell is marked
    /// as a conflict (decision 120).
    pub excluded: bool,
    pub dates: Vec<DoodDateDto>,
    pub rows: Vec<DoodRowDto>,
    /// Why there are no rows, when there are none.
    pub empty: Option<String>,
    /// The schedule's code letters, `[key, label, letters]` in order, for the legend.
    pub codes: Vec<(&'static str, &'static str, String)>,
    /// Worked out without holds (the "without holds" view, decision 59).
    pub no_holds: bool,
}

/// How [`dood_grid_with`] works the grid out beyond the defaults.
#[derive(Clone, Debug, Default)]
pub struct GridOpts {
    pub no_holds: bool,
    /// Up to four sort keys, each with descending (`true`) or ascending.
    pub sort: Vec<(SortKey, bool)>,
    /// Only dates in this range (inclusive), when given.
    pub from: Option<Date>,
    pub to: Option<Date>,
    /// Only these elements (empty: all).
    pub elements: Vec<method_schedule_core::model::ElementId>,
    /// The first date moves back to this weekday, the week's first: the DOOD report
    /// pages by whole weeks from there (DOD-19, behavior note 27, decision 141).
    pub week_start: Option<method_schedule_core::Weekday>,
}

/// The code letters' labels, in `DoodCodes::KEYS` order.
const CODE_LABELS: [&str; 11] = [
    "start",
    "work",
    "finish",
    "hold",
    "drop",
    "pickup",
    "travel",
    "fitting",
    "rehearsal",
    "holiday",
    "company travel day",
];

#[derive(Clone, Debug, Serialize)]
pub struct DoodDateDto {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// `Mon` to `Sun`.
    pub weekday: &'static str,
    /// The day of the month, and the month's short name.
    pub day: u32,
    pub month: &'static str,
    /// `work` (a shoot day), `off` (a weekly or single day off), `holiday` or `move` (a
    /// company travel day).
    pub kind: &'static str,
    /// The shoot day number, when a shoot day on the board falls on this date.
    pub shoot_day: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoodRowDto {
    pub element: u32,
    pub board_id: String,
    pub name: String,
    /// One code per date in `dates` (`SW`, `W`, `WF`, `H`, `FTH`, `/`, `M`, ...), empty
    /// where the element does nothing.
    pub cells: Vec<String>,
    /// The work span (start to finish) as indexes into `dates`, drawn as a bar; `None` for
    /// an element with events only.
    pub span: Option<(usize, usize)>,
    pub work: u32,
    pub hold: u32,
    pub start: Option<String>,
    pub finish: Option<String>,
    pub total: u32,
    /// The element's own switches, which the row edits (decision 59).
    pub holds: bool,
    pub drops: bool,
    /// The cells that clash with a red flag: the date's index and the clash in words.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red_flags: Vec<DoodFlagDto>,
    /// The cells the DOOD flags as conflicts: an event on a work day (behavior note 17,
    /// A4). The date's index and why, in words.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<DoodFlagDto>,
}

/// A day out of days cell that clashes with a red flag.
#[derive(Clone, Debug, Serialize)]
pub struct DoodFlagDto {
    pub at: usize,
    pub text: String,
}

/// The view's `file` part (also sent alone after an autosave).
pub fn file_dto(s: &Session) -> FileDto {
    FileDto {
        name: s.display_name(),
        saved_once: s.path().is_some(),
        dirty: s.is_dirty(),
        conflict: s.in_conflict(),
    }
}

pub fn view(s: &Session, notice: Option<String>) -> Result<View, String> {
    let shown = s.shown_board().ok_or("The schedule has no board.")?;
    Ok(View {
        title: s.schedule().title.clone(),
        file: file_dto(s),
        can_undo: s.can_undo(),
        can_redo: s.can_redo(),
        boards: s
            .schedule()
            .boards
            .iter()
            .map(|b| NamedDto {
                id: b.id.0,
                name: b.name.clone(),
            })
            .collect(),
        board: board(s.schedule(), shown)?,
        notice,
        autosave: true,
        sheet: None,
        category: None,
        sub_board: None,
        strip: None,
        strips: Vec::new(),
        conflicts: crate::conflicts::count(s.schedule(), shown),
        red_flag_types: crate::redflags::types(s.schedule()),
        standard_types_missing: s.schedule().missing_standard_red_flag_types(),
        layouts: layouts_dto(s.schedule()),
        one_line_layout: s.schedule().layouts.one_line.map(|x| x.0),
        clipboard: s.clipboard().map(crate::clipboard::dto),
    })
}

pub fn weekday(d: Date) -> &'static str {
    match d.weekday() {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

pub fn hm(m: Minutes) -> String {
    format!("{}:{:02}", m.0 / 60, m.0 % 60)
}

/// Strip color by the usual stripboard convention. Anything not marked exterior is
/// interior; anything not marked night is day (dusk and dawn included, for now).
pub fn tone(sheet: &Sheet) -> &'static str {
    // Compared in place, ignoring case, so drawing a strip copies no text.
    let ext = sheet
        .int_ext
        .trim()
        .as_bytes()
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"EXT"));
    let night = sheet
        .time_of_day
        .as_bytes()
        .windows(5)
        .any(|w| w.eq_ignore_ascii_case(b"NIGHT"));
    match (ext, night) {
        (false, false) => "int-day",
        (true, false) => "ext-day",
        (false, true) => "int-night",
        (true, true) => "ext-night",
    }
}

/// What drawing a board looks up again and again, indexed once per view.
pub struct Lookup<'a> {
    sheets: SheetIndex<'a>,
    elements: ElementIndex<'a>,
    cast: CastOrder<'a>,
}

impl<'a> Lookup<'a> {
    pub fn new(s: &'a Schedule) -> Lookup<'a> {
        Lookup {
            sheets: s.sheet_index(),
            elements: s.element_index(),
            cast: CastOrder::new(s, s.cast()),
        }
    }
}

fn strip(s: &Schedule, look: &Lookup, layout: &StripLayout, st: &Strip, index: usize) -> StripDto {
    let mut out = StripDto {
        id: st.id.0,
        index,
        kind: "break",
        sheet: None,
        scene: String::new(),
        int_ext: String::new(),
        set: String::new(),
        time_of_day: String::new(),
        pages: String::new(),
        cast: Vec::new(),
        cells: Vec::new(),
        text: String::new(),
        duration: String::new(),
        tone: "",
        color: String::new(),
        ink: String::new(),
        part: 0,
        parts: 0,
        omitted: false,
        shot: "",
        shot_text: String::new(),
        bold: false,
        banner_color: String::new(),
        tall: false,
        banner_size: "",
        banner_align: "",
        banner_font: "",
        red_flags: Vec::new(),
        red_flag_types: Vec::new(),
        conflicts: Vec::new(),
    };
    match &st.kind {
        StripKind::Sheet(id) => {
            out.kind = "scene";
            out.sheet = Some(id.0);
            out.omitted = s.plan.is_omitted(*id);
            if let Some(h) = look.sheets.get(id) {
                out.scene = h.scenes.clone();
                out.int_ext = h.int_ext.clone();
                out.set = h.set.clone();
                out.time_of_day = h.time_of_day.clone();
                out.pages = h.pages.to_string();
                if let Some(x) = s.plan.shot_so_far(*id) {
                    out.shot = if x.complete { "done" } else { "part" };
                    out.shot_text = if x.complete {
                        format!("Shot, finished ({} of {})", x.eighths, h.pages)
                    } else {
                        format!("Shot {} of {}", x.eighths, h.pages)
                    };
                }
                out.tone = tone(h);
                if let Some(r) = s.strip_colors.rule_for(h) {
                    // The pair's own text color wins, then the scene text color (decision 108).
                    out.color = r.color.hex();
                    out.ink = s.strip_colors.text_of(h).unwrap_or(r.color.ink()).hex();
                }
                // The cast once for the strip: its own list and any Cast cells.
                out.cast = look.cast.ids(h);
                out.cells = layout
                    .columns
                    .iter()
                    .map(|c| column_cell(s, h, c, &out.cast))
                    .collect();
            }
        }
        StripKind::Banner(b) => {
            out.kind = "banner";
            out.text = b.text.clone();
            out.duration = hm(b.duration);
            out.bold = b.style.bold;
            out.banner_color = b
                .style
                .color
                .map(method_schedule_core::model::BannerColor::text)
                .unwrap_or_default();
            out.tall = b.style.tall;
            if b.style.size != TextSize::Normal {
                out.banner_size = b.style.size.name();
            }
            if b.style.align != Align::Auto {
                out.banner_align = b.style.align.name();
            }
            if b.style.font != method_schedule_core::model::BannerFont::Usual {
                out.banner_font = b.style.font.name();
            }
        }
        StripKind::DayBreak => {}
    }
    out
}

fn sub_board_dto(
    s: &Schedule,
    board: method_schedule_core::model::BoardId,
    sb: &SubBoard,
    look: &Lookup,
    marks: &crate::redflags::BoardMarks,
) -> Result<SubBoardDto, String> {
    let totals = days_with(s, &look.sheets, board, sb.id).map_err(|e| e.to_string())?;
    let mut out = SubBoardDto {
        id: sb.id.0,
        name: sb.name.clone(),
        start: sb.start.map(|d| d.to_string()),
        days: Vec::with_capacity(totals.days.len()),
        unscheduled: Vec::new(),
        len: sb.strips.len(),
        red_flags: marks.counts.get(&sb.id.0).copied().unwrap_or(0),
        left: String::new(),
    };
    let reported = !s.plan.scenes_shot.is_empty();
    let mut sub_left = 0u32;
    let mut sub_seen = std::collections::HashSet::new();
    // Each day's strips, room made for them up front.
    let room = |n: usize| {
        totals
            .days
            .get(n)
            .map_or(totals.unscheduled.len(), |d| d.strips.len())
    };
    let mut current = Vec::with_capacity(room(0));
    let mut dates = sb.start.map(|d| sb.calendar.day_dates(d));
    s.board(board)
        .ok_or("That board is no longer in the schedule.")?;
    // The strip layout the board uses: a named one it picked, else its own (entry 78).
    let layout = s.strip_layout_of(board);
    // The day's element quantities, only when its day breaks show them (entry 87).
    let quantity = s.day_look_of(board).0.quantity;
    for (i, st) in sb.strips.iter().enumerate() {
        let mut dto = strip(s, look, &layout, st, i);
        // The strip designer's switch can hide the red flag marks (decision 136).
        if !layout.hide_red_flags {
            if let Some(m) = marks.strips.get(&st.id.0) {
                dto.red_flags = m.clone();
            }
            if let Some(k) = marks.kinds.get(&st.id.0) {
                dto.red_flag_types = k.clone();
            }
        }
        if st.kind != StripKind::DayBreak {
            current.push(dto);
            continue;
        }
        let n = out.days.len();
        let day = totals
            .days
            .get(n)
            .ok_or("day totals don't match the board")?;
        let date = dates.as_mut().and_then(|x| x.date(day.number));
        let sheets: Vec<&Sheet> = current
            .iter()
            .filter_map(|x| x.sheet)
            .filter_map(|id| {
                look.sheets
                    .get(&method_schedule_core::model::SheetId(id))
                    .copied()
            })
            .collect();
        let clock = |v: Option<u32>| v.map(crate::sheet::show_clock).unwrap_or_default();
        // Typed times, else the sun at a located sheet's place (decision 167).
        let sun = method_schedule_core::timeline::day_break_sun(s, &sheets, date);
        let left = if reported {
            // A scene in parts over several days counts on each of its days, and once in
            // the sub-board's total.
            let mut seen = std::collections::HashSet::new();
            let mut n = 0u32;
            for h in sheets.iter().filter(|h| !s.plan.is_omitted(h.id)) {
                let rest =
                    method_schedule_core::plan::eighths_left(h.pages, s.plan.shot_so_far(h.id)).0;
                if seen.insert(h.id) {
                    n += rest;
                }
                if sub_seen.insert(h.id) {
                    sub_left += rest;
                }
            }
            format!("{} left", method_schedule_core::Eighths(n))
        } else {
            String::new()
        };
        out.days.push(DayDto {
            left,
            number: day.number,
            date: date.map(|d| d.to_string()),
            weekday: date.map(|d| weekday(d).to_string()),
            pages: day.pages.to_string(),
            time: hm(day.time),
            scenes: current.iter().filter(|x| x.kind == "scene").count(),
            strips: std::mem::replace(&mut current, Vec::with_capacity(room(n + 1))),
            end: dto,
            screen_time: crate::sheet::show_seconds(day.screen_time),
            sunrise: clock(sun.0),
            sunset: clock(sun.1),
            full_date: date.map(full_date),
            quantity: if quantity {
                method_schedule_core::reports::day_quantity_text_with(s, &look.elements, &sheets)
            } else {
                String::new()
            },
        });
    }
    out.unscheduled = current;
    if reported {
        out.left = format!("{} left", method_schedule_core::Eighths(sub_left));
    }
    Ok(out)
}

/// A sub-board by id, with the board it is on.
pub fn sub_ref(s: &Schedule, sub: u32) -> Result<(&Board, &SubBoard), String> {
    s.boards
        .iter()
        .find_map(|b| b.sub_boards.iter().find(|x| x.id.0 == sub).map(|x| (b, x)))
        .ok_or_else(|| "That sub-board is no longer in the schedule.".to_string())
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Each split scene's strips on board `b`, in board order (decisions.md, entry 76): strip id
/// to (part, parts).
fn parts_of(b: &Board) -> IdMap<u32, (u32, u32)> {
    let sheet_strips = || {
        b.sub_boards
            .iter()
            .flat_map(|sb| &sb.strips)
            .filter_map(|st| match st.kind {
                StripKind::Sheet(id) => Some((id.0, st.id.0)),
                _ => None,
            })
    };
    let mut parts: IdMap<u32, u32> = IdMap::default();
    for (sheet, _) in sheet_strips() {
        *parts.entry(sheet).or_default() += 1;
    }
    let mut seen: IdMap<u32, u32> = IdMap::default();
    let mut out = IdMap::default();
    for (sheet, strip) in sheet_strips() {
        let of = parts[&sheet];
        if of > 1 {
            let n = seen.entry(sheet).or_default();
            *n += 1;
            out.insert(strip, (*n, of));
        }
    }
    out
}

/// A board by id.
pub fn board_ref(s: &Schedule, board: u32) -> Result<&Board, String> {
    s.board(BoardId(board))
        .ok_or_else(|| "That board is no longer in the schedule.".to_string())
}

/// Board `id` as the page draws it.
pub fn board(s: &Schedule, id: BoardId) -> Result<BoardDto, String> {
    let b = s.board(id).ok_or("The schedule has no board.")?;
    let lookup = Lookup::new(s);
    let layout = s.strip_layout_of(id);
    let (look, shoot_text) = s.day_look_of(id);
    let marks = crate::redflags::board_marks(s, b);
    let sub_boards = b
        .sub_boards
        .iter()
        .map(|sb| sub_board_dto(s, b.id, sb, &lookup, &marks))
        .collect::<Result<Vec<_>, _>>()?;
    let boneyard = b
        .boneyard
        .iter()
        .enumerate()
        .map(|(i, st)| strip(s, &lookup, &layout, st, i))
        .collect();
    let pages: Eighths = s.sheets.iter().map(|h| h.pages).sum();
    let parts = parts_of(b);
    let mut sub_boards = sub_boards;
    // Conflict marks on the strips (decisions.md, entry 83).
    let conflicts = crate::conflictmarks::marks(s, b.id);
    // The strip designer's switch can hide them (decision 136).
    if !conflicts.strips.is_empty() && !layout.hide_conflicts {
        for sb in &mut sub_boards {
            for st in sb
                .days
                .iter_mut()
                .flat_map(|d| d.strips.iter_mut())
                .chain(sb.unscheduled.iter_mut())
            {
                if let Some(c) = conflicts.strips.get(&st.id) {
                    st.conflicts = c.clone();
                }
            }
        }
    }
    if !parts.is_empty() {
        for sb in &mut sub_boards {
            for st in sb
                .days
                .iter_mut()
                .flat_map(|d| d.strips.iter_mut())
                .chain(sb.unscheduled.iter_mut())
            {
                if let Some(&(n, of)) = parts.get(&st.id) {
                    st.part = n;
                    st.parts = of;
                }
            }
        }
    }
    Ok(BoardDto {
        id: b.id.0,
        name: b.name.clone(),
        layout: layout_dto(s, &layout),
        layout_size: layout.size.name(),
        layout_marks: marks_dto(&layout),
        sub_boards,
        boneyard,
        colors: colors_dto(&s.strip_colors),
        day_look: look_dto(&look),
        shoot_text,
        own_layout: layout_dto(s, &b.strip_layout),
        own_layout_size: b.strip_layout.size.name(),
        own_layout_marks: marks_dto(&b.strip_layout),
        own_day_look: look_dto(&b.day_look),
        uses: UsesDto {
            strips: b.uses.strips.map(|x| x.0),
            day_breaks: b.uses.day_breaks.map(|x| x.0),
            shoot_day: b.uses.shoot_day.map(|x| x.0),
        },
        scenes: s.sheets.len(),
        pages: pages.to_string(),
    })
}

/// The Breakdown tab: one sheet's header fields as typed, its elements by category, where
/// its strip is, and what quick add and the pick lists offer.
#[derive(Clone, Debug, Serialize)]
pub struct SheetDto {
    pub id: u32,
    /// The sheet number as typed (decisions.md, entry 77).
    pub number: String,
    pub scenes: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    /// `hh:mm`, or empty.
    pub sunrise: String,
    pub sunset: String,
    /// Whole pages and the eighths left over, as typed into the two page fields.
    pub pages_whole: String,
    pub pages_eighths: String,
    /// The length as the board shows it: `1 3/8`.
    pub pages: String,
    /// `h:mm`, or empty.
    pub shoot_time: String,
    /// The shoot time as decimal hours (`1.50`), or empty: the Breakdown tab shows it so
    /// when the decimal display is on (decisions.md, entry 81).
    pub shoot_time_decimal: String,
    /// `m:ss`, or empty.
    pub screen_time: String,
    pub synopsis: String,
    pub location: String,
    pub script_pages: String,
    pub script_day: String,
    pub unit: String,
    pub sequence: String,
    pub episode: String,
    pub comments: String,
    /// Where its strip is on the board shown: `Main, day 3, Wed 2027-03-03`, `Main, not
    /// scheduled` or `Boneyard`.
    pub place: String,
    /// Its strip on the board shown, and the sub-board that holds it (`None` for the
    /// Boneyard): the place is a link to it on the Board tab (decisions.md, entry 58).
    pub strip: Option<u32>,
    pub strip_sub_board: Option<u32>,
    /// The date it shoots on, when scheduled on a dated sub-board (`YYYY-MM-DD`): where
    /// New Event starts.
    pub date: Option<String>,
    /// Its place in the stepping order (from 0; board order unless another was asked
    /// for, decisions.md, entry 81), how many sheets there are, its neighbors and the
    /// first and last sheets in that order.
    pub index: usize,
    pub count: usize,
    pub prev: Option<u32>,
    pub next: Option<u32>,
    pub first: Option<u32>,
    pub last: Option<u32>,
    /// The stepping order: `board`, `number` or `scene`.
    pub order: &'static str,
    /// The header fields in the order the tab lays them out (`Schedule::sheet_fields`).
    pub field_order: Vec<&'static str>,
    /// The fields the tab hides, and the widths set, `[key, narrow|medium|wide]`
    /// (the breakdown designer, decisions.md, entry 101).
    pub hidden_fields: Vec<&'static str>,
    pub field_widths: Vec<(&'static str, &'static str)>,
    /// How many shown fields each row holds (decision 186); empty: six, five and the rest.
    pub field_rows: Vec<u8>,
    /// The sheet's image paths (decisions.md, entry 77), as stored.
    pub images: Vec<String>,
    /// The elements on the sheet that have images, with their paths: the images pane
    /// beside the board shows them (decisions.md, entry 80).
    pub element_images: Vec<ElementImagesDto>,
    /// The red flag clashes on the date it shoots, on its sub-board of the board shown, in
    /// words, and their types (decisions.md, entry 84).
    pub red_flags: Vec<String>,
    pub red_flag_types: Vec<u32>,
    /// The conflicts on its strip, in words (decisions.md, entry 83).
    pub conflicts: Vec<String>,
    /// The elements on the sheet, by category in category order; empty categories left
    /// out.
    pub groups: Vec<GroupDto>,
    /// Every category, for quick add's category field.
    pub categories: Vec<NamedDto>,
    /// Every element, for quick add's matches.
    pub elements: Vec<ElementDto>,
    /// The values used on any sheet, for each one-value header field by its key (`set`,
    /// `location`, `int_ext` and the rest of `reports::HeaderField`), offered as the field
    /// is typed in (decisions.md, entry 155). Values differing only in case are one,
    /// spelled as first met.
    pub header_values: BTreeMap<&'static str, Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementImagesDto {
    pub element: u32,
    pub name: String,
    pub category: String,
    pub images: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupDto {
    pub category: String,
    pub elements: Vec<ElementDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NamedDto {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementDto {
    pub id: u32,
    pub name: String,
    pub board_id: String,
    pub category: String,
    /// On a breakdown sheet: how many the sheet needs, when not one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    /// On a breakdown sheet: whether that quantity counts in the daily total, when it
    /// doesn't.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily: Option<bool>,
}

fn distinct(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = values.filter(|x| !x.trim().is_empty()).collect();
    v.sort_by_key(|x| x.to_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// Where a sheet's strip is on board `b`, in words, and the date it shoots on, if any.
fn place_and_date(
    s: &Schedule,
    b: &Board,
    id: method_schedule_core::model::SheetId,
) -> (String, Option<Date>) {
    let Some(place) = s.sheet_place(b.id, id) else {
        return (String::new(), None);
    };
    let Some(sb) = place
        .sub_board
        .and_then(|sub| b.sub_boards.iter().find(|x| x.id == sub))
    else {
        return ("Boneyard".to_string(), None);
    };
    let day = days(s, b.id, sb.id).ok().and_then(|t| {
        t.days
            .into_iter()
            .find(|d| d.strips.contains(&sb.strips[place.index].id))
    });
    match day {
        None => (format!("{}, not scheduled", sb.name), None),
        Some(d) => match sb
            .start
            .and_then(|st| sb.calendar.date_of_day(st, d.number))
        {
            Some(date) => (
                format!("{}, day {}, {} {date}", sb.name, d.number, weekday(date)),
                Some(date),
            ),
            None => (format!("{}, day {}", sb.name, d.number), None),
        },
    }
}

/// The Breakdown tab's sheet: `sheet` when it is still there, else the one now at `index`
/// in `board`'s order (the last when past the end), else the first. `None` when there are
/// no sheets.
pub fn sheet_view(
    s: &Schedule,
    board: u32,
    sheet: Option<u32>,
    index: Option<usize>,
) -> Result<Option<SheetDto>, String> {
    sheet_view_in(s, board, sheet, index, SheetOrder::Board)
}

/// [`sheet_view`] with the sheets stepped through in `by`: board order, sheet number or
/// scene number (decisions.md, entry 81); `index` counts in that order.
pub fn sheet_view_in(
    s: &Schedule,
    board: u32,
    sheet: Option<u32>,
    index: Option<usize>,
    by: SheetOrder,
) -> Result<Option<SheetDto>, String> {
    let b = board_ref(s, board)?;
    let order = s.sheets_in_order(b.id, by);
    if order.is_empty() {
        return Ok(None);
    }
    let at = sheet
        .and_then(|id| order.iter().position(|x| x.0 == id))
        .unwrap_or_else(|| index.unwrap_or(0).min(order.len() - 1));
    let h = s.sheet(order[at]).ok_or("missing sheet")?;
    let element = |id: &method_schedule_core::model::ElementId| {
        s.element(*id).map(|e| {
            let (n, daily) = h.quantity(e.id);
            ElementDto {
                id: e.id.0,
                name: e.name.clone(),
                board_id: e.board_id.clone().unwrap_or_default(),
                category: s
                    .category(e.category)
                    .map(|c| c.name.clone())
                    .unwrap_or_default(),
                quantity: (n != 1 || !daily).then_some(n),
                daily: (!daily).then_some(false),
            }
        })
    };
    let groups = s
        .categories
        .iter()
        .filter_map(|c| {
            let mut els: Vec<(Option<String>, ElementDto)> = h
                .elements
                .iter()
                .filter_map(|id| s.element(*id).filter(|e| e.category == c.id))
                .filter_map(|e| Some((e.board_id.clone(), element(&e.id)?)))
                .collect();
            if els.is_empty() {
                return None;
            }
            els.sort_by(|a, b| {
                id_order(a.0.as_deref(), b.0.as_deref()).then(a.1.name.cmp(&b.1.name))
            });
            Some(GroupDto {
                category: c.name.clone(),
                elements: els.into_iter().map(|x| x.1).collect(),
            })
        })
        .collect();
    let mut elements: Vec<ElementDto> = s.elements.iter().filter_map(|e| element(&e.id)).collect();
    elements.sort_by_key(|a| a.name.to_lowercase());
    let place = place_and_date(s, b, h.id);
    let strip_place = s.sheet_place(b.id, h.id);
    let (red_flags, red_flag_types) = strip_place
        .and_then(|p| p.sub_board)
        .and_then(|sub| b.sub_boards.iter().find(|x| x.id == sub))
        .map(|sb| crate::redflags::on_sheet(s, b, sb, h.id))
        .unwrap_or_default();
    let sheet_conflicts = strip_place
        .and_then(|p| {
            let sb = b.sub_boards.iter().find(|x| Some(x.id) == p.sub_board)?;
            sb.strips.get(p.index).map(|st| st.id.0)
        })
        .and_then(|st| crate::conflictmarks::marks(s, b.id).strips.remove(&st))
        .unwrap_or_default();
    let element_images = h
        .elements
        .iter()
        .filter_map(|id| s.element(*id))
        .filter(|e| !e.images.is_empty())
        .map(|e| ElementImagesDto {
            element: e.id.0,
            name: e.name.clone(),
            category: s
                .category(e.category)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            images: e.images.clone(),
        })
        .collect();
    Ok(Some(SheetDto {
        id: h.id.0,
        number: h.number.clone(),
        scenes: h.scenes.clone(),
        int_ext: h.int_ext.clone(),
        set: h.set.clone(),
        time_of_day: h.time_of_day.clone(),
        sunrise: h.sunrise.map(crate::sheet::show_clock).unwrap_or_default(),
        sunset: h.sunset.map(crate::sheet::show_clock).unwrap_or_default(),
        pages_whole: h.pages.pages().to_string(),
        pages_eighths: h.pages.remainder().to_string(),
        pages: h.pages.to_string(),
        shoot_time: h
            .shoot_time
            .map(crate::sheet::show_minutes)
            .unwrap_or_default(),
        shoot_time_decimal: h
            .shoot_time
            .map(crate::sheet::show_decimal_hours)
            .unwrap_or_default(),
        screen_time: h
            .screen_time
            .map(crate::sheet::show_seconds)
            .unwrap_or_default(),
        synopsis: h.synopsis.clone(),
        location: h.location.clone(),
        script_pages: h.script_pages.clone(),
        script_day: h.script_day.clone(),
        unit: h.unit.clone(),
        sequence: h.sequence.clone(),
        episode: h.episode.clone(),
        comments: h.comments.clone(),
        place: place.0,
        date: place.1.map(|d| d.to_string()),
        strip: s.sheet_place(b.id, h.id).and_then(|p| {
            let list = match p.sub_board {
                None => &b.boneyard,
                Some(sub) => &b.sub_boards.iter().find(|x| x.id == sub)?.strips,
            };
            list.get(p.index).map(|st| st.id.0)
        }),
        strip_sub_board: s
            .sheet_place(b.id, h.id)
            .and_then(|p| p.sub_board.map(|x| x.0)),
        index: at,
        count: order.len(),
        prev: at.checked_sub(1).map(|i| order[i].0),
        next: order.get(at + 1).map(|x| x.0),
        first: order.first().map(|x| x.0),
        last: order.last().map(|x| x.0),
        order: match by {
            SheetOrder::Board => "board",
            SheetOrder::Number => "number",
            SheetOrder::Scene => "scene",
        },
        field_order: s.sheet_field_order(),
        hidden_fields: s.sheet_looks.hidden.clone(),
        field_widths: s
            .sheet_looks
            .widths
            .iter()
            .map(|(k, w)| (*k, w.name()))
            .collect(),
        field_rows: s.sheet_looks.rows.clone(),
        images: h.images.clone(),
        element_images,
        red_flags,
        red_flag_types,
        conflicts: sheet_conflicts,
        groups,
        categories: s
            .categories
            .iter()
            .map(|c| NamedDto {
                id: c.id.0,
                name: c.name.clone(),
            })
            .collect(),
        elements,
        header_values: HeaderField::ALL
            .iter()
            .map(|f| {
                let v = distinct(s.sheets.iter().map(|x| f.value(x).to_string()));
                (f.key(), v)
            })
            .collect(),
    }))
}

/// The Elements tab: every category with its settings, and one category's elements with
/// their counts and dates on one sub-board. A header field (`reports::HeaderField`, set
/// to sequence) can be shown in place of a category: its values, read only.
#[derive(Clone, Debug, Serialize)]
pub struct ElementsDto {
    pub categories: Vec<CategoryDto>,
    /// The category shown: the one asked for while it exists, else the first. `None`
    /// while a header field is shown.
    pub category: Option<u32>,
    /// The header fields that work as categories (inventory section 4), with how many
    /// values each takes.
    pub headers: Vec<HeaderDto>,
    /// The header field shown instead of a category (`set`, `location`, `unit`,
    /// `episode`), if any. Its rows are values, never edited here.
    pub header: Option<&'static str>,
    pub elements: Vec<ElementRowDto>,
    /// Every element, by category in category order, then board ID and name: what the
    /// linked elements editor offers.
    pub all_elements: Vec<ElementDto>,
    /// The sub-board work days and dates come from.
    pub sub_board: String,
    /// Why there are no work days or dates (no start date yet), if so.
    pub dates_note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeaderDto {
    pub key: &'static str,
    pub label: &'static str,
    /// How many different values the field takes across the sheets.
    pub values: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CategoryDto {
    pub id: u32,
    pub name: String,
    /// Can't be renamed, moved or deleted.
    pub built_in: bool,
    /// Its place among the user categories (from 0), `None` for a built-in one.
    pub place: Option<usize>,
    /// How many user categories there are (to tell first from last).
    pub user_count: usize,
    pub auto_board_id: bool,
    /// Left off the board: no IDs on strips, no conflicts.
    pub exclude_from_board: bool,
    /// Quantities add up in the daily category totals.
    pub daily_quantity: bool,
    /// Custom element properties, in order.
    pub properties: Vec<String>,
    /// The ones whose values take several lines (decision 119).
    pub multi_line: Vec<String>,
    /// The defaults new elements get.
    pub holds: bool,
    pub drops: bool,
    pub min_drop_gap: String,
    /// How many elements it holds.
    pub elements: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementRowDto {
    pub id: u32,
    pub board_id: String,
    pub locked: bool,
    /// Left off the board on its own (its category may leave it off too; decision 105).
    pub exclude_from_board: bool,
    pub name: String,
    /// Sheets it is on (occurrences), and their pages.
    pub sheets: u32,
    pub pages: String,
    /// Work days on the sub-board, and the first and last (`YYYY-MM-DD`).
    pub days: u32,
    pub start: Option<String>,
    pub finish: Option<String>,
    /// Events on the sub-board that list it.
    pub events: u32,
    /// Conflicts on the sub-board's board that involve it on this sub-board: dates it is
    /// booked in another sub-board too, and events on its work days (decisions 28 and 43).
    pub conflicts: u32,
    /// Red flag clashes on the sub-board: dates it works, or is at a counted event, that a
    /// flag of its rules out (decisions.md, entry 84).
    pub clashes: u32,
    pub holds: bool,
    pub drops: bool,
    pub min_drop_gap: String,
    pub notes: String,
    /// Every property of its category, in order, with its value (empty when none).
    pub properties: Vec<PropertyDto>,
    /// The elements it links to (adding it to a sheet adds them), by id, and in words.
    pub links: Vec<u32>,
    pub link_names: Vec<String>,
    /// The elements that link to it, in words.
    pub linked_by: Vec<String>,
    /// Linked to other elements either way (deleting asks to unlink first).
    pub linked: bool,
    /// Its image paths as stored, in order.
    pub images: Vec<String>,
    /// Its red flags, in the order added.
    pub red_flags: Vec<crate::redflags::RedFlagDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PropertyDto {
    pub name: String,
    pub value: String,
    /// The property takes several lines (decision 119).
    pub multi_line: bool,
}

/// An element as `ID NAME`, or its name.
fn id_name(e: &method_schedule_core::model::Element) -> String {
    match &e.board_id {
        Some(b) => format!("{b} {}", e.name),
        None => e.name.clone(),
    }
}

/// Every element by category in category order, then board ID and name.
fn elements_by_category(s: &Schedule) -> Vec<ElementDto> {
    let mut out = Vec::new();
    for c in &s.categories {
        let mut els: Vec<_> = s.elements.iter().filter(|e| e.category == c.id).collect();
        els.sort_by(|a, b| {
            id_order(a.board_id.as_deref(), b.board_id.as_deref()).then_with(|| {
                a.name
                    .to_ascii_uppercase()
                    .cmp(&b.name.to_ascii_uppercase())
            })
        });
        out.extend(els.into_iter().map(|e| ElementDto {
            id: e.id.0,
            name: e.name.clone(),
            board_id: e.board_id.clone().unwrap_or_default(),
            category: c.name.clone(),
            quantity: None,
            daily: None,
        }));
    }
    out
}

/// The Elements tab for `category` (the first category when it is gone or not given), or
/// for header field `header` when one is named, with work days and dates from sub-board
/// `sub`.
pub fn elements_view(
    s: &Schedule,
    sub: u32,
    category: Option<u32>,
    header: Option<&str>,
) -> Result<ElementsDto, String> {
    use method_schedule_core::reports::header_category;
    let (b, sb) = sub_ref(s, sub)?;
    let user_count = s.categories.iter().filter(|c| !c.is_built_in()).count();
    let mut place = 0;
    let categories = s
        .categories
        .iter()
        .map(|c| {
            let built_in = c.is_built_in();
            let at = (!built_in).then(|| {
                place += 1;
                place - 1
            });
            CategoryDto {
                id: c.id.0,
                name: c.name.clone(),
                built_in,
                place: at,
                user_count,
                auto_board_id: c.auto_board_id,
                exclude_from_board: c.exclude_from_board,
                daily_quantity: c.daily_quantity,
                properties: c.properties.clone(),
                multi_line: c.multi_line.clone(),
                holds: c.defaults.holds,
                drops: c.defaults.drops,
                min_drop_gap: c.defaults.min_drop_gap.to_string(),
                elements: s.elements.iter().filter(|e| e.category == c.id).count(),
            }
        })
        .collect();
    let headers = HeaderField::ALL
        .iter()
        .map(|f| HeaderDto {
            key: f.key(),
            label: f.label(),
            values: distinct(s.sheets.iter().map(|h| f.value(h).to_string())).len(),
        })
        .collect();
    let dates_note = sb.start.is_none().then(|| {
        format!(
            "Set a start date for {} to see work days and dates.",
            sb.name
        )
    });
    let row = |l: &method_schedule_core::reports::ElementLine| ElementRowDto {
        id: l.element.0,
        board_id: l.board_id.clone().unwrap_or_default(),
        locked: false,
        exclude_from_board: false,
        name: l.name.clone(),
        sheets: l.sheets,
        pages: l.pages.to_string(),
        days: l.days,
        start: l.start.map(|d| d.to_string()),
        finish: l.finish.map(|d| d.to_string()),
        events: 0,
        conflicts: 0,
        clashes: 0,
        holds: false,
        drops: false,
        min_drop_gap: String::new(),
        notes: String::new(),
        properties: Vec::new(),
        links: Vec::new(),
        link_names: Vec::new(),
        linked_by: Vec::new(),
        linked: false,
        images: Vec::new(),
        red_flags: Vec::new(),
    };
    if let Some(field) = header.and_then(HeaderField::from_key) {
        let hc = header_category(s, field);
        let elements = method_schedule_core::reports::element_list(&hc.schedule, b.id, sb.id)
            .map_err(|e| e.to_string())?
            .iter()
            .filter(|l| l.category == hc.category)
            .map(row)
            .collect();
        return Ok(ElementsDto {
            categories,
            category: None,
            headers,
            header: Some(field.key()),
            elements,
            all_elements: Vec::new(),
            sub_board: sb.name.clone(),
            dates_note,
        });
    }
    let chosen = category
        .and_then(|id| s.categories.iter().find(|c| c.id.0 == id))
        .or_else(|| s.categories.first())
        .map(|c| c.id);
    let found = method_schedule_core::conflicts::conflicts(s, b.id).ok();
    let clashes = crate::redflags::count_by_element(s, b, sb);
    let mut elements = Vec::new();
    if let Some(cat) = chosen {
        let category = s.category(cat).ok_or("missing category")?;
        let (props, multi) = (&category.properties, &category.multi_line);
        for l in method_schedule_core::reports::element_list(s, b.id, sb.id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|l| l.category == cat)
        {
            let e = s.element(l.element).ok_or("missing element")?;
            let mut r = row(&l);
            r.locked = e.board_id_locked;
            r.exclude_from_board = e.exclude_from_board;
            r.events = sb
                .calendar
                .events
                .iter()
                .filter(|ev| ev.elements.contains(&e.id))
                .count() as u32;
            r.conflicts = found.as_ref().map_or(0, |f| {
                (f.conflicts
                    .iter()
                    .filter(|c| {
                        c.element == e.id && c.bookings.iter().any(|x| x.sub_board == sb.id)
                    })
                    .count()
                    + f.event_on_work
                        .iter()
                        .filter(|c| c.element == e.id && c.sub_board == sb.id)
                        .count()) as u32
            });
            r.clashes = clashes.get(&e.id).copied().unwrap_or(0) as u32;
            r.holds = e.rules.holds;
            r.drops = e.rules.drops;
            r.min_drop_gap = e.rules.min_drop_gap.to_string();
            r.notes = e.notes.clone();
            r.properties = props
                .iter()
                .map(|p| PropertyDto {
                    name: p.clone(),
                    value: e
                        .properties
                        .iter()
                        .find(|v| v.name == *p)
                        .map(|v| v.value.clone())
                        .unwrap_or_default(),
                    multi_line: multi.contains(p),
                })
                .collect();
            r.links = e.links.iter().map(|x| x.0).collect();
            r.link_names = e
                .links
                .iter()
                .filter_map(|x| s.element(*x))
                .map(id_name)
                .collect();
            r.linked_by = s
                .elements
                .iter()
                .filter(|o| o.links.contains(&e.id))
                .map(id_name)
                .collect();
            r.linked = s.is_linked(e.id);
            r.images = e.images.clone();
            r.red_flags = crate::redflags::flags_of(s, e.id);
            elements.push(r);
        }
    }
    Ok(ElementsDto {
        categories,
        category: chosen.map(|c| c.0),
        headers,
        header: None,
        elements,
        all_elements: elements_by_category(s),
        sub_board: sb.name.clone(),
        dates_note,
    })
}

/// One element's days and dates on a sub-board (inventory section 4): every date of its
/// day out of days row, with the code and the shoot day.
#[derive(Clone, Debug, Serialize)]
pub struct ElementDaysDto {
    pub element: u32,
    pub name: String,
    pub sub_board: String,
    pub days: Vec<ElementDayDto>,
    /// Why there are no dates, when there are none.
    pub empty: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementDayDto {
    pub date: String,
    pub weekday: &'static str,
    /// The shoot day number on this date, if it is one.
    pub shoot_day: Option<u32>,
    /// The day out of days code (`SW`, `H`, `FT`, ...), empty when the day counts nowhere.
    pub code: String,
    /// What the code means, in words.
    pub text: &'static str,
}

/// Element `element`'s days and dates on sub-board `sub`.
pub fn element_days(s: &Schedule, sub: u32, element: u32) -> Result<ElementDaysDto, String> {
    use method_schedule_core::dood::Cell;
    let (b, sb) = sub_ref(s, sub)?;
    let e = s
        .element(method_schedule_core::model::ElementId(element))
        .ok_or("That element is no longer in the schedule.")?;
    let mut out = ElementDaysDto {
        element,
        name: id_name(e),
        sub_board: sb.name.clone(),
        days: Vec::new(),
        empty: None,
    };
    let Some(start) = sb.start else {
        out.empty = Some(format!(
            "Set a start date for {} to see its days and dates.",
            sb.name
        ));
        return Ok(out);
    };
    let rows = method_schedule_core::dood::dood(s, b.id, sb.id).map_err(|e| e.to_string())?;
    let Some(r) = rows.into_iter().find(|r| r.element == e.id) else {
        out.empty = Some(format!(
            "{} has no work days or events on {}.",
            e.name, sb.name
        ));
        return Ok(out);
    };
    let total = days(s, b.id, sb.id).map_err(|e| e.to_string())?.days.len() as u32;
    let mut numbers = std::collections::HashMap::new();
    let (mut d, mut n) = (start, 0);
    let last = r.cells.last().map_or(start, |c| c.0);
    while n < total && d <= last {
        if sb.calendar.is_shoot_day(d) {
            n += 1;
            numbers.insert(d, n);
        }
        d = d.add_days(1);
    }
    for (date, cell) in &r.cells {
        let text = match cell {
            Cell::Work { .. } => "works",
            Cell::EventOnWork { .. } => "an event on a work day (a conflict)",
            Cell::Hold => "held",
            Cell::Event { held: true, .. } => "an event on a held day",
            Cell::Event { .. } => "an event",
            Cell::Holiday => "a holiday that counts",
            Cell::Move => "a company travel day that counts",
            Cell::Idle => "counts nowhere",
        };
        out.days.push(ElementDayDto {
            date: date.to_string(),
            weekday: weekday(*date),
            shoot_day: numbers.get(date).copied(),
            code: cell.code(),
            text,
        });
    }
    Ok(out)
}

/// The Calendar tab: one month of one sub-board, Sunday to Saturday,
/// with what each date is, its shoot day, and the events on it.
#[derive(Clone, Debug, Serialize)]
pub struct CalendarDto {
    pub sub_board: String,
    /// The board holding the sub-board, for dragging strips (`move_strips`).
    pub board: u32,
    pub start: Option<String>,
    /// The weekday headings, in the user's week (`Sun` or `Mon` first).
    pub week: Vec<&'static str>,
    /// The fields each strip's `cells` follow (decision 102), by name.
    pub fields: Vec<&'static str>,
    /// Today's date, `YYYY-MM-DD` (UTC).
    pub today: String,
    /// The sub-board's prep start and wrap end, for the budget exchange (decision 59).
    pub prep_start: Option<String>,
    pub wrap_end: Option<String>,
    pub year: i32,
    pub month: u32,
    /// `March 2027`.
    pub title: String,
    /// The weekly days off, `mon` to `sun`.
    pub weekly_off: Vec<&'static str>,
    /// Six weeks of seven dates, Sunday first; dates outside the month are included. In
    /// the weeks view (`span`), every week of the schedule's span, all `in_month`.
    pub weeks: Vec<Vec<CalendarDayDto>>,
    /// The continuous weeks view rather than one month (decision 111).
    pub span: bool,
    /// In the weeks view, the events more than 26 weeks from the shoot, left out of the
    /// span (decision 135); 0 in the month view.
    pub outside_events: u32,
    /// What the Calendar hides (its View menu, kept in the schedule; decision 111), by
    /// key, and every switch the menu offers, as key and words.
    pub hidden: Vec<&'static str>,
    pub shows: Vec<(&'static str, &'static str)>,
    /// Every event on the sub-board, in the calendar's order (`index` names it in
    /// `save_event` and `delete_event`).
    pub events: Vec<EventDto>,
    /// Every element, by category in category order, then board ID and name: what an
    /// event can list.
    pub elements: Vec<ElementDto>,
    /// The first and last shoot day's dates, when there is a start date and a day break.
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    /// The month before and after, as `[year, month]`.
    pub prev: (i32, u32),
    pub next: (i32, u32),
    /// Every event kind the editor offers, as key and words, in order (decision 109).
    pub event_kinds: Vec<(&'static str, &'static str)>,
    /// The ten event colors, as key and name (decision 109).
    pub event_colors: Vec<(&'static str, &'static str)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalendarDayDto {
    pub date: String,
    pub day: u32,
    pub weekday: &'static str,
    /// In the month shown (the grid shows a few days of the months around it).
    pub in_month: bool,
    /// `work`, `off`, `holiday` or `move`.
    pub kind: &'static str,
    /// Falls on a weekly day off (so `work` here means marked back to working).
    pub weekly_off: bool,
    /// The shoot day number, when a shoot day on the board falls on it.
    pub shoot_day: Option<u32>,
    /// Indexes into `events` of the events covering it.
    pub events: Vec<usize>,
    /// The red flag clashes on it, in words. Left out when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red_flags: Vec<String>,
    /// Their types, whose colors paint the mark (decisions.md, entry 84).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub red_flag_types: Vec<u32>,
    /// The conflicts on it, in words (decisions.md, entry 83).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
    /// A shoot day's scene strips, in board order, with the user's calendar fields.
    pub strips: Vec<CalendarStripDto>,
    /// A shoot day's pages and estimated length (`h:mm`).
    pub pages: String,
    pub time: String,
    /// How many scenes the shoot day holds (whatever the filter shows).
    pub scenes: u32,
    /// The shoot day's banners, by their text, in board order (decision 111).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub banners: Vec<String>,
    /// A shoot day's day break, as a place in the sub-board's strips: a strip dropped on
    /// the date moves there, ending the day (`move_strips`, decision 59).
    pub drop_at: Option<usize>,
}

/// A scene strip on a calendar date.
#[derive(Clone, Debug, Serialize)]
pub struct CalendarStripDto {
    pub strip: u32,
    pub sheet: u32,
    /// The calendar strip's fields (`Schedule::calendar_fields`), in order.
    pub cells: Vec<String>,
    pub tone: &'static str,
    /// The sheet's elements, for the calendar's filter.
    pub elements: Vec<u32>,
}

/// How [`calendar_view_with`] draws a month: the week's first day and the strips' fields.
#[derive(Clone, Debug)]
pub struct CalOpts {
    /// Monday first rather than Sunday.
    pub monday: bool,
    pub fields: Vec<method_schedule_core::model::StripField>,
    /// The continuous weeks view (decision 111): every week of the shoot (start to last
    /// shoot day) and the month asked for, widened by events within 26 weeks of it
    /// (decision 135), rather than the six weeks of one month.
    pub span: bool,
}

/// The most weeks the continuous weeks view draws (two years and a little).
pub const MAX_SPAN_WEEKS: i32 = 110;

/// How far from the shoot an event may be and still widen the weeks view (decision 135).
pub const SPAN_NEAR_WEEKS: i32 = 26;

impl Default for CalOpts {
    fn default() -> CalOpts {
        CalOpts {
            monday: false,
            fields: method_schedule_core::model::DEFAULT_CALENDAR_FIELDS.to_vec(),
            span: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EventDto {
    pub index: usize,
    /// `travel`, `fitting`, `rehearsal` or `other`.
    pub kind: &'static str,
    /// For `other`: what kind it is.
    pub label: String,
    pub name: String,
    pub from: String,
    pub to: String,
    pub elements: Vec<u32>,
    /// The elements as the page lists them: `ID NAME`, or the name.
    pub element_names: Vec<String>,
    /// Counts in the day out of days (travel, fitting, rehearsal).
    pub counts: bool,
    /// One of the ten event colors by key, or none (decision 109).
    pub color: Option<&'static str>,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Today in UTC, on the clock the host installed (src/clock.rs): the Calendar tab's
/// month when the sub-board has no start date.
fn today() -> Date {
    Date((crate::clock::wall_ms() / 86_400_000) as i32)
}

/// The Calendar tab of sub-board `sub` for `year` and `month` (1 to 12); without them, the
/// month of the start date, else this month.
pub fn calendar_view(
    s: &Schedule,
    sub: u32,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<CalendarDto, String> {
    calendar_view_with(s, sub, year, month, &CalOpts::default())
}

/// [`calendar_view`] with the user's week start and calendar strip fields.
pub fn calendar_view_with(
    s: &Schedule,
    sub: u32,
    year: Option<i32>,
    month: Option<u32>,
    opts: &CalOpts,
) -> Result<CalendarDto, String> {
    let (b, sb) = sub_ref(s, sub)?;
    let cal = &sb.calendar;
    let (year, month) = match (year, month) {
        (Some(y), Some(m)) if (1..=12).contains(&m) && (1..=9999).contains(&y) => (y, m),
        (None, None) => {
            let (y, m, _) = sb.start.unwrap_or_else(today).ymd();
            (y, m)
        }
        _ => return Err("That is not a month.".to_string()),
    };
    let first = Date::from_ymd(year, month, 1).ok_or("That is not a month.")?;
    // Back to the week's first day (Sunday, or Monday) on or before the 1st.
    let monday_based = Weekday::ALL
        .iter()
        .position(|w| *w == first.weekday())
        .map_or(0, |i| i as i32);
    let back = if opts.monday {
        monday_based
    } else {
        (monday_based + 1) % 7
    };
    let mut grid_start = first.add_days(-back);
    let mut week_count = 6;
    // Each shoot day's strips and its day break's place, by date.
    let shoot = days(s, b.id, sb.id).map_err(|e| e.to_string())?;
    let cast = s.cast();
    let mut by_number: std::collections::HashMap<u32, (&method_schedule_core::days::Day, usize)> =
        std::collections::HashMap::new();
    for d in &shoot.days {
        if let Some(at) = sb.strips.iter().position(|x| x.id == d.break_id) {
            by_number.insert(d.number, (d, at));
        }
    }

    // Shoot day numbers by date.
    let shoot_days = shoot.days.len() as u32;
    let mut numbers = std::collections::HashMap::new();
    let (mut first_day, mut last_day) = (None, None);
    if let Some(start) = sb.start {
        if Weekday::ALL.iter().any(|w| !cal.weekly_off.contains(w)) {
            let mut d = start;
            let mut n = 0;
            while n < shoot_days {
                if cal.is_shoot_day(d) {
                    n += 1;
                    numbers.insert(d, n);
                    first_day.get_or_insert(d);
                    last_day = Some(d);
                }
                d = d.add_days(1);
            }
        }
    }
    // The continuous weeks view (decisions 111 and 135): the shoot's span (its start and
    // shoot days) and the month asked for, then any event within SPAN_NEAR_WEEKS of that.
    // An event farther away (a mistyped year) is counted in `outside_events` rather than
    // moving the span, so the shoot days are always drawn.
    let mut outside_events = 0;
    if opts.span {
        let month_end = Date::from_ymd(
            shift_month(year, month, 1).0,
            shift_month(year, month, 1).1,
            1,
        )
        .map_or(first, |d| d.add_days(-1));
        let shoot_lo = [sb.start, first_day].into_iter().flatten().min();
        let shoot_hi = [sb.start, last_day].into_iter().flatten().max();
        let (mut core_lo, mut core_hi) = (
            shoot_lo.map_or(first, |d| d.min(first)),
            shoot_hi.map_or(month_end, |d| d.max(month_end)),
        );
        // The month asked for joins only when the whole still fits.
        if let (Some(a), Some(b)) = (shoot_lo, shoot_hi) {
            if (core_hi.0 - core_lo.0) / 7 + 2 > MAX_SPAN_WEEKS {
                (core_lo, core_hi) = (a, b);
            }
        }
        let near = SPAN_NEAR_WEEKS * 7;
        let (mut lo, mut hi) = (core_lo, core_hi);
        for e in &cal.events {
            if e.to.0 < core_lo.0 - near || e.from.0 > core_hi.0 + near {
                outside_events += 1;
            } else {
                lo = lo.min(e.from.max(Date(core_lo.0 - near)));
                hi = hi.max(e.to.min(Date(core_hi.0 + near)));
            }
        }
        let lo_back = {
            let i = Weekday::ALL
                .iter()
                .position(|w| *w == lo.weekday())
                .map_or(0, |i| i as i32);
            if opts.monday {
                i
            } else {
                (i + 1) % 7
            }
        };
        grid_start = lo.add_days(-lo_back);
        week_count = ((hi.0 - grid_start.0) / 7 + 1).clamp(6, MAX_SPAN_WEEKS);
    }
    let element_text = |id: &method_schedule_core::model::ElementId| {
        s.element(*id).map(|e| match &e.board_id {
            Some(b) => format!("{b} {}", e.name),
            None => e.name.clone(),
        })
    };
    let events: Vec<EventDto> = cal
        .events
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let kind = ev.kind.key();
            let label = match &ev.kind {
                method_schedule_core::model::EventKind::Other(l) => l.clone(),
                _ => String::new(),
            };
            EventDto {
                index: i,
                kind,
                label,
                name: ev.name.clone(),
                from: ev.from.to_string(),
                to: ev.to.to_string(),
                elements: ev.elements.iter().map(|e| e.0).collect(),
                element_names: ev.elements.iter().filter_map(element_text).collect(),
                counts: ev.kind.code().is_some(),
                color: ev
                    .color
                    .and_then(|c| method_schedule_core::model::EVENT_COLORS.get(usize::from(c)))
                    .map(|(k, _, _)| *k),
            }
        })
        .collect();
    let flagged = crate::redflags::by_date(s, b, sb);
    let flag_kinds = crate::redflags::kinds_by_date(s, b, sb);
    let conflict_marks = crate::conflictmarks::marks(s, b.id);
    let mut weeks = Vec::new();
    for w in 0..week_count {
        let mut week = Vec::new();
        for d in 0..7 {
            let date = grid_start.add_days(w * 7 + d);
            let (_, m, day) = date.ymd();
            let shoot_day = numbers.get(&date).and_then(|n| by_number.get(n));
            week.push(CalendarDayDto {
                date: date.to_string(),
                day,
                weekday: weekday(date),
                in_month: opts.span || m == month,
                scenes: shoot_day.map_or(0, |(d, _)| {
                    d.strips
                        .iter()
                        .filter_map(|id| sb.strips.iter().find(|x| x.id == *id))
                        .filter(|st| matches!(st.kind, StripKind::Sheet(_)))
                        .count() as u32
                }),
                banners: shoot_day.map_or_else(Vec::new, |(d, _)| {
                    d.strips
                        .iter()
                        .filter_map(|id| sb.strips.iter().find(|x| x.id == *id))
                        .filter_map(|st| match &st.kind {
                            StripKind::Banner(b) => Some(b.text.clone()),
                            _ => None,
                        })
                        .collect()
                }),
                kind: match cal.day_kind(date) {
                    method_schedule_core::model::DayKind::Work => "work",
                    method_schedule_core::model::DayKind::Off => "off",
                    method_schedule_core::model::DayKind::Holiday => "holiday",
                    method_schedule_core::model::DayKind::Move => "move",
                },
                weekly_off: cal.weekly_off.contains(&date.weekday()),
                shoot_day: numbers.get(&date).copied(),
                events: cal
                    .events
                    .iter()
                    .enumerate()
                    .filter(|(_, ev)| ev.from <= date && date <= ev.to)
                    .map(|(i, _)| i)
                    .collect(),
                red_flags: flagged.get(&date).cloned().unwrap_or_default(),
                red_flag_types: flag_kinds.get(&date).cloned().unwrap_or_default(),
                conflicts: conflict_marks
                    .dates
                    .get(&(sb.id.0, date))
                    .cloned()
                    .unwrap_or_default(),
                strips: numbers
                    .get(&date)
                    .and_then(|n| by_number.get(n))
                    .map_or_else(Vec::new, |(d, _)| {
                        d.strips
                            .iter()
                            .filter_map(|id| sb.strips.iter().find(|x| x.id == *id))
                            .filter_map(|st| match &st.kind {
                                StripKind::Sheet(h) => s.sheet(*h).map(|h| (st.id, h)),
                                _ => None,
                            })
                            .map(|(id, h)| CalendarStripDto {
                                strip: id.0,
                                sheet: h.id.0,
                                cells: opts
                                    .fields
                                    .iter()
                                    .map(|f| strip_cell(s, h, *f, cast))
                                    .collect(),
                                tone: tone(h),
                                elements: h.elements.iter().map(|e| e.0).collect(),
                            })
                            .collect()
                    }),
                pages: numbers
                    .get(&date)
                    .and_then(|n| by_number.get(n))
                    .map_or(String::new(), |(d, _)| d.pages.to_string()),
                time: numbers
                    .get(&date)
                    .and_then(|n| by_number.get(n))
                    .map_or(String::new(), |(d, _)| hm(d.time)),
                drop_at: numbers
                    .get(&date)
                    .and_then(|n| by_number.get(n))
                    .map(|(_, at)| *at),
            });
        }
        weeks.push(week);
    }
    let elements = elements_by_category(s);
    let shift = shift_month;
    let names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let title = if opts.span {
        let (a, b) = (
            weeks
                .first()
                .and_then(|w| w.first())
                .map(|d| d.date.clone()),
            weeks.last().and_then(|w| w.last()).map(|d| d.date.clone()),
        );
        let words = |iso: Option<String>| {
            iso.and_then(|t| t.parse::<Date>().ok())
                .map_or(String::new(), |d| {
                    let (y, m, _) = d.ymd();
                    format!("{} {y}", MONTHS[m as usize - 1])
                })
        };
        format!("{} to {}", words(a), words(b))
    } else {
        format!("{} {year}", MONTHS[month as usize - 1])
    };
    Ok(CalendarDto {
        sub_board: sb.name.clone(),
        board: b.id.0,
        start: sb.start.map(|d| d.to_string()),
        fields: opts.fields.iter().map(|f| f.name()).collect(),
        week: (0..7)
            .map(|i| names[(i + usize::from(opts.monday)) % 7])
            .collect(),
        today: today().to_string(),
        prep_start: cal.prep_start.map(|d| d.to_string()),
        wrap_end: cal.wrap_end.map(|d| d.to_string()),
        year,
        month,
        title,
        span: opts.span,
        outside_events,
        hidden: s.calendar_hidden.iter().map(|c| c.key()).collect(),
        shows: method_schedule_core::model::CalendarShow::ALL
            .iter()
            .map(|c| (c.key(), c.label()))
            .collect(),
        weekly_off: Weekday::ALL
            .iter()
            .filter(|w| cal.weekly_off.contains(w))
            .map(|w| crate::calendar::weekday_name(*w))
            .collect(),
        weeks,
        events,
        elements,
        first_day: first_day.map(|d| d.to_string()),
        last_day: last_day.map(|d| d.to_string()),
        prev: shift(year, month, -1),
        next: shift(year, month, 1),
        event_kinds: method_schedule_core::model::EventKind::keys()
            .into_iter()
            .map(|k| (k, kind_words(k)))
            .collect(),
        event_colors: method_schedule_core::model::EVENT_COLORS
            .iter()
            .map(|(k, n, _)| (*k, *n))
            .collect(),
    })
}

/// `year` and `month` moved by `by` months.
fn shift_month(year: i32, month: u32, by: i32) -> (i32, u32) {
    let n = year * 12 + month as i32 - 1 + by;
    (n.div_euclid(12), n.rem_euclid(12) as u32 + 1)
}

/// An event kind's key in words, for the editor's list.
fn kind_words(key: &'static str) -> &'static str {
    use method_schedule_core::model::PostKind;
    match key {
        "travel" => "Travel",
        "fitting" => "Fitting",
        "rehearsal" => "Rehearsal",
        "unavailable" => "Unavailable",
        "camera_test" => "Camera test",
        "other" => "Other",
        k => PostKind::from_key(k).map_or("Other", PostKind::label),
    }
}

fn month_name(m: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES[(m as usize).saturating_sub(1) % 12]
}

/// The day out of days grid of sub-board `sub` for `category` (the cast, else the first
/// category, when it is gone or not given). Rows come in the order
/// `method_schedule_core::dood::dood` returns them; codes are `Cell::code()`.
pub fn dood_grid(s: &Schedule, sub: u32, category: Option<u32>) -> Result<DoodGridDto, String> {
    dood_grid_for(s, sub, category, None, &GridOpts::default())
}

/// [`dood_grid`] with options: without holds, sorted, a date range, chosen elements.
pub fn dood_grid_with(
    s: &Schedule,
    sub: u32,
    category: Option<u32>,
    opts: &GridOpts,
) -> Result<DoodGridDto, String> {
    dood_grid_for(s, sub, category, None, opts)
}

/// The day out of days grid as [`dood_grid_with`], or for header field `header` (`set`,
/// `location`, `unit`, `episode`) when one is named: a row per value it takes, worked
/// out from the sheets (`reports::header_category`).
pub fn dood_grid_for(
    s: &Schedule,
    sub: u32,
    category: Option<u32>,
    header: Option<&str>,
    opts: &GridOpts,
) -> Result<DoodGridDto, String> {
    use method_schedule_core::reports::header_category;
    if let Some(field) = header.and_then(HeaderField::from_key) {
        let hc = header_category(s, field);
        // The header's rows are made-up elements: an element filter can't name them.
        let header_opts = GridOpts {
            elements: Vec::new(),
            ..opts.clone()
        };
        let mut g = dood_grid_for(&hc.schedule, sub, Some(hc.category.0), None, &header_opts)?;
        let own = dood_grid_for(s, sub, category, None, opts)?;
        g.categories = own.categories;
        g.headers = own.headers;
        g.category = None;
        g.header = Some(field.key());
        g.codes = own.codes;
        if g.rows.is_empty() && sb_has_start(s, sub) {
            g.empty = Some(format!(
                "No {} is on a scheduled sheet yet.",
                field.label().to_lowercase()
            ));
        }
        return Ok(g);
    }
    let (b, sb) = sub_ref(s, sub)?;
    let chosen = category
        .and_then(|id| s.categories.iter().find(|c| c.id.0 == id))
        .or_else(|| s.cast().and_then(|id| s.category(id)))
        .or_else(|| s.categories.first());
    let mut out = DoodGridDto {
        sub_board: sb.name.clone(),
        start: sb.start.map(|d| d.to_string()),
        categories: s
            .categories
            .iter()
            .map(|c| NamedDto {
                id: c.id.0,
                name: c.name.clone(),
            })
            .collect(),
        category: chosen.map(|c| c.id.0),
        headers: HeaderField::ALL
            .iter()
            .map(|f| HeaderDto {
                key: f.key(),
                label: f.label(),
                values: distinct(s.sheets.iter().map(|h| f.value(h).to_string())).len(),
            })
            .collect(),
        header: None,
        excluded: chosen.is_some_and(|c| {
            let mut els = s.elements.iter().filter(|e| e.category == c.id).peekable();
            els.peek().is_some() && els.all(|e| e.exclude_from_board)
        }),
        dates: Vec::new(),
        rows: Vec::new(),
        empty: None,
        codes: method_schedule_core::setup::DoodCodes::KEYS
            .iter()
            .zip(CODE_LABELS)
            .map(|(k, l)| (*k, l, s.dood_codes.get(k).unwrap_or_default().to_string()))
            .collect(),
        no_holds: opts.no_holds,
    };
    let Some(start) = sb.start else {
        out.empty = Some(format!(
            "Set a start date for {} to see the day out of days.",
            sb.name
        ));
        return Ok(out);
    };
    let Some(cat) = chosen else {
        out.empty = Some("There are no categories yet.".to_string());
        return Ok(out);
    };
    let mut rows: Vec<_> = dood_with(
        s,
        b.id,
        sb.id,
        DoodOptions {
            no_holds: opts.no_holds,
        },
    )
    .map_err(|e| e.to_string())?
    .into_iter()
    .filter(|r| s.element(r.element).is_some_and(|e| e.category == cat.id))
    .filter(|r| opts.elements.is_empty() || opts.elements.contains(&r.element))
    .collect();
    sort_rows(s, &mut rows, &opts.sort);
    let first = rows
        .iter()
        .filter_map(|r| r.cells.first())
        .map(|c| c.0)
        .min()
        .map(|d| opts.from.map_or(d, |f| d.max(f)));
    let last = rows
        .iter()
        .filter_map(|r| r.cells.last())
        .map(|c| c.0)
        .max()
        .map(|d| opts.to.map_or(d, |t| d.min(t)));
    let (first, last) = match (first, last) {
        (Some(a), Some(z)) if a <= z => (Some(a), Some(z)),
        _ => (None, None),
    };
    let (Some(first), Some(last)) = (first, last) else {
        out.empty = Some(format!(
            "No one in {} works on a shoot day or has an event yet.",
            cat.name
        ));
        return Ok(out);
    };
    let first = opts.week_start.map_or(first, |w| week_first(first, w));
    // Shoot day numbers by date, walking the calendar once.
    let shoot_days = days(s, b.id, sb.id).map_err(|e| e.to_string())?.days.len() as u32;
    let mut numbers = std::collections::HashMap::new();
    let mut d = start;
    let mut n = 0;
    while n < shoot_days && d <= last {
        if sb.calendar.is_shoot_day(d) {
            n += 1;
            numbers.insert(d, n);
        }
        d = d.add_days(1);
    }
    let cal = &sb.calendar;
    let mut d = first;
    while d <= last {
        let (_, m, day) = d.ymd();
        out.dates.push(DoodDateDto {
            date: d.to_string(),
            weekday: weekday(d),
            day,
            month: month_name(m),
            kind: if cal.is_move(d) {
                "move"
            } else if cal.is_holiday(d) {
                "holiday"
            } else if cal.is_off(d) {
                "off"
            } else {
                "work"
            },
            shoot_day: numbers.get(&d).copied(),
        });
        d = d.add_days(1);
    }
    let width = out.dates.len();
    let flagged = crate::redflags::by_element_date(s, b, sb);
    for (r, sum) in rows.iter().zip(summary(&rows)) {
        let e = s.element(r.element).ok_or("missing element")?;
        let mut cells = vec![String::new(); width];
        let mut warnings = Vec::new();
        let at = |d: Date| (d.0 - first.0) as usize;
        for (d, c) in r.cells.iter().filter(|(d, _)| *d >= first && *d <= last) {
            cells[at(*d)] = c.code_with(&s.dood_codes);
            // An element left off the board never conflicts (decisions 28, 105 and 120;
            // E-E3).
            if c.is_flagged() && !e.exclude_from_board {
                warnings.push(DoodFlagDto {
                    at: at(*d),
                    text: "an event on a work day, a conflict; the day counts once, as work"
                        .to_string(),
                });
            }
        }
        out.rows.push(DoodRowDto {
            element: e.id.0,
            board_id: e.board_id.clone().unwrap_or_default(),
            name: e.name.clone(),
            cells,
            span: r
                .start
                .zip(r.finish)
                .map(|(a, z)| (a.max(first), z.min(last)))
                .filter(|(a, z)| a <= z)
                .map(|(a, z)| (at(a), at(z))),
            work: sum.work,
            hold: sum.hold,
            start: sum.start.map(|d| d.to_string()),
            finish: sum.finish.map(|d| d.to_string()),
            total: sum.total,
            holds: e.rules.holds,
            drops: e.rules.drops,
            red_flags: flags_on_row(&flagged, r.element, first, width),
            warnings,
        });
    }
    Ok(out)
}

/// The date on or before `d` that falls on `start`, the week's first day.
pub fn week_first(d: Date, start: method_schedule_core::Weekday) -> Date {
    use method_schedule_core::Weekday;
    let at = |w: Weekday| Weekday::ALL.iter().position(|x| *x == w).unwrap_or(0) as i32;
    d.add_days(-(at(d.weekday()) - at(start)).rem_euclid(7))
}

fn sb_has_start(s: &Schedule, sub: u32) -> bool {
    sub_ref(s, sub).is_ok_and(|(_, sb)| sb.start.is_some())
}

/// The red flag clashes on one day out of days row, by date index into the grid.
fn flags_on_row(
    flagged: &std::collections::HashMap<
        (method_schedule_core::model::ElementId, Date),
        Vec<String>,
    >,
    element: method_schedule_core::model::ElementId,
    first: Date,
    width: usize,
) -> Vec<DoodFlagDto> {
    if flagged.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<DoodFlagDto> = flagged
        .iter()
        .filter(|((e, _), _)| *e == element)
        .filter_map(|((_, d), texts)| {
            let at = usize::try_from(d.0 - first.0).ok().filter(|&i| i < width)?;
            Some(DoodFlagDto {
                at,
                text: texts.join("; "),
            })
        })
        .collect();
    out.sort_by_key(|f| f.at);
    out
}

/// The first board's first sub-board, for tests.
#[doc(hidden)]
pub fn first_sub(s: &Schedule) -> u32 {
    s.boards[0].sub_boards[0].id.0
}

/// The first board, for tests.
#[doc(hidden)]
pub fn first_board(s: &Schedule) -> u32 {
    s.boards[0].id.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::session::{read_script, Session};
    use std::path::Path;

    /// A session holding Harbor Light, every scene still in the Boneyard.
    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        let mut s = Session::new();
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s
    }

    /// The strip color compared in place reads the same as upper-casing a copy did
    /// (decision 190), for odd spacing, case and text that is not ASCII.
    #[test]
    fn tone_reads_int_ext_and_night_in_any_case() {
        let texts = [
            "",
            "EXT",
            "ext.",
            " Ext",
            "EX",
            "E",
            "INT/EXT",
            "éxt",
            "EXTÉ",
            "xEXT",
            "\tEXT",
            "NIGHT",
            "night",
            "Late Night",
            "NIGH",
            "NIGHTS",
            "dusk",
            "nÏght",
            "NIGHT é",
        ];
        for ie in texts {
            for tod in texts {
                let h = Sheet {
                    int_ext: ie.to_string(),
                    time_of_day: tod.to_string(),
                    ..Sheet::default()
                };
                let ext = ie.trim().to_ascii_uppercase().starts_with("EXT");
                let night = tod.to_ascii_uppercase().contains("NIGHT");
                let want = match (ext, night) {
                    (false, false) => "int-day",
                    (true, false) => "ext-day",
                    (false, true) => "int-night",
                    (true, true) => "ext-night",
                };
                assert_eq!(tone(&h), want, "{ie:?} {tod:?}");
            }
        }
    }

    /// Where the board view spends its time at 1,500 strips, piece by piece.
    /// `cargo test --release -p method-schedule-app review_profile_view -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn review_profile_view() {
        use std::time::Instant;
        let sch = method_schedule_core::sample::sample(1500);
        let b = sch.boards[0].id;
        let bd = sch.board(b).unwrap();
        let sb = bd.sub_boards[0].id;
        let n = 50u32;
        let time = |label: &str, f: &mut dyn FnMut()| {
            f();
            let t = Instant::now();
            for _ in 0..n {
                f();
            }
            println!("{label:<40} {:>10.1?}", t.elapsed() / n);
        };
        let cast = sch.cast();
        let layout = sch.strip_layout_of(b);
        time("board() total", &mut || {
            let _ = board(&sch, b).unwrap();
        });
        time("conflicts::count", &mut || {
            let _ = crate::conflicts::count(&sch, b);
        });
        time("conflictmarks::marks", &mut || {
            let _ = crate::conflictmarks::marks(&sch, b);
        });
        time("core conflicts()", &mut || {
            let _ = method_schedule_core::conflicts::conflicts(&sch, b);
        });
        time("redflags::board_marks", &mut || {
            let _ = crate::redflags::board_marks(&sch, bd);
        });
        time("core days()", &mut || {
            let _ = method_schedule_core::days::days(&sch, b, sb).unwrap();
        });
        let marks = crate::redflags::board_marks(&sch, bd);
        time("Lookup::new", &mut || {
            std::hint::black_box(Lookup::new(&sch));
        });
        let lookup = Lookup::new(&sch);
        time("sub_board_dto", &mut || {
            let _ = sub_board_dto(&sch, b, &bd.sub_boards[0], &lookup, &marks).unwrap();
        });
        time("strip() x all strips", &mut || {
            for (i, st) in bd.sub_boards[0].strips.iter().enumerate() {
                let _ = strip(&sch, &lookup, &layout, st, i);
            }
        });
        time("s.sheet() x all strips", &mut || {
            for st in &bd.sub_boards[0].strips {
                if let StripKind::Sheet(id) = st.kind {
                    std::hint::black_box(sch.sheet(id));
                }
            }
        });
        time("cast_ids x all strips", &mut || {
            for st in &bd.sub_boards[0].strips {
                if let StripKind::Sheet(id) = st.kind {
                    let h = sch.sheet(id).unwrap();
                    std::hint::black_box(method_schedule_core::reports::cast_ids(&sch, h, cast));
                }
            }
        });
        time("strip_cell x cols x strips", &mut || {
            for st in &bd.sub_boards[0].strips {
                if let StripKind::Sheet(id) = st.kind {
                    let h = sch.sheet(id).unwrap();
                    for c in &layout.columns {
                        std::hint::black_box(strip_cell(&sch, h, c.field, cast));
                    }
                }
            }
        });
        time("color_of x strips", &mut || {
            for st in &bd.sub_boards[0].strips {
                if let StripKind::Sheet(id) = st.kind {
                    let h = sch.sheet(id).unwrap();
                    std::hint::black_box(sch.strip_colors.color_of(h));
                }
            }
        });
        time("day_quantity_text x days", &mut || {
            let d = method_schedule_core::days::days(&sch, b, sb).unwrap();
            for day in &d.days {
                let hs: Vec<&Sheet> = day
                    .entries
                    .iter()
                    .filter_map(|e| e.sheet)
                    .filter_map(|h| lookup.sheets.get(&h).copied())
                    .collect();
                std::hint::black_box(method_schedule_core::reports::day_quantity_text_with(
                    &sch,
                    &lookup.elements,
                    &hs,
                ));
            }
        });
        time("parts_of", &mut || {
            std::hint::black_box(parts_of(bd));
        });
        time("layouts_dto + types", &mut || {
            std::hint::black_box(layouts_dto(&sch));
            std::hint::black_box(crate::redflags::types(&sch));
        });
        time("pages sum", &mut || {
            let p: Eighths = sch.sheets.iter().map(|h| h.pages).sum();
            std::hint::black_box(p);
        });
        println!(
            "sheets {} elements {} categories {} strips {} columns {}",
            sch.sheets.len(),
            sch.elements.len(),
            sch.categories.len(),
            bd.sub_boards[0].strips.len(),
            layout.columns.len()
        );
    }

    /// What one edit costs the app at the kickoff's 1,500-strip target: the edit (copy,
    /// apply, check, undo step), then the view the page redraws from, serialized as the
    /// page receives it. `cargo test --release -p method-schedule-app perf -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn perf_edit_and_view_at_1500_strips() {
        use method_schedule_core::model::{RedFlag, RedFlagDates, RedFlagId};
        use std::time::Instant;
        let mut s = Session::new();
        s.edit(|sch| {
            *sch = method_schedule_core::sample::sample(1500);
            Ok(())
        })
        .unwrap();
        let (b, sb) = (
            s.schedule().boards[0].id,
            s.schedule().boards[0].sub_boards[0].id,
        );
        let strips: Vec<_> = s.schedule().boards[0].sub_boards[0]
            .strips
            .iter()
            .map(|x| x.id)
            .collect();
        let rounds = 20u32;
        let run = |s: &mut Session, label: &str| {
            let mut bytes = 0;
            let (mut edit, mut build, mut json) = Default::default();
            for i in 0..rounds as usize {
                let pick = strips[(i * 37) % strips.len()];
                let t = Instant::now();
                s.edit(|sch| {
                    sch.move_strips(
                        b,
                        &[pick],
                        method_schedule_core::model::Place {
                            sub_board: Some(sb),
                            index: (i * 101) % strips.len(),
                        },
                    )
                    .map_err(|e| e.to_string())
                })
                .unwrap();
                edit += t.elapsed();
                let t = Instant::now();
                let v = view(s, None).unwrap();
                build += t.elapsed();
                let t = Instant::now();
                bytes = serde_json::to_string(&v).unwrap().len();
                json += t.elapsed();
            }
            let each = |d: std::time::Duration| d / rounds;
            println!(
                "1500 strips{label}: edit {:?}, view {:?}, JSON {:?} each ({:?} in all), view {} KB",
                each(edit),
                each(build),
                each(json),
                each(edit + build + json),
                bytes / 1024
            );
        };
        run(&mut s, "");
        // The sample has no red flags: give the six elements on most sheets a flag each
        // (three types; ranges, open ends, hard and soft), then measure again.
        s.edit(|sch| {
            let kinds: Vec<_> = ["Unavailable", "Second job", "Child hours"]
                .iter()
                .map(|n| sch.add_red_flag_type(n).unwrap())
                .collect();
            let start = sch.boards[0].sub_boards[0]
                .start
                .expect("the sample is dated");
            let mut busy: Vec<_> = sch
                .elements
                .iter()
                .map(|e| {
                    let n = sch
                        .sheets
                        .iter()
                        .filter(|h| h.elements.contains(&e.id))
                        .count();
                    (n, e.id)
                })
                .collect();
            busy.sort_by(|a, b| b.cmp(a));
            for (i, &(_, el)) in busy.iter().take(6).enumerate() {
                let at = start.add_days(7 * i as i32 + 3);
                let dates = match i % 3 {
                    0 => RedFlagDates::Range {
                        from: at,
                        to: at.add_days(6),
                    },
                    1 => RedFlagDates::NotBefore(at),
                    _ => RedFlagDates::NotAfter(at.add_days(60)),
                };
                sch.set_red_flag(
                    el,
                    RedFlag {
                        id: RedFlagId(0),
                        kind: kinds[i % 3],
                        dates,
                        note: String::new(),
                        hard: i % 2 == 0,
                    },
                )
                .unwrap();
            }
            Ok(())
        })
        .unwrap();
        run(&mut s, " with 6 red flags");
        let t = Instant::now();
        let mut found = 0;
        for _ in 0..rounds {
            found = method_schedule_core::redflags::red_flags(s.schedule(), b, sb)
                .unwrap()
                .clashes
                .len();
        }
        let engine = t.elapsed() / rounds;
        let t = Instant::now();
        let mut kb = 0;
        for _ in 0..rounds {
            let tab = crate::redflags::red_flags_view(s.schedule(), sb.0).unwrap();
            kb = serde_json::to_string(&tab).unwrap().len() / 1024;
        }
        println!(
            "red_flags on one sub-board: {:?} ({found} clashes); the Red flags tab with JSON: {:?}, {kb} KB",
            engine,
            t.elapsed() / rounds
        );
        // For timing the page: METHOD_SCHEDULE_VIEW_DUMP=path writes the view the page would get.
        if let Ok(path) = std::env::var("METHOD_SCHEDULE_VIEW_DUMP") {
            std::fs::write(
                path,
                serde_json::to_string(&view(&s, None).unwrap()).unwrap(),
            )
            .unwrap();
        }
    }

    fn all_ids(v: &BoardDto) -> Vec<u32> {
        v.boneyard.iter().map(|x| x.id).collect()
    }

    fn sub(s: &Session) -> u32 {
        s.schedule().boards[0].sub_boards[0].id.0
    }

    #[test]
    fn an_imported_script_waits_in_the_boneyard() {
        let s = harbor();
        let v = view(&s, None).unwrap();
        assert_eq!(v.title, "Harbor Light");
        assert_eq!(v.file.name, "Untitled");
        assert!(v.file.dirty && v.can_undo && !v.file.saved_once);
        let b = &v.board;
        assert_eq!(b.sub_boards.len(), 1);
        assert!(b.sub_boards[0].days.is_empty());
        assert_eq!(b.boneyard.len(), s.schedule().sheets.len());
        let first = &b.boneyard[0];
        assert_eq!(
            (first.kind, first.scene.as_str(), first.int_ext.as_str()),
            ("scene", "1", "INT")
        );
        assert_eq!(
            (first.set.as_str(), first.time_of_day.as_str()),
            ("NET LOFT", "NIGHT")
        );
        assert_eq!(first.tone, "int-night");
        // No board IDs after an import (decisions.md, 10): names stand in.
        assert_eq!(first.cast, ["KIT", "TOVE"]);
        assert_eq!(b.boneyard[1].tone, "ext-night");
        assert_eq!(b.boneyard[2].tone, "ext-day");
        for (i, st) in b.boneyard.iter().enumerate() {
            assert_eq!(st.index, i);
        }
    }

    #[test]
    fn scheduling_breaking_and_dating_days() {
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        // Everything onto the sub-board (7/8 pages in five scenes), then breaks every 3/8.
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "3/8")).unwrap();
        // Typed month first, as the start date field takes it (decision 49).
        s.edit(|x| board::set_start_date(x, sb, Some("3/5/2027")))
            .unwrap();
        let v = view(&s, None).unwrap();
        let b = &v.board.sub_boards[0];
        assert!(v.board.boneyard.is_empty());
        assert!(b.unscheduled.is_empty(), "a break closes the last day");
        assert_eq!(b.days.len(), 3);
        // 2027-03-05 is a Friday; day 2 skips the weekend.
        assert_eq!(b.days[0].date.as_deref(), Some("2027-03-05"));
        assert_eq!(b.days[0].weekday.as_deref(), Some("Fri"));
        assert_eq!(b.days[1].date.as_deref(), Some("2027-03-08"));
        assert_eq!(b.days[1].weekday.as_deref(), Some("Mon"));
        // Day totals agree with the engine, and indexes run through the whole list.
        let engine = days(
            s.schedule(),
            s.schedule().boards[0].id,
            s.schedule().boards[0].sub_boards[0].id,
        )
        .unwrap();
        let mut next = 0;
        for (d, e) in b.days.iter().zip(&engine.days) {
            assert_eq!(d.pages, e.pages.to_string());
            assert_eq!(d.time, "0:00");
            assert_eq!(d.number, e.number);
            for st in &d.strips {
                assert_eq!(st.index, next);
                next += 1;
            }
            assert_eq!((d.end.kind, d.end.index), ("break", next));
            next += 1;
        }
        assert_eq!(b.len, next);
        // Every day reaches the limit except perhaps the last.
        for d in &b.days[..b.days.len() - 1] {
            assert!(
                d.pages.parse::<Eighths>().unwrap() >= Eighths(3),
                "{}",
                d.pages
            );
        }
        assert_eq!(b.days[2].pages, "1/8");
    }

    #[test]
    fn moving_strips_one_place_and_between_lists() {
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids[..3], Some(sb), 0))
            .unwrap();
        let order = |s: &Session| -> Vec<u32> {
            view(s, None).unwrap().board.sub_boards[0]
                .unscheduled
                .iter()
                .map(|x| x.id)
                .collect()
        };
        assert_eq!(order(&s), ids[..3]);
        // Down one place: index + 2 counts positions before the move.
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &[ids[0]], Some(sb), 2))
            .unwrap();
        assert_eq!(order(&s), [ids[1], ids[0], ids[2]]);
        // Up one place.
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &[ids[2]], Some(sb), 1))
            .unwrap();
        assert_eq!(order(&s), [ids[1], ids[2], ids[0]]);
        // Back to the top of the Boneyard.
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &[ids[0]], None, 0))
            .unwrap();
        assert_eq!(view(&s, None).unwrap().board.boneyard[0].id, ids[0]);
        // Undo walks the moves back.
        s.undo();
        assert_eq!(order(&s), [ids[1], ids[2], ids[0]]);
    }

    #[test]
    fn day_breaks_are_inserted_parked_and_deleted() {
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        let brk = s.edit(|x| board::insert_day_break(x, sb, 2)).unwrap();
        let v = view(&s, None).unwrap();
        let day = &v.board.sub_boards[0].days[0];
        assert_eq!(day.strips.len(), 2);
        assert_eq!(day.end.id, brk);
        assert_eq!(day.scenes, 2);

        // Parked in the Boneyard: no day any more (decisions.md, entry 57).
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &[brk], None, 0))
            .unwrap();
        let v = view(&s, None).unwrap();
        assert!(v.board.sub_boards[0].days.is_empty());
        assert_eq!(v.board.boneyard[0].kind, "break");
        s.edit(|x| board::delete_strips(x, x.boards[0].id.0, &[brk]))
            .unwrap();
        assert!(view(&s, None).unwrap().board.sub_boards[0].days.is_empty());
        assert!(s
            .edit(|x| board::delete_strips(x, x.boards[0].id.0, &[brk]))
            .is_err());
    }

    #[test]
    fn bad_input_is_refused_and_changes_nothing() {
        let mut s = harbor();
        let sb = sub(&s);
        let before = s.schedule().clone();
        for bad in ["", "0", "1/3", "two"] {
            assert!(
                s.edit(|x| board::auto_day_breaks(x, sb, bad)).is_err(),
                "{bad}"
            );
        }
        assert!(s
            .edit(|x| board::set_start_date(x, sb, Some("2027-02-30")))
            .is_err());
        let err = s
            .edit(|x| board::set_start_date(x, sb, Some("3/1/27")))
            .unwrap_err();
        assert_eq!(
            err,
            "The start date \"3/1/27\" is not a date. Type it as 3/1/2027 or 2027-03-01."
        );
        assert!(s
            .edit(|x| board::set_start_date(x, 999_999, Some("2027-03-01")))
            .is_err());
        assert!(s
            .edit(|x| board::move_strips(x, x.boards[0].id.0, &[999_999], Some(sb), 0))
            .is_err());
        assert!(s.edit(|x| board::insert_day_break(x, 999_999, 0)).is_err());
        assert_eq!(s.schedule(), &before);
        // Clearing a start date that isn't set is no step at all.
        let steps_before = s.can_redo();
        s.edit(|x| board::set_start_date(x, sb, Some(""))).unwrap();
        assert_eq!(s.can_redo(), steps_before);
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn the_sheet_view_shows_fields_groups_place_and_neighbors() {
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids[..2], Some(sb), 0))
            .unwrap();
        s.edit(|x| board::insert_day_break(x, sb, 1)).unwrap();
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
            .unwrap();
        // Strips carry their sheet, so a double-click can open it.
        let v = view(&s, None).unwrap();
        let first = &v.board.sub_boards[0].days[0].strips[0];
        let id = first.sheet.unwrap();
        assert_eq!(v.board.sub_boards[0].days[0].end.sheet, None);
        s.edit(|x| crate::sheet::set_field(x, id, "shoot_time", "1:05"))
            .unwrap();
        s.edit(|x| crate::sheet::set_field(x, id, "sunrise", "6:30"))
            .unwrap();
        s.edit(|x| crate::sheet::set_pages(x, id, "1", "11"))
            .unwrap();
        s.edit(|x| crate::sheet::create_element(x, id, "Props", "Net"))
            .unwrap();

        let d = sheet_view(
            s.schedule(),
            crate::view::first_board(s.schedule()),
            Some(id),
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(d.id, id);
        assert_eq!((d.index, d.prev), (0, None));
        assert_eq!(d.count, s.schedule().sheets.len());
        assert_eq!(
            d.next,
            v.board.sub_boards[0]
                .unscheduled
                .first()
                .and_then(|x| x.sheet)
        );
        assert_eq!(d.shoot_time, "1:05");
        assert_eq!(d.sunrise, "06:30");
        assert_eq!(d.sunset, "");
        assert_eq!(
            (d.pages_whole.as_str(), d.pages_eighths.as_str()),
            ("2", "3")
        );
        assert_eq!(d.pages, "2 3/8");
        assert_eq!(d.place, "Main, day 1, Mon 2027-03-01");
        // The place links to the strip on the board shown (decisions.md, entry 58).
        let main = &s.schedule().boards[0].sub_boards[0];
        assert_eq!(d.strip_sub_board, Some(main.id.0));
        assert_eq!(
            d.strip,
            main.strips
                .iter()
                .find(|x| x.kind
                    == method_schedule_core::model::StripKind::Sheet(
                        method_schedule_core::model::SheetId(id)
                    ))
                .map(|x| x.id.0)
        );
        let cats: Vec<&str> = d.groups.iter().map(|g| g.category.as_str()).collect();
        assert_eq!(
            cats,
            ["Cast", "Props"],
            "category order, empty ones left out"
        );
        assert_eq!(d.groups[1].elements[0].name, "Net");
        assert!(d.categories.iter().any(|c| c.name == "Props"));
        assert!(d
            .elements
            .iter()
            .any(|e| e.name == "TOVE" && e.category == "Cast"));
        assert!(d.header_values["int_ext"].contains(&"INT".to_string()));
        assert!(d.header_values["time_of_day"].contains(&"NIGHT".to_string()));
        assert_eq!(d.header_values.len(), 8, "every one-value header field");
        assert!(!d.header_values["set"].is_empty());

        // Unscheduled and Boneyard places; a missing sheet falls back to the index.
        let second = sheet_view(
            s.schedule(),
            crate::view::first_board(s.schedule()),
            d.next,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(second.place, "Main, not scheduled");
        assert_eq!(second.prev, Some(id));
        let last = sheet_view(
            s.schedule(),
            crate::view::first_board(s.schedule()),
            Some(999_999),
            Some(10_000),
        )
        .unwrap()
        .unwrap();
        assert_eq!(last.index, last.count - 1);
        assert_eq!(last.place, "Boneyard");
        assert_eq!(last.strip_sub_board, None);
        assert!(last.strip.is_some());
        assert_eq!(last.next, None);
        assert_eq!(
            sheet_view(
                s.schedule(),
                crate::view::first_board(s.schedule()),
                None,
                None
            )
            .unwrap()
            .unwrap()
            .index,
            0
        );
        let empty = Session::new();
        assert!(
            sheet_view(empty.schedule(), first_board(empty.schedule()), None, None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn the_elements_view_lists_categories_and_one_categorys_elements() {
        let mut s = harbor();
        let sb = sub(&s);
        let v = elements_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(v.categories.len(), s.schedule().categories.len());
        let cast = &v.categories[0];
        assert!(cast.built_in && cast.place.is_none());
        assert_eq!(v.category, Some(cast.id));
        assert_eq!(cast.elements, v.elements.len());
        assert!(v
            .dates_note
            .as_deref()
            .unwrap()
            .starts_with("Set a start date"));
        let kit = v.elements.iter().find(|e| e.name == "KIT").unwrap();
        assert!(kit.sheets >= 3 && kit.days == 0 && kit.start.is_none());
        assert_eq!(kit.min_drop_gap, "1");

        let props = s
            .edit(|x| crate::elements::add_category(x, "Props"))
            .unwrap();
        let fx = s
            .edit(|x| crate::elements::add_category(x, "Effects"))
            .unwrap();
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "3/8")).unwrap();
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
            .unwrap();
        let v = elements_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(cast.id),
            None,
        )
        .unwrap();
        assert!(v.dates_note.is_none());
        let kit = v.elements.iter().find(|e| e.name == "KIT").unwrap();
        assert_eq!(kit.start.as_deref(), Some("2027-03-01"));
        assert!(kit.days > 0);
        let places: Vec<Option<usize>> = v.categories.iter().map(|c| c.place).collect();
        let n = v.categories.len() - 1;
        assert_eq!(places[0], None);
        assert_eq!(places[n], Some(n - 1));
        assert!(v.categories.iter().all(|c| c.user_count == n));
        // A category that is gone falls back to the first.
        let v = elements_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(fx),
            None,
        )
        .unwrap();
        assert_eq!(v.category, Some(fx));
        assert!(v.elements.is_empty());
        s.edit(|x| crate::elements::delete_category(x, fx)).unwrap();
        assert_eq!(
            elements_view(
                s.schedule(),
                crate::view::first_sub(s.schedule()),
                Some(fx),
                None
            )
            .unwrap()
            .category,
            Some(cast.id)
        );
        let _ = props;
    }

    #[test]
    fn the_elements_view_has_events_conflicts_days_and_header_fields() {
        use method_schedule_core::model::{ElementId, Event, EventKind};
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "3/8")).unwrap();
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
            .unwrap();
        let cast = s.schedule().cast().unwrap();
        let kit = s.schedule().element_named(cast, "KIT").unwrap();
        // A fitting on KIT's first work day: one event, one conflict (decision 43).
        s.edit(|x| {
            x.boards[0].sub_boards[0].calendar.events.push(Event {
                name: "Wardrobe".into(),
                kind: EventKind::Fitting,
                from: "2027-03-01".parse().unwrap(),
                to: "2027-03-01".parse().unwrap(),
                elements: vec![kit],
                color: None,
            });
            Ok(())
        })
        .unwrap();
        let v = elements_view(s.schedule(), sb, Some(cast.0), None).unwrap();
        let row = v.elements.iter().find(|e| e.id == kit.0).unwrap();
        assert_eq!((row.events, row.conflicts), (1, 1));
        assert_eq!(v.headers.len(), 8, "every header field (decision 118)");
        assert!(v.all_elements.iter().any(|e| e.id == kit.0));
        // Left off the board: no conflict, here and on the day out of days.
        s.edit(|x| crate::elements::set_category_field(x, cast.0, "exclude_from_board", "true"))
            .unwrap();
        // A default for new elements; applied, it reaches the others (decision 120).
        s.edit(|x| crate::elements::apply_category_defaults(x, cast.0))
            .unwrap();
        let v = elements_view(s.schedule(), sb, Some(cast.0), None).unwrap();
        assert_eq!(
            v.elements.iter().find(|e| e.id == kit.0).unwrap().conflicts,
            0
        );
        let g = dood_grid(s.schedule(), sb, Some(cast.0)).unwrap();
        assert!(g.excluded && g.rows.iter().all(|r| r.warnings.is_empty()));
        // Days and dates: every date of KIT's row, with its code.
        let d = element_days(s.schedule(), sb, kit.0).unwrap();
        assert!(d.empty.is_none());
        assert_eq!(d.days[0].date, "2027-03-01");
        assert_eq!(d.days[0].shoot_day, Some(1));
        assert!(d.days[0].code.starts_with("FT"));
        assert!(element_days(s.schedule(), sb, 999_999).is_err());
        assert!(element_days(s.schedule(), sb, ElementId(kit.0).0).is_ok());
        // A header field as a category, in the Elements tab and the day out of days.
        let v = elements_view(s.schedule(), sb, None, Some("set")).unwrap();
        assert_eq!((v.header, v.category), (Some("set"), None));
        let loft = v.elements.iter().find(|e| e.name == "NET LOFT").unwrap();
        assert!(loft.sheets >= 1 && loft.days >= 1);
        let g = dood_grid_for(s.schedule(), sb, None, Some("set"), &GridOpts::default()).unwrap();
        assert_eq!(g.header, Some("set"));
        assert!(g.rows.iter().any(|r| r.name == "NET LOFT"));
        assert_eq!(g.categories.len(), s.schedule().categories.len());
        // An unknown header key is a category view as usual.
        let v = elements_view(s.schedule(), sb, None, Some("cast")).unwrap();
        assert_eq!(v.header, None);
    }

    #[test]
    fn the_calendar_view_shows_a_month_of_marks_days_and_events() {
        let mut s = harbor();
        let sb = sub(&s);
        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "1/8")).unwrap();
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
            .unwrap();
        s.edit(|x| {
            crate::calendar::set_day_kind(
                x,
                x.boards[0].sub_boards[0].id.0,
                "2027-03-02",
                "holiday",
            )
        })
        .unwrap();
        s.edit(|x| {
            crate::calendar::set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-06", "work")
        })
        .unwrap();
        let kit = s
            .schedule()
            .elements
            .iter()
            .find(|e| e.name == "KIT")
            .unwrap()
            .id
            .0;
        let ev = crate::calendar::EventInput {
            kind: "travel".into(),
            label: String::new(),
            name: "To the coast".into(),
            from: "2027-02-27".into(),
            to: "2027-03-01".into(),
            elements: vec![kit],
            color: None,
        };
        s.edit(|x| crate::calendar::save_event(x, x.boards[0].sub_boards[0].id.0, None, &ev))
            .unwrap();

        // No month given: the start date's.
        let c = calendar_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            None,
            None,
        )
        .unwrap();
        assert_eq!((c.year, c.month, c.title.as_str()), (2027, 3, "March 2027"));
        assert_eq!(c.weekly_off, ["sat", "sun"]);
        assert_eq!(c.weeks.len(), 6);
        assert!(c.weeks.iter().all(|w| w.len() == 7));
        // March 2027 starts on a Monday: the grid starts on Sunday February 28.
        let first = &c.weeks[0][0];
        assert_eq!(
            (first.date.as_str(), first.weekday, first.in_month),
            ("2027-02-28", "Sun", false)
        );
        let day = |date: &str| {
            c.weeks
                .iter()
                .flatten()
                .find(|d| d.date == date)
                .unwrap()
                .clone()
        };
        assert_eq!(day("2027-03-01").shoot_day, Some(1));
        assert_eq!(day("2027-03-01").events, [0]);
        assert_eq!(
            day("2027-02-28").events,
            [0],
            "the travel starts in February"
        );
        assert_eq!(day("2027-03-02").kind, "holiday");
        assert_eq!(day("2027-03-03").shoot_day, Some(2));
        let sat = day("2027-03-06");
        assert!(sat.kind == "work" && sat.weekly_off && sat.shoot_day.is_some());
        assert_eq!(day("2027-03-07").kind, "off");
        assert_eq!(c.first_day.as_deref(), Some("2027-03-01"));
        assert!(c.last_day.is_some());
        assert_eq!(c.events[0].kind, "travel");
        assert!(c.events[0].counts);
        assert_eq!(c.events[0].element_names, ["KIT"]);
        assert!(c
            .elements
            .iter()
            .any(|e| e.name == "KIT" && e.category == "Cast"));
        assert_eq!((c.prev, c.next), ((2027, 2), (2027, 4)));
        // Another month, and the year turning.
        let d = calendar_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(2026),
            Some(12),
        )
        .unwrap();
        assert_eq!(d.title, "December 2026");
        assert_eq!(d.next, (2027, 1));
        // Monday first (Settings, CAL-03): the week's names and the grid follow it, and the
        // dates keep their kinds and days. December 2026 starts on a Tuesday.
        let monday = CalOpts {
            monday: true,
            ..CalOpts::default()
        };
        let m = calendar_view_with(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(2026),
            Some(12),
            &monday,
        )
        .unwrap();
        assert_eq!(m.week, ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]);
        assert_eq!(
            (m.weeks[0][0].date.as_str(), m.weeks[0][0].weekday),
            ("2026-11-30", "Mon")
        );
        let march = calendar_view_with(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            None,
            None,
            &monday,
        )
        .unwrap();
        let first = &march.weeks[0][0];
        assert_eq!((first.date.as_str(), first.in_month), ("2027-03-01", true));
        assert_eq!(first.shoot_day, Some(1));
        assert_eq!(march.weeks[0][1].kind, "holiday");
        assert_eq!(march.weeks[0][6].weekday, "Sun");
        assert!(calendar_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(2027),
            Some(13)
        )
        .is_err());
        assert!(calendar_view(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(2027),
            None
        )
        .is_err());
    }

    #[test]
    fn the_dood_grid_has_a_column_per_date_and_the_summary() {
        let mut s = harbor();
        let sb = sub(&s);
        let d = dood_grid(s.schedule(), crate::view::first_sub(s.schedule()), None).unwrap();
        assert!(d.rows.is_empty() && d.dates.is_empty());
        assert!(d.empty.as_deref().unwrap().starts_with("Set a start date"));
        let cast = s.schedule().category_named("Cast").unwrap().0;
        assert_eq!(d.category, Some(cast), "Cast by default");

        let ids = all_ids(&view(&s, None).unwrap().board);
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "1/8")).unwrap();
        // Friday: the weekend falls inside the grid.
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-05")))
            .unwrap();
        let d = dood_grid(s.schedule(), crate::view::first_sub(s.schedule()), None).unwrap();
        assert!(d.empty.is_none());
        assert_eq!(d.dates[0].date, "2027-03-05");
        assert_eq!((d.dates[0].weekday, d.dates[0].day), ("Fri", 5));
        assert_eq!(d.dates[0].month, "Mar");
        assert_eq!(d.dates[0].shoot_day, Some(1));
        assert_eq!((d.dates[1].kind, d.dates[1].shoot_day), ("off", None));
        assert_eq!(d.dates[3].shoot_day, Some(2));
        // Rows in the engine's order, every one as wide as the dates.
        let engine = method_schedule_core::dood::dood(
            s.schedule(),
            s.schedule().boards[0].id,
            s.schedule().boards[0].sub_boards[0].id,
        )
        .unwrap();
        let names: Vec<&str> = d.rows.iter().map(|r| r.name.as_str()).collect();
        let want: Vec<&str> = engine
            .iter()
            .map(|r| s.schedule().element(r.element).unwrap().name.as_str())
            .collect();
        assert_eq!(names, want);
        for (r, e) in d.rows.iter().zip(&engine) {
            assert_eq!(r.cells.len(), d.dates.len());
            assert_eq!(r.total, r.work + r.hold);
            assert_eq!(r.work, e.worked);
            for (date, cell) in &e.cells {
                let i = d
                    .dates
                    .iter()
                    .position(|x| x.date == date.to_string())
                    .unwrap();
                assert_eq!(r.cells[i], cell.code());
            }
            let (a, z) = r.span.unwrap();
            assert!(r.cells[a].starts_with('S') && r.cells[z].ends_with('F'));
        }
        let kit = d.rows.iter().find(|r| r.name == "KIT").unwrap();
        assert_eq!(kit.cells[0], "SW");
        assert_eq!(kit.hold, 0, "holds are off by default");
        assert_eq!(kit.start.as_deref(), Some("2027-03-05"));
        assert!(kit.warnings.is_empty());

        // A fitting on KIT's first day: the fitting code and the start mark, flagged.
        let kit_id = method_schedule_core::model::ElementId(kit.element);
        let on: Date = "2027-03-05".parse().unwrap();
        s.edit(|x| {
            x.boards[0].sub_boards[0]
                .calendar
                .events
                .push(method_schedule_core::model::Event {
                    name: "Wardrobe".into(),
                    kind: method_schedule_core::model::EventKind::Fitting,
                    from: on,
                    to: on,
                    elements: vec![kit_id],
                    color: None,
                });
            Ok(())
        })
        .unwrap();
        let d2 = dood_grid(s.schedule(), crate::view::first_sub(s.schedule()), None).unwrap();
        let kit2 = d2.rows.iter().find(|r| r.name == "KIT").unwrap();
        assert_eq!(kit2.cells[0], "FTSW");
        assert_eq!(kit2.work, kit.work, "counted once");
        assert_eq!(kit2.warnings.len(), 1);
        assert_eq!(kit2.warnings[0].at, 0);
        s.undo();

        // Another category: its own rows; a category with nobody working says so.
        let props = s
            .edit(|x| crate::elements::add_category(x, "Props"))
            .unwrap();
        let d = dood_grid(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(props),
        )
        .unwrap();
        assert_eq!(d.category, Some(props));
        assert!(d.rows.is_empty());
        assert!(d.empty.as_deref().unwrap().contains("Props"));
        let first = s.schedule().sheets[0].id.0;
        s.edit(|x| crate::sheet::create_element(x, first, "Props", "Net"))
            .unwrap();
        let d = dood_grid(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Some(props),
        )
        .unwrap();
        assert_eq!(d.rows.len(), 1);
        assert_eq!(d.rows[0].cells, ["SWF"]);
        // Events and calendar marks: a fitting before the start widens the grid; a
        // holiday and a company travel day shade their columns and reflow the days.
        let kit_id = s
            .schedule()
            .elements
            .iter()
            .find(|e| e.name == "KIT")
            .unwrap()
            .id;
        s.edit(|x| {
            let cal = &mut x.boards[0].sub_boards[0].calendar;
            cal.holidays.push("2027-03-08".parse().unwrap());
            cal.moves.push("2027-03-09".parse().unwrap());
            cal.events.push(method_schedule_core::model::Event {
                name: "Wardrobe".into(),
                kind: method_schedule_core::model::EventKind::Fitting,
                from: "2027-03-03".parse().unwrap(),
                to: "2027-03-03".parse().unwrap(),
                elements: vec![kit_id],
                color: None,
            });
            Ok(())
        })
        .unwrap();
        let d = dood_grid(s.schedule(), crate::view::first_sub(s.schedule()), None).unwrap();
        assert_eq!(d.dates[0].date, "2027-03-03");
        let kit = d.rows.iter().find(|r| r.name == "KIT").unwrap();
        assert_eq!(kit.cells[0], "FT");
        assert_eq!(
            kit.span.unwrap().0,
            2,
            "the bar starts at the first work day"
        );
        let kind = |date: &str| d.dates.iter().find(|x| x.date == date).unwrap();
        assert_eq!(kind("2027-03-08").kind, "holiday");
        assert_eq!(kind("2027-03-09").kind, "move");
        assert_eq!(
            kind("2027-03-10").shoot_day,
            Some(2),
            "Monday and Tuesday are out"
        );
        // A category that is gone falls back to Cast.
        assert_eq!(
            dood_grid(
                s.schedule(),
                crate::view::first_sub(s.schedule()),
                Some(999_999)
            )
            .unwrap()
            .category,
            Some(cast)
        );
    }

    #[test]
    fn the_view_lists_boards_and_draws_the_one_shown_with_banners() {
        let mut s = harbor();
        let b = s.schedule().boards[0].id.0;
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        let ids: Vec<u32> = s.schedule().boards[0]
            .boneyard
            .iter()
            .map(|x| x.id.0)
            .collect();
        s.edit(|x| board::move_strips(x, b, &ids[..2], Some(sb), 0))
            .unwrap();
        s.edit(|x| board::insert_banner(x, sb, 1, "Lunch", "0:45"))
            .unwrap();
        s.edit(|x| board::insert_day_break(x, sb, 3)).unwrap();
        let v = view(&s, None).unwrap();
        let day = &v.board.sub_boards[0].days[0];
        assert_eq!(day.strips[1].kind, "banner");
        assert_eq!(
            (day.strips[1].text.as_str(), day.strips[1].duration.as_str()),
            ("Lunch", "0:45")
        );
        assert_eq!(day.scenes, 2);
        let copy = s
            .edit(|x| board::duplicate_board(x, b, "Rain plan"))
            .unwrap();
        s.edit(|x| board::rename_board(x, b, "Dry")).unwrap();
        let names: Vec<_> = view(&s, None)
            .unwrap()
            .boards
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(names, ["Dry", "Rain plan"]);
        s.show_board(method_schedule_core::model::BoardId(copy))
            .unwrap();
        let v = view(&s, None).unwrap();
        assert_eq!(v.board.id, copy);
        assert_eq!(v.board.name, "Rain plan");
        // The copy's sub-board has its own id; the other tabs take it.
        let csb = v.board.sub_boards[0].id;
        assert_ne!(csb, sb);
        assert!(dood_grid(s.schedule(), csb, None).is_ok());
        assert!(calendar_view(s.schedule(), csb, None, None).is_ok());
        assert!(elements_view(s.schedule(), csb, None, None).is_ok());
        assert!(dood_grid(s.schedule(), 9999, None).is_err());
        assert!(crate::explain::explain_count(s.schedule(), csb, 1).is_err());
        // A sheet's place is on the board it is asked about.
        let sheet = v.board.sub_boards[0].days[0].strips[0].sheet.unwrap();
        let shown = sheet_view(s.schedule(), copy, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert!(shown.place.starts_with("Main, day 1"), "{}", shown.place);
        s.edit(|x| {
            board::move_strips(
                x,
                copy,
                &[v.board.sub_boards[0].days[0].strips[0].id],
                None,
                0,
            )
        })
        .unwrap();
        let moved = sheet_view(s.schedule(), copy, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert_eq!(moved.place, "Boneyard");
        let still = sheet_view(s.schedule(), b, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert!(still.place.starts_with("Main, day 1"), "{}", still.place);
    }
}
