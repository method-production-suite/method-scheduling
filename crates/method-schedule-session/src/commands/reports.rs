//! The `reports` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `reports.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::reports::*;
use method_schedule_core::model::Schedule;
use method_schedule_format::pdf_info::PdfInfo;
use serde::Serialize;

/// Report `index` of the report list, for `sub_board`. `week_start` (`sun` or `mon`, the
/// user's preference) orders the calendar's weeks.
pub fn report_data<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    index: usize,
    week_start: Option<String>,
) -> CmdResult<ReportDto> {
    build_report(
        state.lock().schedule(),
        sub_board,
        index,
        week_start.as_deref().unwrap_or("sun"),
    )
}
