//! Method Schedule's open schedules without a window (docs/research/web-session.md,
//! decision 222): everything about an open schedule that needs no window, file or system
//! clock, so the same code runs in the desktop app and, built to WebAssembly, in a
//! browser.
//!
//! [`session`] holds the open schedule with undo and redo; [`board`], [`sheet`],
//! [`elements`] and [`calendar`] hold the edits; [`view`] is what the page draws;
//! [`clipboard`] cut, copy and paste; [`conflicts`], [`conflictmarks`] and [`redflags`]
//! the Conflicts tab, the marks on strips and red flags; [`explain`] the Day out of days
//! tab's Explain panel; [`find`] Find and Replace; [`events`], [`breakdown`] and
//! [`flagcolors`] moving calendar events, the breakdown sheet's design and red flag
//! colors; [`split`] a scene's images and the board beside the board; [`safety`] the
//! file-free half of keeping files safe; [`clock`] the time the host hands in. `method-schedule-app` re-exports these under their old
//! names; the rest of its window-free modules follow (docs/research/web-session.md).

pub mod board;
pub mod breakdown;
pub mod calendar;
pub mod clipboard;
pub mod clock;
pub mod conflictmarks;
pub mod conflicts;
pub mod elements;
pub mod events;
pub mod explain;
pub mod find;
pub mod flagcolors;
pub mod layouts;
pub mod plan;
pub mod redflags;
pub mod reports;
pub mod reportsetup;
pub mod safety;
pub mod session;
pub mod sheet;
pub mod split;
pub mod view;

pub use session::Session;
pub use view::View;

#[cfg(test)]
mod tests {
    /// Nothing outside src/clock.rs reads the standard clock: on wasm32 it compiles and
    /// then panics when called, so `cargo xtask wasm-check` can't catch it. Tests (after
    /// each file's `mod tests`) may, since they run on the desktop.
    #[test]
    fn only_the_clock_reads_the_time() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .is_some_and(|n| n == "clock.rs" || n == "lib.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let code = text.split("#[cfg(test)]\nmod tests").next().unwrap();
            for needle in ["SystemTime::now", "Instant::now", "process::id"] {
                if code.contains(needle) {
                    found.push(format!("{}: {needle}", path.display()));
                }
            }
        }
        assert!(
            found.is_empty(),
            "read the time through clock.rs: {found:?}"
        );
    }
}
