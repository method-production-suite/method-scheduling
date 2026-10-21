//! The `flagcolors` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `flagcolors.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::flagcolors::*;

/// Red Flag Types > a type's color, one undo step.
pub fn set_red_flag_type_color<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    red_flag_type: u32,
    color: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_color(x, red_flag_type, &color))?;
    finish(&window, &s, None)
}
