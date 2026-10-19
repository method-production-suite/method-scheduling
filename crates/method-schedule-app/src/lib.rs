//! The scheduling desktop app.
//!
//! [`app`], [`menu`] and the wiring half of [`safety_app`] are the Tauri glue. The rest is
//! plain Rust with no window, and the parts that need no file either live in
//! `method-schedule-session` and are re-exported here: [`session`] holds the open schedule with undo and redo,
//! [`board`], [`sheet`], [`elements`] and [`calendar`] hold the edits, [`view`] is what
//! the page draws, [`reports`] the Reports tab's data (and printing), [`export`] the
//! spreadsheet and budget exports, [`explain`] the Day out of days tab's Explain panel,
//! [`find`] Find and Replace, [`clipboard`] cut, copy and paste, [`conflicts`] the Conflicts tab, [`redflags`] red flags and their clashes, [`diffmerge`] What Changed and Merge Changes From, [`outside`] changes made outside the app and Revert to Backup, [`help`] the sample schedule and the user guide, [`safety`] keeps files safe (atomic
//! writes, recovery copies, backups, settings, recent files) and [`safety_app`]'s core runs
//! autosave and recovery. The page
//! itself is static HTML, CSS and JavaScript in `ui/`; see docs/APP.md.

pub mod app;
pub mod breakdown;
#[cfg(test)]
mod budgets;
pub mod diffmerge;
pub mod docs;
pub mod elements;
pub mod events;
pub mod export;
pub mod find;
pub mod flagcolors;
pub mod help;
pub mod layouts;
pub mod menu;
#[cfg(test)]
mod moved_tests;
pub mod outside;
#[cfg(target_os = "macos")]
mod pdf_mac;
pub mod plan;
pub mod reports;
pub mod reportsetup;
pub mod safety;
pub mod safety_app;
#[cfg(test)]
mod safety_crash;
pub mod session;
pub mod split;
pub mod templates;
pub mod updater;
pub mod windows;

// Window-free, in method-schedule-session (docs/research/web-session.md), under their old
// names here. `session` and `elements` are the desktop's own modules on top of theirs.
pub use method_schedule_session::{
    board, calendar, clipboard, conflictmarks, conflicts, explain, redflags, sheet, view,
};

pub use app::run;
pub use session::Session;
pub use view::View;
