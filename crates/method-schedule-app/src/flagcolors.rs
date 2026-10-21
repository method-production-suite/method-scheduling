//! A color per red flag type (decisions.md, entry 84): its marks on the board, the
//! calendar, the Breakdown tab, the Red flags tab and an element's flags are drawn in it.
use crate::app::CmdResult;
use crate::docs::Doc;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::flagcolors::*;

/// Red Flag Types > a type's color, one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn set_red_flag_type_color<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    red_flag_type: u32,
    color: String,
) -> CmdResult {
    method_schedule_session::commands::flagcolors::set_red_flag_type_color(
        &crate::app::Win(&window),
        &*state,
        red_flag_type,
        color,
    )
}
