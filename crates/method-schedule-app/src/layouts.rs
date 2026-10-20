//! The layout library in the app (decisions.md, entry 78): named layouts of scene strips,
//! day breaks and shoot day strips, picked per board and by the one-line report, saved
//! and loaded as layout files. The edits are plain functions on a [`Schedule`] (one undo
//! step each through `Session::edit`); the two file commands show their dialogs from
//! Rust, so the page never names a path.
use crate::app::{finish, AppState, CmdResult};
use crate::board::EditResult;
use crate::docs::Doc;
use crate::export::ExportOutcome;
use method_schedule_core::Schedule;
use std::path::{Path, PathBuf};
use tauri::{Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

pub use method_schedule_session::layouts::*;

fn edit_with<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    f: impl FnOnce(&mut Schedule) -> EditResult,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(f)?;
    finish(window, &s, None)
}

/// Adds a named layout (from the Strip Layout or Day Breaks dialog), and with `board` picks
/// it there, as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn add_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    name: String,
    kind: String,
    layout: LayoutInput,
    board: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| {
        add_named(s, &name, &kind, &layout, board).map(|_| ())
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: u32,
    layout: LayoutInput,
) -> CmdResult {
    edit_with(&window, &state, |s| save_named(s, id, &layout))
}

#[tauri::command(rename_all = "snake_case")]
pub fn rename_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: u32,
    name: String,
) -> CmdResult {
    edit_with(&window, &state, |s| rename_named(s, id, &name))
}

#[tauri::command(rename_all = "snake_case")]
pub fn move_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: u32,
    to: usize,
) -> CmdResult {
    edit_with(&window, &state, |s| move_named(s, id, to))
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_layout<R: Runtime>(window: WebviewWindow<R>, state: Doc, id: u32) -> CmdResult {
    edit_with(&window, &state, |s| delete_named(s, id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn use_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    kind: String,
    id: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| pick_layout(s, board, &kind, id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn use_one_line_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| pick_one_line(s, id))
}

const EXT: &str = "mtlayouts";

/// Save Layouts: the named layouts `ids` (all when empty) into a layout file where a Save
/// dialog says. Never replaces the open schedule's file.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_layouts<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    ids: Vec<u32>,
) -> CmdResult<ExportOutcome> {
    let (text, dir, stem, open) = {
        let s = state.lock();
        let (text, _) = layouts_file(s.schedule(), &ids)?;
        let path = s.path().map(PathBuf::from);
        let dir = path.as_deref().and_then(Path::parent).map(PathBuf::from);
        let stem = crate::app::safe_file_stem(&s.schedule().title);
        (text, dir, stem, path)
    };
    let mut dialog = window
        .dialog()
        .file()
        .set_title("Save layouts")
        .add_filter("Method Schedule layouts", &[EXT, "json"])
        .set_file_name(format!("{stem} - Layouts.{EXT}"))
        .set_parent(&window);
    if let Some(d) = dir {
        dialog = dialog.set_directory(d);
    }
    let Some(picked) = dialog.blocking_save_file() else {
        return Ok(ExportOutcome::default());
    };
    let path = crate::export::with_extension(picked.into_path().map_err(|e| e.to_string())?, EXT);
    let file = crate::export::write_export(open.as_deref(), &path, text.as_bytes())?;
    Ok(ExportOutcome {
        done: true,
        file: Some(file),
    })
}

/// Load Layouts: adds the layouts of a layout file an Open dialog picks, as one undo
/// step. `None` when cancelled.
#[tauri::command(rename_all = "snake_case")]
pub async fn import_layouts<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<crate::view::View>> {
    let dir = state
        .lock()
        .path()
        .and_then(Path::parent)
        .map(PathBuf::from);
    let mut dialog = window
        .dialog()
        .file()
        .set_title("Load layouts")
        .add_filter("Method Schedule layouts", &[EXT, "json"])
        .set_parent(&window);
    if let Some(d) = dir {
        dialog = dialog.set_directory(d);
    }
    let Some(picked) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let bytes =
        crate::safety::read_limited(&path).map_err(|e| format!("Couldn't read that file: {e}."))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "That isn't a layout file Method Schedule can read.".to_string())?;
    let mut s = state.lock();
    let n = s.edit(|x| load_layouts(x, &text))?;
    let what = if n == 1 {
        "1 layout".to_string()
    } else {
        format!("{n} layouts")
    };
    finish(&window, &s, Some(format!("Loaded {what}."))).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{DayLookInput, LayoutColumnInput};
    use crate::session::Session;

    fn col(field: &str, width: &str, second_line: bool) -> LayoutColumnInput {
        LayoutColumnInput {
            field: field.into(),
            width: width.into(),
            line: if second_line { 2 } else { 1 },
            span: 1,
            align: String::new(),
            bold: false,
            italic: false,
            text: String::new(),
            category: None,
            board_ids: false,
            rule_after: false,
            rule_under: false,
        }
    }

    #[test]
    fn named_layouts_are_added_picked_saved_and_loaded() {
        let mut s = Session::new();
        let b = s.schedule().boards[0].id.0;
        let two = LayoutInput {
            columns: Some(vec![
                col("scene", "narrow", false),
                col("set", "wide", false),
                col("synopsis", "widest", true),
            ]),
            size: Some("small".into()),
            look: None,
            marks: None,
        };
        let id = s
            .edit(|x| add_named(x, "Two lines", "strips", &two, Some(b)))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.uses.strips, Some(id));
        assert_eq!(v.board.layout.len(), 3);
        assert_eq!(v.board.layout[2].line, 2);
        // Its text size goes with it (decisions.md, entry 87).
        assert_eq!(v.board.layout_size, "small");
        assert_eq!(v.board.own_layout_size, "normal");
        assert_eq!(v.layouts[0].size, "small");
        assert_eq!(v.board.own_layout.len(), 6, "its own stays");
        // Refused: a name taken, no field on line 1, a body of the wrong type.
        assert!(s
            .edit(|x| add_named(x, "two lines", "strips", &two, None))
            .is_err());
        let all_second = LayoutInput {
            columns: Some(vec![col("scene", "narrow", true)]),
            size: None,
            look: None,
            marks: None,
        };
        let e = s
            .edit(|x| add_named(x, "Odd", "strips", &all_second, None))
            .unwrap_err();
        assert!(e.ends_with('.'), "{e}");
        assert!(s
            .edit(|x| add_named(x, "Odd", "panels", &two, None))
            .is_err());
        // A day break layout picked by the board; the shoot day strip from a layout.
        let night = LayoutInput {
            columns: None,
            size: None,
            look: Some(DayLookInput {
                sunset: true,
                text: "Night".into(),
                ..DayLookInput::default()
            }),
            marks: None,
        };
        let n = s
            .edit(|x| add_named(x, "Night", "day_breaks", &night, Some(b)))
            .unwrap();
        let sd = s
            .edit(|x| add_named(x, "Shoot day", "shoot_day", &night, None))
            .unwrap();
        s.edit(|x| pick_layout(x, b, "shoot_day", Some(sd)))
            .unwrap();
        assert!(s.edit(|x| pick_layout(x, b, "strips", Some(n))).is_err());
        let v = crate::view::view(&s, None).unwrap();
        assert!(v.board.day_look.sunset && v.board.day_look.shoot_day_strip);
        assert_eq!(v.board.shoot_text, "Night");
        // The one-line report picks its own.
        s.edit(|x| pick_one_line(x, None)).unwrap();
        // Save and load as a file: names that clash get a number.
        let (text, count) = layouts_file(s.schedule(), &[]).unwrap();
        assert_eq!(count, 3);
        let loaded = s.edit(|x| load_layouts(x, &text)).unwrap();
        assert_eq!(loaded, 3);
        assert!(s
            .schedule()
            .layouts
            .layouts
            .iter()
            .any(|l| l.name == "Night 2"));
        assert!(s
            .edit(|x| load_layouts(x, "{\"format\": \"method-schedule\"}"))
            .is_err());
        // Deleting a picked layout sends the board back to its own.
        s.edit(|x| delete_named(x, id)).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.uses.strips, None);
        assert_eq!(v.board.layout.len(), 6);
        s.schedule().check().unwrap();
    }
}
