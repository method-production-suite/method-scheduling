//! The Reports tab: the report data the page lays out into printable pages, and printing
//! them, to the system print sheet or straight into a PDF file.
//!
//! The reports are the schedule's report list (`Schedule::report_list`, decision 59):
//! each a kind with its own options. The kinds are the one-line schedule, the shooting
//! schedule, the breakdown by strip and by day, the day out of days (grid and summary),
//! the element list, the element detail, the daily category totals, the calendar, the
//! vertical one-line with its cast ID grid, the multi-unit one-line, and the conflict and
//! red flag reports. Rust gives the values, from the engine (`method_schedule_core::reports`,
//! `report_data` and `dood`), with every option applied: the date range, the filters,
//! the columns, cast by name or ID. The page (ui/js/reports.js) only arranges them, in
//! our own plain layouts, and never computes a count.
//!
//! Printing follows Method Budget (`src/reports.rs` and `src/pdf_mac.rs` there, same owner
//! and license): the page lays the report out into Letter-sized page boxes in
//! `#print-root` (ui/js/report.js), then asks [`print_report`]. On macOS that hands the
//! main web view to AppKit's print operation, with the print sheet or, for a PDF, with no
//! panel into a file chosen in a Save dialog shown here. Elsewhere the page falls back to
//! `window.print()` and the system dialog's own PDF choice.
//!
//! Security: the PDF's path comes only from the native Save dialog shown here; the page
//! names the job, never a path. The PDF is written beside its target under a temporary
//! name and renamed into place when the job is over, so it lands whole or not at all.

use std::collections::HashMap;

use serde::Serialize;

use method_schedule_core::days::days;
use method_schedule_core::model::{
    CategoryId, ElementId, Schedule, Sheet, SheetId, StripColumn, StripField, StripKind,
    StripLayout,
};
use method_schedule_core::report_data::{self, element_text};
use method_schedule_core::reports::{self, OneLine, ShootingLine};
use method_schedule_core::setup::{PageBreak, ReportKind, ReportOptions, ReportSetup};
use method_schedule_core::{Date, Weekday};

use crate::view::{self, hm, tone, weekday, DoodGridDto, LayoutColumnDto};

/// One report's data. Only the parts its kind uses are filled; the rest stay empty.
#[derive(Clone, Debug, Serialize)]
pub struct ReportDto {
    /// The kind's name (`one_line`, `breakdown`, `calendar`...).
    pub report: &'static str,
    /// Its place in the report list.
    pub index: usize,
    /// Its name in the list, for the page header.
    pub name: String,
    /// The schedule's title.
    pub title: String,
    pub board: String,
    pub sub_board: String,
    /// The sub-board's start date, `YYYY-MM-DD`.
    pub start: Option<String>,
    /// Why there is nothing to print, when there is nothing.
    pub empty: Option<String>,
    /// The header layout it prints with, its items' texts filled in (decision 99).
    pub head: HeadDto,
    /// `small`, `normal` or `large`.
    pub text: &'static str,
    pub landscape: bool,
    /// The date range, in words, when one is set.
    pub range: Option<String>,
    /// The options the page arranges by: synopsis, banners, the shoot day lines and day
    /// breaks, strip colors, red flag marks, weekdays only. Where pages break is each
    /// day's (and block's) `new_page`.
    pub synopsis: bool,
    pub banners: bool,
    pub shoot_days: bool,
    pub day_breaks: bool,
    /// The last line with the report's scene count and pages, when asked for.
    pub total: Option<String>,
    pub colors: bool,
    pub icons: bool,
    pub weekdays_only: bool,
    /// The categories left out by the report's filter, by name (empty: none).
    pub filtered: Vec<String>,
    /// One-line, shooting schedule, breakdown by day: the shoot days in order.
    pub days: Vec<ReportDayDto>,
    /// Elements: every element, by category in category order, then board ID and name.
    pub elements: Vec<ElementLineDto>,
    /// Day out of days: one grid per category with anyone working or on an event, in
    /// category order. Each row carries its summary.
    pub dood: Vec<DoodPartDto>,
    /// Daily category totals.
    pub totals: Option<TotalsDto>,
    /// The scene columns (the board's strip layout for the one-line reports, else the
    /// report's columns or its kind's); each scene line's `cells` follow them.
    pub layout: Vec<LayoutColumnDto>,
    /// Breakdown by strip: a block per sheet.
    pub blocks: Vec<BlockDto>,
    /// Element detail.
    pub details: Vec<DetailDto>,
    /// The vertical one-line.
    pub vertical: Option<VerticalDto>,
    /// The multi-unit one-line: each date with its units' days, and the sub-boards left out.
    pub units: Vec<UnitDateDto>,
    pub undated: Vec<String>,
    /// The calendar: month grids, and the weekday headings (`Sun` or `Mon` first).
    pub months: Vec<MonthDto>,
    pub week: Vec<&'static str>,
    /// The conflict and red flag reports: the tabs' data.
    pub conflicts: Option<crate::conflicts::ConflictsDto>,
    pub red_flags: Option<crate::redflags::RedFlagsDto>,
}

/// A report's header as it prints: the layout's looks and its rows, each item's text
/// worked out (`today` and `start_date` are dated by the page, in the user's format).
#[derive(Clone, Debug, Serialize)]
pub struct HeadDto {
    pub include: bool,
    pub every_page: bool,
    pub bold: bool,
    pub italic: bool,
    /// `left`, `center` or `right`.
    pub align: &'static str,
    /// `small`, `normal` or `large`.
    pub size: &'static str,
    pub rows: Vec<Vec<HeadItemDto>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeadItemDto {
    /// The item's name: `title`, `space`, `text`, `today`, `start_date`, a production key...
    pub kind: &'static str,
    /// What it prints; empty for a space, today's date, and a field with nothing in it.
    pub text: String,
}

/// The header `setup` prints with on `board` and `sub_board` starting `start`.
pub fn head_of(
    s: &Schedule,
    setup: &ReportSetup,
    board: &str,
    sub_board: &str,
    start: Option<Date>,
) -> HeadDto {
    use method_schedule_core::headers::HeaderItem;
    let h = s.header_of(setup.options.header);
    let text = |i: &HeaderItem| -> String {
        match i {
            HeaderItem::Space | HeaderItem::Today => String::new(),
            HeaderItem::Text(t) => t.clone(),
            HeaderItem::Title => {
                if s.title.trim().is_empty() {
                    "Untitled".to_string()
                } else {
                    s.title.clone()
                }
            }
            HeaderItem::ReportName => setup.name.clone(),
            HeaderItem::Board => board.to_string(),
            HeaderItem::SubBoard => sub_board.to_string(),
            HeaderItem::StartDate => start.map_or(String::new(), |d| d.to_string()),
            HeaderItem::Production(f) => {
                let v = s.production.get(f).unwrap_or_default();
                match *f {
                    _ if v.is_empty() => String::new(),
                    "company" | "custom_1" | "custom_2" | "custom_3" | "custom_4" => v.to_string(),
                    other => format!(
                        "{} {v}",
                        method_schedule_core::setup::Production::label(other)
                    ),
                }
            }
        }
    };
    HeadDto {
        include: h.include,
        every_page: !h.first_page_only,
        bold: h.bold,
        italic: h.italic,
        align: h.align.name(),
        size: h.size.name(),
        rows: h
            .rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|i| HeadItemDto {
                        kind: i.name(),
                        text: text(i),
                    })
                    .collect()
            })
            .collect(),
    }
}

/// Which week `d` falls in, weeks starting Sunday (or Monday with `monday`).
pub fn week_of(d: Date, monday: bool) -> i32 {
    // Day 0 (1970-01-01) was a Thursday: four days after a Sunday, three after a Monday.
    (d.0 + if monday { 3 } else { 4 }).div_euclid(7)
}

/// Whether each of `dates` starts a new page under `brk`: every one after the first for
/// a page per day; for a page per week, one whose week differs from the dated one before.
fn new_pages(dates: &[Option<Date>], brk: PageBreak, monday: bool) -> Vec<bool> {
    let mut last: Option<i32> = None;
    dates
        .iter()
        .enumerate()
        .map(|(k, d)| match brk {
            PageBreak::None => false,
            PageBreak::Day => k > 0,
            PageBreak::Week => match d {
                Some(d) => {
                    let w = week_of(*d, monday);
                    let new = last.is_some_and(|l| l != w);
                    last = Some(w);
                    new
                }
                None => false,
            },
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportDayDto {
    /// Starts a new page (a page per day or per week).
    pub new_page: bool,
    pub number: u32,
    /// `YYYY-MM-DD`, when the sub-board has a start date.
    pub date: Option<String>,
    /// `Mon` to `Sun`, with the date.
    pub weekday: Option<&'static str>,
    /// The day's page length (`2 6/8`) and shoot time (`h:mm`).
    pub pages: String,
    pub time: String,
    pub scenes: usize,
    pub lines: Vec<ReportLineDto>,
    /// Breakdown by day: each category's elements over the day, `ID NAME (n)` with how
    /// many of the day's scenes list it.
    pub groups: Vec<ReportGroupDto>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ReportLineDto {
    /// `scene` or `banner`.
    pub kind: &'static str,
    pub scenes: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    pub pages: String,
    pub synopsis: String,
    /// Shooting schedule and breakdown: the location.
    pub location: String,
    /// Cast board IDs (or names), in board ID order.
    pub cast: Vec<String>,
    /// The scene's text for each column of `layout`.
    pub cells: Vec<String>,
    /// The sheet's elements by category, in category order, each as `ID NAME` or its name
    /// alone. Empty categories (and those the filter leaves out) are left out.
    pub groups: Vec<ReportGroupDto>,
    /// A banner's text and length (`h:mm`).
    pub text: String,
    pub duration: String,
    /// The strip's color: `int-day`, `ext-day`, `int-night` or `ext-night`.
    pub tone: &'static str,
    /// Red flag clashes on the strip, in words.
    pub flags: Vec<String>,
    /// A scene a script revision left out (decisions 69 and 91): listed, struck. Left out
    /// when false.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub omitted: bool,
    /// A banner's look, as the board's strips carry it (view.rs, StripDto): bold, its
    /// palette color's name, two lines high, a size step and an alignment; left out when
    /// plain.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_color: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tall: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_size: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_align: &'static str,
    /// Its font family (decisions.md, entry 187); left out for the usual.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub banner_font: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReportGroupDto {
    pub category: String,
    pub elements: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementLineDto {
    pub category: String,
    pub board_id: String,
    pub name: String,
    /// Sheets it is on anywhere in the schedule, and their page length.
    pub sheets: u32,
    pub pages: String,
    /// Work days on the sub-board, and its first and last.
    pub days: u32,
    pub start: Option<String>,
    pub finish: Option<String>,
    /// Its custom property values, each `Name: value`, in the category's order.
    pub properties: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoodPartDto {
    pub category: String,
    pub grid: DoodGridDto,
}

#[derive(Clone, Debug, Serialize)]
pub struct TotalsDto {
    /// The categories with at least one element on some shoot day, in category order.
    pub categories: Vec<String>,
    /// How many categories were left out because no shoot day uses them.
    pub left_out: usize,
    pub days: Vec<TotalsDayDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TotalsDayDto {
    pub number: u32,
    pub date: Option<String>,
    pub weekday: Option<&'static str>,
    /// One count per entry in `categories`: how many different elements of it the day
    /// uses, or for a category that uses daily quantities, the sum of their quantities.
    pub counts: Vec<u32>,
    /// One per count: a partial total, leaving out a quantity a sheet marks not to count
    /// daily (shown with an asterisk).
    pub partial: Vec<bool>,
}

/// A sheet of the breakdown by strip, with its day.
#[derive(Clone, Debug, Serialize)]
pub struct BlockDto {
    /// Starts a new page (a page per day or per week).
    pub new_page: bool,
    pub day: Option<u32>,
    pub date: Option<String>,
    pub weekday: Option<&'static str>,
    pub line: ReportLineDto,
}

/// One element of the element detail.
#[derive(Clone, Debug, Serialize)]
pub struct DetailDto {
    pub category: String,
    /// `ID NAME`, or the name.
    pub name: String,
    pub work_dates: Vec<String>,
    pub scenes: Vec<BlockDto>,
}

/// The vertical one-line (behavior note 23): the stripboard on its side, a column per
/// scene and a dark column per day break, with the production info and a row per cast
/// member (board ID order) on the left of every page; no cover page.
#[derive(Clone, Debug, Serialize)]
pub struct VerticalDto {
    /// The rows: board ID (or empty) and name.
    pub cast: Vec<(String, String)>,
    pub columns: Vec<VerticalColumnDto>,
    /// Producer, director, assistant director and script date, as label and value, the
    /// ones filled in (the left of each page).
    pub production: Vec<(&'static str, String)>,
}

/// A day's totals, on its last scene: the page draws the day break column after it.
#[derive(Clone, Debug, Serialize)]
pub struct VerticalEndDto {
    pub pages: String,
    /// `h:mm`.
    pub time: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerticalColumnDto {
    pub day: u32,
    pub date: Option<String>,
    pub weekday: Option<&'static str>,
    /// The day's first scene: the page draws the day's heading over it.
    pub first: bool,
    pub scene: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    pub pages: String,
    /// The rows (indexes into `cast`) the scene lists.
    pub rows: Vec<usize>,
    /// On the day's last scene: the day's totals, for its day break column.
    pub end: Option<VerticalEndDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitDateDto {
    pub date: String,
    pub weekday: &'static str,
    pub units: Vec<UnitDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitDto {
    pub name: String,
    pub day: u32,
    pub pages: String,
    pub time: String,
    pub lines: Vec<ReportLineDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MonthDto {
    /// `March 2027`.
    pub title: String,
    pub weeks: Vec<Vec<CalDayDto>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalDayDto {
    pub date: String,
    pub day: u32,
    /// In the month (the grid shows a few days of the months around it), and in the
    /// report's range.
    pub in_month: bool,
    /// `work`, `off`, `holiday` or `move`.
    pub kind: &'static str,
    pub shoot_day: Option<u32>,
    /// The day's scene numbers.
    pub scenes: Vec<String>,
    pub pages: String,
    /// The estimated length of the day (`h:mm`).
    pub time: String,
    pub events: Vec<String>,
    pub flags: Vec<String>,
}

fn day_date(d: Option<Date>) -> (Option<String>, Option<&'static str>) {
    (d.map(|x| x.to_string()), d.map(weekday))
}

fn category_name(s: &Schedule, id: CategoryId) -> String {
    s.category(id).map_or(String::new(), |c| c.name.clone())
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

/// How a report shows a scene: its columns, cast by name or ID, the category filter, the
/// red flag clashes by sheet.
struct Lines<'a> {
    s: &'a Schedule,
    o: &'a ReportOptions,
    columns: Vec<StripColumn>,
    cast: Option<CategoryId>,
    flags: HashMap<SheetId, Vec<String>>,
}

impl Lines<'_> {
    fn cast_list(&self, h: &Sheet) -> Vec<String> {
        if self.o.cast_names {
            let mut els: Vec<_> = h
                .elements
                .iter()
                .filter_map(|e| self.s.element(*e))
                // Left off the board: no name either, as no ID (decisions 120 and 128).
                .filter(|e| Some(e.category) == self.cast && !e.exclude_from_board)
                .collect();
            els.sort_by(|a, b| {
                reports::id_order(a.board_id.as_deref(), b.board_id.as_deref())
                    .then_with(|| a.name.cmp(&b.name))
            });
            els.into_iter().map(|e| e.name.clone()).collect()
        } else {
            reports::cast_ids(self.s, h, self.cast)
        }
    }

    fn cell(&self, h: &Sheet, c: &StripColumn) -> String {
        match c.field {
            StripField::Cast => self.cast_list(h).join(", "),
            _ => reports::report_cell(self.s, h, c, self.cast),
        }
    }

    /// The sheet's elements of category `c`, as the cast column shows cast (names or
    /// board IDs) for the cast, else `ID NAME` each, in board ID order.
    fn elements_of(&self, h: &Sheet, c: CategoryId) -> String {
        if Some(c) == self.cast {
            return self.cast_list(h).join(", ");
        }
        report_data::elements_by_category(self.s, &h.elements)
            .into_iter()
            .find(|(x, _)| *x == c)
            .map(|(_, els)| {
                els.into_iter()
                    .map(|e| element_text(self.s, e))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    }

    fn groups(&self, h: &Sheet) -> Vec<ReportGroupDto> {
        report_data::elements_by_category(self.s, &h.elements)
            .into_iter()
            .filter(|(c, _)| self.o.shows_category(*c))
            .map(|(c, els)| ReportGroupDto {
                category: category_name(self.s, c),
                elements: els.into_iter().map(|e| element_text(self.s, e)).collect(),
            })
            .collect()
    }

    fn scene(&self, sheet: SheetId) -> ReportLineDto {
        let Some(h) = self.s.sheet(sheet) else {
            return ReportLineDto::default();
        };
        ReportLineDto {
            kind: "scene",
            scenes: h.scenes.clone(),
            int_ext: h.int_ext.clone(),
            set: h.set.clone(),
            time_of_day: h.time_of_day.clone(),
            pages: h.pages.to_string(),
            synopsis: if self.o.synopsis {
                h.synopsis.clone()
            } else {
                String::new()
            },
            location: h.location.clone(),
            cast: self.cast_list(h),
            cells: self
                .columns
                .iter()
                .map(|c| self.cell(h, c))
                .chain(
                    self.o
                        .element_columns
                        .iter()
                        .filter(|c| self.s.category(**c).is_some())
                        .map(|c| self.elements_of(h, *c)),
                )
                .collect(),
            groups: self.groups(h),
            tone: tone(h),
            omitted: self.s.plan.is_omitted(sheet),
            flags: if self.o.icons {
                self.flags.get(&sheet).cloned().unwrap_or_default()
            } else {
                Vec::new()
            },
            ..ReportLineDto::default()
        }
    }

    fn line(&self, l: &OneLine) -> Option<ReportLineDto> {
        match l {
            OneLine::Scene { sheet, .. } => Some(self.scene(*sheet)),
            OneLine::Banner {
                text,
                duration,
                style,
            } => self
                .o
                .banners
                .then(|| banner(text.clone(), *duration, *style)),
        }
    }
}

/// The red flag clashes on a sub-board's sheets, in words.
fn flags_by_sheet(
    s: &Schedule,
    b: &method_schedule_core::model::Board,
    sb: &method_schedule_core::model::SubBoard,
) -> HashMap<SheetId, Vec<String>> {
    let marks = crate::redflags::board_marks(s, b);
    let mut out = HashMap::new();
    for st in &sb.strips {
        if let (StripKind::Sheet(h), Some(texts)) = (&st.kind, marks.strips.get(&st.id.0)) {
            out.insert(*h, texts.clone());
        }
    }
    out
}

/// The data for report `index` of the report list on sub-board `sub`. `week_start` is the
/// user's (`sun` or `mon`) for the calendar.
pub fn build_report(
    s: &Schedule,
    sub: u32,
    index: usize,
    week_start: &str,
) -> Result<ReportDto, String> {
    let list = s.report_list();
    let setup = list
        .get(index)
        .ok_or("That report is no longer in the list.")?;
    build(s, sub, index, setup, week_start)
}

/// The data for `setup` (a report of the list, or one made up for a test).
pub fn build(
    s: &Schedule,
    sub: u32,
    index: usize,
    setup: &ReportSetup,
    week_start: &str,
) -> Result<ReportDto, String> {
    let o = &setup.options;
    let (b, sb) = crate::view::sub_ref(s, sub)?;
    let range = match (o.from, o.to) {
        (None, None) => None,
        (Some(f), None) => Some(format!("From {f}")),
        (None, Some(t)) => Some(format!("Up to {t}")),
        (Some(f), Some(t)) => Some(format!("{f} to {t}")),
    };
    let mut out = ReportDto {
        report: setup.kind.name(),
        index,
        name: setup.name.clone(),
        title: s.title.clone(),
        board: b.name.clone(),
        sub_board: sb.name.clone(),
        start: sb.start.map(|d| d.to_string()),
        empty: None,
        head: head_of(s, setup, &b.name, &sb.name, sb.start),
        text: o.text.name(),
        landscape: setup.landscape(),
        range,
        synopsis: o.synopsis,
        banners: o.banners,
        shoot_days: o.shoot_days,
        day_breaks: o.day_breaks,
        total: None,
        colors: o.colors,
        icons: o.icons,
        weekdays_only: o.weekdays_only,
        filtered: s
            .categories
            .iter()
            .filter(|c| !o.shows_category(c.id))
            .map(|c| c.name.clone())
            .collect(),
        days: Vec::new(),
        elements: Vec::new(),
        dood: Vec::new(),
        totals: None,
        layout: Vec::new(),
        blocks: Vec::new(),
        details: Vec::new(),
        vertical: None,
        units: Vec::new(),
        undated: Vec::new(),
        months: Vec::new(),
        week: Vec::new(),
        conflicts: None,
        red_flags: None,
    };
    let no_days = || {
        format!(
            "There are no shoot days on {} yet. Put day breaks on the Board first.",
            sb.name
        )
    };
    let none_in_range = "No shoot day falls in the report's date range.".to_string();
    let shoot_days = days(s, b.id, sb.id).map_err(|e| e.to_string())?;
    // Scene and page totals for each day, as the Board shows them.
    let day_head = |number: u32, date: Option<Date>| {
        let d = shoot_days.days.iter().find(|x| x.number == number);
        let (date, wd) = day_date(date);
        ReportDayDto {
            new_page: false,
            number,
            date,
            weekday: wd,
            pages: d.map_or(String::new(), |x| x.pages.to_string()),
            time: d.map_or(String::new(), |x| hm(x.time)),
            scenes: 0,
            lines: Vec::new(),
            groups: Vec::new(),
        }
    };
    // The one-line report's own named layout, else the board's (decisions.md, entry 78),
    // its lines, spans and looks kept (entry 87); a report's own columns keep theirs too,
    // with a text size each (the report designer, entry 90), at the board's widths.
    // A report may follow a named strip layout of its own (decision 100).
    let picked = o.layout.and_then(|id| match s.layouts.get(id) {
        Some(method_schedule_core::layouts::SavedLayout {
            body: method_schedule_core::layouts::LayoutBody::Strips(l),
            ..
        }) => Some(l.clone()),
        _ => None,
    });
    let lay = if let Some(l) = &picked {
        l.clone()
    } else if matches!(setup.kind, ReportKind::OneLine) {
        s.one_line_layout_of(b.id)
    } else {
        s.strip_layout_of(b.id)
    };
    let cols: Vec<StripColumn> = if !o.columns.is_empty() {
        o.columns
            .iter()
            .map(|r| {
                r.strip_column(
                    lay.columns
                        .iter()
                        .find(|c| c.field == r.field)
                        .map_or(r.field.default_width(), |c| c.width),
                )
            })
            .collect()
    } else if picked.is_some() || matches!(setup.kind, ReportKind::OneLine | ReportKind::MultiUnit)
    {
        lay.columns.clone()
    } else {
        setup
            .kind
            .default_columns()
            .iter()
            .map(|f| StripColumn::of(*f))
            .collect()
    };
    // A report's element count counts every element (decision 136); the board's
    // category pick stays on the board (decision 147). A category's elements keeps its
    // category and switch, so a report that follows a layout lists them as the board
    // does (decision 194).
    let cols: Vec<StripColumn> = cols
        .into_iter()
        .map(|c| StripColumn {
            category: c
                .category
                .filter(|_| c.field == StripField::CategoryElements),
            ..c
        })
        .collect();
    out.layout = view::layout_dto(s, &StripLayout::of(cols.clone()));
    for (dto, c) in out.layout.iter_mut().zip(&o.columns) {
        dto.size = c.size.map_or("", |t| t.name());
    }
    // A column per picked category after the scene fields: the scene's elements of it.
    for c in &o.element_columns {
        if let Some(cat) = s.category(*c) {
            out.layout.push(LayoutColumnDto {
                field: "elements",
                label: cat.name.clone(),
                width: "medium",
                line: 1,
                span: 1,
                align: "auto",
                bold: false,
                italic: false,
                text: String::new(),
                size: "",
                category: None,
                board_ids: false,
                rule_after: false,
                rule_under: false,
            });
        }
    }
    let lines = Lines {
        s,
        o,
        columns: cols,
        cast: s.cast(),
        flags: if o.icons {
            flags_by_sheet(s, b, sb)
        } else {
            HashMap::new()
        },
    };
    let had_days = !shoot_days.days.is_empty();
    match setup.kind {
        ReportKind::OneLine => {
            for day in reports::one_line(s, b.id, sb.id, s.cast()).map_err(|e| e.to_string())? {
                if !o.in_range(day.date) {
                    continue;
                }
                let mut d = day_head(day.number, day.date);
                d.lines = day.lines.iter().filter_map(|l| lines.line(l)).collect();
                d.scenes = d.lines.iter().filter(|l| l.kind == "scene").count();
                out.days.push(d);
            }
            if out.days.is_empty() {
                out.empty = Some(if had_days { none_in_range } else { no_days() });
            }
        }
        ReportKind::Shooting => {
            for day in reports::shooting_schedule(s, b.id, sb.id).map_err(|e| e.to_string())? {
                if !o.in_range(day.date) {
                    continue;
                }
                let mut d = day_head(day.number, day.date);
                for line in day.lines {
                    match line {
                        ShootingLine::Scene(x) => d.lines.push(lines.scene(x.sheet)),
                        ShootingLine::Banner {
                            text,
                            duration,
                            style,
                        } if o.banners => d.lines.push(banner(text, duration, style)),
                        ShootingLine::Banner { .. } => {}
                    }
                }
                d.scenes = d.lines.iter().filter(|l| l.kind == "scene").count();
                out.days.push(d);
            }
            if out.days.is_empty() {
                out.empty = Some(if had_days { none_in_range } else { no_days() });
            }
        }
        ReportKind::Breakdown => {
            for blk in report_data::breakdown_by_strip(s, b.id, sb.id).map_err(|e| e.to_string())? {
                if (o.from.is_some() || o.to.is_some())
                    && (blk.slot.date.is_none() || !o.in_range(blk.slot.date))
                {
                    continue;
                }
                let (date, wd) = day_date(blk.slot.date);
                out.blocks.push(BlockDto {
                    new_page: false,
                    day: blk.slot.day,
                    date,
                    weekday: wd,
                    line: lines.scene(blk.sheet),
                });
            }
            if out.blocks.is_empty() {
                out.empty = Some(
                    if sb
                        .strips
                        .iter()
                        .any(|x| matches!(x.kind, StripKind::Sheet(_)))
                    {
                        none_in_range
                    } else {
                        format!("{} has no scenes yet.", sb.name)
                    },
                );
            }
        }
        ReportKind::BreakdownDay => {
            for day in report_data::breakdown_by_day(s, b.id, sb.id).map_err(|e| e.to_string())? {
                if !o.in_range(day.date) {
                    continue;
                }
                let mut d = day_head(day.number, day.date);
                d.lines = day.sheets.iter().map(|h| lines.scene(*h)).collect();
                d.scenes = d.lines.len();
                d.groups = day
                    .elements
                    .iter()
                    .filter(|(c, _)| o.shows_category(*c))
                    .map(|(c, els)| ReportGroupDto {
                        category: category_name(s, *c),
                        elements: els
                            .iter()
                            .map(|(e, n)| {
                                // The day's quantity of a daily-quantity element (decision 63).
                                match day.needed.iter().find(|(x, _)| x == e) {
                                    Some((_, q)) => {
                                        let star = if day.partial.contains(e) { "*" } else { "" };
                                        format!("{} x{q}{star} ({n})", element_text(s, *e))
                                    }
                                    None => format!("{} ({n})", element_text(s, *e)),
                                }
                            })
                            .collect(),
                    })
                    .collect();
                out.days.push(d);
            }
            if out.days.is_empty() {
                out.empty = Some(if had_days { none_in_range } else { no_days() });
            }
        }
        ReportKind::Dood => {
            let mut why = None;
            let opts = view::GridOpts {
                no_holds: o.no_holds,
                sort: Vec::new(),
                from: o.from,
                to: o.to,
                elements: o.elements.clone(),
                // Whole weeks from the week's first day, also before the range's start
                // (behavior note 27, decision 141). The page leaves weekends out when the
                // DOOD tab hides them.
                week_start: Some(if week_start == "mon" {
                    Weekday::Mon
                } else {
                    Weekday::Sun
                }),
            };
            for c in s.categories.iter().filter(|c| o.shows_category(c.id)) {
                let mut grid = view::dood_grid_with(s, sub, Some(c.id.0), &opts)?;
                if grid.rows.is_empty() {
                    why = why.or(grid.empty.take());
                    continue;
                }
                // The picker's list is the tab's business, not the report's.
                grid.categories.clear();
                out.dood.push(DoodPartDto {
                    category: c.name.clone(),
                    grid,
                });
            }
            if out.dood.is_empty() {
                // Rows there are, outside the range: say the range is why.
                let ranged = (o.from.is_some() || o.to.is_some())
                    && method_schedule_core::dood::dood(s, b.id, sb.id)
                        .is_ok_and(|rows| !rows.is_empty());
                out.empty = Some(match (sb.start, why) {
                    (None, Some(w)) => w,
                    _ if ranged => none_in_range.clone(),
                    _ => "No element works on a shoot day or has an event yet.".to_string(),
                });
            }
        }
        ReportKind::Elements => {
            out.elements = reports::element_list(s, b.id, sb.id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|l| o.shows_category(l.category))
                .filter(|l| o.elements.is_empty() || o.elements.contains(&l.element))
                .map(|l| ElementLineDto {
                    properties: s.element(l.element).map_or(Vec::new(), |e| {
                        e.properties
                            .iter()
                            .map(|p| format!("{}: {}", p.name, p.value))
                            .collect()
                    }),
                    category: category_name(s, l.category),
                    board_id: l.board_id.unwrap_or_default(),
                    name: l.name,
                    sheets: l.sheets,
                    pages: l.pages.to_string(),
                    days: l.days,
                    start: l.start.map(|d| d.to_string()),
                    finish: l.finish.map(|d| d.to_string()),
                })
                .collect();
            if out.elements.is_empty() {
                out.empty = Some("The schedule has no elements yet.".to_string());
            }
        }
        ReportKind::ElementDetail => {
            let keep = |e: ElementId, c: CategoryId| {
                o.shows_category(c) && (o.elements.is_empty() || o.elements.contains(&e))
            };
            for d in report_data::element_detail(s, b.id, sb.id, keep).map_err(|e| e.to_string())? {
                let scenes: Vec<BlockDto> = d
                    .scenes
                    .iter()
                    .filter(|(_, slot)| {
                        (o.from.is_none() && o.to.is_none())
                            || (slot.date.is_some() && o.in_range(slot.date))
                    })
                    .map(|(h, slot)| {
                        let (date, wd) = day_date(slot.date);
                        BlockDto {
                            new_page: false,
                            day: slot.day,
                            date,
                            weekday: wd,
                            line: lines.scene(*h),
                        }
                    })
                    .collect();
                if scenes.is_empty() {
                    continue;
                }
                out.details.push(DetailDto {
                    category: category_name(s, d.category),
                    name: element_text(s, d.element),
                    work_dates: d
                        .work_dates
                        .iter()
                        .filter(|x| o.in_range(Some(**x)))
                        .map(|x| x.to_string())
                        .collect(),
                    scenes,
                });
            }
            if out.details.is_empty() {
                out.empty = Some(format!("No element is on a scene of {} yet.", sb.name));
            }
        }
        ReportKind::Totals => {
            let rows: Vec<_> = reports::daily_category_totals(s, b.id, sb.id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|r| o.in_range(r.date))
                .collect();
            // Keep the categories some day uses; their columns stay in schedule order.
            let used: Vec<usize> = (0..s.categories.len())
                .filter(|&i| o.shows_category(s.categories[i].id))
                .filter(|&i| {
                    rows.iter()
                        .any(|r| r.counts.get(i).is_some_and(|c| c.1 > 0))
                })
                .collect();
            let shown = s
                .categories
                .iter()
                .filter(|c| o.shows_category(c.id))
                .count();
            let totals = TotalsDto {
                categories: used.iter().map(|&i| s.categories[i].name.clone()).collect(),
                left_out: shown - used.len(),
                days: rows
                    .iter()
                    .map(|r| {
                        let (date, wd) = day_date(r.date);
                        TotalsDayDto {
                            number: r.number,
                            date,
                            weekday: wd,
                            counts: used
                                .iter()
                                .map(|&i| r.counts.get(i).map_or(0, |c| c.1))
                                .collect(),
                            partial: used
                                .iter()
                                .map(|&i| r.partial.contains(&s.categories[i].id))
                                .collect(),
                        }
                    })
                    .collect(),
            };
            if totals.days.is_empty() {
                out.empty = Some(if had_days { none_in_range } else { no_days() });
            }
            out.totals = Some(totals);
        }
        ReportKind::Calendar => calendar(s, b, sb, o, week_start, &mut out)?,
        ReportKind::Vertical => {
            let v = report_data::vertical(s, b.id, sb.id, s.cast()).map_err(|e| e.to_string())?;
            let mut columns = Vec::new();
            for d in v.days.iter().filter(|d| o.in_range(d.date)) {
                let (date, wd) = day_date(d.date);
                let n = d.scenes.len();
                for (k, (h, rows)) in d.scenes.iter().enumerate() {
                    let Some(x) = s.sheet(*h) else { continue };
                    columns.push(VerticalColumnDto {
                        end: (k + 1 == n).then(|| VerticalEndDto {
                            pages: d.pages.to_string(),
                            time: hm(d.time),
                        }),
                        day: d.number,
                        date: date.clone(),
                        weekday: wd,
                        first: k == 0,
                        scene: x.scenes.clone(),
                        int_ext: x.int_ext.clone(),
                        set: x.set.clone(),
                        time_of_day: x.time_of_day.clone(),
                        pages: x.pages.to_string(),
                        rows: rows.clone(),
                    });
                }
            }
            if columns.is_empty() {
                out.empty = Some(if had_days { none_in_range } else { no_days() });
            }
            out.vertical = Some(VerticalDto {
                cast: v
                    .cast
                    .iter()
                    .filter_map(|e| s.element(*e))
                    .map(|e| (e.board_id.clone().unwrap_or_default(), e.name.clone()))
                    .collect(),
                columns,
                production: [
                    ("Producer", &s.production.producer),
                    ("Director", &s.production.director),
                    ("Assistant director", &s.production.assistant_director),
                    ("Script date", &s.production.script_date),
                ]
                .into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(k, v)| (k, v.trim().to_string()))
                .collect(),
            });
        }
        ReportKind::MultiUnit => {
            let (dates, undated) = report_data::multi_unit(s, b.id, s.cast(), &o.elements)
                .map_err(|e| e.to_string())?;
            for (date, units) in dates {
                if !o.in_range(Some(date)) {
                    continue;
                }
                out.units.push(UnitDateDto {
                    date: date.to_string(),
                    weekday: weekday(date),
                    units: units
                        .iter()
                        .map(|u| {
                            let unit_lines = Lines {
                                flags: if o.icons {
                                    b.sub_boards
                                        .iter()
                                        .find(|x| x.id == u.sub_board)
                                        .map(|x| flags_by_sheet(s, b, x))
                                        .unwrap_or_default()
                                } else {
                                    HashMap::new()
                                },
                                columns: lines.columns.clone(),
                                ..lines
                            };
                            UnitDto {
                                name: b
                                    .sub_boards
                                    .iter()
                                    .find(|x| x.id == u.sub_board)
                                    .map_or(String::new(), |x| x.name.clone()),
                                day: u.number,
                                pages: u.pages.to_string(),
                                time: hm(u.time),
                                lines: u.lines.iter().filter_map(|l| unit_lines.line(l)).collect(),
                            }
                        })
                        .collect(),
                });
            }
            out.undated = undated
                .iter()
                .filter_map(|id| b.sub_boards.iter().find(|x| x.id == *id))
                .map(|x| x.name.clone())
                .collect();
            if out.units.is_empty() {
                out.empty = Some(
                    "No sub-board of this board has a dated shoot day in the report's range."
                        .to_string(),
                );
            }
        }
        ReportKind::Conflicts => {
            let mut c = crate::conflicts::conflicts_view(s, b.id.0)?;
            let none = |c: &crate::conflicts::ConflictsDto| {
                c.conflicts.is_empty() && c.on_work.is_empty() && c.event_clashes.is_empty()
            };
            let any = !none(&c);
            let keep = |date: &str| o.in_range(date.parse::<Date>().ok());
            c.conflicts.retain(|x| keep(&x.date));
            c.on_work.retain(|x| keep(&x.date));
            c.event_clashes.retain(|x| keep(&x.date));
            if none(&c) {
                out.empty = Some(if any {
                    "No conflict falls in the report's date range.".to_string()
                } else {
                    format!("{} has no conflicts.", b.name)
                });
            }
            out.conflicts = Some(c);
        }
        ReportKind::RedFlags => {
            let mut r = crate::redflags::red_flags_view(s, sub)?;
            r.clashes.retain(|x| {
                o.in_range(x.date.parse::<Date>().ok())
                    && (o.elements.is_empty() || o.elements.contains(&ElementId(x.element)))
                    && o.shows_category(CategoryId(x.category))
            });
            // Every flag, the Red flags tab's table (behavior note 23), with the same
            // filters: a flag shows when its dates meet the range.
            r.rows.retain(|x| {
                let (start, end) = (
                    x.start.as_deref().and_then(|d| d.parse::<Date>().ok()),
                    x.end.as_deref().and_then(|d| d.parse::<Date>().ok()),
                );
                o.to.is_none_or(|t| start.is_none_or(|a| a <= t))
                    && o.from.is_none_or(|f| end.is_none_or(|b| b >= f))
                    && (o.elements.is_empty() || o.elements.contains(&ElementId(x.element)))
                    && o.shows_category(CategoryId(x.category))
            });
            r.categories.clear();
            if r.clashes.is_empty() && r.rows.is_empty() {
                out.empty = Some(if r.flags == 0 {
                    "The schedule has no red flags yet.".to_string()
                } else {
                    format!(
                        "No red flags or clashes on {} meet the report's filters.",
                        sb.name
                    )
                });
            }
            out.red_flags = Some(r);
        }
    }
    // Page breaks per day or per week, and the total line (decision 100).
    let monday = week_start == "mon";
    let dates: Vec<Option<Date>> = out
        .days
        .iter()
        .map(|d| d.date.as_deref().and_then(|x| x.parse().ok()))
        .collect();
    for (d, new) in out
        .days
        .iter_mut()
        .zip(new_pages(&dates, o.page_break, monday))
    {
        d.new_page = new;
    }
    let dates: Vec<Option<Date>> = out
        .blocks
        .iter()
        .map(|b| b.date.as_deref().and_then(|x| x.parse().ok()))
        .collect();
    for (b, new) in out
        .blocks
        .iter_mut()
        .zip(new_pages(&dates, o.page_break, monday))
    {
        b.new_page = new;
    }
    if o.total_count {
        let plural = |n: usize, one: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {one}s")
            }
        };
        if !out.days.is_empty() {
            let numbers: std::collections::HashSet<u32> =
                out.days.iter().map(|d| d.number).collect();
            let pages = shoot_days
                .days
                .iter()
                .filter(|d| numbers.contains(&d.number))
                .fold(method_schedule_core::eighths::Eighths(0), |a, d| {
                    a + d.pages
                });
            let scenes: usize = out.days.iter().map(|d| d.scenes).sum();
            out.total = Some(format!(
                "Total: {} on {}, {pages} pages",
                plural(scenes, "scene"),
                plural(out.days.len(), "shoot day")
            ));
        } else if !out.blocks.is_empty() {
            out.total = Some(format!("Total: {}", plural(out.blocks.len(), "scene")));
        }
    }
    Ok(out)
}

/// The calendar report: a month grid per month from the range's start (or the first
/// shoot day) to its end (or the last), at most two years.
fn calendar(
    s: &Schedule,
    b: &method_schedule_core::model::Board,
    sb: &method_schedule_core::model::SubBoard,
    o: &ReportOptions,
    week_start: &str,
    out: &mut ReportDto,
) -> Result<(), String> {
    let monday = week_start == "mon";
    let names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let order: Vec<usize> = if monday {
        vec![1, 2, 3, 4, 5, 6, 0]
    } else {
        vec![0, 1, 2, 3, 4, 5, 6]
    };
    let keep = |i: usize| !o.weekdays_only || (1..=5).contains(&i);
    out.week = order
        .iter()
        .copied()
        .filter(|i| keep(*i))
        .map(|i| names[i])
        .collect();
    let span = report_data::shoot_span(s, b.id, sb.id).map_err(|e| e.to_string())?;
    let (Some(from), Some(to)) = (o.from.or(span.map(|x| x.0)), o.to.or(span.map(|x| x.1))) else {
        out.empty = Some(format!(
            "There are no dated shoot days on {} yet: set its start date and put day breaks on the Board, or give the report a date range.",
            sb.name
        ));
        return Ok(());
    };
    if to < from {
        out.empty = Some("No shoot day falls in the report's date range.".to_string());
        return Ok(());
    }
    let (fy, fm, _) = from.ymd();
    let (ty, tm, _) = to.ymd();
    let asked = (ty * 12 + tm as i32) - (fy * 12 + fm as i32) + 1;
    let months = asked.clamp(1, 24);
    if asked > months {
        // At most two years print; the note under the title says which.
        out.range = Some(format!(
            "{from} to {to}; the first 24 months print, a narrower range shows the rest"
        ));
    }
    let first = Date::from_ymd(fy, fm, 1).ok_or("That is not a date.")?;
    // Back to the week's first day on or before the 1st.
    let back = |d: Date| {
        let wd = Weekday::ALL
            .iter()
            .position(|w| *w == d.weekday())
            .unwrap_or(0) as i32; // Mon 0
        let sunday_based = (wd + 1) % 7;
        if monday {
            wd
        } else {
            sunday_based
        }
    };
    let grid_start = first.add_days(-back(first));
    let last_month = {
        let n = fy * 12 + fm as i32 - 1 + months - 1;
        Date::from_ymd(n.div_euclid(12), n.rem_euclid(12) as u32 + 1, 1)
            .ok_or("That is not a date.")?
    };
    let grid_end = {
        let (y, m, _) = last_month.ymd();
        let next = Date::from_ymd(
            if m == 12 { y + 1 } else { y },
            if m == 12 { 1 } else { m + 1 },
            1,
        )
        .ok_or("That is not a date.")?;
        next.add_days(6 * 7)
    };
    let dates = report_data::calendar_dates(s, b.id, sb.id, grid_start, grid_end)
        .map_err(|e| e.to_string())?;
    let by_date: HashMap<Date, &report_data::CalendarDate> =
        dates.iter().map(|d| (d.date, d)).collect();
    let flags = if o.icons {
        crate::redflags::by_date(s, b, sb)
    } else {
        HashMap::new()
    };
    for k in 0..months {
        let n = fy * 12 + fm as i32 - 1 + k;
        let (y, m) = (n.div_euclid(12), n.rem_euclid(12) as u32 + 1);
        let first = Date::from_ymd(y, m, 1).ok_or("That is not a date.")?;
        let start = first.add_days(-back(first));
        let mut weeks = Vec::new();
        for w in 0..6 {
            let week_first = start.add_days(w * 7);
            if w > 3 && week_first.ymd().1 != m {
                break;
            }
            let mut week = Vec::new();
            for d in 0..7 {
                let date = week_first.add_days(d);
                let i = (Weekday::ALL
                    .iter()
                    .position(|x| *x == date.weekday())
                    .unwrap_or(0)
                    + 1)
                    % 7;
                if !keep(i) {
                    continue;
                }
                let (_, dm, day) = date.ymd();
                let info = by_date.get(&date);
                week.push(CalDayDto {
                    date: date.to_string(),
                    day,
                    in_month: dm == m,
                    kind: match info.map(|x| x.kind) {
                        Some(method_schedule_core::model::DayKind::Off) => "off",
                        Some(method_schedule_core::model::DayKind::Holiday) => "holiday",
                        Some(method_schedule_core::model::DayKind::Move) => "move",
                        _ => "work",
                    },
                    shoot_day: info.and_then(|x| x.shoot_day),
                    scenes: info.map_or_else(Vec::new, |x| {
                        x.sheets
                            .iter()
                            .filter_map(|h| s.sheet(*h))
                            .map(|h| h.scenes.clone())
                            .collect()
                    }),
                    pages: info
                        .filter(|x| x.shoot_day.is_some())
                        .map_or(String::new(), |x| x.pages.to_string()),
                    time: info
                        .filter(|x| x.shoot_day.is_some())
                        .map_or(String::new(), |x| hm(x.time)),
                    events: info.map_or_else(Vec::new, |x| {
                        x.events
                            .iter()
                            .filter_map(|i| sb.calendar.events.get(*i))
                            .map(|e| {
                                let kind = e.kind.label();
                                if e.name.is_empty() {
                                    kind.to_string()
                                } else {
                                    format!("{kind}: {}", e.name)
                                }
                            })
                            .collect()
                    }),
                    flags: flags.get(&date).cloned().unwrap_or_default(),
                });
            }
            weeks.push(week);
        }
        out.months.push(MonthDto {
            title: format!("{} {y}", MONTHS[m as usize - 1]),
            weeks,
        });
    }
    Ok(())
}

fn banner(
    text: String,
    duration: method_schedule_core::model::Minutes,
    style: method_schedule_core::model::BannerStyle,
) -> ReportLineDto {
    use method_schedule_core::model::{Align, TextSize};
    ReportLineDto {
        kind: "banner",
        duration: hm(duration),
        text,
        bold: style.bold,
        banner_color: style
            .color
            .map(method_schedule_core::model::BannerColor::text)
            .unwrap_or_default(),
        tall: style.tall,
        banner_size: if style.size == TextSize::Normal {
            ""
        } else {
            style.size.name()
        },
        banner_align: if style.align == Align::Auto {
            ""
        } else {
            style.align.name()
        },
        banner_font: if style.font == method_schedule_core::model::BannerFont::Usual {
            ""
        } else {
            style.font.name()
        },
        ..ReportLineDto::default()
    }
}

/// Session sort keys from the page (`work:desc`, `name`), at most four.
pub fn parse_sort(
    keys: &[String],
) -> Result<Vec<(method_schedule_core::dood::SortKey, bool)>, String> {
    if keys.len() > 4 {
        return Err("Sort by four keys at most.".to_string());
    }
    keys.iter()
        .map(|k| {
            let (n, desc) = match k.split_once(':') {
                Some((n, "desc")) => (n, true),
                Some((n, "asc")) => (n, false),
                None => (k.as_str(), false),
                Some(_) => return Err(format!("{k:?} is not a sort key.")),
            };
            method_schedule_core::dood::SortKey::from_name(n)
                .map(|x| (x, desc))
                .ok_or_else(|| format!("{n:?} is not a sort key."))
        })
        .collect()
}
