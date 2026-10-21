//! The `split` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `split.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::split::*;
use crate::view::{self, BoardDto, View};
use method_schedule_core::model::BoardId;
use std::path::PathBuf;

/// Board `board` as the page draws it, for the board beside the board shown (not an
/// edit, and the board shown stays as it is).
pub fn board_view<D: DocLock + ?Sized>(state: &D, board: u32) -> CmdResult<BoardDto> {
    let s = state.lock();
    view::board_ref(s.schedule(), board)?;
    view::board(s.schedule(), BoardId(board))
}

/// Takes image `at` off a sheet; the file stays where it is.
pub fn remove_sheet_image<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    at: usize,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| remove_image(x, sheet, at))?;
    finish(&window, &s, None)
}
