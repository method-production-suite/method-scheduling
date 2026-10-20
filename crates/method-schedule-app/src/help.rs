//! The Help menu's own content (decisions.md, entries 198, 199 and 212): the sample
//! schedule, the user guide, What's New, the licenses and notices, all built into the app,
//! so none needs a file on disk, a path or the network, and none can be written over; and
//! the three web pages it opens in the browser (the website, the suite's website and a
//! pre-filled bug report), a fixed list so the page can't ask for any other address.
//!
//! The sample is Field Station, the suite's shared invented feature
//! (`method_schedule_core::sample::field_station`, decision 223): Help > Open Sample opens it
//! as a new, untitled schedule, and so does the first launch with nothing to open, once
//! (SUITE-STANDARDS.md section 8). `resources/sample.mtsc` is the smaller demo production
//! (`method-schedule sample --demo` writes the same bytes; a test below holds the two
//! together), kept for the command line's tests. The guide is docs/GUIDE.md, shown
//! read-only in the page.

use tauri::{Manager, Runtime, WebviewWindow};

use crate::app::{finish, settle_unsaved, CmdResult};
use crate::docs::Doc;
use crate::safety_app::{self, Safety};
use crate::view::View;

/// The sample schedule's file text.
pub const SAMPLE: &str = include_str!("../resources/sample.mtsc");

/// The user guide, as Markdown (docs/GUIDE.md).
pub const GUIDE: &str = include_str!("../../../docs/GUIDE.md");

/// The licenses and the third-party notices (LICENSE-MIT, LICENSE-APACHE,
/// THIRD-PARTY-NOTICES.md from scripts/notices.py), for Help > Licenses and Notices.
pub const LICENSE_MIT: &str = include_str!("../../../LICENSE-MIT");
pub const LICENSE_APACHE: &str = include_str!("../../../LICENSE-APACHE");
pub const NOTICES: &str = include_str!("../../../THIRD-PARTY-NOTICES.md");

/// This version's release notes, docs/releases/<version>.md, empty when there are none
/// (build.rs copies the file, or writes it empty).
pub const WHATS_NEW: &str = include_str!(concat!(env!("OUT_DIR"), "/whats_new.md"));

/// The product's website, About's link.
pub const WEBSITE: &str = "https://methodschedule.com";
/// The Method Production Suite's website.
pub const SUITE: &str = "https://methodproduction.com";
/// A new issue from the bug form (.github/ISSUE_TEMPLATE/bug.yml).
pub const NEW_ISSUE: &str =
    "https://github.com/method-production-suite/method-scheduling/issues/new";

pub use method_schedule_session::session::{sample_session, SAMPLE_NOTICE};

/// Help > Open Sample: the unsaved-changes question when anything is unsaved, as New
/// asks, then the sample in place of the open schedule. `None` when cancelled.
#[tauri::command]
pub async fn open_sample<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<View>> {
    let next = sample_session()?;
    if !settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    safety_app::replace(&state, &window.state::<Safety>(), next);
    finish(&window, &state.lock(), Some(SAMPLE_NOTICE.into())).map(Some)
}

/// The host the page runs in (ui/js/host.js, docs/research/web-session.md): the desktop
/// app. A browser host answers `"web"`.
#[tauri::command]
pub fn host_info() -> method_schedule_session::host::HostInfo {
    method_schedule_session::host::HostInfo::new("desktop", false)
}

/// The menus a page draws itself, as a browser host gives them (the desktop draws its
/// native menu instead and the page never asks it; answered here so both hosts take the
/// same commands).
#[tauri::command]
pub fn menu_table() -> Vec<method_schedule_session::menu::MenuDto> {
    method_schedule_session::menu::web_menus()
}

/// Help > User Guide: the guide's text, for the page to show.
#[tauri::command]
pub fn user_guide() -> String {
    GUIDE.to_string()
}

/// `application/x-www-form-urlencoded`, as URLSearchParams writes it (Reader's
/// feedback.rs).
fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The bug form with this version and computer filled in: nothing else goes in the address,
/// no file, path or schedule.
pub fn bug_report_url(version: &str, os: &str) -> String {
    let platform = match os {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        _ => "",
    };
    let mut url = format!(
        "{NEW_ISSUE}?template=bug.yml&version={}",
        form_encode(version)
    );
    if !platform.is_empty() {
        url.push_str(&format!("&platform={platform}"));
    }
    url
}

/// The address behind one of Help's links, by name; no other address can be opened.
pub fn link(which: &str, version: &str) -> Result<String, String> {
    match which {
        "website" => Ok(WEBSITE.to_string()),
        "suite" => Ok(SUITE.to_string()),
        "bug" => Ok(bug_report_url(version, std::env::consts::OS)),
        _ => Err("That isn't a link the app opens.".to_string()),
    }
}

/// Opens `url` in the default browser, with the system's own opener.
fn open_external(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("The browser couldn't be opened: {e}"))
}

/// Help > Report a Bug, Help > methodproduction.com and About's website: opens a link from
/// the fixed list (`website`, `suite`, `bug`) in the browser.
#[tauri::command]
pub fn open_link<R: Runtime>(app: tauri::AppHandle<R>, which: String) -> CmdResult<()> {
    let url = link(&which, &app.package_info().version.to_string())?;
    open_external(&url)
}

/// The licenses and notices, for Help > Licenses and Notices.
#[derive(serde::Serialize)]
pub struct Notices {
    pub mit: &'static str,
    pub apache: &'static str,
    pub third_party: &'static str,
}

/// Help > Licenses and Notices: the two license texts and the third-party notices.
#[tauri::command]
pub fn notices() -> Notices {
    Notices {
        mit: LICENSE_MIT,
        apache: LICENSE_APACHE,
        third_party: NOTICES,
    }
}

/// What's New: this version and its notes (None when docs/releases has none for it).
#[derive(serde::Serialize)]
pub struct WhatsNew {
    pub version: String,
    pub notes: Option<&'static str>,
}

/// Help > What's New.
#[tauri::command]
pub fn whats_new<R: Runtime>(app: tauri::AppHandle<R>) -> WhatsNew {
    WhatsNew {
        version: app.package_info().version.to_string(),
        notes: (!WHATS_NEW.trim().is_empty()).then_some(WHATS_NEW),
    }
}

#[cfg(test)]
mod tests {
    /// Report a Bug's address carries the version and the computer and nothing else, and
    /// Help opens only its three links (decision 212).
    #[test]
    fn help_links_are_a_fixed_list_and_the_bug_report_says_only_the_version() {
        assert_eq!(
            bug_report_url("0.1.0", "macos"),
            "https://github.com/method-production-suite/method-scheduling/issues/new?template=bug.yml&version=0.1.0&platform=macOS"
        );
        assert_eq!(
            bug_report_url("1.2.0-beta.1", "plan9"),
            format!("{NEW_ISSUE}?template=bug.yml&version=1.2.0-beta.1")
        );
        assert_eq!(
            link("website", "0.1.0").unwrap(),
            "https://methodschedule.com"
        );
        assert_eq!(
            link("suite", "0.1.0").unwrap(),
            "https://methodproduction.com"
        );
        assert!(link("bug", "0.1.0").unwrap().starts_with(NEW_ISSUE));
        for other in ["", "https://example.com", "file:///etc/passwd", "WEBSITE"] {
            assert!(link(other, "0.1.0").is_err(), "{other}");
        }
    }

    /// The notices built in are the files on disk, and the third-party file is current
    /// enough to name the app's own framework.
    #[test]
    fn the_licenses_and_notices_are_built_in() {
        assert!(LICENSE_MIT
            .contains("Method Production Suite Team and the Method Schedule contributors"));
        assert!(LICENSE_APACHE.contains("Apache License"));
        assert!(NOTICES.starts_with("# Third-party notices"));
        assert!(NOTICES.contains("### tauri "));
    }

    /// The suite's identity rules (decision 211): document types named
    /// `com.methodschedule.<ext>`, the website, and the copyright line LICENSE-MIT has, with
    /// a publisher that names no company.
    #[test]
    fn the_bundle_follows_the_suite_identity() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], "com.methodschedule.desktop");
        let bundle = &conf["bundle"];
        assert_eq!(bundle["homepage"], "https://methodschedule.com");
        let copyright = bundle["copyright"].as_str().unwrap();
        let holder = copyright.trim_start_matches("Copyright (c) 2026 ");
        assert!(
            include_str!("../../../LICENSE-MIT").contains(holder),
            "{copyright}"
        );
        let publisher = bundle["publisher"].as_str().unwrap();
        for company in ["LLC", "Inc", "Ltd", "Post", "Darkwell", "Clockwork"] {
            assert!(!publisher.contains(company), "{publisher}");
        }
        let types: Vec<(String, String)> = bundle["fileAssociations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["ext"][0].as_str().unwrap().to_string(),
                    a["exportedType"]["identifier"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                )
            })
            .collect();
        assert_eq!(
            types,
            [
                ("mtsc", "com.methodschedule.mtsc"),
                ("mtst", "com.methodschedule.mtst")
            ]
            .map(|(e, u)| (e.to_string(), u.to_string()))
        );
    }

    use super::*;

    #[test]
    fn the_sample_is_the_engines_demo_and_reads_cleanly() {
        // The bundled file is exactly what `method-schedule sample --demo` writes. When the
        // demo or the format changes, write it again (delete the file first):
        //   cargo run -p method-schedule-cli -- sample --demo crates/method-schedule-app/resources/sample.mtsc
        let demo = method_schedule_core::sample::demo();
        assert_eq!(SAMPLE, method_schedule_format::to_string(&demo));
        // The strict reader runs the schedule's check.
        let read = method_schedule_format::from_str(SAMPLE).unwrap();
        read.check().unwrap();
        assert_eq!(read, demo);
    }

    #[test]
    fn the_sample_opens_untitled_with_nothing_unsaved() {
        let s = sample_session().unwrap();
        assert!(s.path().is_none(), "never its own file");
        assert_eq!(s.display_name(), "Untitled");
        assert!(!s.is_dirty());
        assert!(!s.can_autosave_to_file());
        assert_eq!(s.schedule().title, "Field Station (sample)");
        assert_eq!(s.schedule().sheets.len(), 90);
    }

    #[test]
    fn the_guide_keeps_the_house_voice_and_covers_its_sections() {
        assert!(!GUIDE.contains('\u{2014}'), "no em dashes");
        assert!(!GUIDE.contains('!'), "no exclamation marks");
        for heading in [
            "## Getting started",
            "## Coming from another scheduling app",
            "## The breakdown",
            "## The board and day breaks",
            "## The calendar and events",
            "## The day out of days, in plain words",
            "## Reports and exports",
            "## Red flags and conflicts",
            "## Merging and working together",
            "## The budget exchange file",
            "## Keyboard shortcuts",
            "## Files and backups",
        ] {
            assert!(GUIDE.contains(heading), "{heading}");
        }
        assert!(GUIDE.contains("Help > Open Sample"));
        assert!(GUIDE.contains("Help > Keyboard Shortcuts"));
        assert_eq!(user_guide(), GUIDE);
    }
}
