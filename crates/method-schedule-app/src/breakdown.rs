//! The Breakdown tab's header field order (decisions.md, entry 81) and the breakdown
//! designer's hidden fields, widths (entry 101) and row lengths (entry 186): kept in the schedule
//! (`Schedule::sheet_fields`, `Schedule::sheet_looks`), one undo step.
use crate::app::{finish, CmdResult};
use crate::docs::Doc;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::breakdown::*;

/// Breakdown > Breakdown Designer: lays the sheet's fields out in `fields`' order, hides
/// `hidden`, sets `widths` (key to `narrow`, `medium` or `wide`) and `rows` (how many
/// shown fields each row holds; empty for the usual rows).
#[tauri::command]
pub fn set_sheet_field_order<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    fields: Vec<String>,
    hidden: Option<Vec<String>>,
    widths: Option<std::collections::BTreeMap<String, String>>,
    rows: Option<Vec<u8>>,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| {
        set_design(
            x,
            &fields,
            hidden.as_deref(),
            widths.as_ref(),
            rows.as_deref(),
        )
    })?;
    finish(&window, &s, None)
}
