//! The `events` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `events.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::events::*;

/// The Calendar's drag and drop of an event: one undo step.
pub fn move_event_to<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    index: usize,
    date: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| move_event(x, sub_board, index, &date))?;
    finish(&window, &s, None)
}
