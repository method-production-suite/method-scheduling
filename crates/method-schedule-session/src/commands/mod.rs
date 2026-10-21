//! The commands behind the page, shared by both hosts (docs/research/web-session.md): each
//! desktop command in method-schedule-app is a one-line call to its function here, and the
//! browser host's dispatch (`crate::host`) calls the same function. One body for each
//! command, so the two can't drift apart.
//!
//! A command takes the window it runs for as a [`Shell`] (the desktop's window, or the
//! browser host's stand-in) and the open schedule as a [`DocLock`]. The helpers below
//! ([`finish`], [`edit`] and the rest) are the ones every command uses.

use std::sync::MutexGuard;

use crate::session::Session;
use crate::view::View;

pub mod app;
pub mod breakdown;
pub mod events;
pub mod find;
pub mod flagcolors;
pub mod layouts;
pub mod plan;
pub mod reports;
pub mod reportsetup;
pub mod split;

/// A command's reply: its value or the message the page shows.
pub type CmdResult<T = View> = Result<T, String>;

/// What a command needs of the window it runs for.
pub trait Shell {
    /// The view after a command, with `notice` to show: the desktop also keeps its
    /// window's title in step and reads its Autosave switch.
    fn finish(&self, s: &Session, notice: Option<String>) -> CmdResult;
}

impl<T: Shell + ?Sized> Shell for &T {
    fn finish(&self, s: &Session, notice: Option<String>) -> CmdResult {
        (**self).finish(s, notice)
    }
}

/// The open schedule a command acts on.
pub trait DocLock {
    fn lock(&self) -> MutexGuard<'_, Session>;
}

impl<T: DocLock + ?Sized> DocLock for &T {
    fn lock(&self) -> MutexGuard<'_, Session> {
        (**self).lock()
    }
}

/// The view after a command.
pub fn finish<W: Shell + ?Sized>(window: &W, s: &Session, notice: Option<String>) -> CmdResult {
    window.finish(s, notice)
}

/// One edit as one undo step, then the view.
pub fn edit<W: Shell + ?Sized, D: DocLock + ?Sized, T>(
    window: &W,
    state: &D,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<T>,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(f)?;
    finish(window, &s, None)
}

/// The layouts module's name for [`edit`].
pub fn edit_with<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult,
) -> CmdResult {
    edit(window, state, f)
}

/// An edit that makes a strip, which the view names so the board selects it.
pub fn strip_edit<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<u32>,
) -> CmdResult {
    let mut s = state.lock();
    let id = s.edit(f)?;
    let mut v = finish(window, &s, None)?;
    v.strip = Some(id);
    Ok(v)
}

/// An edit that makes a board, which is then shown.
pub fn board_edit<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<u32>,
) -> CmdResult {
    let mut s = state.lock();
    let id = s.edit(f)?;
    s.show_board(method_schedule_core::model::BoardId(id))?;
    finish(window, &s, None)
}

/// An edit that picks the Breakdown tab's sheet.
pub fn sheet_edit<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<Option<u32>>,
) -> CmdResult {
    let mut s = state.lock();
    let show = s.edit(f)?;
    let mut v = finish(window, &s, None)?;
    v.sheet = show;
    Ok(v)
}

/// The Breakdown tab's stepping order from the page's key.
pub fn sheet_order(key: Option<&str>) -> CmdResult<method_schedule_core::model::SheetOrder> {
    match key {
        None | Some("") => Ok(method_schedule_core::model::SheetOrder::Board),
        Some(k) => method_schedule_core::model::SheetOrder::parse(k)
            .ok_or_else(|| "Step by board order, sheet number or scene number.".into()),
    }
}

/// Today where the user is, from the installed clock (src/clock.rs).
pub fn today() -> Option<method_schedule_core::Date> {
    crate::clock::today()
}
