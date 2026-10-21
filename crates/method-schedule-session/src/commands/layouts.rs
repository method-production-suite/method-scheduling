//! The `layouts` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `layouts.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::board::EditResult;
use crate::layouts::*;
use method_schedule_core::Schedule;
use std::path::{Path, PathBuf};

/// Adds a named layout (from the Strip Layout or Day Breaks dialog), and with `board` picks
/// it there, as one undo step.
pub fn add_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
    kind: String,
    layout: LayoutInput,
    board: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| {
        add_named(s, &name, &kind, &layout, board).map(|_| ())
    })
}

/// The `save_layout` command.
pub fn save_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
    layout: LayoutInput,
) -> CmdResult {
    edit_with(&window, &state, |s| save_named(s, id, &layout))
}

/// The `rename_layout` command.
pub fn rename_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
    name: String,
) -> CmdResult {
    edit_with(&window, &state, |s| rename_named(s, id, &name))
}

/// The `move_layout` command.
pub fn move_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
    to: usize,
) -> CmdResult {
    edit_with(&window, &state, |s| move_named(s, id, to))
}

/// The `delete_layout` command.
pub fn delete_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
) -> CmdResult {
    edit_with(&window, &state, |s| delete_named(s, id))
}

/// The `use_layout` command.
pub fn use_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    kind: String,
    id: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| pick_layout(s, board, &kind, id))
}

/// The `use_one_line_layout` command.
pub fn use_one_line_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: Option<u32>,
) -> CmdResult {
    edit_with(&window, &state, |s| pick_one_line(s, id))
}
