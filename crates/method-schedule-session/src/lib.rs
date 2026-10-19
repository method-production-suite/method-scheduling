//! Method Schedule's open schedules without a window (docs/research/web-session.md,
//! decision 222): everything about an open schedule that needs no window, file or system
//! clock, so the same code runs in the desktop app and, built to WebAssembly, in a
//! browser.
//!
//! [`session`] holds the open schedule with undo and redo; [`board`], [`sheet`],
//! [`elements`] and [`calendar`] hold the edits; [`view`] is what the page draws;
//! [`clipboard`] cut, copy and paste; [`conflicts`], [`conflictmarks`] and [`redflags`]
//! the Conflicts tab, the marks on strips and red flags; [`explain`] the Day out of days
//! tab's Explain panel; [`safety`] the file-free half of keeping files safe; [`clock`]
//! the time the host hands in. `method-schedule-app` re-exports these under their old
//! names; the rest of its window-free modules follow (docs/research/web-session.md).

pub mod board;
pub mod calendar;
pub mod clipboard;
pub mod clock;
pub mod conflictmarks;
pub mod conflicts;
pub mod elements;
pub mod explain;
pub mod redflags;
pub mod safety;
pub mod session;
pub mod sheet;
pub mod view;

pub use session::Session;
pub use view::View;
