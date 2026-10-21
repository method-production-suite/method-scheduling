//! The `find` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `find.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::find::*;
use crate::view::View;

/// Every place `query` matches, in board order.
pub fn find_text<D: DocLock + ?Sized>(state: &D, query: QueryArg) -> CmdResult<FoundDto> {
    find(state.lock().schedule(), &query)
}

/// Replaces `query`'s matches by `with` in the checked results `picks` (their keys), as
/// one undo step. The view carries a notice saying how many places changed.
pub fn replace_text<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    query: QueryArg,
    with: String,
    picks: Vec<String>,
) -> CmdResult<View> {
    let mut s = state.lock();
    let notice = replace(&mut s, &query, &with, &picks)?;
    finish(&window, &s, Some(notice))
}
