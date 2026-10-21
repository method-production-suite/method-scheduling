//! The halves of the file commands that need no file system, shared by both hosts: what a
//! download is called and holds (the desktop shows a Save dialog with that name and
//! writes the bytes where it says; the browser host offers them as a download), and what
//! a picked file's text does to the schedule (the desktop reads the file the Open dialog
//! picked; the browser host is handed its bytes).

use std::path::Path;

use method_schedule_core::fountain::ImportedScript;

use super::{today, CmdResult};
use crate::export::{
    budget_text, drift_bytes, file_bytes, safe_file_stem, workbook, Format, Report,
};
use crate::layouts::{layouts_file, load_layouts, LAYOUTS_EXTENSION};
use crate::plan::{call_sheet_bytes, upper};
use crate::session::{self, Session};

/// A file to keep: the name to suggest and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The stem an export's name starts with: the schedule file's name, or its title's when
/// it has no file yet.
pub fn file_stem(s: &Session) -> String {
    s.path()
        .and_then(Path::file_stem)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| safe_file_stem(&s.schedule().title))
}

fn format_of(format: &str) -> CmdResult<Format> {
    Format::parse(format).ok_or_else(|| "That export format isn't one the app knows.".to_string())
}

/// File > Export: `report` of sub-board `sub_board` in `format` (`xlsx` or `csv`), and the
/// format, whose Save dialog text the desktop shows.
pub fn report_download(
    s: &Session,
    sub_board: u32,
    report: &str,
    format: &str,
) -> CmdResult<(Download, Format)> {
    let report = Report::parse(report).ok_or("That report can't be exported.")?;
    let format = format_of(format)?;
    let bytes = file_bytes(&workbook(s.schedule(), sub_board, report)?, format)?;
    let name = format!(
        "{} - {}.{}",
        file_stem(s),
        report.file_label(),
        format.extension()
    );
    Ok((Download { name, bytes }, format))
}

/// Export for Budget: the exchange file of sub-board `sub_board`, stamped today.
pub fn budget_download(s: &Session, sub_board: u32) -> CmdResult<Download> {
    let text = budget_text(s.schedule(), sub_board, today())?;
    Ok(Download {
        name: format!("{} - Budget Days.json", file_stem(s)),
        bytes: text.into_bytes(),
    })
}

/// Export Call Sheet Data: day `day` of sub-board `sub_board` as `csv` or `json`.
pub fn call_sheet_download(
    s: &Session,
    sub_board: u32,
    day: u32,
    format: &str,
) -> CmdResult<Download> {
    let csv = match format {
        "csv" => true,
        "json" => false,
        _ => return Err("That call sheet format isn't one the app knows.".to_string()),
    };
    let bytes = call_sheet_bytes(s.schedule(), sub_board, day, csv)?;
    let ext = if csv { "csv" } else { "json" };
    Ok(Download {
        name: format!("{} - Call Sheet Day {day}.{ext}", file_stem(s)),
        bytes,
    })
}

/// Schedule Locks > Export: each actor's drift since lock `name`, and the format.
pub fn drift_download(s: &Session, name: &str, format: &str) -> CmdResult<(Download, Format)> {
    let format = format_of(format)?;
    let bytes = drift_bytes(s.schedule(), name, format)?;
    let name = format!(
        "{} - Drift since {}.{}",
        file_stem(s),
        safe_file_stem(name),
        format.extension()
    );
    Ok((Download { name, bytes }, format))
}

/// Save Layouts: the layouts `ids` (all when empty) as a layout file.
pub fn layouts_download(s: &Session, ids: &[u32]) -> CmdResult<Download> {
    let (text, _) = layouts_file(s.schedule(), ids)?;
    Ok(Download {
        name: format!(
            "{} - Layouts.{LAYOUTS_EXTENSION}",
            safe_file_stem(&s.schedule().title)
        ),
        bytes: text.into_bytes(),
    })
}

/// Save as Template: the schedule's setup as a template file.
pub fn template_download(s: &Session) -> Download {
    Download {
        name: format!(
            "{}.{}",
            safe_file_stem(&s.schedule().title),
            crate::templates::EXTENSION
        ),
        bytes: crate::templates::template_text(s.schedule()).into_bytes(),
    }
}

/// Load Layouts: adds the layouts in a layout file's bytes, as one undo step; the notice.
pub fn load_layouts_bytes(s: &mut Session, bytes: &[u8]) -> CmdResult<String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "That isn't a layout file Method Schedule can read.".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let n = s.edit(|x| load_layouts(x, text))?;
    Ok(if n == 1 {
        "Loaded 1 layout.".to_string()
    } else {
        format!("Loaded {n} layouts.")
    })
}

/// A script read with its production tags: only from an `.fdx` file (by `name`'s
/// extension), refused in words otherwise.
pub fn tagged_script(name: &str, bytes: &[u8]) -> CmdResult<ImportedScript> {
    let fdx_file = Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fdx"));
    if !fdx_file {
        return Err("Production tags come only from .fdx files.".to_string());
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| "The script is not UTF-8 text.".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    method_schedule_core::fdx::parse_with_tags(text).map_err(upper)
}

/// Import Script with Tags: the scenes with their production tags into the Boneyard, as
/// one undo step; the notice.
pub fn import_tagged(s: &mut Session, script: &ImportedScript) -> CmdResult<String> {
    let tags: usize = script.scenes.iter().map(|x| x.tags.len()).sum();
    let added = s.edit(|x| {
        let cast = x.cast_or_add();
        method_schedule_core::revise::import_tagged(x, script, cast)
            .map(|v| v.len())
            .map_err(upper)
    })?;
    Ok(format!(
        "Added {added} {} to the Boneyard, with {tags} production {}.",
        if added == 1 { "scene" } else { "scenes" },
        if tags == 1 { "tag" } else { "tags" }
    ))
}

/// Import Script: the scenes into the Boneyard (the first import names an untitled
/// schedule after the script); the notice.
pub fn import_script(s: &mut Session, script: &ImportedScript) -> CmdResult<String> {
    let added = s.import_script(script)?;
    Ok(session::import_notice_for(script, added))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Session {
        Session::with_schedule(method_schedule_core::sample::field_station(), None)
    }

    #[test]
    fn downloads_are_named_as_the_desktop_names_them() {
        let s = sample();
        let sub = s.schedule().boards[0].sub_boards[0].id.0;
        let (d, f) = report_download(&s, sub, "dood", "csv").unwrap();
        assert_eq!(d.name, "Field Station - Day Out of Days.csv");
        assert_eq!(f, Format::Csv);
        assert!(report_download(&s, sub, "nope", "csv").is_err());
        assert!(report_download(&s, sub, "dood", "tsv").is_err());
        let cs = call_sheet_download(&s, sub, 1, "json").unwrap();
        assert_eq!(cs.name, "Field Station - Call Sheet Day 1.json");
        assert!(call_sheet_download(&s, sub, 1, "pdf").is_err());
        assert_eq!(template_download(&s).name, "Field Station.mtst");
        assert!(
            layouts_download(&s, &[]).is_err(),
            "the sample has no named layouts"
        );
    }

    #[test]
    fn tags_come_only_from_fdx() {
        assert!(tagged_script("a.fountain", b"INT. A - DAY").is_err());
        let text = include_str!("../../../../fixtures/scripts/field-station-tagged.fdx");
        let script = tagged_script("t.FDX", text.as_bytes()).unwrap();
        let mut s = Session::with_schedule(
            method_schedule_core::Schedule::with_default_categories("T"),
            None,
        );
        let notice = import_tagged(&mut s, &script).unwrap();
        assert_eq!(
            notice,
            "Added 3 scenes to the Boneyard, with 7 production tags."
        );
        assert!(load_layouts_bytes(&mut s, b"[]").is_err());
    }
}
