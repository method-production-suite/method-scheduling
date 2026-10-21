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
use crate::app::CmdResult;
use crate::docs::Doc;
use method_schedule_core::model::Schedule;
use method_schedule_format::pdf_info::PdfInfo;
use serde::Serialize;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::reports::*;

/// Report `index` of the report list, for `sub_board`. `week_start` (`sun` or `mon`, the
/// user's preference) orders the calendar's weeks.
#[tauri::command(rename_all = "snake_case")]
pub fn report_data(
    state: Doc,
    sub_board: u32,
    index: usize,
    week_start: Option<String>,
) -> CmdResult<ReportDto> {
    method_schedule_session::commands::reports::report_data(&*state, sub_board, index, week_start)
}

// ---------------------------------------------------------------------------
// Printing

/// Letter paper, portrait, in points: the size report.js lays pages out for.
pub const LETTER: [f64; 2] = [612.0, 792.0];

/// How a print or PDF request ended.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PrintOutcome {
    /// False when this system can't print this way: the page uses `window.print()`
    /// instead.
    pub supported: bool,
    /// True when it printed, or the PDF was written; false when cancelled.
    pub done: bool,
    /// The PDF's file name (never its folder), when one was written.
    pub file: Option<String>,
}

/// A file name for `title` (no path separators or other characters file systems refuse),
/// ending in `.pdf`.
pub fn pdf_file_name(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_start_matches('.').trim();
    let stem: String = clean.chars().take(120).collect();
    let stem = if stem.is_empty() { "Report" } else { &stem };
    format!("{stem}.pdf")
}

/// Prints what the calling window's page shows for print media (the page has laid the
/// report out into `#print-root` first): with `pdf`, into a PDF file chosen in a Save dialog,
/// with no print sheet; otherwise through the print sheet. `landscape` sets the
/// orientation of the Letter page. `title` names the job and suggests the PDF's file name;
/// `subject` says what it shows. On macOS a saved PDF gets both as document properties,
/// with the production company as author and the app as creator ([`pdf_info`]). Waits
/// until the job is over.
#[tauri::command]
pub async fn print_report<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    pdf: bool,
    landscape: bool,
    title: String,
    subject: Option<String>,
) -> CmdResult<PrintOutcome> {
    let title: String = title.trim().chars().take(200).collect();
    let (dir, info) = {
        let s = state.lock();
        let dir = s
            .path()
            .and_then(std::path::Path::parent)
            .map(std::path::PathBuf::from);
        (dir, pdf_info(s.schedule(), &title, subject.as_deref()))
    };
    platform::print(window, dir, pdf, landscape, title, info).await
}

/// A saved PDF's document properties (decisions.md, entry 96): the job's title, what it
/// shows (the page says: the report, board and sub-board, date range), the production
/// company from the production info as the author, and the app as the creator.
pub fn pdf_info(s: &Schedule, title: &str, subject: Option<&str>) -> PdfInfo {
    let clean = |t: &str| -> String {
        t.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(200)
            .collect::<String>()
            .trim()
            .to_string()
    };
    PdfInfo {
        title: clean(title),
        subject: clean(subject.unwrap_or("")),
        author: clean(&s.production.company),
        creator: "Method Schedule".to_string(),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use tauri::{Runtime, WebviewWindow};
    use tauri_plugin_dialog::DialogExt;

    use super::{pdf_file_name, PdfInfo, PrintOutcome, LETTER};
    use crate::pdf_mac::{self, PageSetup};
    use crate::safety::{rename_into_place, temp_beside};

    pub async fn print<R: Runtime>(
        window: WebviewWindow<R>,
        dir: Option<PathBuf>,
        pdf: bool,
        landscape: bool,
        title: String,
        info: PdfInfo,
    ) -> Result<PrintOutcome, String> {
        let target = if pdf {
            let mut dialog = window
                .dialog()
                .file()
                .set_title("Save report as PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(pdf_file_name(&title))
                .set_parent(&window);
            if let Some(d) = &dir {
                dialog = dialog.set_directory(d);
            }
            let Some(picked) = dialog.blocking_save_file() else {
                return Ok(PrintOutcome {
                    supported: true,
                    ..PrintOutcome::default()
                });
            };
            Some(crate::export::with_extension(
                picked.into_path().map_err(|e| e.to_string())?,
                "pdf",
            ))
        } else {
            None
        };
        // The print operation writes a temporary file beside the target; it is renamed
        // into place once the job is over.
        let tmp = target.as_deref().map(|p| temp_beside(p, "pdf"));

        let setup = PageSetup {
            paper: LETTER,
            landscape,
            margins: [0.0; 4],
            title,
        };
        let (tx, rx) = mpsc::channel::<Result<bool, String>>();
        let started = tx.clone();
        let into = tmp.clone();
        window
            .with_webview(move |w| {
                let done = Box::new(move |ok: bool| {
                    let _ = tx.send(Ok(ok));
                });
                // SAFETY: Tauri hands us the live WKWebView and its NSWindow, on the main
                // thread.
                let run = unsafe {
                    pdf_mac::run(w.inner(), w.ns_window(), &setup, into.as_deref(), done)
                };
                if let Err(e) = run {
                    let _ = started.send(Err(e));
                }
            })
            .map_err(|e| e.to_string())?;
        let result = tauri::async_runtime::spawn_blocking(move || rx.recv())
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|_| "Printing stopped unexpectedly.".to_string()))
            .and_then(|r| r);
        let done = match (result, &tmp, &target) {
            (Ok(true), Some(tmp), Some(target)) => {
                if !tmp.is_file() {
                    return Err("The PDF could not be written there.".into());
                }
                // The document properties go in as an update at the file's end. One the
                // print system wrote in a way we don't update stays as it was.
                if let Some(with) = std::fs::read(tmp)
                    .ok()
                    .and_then(|b| method_schedule_format::pdf_info::set_info(&b, &info))
                {
                    std::fs::write(tmp, with)
                        .map_err(|e| format!("The PDF could not be written there: {e}"))?;
                }
                rename_into_place(tmp, target)
                    .map_err(|e| format!("The PDF could not be saved there: {e}"))?;
                true
            }
            (Ok(done), tmp, _) => {
                if let Some(t) = tmp {
                    let _ = std::fs::remove_file(t);
                }
                done
            }
            (Err(e), tmp, _) => {
                if let Some(t) = tmp {
                    let _ = std::fs::remove_file(t);
                }
                return Err(e);
            }
        };
        Ok(PrintOutcome {
            supported: true,
            done,
            file: target
                .filter(|_| done)
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::PathBuf;

    use tauri::{Runtime, WebviewWindow};

    use super::{PdfInfo, PrintOutcome};

    /// Not built for this system yet: the page prints with `window.print()`, and the
    /// system dialog's PDF carries no properties of ours.
    pub async fn print<R: Runtime>(
        _window: WebviewWindow<R>,
        _dir: Option<PathBuf>,
        _pdf: bool,
        _landscape: bool,
        _title: String,
        _info: PdfInfo,
    ) -> Result<PrintOutcome, String> {
        Ok(PrintOutcome::default())
    }
}

#[cfg(test)]
mod tests;
