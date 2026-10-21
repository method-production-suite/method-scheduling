//! The `breakdown` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `breakdown.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::breakdown::*;

/// Breakdown > Breakdown Designer: lays the sheet's fields out in `fields`' order, hides
/// `hidden`, sets `widths` (key to `narrow`, `medium` or `wide`) and `rows` (how many
/// shown fields each row holds; empty for the usual rows).
pub fn set_sheet_field_order<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
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
