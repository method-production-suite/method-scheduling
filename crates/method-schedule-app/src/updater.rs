//! In-app updates and What's New once after an update (decision 213; the suite standard,
//! SUITE-STANDARDS.md section 1). Method Budget's updater (`crates/method-app/src/updater.rs`
//! there) is the model: the same state machine, timing, host rule and settings file.
//!
//! The updater plugin (`tauri-plugin-updater`) is driven from here only. No window is
//! granted the plugin's own commands: the page has three narrow commands of ours
//! (`update_status`, `update_restart`, `update_whats_new`) and hears `update-status`
//! events; Help > Check for Updates… and the Check for Updates Automatically switch are
//! answered here.
//!
//! - Two minutes after launch, then every six hours (unless turned off), the app fetches
//!   the manifest from the endpoint in `tauri.conf.json` (`plugins.updater.endpoints`). A
//!   newer version downloads in the background; the plugin checks its minisign signature
//!   against the public key the app was built with before anything is kept. Then the page
//!   shows a quiet "Update ready" with Restart to Update.
//! - Nothing interrupts: an automatic check that fails says nothing. A check the user asked
//!   for answers in the status line: up to date, or couldn't check.
//! - Restart to Update goes the way Quit does: each window's unsaved changes get the
//!   question (Save, Don't Save, Cancel), then the update is installed and the app starts
//!   again.
//! - After an update, What's New for the new version shows once (`update_whats_new`);
//!   Help > What's New shows it again (src/help.rs).
//!
//! It is inactive (no timer, no network; a check the user asks for says why) while
//! `tauri.conf.json` carries no public key, which is until the owner makes the key
//! (decision 197 kept updates off until the repository is public), and on Windows and
//! Linux, whose updates come later. The only hosts it may reach are the endpoint's and, for
//! GitHub, its two download hosts, over https, redirects included.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::MenuItemKind;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::safety::{create_private_dir, write_atomically, Access};

/// Help > Check for Updates…, answered here.
pub const CHECK_ITEM: &str = "help.check_updates";
/// Help > Check for Updates Automatically (a check item), answered here.
pub const AUTO_ITEM: &str = "help.auto_updates";
/// The event every window hears; the payload is a [`Status`].
pub const STATUS_EVENT: &str = "update-status";

/// The first automatic check, after the app starts.
pub const FIRST_CHECK: Duration = Duration::from_secs(2 * 60);
/// Then one every this long, while the app runs.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the timer looks at the clock (and at the setting).
const TICK: Duration = Duration::from_secs(30);
/// A check that takes longer gives up.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// A download that takes longer gives up (and is tried again next time).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// GitHub serves a release's files from these, after a redirect from github.com.
const GITHUB_DOWNLOAD_HOSTS: &[&str] = &[
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// The core, without Tauri (tested below)

/// Where an update is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// `manual`: the user asked (Help > Check for Updates), so the page says what is
    /// happening and how it ended.
    Checking {
        manual: bool,
    },
    Downloading {
        version: String,
        manual: bool,
    },
    /// Downloaded and its signature checked: Restart to Update installs it.
    Ready {
        version: String,
    },
    Installing {
        version: String,
    },
}

/// A one-time answer the page shows in the status line, then lets go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Said {
    /// A check the user asked for found nothing newer.
    Current,
    /// A check (or its download) the user asked for didn't work.
    Failed,
    /// The user asked, but this copy doesn't update itself (no key yet, or not macOS).
    Unavailable,
    /// Restart to Update couldn't install; the update is still ready.
    InstallFailed,
}

/// What a check asks for next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Go ahead and check.
    Check,
    /// A check or download is already under way (now counted as asked for, if this was).
    Busy,
    /// An update is ready or installing already.
    Ready,
}

/// The state machine of an update. Every change goes through here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub phase: Phase,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { phase: Phase::Idle }
    }
}

impl Machine {
    pub fn start_check(&mut self, manual: bool) -> Start {
        match &mut self.phase {
            Phase::Idle => {
                self.phase = Phase::Checking { manual };
                Start::Check
            }
            Phase::Checking { manual: m } | Phase::Downloading { manual: m, .. } => {
                *m |= manual;
                Start::Busy
            }
            Phase::Ready { .. } | Phase::Installing { .. } => Start::Ready,
        }
    }

    /// The check finished: `Ok(Some(v))` found version `v`, `Ok(None)` found nothing
    /// newer, `Err` didn't work. True when `v` should be downloaded; and what to tell a
    /// user who asked.
    pub fn checked(&mut self, found: Result<Option<String>, ()>) -> (bool, Option<Said>) {
        let Phase::Checking { manual } = self.phase else {
            return (false, None);
        };
        match found {
            Ok(Some(version)) => {
                self.phase = Phase::Downloading { version, manual };
                (true, None)
            }
            Ok(None) => {
                self.phase = Phase::Idle;
                (false, manual.then_some(Said::Current))
            }
            Err(()) => {
                self.phase = Phase::Idle;
                (false, manual.then_some(Said::Failed))
            }
        }
    }

    /// The download finished (`ok`: downloaded and its signature checked).
    pub fn downloaded(&mut self, ok: bool) -> Option<Said> {
        let Phase::Downloading { version, manual } = &self.phase else {
            return None;
        };
        let manual = *manual;
        if ok {
            self.phase = Phase::Ready {
                version: version.clone(),
            };
            None
        } else {
            self.phase = Phase::Idle;
            manual.then_some(Said::Failed)
        }
    }

    /// Restart to Update was chosen and nothing unsaved stands in the way: the version to
    /// install, if one is ready.
    pub fn start_install(&mut self) -> Option<String> {
        let Phase::Ready { version } = &self.phase else {
            return None;
        };
        let v = version.clone();
        self.phase = Phase::Installing { version: v.clone() };
        Some(v)
    }

    /// Installing failed: the update stays ready for another try.
    pub fn install_failed(&mut self) -> Said {
        if let Phase::Installing { version } = &self.phase {
            self.phase = Phase::Ready {
                version: version.clone(),
            };
        }
        Said::InstallFailed
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready { .. })
    }

    /// What the page is told.
    pub fn status(&self, automatic: bool, available: bool, said: Option<Said>) -> Status {
        let (phase, version, manual) = match &self.phase {
            Phase::Idle => ("idle", None, false),
            Phase::Checking { manual } => ("checking", None, *manual),
            Phase::Downloading { version, manual } => {
                ("downloading", Some(version.clone()), *manual)
            }
            Phase::Ready { version } => ("ready", Some(version.clone()), false),
            Phase::Installing { version } => ("installing", Some(version.clone()), false),
        };
        Status {
            phase,
            version,
            manual,
            said,
            automatic,
            available,
        }
    }
}

/// What the page hears (`update-status`, `update_status`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    /// "idle", "checking", "downloading", "ready" or "installing".
    pub phase: &'static str,
    /// The update's version, once one was found.
    pub version: Option<String>,
    /// The user asked for this check: show its progress.
    pub manual: bool,
    /// A one-time answer to show (then let go).
    pub said: Option<Said>,
    /// Automatic checks are on.
    pub automatic: bool,
    /// This copy can update itself.
    pub available: bool,
}

/// Why this copy can't update itself: `None` when it can. No public key (until the owner
/// makes one), a system other than macOS, or a missing or non-https endpoint all mean no
/// checks at all.
pub fn unavailable_reason(pubkey: &str, endpoints: &[String], os: &str) -> Option<String> {
    if pubkey.trim().is_empty() {
        return Some("no updater key yet".into());
    }
    if os != "macos" {
        return Some("updates on this system come later".into());
    }
    if endpoints.is_empty() {
        return Some("no update endpoint".into());
    }
    if endpoints.iter().any(|e| !e.starts_with("https://")) {
        return Some("an update endpoint isn't https".into());
    }
    None
}

/// The hosts the updater may reach: each endpoint's, plus GitHub's download hosts when an
/// endpoint is on github.com.
pub fn allowed_hosts(endpoints: &[String]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for e in endpoints {
        let Ok(url) = tauri::Url::parse(e) else {
            continue;
        };
        if let Some(h) = url.host_str() {
            if !hosts.iter().any(|x| x == h) {
                hosts.push(h.to_string());
            }
            if h == "github.com" {
                for g in GITHUB_DOWNLOAD_HOSTS {
                    if !hosts.iter().any(|x| x == g) {
                        hosts.push((*g).to_string());
                    }
                }
            }
        }
    }
    hosts
}

/// True when a request to `scheme://host` is one the updater may make.
pub fn allowed(scheme: &str, host: Option<&str>, hosts: &[String]) -> bool {
    scheme == "https" && host.is_some_and(|h| hosts.iter().any(|a| a == h))
}

/// The updater's settings, `updates.json` in the app's data folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Check now and then without being asked. On unless turned off.
    pub automatic: bool,
    /// The version whose What's New was shown last (or that ran first).
    pub seen: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            automatic: true,
            seen: None,
        }
    }
}

impl UpdateSettings {
    /// Reads `file`; missing or unreadable means the defaults.
    pub fn load(file: &Path) -> UpdateSettings {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            create_private_dir(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_atomically(file, &bytes, Access::Private)
    }
}

/// Whether What's New is due for `current`, given the version last `seen`: (show it, the
/// version to remember now). A first run shows nothing (it isn't an update); a newer
/// version shows it (and is remembered once shown); going back to an older one shows
/// nothing.
pub fn whats_new_due(seen: Option<&str>, current: &str) -> (bool, Option<String>) {
    let Ok(now) = semver::Version::parse(current) else {
        return (false, None);
    };
    match seen.map(semver::Version::parse) {
        None | Some(Err(_)) => (false, Some(current.to_string())),
        Some(Ok(before)) if before < now => (true, None),
        Some(Ok(before)) if before == now => (false, None),
        Some(Ok(_)) => (false, Some(current.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Tauri wiring

/// The updater's state for the app's life.
pub struct Updates {
    machine: Mutex<Machine>,
    /// The downloaded update, ready to install.
    pending: Mutex<Option<(Update, Vec<u8>)>>,
    settings: Mutex<UpdateSettings>,
    settings_file: Option<PathBuf>,
    /// Why this copy can't update itself (`None`: it can).
    unavailable: Option<String>,
    hosts: Vec<String>,
    /// What's New is to be shown once (decided at the start).
    whats_new_due: Mutex<bool>,
    current: String,
}

impl Updates {
    pub fn automatic(&self) -> bool {
        lock(&self.settings).automatic
    }

    fn set_automatic(&self, on: bool) {
        let mut s = lock(&self.settings);
        s.automatic = on;
        if let Some(f) = &self.settings_file {
            let _ = s.save(f);
        }
    }

    fn status(&self, said: Option<Said>) -> Status {
        lock(&self.machine).status(self.automatic(), self.unavailable.is_none(), said)
    }
}

/// The updater settings `tauri.conf.json` carries: the public key and the endpoints.
fn configured<R: Runtime>(app: &AppHandle<R>) -> (String, Vec<String>) {
    let conf = app
        .config()
        .plugins
        .0
        .get("updater")
        .cloned()
        .unwrap_or_default();
    let key = conf["pubkey"].as_str().unwrap_or_default().to_string();
    let endpoints = conf["endpoints"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (key, endpoints)
}

/// Tells every window where the update is.
fn tell<R: Runtime>(app: &AppHandle<R>, said: Option<Said>) {
    if let Some(u) = app.try_state::<Updates>() {
        let _ = app.emit(STATUS_EVENT, u.status(said));
    }
}

/// Sets everything up when the app starts: the settings, whether What's New is due, the
/// menu's check, and the timer for automatic checks.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let current = app.package_info().version.to_string();
    let settings_file = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|d| d.join("updates.json"));
    let mut settings = settings_file
        .as_deref()
        .map(UpdateSettings::load)
        .unwrap_or_default();
    let (due, remember) = whats_new_due(settings.seen.as_deref(), &current);
    if let Some(v) = remember {
        settings.seen = Some(v);
        if let Some(f) = &settings_file {
            let _ = settings.save(f);
        }
    }
    let (key, endpoints) = configured(app);
    let unavailable = unavailable_reason(&key, &endpoints, std::env::consts::OS);
    app.manage(Updates {
        machine: Mutex::new(Machine::default()),
        pending: Mutex::new(None),
        settings: Mutex::new(settings),
        settings_file,
        hosts: allowed_hosts(&endpoints),
        unavailable,
        whats_new_due: Mutex::new(due),
        current,
    });
    refresh_menu(app);
    if app.state::<Updates>().unavailable.is_some() {
        // Never checks: no timer, no network.
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut next = Instant::now() + FIRST_CHECK;
        loop {
            std::thread::sleep(TICK);
            if Instant::now() < next {
                continue;
            }
            next = Instant::now() + CHECK_EVERY;
            if handle.state::<Updates>().automatic() {
                check_now(&handle, false);
            }
        }
    });
}

/// The redirect rule for the updater's requests: https, to the allowed hosts only, at most
/// ten hops.
fn redirect_policy(hosts: Vec<String>) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects");
        }
        if allowed(attempt.url().scheme(), attempt.url().host_str(), &hosts) {
            attempt.follow()
        } else {
            attempt.error("a redirect to a host the updater doesn't use")
        }
    })
}

/// Asks the endpoint for a newer version.
async fn fetch<R: Runtime>(
    app: &AppHandle<R>,
    hosts: Vec<String>,
) -> Result<Option<Update>, String> {
    let client_hosts = hosts.clone();
    let updater = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .configure_client(move |b| {
            b.https_only(true)
                .redirect(redirect_policy(client_hosts.clone()))
        })
        .build()
        .map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    if let Some(u) = &update {
        let url = &u.download_url;
        if !allowed(url.scheme(), url.host_str(), &hosts) {
            return Err(format!(
                "the update is on a host the updater doesn't use ({url})"
            ));
        }
    }
    Ok(update)
}

/// One check, then the download if it found something. Blocks: run it on a thread of its
/// own. `manual`: the user asked (Help > Check for Updates).
pub fn check_now<R: Runtime>(app: &AppHandle<R>, manual: bool) {
    let updates = app.state::<Updates>();
    if updates.unavailable.is_some() {
        if manual {
            tell(app, Some(Said::Unavailable));
        }
        return;
    }
    let start = lock(&updates.machine).start_check(manual);
    tell(app, None);
    if start != Start::Check {
        return;
    }
    let found = tauri::async_runtime::block_on(fetch(app, updates.hosts.clone()));
    let version = match &found {
        Ok(Some(u)) => Ok(Some(u.version.clone())),
        Ok(None) => Ok(None),
        Err(_) => Err(()),
    };
    let (download, said) = lock(&updates.machine).checked(version);
    tell(app, said);
    let (true, Ok(Some(mut update))) = (download, found) else {
        return;
    };
    update.timeout = Some(DOWNLOAD_TIMEOUT);
    // The plugin checks the signature against the app's public key before it hands the
    // bytes over; a bad one is an error here.
    let bytes = tauri::async_runtime::block_on(update.download(|_, _| {}, || {}));
    let said = match bytes {
        Ok(bytes) => {
            *lock(&updates.pending) = Some((update, bytes));
            lock(&updates.machine).downloaded(true)
        }
        Err(_) => lock(&updates.machine).downloaded(false),
    };
    tell(app, said);
}

/// Installs the ready update and starts the app again. Runs on a thread of its own.
fn install_and_restart<R: Runtime>(app: &AppHandle<R>) {
    let updates = app.state::<Updates>();
    if lock(&updates.machine).start_install().is_none() {
        return;
    }
    tell(app, None);
    let pending = lock(&updates.pending).take();
    let result = match &pending {
        Some((update, bytes)) => update.install(bytes).map_err(|e| e.to_string()),
        None => Err("the update is gone".to_string()),
    };
    match result {
        Ok(()) => app.restart(),
        Err(_) => {
            *lock(&updates.pending) = pending;
            let said = lock(&updates.machine).install_failed();
            tell(app, Some(said));
        }
    }
}

/// Brings the Check for Updates Automatically check up to date.
pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let (Some(menu), Some(updates)) = (handle.menu(), handle.try_state::<Updates>()) else {
            return;
        };
        if let Some(MenuItemKind::Check(c)) =
            crate::safety_app::find(menu.items().unwrap_or_default(), AUTO_ITEM)
        {
            let _ = c.set_checked(updates.automatic());
        }
    });
}

/// Menu items answered here: Check for Updates… and the automatic switch. False for
/// anything else (the caller passes it on).
pub fn handle_menu<R: Runtime>(app: &AppHandle<R>, id: &str) -> bool {
    match id {
        CHECK_ITEM => {
            let handle = app.clone();
            std::thread::spawn(move || check_now(&handle, true));
            true
        }
        AUTO_ITEM => {
            if let Some(u) = app.try_state::<Updates>() {
                let on = !u.automatic();
                u.set_automatic(on);
            }
            refresh_menu(app);
            tell(app, None);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Commands (registered in app.rs, build.rs and capabilities/main.json)

/// Where an update is, for the page's quiet indicator.
#[tauri::command]
pub fn update_status(updates: State<'_, Updates>) -> Status {
    updates.status(None)
}

/// Restart to Update: the way Quit goes (each window's unsaved changes get the question),
/// then the ready update is installed and the app starts again.
#[tauri::command]
pub fn update_restart<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if !lock(&app.state::<Updates>().machine).is_ready() {
        return Err("No update is ready to install.".into());
    }
    let handle = app.clone();
    let all = app.webview_windows().into_values().collect();
    crate::app::confirm_close(all, move || {
        std::thread::spawn(move || install_and_restart(&handle));
    });
    Ok(())
}

/// What's New for this version, once after an update: its notes (src/help.rs), or `None`
/// otherwise and from then on.
#[tauri::command]
pub fn update_whats_new(updates: State<'_, Updates>) -> Option<&'static str> {
    {
        let mut due = lock(&updates.whats_new_due);
        if !*due {
            return None;
        }
        *due = false;
    }
    {
        let mut s = lock(&updates.settings);
        s.seen = Some(updates.current.clone());
        if let Some(f) = &updates.settings_file {
            let _ = s.save(f);
        }
    }
    let notes = crate::help::WHATS_NEW;
    (!notes.trim().is_empty()).then_some(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT: &str =
        "https://github.com/method-production-suite/method-scheduling/releases/latest/download/latest.json";

    #[test]
    fn automatic_checks_are_quiet() {
        let mut m = Machine::default();
        assert_eq!(m.start_check(false), Start::Check);
        assert_eq!(m.status(true, true, None).phase, "checking");
        assert!(!m.status(true, true, None).manual);
        // Nothing newer, or no network: back to idle, nothing said.
        assert_eq!(m.checked(Ok(None)), (false, None));
        assert_eq!(m.phase, Phase::Idle);
        m.start_check(false);
        assert_eq!(m.checked(Err(())), (false, None));
        // A newer version downloads, then waits for Restart to Update.
        m.start_check(false);
        assert_eq!(m.checked(Ok(Some("0.2.0".into()))), (true, None));
        assert_eq!(m.downloaded(true), None);
        assert!(m.is_ready());
        assert_eq!(m.status(true, true, None).version.as_deref(), Some("0.2.0"));
        // While one is ready, another check does nothing.
        assert_eq!(m.start_check(true), Start::Ready);
    }

    #[test]
    fn a_check_the_user_asked_for_answers() {
        let mut m = Machine::default();
        m.start_check(true);
        assert_eq!(m.checked(Ok(None)), (false, Some(Said::Current)));
        m.start_check(true);
        assert_eq!(m.checked(Err(())), (false, Some(Said::Failed)));
        // An automatic check under way becomes asked-for when the user asks too.
        m.start_check(false);
        assert_eq!(m.start_check(true), Start::Busy);
        assert_eq!(m.checked(Ok(Some("0.2.0".into()))), (true, None));
        assert_eq!(m.downloaded(false), Some(Said::Failed));
        assert_eq!(m.phase, Phase::Idle);
    }

    #[test]
    fn a_failed_install_keeps_the_update_ready() {
        let mut m = Machine::default();
        m.start_check(false);
        m.checked(Ok(Some("0.2.0".into())));
        m.downloaded(true);
        assert_eq!(m.start_install().as_deref(), Some("0.2.0"));
        assert_eq!(m.status(true, true, None).phase, "installing");
        assert_eq!(m.install_failed(), Said::InstallFailed);
        assert!(m.is_ready());
        // Nothing to install when nothing is ready.
        let mut idle = Machine::default();
        assert_eq!(idle.start_install(), None);
    }

    #[test]
    fn no_key_or_not_macos_means_no_checks() {
        let ep = vec![ENDPOINT.to_string()];
        assert_eq!(
            unavailable_reason("", &ep, "macos").as_deref(),
            Some("no updater key yet")
        );
        assert_eq!(
            unavailable_reason("  ", &ep, "macos").as_deref(),
            Some("no updater key yet")
        );
        assert!(unavailable_reason("KEY", &ep, "windows").is_some());
        assert!(unavailable_reason("KEY", &ep, "linux").is_some());
        assert!(unavailable_reason("KEY", &[], "macos").is_some());
        assert!(
            unavailable_reason("KEY", &["http://example.com/latest.json".into()], "macos")
                .is_some()
        );
        assert_eq!(unavailable_reason("KEY", &ep, "macos"), None);
    }

    #[test]
    fn requests_stay_on_the_endpoints_hosts_over_https() {
        let hosts = allowed_hosts(&[ENDPOINT.to_string()]);
        assert_eq!(
            hosts,
            [
                "github.com",
                "objects.githubusercontent.com",
                "release-assets.githubusercontent.com"
            ]
        );
        assert!(allowed("https", Some("github.com"), &hosts));
        assert!(allowed(
            "https",
            Some("release-assets.githubusercontent.com"),
            &hosts
        ));
        assert!(!allowed("http", Some("github.com"), &hosts));
        assert!(!allowed("https", Some("example.com"), &hosts));
        assert!(!allowed("https", None, &hosts));
    }

    #[test]
    fn whats_new_shows_once_after_an_update_only() {
        // A first run is not an update: nothing shows, this version is remembered.
        assert_eq!(whats_new_due(None, "0.1.0"), (false, Some("0.1.0".into())));
        assert_eq!(
            whats_new_due(Some("not a version"), "0.1.0"),
            (false, Some("0.1.0".into()))
        );
        // A newer version shows it.
        assert_eq!(whats_new_due(Some("0.1.0"), "0.2.0"), (true, None));
        assert_eq!(whats_new_due(Some("0.2.0-beta.1"), "0.2.0"), (true, None));
        // The same version, or an older one, doesn't.
        assert_eq!(whats_new_due(Some("0.2.0"), "0.2.0"), (false, None));
        assert_eq!(
            whats_new_due(Some("0.3.0"), "0.2.0"),
            (false, Some("0.2.0".into()))
        );
    }

    #[test]
    fn settings_are_on_by_default_and_survive_a_save() {
        let dir = std::env::temp_dir().join(format!("ms-updates-{}", std::process::id()));
        let file = dir.join("updates.json");
        assert_eq!(UpdateSettings::load(&file), UpdateSettings::default());
        assert!(UpdateSettings::default().automatic);
        let s = UpdateSettings {
            automatic: false,
            seen: Some("0.1.0".into()),
        };
        s.save(&file).unwrap();
        assert_eq!(UpdateSettings::load(&file), s);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(UpdateSettings::load(&file), UpdateSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// tauri.conf.json: the endpoint is the release's latest.json, and the public key is
    /// the owner's minisign key 460428DB6D38CE76 (decision 214), so this copy may update
    /// itself on macOS.
    #[test]
    fn the_endpoint_is_the_latest_release_and_the_key_is_the_owners() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let up = &conf["plugins"]["updater"];
        assert_eq!(up["endpoints"], serde_json::json!([ENDPOINT]));
        // "untrusted comment: minisign public key: 460428DB6D38CE76", base64.
        let key = up["pubkey"].as_str().unwrap();
        assert!(key.starts_with(
            "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQ2MDQyOERCNkQzOENFNzYK"
        ));
        assert_eq!(
            unavailable_reason(key, &[ENDPOINT.to_string()], "macos"),
            None
        );
    }
}
