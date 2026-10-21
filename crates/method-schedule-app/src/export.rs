//! File > Export: a report as an Excel workbook or a CSV file, and Export for Budget, the
//! budget exchange file (docs/EXCHANGE.md). The tables and file formats are
//! `method_schedule_format::export` and `method_schedule_format::exchange`, used as they are; this is the
//! app's side: which report, the Save dialog, writing the file.
//!
//! Exports cover the sub-board the page shows (its board and sub-board pickers).
//!
//! Security: the page names only the report and the format; the path comes from the
//! native Save dialog shown here, and the extension is the format's. Each file is written
//! atomically (`safety::write_atomically`), and an export never replaces the open
//! schedule's own file.

use std::path::{Path, PathBuf};

use tauri::{Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

pub use method_schedule_session::export::*;

use crate::app::CmdResult;
use crate::docs::Doc;
use crate::safety::{same_file, write_atomically, Access};

/// `p`, ending in `ext` (added when it has another or none).
pub fn with_extension(mut p: PathBuf, ext: &str) -> PathBuf {
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        p.as_mut_os_string().push(format!(".{ext}"));
    }
    p
}

fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Writes `bytes` to `path` atomically, refusing to replace the open schedule's file
/// (`open`).
pub fn write_export(open: Option<&Path>, path: &Path, bytes: &[u8]) -> Result<String, String> {
    if open.is_some_and(|o| same_file(o, path)) {
        return Err(format!(
            "\"{}\" is the open schedule; export to another name.",
            display_name(path)
        ));
    }
    write_atomically(path, bytes, Access::Document)
        .map_err(|e| format!("Couldn't write \"{}\": {e}", display_name(path)))?;
    Ok(display_name(path))
}

/// The open schedule's folder and a file name stem (its file's, or its title's). An
/// export's Save dialog opens in that folder (decision 171); a schedule not yet saved
/// has none, and the dialog opens where the system last saved.
pub(crate) fn place(s: &crate::session::Session) -> (Option<PathBuf>, String, Option<PathBuf>) {
    let path = s.path().map(PathBuf::from);
    let dir = path.as_deref().and_then(Path::parent).map(PathBuf::from);
    let stem = path
        .as_deref()
        .and_then(Path::file_stem)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| safe_file_stem(&s.schedule().title));
    (dir, stem, path)
}

/// Shows a Save dialog and writes `bytes` where it says. `None` when cancelled. Every
/// export goes through here, so each opens in `dir`, the schedule's folder (decision
/// 171).
#[allow(clippy::too_many_arguments)]
pub(crate) fn save_dialog<R: Runtime>(
    window: &WebviewWindow<R>,
    title: &str,
    filter: &str,
    ext: &str,
    name: String,
    dir: Option<PathBuf>,
    open: Option<&Path>,
    bytes: &[u8],
) -> CmdResult<ExportOutcome> {
    let mut dialog = window
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter, &[ext])
        .set_file_name(name)
        .set_parent(window);
    if let Some(d) = dir {
        dialog = dialog.set_directory(d);
    }
    let Some(picked) = dialog.blocking_save_file() else {
        return Ok(ExportOutcome::default());
    };
    let path = with_extension(picked.into_path().map_err(|e| e.to_string())?, ext);
    let file = write_export(open, &path, bytes)?;
    Ok(ExportOutcome {
        done: true,
        file: Some(file),
    })
}

/// `report`: `dood` (the summary), `one_line`, `elements`, `dood_grid`, `element_detail`,
/// `breakdown`, `breakdown_day`, `board` or `totals`; `format`: `xlsx` or `csv`.
/// Shows a Save dialog and writes the file there.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_report<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    report: String,
    format: String,
) -> CmdResult<ExportOutcome> {
    // The table is made under the lock; the dialog and the writing happen after it is
    // released, so other commands aren't held up.
    let (d, format, dir, open) = {
        let s = state.lock();
        let (d, format) = method_schedule_session::commands::files::report_download(
            &s, sub_board, &report, &format,
        )?;
        let (dir, _, open) = place(&s);
        (d, format, dir, open)
    };
    let (title, filter) = format.dialog_text();
    save_dialog(
        &window,
        title,
        filter,
        format.extension(),
        d.name,
        dir,
        open.as_deref(),
        &d.bytes,
    )
}

/// Export for Budget: the exchange file (docs/EXCHANGE.md) with today's date and the
/// schedule's hash, where a Save dialog says.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_budget<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
) -> CmdResult<ExportOutcome> {
    let (d, dir, open) = {
        let s = state.lock();
        let d = method_schedule_session::commands::files::budget_download(&s, sub_board)?;
        let (dir, _, open) = place(&s);
        (d, dir, open)
    };
    save_dialog(
        &window,
        "Export for budget",
        "Budget exchange file",
        "json",
        d.name,
        dir,
        open.as_deref(),
        &d.bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::safety::tests::temp_dir;
    use crate::session::SessionFiles;
    use crate::session::{read_script, Session};
    use method_schedule_core::Date;
    use method_schedule_format::exchange;
    use method_schedule_format::export::{self, xlsx, Cell, PAGES};

    fn harbor(start: bool) -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        let mut s = Session::new();
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s.edit(|x| {
            let sb = x.boards[0].sub_boards[0].id.0;
            let ids: Vec<u32> = x.boards[0].boneyard.iter().map(|st| st.id.0).collect();
            board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0)?;
            board::auto_day_breaks(x, sb, "3/8")?;
            if start {
                board::set_start_date(x, sb, Some("2027-03-01"))?;
            }
            Ok(())
        })
        .unwrap();
        s
    }

    /// Every report File > Export offers (ui/js/export.js) is one Rust exports, and one the
    /// harness fake answers too (a review found the fake refusing red flags).
    #[test]
    fn the_page_offers_what_rust_and_the_harness_export() {
        let page = include_str!("../ui/js/export.js");
        let list = page
            .split("const REPORTS = [")
            .nth(1)
            .and_then(|t| t.split("\n];").next())
            .unwrap();
        let keys: Vec<&str> = list
            .lines()
            .filter_map(|l| l.trim().strip_prefix("['"))
            .map(|l| l.split('\'').next().unwrap())
            .collect();
        assert!(keys.len() >= 11, "{keys:?}");
        let fake = include_str!("../harness/fake-tauri.js");
        let answered = fake
            .split("throw \"That report can't be exported.\"")
            .next()
            .and_then(|t| t.rsplit("if (![").next())
            .unwrap();
        for k in keys {
            assert!(Report::parse(k).is_some(), "Rust does not export {k}");
            assert!(
                answered.contains(&format!("'{k}'")),
                "the harness refuses {k}"
            );
        }
    }

    #[test]
    fn names_parse() {
        assert_eq!(Report::parse("dood"), Some(Report::DoodSummary));
        assert_eq!(Report::parse("one_line"), Some(Report::OneLine));
        assert_eq!(Report::parse("one_line_fixed"), Some(Report::OneLineFixed));
        assert_eq!(Report::parse("elements"), Some(Report::Elements));
        assert_eq!(Report::parse("dood_grid"), Some(Report::DoodGrid));
        assert_eq!(Report::parse("board"), Some(Report::Board));
        assert_eq!(Report::parse("breakdown_day"), Some(Report::BreakdownDay));
        assert_eq!(Report::parse("totals"), Some(Report::Totals));
        assert_eq!(Report::parse("red_flags"), Some(Report::RedFlags));
        assert_eq!(Report::parse("shooting"), None);
        assert_eq!(Format::parse("xlsx"), Some(Format::Xlsx));
        assert_eq!(Format::parse("csv"), Some(Format::Csv));
        assert_eq!(Format::parse("tsv"), None);
    }

    #[test]
    fn workbooks_round_trip_and_csv_has_a_bom() {
        let s = harbor(true);
        for r in [
            Report::DoodSummary,
            Report::OneLine,
            Report::OneLineFixed,
            Report::Elements,
            Report::DoodGrid,
            Report::ElementDetail,
            Report::Breakdown,
            Report::BreakdownDay,
            Report::Board,
            Report::Totals,
        ] {
            let wb = workbook(s.schedule(), crate::view::first_sub(s.schedule()), r).unwrap();
            assert_eq!(wb.title, "Harbor Light");
            let bytes = file_bytes(&wb, Format::Xlsx).unwrap();
            let back = xlsx::read_cells(&bytes).unwrap();
            assert_eq!(back[0].name, wb.sheets[0].name);
            assert_eq!(&back[0].rows[1..], &wb.sheets[0].rows[..]);
            let text = String::from_utf8(file_bytes(&wb, Format::Csv).unwrap()).unwrap();
            assert!(text.starts_with('\u{feff}'), "{r:?}");
            assert!(!text.lines().next().unwrap().contains(PAGES), "{r:?}");
        }
        let one = workbook(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Report::OneLineFixed,
        )
        .unwrap();
        // Cast board IDs, or names where there are none yet.
        assert_eq!(one.sheets[0].rows[0][10], Cell::text("KIT TOVE"));
        // The plain one-line follows the strip layout.
        let lay = workbook(
            s.schedule(),
            crate::view::first_sub(s.schedule()),
            Report::OneLine,
        )
        .unwrap();
        assert_eq!(lay.sheets[0].columns[0], export::BREAKS);
        assert_eq!(lay.sheets[0].columns[1], "Scene");
    }

    #[test]
    fn the_budget_file_is_the_exchange_file() {
        let s = harbor(true);
        let day = "2026-10-11".parse::<Date>().ok();
        let text = budget_text(s.schedule(), crate::view::first_sub(s.schedule()), day).unwrap();
        let (b, sb) = ids(s.schedule(), crate::view::first_sub(s.schedule())).unwrap();
        assert_eq!(text, exchange::to_string(s.schedule(), b, sb, day).unwrap());
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["format"], "method-schedule-days");
        assert_eq!(v["generated"], "2026-10-11");
        // The hash of the schedule's canonical text: 16 hex digits that change with it.
        let hash = v["source_hash"].as_str().unwrap().to_string();
        assert!(hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()));
        let mut changed = s.schedule().clone();
        changed.title = "Harbor Light 2".into();
        let other: serde_json::Value = serde_json::from_str(
            &budget_text(&changed, crate::view::first_sub(&changed), day).unwrap(),
        )
        .unwrap();
        assert_ne!(other["source_hash"], v["source_hash"]);
        assert!(method_schedule_format::exchange::local_today().is_some());

        let h = harbor(false);
        let e = budget_text(h.schedule(), crate::view::first_sub(h.schedule()), day).unwrap_err();
        assert!(e.starts_with("Set a start date for Main"), "{e}");
        assert!(!e.contains('!') && !e.contains('\u{2014}'));
    }

    #[test]
    fn writes_are_atomic_and_never_replace_the_schedule() {
        let dir = temp_dir("export");
        let open = dir.join("Harbor.mtsc");
        std::fs::write(&open, b"schedule").unwrap();
        let e = write_export(Some(&open), &open, b"x").unwrap_err();
        assert!(e.contains("open schedule"), "{e}");
        assert_eq!(std::fs::read(&open).unwrap(), b"schedule");
        let out = dir.join("Harbor - One-Line.csv");
        assert_eq!(
            write_export(Some(&open), &out, b"a,b").unwrap(),
            "Harbor - One-Line.csv"
        );
        assert_eq!(std::fs::read(&out).unwrap(), b"a,b");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        assert!(write_export(None, &dir.join("missing/x.csv"), b"x").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Note 27: the call sheet export's Save dialog opened in the folder last used for
    /// images. Every Save dialog opens in the schedule's folder (decision 171): an
    /// export's through `save_dialog`, the others setting it themselves. Templates have
    /// their own folder.
    #[test]
    fn every_save_dialog_opens_in_the_schedules_folder() {
        let dir = temp_dir("export-place");
        let file = dir.join("Harbor.mtsc");
        let mut s = Session::new();
        assert_eq!(place(&s).0, None, "not saved yet: the system's choice");
        s.save_to(&file).unwrap();
        let (folder, stem, open) = place(&s);
        assert_eq!(folder.as_deref(), Some(dir.as_path()));
        assert_eq!(stem, "Harbor");
        assert_eq!(open.as_deref(), Some(file.as_path()));
        std::fs::remove_dir_all(&dir).unwrap();
        // Each Save dialog sets its folder before it shows.
        for (name, src) in [
            ("export.rs", include_str!("export.rs")),
            ("plan.rs", include_str!("plan.rs")),
            ("layouts.rs", include_str!("layouts.rs")),
            ("reports.rs", include_str!("reports.rs")),
            ("app.rs", include_str!("app.rs")),
            ("templates.rs", include_str!("templates.rs")),
        ] {
            // The code only, not its tests.
            let mut rest = src.split("#[cfg(test)]").next().unwrap_or(src);
            while let Some(at) = rest.find(concat!(".blocking_", "save_file()")) {
                let before = &rest[..at];
                let start = before.rfind(".dialog()").unwrap_or(0);
                assert!(
                    before[start..].contains("set_directory("),
                    "a Save dialog in {name} with no folder"
                );
                rest = &rest[at + 1..];
            }
        }
        let plan = include_str!("plan.rs");
        assert!(plan.contains("crate::export::save_dialog("));
    }

    #[test]
    fn extensions() {
        assert_eq!(
            with_extension(PathBuf::from("/x/a"), "xlsx"),
            PathBuf::from("/x/a.xlsx")
        );
        assert_eq!(
            with_extension(PathBuf::from("/x/a.XLSX"), "xlsx"),
            PathBuf::from("/x/a.XLSX")
        );
        assert_eq!(
            with_extension(PathBuf::from("/x/a.csv"), "json"),
            PathBuf::from("/x/a.csv.json")
        );
    }
}
