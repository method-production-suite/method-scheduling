//! File > Export's tables and files without a window: which report, its workbook or CSV
//! bytes, and the budget exchange file (docs/EXCHANGE.md). The desktop shows a Save dialog
//! and writes them (method-schedule-app's export.rs); a host offers them as downloads.

use serde::Serialize;

use method_schedule_core::{Date, Schedule};
use method_schedule_format::exchange;
use method_schedule_format::export::{self, csv, xlsx, Workbook, PAGES};

/// The reports File > Export offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// The day out of days summary: every element the sub-board schedules.
    DoodSummary,
    /// The one-line schedule, following the report's strip layout (decision 103).
    OneLine,
    /// The one-line schedule with our stable `day` to `text` columns.
    OneLineFixed,
    Elements,
    /// The day out of days grid, every category (decision 59).
    DoodGrid,
    ElementDetail,
    Breakdown,
    /// The breakdown by shoot day: a row per day and element.
    BreakdownDay,
    /// The whole board of the sub-board, every strip.
    Board,
    /// The daily category totals, with a total row of formulas.
    Totals,
    /// Every red flag in the schedule, the Red flags tab's table (behavior note 23).
    RedFlags,
}

impl Report {
    /// The page's names: `dood`, `one_line`, `elements`.
    pub fn parse(s: &str) -> Option<Report> {
        match s {
            "dood" => Some(Report::DoodSummary),
            "one_line" => Some(Report::OneLine),
            "one_line_fixed" => Some(Report::OneLineFixed),
            "elements" => Some(Report::Elements),
            "dood_grid" => Some(Report::DoodGrid),
            "element_detail" => Some(Report::ElementDetail),
            "breakdown" => Some(Report::Breakdown),
            "breakdown_day" => Some(Report::BreakdownDay),
            "board" => Some(Report::Board),
            "totals" => Some(Report::Totals),
            "red_flags" => Some(Report::RedFlags),
            _ => None,
        }
    }

    /// The part of the suggested file name after the schedule's.
    pub fn file_label(self) -> &'static str {
        match self {
            Report::DoodSummary => "Day Out of Days",
            Report::OneLine | Report::OneLineFixed => "One-Line",
            Report::Elements => "Elements",
            Report::DoodGrid => "Day Out of Days Grid",
            Report::ElementDetail => "Element Detail",
            Report::Breakdown => "Breakdown",
            Report::BreakdownDay => "Breakdown by Day",
            Report::Board => "Board",
            Report::Totals => "Daily Totals",
            Report::RedFlags => "Red Flags",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Xlsx,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Option<Format> {
        match s {
            "xlsx" => Some(Format::Xlsx),
            "csv" => Some(Format::Csv),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Csv => "csv",
        }
    }

    /// The Save dialog's title and filter name.
    pub fn dialog_text(self) -> (&'static str, &'static str) {
        match self {
            Format::Xlsx => ("Export Excel workbook", "Excel workbook"),
            Format::Csv => ("Export CSV file", "CSV file"),
        }
    }
}

/// How an export ended.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ExportOutcome {
    /// False when the Save dialog was cancelled: nothing was written.
    pub done: bool,
    /// The file written, by name (never its folder).
    pub file: Option<String>,
}

/// The board and sub-board ids of sub-board `sub`, or why it isn't one.
pub fn ids(
    s: &Schedule,
    sub: u32,
) -> Result<
    (
        method_schedule_core::model::BoardId,
        method_schedule_core::model::SubBoardId,
    ),
    String,
> {
    let (b, sb) = crate::view::sub_ref(s, sub)?;
    Ok((b.id, sb.id))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect::<String>() + ".",
        None => String::new(),
    }
}

/// `report`'s table for sub-board `sub`. The one-line schedule lists Cast board IDs.
pub fn workbook(s: &Schedule, sub: u32, report: Report) -> Result<Workbook, String> {
    let (b, sb) = ids(s, sub)?;
    let wb = match report {
        Report::DoodSummary => export::dood_summary(s, b, sb),
        Report::OneLine => export::one_line(s, b, sb, s.cast(), export::OneLineColumns::Layout),
        Report::OneLineFixed => export::one_line(s, b, sb, s.cast(), export::OneLineColumns::Fixed),
        Report::Elements => export::element_list(s, b, sb),
        Report::DoodGrid => export::dood_grid(s, b, sb, None),
        Report::ElementDetail => export::element_detail(s, b, sb),
        Report::Breakdown => export::breakdown(s, b, sb),
        Report::BreakdownDay => export::breakdown_day(s, b, sb),
        Report::Board => export::board(s, b),
        Report::Totals => export::daily_totals(s, b, sb),
        Report::RedFlags => Ok(export::red_flags(s)),
    };
    wb.map_err(|e| capitalize(&e.to_string()))
}

/// The file's bytes: the workbook, or its one table as CSV without the page-length text
/// column (it repeats the eighths), with a UTF-8 byte order mark so Excel reads accented
/// letters.
pub fn file_bytes(wb: &Workbook, format: Format) -> Result<Vec<u8>, String> {
    match format {
        Format::Xlsx => xlsx::write(wb).map_err(|e| capitalize(&e.to_string())),
        Format::Csv => {
            let sheet = wb.sheets.first().ok_or("There is nothing to export.")?;
            Ok(csv::write(
                &sheet.without(PAGES),
                csv::Delimiter::Comma,
                true,
            ))
        }
    }
}

/// The budget exchange file for sub-board `sub`, stamped `today`. Refused, with a plain
/// sentence, while the sub-board has no start date.
pub fn budget_text(s: &Schedule, sub: u32, today: Option<Date>) -> Result<String, String> {
    let (b, sb) = ids(s, sub)?;
    if let Some(x) = Some(crate::view::sub_ref(s, sub)?.1).filter(|x| x.start.is_none()) {
        return Err(format!(
            "Set a start date for {} before exporting for a budget: the file counts days by date.",
            x.name
        ));
    }
    exchange::to_string(s, b, sb, today).map_err(|e| capitalize(&e.to_string()))
}

/// A file name stem from a schedule title: path separators and other characters file
/// systems reject become spaces.
pub fn safe_file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        crate::session::UNTITLED.into()
    } else {
        trimmed.to_string()
    }
}
