//! User templates (decisions.md, entries 77 and 85): File > Save as Template writes the
//! schedule's setup, without its sheets, strips, events or red flags, as our own schedule
//! file marked as a template (`.mtst`); File > New from Template starts a new, untitled
//! schedule from one. Paths come only from Rust dialogs, which open in the data folder's
//! `Templates/` the first time.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::app::{finish, settle_unsaved, CmdResult};
use crate::docs::Doc;
use crate::safety::{self, Access};
use crate::safety_app::{self, Safety};
use crate::session::{self, Session};
use crate::view::View;

pub use method_schedule_session::templates::*;

/// Reads a template file (or a schedule file, used as one) into a new untitled session.
pub fn open_template(path: &Path) -> Result<Session, String> {
    let (schedule, _) = session::read_schedule(path)?;
    Ok(session_from(&schedule))
}

/// The folder templates are kept in: the data folder's `Templates/` (made when needed).
fn folder<R: Runtime>(window: &WebviewWindow<R>) -> Option<PathBuf> {
    let dir = window
        .app_handle()
        .path()
        .app_local_data_dir()
        .ok()?
        .join("Templates");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedDto {
    pub done: bool,
    /// The file's name, never its folder.
    pub file: Option<String>,
}

/// File > Save as Template…: asks where (in Templates/ by default) and writes the template
/// atomically. The schedule itself is not changed and stays where it is.
#[tauri::command]
pub async fn save_template<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<SavedDto> {
    let (text, title) = {
        let s = state.lock();
        (template_text(s.schedule()), s.schedule().title.clone())
    };
    let mut dialog = window
        .dialog()
        .file()
        .set_title("Save as template")
        .add_filter("Schedule template", &[EXTENSION])
        .set_file_name(format!(
            "{}.{EXTENSION}",
            crate::app::safe_file_stem(&title)
        ))
        .set_parent(&window);
    if let Some(dir) = folder(&window) {
        dialog = dialog.set_directory(dir);
    }
    let Some(picked) = dialog.blocking_save_file() else {
        return Ok(SavedDto {
            done: false,
            file: None,
        });
    };
    let path =
        crate::export::with_extension(picked.into_path().map_err(|e| e.to_string())?, EXTENSION);
    if state
        .lock()
        .path()
        .is_some_and(|p| safety::same_file(p, &path))
    {
        return Err("Choose another file: that is the schedule itself.".into());
    }
    safety::write_atomically(&path, text.as_bytes(), Access::Document)
        .map_err(|e| format!("The template couldn't be saved: {e}"))?;
    Ok(SavedDto {
        done: true,
        file: path.file_name().map(|n| n.to_string_lossy().into_owned()),
    })
}

/// File > New from Template…: an Open dialog (in Templates/ by default), then, as File >
/// New and Open do, the unsaved-changes question when there is anything unsaved (Autosave
/// off, or a schedule with no file yet); the template starts a new, untitled schedule in
/// place of the open one, as New does (decisions 86 and 166). Picking first means a
/// cancelled pick or a template that can't be read never asks or saves. `None` when
/// cancelled.
#[tauri::command]
pub async fn new_from_template<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<View>> {
    let mut dialog = window
        .dialog()
        .file()
        .set_title("New from template")
        .add_filter("Schedule template", &[EXTENSION, session::EXTENSION])
        .set_parent(&window);
    if let Some(dir) = folder(&window) {
        dialog = dialog.set_directory(dir);
    }
    let Some(picked) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let next = open_template(&path)?;
    if !settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    safety_app::replace(&state, &window.state::<Safety>(), next);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    finish(
        &window,
        &state.lock(),
        Some(format!("A new schedule from the template {name}.")),
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionFiles;
    use method_schedule_core::model::Sheet;
    use method_schedule_core::Schedule;

    #[test]
    fn a_template_round_trips_into_an_untitled_schedule() {
        let dir = std::env::temp_dir().join(format!("ms-templates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Schedule::with_default_categories("ZTEST Series");
        s.add_sheet(Sheet {
            scenes: "1".into(),
            ..Sheet::default()
        });
        s.add_red_flag_type("Unavailable").unwrap();
        let path = dir.join("series.mtst");
        std::fs::write(&path, template_text(&s)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(method_schedule_format::is_template(&text));
        let fresh = open_template(&path).unwrap();
        assert!(fresh.path().is_none(), "untitled");
        assert_eq!(fresh.display_name(), "Untitled");
        let x = fresh.schedule();
        assert!(x.sheets.is_empty());
        assert_eq!(x.categories, s.categories);
        assert_eq!(x.red_flag_types, s.red_flag_types);
        assert!(!fresh.is_dirty());
        // A plain schedule file works as a template too.
        let plain = dir.join("film.mtsc");
        std::fs::write(&plain, method_schedule_format::to_string(&s)).unwrap();
        assert!(open_template(&plain).unwrap().schedule().sheets.is_empty());
        // Opening a template with Open starts untitled as well.
        let opened = Session::open(&path).unwrap();
        assert!(opened.path().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
