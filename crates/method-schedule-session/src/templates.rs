//! User templates (decisions.md, entries 77 and 85) without a window: a template's text,
//! and a new untitled schedule from one. The desktop's dialogs are in method-schedule-app.

use method_schedule_core::Schedule;

use crate::session::{self, Session};

/// A template file's working extension.
pub const EXTENSION: &str = "mtst";

/// A new, untitled session from a template's schedule (or any schedule, taken as one).
pub fn session_from(schedule: &Schedule) -> Session {
    let mut t = schedule.to_template();
    t.title = session::UNTITLED.to_string();
    Session::with_schedule(t, None)
}

/// The template text for a schedule: our own file, marked as a template.
pub fn template_text(s: &Schedule) -> String {
    method_schedule_format::to_template_string(s)
}
