//! Spreadsheet export: reports as plain tables, written as an Excel workbook (.xlsx) or
//! CSV.
//!
//! The builders in [`tables`] ([`dood_summary`], [`one_line`], [`element_list`]) turn a
//! schedule's reports into a [`Workbook`] of [`Sheet`]s. The writers ([`csv`], [`xlsx`])
//! only serialize those tables, so every format carries the same values.
//!
//! Every number is an integer: day counts and minutes are whole numbers, page lengths
//! are eighths (with the same length as text, `1 3/8`, beside them in a workbook), and
//! dates are ISO `YYYY-MM-DD`. Nothing goes through a float.
//!
//! Everything here is plain std: the .xlsx is a ZIP of XML parts, written by [`zip`]
//! (stored entries, CRC-32) and [`xlsx`] (the SpreadsheetML parts).

pub mod csv;
mod more;
mod tables;
pub mod xlsx;
pub mod zip;

use std::fmt;

use method_schedule_core::model::Error;
use method_schedule_core::Date;

/// The `kind` of a scene a script revision no longer has, in the one-line and board
/// sheets: listed in its place, its length counting nowhere (decision 91).
pub(crate) const OMITTED: &str = "omitted_scene";

pub use more::{
    board, breakdown, breakdown_day, daily_totals, dood_grid, dood_grid_of, element_detail,
    red_flags, shot_against_planned,
};
pub use tables::{
    dood_summary, element_list, one_line, one_line_options, one_line_with, OneLineColumns, BREAKS,
    PAGES,
};

/// One cell's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Text(String),
    /// A whole number (a day number, a count, minutes, eighths).
    Int(u64),
    /// A calendar date. A workbook shows it as a real date (`yyyy-mm-dd`); CSV writes it
    /// as `YYYY-MM-DD`.
    Date(Date),
    /// A total the exporter worked out, written to a workbook as a working formula with
    /// its value cached beside it; CSV writes the value alone.
    Formula(Formula),
}

/// A formula over other cells of the export, with its value worked out here. Only the
/// constructors below make one, from column and row numbers and the sheets' own names,
/// so no text from a schedule ever reaches a formula.
///
/// Rows are data rows counted from zero, as in [`Sheet::rows`]; the header is the
/// workbook's first row, so data row `r` is row `r + 2` in a formula.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formula {
    pub(crate) expr: String,
    pub(crate) value: u64,
}

impl Formula {
    /// `SUM(H2:H9)`: data rows `rows` of column `col` on the same sheet.
    pub fn sum(col: usize, rows: std::ops::Range<usize>, value: u64) -> Cell {
        Formula::sum_in(None, col, rows, value)
    }

    /// `SUM('One-Line'!H2:H9)`: the same over another sheet of the workbook (`None`: this
    /// one). An empty range is a plain 0.
    pub fn sum_in(
        sheet: Option<&'static str>,
        col: usize,
        rows: std::ops::Range<usize>,
        value: u64,
    ) -> Cell {
        match range(sheet, col, rows) {
            Some(r) => Cell::Formula(Formula {
                expr: format!("SUM({r})"),
                value,
            }),
            None => Cell::Int(0),
        }
    }

    /// `COUNTIF('One-Line'!C2:C9,"scene")`: how many of those cells hold `word`, one of the
    /// exporter's own fixed words (`scene`), never schedule text. An empty range is 0.
    pub fn count_if_in(
        sheet: Option<&'static str>,
        col: usize,
        rows: std::ops::Range<usize>,
        word: &'static str,
        value: u64,
    ) -> Cell {
        debug_assert!(word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        let word: String = word
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        match range(sheet, col, rows) {
            Some(r) => Cell::Formula(Formula {
                expr: format!("COUNTIF({r},\"{word}\")"),
                value,
            }),
            None => Cell::Int(0),
        }
    }

    /// `D2+E2`: columns `cols` of data row `row`, added.
    pub fn add(cols: &[usize], row: usize, value: u64) -> Cell {
        if cols.is_empty() {
            return Cell::Int(0);
        }
        let expr = cols
            .iter()
            .map(|c| format!("{}{}", xlsx::column_name(*c), row + 2))
            .collect::<Vec<_>>()
            .join("+");
        Cell::Formula(Formula { expr, value })
    }

    /// The formula as a workbook stores it (no leading `=`).
    pub fn expr(&self) -> &str {
        &self.expr
    }

    /// Its value, worked out when the export was made.
    pub fn value(&self) -> u64 {
        self.value
    }
}

/// `'Sheet'!H2:H9` (or `H2:H9`), or `None` for an empty range.
fn range(sheet: Option<&'static str>, col: usize, rows: std::ops::Range<usize>) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let c = xlsx::column_name(col);
    let r = format!("{c}{}:{c}{}", rows.start + 2, rows.end + 1);
    Some(match sheet {
        Some(name) => format!("'{}'!{r}", name.replace('\'', "''")),
        None => r,
    })
}

impl Cell {
    pub fn text(s: impl Into<String>) -> Cell {
        let s = s.into();
        if s.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(s)
        }
    }

    /// A date, or an empty cell for none.
    pub fn date(d: Option<Date>) -> Cell {
        d.map_or(Cell::Empty, Cell::Date)
    }
}

/// One table: a header row of column titles, then data rows of the same width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    /// The sheet's name in the workbook.
    pub name: &'static str,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

impl Sheet {
    /// The same table without the column titled `name` (unchanged if there is none).
    pub fn without(&self, name: &str) -> Sheet {
        let Some(at) = self.columns.iter().position(|c| c == name) else {
            return self.clone();
        };
        let mut out = self.clone();
        out.columns.remove(at);
        for row in &mut out.rows {
            if at < row.len() {
                row.remove(at);
            }
        }
        out
    }
}

/// Every table of one export, in workbook order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workbook {
    /// The schedule's title (the workbook's document title).
    pub title: String,
    pub sheets: Vec<Sheet>,
}

impl Workbook {
    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.name == name)
    }
}

/// Why an export could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    /// The report could not be made from the schedule.
    Schedule(Error),
    /// Too big for a plain ZIP file (an .xlsx of 4 GB or more).
    Zip(zip::ZipError),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Schedule(e) => write!(f, "{e}"),
            ExportError::Zip(e) => write!(f, "the workbook is {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<zip::ZipError> for ExportError {
    fn from(e: zip::ZipError) -> ExportError {
        ExportError::Zip(e)
    }
}

impl From<Error> for ExportError {
    fn from(e: Error) -> ExportError {
        ExportError::Schedule(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_an_empty_cell() {
        assert_eq!(Cell::text(""), Cell::Empty);
        assert_eq!(Cell::text("KIT"), Cell::Text("KIT".into()));
        assert_eq!(Cell::date(None), Cell::Empty);
    }

    #[test]
    fn formulas_name_cells_only() {
        let Cell::Formula(f) = Formula::sum(7, 0..3, 9) else {
            panic!("not a formula");
        };
        assert_eq!((f.expr(), f.value()), ("SUM(H2:H4)", 9));
        let Cell::Formula(f) = Formula::sum_in(Some("One-Line"), 7, 2..5, 4) else {
            panic!("not a formula");
        };
        assert_eq!(f.expr(), "SUM('One-Line'!H4:H6)");
        let Cell::Formula(f) = Formula::count_if_in(Some("One-Line"), 2, 0..2, "scene", 2) else {
            panic!("not a formula");
        };
        assert_eq!(f.expr(), "COUNTIF('One-Line'!C2:C3,\"scene\")");
        let Cell::Formula(f) = Formula::add(&[3, 4], 0, 5) else {
            panic!("not a formula");
        };
        assert_eq!(f.expr(), "D2+E2");
        // Nothing to add up is a plain zero.
        assert_eq!(Formula::sum(7, 3..3, 0), Cell::Int(0));
        assert_eq!(Formula::add(&[], 0, 0), Cell::Int(0));
    }

    #[test]
    fn a_column_can_be_left_out() {
        let s = Sheet {
            name: "T",
            columns: vec!["a".into(), "b".into(), "c".into()],
            rows: vec![vec![Cell::Int(1), Cell::Int(2), Cell::Int(3)]],
        };
        let t = s.without("b");
        assert_eq!(t.columns, ["a", "c"]);
        assert_eq!(t.rows, [vec![Cell::Int(1), Cell::Int(3)]]);
        assert_eq!(s.without("z"), s);
    }
}
