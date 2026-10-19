//! Method Schedule's open schedules without a window (docs/research/web-session.md,
//! decision 222): everything about an open schedule that needs no window, file or system
//! clock, so the same code runs in the desktop app and, built to WebAssembly, in a
//! browser.
//!
//! It fills in phase by phase. Today it holds the injected clock; the window-free
//! modules of `method-schedule-app` move here next, and the app re-exports them.

pub mod clock;
