//! The app side of saving safety (src/safety.rs): the autosave timer, the Autosave switch,
//! crash recovery, backups, File > Open Recent and opening files the system hands the app
//! (double-click, Open With, a path on the command line). Method Budget's
//! `src/safety_app.rs`, scaled down to one schedule per window. Behavior: docs/APP.md,
//! "Saving and recovery".
//!
//! Paths still never come from the web view. They come from native dialogs, the operating
//! system (open events, the command line), the recent list and the recovery folder. The
//! page names a recent file by its place in the list and a recovery copy by its key, and
//! each is checked against what is actually there.
//!
//! Locks: `Safety::writer` is always taken before the session's lock, never while holding
//! it, so autosave, saves and opening files can't deadlock.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tauri::menu::{MenuItem, MenuItemKind, PredefinedMenuItem};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};

use crate::app::{self, AppState, CmdResult, MAIN};
use crate::docs::Doc;
use crate::menu::{AUTOSAVE, CLEAR_RECENT, OPEN_RECENT, RECENT_PREFIX};
use crate::safety::{
    self, Backups, RecentFiles, RecoveryEntry, RecoveryStore, Settings, BACKUP_GAP,
};
use crate::session::Session;
use crate::session::SessionFiles;
use crate::view::View;
use method_schedule_session::clock;

/// Events to the page.
/// A file to open arrived from the system; the payload is its name.
pub const OPEN_REQUEST: &str = "open-request";
/// The schedule was saved into its file without a command (autosave): the payload is the
/// view's `file` part.
pub const FILE_STATE: &str = "file-state";
/// The Autosave switch changed (from the menu); the payload is on or off.
pub const AUTOSAVE_CHANGED: &str = "autosave-changed";
/// Autosave couldn't write into the file; the payload is the message.
pub const AUTOSAVE_ERROR: &str = "autosave-error";

/// A file the system asked to open before the app finished starting (macOS can send it
/// before setup runs); `init` picks it up.
static EARLY_OPEN: Mutex<Option<PathBuf>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Recovery copies, backups, settings and the recent list, all in the app's local data
/// folder.
pub struct Safety {
    store: Option<RecoveryStore>,
    backups: Option<Backups>,
    settings_file: Option<PathBuf>,
    settings: Mutex<Settings>,
    recent: Mutex<RecentFiles>,
    /// Held while writing or removing recovery copies, saving and replacing the session.
    writer: Mutex<()>,
    /// A file the system asked us to open, waiting for the page.
    pending_open: Mutex<Option<PathBuf>>,
}

impl Safety {
    /// Everything under `dir` (the app's local data folder). Without one, nothing is kept
    /// between runs, and autosave into files still works.
    pub fn new(dir: Option<&Path>) -> Safety {
        let store = dir.and_then(|d| RecoveryStore::open(&d.join("Recovery")).ok());
        if let Some(s) = &store {
            s.tidy();
        }
        let settings_file = dir.map(|d| d.join("settings.json"));
        let settings = settings_file
            .as_deref()
            .map(Settings::load)
            .unwrap_or_default();
        Safety {
            store,
            backups: dir.map(|d| Backups::new(&d.join("Backups"))),
            settings_file,
            settings: Mutex::new(settings),
            recent: Mutex::new(RecentFiles::load(dir.map(|d| d.join("recent.json")))),
            writer: Mutex::new(()),
            pending_open: Mutex::new(None),
        }
    }

    pub fn autosave_on(&self) -> bool {
        lock(&self.settings).autosave
    }

    /// The user's display preferences (settings.json).
    pub fn prefs(&self) -> crate::reportsetup::Prefs {
        lock(&self.settings).prefs.clone()
    }

    pub fn set_prefs(&self, p: &crate::reportsetup::Prefs) {
        let mut s = lock(&self.settings);
        s.prefs = p.clone();
        if let Some(f) = &self.settings_file {
            let _ = s.save(f);
        }
    }

    pub fn set_autosave(&self, on: bool) {
        let mut s = lock(&self.settings);
        s.autosave = on;
        if let Some(f) = &self.settings_file {
            let _ = s.save(f);
        }
    }

    /// Held while writing or removing recovery copies, saving and replacing the session;
    /// always taken before the session's lock.
    pub(crate) fn write_lock(&self) -> MutexGuard<'_, ()> {
        lock(&self.writer)
    }

    pub(crate) fn remove_recovery(&self, s: &mut Session) {
        if let Some(store) = &self.store {
            let _ = store.remove(s.recovery_key());
        }
        s.recovery_removed();
    }

    /// Backs up what `path` holds now before a save replaces it: always for an explicit
    /// save and the first write in a session, else at most every [`BACKUP_GAP`]. Best
    /// effort: a backup that fails never stops a save.
    fn back_up(&self, s: &mut Session, path: &Path, explicit: bool) {
        let Some(b) = &self.backups else {
            return;
        };
        let gap = (!explicit && s.has_backed_up(path)).then_some(BACKUP_GAP);
        if b.back_up(path, gap).is_ok() {
            s.backed_up(path);
        }
    }

    /// Backs up what `path` holds now, whatever the gap since the last backup: a version
    /// about to be replaced on purpose (an outside change settled, a revert). Best effort.
    pub(crate) fn back_up_file(&self, path: &Path) {
        if let Some(b) = &self.backups {
            let _ = b.back_up(path, None);
        }
    }

    /// `document`'s rolling backups, oldest first (empty without a data folder).
    pub fn backups_of(&self, document: &Path) -> Vec<PathBuf> {
        self.backups
            .as_ref()
            .map(|b| b.list(document))
            .unwrap_or_default()
    }

    pub fn recent_paths(&self) -> Vec<PathBuf> {
        lock(&self.recent).paths().to_vec()
    }

    /// Lists `path` in Open Recent.
    pub fn remember(&self, path: &Path) {
        let _ = lock(&self.recent).add(path);
    }

    /// The app is exiting cleanly: lets go of this run's lock in the recovery folder and
    /// removes its lock file (recovery copies written for unsaved work stay). Waits for a
    /// write in progress first.
    pub fn close(&self) {
        let _w = lock(&self.writer);
        if let Some(store) = &self.store {
            store.close();
        }
    }

    /// Recovery copies left by a run that didn't finish, newest first.
    pub fn recovery_list(&self) -> Vec<RecoveryEntry> {
        self.store
            .as_ref()
            .map(RecoveryStore::list)
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// The core, without Tauri (tested below)

/// What one autosave pass did.
#[derive(Debug, PartialEq, Eq)]
pub enum Autosaved {
    Nothing,
    /// Saved into the schedule's file; nothing is unsaved now.
    File,
    /// Wrote the recovery copy (untitled, Autosave off, a recovered schedule not yet saved
    /// into its file, or a file changed outside the app).
    Recovery,
    /// Removed a recovery copy nothing needs any more.
    Removed,
    /// Couldn't save into the file (a recovery copy was kept instead).
    Failed(String),
    /// The file changed outside the app, so it was left alone and autosave into it is
    /// paused (a recovery copy was kept instead).
    Conflict(String),
}

/// One autosave pass at `now` (milliseconds on `clock::mono_ms`). `force` skips the
/// waiting (the window lost focus, the app is closing).
pub fn autosave(state: &AppState, safety: &Safety, now: u64, force: bool) -> Autosaved {
    let _w = lock(&safety.writer);
    if state.discarded() {
        return Autosaved::Nothing;
    }
    let mut s = state.lock();
    if s.recovery_stale() {
        safety.remove_recovery(&mut s);
        return Autosaved::Removed;
    }
    let due = s.is_dirty() && (force || s.autosave_due(now)) && !s.recovery_current();
    if !due {
        return Autosaved::Nothing;
    }
    let mut problem = None;
    if safety.autosave_on() && s.can_autosave_to_file() {
        let path = s.path().expect("can_autosave_to_file").to_path_buf();
        if s.changed_on_disk() {
            // Never write over someone else's changes: pause and say so.
            s.set_conflict(true);
            problem = Some(Autosaved::Conflict(format!(
                "{} changed on disk after it was opened here, so autosave stopped writing \
                 into it. Your changes are kept in a recovery copy. Use Save As to keep them \
                 in another file.",
                s.display_name()
            )));
        } else {
            safety.back_up(&mut s, &path, false);
            match s.save_to(&path) {
                Ok(()) => {
                    if s.recovery_stale() {
                        safety.remove_recovery(&mut s);
                    }
                    return Autosaved::File;
                }
                Err(e) => problem = Some(Autosaved::Failed(e.to_string())),
            }
        }
    }
    if let Some(store) = &safety.store {
        let title = s.schedule().title.clone();
        if store
            .write(s.recovery_key(), &s.file_text(), s.path(), &title)
            .is_ok()
        {
            s.recovery_written();
        }
    }
    problem.unwrap_or(Autosaved::Recovery)
}

/// An explicit Save (or Save As) to `path`: back up what is there, write the schedule
/// atomically (the outside-change guard refuses a file changed elsewhere), drop the
/// recovery copy and list the file in Open Recent. Unchanged on failure.
pub fn save(state: &AppState, safety: &Safety, path: &Path) -> Result<(), String> {
    let _w = lock(&safety.writer);
    let mut s = state.lock();
    let own = s.path().is_some_and(|p| safety::same_file(p, path));
    if own && s.changed_on_disk() {
        // Refused below without writing; autosave into it stays paused too.
        s.set_conflict(true);
    } else {
        safety.back_up(&mut s, path, true);
    }
    s.save_to(path).map_err(|e| e.to_string())?;
    if s.recovery_stale() {
        safety.remove_recovery(&mut s);
    }
    drop(s);
    safety.remember(path);
    Ok(())
}

/// Puts `next` in place of the open schedule. The old one's recovery copy goes: by now
/// its changes were saved or thrown away on purpose. The new schedule is kept safe as any
/// other (a Don't Save on the old one doesn't carry over).
pub fn replace(state: &AppState, safety: &Safety, next: Session) {
    let _w = lock(&safety.writer);
    let mut s = state.lock();
    if s.has_recovery() {
        safety.remove_recovery(&mut s);
    }
    *s = next;
    state.set_discarded(false);
}

/// Opens the schedule file at `path` in place of the open one and lists it in Open
/// Recent. A file that can't be read leaves everything as it was.
pub fn open_path(state: &AppState, safety: &Safety, path: &Path) -> Result<(), String> {
    let next = Session::open(path)?;
    replace(state, safety, next);
    safety.remember(path);
    Ok(())
}

/// Opens recovery copy `key` as an unsaved schedule pointing at its original file, in
/// place of the open one, and makes the copy this run's own (it stays until saved or
/// discarded).
pub fn open_recovery(state: &AppState, safety: &Safety, key: &str) -> Result<(), String> {
    let next = recovered(safety, key)?;
    adopt_recovery(state, safety, key, next)
}

/// Recovery copy `key` read as an unsaved schedule pointing at its original file; nothing
/// changes yet.
pub fn recovered(safety: &Safety, key: &str) -> Result<Session, String> {
    let store = safety
        .store
        .as_ref()
        .ok_or("There are no recovered schedules.")?;
    let (text, meta) = store.read(key).map_err(|e| e.to_string())?;
    Session::recovered(&text, meta.path.map(PathBuf::from), key.to_string())
}

/// Puts `next` (recovery copy `key`, from [`recovered`]) in place of the open schedule
/// and makes the copy this run's own.
pub fn adopt_recovery(
    state: &AppState,
    safety: &Safety,
    key: &str,
    next: Session,
) -> Result<(), String> {
    let store = safety
        .store
        .as_ref()
        .ok_or("There are no recovered schedules.")?;
    store.adopt(key).map_err(|e| e.to_string())?;
    replace(state, safety, next);
    Ok(())
}

/// Deletes recovery copy `key` (one left by an earlier run) and returns the rest.
pub fn discard_recovery(safety: &Safety, key: &str) -> Vec<RecoveryEntry> {
    if let Some(store) = &safety.store {
        if store.list().iter().any(|e| e.key == key) {
            let _ = store.remove(key);
        }
    }
    safety.recovery_list()
}

/// The user chose to close or quit without saving, and every window asked has settled:
/// nothing more is autosaved or kept as a recovery copy, and the recovery copy goes.
pub fn discard_all(state: &AppState, safety: &Safety) {
    state.set_discarded(true);
    let _w = lock(&safety.writer);
    let mut s = state.lock();
    safety.remove_recovery(&mut s);
}

/// The app is exiting. Changes the user didn't throw away are kept (into the file with
/// Autosave on, else a recovery copy), so an exit the guard didn't see (logging out, the
/// system quitting the app) loses nothing. A clean schedule leaves no recovery copy.
pub fn leaving(state: &AppState, safety: &Safety) {
    if state.discarded() {
        return;
    }
    autosave(state, safety, clock::mono_ms(), true);
    let _w = lock(&safety.writer);
    let mut s = state.lock();
    if !s.is_dirty() && s.has_recovery() {
        safety.remove_recovery(&mut s);
    }
}

/// The first command-line argument that names an existing file (Windows and Linux pass a
/// double-clicked file this way; options are skipped).
pub fn path_from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Option<PathBuf> {
    args.into_iter()
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

// ---------------------------------------------------------------------------
// Tauri wiring

/// Sets everything up when the app starts: the data folder, the menu's recent list and
/// Autosave check, a file named on the command line, and the autosave timer.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let dir = app.path().app_local_data_dir().ok();
    if let Some(d) = &dir {
        let _ = safety::create_private_dir(d);
    }
    app.manage(Safety::new(dir.as_deref()));
    refresh_menu(app);
    let early = lock(&EARLY_OPEN).take();
    if let Some(p) = early.or_else(|| path_from_args(std::env::args_os().skip(1))) {
        *lock(&app.state::<Safety>().pending_open) = Some(p);
    }
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        autosave_now(&handle, false);
    });
}

/// Runs an autosave pass on every window's schedule and tells each window what happened.
pub fn autosave_now<R: Runtime>(app: &AppHandle<R>, force: bool) {
    let (Some(docs), Some(safety)) = (
        app.try_state::<crate::docs::Docs>(),
        app.try_state::<Safety>(),
    ) else {
        return;
    };
    for (label, state) in docs.all() {
        autosave_one(app, &label, &state, &safety, force);
    }
}

fn autosave_one<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    state: &AppState,
    safety: &Safety,
    force: bool,
) {
    let done = autosave(state, safety, clock::mono_ms(), force);
    match done {
        Autosaved::File => {
            *lock(&state.autosave_error) = None;
            let (file, retitled) = {
                let s = state.lock();
                // Only queued setters under the lock (decisions.md, entry 88).
                let retitled = app
                    .get_webview_window(label)
                    .is_some_and(|w| state.retitle(&w, &s));
                (crate::view::file_dto(&s), retitled)
            };
            let _ = app.emit_to(label, FILE_STATE, file);
            if retitled {
                crate::windows::refresh_menu(app);
            }
        }
        Autosaved::Conflict(_) => {
            // The page keeps its own warning up while the file is in conflict.
            let file = crate::view::file_dto(&state.lock());
            let _ = app.emit_to(label, FILE_STATE, file);
        }
        Autosaved::Failed(msg) => {
            // Said once per window: another window's problem is its own.
            let mut last = lock(&state.autosave_error);
            if last.as_deref() != Some(msg.as_str()) {
                let text = if msg.contains("recovery copy") {
                    msg.clone()
                } else {
                    format!("Autosave couldn't save: {msg} A recovery copy was kept.")
                };
                let _ = app.emit_to(label, AUTOSAVE_ERROR, text);
                *last = Some(msg);
            }
        }
        _ => {}
    }
}

/// Before closing or quitting: with Autosave on, unsaved changes go into the file. True
/// when something is still unsaved afterwards (untitled, Autosave off, a conflict).
pub fn save_before_leaving<R: Runtime>(app: &AppHandle<R>) -> bool {
    autosave_now(app, true);
    app.try_state::<crate::docs::Docs>()
        .is_some_and(|d| d.all().iter().any(|(_, s)| s.lock().is_dirty()))
}

/// The app is exiting (any way at all): keeps what is unsaved (`leaving`), then releases
/// and removes this run's lock file. Tauri's managed state is never dropped on exit, so
/// this is the only place the lock goes away; without it every clean quit left an
/// `owner-*.lock` behind in `Recovery/` (first live run, docs/APP.md).
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    if let (Some(docs), Some(safety)) = (
        app.try_state::<crate::docs::Docs>(),
        app.try_state::<Safety>(),
    ) {
        for (_, state) in docs.all() {
            leaving(&state, &safety);
        }
        safety.close();
    }
}

/// The system asked to open `paths` (macOS: double-click, Open With, a drop on the Dock
/// icon). A window that already holds a schedule keeps it: the file opens in a window of
/// its own (decision 219). Only an untouched Untitled window takes it in place, once its
/// page asks for it (`open_pending`).
pub fn on_opened<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>) {
    let Some(path) = paths.into_iter().find(|p| p.is_file()) else {
        return;
    };
    let Some(safety) = app.try_state::<Safety>() else {
        *lock(&EARLY_OPEN) = Some(path);
        return;
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // A file already open in a window: that window comes to the front (decisions.md,
    // entry 86).
    if let Some(label) = app
        .try_state::<crate::docs::Docs>()
        .and_then(|d| d.holding(&path))
    {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.set_focus();
            return;
        }
    }
    let target = crate::windows::focused(app)
        .map(|w| w.label().to_string())
        .unwrap_or_else(|| MAIN.to_string());
    let untouched = app
        .try_state::<crate::docs::Docs>()
        .is_some_and(|d| d.get(&target).lock().is_untouched());
    if !untouched {
        let from = app.get_webview_window(&target);
        if open_in_own_window(app, &path, from.as_ref()).is_ok() {
            return;
        }
        // A file that can't be read is said in the window in front, as before.
    }
    *lock(&safety.pending_open) = Some(path);
    if let Some(w) = app.get_webview_window(&target) {
        let _ = w.set_focus();
    }
    // If the page isn't listening yet, it asks for the pending file itself when it starts.
    let _ = app.emit_to(target.as_str(), OPEN_REQUEST, name);
}

/// Opens `path` in a new window, cascaded from `from`, and adds it to Open Recent.
fn open_in_own_window<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
    from: Option<&WebviewWindow<R>>,
) -> Result<(), String> {
    let next = Session::open(path)?;
    if next.path().is_some() {
        app.state::<Safety>().remember(path);
    }
    crate::windows::open_window(app, next, from)
        .map(|_| ())
        .map_err(|e| format!("A new window couldn't be opened: {e}"))
}

// ---------------------------------------------------------------------------
// Menu

/// The menu item `id` among `items` and their submenus.
pub(crate) fn find<R: Runtime>(items: Vec<MenuItemKind<R>>, id: &str) -> Option<MenuItemKind<R>> {
    for item in items {
        if item.id() == id {
            return Some(item);
        }
        if let MenuItemKind::Submenu(sub) = &item {
            if let Some(found) = find(sub.items().unwrap_or_default(), id) {
                return Some(found);
            }
        }
    }
    None
}

/// Brings Open Recent and the Autosave check up to date. The work runs on the main thread
/// once it is free, so this never waits for it (decisions.md, entry 88).
pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || rebuild_menu(&handle));
}

fn rebuild_menu<R: Runtime>(app: &AppHandle<R>) {
    let (Some(menu), Some(safety)) = (app.menu(), app.try_state::<Safety>()) else {
        return;
    };
    let items = menu.items().unwrap_or_default();
    if let Some(MenuItemKind::Check(c)) = find(items.clone(), AUTOSAVE) {
        let _ = c.set_checked(safety.autosave_on());
    }
    let Some(MenuItemKind::Submenu(recent)) = find(items, OPEN_RECENT) else {
        return;
    };
    let labels = {
        let mut r = lock(&safety.recent);
        r.prune();
        r.labels()
    };
    let _ = (|| -> tauri::Result<()> {
        while recent.remove_at(0)?.is_some() {}
        for (i, label) in labels.iter().enumerate() {
            let id = format!("{RECENT_PREFIX}{i}");
            recent.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
        }
        if !labels.is_empty() {
            recent.append(&PredefinedMenuItem::separator(app)?)?;
        }
        recent.append(&MenuItem::with_id(
            app,
            CLEAR_RECENT,
            "Clear Menu",
            !labels.is_empty(),
            None::<&str>,
        )?)?;
        Ok(())
    })();
}

fn set_autosave_everywhere<R: Runtime>(app: &AppHandle<R>, on: bool) {
    app.state::<Safety>().set_autosave(on);
    refresh_menu(app);
    // Every window's switch follows.
    let _ = app.emit(AUTOSAVE_CHANGED, on);
}

/// Menu items answered here: Clear Menu and the Autosave check. False for anything else
/// (the caller passes it on).
pub fn handle_menu<R: Runtime>(app: &AppHandle<R>, id: &str) -> bool {
    match id {
        CLEAR_RECENT => {
            let _ = lock(&app.state::<Safety>().recent).clear();
            refresh_menu(app);
            true
        }
        AUTOSAVE => {
            let on = !app.state::<Safety>().autosave_on();
            set_autosave_everywhere(app, on);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Commands

/// Turns Autosave on or off (the top bar's switch); returns the new setting.
#[tauri::command]
pub fn set_autosave<R: Runtime>(app: AppHandle<R>, on: bool) -> bool {
    set_autosave_everywhere(&app, on);
    on
}

/// Opens the file the system asked to open, if there is one, in place of the open
/// schedule after asking about unsaved changes. `None` when there is none or the user
/// cancelled. Refused, before the question, when another window has the file open.
#[tauri::command]
pub async fn open_pending<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    safety: State<'_, Safety>,
) -> CmdResult<Option<View>> {
    let Some(path) = lock(&safety.pending_open).take() else {
        return Ok(None);
    };
    crate::windows::refuse_if_open_elsewhere(window.app_handle(), &path, window.label())?;
    if !app::settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    open_path(&state, &safety, &path)?;
    refresh_menu(window.app_handle());
    app::finish(&window, &state.lock(), None).map(Some)
}

/// Opens File > Open Recent's `index`-th file in place of the open schedule, after asking
/// about unsaved changes. `None` when cancelled.
#[tauri::command]
pub async fn open_recent<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    safety: State<'_, Safety>,
    index: usize,
) -> CmdResult<Option<View>> {
    let Some(path) = safety.recent_paths().get(index).cloned() else {
        return Ok(None);
    };
    if crate::windows::focus_holder(window.app_handle(), &path, window.label()) {
        return Ok(None);
    }
    if !app::settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    let opened = open_path(&state, &safety, &path);
    // A file that has gone drops out of the menu either way.
    refresh_menu(window.app_handle());
    opened?;
    app::finish(&window, &state.lock(), None).map(Some)
}

/// Recovery copies left by a run of the app that didn't finish (a crash, a forced quit, a
/// power cut), newest first.
#[tauri::command]
pub fn recovery_list<R: Runtime>(
    window: WebviewWindow<R>,
    safety: State<'_, Safety>,
) -> Vec<RecoveryEntry> {
    // Offered once, in the window the app starts with, not in every new window.
    if window.label() != MAIN {
        return Vec::new();
    }
    safety.recovery_list()
}

/// Opens recovery copy `key` as an unsaved schedule, after asking about unsaved changes.
/// `None` when cancelled. Refused, before the question, when its file is open in another
/// window.
#[tauri::command]
pub async fn open_recovered<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    safety: State<'_, Safety>,
    key: String,
) -> CmdResult<Option<View>> {
    let next = recovered(&safety, &key)?;
    if let Some(p) = next.path() {
        crate::windows::refuse_if_open_elsewhere(window.app_handle(), p, window.label())?;
    }
    if !app::settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    adopt_recovery(&state, &safety, &key, next)?;
    app::finish(&window, &state.lock(), None).map(Some)
}

/// Deletes recovery copy `key` for good and returns the rest.
#[tauri::command]
pub fn discard_recovered(safety: State<'_, Safety>, key: String) -> Vec<RecoveryEntry> {
    discard_recovery(&safety, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::safety::tests::temp_dir;
    use crate::safety::AUTOSAVE_QUIET;
    use std::fs;

    fn rename(state: &AppState, title: &str) {
        state
            .lock()
            .edit(|x| {
                x.title = title.into();
                Ok(())
            })
            .unwrap();
    }

    fn later() -> u64 {
        clock::mono_ms() + AUTOSAVE_QUIET.as_millis() as u64 + 1_000
    }

    #[test]
    fn autosave_writes_into_the_file_after_a_quiet_spell() {
        let dir = temp_dir("autosave-file");
        let data = dir.join("data");
        let safety = Safety::new(Some(&data));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        assert_eq!(safety.recent_paths().len(), 1, "a save lists the file");

        rename(&state, "Changed");
        // Still typing: nothing yet.
        assert_eq!(
            autosave(&state, &safety, clock::mono_ms(), false),
            Autosaved::Nothing
        );
        assert_eq!(autosave(&state, &safety, later(), false), Autosaved::File);
        assert!(!state.lock().is_dirty());
        assert_eq!(
            Session::open(&path).unwrap().schedule().title,
            "Changed",
            "the file holds the change"
        );
        // The version it replaced was backed up.
        let backups = Backups::new(&data.join("Backups"));
        assert_eq!(backups.list(&path).len(), 1);
        // Nothing to do when nothing changed.
        assert_eq!(autosave(&state, &safety, later(), true), Autosaved::Nothing);
        // Autosaves back up again only after the gap.
        rename(&state, "Again");
        assert_eq!(autosave(&state, &safety, later(), false), Autosaved::File);
        assert_eq!(backups.list(&path).len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn untitled_and_autosave_off_get_recovery_copies() {
        let dir = temp_dir("autosave-recovery");
        let data = dir.join("data");
        let safety = Safety::new(Some(&data));
        let state = AppState::default();
        rename(&state, "Never saved");
        assert_eq!(
            autosave(&state, &safety, later(), false),
            Autosaved::Recovery
        );
        assert!(state.lock().is_dirty(), "still unsaved");
        // Written once per change.
        assert_eq!(autosave(&state, &safety, later(), true), Autosaved::Nothing);

        // Autosave off: a file with a path gets a recovery copy, not a write.
        safety.set_autosave(false);
        assert!(
            !Safety::new(Some(&data)).autosave_on(),
            "the setting is kept"
        );
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        assert!(!state.lock().has_recovery(), "saving drops the copy");
        let on_disk = fs::read_to_string(&path).unwrap();
        rename(&state, "Off");
        assert_eq!(
            autosave(&state, &safety, later(), false),
            Autosaved::Recovery
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            on_disk,
            "file untouched"
        );
        // Undo back to the saved state: the copy goes on the next pass.
        state.lock().undo();
        assert_eq!(
            autosave(&state, &safety, later(), false),
            Autosaved::Removed
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_crash_leaves_a_copy_the_next_run_offers_and_opens() {
        let dir = temp_dir("autosave-crash");
        let data = dir.join("data");
        let path = dir.join("film.mtsc");
        {
            let safety = Safety::new(Some(&data));
            safety.set_autosave(false);
            let state = AppState::default();
            save(&state, &safety, &path).unwrap();
            let sb = state.lock().schedule().boards[0].sub_boards[0].id.0;
            state
                .lock()
                .edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
                .unwrap();
            assert_eq!(
                autosave(&state, &safety, later(), false),
                Autosaved::Recovery
            );
            // The app dies here: no leaving(), the lock is released.
        }
        let safety = Safety::new(Some(&data));
        let list = safety.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "film.mtsc");
        let state = AppState::default();
        open_recovery(&state, &safety, &list[0].key).unwrap();
        {
            let s = state.lock();
            assert!(s.is_dirty());
            assert_eq!(s.path(), Some(path.as_path()));
            assert!(s.needs_overwrite_confirm());
            assert!(!s.can_autosave_to_file(), "the original waits for a Save");
            let start = s.schedule().boards[0].sub_boards[0].start;
            assert_eq!(start.map(|d| d.to_string()).as_deref(), Some("2027-03-01"));
        }
        // Adopted: not offered again while this run lives.
        assert!(safety.recovery_list().is_empty());
        // Saving into the file drops the copy for good.
        save(&state, &safety, &path).unwrap();
        assert!(!state.lock().has_recovery());
        drop(safety);
        assert!(Safety::new(Some(&data)).recovery_list().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discarding_and_leaving() {
        let dir = temp_dir("autosave-leave");
        let data = dir.join("data");
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            rename(&state, "Keep me");
            // Quitting without the guard (logging out): the changes are kept.
            leaving(&state, &safety);
        }
        let safety = Safety::new(Some(&data));
        let list = safety.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Keep me");
        assert!(discard_recovery(&safety, &list[0].key).is_empty());
        assert!(discard_recovery(&safety, "../nope").is_empty());
        {
            let state = AppState::default();
            rename(&state, "Throw away");
            autosave(&state, &safety, later(), true);
            discard_all(&state, &safety);
            leaving(&state, &safety);
            assert_eq!(autosave(&state, &safety, later(), true), Autosaved::Nothing);
            // A schedule put in its place is kept safe again: Don't Save doesn't carry
            // over.
            replace(&state, &safety, Session::new());
            assert!(!state.discarded());
            rename(&state, "Next one");
            assert_eq!(
                autosave(&state, &safety, later(), true),
                Autosaved::Recovery
            );
            discard_all(&state, &safety);
        }
        drop(safety);
        assert!(Safety::new(Some(&data)).recovery_list().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn autosave_never_writes_over_a_file_changed_elsewhere() {
        let dir = temp_dir("autosave-guard");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        fs::write(&path, "their version").unwrap();
        rename(&state, "Mine");
        let Autosaved::Conflict(msg) = autosave(&state, &safety, later(), false) else {
            panic!("expected a conflict");
        };
        assert!(msg.contains("Save As") && !msg.contains('!'), "{msg}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "their version");
        assert!(state.lock().has_recovery(), "the work is kept");
        // Paused: the next change goes to the recovery copy only.
        rename(&state, "Mine 2");
        assert_eq!(
            autosave(&state, &safety, later(), false),
            Autosaved::Recovery
        );
        // An explicit Save is refused too; Save As works and autosave carries on there.
        assert!(save(&state, &safety, &path).is_err());
        let other = dir.join("mine.mtsc");
        save(&state, &safety, &other).unwrap();
        assert!(state.lock().can_autosave_to_file());
        assert_eq!(fs::read_to_string(&path).unwrap(), "their version");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn opening_files_replaces_the_schedule_and_lists_them() {
        let dir = temp_dir("open-path");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let a = dir.join("a.mtsc");
        save(&state, &safety, &a).unwrap();
        rename(&state, "Untitled work");
        autosave(&state, &safety, later(), true);
        let fresh = AppState::default();
        let b = dir.join("b.mtsc");
        save(&fresh, &safety, &b).unwrap();
        // Opening b in place of the edited a (whose changes were autosaved into a).
        open_path(&state, &safety, &b).unwrap();
        assert_eq!(state.lock().path(), Some(b.as_path()));
        assert_eq!(safety.recent_paths()[0], fs::canonicalize(&b).unwrap());
        // A file that can't be read leaves everything as it was.
        assert!(open_path(&state, &safety, &dir.join("missing.mtsc")).is_err());
        assert_eq!(state.lock().path(), Some(b.as_path()));
        // Without a data folder, autosave into files still works.
        let bare = Safety::new(None);
        rename(&state, "No data folder");
        assert_eq!(autosave(&state, &bare, later(), false), Autosaved::File);
        assert!(bare.recovery_list().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    fn lock_files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".lock"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn a_clean_exit_leaves_no_lock_file() {
        let dir = temp_dir("clean-exit");
        let data = dir.join("data");
        let recovery = data.join("Recovery");
        // Quit with nothing unsaved.
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            assert_eq!(lock_files(&recovery).len(), 1, "a running copy holds one");
            leaving(&state, &safety);
            safety.close();
            assert!(lock_files(&recovery).is_empty());
            // Closing twice, and the drop afterwards, are harmless.
            safety.close();
        }
        // Quit and choose Don't Save.
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            rename(&state, "Throw away");
            autosave(&state, &safety, later(), true);
            discard_all(&state, &safety);
            leaving(&state, &safety);
            safety.close();
            assert!(lock_files(&recovery).is_empty());
        }
        // Quit without the guard (logging out): the work is kept and offered next time,
        // and still no lock file stays.
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            rename(&state, "Keep me");
            leaving(&state, &safety);
            safety.close();
            assert!(lock_files(&recovery).is_empty());
        }
        let next = Safety::new(Some(&data));
        let list = next.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Keep me");
        assert_eq!(lock_files(&recovery).len(), 1, "only the new run's");
        drop(next);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn command_line_paths() {
        let dir = temp_dir("args");
        let f = dir.join("film.mtsc");
        fs::write(&f, "{}").unwrap();
        let args = ["--flag", "missing.mtsc", f.to_str().unwrap()].map(std::ffi::OsString::from);
        assert_eq!(path_from_args(args), Some(f.clone()));
        assert_eq!(
            path_from_args(["-psn_0_1"].map(std::ffi::OsString::from)),
            None
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
