//! The scheduling engine. Zero dependencies, integers only: page lengths in eighths,
//! durations in whole minutes, dates as ISO `YYYY-MM-DD` (decisions.md, entry 3).

pub mod callsheet;
pub mod conflicts;
pub mod date;
pub mod days;
pub mod diff;
pub mod dood;
pub mod drift;
pub mod eighths;
pub mod explain;
pub mod extensions;
pub mod fdx;
pub mod find;
pub mod fountain;
pub mod headers;
pub mod layouts;
pub mod merge;
pub mod model;
pub mod optimize;
pub mod plan;
pub mod redflags;
pub mod report_data;
pub mod reports;
pub mod rest;
pub mod revise;
pub mod sample;
pub mod setup;
pub mod solar;
pub mod timeline;

/// Seeded randomness for property tests and the sample schedule; with the `testgen`
/// feature, random schedules and file mutations for other crates' tests. Not part of the
/// engine's API.
#[doc(hidden)]
pub mod testgen;

#[cfg(test)]
mod import_fuzz;

pub use date::{Date, Weekday};
pub use eighths::Eighths;
pub use model::Schedule;
