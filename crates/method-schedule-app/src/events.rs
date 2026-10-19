//! Dragging an event on the Calendar to another date (decisions.md, entry 83): the event
//! keeps its length and moves to start on the date it was dropped on, as one undo step.
use crate::app::{finish, CmdResult};
use crate::docs::Doc;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::events::*;

/// The Calendar's drag and drop of an event: one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn move_event_to<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    index: usize,
    date: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| move_event(x, sub_board, index, &date))?;
    finish(&window, &s, None)
}
