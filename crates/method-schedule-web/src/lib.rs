//! Method Schedule in a browser: [`method_schedule_session::host::Host`] compiled to
//! WebAssembly with a small JS API, for the existing page (`crates/method-schedule-app/ui/`)
//! to run against through `js/tauri-shim.js` (docs/research/web-session.md, phase 2).
//!
//! The JS API (wasm-bindgen, `--target web`):
//!
//! ```js
//! import init, { Host, commands } from './method_schedule_web.js';
//! await init();
//! const host = new Host();                        // no schedule open yet
//! JSON.parse(host.dispatch('open_sample', '{}'));  // {doc, view}
//! JSON.parse(host.openMtsc(text, 'Pilot.mtsc'));   // {doc, view}
//! const { name, text } = host.saveMtsc();          // the schedule in front
//! host.setReadOnly(true);                          // a viewer
//! JSON.parse(host.hostInfo());                     // {kind: "web", commands_version, read_only}
//! host.putImage('images/truck.png', bytes);        // the picture store, as a Uint8Array
//! host.image('images/truck.png');                  // a Uint8Array, or undefined
//! commands();                                      // every command dispatch runs
//! ```
//!
//! `dispatch` and the others throw an `Error` whose `message` is the text the page shows.
//! The clock is the browser's (`Date.now()`), installed by the first `new Host()`; names
//! that must be unique are seeded from `crypto.getRandomValues`.
//!
//! On the desktop the crate holds only [`mtsc_name`]: everything else is for wasm32 alone.

#[cfg(target_arch = "wasm32")]
mod web;

/// The name a schedule is downloaded under: `name` with the `.mtsc` extension (a schedule
/// opened from `.json` saves as a schedule file), and a plain name when it has none.
pub fn mtsc_name(name: &str) -> String {
    let name = name.trim();
    let ext = method_schedule_session::session::EXTENSION;
    if name.is_empty() {
        return format!("{}.{ext}", method_schedule_session::session::UNTITLED);
    }
    let stem = match name.rsplit_once('.') {
        Some((_, e)) if e.eq_ignore_ascii_case(ext) => return name.into(),
        Some((stem, e)) if !stem.is_empty() && !e.contains(' ') && e.len() <= 5 => stem,
        _ => name,
    };
    format!("{stem}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_download_as_mtsc_files() {
        assert_eq!(mtsc_name("Pilot.mtsc"), "Pilot.mtsc");
        assert_eq!(mtsc_name("Pilot.MTSC"), "Pilot.MTSC");
        assert_eq!(mtsc_name("pilot.json"), "pilot.mtsc");
        assert_eq!(
            mtsc_name("Field Station (sample)"),
            "Field Station (sample).mtsc"
        );
        assert_eq!(mtsc_name("Draft 2.0 notes"), "Draft 2.0 notes.mtsc");
        assert_eq!(mtsc_name("  "), "Untitled.mtsc");
    }
}
