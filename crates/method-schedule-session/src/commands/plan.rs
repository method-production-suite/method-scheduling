//! The `plan` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `plan.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::plan::*;
use crate::session;
use crate::view::View;
use method_schedule_core::drift::{lock, unlock};
use method_schedule_core::fdx;
use method_schedule_core::fountain::ImportedScript;
use method_schedule_core::model::{BoardId, StripId};
use method_schedule_core::plan::{self as p};
use method_schedule_core::revise::{import_tagged, revise, revision_text};
use std::sync::Mutex;

/// The `timeline_data` command.
pub fn timeline_data<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    strip: Option<u32>,
) -> CmdResult<TimelineDto> {
    timeline_view(state.lock().schedule(), sub_board, strip)
}

/// The `push_past_wrap` command.
pub fn push_past_wrap<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    day: u32,
    strips: Vec<u32>,
) -> CmdResult {
    if strips.len() > 100_000 {
        return Err("Too many strips at once.".to_string());
    }
    let mut s = state.lock();
    let n = s.edit(|x| push_past(x, sub_board, day, &strips))?;
    let what = if n == 1 {
        "1 strip".to_string()
    } else {
        format!("{n} strips")
    };
    finish(
        &window,
        &s,
        Some(format!(
            "Moved {what} to the start of day {}. Undo puts them back.",
            day + 1
        )),
    )
}

/// The `set_call_time` command.
pub fn set_call_time<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    day: u32,
    call: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_call(x, day, &call))?;
    finish(&window, &s, None)
}

/// Sets one person's own call and wrap on a shoot day (decision 231); empty keeps the
/// company's.
pub fn set_person_call<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    day: u32,
    element: u32,
    call: String,
    wrap: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_own_call(x, day, element, &call, &wrap))?;
    finish(&window, &s, None)
}

/// Records what was shot of a scene on a shoot day (M5): its eighths, and whether that day
/// finished it.
pub fn set_shot<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    day: u32,
    eighths: String,
    complete: bool,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_scene_shot(x, sheet, day, &eighths, complete))?;
    finish(&window, &s, None)
}

/// The `set_long_day` command.
pub fn set_long_day<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    value: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_long(x, &value))?;
    finish(&window, &s, None)
}

/// The `locations_data` command.
pub fn locations_data<D: DocLock + ?Sized>(state: &D) -> CmdResult<Vec<LocationDto>> {
    Ok(locations_view(state.lock().schedule()))
}

/// The `set_location` command.
pub fn set_location<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
    latitude: String,
    longitude: String,
    utc_offset: String,
    daylight_saving: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| {
        place_location(
            x,
            &name,
            &latitude,
            &longitude,
            &utc_offset,
            &daylight_saving,
        )
    })?;
    finish(&window, &s, None)
}

/// The `rest_data` command.
pub fn rest_data<D: DocLock + ?Sized>(state: &D, board: u32) -> CmdResult<RestDto> {
    rest_view(state.lock().schedule(), board)
}

/// The `set_turnaround` command.
pub fn set_turnaround<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    value: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| set_minimum(x, category, &value))?;
    finish(&window, &s, None)
}

/// The `locks_data` command.
pub fn locks_data<D: DocLock + ?Sized>(state: &D, board: u32) -> CmdResult<Vec<LockDto>> {
    let s = state.lock();
    Ok(s.schedule()
        .plan
        .locks
        .iter()
        .filter(|l| l.board.0 == board)
        .map(|l| LockDto {
            name: l.name.clone(),
            made: l.made.map_or(String::new(), |d| d.to_string()),
            scenes: l.scenes.len(),
        })
        .collect())
}

/// The `lock_schedule` command.
pub fn lock_schedule<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    name: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| {
        if name.chars().count() > 200 {
            return Err("A lock's name is 200 characters at most.".to_string());
        }
        lock(x, BoardId(board), &name, today()).map_err(upper)
    })?;
    finish(
        &window,
        &s,
        Some(format!("Locked the schedule as {:?}.", name.trim())),
    )
}

/// The `delete_lock` command.
pub fn delete_lock<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| unlock(x, &name).map_err(upper))?;
    finish(&window, &s, None)
}

/// The `drift_data` command.
pub fn drift_data<D: DocLock + ?Sized>(state: &D, name: String) -> CmdResult<DriftDto> {
    drift_view(state.lock().schedule(), &name)
}

/// The `pin_strips` command.
pub fn pin_strips<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    strips: Vec<u32>,
    pinned: bool,
) -> CmdResult {
    if strips.len() > 100_000 {
        return Err("Too many strips at once.".to_string());
    }
    let ids: Vec<StripId> = strips.into_iter().map(StripId).collect();
    let mut s = state.lock();
    let n = s.edit(|x| {
        p::set_pinned(x, &ids, pinned).map_err(|_| "Only scene strips can be pinned.".to_string())
    })?;
    let what = if n == 1 {
        "1 strip".to_string()
    } else {
        format!("{n} strips")
    };
    let verb = if pinned { "Pinned" } else { "Unpinned" };
    finish(&window, &s, Some(format!("{verb} {what}.")))
}

/// The `suggest_order` command.
pub fn suggest_order<D: DocLock + ?Sized>(state: &D, sub_board: u32) -> CmdResult<SuggestionDto> {
    suggestion_view(state.lock().schedule(), sub_board)
}

/// The `apply_suggested_order` command.
pub fn apply_suggested_order<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    order: Vec<u32>,
) -> CmdResult {
    let mut s = state.lock();
    let n = s.edit(|x| apply_order(x, sub_board, &order))?;
    let what = if n == 1 {
        "1 scene".to_string()
    } else {
        format!("{n} scenes")
    };
    finish(
        &window,
        &s,
        Some(format!(
            "Moved {what} into the suggested order. Undo puts them back."
        )),
    )
}

/// Print Call Sheet: the same data as the JSON export (docs/CALLSHEET.md), for the page to
/// lay out as a printed sheet (decision 207).
pub fn call_sheet_data<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    day: u32,
) -> CmdResult<serde_json::Value> {
    let s = state.lock();
    let bytes = call_sheet_bytes(s.schedule(), sub_board, day, false)?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// How many shoot days a sub-board has, for the export's day picker.
pub fn shoot_day_count<D: DocLock + ?Sized>(state: &D, sub_board: u32) -> CmdResult<u32> {
    let s = state.lock();
    let (b, sb) = ids(s.schedule(), sub_board)?;
    Ok(method_schedule_core::days::days(s.schedule(), b, sb)
        .map_err(upper)?
        .days
        .len() as u32)
}
