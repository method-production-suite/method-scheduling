//! The `reportsetup` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `reportsetup.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::board::EditResult;
use crate::reportsetup::*;
use method_schedule_core::headers::HeaderId;
use method_schedule_core::model::Schedule;

/// The report list, the production info and what the filters can pick.
pub fn report_list<D: DocLock + ?Sized>(state: &D) -> CmdResult<ReportsDto> {
    Ok(reports_dto(state.lock().schedule()))
}

/// The `save_report` command.
pub fn save_report<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    index: usize,
    name: String,
    options: OptionsDto,
) -> CmdResult {
    edit(&window, &state, |s| apply_report(s, index, &name, &options))
}

/// Adds a copy right after report `index`; the view's notice names it.
pub fn duplicate_report<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    index: usize,
) -> CmdResult {
    let mut s = state.lock();
    let at = s.edit(|x| x.duplicate_report(index).map_err(say))?;
    let name = s
        .schedule()
        .report_list()
        .get(at)
        .map_or(String::new(), |r| r.name.clone());
    finish(&window, &s, Some(format!("Added {name}.")))
}

/// The `move_report` command.
pub fn move_report<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    index: usize,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| s.move_report(index, to).map_err(say))
}

/// The `delete_report` command.
pub fn delete_report<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    index: usize,
) -> CmdResult {
    edit(&window, &state, |s| s.delete_report(index).map_err(say))
}

/// Saves a header layout (id 0: a new one); the notice names it.
pub fn save_header_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    design: HeaderDto,
) -> CmdResult {
    let mut s = state.lock();
    let added = design.id == 0;
    s.edit(|x| apply_header(x, &design).map(|_| ()))?;
    let note = added.then(|| format!("Added header layout {}.", design.name.trim()));
    finish(&window, &s, note)
}

/// The `delete_header_layout` command.
pub fn delete_header_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
) -> CmdResult {
    edit(&window, &state, |s| {
        s.delete_header(HeaderId(id)).map_err(header_say)
    })
}

/// The `move_header_layout` command.
pub fn move_header_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    id: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        s.move_header(HeaderId(id), to).map_err(header_say)
    })
}

/// `field`: `title` or a production field's key (`company`, `art_director`...).
pub fn set_production<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    field: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| apply_production(s, &field, &value))
}

/// `codes`: the letters to change, by key (`hold`, `company_move`...).
pub fn set_dood_codes<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    codes: std::collections::BTreeMap<String, String>,
) -> CmdResult {
    edit(&window, &state, |s| apply_codes(s, &codes))
}

/// The `set_prep_wrap` command.
pub fn set_prep_wrap<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    prep_start: String,
    wrap_end: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        apply_prep_wrap(s, sub_board, &prep_start, &wrap_end)
    })
}

/// Calendar > Strip Fields: what a strip shows on a date, kept in the schedule.
pub fn set_calendar_fields<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    fields: Vec<String>,
) -> CmdResult {
    edit(&window, &state, |s| apply_calendar_fields(s, &fields))
}
