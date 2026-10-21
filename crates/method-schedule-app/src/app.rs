//! Tauri glue: the commands the page invokes, native file dialogs, the unsaved-changes
//! guard on close and quit, and the navigation guard.
//!
//! Security notes (docs/APP.md, "Security model"): file paths only ever come from native
//! dialogs shown here in Rust, never from the web view; every command works on the
//! schedule of the window that called it ([`AppState`], one per window, src/docs.rs);
//! navigation away from the bundled UI is blocked.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

use crate::board;
use crate::calendar::EventInput;
use crate::clipboard;
use crate::docs::Doc;
use crate::elements;
use crate::explain::ExplainDto;
use crate::export::with_extension;
use crate::redflags;
use crate::safety_app::{self, Safety};
use crate::session::{self, Session};
use crate::sheet;
use crate::view::{self, CalendarDto, DoodGridDto, ElementDaysDto, ElementsDto, SheetDto, View};

/// The product's working name (decisions.md, entry 4). It changes with the name.
pub(crate) const APP_NAME: &str = "Method Schedule";
/// The main window's label.
pub(crate) const MAIN: &str = "main";

pub(crate) type CmdResult<T = View> = Result<T, String>;

/// One window's open schedule (src/docs.rs keeps one per window, decisions.md, entry
/// 86) and what belongs to that window alone.
///
/// Lock rule (decisions.md, entry 88): while the session's lock is held, nothing may wait
/// for the main thread. Window and menu getters (`title()`, `is_focused()`, menu items)
/// and blocking dialogs do; setters (`set_title`) and `run_on_main_thread` only queue.
/// The title last set is kept here, so nothing asks the window for it.
pub struct AppState {
    session: Mutex<Session>,
    /// The user chose Don't Save on the way out and every window has settled.
    discarded: AtomicBool,
    /// The window's title as last set (the Window menu lists these).
    title: Mutex<String>,
    /// The unsaved-changes question is up for this window's close.
    closing: AtomicBool,
    /// The last autosave problem said in this window, so it is said once.
    pub(crate) autosave_error: Mutex<Option<String>>,
    /// Files picked for merging in this window, by token.
    pub(crate) merge_files: crate::diffmerge::MergeFiles,
}

impl Default for AppState {
    fn default() -> AppState {
        AppState::with(Session::new())
    }
}

fn hold<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Session> {
        // A panic while holding the lock leaves the session as it was before the edit
        // (`Session::edit` works on a copy), so it is safe to keep using.
        hold(&self.session)
    }

    /// A window's schedule starting from `session`.
    pub(crate) fn with(session: Session) -> AppState {
        AppState {
            title: Mutex::new(window_title(&session)),
            session: Mutex::new(session),
            discarded: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            autosave_error: Mutex::new(None),
            merge_files: Default::default(),
        }
    }

    /// The user chose Don't Save for this schedule on the way out: keep no recovery copy.
    pub(crate) fn discarded(&self) -> bool {
        self.discarded.load(Ordering::SeqCst)
    }

    pub(crate) fn set_discarded(&self, on: bool) {
        self.discarded.store(on, Ordering::SeqCst);
    }

    /// The window's title as last set.
    pub(crate) fn title(&self) -> String {
        hold(&self.title).clone()
    }

    /// Gives `window` the title for `s`, if it changed: true when it did (the Window menu
    /// then needs refreshing). Only queues the change, so safe under the session's lock.
    pub(crate) fn retitle<R: Runtime>(&self, window: &WebviewWindow<R>, s: &Session) -> bool {
        let title = window_title(s);
        let mut last = hold(&self.title);
        if *last == title {
            return false;
        }
        let _ = window.set_title(&title);
        *last = title;
        true
    }

    /// Marks the window as asking about its close; false when it already is (a second
    /// click on the close button starts no second question).
    pub(crate) fn start_closing(&self) -> bool {
        !self.closing.swap(true, Ordering::SeqCst)
    }

    pub(crate) fn stop_closing(&self) {
        self.closing.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn window_title(s: &Session) -> String {
    let edited = if s.is_dirty() { " (edited)" } else { "" };
    format!("{}{edited} - {APP_NAME}", s.display_name())
}

/// The view after a command, with the window title kept in step. The Window menu is
/// refreshed on the main thread once it is free, never here under the session's lock.
pub(crate) fn finish<R: Runtime>(
    window: &WebviewWindow<R>,
    s: &Session,
    notice: Option<String>,
) -> CmdResult {
    let state = crate::docs::doc_of(window);
    if state.retitle(window, s) {
        crate::windows::refresh_menu(window.app_handle());
    }
    let mut v = view::view(s, notice)?;
    v.autosave = window.try_state::<Safety>().is_none_or(|x| x.autosave_on());
    Ok(v)
}

/// The window, as the shared commands see it (method_schedule_session::commands): each
/// desktop command passes `&Win(&window)`.
pub(crate) struct Win<'a, R: Runtime>(pub &'a WebviewWindow<R>);

impl<R: Runtime> method_schedule_session::commands::Shell for Win<'_, R> {
    fn finish(&self, s: &Session, notice: Option<String>) -> CmdResult {
        finish(self.0, s, notice)
    }
}

/// The window's schedule, as the shared commands see it.
impl method_schedule_session::commands::DocLock for AppState {
    fn lock(&self) -> MutexGuard<'_, Session> {
        AppState::lock(self)
    }
}

// ---------------------------------------------------------------------------
// Views

#[tauri::command]
pub fn get_view<R: Runtime>(window: WebviewWindow<R>, state: Doc) -> CmdResult {
    method_schedule_session::commands::app::get_view(&crate::app::Win(&window), &*state)
}

/// The day out of days grid for a sub-board, for `category` (Cast when it is gone or not
/// given), or for header field `header` (`reports::HeaderField`: set, location, unit,
/// episode, I/E, time of day, script day, sequence).
#[tauri::command(rename_all = "snake_case")]
pub fn dood_grid(
    state: Doc,
    sub_board: u32,
    category: Option<u32>,
    header: Option<String>,
    sort: Option<Vec<String>>,
    no_holds: Option<bool>,
) -> CmdResult<DoodGridDto> {
    method_schedule_session::commands::app::dood_grid(
        &*state, sub_board, category, header, sort, no_holds,
    )
}

/// Why an element's day out of days counts are what they are, on the grid's sub-board
/// (src/explain.rs).
#[tauri::command(rename_all = "snake_case")]
pub fn explain_count(state: Doc, sub_board: u32, element: u32) -> CmdResult<ExplainDto> {
    method_schedule_session::commands::app::explain_count(&*state, sub_board, element)
}

/// The Conflicts tab for `board` (src/conflicts.rs).
#[tauri::command]
pub fn conflicts_view(state: Doc, board: u32) -> CmdResult<crate::conflicts::ConflictsDto> {
    method_schedule_session::commands::app::conflicts_view(&*state, board)
}

// ---------------------------------------------------------------------------
// Files

/// Starts a new, empty schedule in place of the open one, after asking about unsaved
/// changes. `None` when the user cancelled.
#[tauri::command]
pub async fn new_schedule<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<View>> {
    if !settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    safety_app::replace(&state, &window.state::<Safety>(), Session::new());
    finish(&window, &state.lock(), None).map(Some)
}

/// Shows an Open dialog and opens the chosen file in place of the open schedule, after
/// asking about unsaved changes. A file open in another window brings that window to the
/// front first, without the question. `None` when cancelled.
#[tauri::command]
pub async fn open_schedule<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<View>> {
    let start = state
        .lock()
        .path()
        .and_then(Path::parent)
        .map(PathBuf::from);
    let mut dialog = window
        .dialog()
        .file()
        .set_title("Open schedule")
        .add_filter(
            "Schedule file",
            &[session::EXTENSION, "json", crate::templates::EXTENSION],
        )
        .set_parent(&window);
    if let Some(dir) = start {
        dialog = dialog.set_directory(dir);
    }
    let Some(picked) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    // Open in another window already: that window comes to the front (decisions.md,
    // entry 86).
    if crate::windows::focus_holder(window.app_handle(), &path, window.label()) {
        return Ok(None);
    }
    if !settle_unsaved(&window, &state)? {
        return Ok(None);
    }
    safety_app::open_path(&state, &window.state::<Safety>(), &path)?;
    safety_app::refresh_menu(window.app_handle());
    finish(&window, &state.lock(), None).map(Some)
}

/// Saves to the schedule's file, or asks where (always, with `save_as`). `None` when the
/// user cancelled. When the file changed on disk since it was opened here, nothing is
/// written and the error says so.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_schedule<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    save_as: bool,
) -> CmdResult<Option<View>> {
    if !save(&window, &state, save_as)? {
        return Ok(None);
    }
    finish(&window, &state.lock(), None).map(Some)
}

/// Shows an Open dialog for a screenplay (Fountain or Final Draft) and adds its scenes to
/// the schedule's Boneyard as one undo step. `None` when cancelled.
#[tauri::command]
pub async fn import_script<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<View>> {
    let Some(picked) = window
        .dialog()
        .file()
        .set_title("Import script")
        .add_filter("Screenplay", &["fountain", "fdx", "mscenes"])
        .set_parent(&window)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let lines = window
        .app_handle()
        .state::<crate::safety_app::Safety>()
        .prefs()
        .fountain_lines;
    let script = session::read_script_with(&path, lines)?;
    let mut s = state.lock();
    let notice = method_schedule_session::commands::files::import_script(&mut s, &script)?;
    finish(&window, &s, Some(notice)).map(Some)
}

/// Saves the open schedule: to its file, or where a Save dialog says. False when the user
/// cancelled. A schedule recovered after a crash asks before its first Save replaces its
/// original file. Backups, the recovery copy and Open Recent follow (src/safety_app.rs).
/// Shows dialogs, so never on the main thread.
fn save<R: Runtime>(window: &WebviewWindow<R>, state: &AppState, save_as: bool) -> CmdResult<bool> {
    let (current, title, confirm) = {
        let s = state.lock();
        (
            s.path().map(PathBuf::from),
            s.schedule().title.clone(),
            s.needs_overwrite_confirm(),
        )
    };
    if confirm && !save_as {
        let name = state.lock().display_name();
        let replace = window
            .dialog()
            .message(recovered_question(&name))
            .title("Recovered schedule")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Replace".into(),
                "Cancel".into(),
            ))
            .parent(window)
            .blocking_show();
        if !replace {
            return Ok(false);
        }
    }
    let path = match current {
        Some(p) if !save_as => p,
        _ => {
            let suggested = match current.as_deref().and_then(Path::file_stem) {
                Some(stem) => stem.to_string_lossy().into_owned(),
                None => safe_file_stem(&title),
            };
            let mut dialog = window
                .dialog()
                .file()
                .set_title(if save_as {
                    "Save schedule as"
                } else {
                    "Save schedule"
                })
                .add_filter("Schedule file", &[session::EXTENSION])
                .set_file_name(format!("{suggested}.{}", session::EXTENSION))
                .set_parent(window);
            if let Some(dir) = current.as_deref().and_then(Path::parent) {
                dialog = dialog.set_directory(dir);
            }
            let Some(picked) = dialog.blocking_save_file() else {
                return Ok(false);
            };
            with_extension(
                picked.into_path().map_err(|e| e.to_string())?,
                session::EXTENSION,
            )
        }
    };
    crate::windows::refuse_if_open_elsewhere(window.app_handle(), &path, window.label())?;
    safety_app::save(state, &window.state::<Safety>(), &path)?;
    safety_app::refresh_menu(window.app_handle());
    Ok(true)
}

pub(crate) fn recovered_question(name: &str) -> String {
    format!(
        "This schedule was recovered after the app closed unexpectedly. Saving replaces \
         {name} with the recovered version. Use Save As to keep both."
    )
}

/// Asks what to do with unsaved changes before the schedule is replaced or the window
/// closes: Save, Don't Save or Cancel. With Autosave on, changes go into the file first,
/// so a schedule with a file rarely asks. True when it is fine to go on (saved, thrown
/// away on purpose, or nothing to lose). Blocks, so never on the main thread.
pub(crate) fn settle_unsaved<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
) -> CmdResult<bool> {
    if state.lock().is_dirty() {
        safety_app::autosave_now(window.app_handle(), true);
    }
    let name = {
        let s = state.lock();
        if !s.is_dirty() {
            return Ok(true);
        }
        s.display_name()
    };
    let (save_label, discard_label, cancel_label) = unsaved_buttons();
    let choice = window
        .dialog()
        .message(unsaved_question(&name))
        .title("Unsaved changes")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            save_label.into(),
            discard_label.into(),
            cancel_label.into(),
        ))
        .parent(window)
        .blocking_show_with_result();
    match choice {
        MessageDialogResult::Custom(c) if c == save_label => save(window, state, false),
        MessageDialogResult::Custom(c) if c == discard_label => Ok(true),
        _ => Ok(false),
    }
}

pub(crate) fn unsaved_question(name: &str) -> String {
    format!("\"{name}\" has unsaved changes. Save them first? If you don't, they will be lost.")
}

pub(crate) fn unsaved_buttons() -> (&'static str, &'static str, &'static str) {
    ("Save", "Don't Save", "Cancel")
}

/// Closing `windows` (one window, or all of them to quit): each with unsaved changes asks
/// about them in turn, on a thread of its own (dialogs block), and `then` runs once every
/// one has settled. Cancel, or a save that fails, on any stops it there. Don't Save takes
/// effect only once all have settled, so after a cancelled Quit every window keeps its
/// changes, its autosave and its recovery copy (decisions.md, entry 88). A window whose
/// question is already up isn't asked twice: the close stops and that window comes to
/// the front.
pub(crate) fn confirm_close<R: Runtime>(
    windows: Vec<WebviewWindow<R>>,
    then: impl FnOnce() + Send + 'static,
) {
    let mut asking: Vec<(WebviewWindow<R>, Arc<AppState>)> = Vec::new();
    for w in windows {
        let state = crate::docs::doc_of(&w);
        if !state.lock().is_dirty() {
            continue;
        }
        if !state.start_closing() {
            // Its question is up already (a second click on the close button).
            for (_, s) in &asking {
                s.stop_closing();
            }
            let _ = w.set_focus();
            return;
        }
        asking.push((w, state));
    }
    if asking.is_empty() {
        then();
        return;
    }
    std::thread::spawn(move || {
        let mut discard = Vec::new();
        let mut settled = true;
        for (window, state) in &asking {
            match settle_window(window, state) {
                Some(true) => discard.push((window, state)),
                Some(false) => {}
                None => {
                    settled = false;
                    break;
                }
            }
        }
        if settled {
            // Don't Save: nothing of these changes is kept.
            for (window, state) in discard {
                safety_app::discard_all(state, &window.state::<Safety>());
            }
        }
        for (_, state) in &asking {
            state.stop_closing();
        }
        if settled {
            then()
        }
    });
}

/// Asks one window about its unsaved changes. `Some(discard)` when it may close (`true`
/// when the user chose Don't Save), `None` when the close stops. Blocks.
fn settle_window<R: Runtime>(window: &WebviewWindow<R>, state: &AppState) -> Option<bool> {
    match settle_unsaved(window, state) {
        Ok(true) => Some(state.lock().is_dirty()),
        Ok(false) => None,
        Err(e) => {
            window
                .dialog()
                .message(format!("{e}\n\nNothing was closed."))
                .title("Couldn't save")
                .kind(MessageDialogKind::Error)
                .parent(window)
                .blocking_show();
            None
        }
    }
}

pub(crate) use method_schedule_session::export::safe_file_stem;

// ---------------------------------------------------------------------------
// History

#[tauri::command]
pub fn undo<R: Runtime>(window: WebviewWindow<R>, state: Doc) -> CmdResult {
    method_schedule_session::commands::app::undo(&crate::app::Win(&window), &*state)
}

#[tauri::command]
pub fn redo<R: Runtime>(window: WebviewWindow<R>, state: Doc) -> CmdResult {
    method_schedule_session::commands::app::redo(&crate::app::Win(&window), &*state)
}

// ---------------------------------------------------------------------------
// The board (src/board.rs)

/// Moves strips of `board` (a drag, or Option-Up and Down) as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn move_strips<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    strips: Vec<u32>,
    sub_board: Option<u32>,
    index: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_strips(
        &crate::app::Win(&window),
        &*state,
        board,
        strips,
        sub_board,
        index,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn insert_day_break<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    index: usize,
) -> CmdResult {
    method_schedule_session::commands::app::insert_day_break(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        index,
    )
}

/// Deletes day breaks and banners (the page asks first about banners).
#[tauri::command]
pub fn delete_strips<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    strips: Vec<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::delete_strips(
        &crate::app::Win(&window),
        &*state,
        board,
        strips,
    )
}

/// Inserts a banner with its text and duration (`h:mm`).
#[tauri::command(rename_all = "snake_case")]
pub fn insert_banner<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    index: usize,
    text: String,
    duration: String,
    style: Option<board::BannerStyleInput>,
) -> CmdResult {
    method_schedule_session::commands::app::insert_banner(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        index,
        text,
        duration,
        style,
    )
}

/// Changes a banner's text and duration.
#[tauri::command]
pub fn set_banner<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    strip: u32,
    text: String,
    duration: String,
    style: Option<board::BannerStyleInput>,
) -> CmdResult {
    method_schedule_session::commands::app::set_banner(
        &crate::app::Win(&window),
        &*state,
        board,
        strip,
        text,
        duration,
        style,
    )
}

/// Sorts a sub-board (or the Boneyard, `sub_board` null) by up to four header fields.
#[tauri::command(rename_all = "snake_case")]
pub fn sort_strips<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    sub_board: Option<u32>,
    keys: Vec<board::SortKeyInput>,
) -> CmdResult {
    method_schedule_session::commands::app::sort_strips(
        &crate::app::Win(&window),
        &*state,
        board,
        sub_board,
        keys,
    )
}

/// Sets `board`'s strip layout (Board > Strip Layout) as one undo step.
#[tauri::command]
pub fn set_strip_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    columns: Vec<board::LayoutColumnInput>,
    size: Option<String>,
    marks: Option<board::MarksInput>,
) -> CmdResult {
    method_schedule_session::commands::app::set_strip_layout(
        &crate::app::Win(&window),
        &*state,
        board,
        columns,
        size,
        marks,
    )
}

/// Continue Scene on Next Day: a part of the scene below the next day break, as one undo
/// step; the view names the new part, so the board selects it.
#[tauri::command]
pub fn continue_scene<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    strip: u32,
) -> CmdResult {
    method_schedule_session::commands::app::continue_scene(
        &crate::app::Win(&window),
        &*state,
        board,
        strip,
    )
}

/// Sets what `board`'s day breaks show (Board > Day Breaks) as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn set_day_look<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    look: board::DayLookInput,
) -> CmdResult {
    method_schedule_session::commands::app::set_day_look(
        &crate::app::Win(&window),
        &*state,
        board,
        look,
    )
}

/// Sets the schedule's strip colors (Board > Strip Colors) as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn set_strip_colors<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    rules: Vec<board::ColorRuleInput>,
    scene_text: Option<String>,
    banner_text: Option<String>,
    break_text: Option<String>,
) -> CmdResult {
    method_schedule_session::commands::app::set_strip_colors(
        &crate::app::Win(&window),
        &*state,
        rules,
        scene_text,
        banner_text,
        break_text,
    )
}

// ---------------------------------------------------------------------------
// Boards and sub-boards (src/board.rs)

/// Shows another board. Not an edit: nothing to undo, nothing unsaved.
#[tauri::command]
pub fn show_board<R: Runtime>(window: WebviewWindow<R>, state: Doc, board: u32) -> CmdResult {
    method_schedule_session::commands::app::show_board(&crate::app::Win(&window), &*state, board)
}

#[tauri::command]
pub fn add_board<R: Runtime>(window: WebviewWindow<R>, state: Doc, name: String) -> CmdResult {
    method_schedule_session::commands::app::add_board(&crate::app::Win(&window), &*state, name)
}

#[tauri::command]
pub fn duplicate_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::duplicate_board(
        &crate::app::Win(&window),
        &*state,
        board,
        name,
    )
}

#[tauri::command]
pub fn rename_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::rename_board(
        &crate::app::Win(&window),
        &*state,
        board,
        name,
    )
}

/// Moves a board to place `to` among the boards.
#[tauri::command]
pub fn move_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    to: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_board(
        &crate::app::Win(&window),
        &*state,
        board,
        to,
    )
}

/// Deletes a board (the page has asked first); the first board shows when it was shown.
#[tauri::command]
pub fn delete_board<R: Runtime>(window: WebviewWindow<R>, state: Doc, board: u32) -> CmdResult {
    method_schedule_session::commands::app::delete_board(&crate::app::Win(&window), &*state, board)
}

/// Adds a sub-board to `board`; the view names it, so the page shows it.
#[tauri::command]
pub fn add_sub_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::add_sub_board(
        &crate::app::Win(&window),
        &*state,
        board,
        name,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn rename_sub_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::rename_sub_board(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        name,
    )
}

/// Moves a sub-board to place `to` on its board.
#[tauri::command(rename_all = "snake_case")]
pub fn move_sub_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    to: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_sub_board(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        to,
    )
}

/// Deletes a sub-board (the page has asked first): its scenes go to the Boneyard.
#[tauri::command(rename_all = "snake_case")]
pub fn delete_sub_board<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
) -> CmdResult {
    method_schedule_session::commands::app::delete_sub_board(
        &crate::app::Win(&window),
        &*state,
        sub_board,
    )
}

/// Automatic day breaks on a sub-board, `by` `pages`, `time` or `screen_time`, every
/// `limit` (as typed). The status line says how many shoot days that made.
#[tauri::command(rename_all = "snake_case")]
pub fn auto_day_breaks<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    by: String,
    limit: String,
) -> CmdResult {
    method_schedule_session::commands::app::auto_day_breaks(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        by,
        limit,
    )
}

/// Remove All Day Breaks on a sub-board, as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn remove_day_breaks<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
) -> CmdResult {
    method_schedule_session::commands::app::remove_day_breaks(
        &crate::app::Win(&window),
        &*state,
        sub_board,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_start_date<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    date: Option<String>,
) -> CmdResult {
    method_schedule_session::commands::app::set_start_date(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        date,
    )
}

// ---------------------------------------------------------------------------
// Breakdown sheets (src/sheet.rs)

/// The Breakdown tab's sheet: `sheet` while it exists, else the one now at `index` in
/// `board`'s order. `None` when the schedule has no sheets.
#[tauri::command]
pub fn get_sheet(
    state: Doc,
    board: u32,
    sheet: Option<u32>,
    index: Option<usize>,
    order: Option<String>,
) -> CmdResult<Option<SheetDto>> {
    method_schedule_session::commands::app::get_sheet(&*state, board, sheet, index, order)
}

/// Sets one header field (src/sheet.rs reads times and text strictly).
#[tauri::command]
pub fn set_sheet_field<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    field: String,
    value: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_sheet_field(
        &crate::app::Win(&window),
        &*state,
        sheet,
        field,
        value,
    )
}

/// Sets the page length from whole pages and eighths.
#[tauri::command]
pub fn set_sheet_pages<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    pages: String,
    eighths: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_sheet_pages(
        &crate::app::Win(&window),
        &*state,
        sheet,
        pages,
        eighths,
    )
}

/// Puts an existing element on a sheet.
#[tauri::command]
pub fn tag_element<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    element: u32,
) -> CmdResult {
    method_schedule_session::commands::app::tag_element(
        &crate::app::Win(&window),
        &*state,
        sheet,
        element,
    )
}

/// Sets how many of an element a sheet needs and whether it counts daily.
#[tauri::command]
pub fn set_quantity<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    element: u32,
    count: String,
    daily: bool,
) -> CmdResult {
    method_schedule_session::commands::app::set_quantity(
        &crate::app::Win(&window),
        &*state,
        sheet,
        element,
        count,
        daily,
    )
}

/// Puts the element `name` of `category` on a sheet, adding it (and the category) when
/// new.
#[tauri::command]
pub fn create_element<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    category: String,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::create_element(
        &crate::app::Win(&window),
        &*state,
        sheet,
        category,
        name,
    )
}

/// Takes an element off a sheet.
#[tauri::command]
pub fn untag_element<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    element: u32,
) -> CmdResult {
    method_schedule_session::commands::app::untag_element(
        &crate::app::Win(&window),
        &*state,
        sheet,
        element,
    )
}

/// Adds an empty sheet after sheet `after` on its sub-board of `board`, or at the end of
/// the Boneyard. The view names the new sheet.
#[tauri::command]
pub fn new_sheet<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    after: Option<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::new_sheet(
        &crate::app::Win(&window),
        &*state,
        board,
        after,
    )
}

/// Deletes a sheet (the page has asked first). The view names the sheet to show next in
/// `board`'s order as the tab steps (`order`, as for `get_sheet`).
#[tauri::command]
pub fn delete_sheet<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    sheet: u32,
    order: Option<String>,
) -> CmdResult {
    method_schedule_session::commands::app::delete_sheet(
        &crate::app::Win(&window),
        &*state,
        board,
        sheet,
        order,
    )
}

/// What Board > Merge Strips would do with `sheets` on `board`: the engine's report on a
/// copy, nothing changed.
#[tauri::command]
pub fn merge_sheets_preview(
    state: Doc,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult<sheet::SheetMergeDto> {
    method_schedule_session::commands::app::merge_sheets_preview(&*state, board, sheets)
}

/// Merges `sheets`, in the order they were selected, into the first of them as one undo
/// step. The view selects
/// the surviving strip and names its sheet.
#[tauri::command]
pub fn merge_sheets<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::merge_sheets(
        &crate::app::Win(&window),
        &*state,
        board,
        sheets,
    )
}

/// Duplicates `sheets` as one undo step (Board > Duplicate Strip, Breakdown > Duplicate
/// Sheet). The view selects the copies' strips on `board` and names the last copy.
#[tauri::command]
pub fn duplicate_sheets<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::duplicate_sheets(
        &crate::app::Win(&window),
        &*state,
        board,
        sheets,
    )
}

// ---------------------------------------------------------------------------
// Cut, copy and paste (src/clipboard.rs)

/// Copies (or, with `cut`, cuts) strips of `board` to the app's clipboard. Not an edit:
/// cut strips stay where they are until the paste.
#[tauri::command]
pub fn copy_strips<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    strips: Vec<u32>,
    cut: bool,
) -> CmdResult {
    let mut s = state.lock();
    let clip = clipboard::copy_strips(s.schedule(), board, &strips, cut)?;
    let n = clipboard::count_text(strips.len(), "strip", "strips");
    let notice = if cut {
        format!("Cut {n}. Paste moves them below the selection on this board.")
    } else {
        format!("Copied {n}. Paste puts copies below the selection.")
    };
    let carried = clipboard::carry(s.schedule(), &clip, &s.display_name());
    s.set_clipboard(Some(clip));
    let v = finish(&window, &s, Some(notice));
    drop(s);
    crate::docs::share_clip(&window, carried);
    v
}

/// Pastes the clipboard's strips on `board` at `index` of `sub_board` (null: the
/// Boneyard), as one undo step. The view selects what was pasted.
#[tauri::command(rename_all = "snake_case")]
pub fn paste_strips<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    board: u32,
    sub_board: Option<u32>,
    index: usize,
) -> CmdResult {
    method_schedule_session::commands::app::paste_strips(
        &crate::app::Win(&window),
        &*state,
        board,
        sub_board,
        index,
    )
}

/// Copies (or, with `cut`, cuts) elements of sheet `sheet` to the app's clipboard. Not an
/// edit: cut elements stay on the sheet until the paste.
#[tauri::command]
pub fn copy_elements<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    elements: Vec<u32>,
    cut: bool,
) -> CmdResult {
    let mut s = state.lock();
    let clip = clipboard::copy_elements(s.schedule(), sheet, &elements, cut)?;
    let n = clipboard::count_text(elements.len(), "element", "elements");
    let notice = if cut {
        format!("Cut {n}. Paste on another sheet moves them there.")
    } else {
        format!("Copied {n}. Paste puts them on the sheet shown.")
    };
    s.set_clipboard(Some(clip));
    let v = finish(&window, &s, Some(notice));
    drop(s);
    crate::docs::share_clip(&window, None);
    v
}

/// Pastes the clipboard's elements on sheet `sheet` as one undo step.
#[tauri::command]
pub fn paste_elements<R: Runtime>(window: WebviewWindow<R>, state: Doc, sheet: u32) -> CmdResult {
    method_schedule_session::commands::app::paste_elements(
        &crate::app::Win(&window),
        &*state,
        sheet,
    )
}

// ---------------------------------------------------------------------------
// The Elements tab (src/elements.rs)

/// The Elements tab: every category, and `category`'s elements (the first category's
/// when it is gone or not given), or header field `header`'s values, with work days and
/// dates from `sub_board`.
#[tauri::command(rename_all = "snake_case")]
pub fn elements_view(
    state: Doc,
    sub_board: u32,
    category: Option<u32>,
    header: Option<String>,
) -> CmdResult<ElementsDto> {
    method_schedule_session::commands::app::elements_view(&*state, sub_board, category, header)
}

/// Adds a category. The view names it, so the tab shows it.
#[tauri::command]
pub fn add_category<R: Runtime>(window: WebviewWindow<R>, state: Doc, name: String) -> CmdResult {
    method_schedule_session::commands::app::add_category(&crate::app::Win(&window), &*state, name)
}

/// Sets a category's name, automatic board IDs or default hold and drop rules.
#[tauri::command]
pub fn set_category_field<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    field: String,
    value: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_category_field(
        &crate::app::Win(&window),
        &*state,
        category,
        field,
        value,
    )
}

/// Moves a user category to place `to` among the user categories.
#[tauri::command]
pub fn move_category<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    to: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_category(
        &crate::app::Win(&window),
        &*state,
        category,
        to,
    )
}

/// Deletes an empty user category.
#[tauri::command]
pub fn delete_category<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
) -> CmdResult {
    method_schedule_session::commands::app::delete_category(
        &crate::app::Win(&window),
        &*state,
        category,
    )
}

/// Gives every element of a category its default hold and drop rules (the page asks
/// first), as one undo step.
#[tauri::command]
pub fn apply_category_defaults<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
) -> CmdResult {
    method_schedule_session::commands::app::apply_category_defaults(
        &crate::app::Win(&window),
        &*state,
        category,
    )
}

/// Sets an element's name, board ID, lock, or hold and drop rules.
#[tauri::command]
pub fn set_element_field<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    field: String,
    value: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_element_field(
        &crate::app::Win(&window),
        &*state,
        element,
        field,
        value,
    )
}

/// One element's days and dates on `sub_board`: every date of its day out of days row.
#[tauri::command(rename_all = "snake_case")]
pub fn element_days(state: Doc, sub_board: u32, element: u32) -> CmdResult<ElementDaysDto> {
    method_schedule_session::commands::app::element_days(&*state, sub_board, element)
}

/// Adds a custom property to a category.
#[tauri::command]
pub fn add_property<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::add_property(
        &crate::app::Win(&window),
        &*state,
        category,
        name,
    )
}

/// Renames property `at` of a category.
#[tauri::command]
pub fn rename_property<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    at: usize,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::rename_property(
        &crate::app::Win(&window),
        &*state,
        category,
        at,
        name,
    )
}

/// Moves property `at` of a category to place `to`.
#[tauri::command]
pub fn move_property<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    at: usize,
    to: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_property(
        &crate::app::Win(&window),
        &*state,
        category,
        at,
        to,
    )
}

/// Deletes property `at` of a category and its values (the page asks first).
#[tauri::command]
pub fn delete_property<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    at: usize,
) -> CmdResult {
    method_schedule_session::commands::app::delete_property(
        &crate::app::Win(&window),
        &*state,
        category,
        at,
    )
}

/// Whether property `at` of a category takes several lines (decision 119).
#[tauri::command(rename_all = "snake_case")]
pub fn set_property_multi_line<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    at: usize,
    multi_line: bool,
) -> CmdResult {
    method_schedule_session::commands::app::set_property_multi_line(
        &crate::app::Win(&window),
        &*state,
        category,
        at,
        multi_line,
    )
}

/// Sets an element's value of one of its category's properties.
#[tauri::command]
pub fn set_property_value<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    name: String,
    value: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_property_value(
        &crate::app::Win(&window),
        &*state,
        element,
        name,
        value,
    )
}

/// Sets the elements `element` links to; with `retroactive`, sheets that already hold it
/// get them too.
#[tauri::command]
pub fn set_links<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    links: Vec<u32>,
    retroactive: bool,
) -> CmdResult {
    method_schedule_session::commands::app::set_links(
        &crate::app::Win(&window),
        &*state,
        element,
        links,
        retroactive,
    )
}

/// Shows an Open dialog for an image and adds its path to an element (decision 62: the
/// path only, never the file). `None` when cancelled.
#[tauri::command]
pub async fn add_element_image<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
) -> CmdResult<Option<View>> {
    let doc = state.lock().path().map(PathBuf::from);
    let Some(path) = elements::pick_image(&window, "Add image", doc.as_deref())? else {
        return Ok(None);
    };
    let stored = elements::stored_image_path(doc.as_deref(), &path);
    let mut s = state.lock();
    s.edit(|x| elements::add_image(x, element, stored))?;
    finish(&window, &s, None).map(Some)
}

/// Takes image `at` off an element; the file stays where it is.
#[tauri::command]
pub fn remove_element_image<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    at: usize,
) -> CmdResult {
    method_schedule_session::commands::app::remove_element_image(
        &crate::app::Win(&window),
        &*state,
        element,
        at,
    )
}

/// Image `at` of an element as a `data:` URL for its thumbnail, or `None` when the file
/// is missing or unreadable. Async, and the file is read with the lock let go.
#[tauri::command]
pub async fn element_image(state: Doc, element: u32, at: usize) -> CmdResult<Option<String>> {
    let (stored, doc) = {
        let s = state.lock();
        (
            elements::image_of(s.schedule(), element, at)?,
            s.path().map(PathBuf::from),
        )
    };
    Ok(elements::read_image(doc.as_deref(), &stored))
}

/// Merges `others` into `survivor` (the page asks which survives).
#[tauri::command]
pub fn merge_elements<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    survivor: u32,
    others: Vec<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::merge_elements(
        &crate::app::Win(&window),
        &*state,
        survivor,
        others,
    )
}

/// Moves an element to another category.
#[tauri::command]
pub fn move_element<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    category: u32,
) -> CmdResult {
    method_schedule_session::commands::app::move_element(
        &crate::app::Win(&window),
        &*state,
        element,
        category,
    )
}

/// Renumbers a category's unlocked elements from `start`, in the tab's order: `order`
/// when its table is sorted by a column.
#[tauri::command]
pub fn renumber_elements<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    category: u32,
    start: String,
    order: Option<Vec<u32>>,
) -> CmdResult {
    method_schedule_session::commands::app::renumber_elements(
        &crate::app::Win(&window),
        &*state,
        category,
        start,
        order,
    )
}

/// Deletes an element (the page has asked first; `unlink` after a second question when
/// it is linked).
#[tauri::command]
pub fn delete_element<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    unlink: bool,
) -> CmdResult {
    method_schedule_session::commands::app::delete_element(
        &crate::app::Win(&window),
        &*state,
        element,
        unlink,
    )
}

// ---------------------------------------------------------------------------
// Red flags (src/redflags.rs)

/// The Red flags tab: every clash on `sub_board`, in date order.
#[tauri::command(rename_all = "snake_case")]
pub fn red_flags_view(state: Doc, sub_board: u32) -> CmdResult<redflags::RedFlagsDto> {
    method_schedule_session::commands::app::red_flags_view(&*state, sub_board)
}

/// Adds a red flag to `element` (`flag.id` null) or changes one of its flags.
#[tauri::command(rename_all = "snake_case")]
pub fn set_red_flag<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    element: u32,
    flag: redflags::RedFlagInput,
) -> CmdResult {
    method_schedule_session::commands::app::set_red_flag(
        &crate::app::Win(&window),
        &*state,
        element,
        flag,
    )
}

/// Deletes a red flag (the page has asked first).
#[tauri::command(rename_all = "snake_case")]
pub fn delete_red_flag<R: Runtime>(window: WebviewWindow<R>, state: Doc, flag: u32) -> CmdResult {
    method_schedule_session::commands::app::delete_red_flag(
        &crate::app::Win(&window),
        &*state,
        flag,
    )
}

/// Adds a red flag type at the end of the list.
#[tauri::command(rename_all = "snake_case")]
pub fn add_red_flag_type<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::add_red_flag_type(
        &crate::app::Win(&window),
        &*state,
        name,
    )
}

/// Renames a red flag type.
#[tauri::command(rename_all = "snake_case")]
pub fn rename_red_flag_type<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    red_flag_type: u32,
    name: String,
) -> CmdResult {
    method_schedule_session::commands::app::rename_red_flag_type(
        &crate::app::Win(&window),
        &*state,
        red_flag_type,
        name,
    )
}

/// Moves a red flag type to place `to` in the list.
#[tauri::command(rename_all = "snake_case")]
pub fn move_red_flag_type<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    red_flag_type: u32,
    to: usize,
) -> CmdResult {
    method_schedule_session::commands::app::move_red_flag_type(
        &crate::app::Win(&window),
        &*state,
        red_flag_type,
        to,
    )
}

/// Deletes a red flag type no flag uses.
#[tauri::command(rename_all = "snake_case")]
pub fn delete_red_flag_type<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    red_flag_type: u32,
) -> CmdResult {
    method_schedule_session::commands::app::delete_red_flag_type(
        &crate::app::Win(&window),
        &*state,
        red_flag_type,
    )
}

/// Red Flag Types > Add the Standard Types: the built-in types an older schedule lacks.
#[tauri::command(rename_all = "snake_case")]
pub fn add_standard_red_flag_types<R: Runtime>(window: WebviewWindow<R>, state: Doc) -> CmdResult {
    method_schedule_session::commands::app::add_standard_red_flag_types(
        &crate::app::Win(&window),
        &*state,
    )
}

/// The Red flags tab's Add: flags for several elements and dates, one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn add_red_flags<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    flags: redflags::AddFlagsInput,
) -> CmdResult {
    method_schedule_session::commands::app::add_red_flags(&crate::app::Win(&window), &*state, flags)
}

/// The Red flags tab's Delete for the checked rows (the page has asked first).
#[tauri::command(rename_all = "snake_case")]
pub fn delete_red_flags<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    flags: Vec<u32>,
) -> CmdResult {
    method_schedule_session::commands::app::delete_red_flags(
        &crate::app::Win(&window),
        &*state,
        flags,
    )
}

// ---------------------------------------------------------------------------
// The Calendar tab (src/calendar.rs)

/// One month of a sub-board's calendar (`year` and `month` both, or neither for the start
/// date's month).
#[tauri::command(rename_all = "snake_case")]
pub fn calendar_view(
    state: Doc,
    sub_board: u32,
    year: Option<i32>,
    month: Option<u32>,
    week_start: Option<String>,
    fields: Option<Vec<String>>,
    span: Option<bool>,
) -> CmdResult<CalendarDto> {
    method_schedule_session::commands::app::calendar_view(
        &*state, sub_board, year, month, week_start, fields, span,
    )
}

/// Makes a date of a sub-board's calendar working, a day off, a holiday or a company
/// travel day.
#[tauri::command(rename_all = "snake_case")]
pub fn set_day_kind<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    date: String,
    kind: String,
) -> CmdResult {
    method_schedule_session::commands::app::set_day_kind(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        date,
        kind,
    )
}

/// Sets a sub-board's weekly days off (`mon` to `sun`).
#[tauri::command(rename_all = "snake_case")]
pub fn set_weekly_off<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    days: Vec<String>,
) -> CmdResult {
    method_schedule_session::commands::app::set_weekly_off(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        days,
    )
}

/// Adds an event to a sub-board's calendar (`index` null) or replaces event `index`.
#[tauri::command(rename_all = "snake_case")]
pub fn save_event<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    index: Option<usize>,
    event: EventInput,
) -> CmdResult {
    method_schedule_session::commands::app::save_event(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        index,
        event,
    )
}

/// The Calendar's View menu: what it hides, by key (decision 111), as one undo step.
#[tauri::command(rename_all = "snake_case")]
pub fn set_calendar_hidden<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    hidden: Vec<String>,
) -> CmdResult {
    method_schedule_session::commands::app::set_calendar_hidden(
        &crate::app::Win(&window),
        &*state,
        hidden,
    )
}

/// Deletes event `index` of a sub-board's calendar (the page has asked first).
#[tauri::command(rename_all = "snake_case")]
pub fn delete_event<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    index: usize,
) -> CmdResult {
    method_schedule_session::commands::app::delete_event(
        &crate::app::Win(&window),
        &*state,
        sub_board,
        index,
    )
}

// ---------------------------------------------------------------------------
// Running

/// Only the bundled UI may load in the web view: no navigating to other sites or files (a
/// dropped link, a stray anchor, injected content).
fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

pub fn run() {
    // The session's clock (method-schedule-session's clock.rs) before anything reads the
    // time: the system's. SystemClock exists only with the `std-clock` feature, so a
    // desktop build without it doesn't compile, rather than running on a stopped clock.
    let installed = method_schedule_session::clock::install(Box::new(
        method_schedule_session::clock::SystemClock,
    ));
    assert!(
        installed,
        "the session clock was read before the app installed it"
    );
    let context = tauri::generate_context!();

    let nav_guard = tauri::plugin::Builder::<tauri::Wry>::new("nav-guard")
        .on_navigation(|_, url| is_app_url(url))
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // In-app updates, driven from src/updater.rs only (no window gets its commands).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(nav_guard)
        .manage(crate::docs::Docs::default())
        // The menu bar (src/menu.rs). Quit asks about unsaved changes first, like closing
        // the window; Clear Menu and the Autosave check are answered in
        // src/safety_app.rs; every other item goes to the page.
        .menu(crate::menu::build)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == crate::menu::QUIT {
                let handle = app.clone();
                let all = app.webview_windows().into_values().collect();
                confirm_close(all, move || handle.exit(0));
            } else if id == crate::menu::CLOSE {
                crate::windows::close_focused(app);
            } else if crate::windows::handle_menu(app, id) {
                // The Window menu: a window brought to the front.
            } else if crate::updater::handle_menu(app, id) {
                // Help > Check for Updates… and its automatic switch.
            } else if !safety_app::handle_menu(app, id) {
                crate::menu::handle(app, id);
            }
        })
        .on_window_event(|window, event| {
            // Leaving the app autosaves now rather than in a few seconds.
            if let tauri::WindowEvent::Focused(false) = event {
                let app = window.app_handle().clone();
                std::thread::spawn(move || safety_app::autosave_now(&app, true));
            }
            // Coming back: the page checks for a change made outside the app meanwhile
            // (decisions.md, entry 179).
            if let tauri::WindowEvent::Focused(true) = event {
                use tauri::Emitter as _;
                let _ = window.emit_to(window.label(), crate::outside::OUTSIDE_CHECK, ());
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let Some(w) = window.app_handle().get_webview_window(window.label()) else {
                    return;
                };
                if crate::docs::doc_of(&w).lock().is_dirty() {
                    api.prevent_close();
                    let ww = w.clone();
                    confirm_close(vec![w], move || {
                        let _ = ww.destroy();
                    });
                }
            }
            // A closed window's schedule goes, leaving no recovery copy when it was clean.
            if let tauri::WindowEvent::Destroyed = event {
                crate::windows::closed(window.app_handle(), window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            dood_grid,
            explain_count,
            crate::find::find_text,
            crate::find::replace_text,
            conflicts_view,
            new_schedule,
            open_schedule,
            save_schedule,
            import_script,
            undo,
            redo,
            move_strips,
            insert_day_break,
            delete_strips,
            insert_banner,
            set_banner,
            sort_strips,
            set_strip_layout,
            set_strip_colors,
            set_day_look,
            continue_scene,
            crate::layouts::add_layout,
            crate::layouts::save_layout,
            crate::layouts::rename_layout,
            crate::layouts::move_layout,
            crate::layouts::delete_layout,
            crate::layouts::use_layout,
            crate::layouts::use_one_line_layout,
            crate::layouts::export_layouts,
            crate::layouts::import_layouts,
            auto_day_breaks,
            remove_day_breaks,
            set_start_date,
            show_board,
            add_board,
            duplicate_board,
            rename_board,
            move_board,
            delete_board,
            add_sub_board,
            rename_sub_board,
            move_sub_board,
            delete_sub_board,
            get_sheet,
            set_sheet_field,
            set_sheet_pages,
            tag_element,
            set_quantity,
            create_element,
            untag_element,
            new_sheet,
            delete_sheet,
            merge_sheets_preview,
            merge_sheets,
            duplicate_sheets,
            copy_strips,
            paste_strips,
            copy_elements,
            paste_elements,
            elements_view,
            add_category,
            set_category_field,
            move_category,
            delete_category,
            apply_category_defaults,
            set_element_field,
            merge_elements,
            move_element,
            renumber_elements,
            delete_element,
            element_days,
            add_property,
            rename_property,
            move_property,
            delete_property,
            set_property_multi_line,
            set_property_value,
            set_links,
            add_element_image,
            remove_element_image,
            element_image,
            red_flags_view,
            set_red_flag,
            delete_red_flag,
            add_red_flag_type,
            rename_red_flag_type,
            move_red_flag_type,
            delete_red_flag_type,
            add_standard_red_flag_types,
            add_red_flags,
            delete_red_flags,
            calendar_view,
            set_calendar_hidden,
            set_day_kind,
            set_weekly_off,
            save_event,
            delete_event,
            crate::reports::report_data,
            crate::reportsetup::report_list,
            crate::reportsetup::save_report,
            crate::reportsetup::duplicate_report,
            crate::reportsetup::move_report,
            crate::reportsetup::delete_report,
            crate::reportsetup::set_production,
            crate::reportsetup::set_calendar_fields,
            crate::reportsetup::save_header_layout,
            crate::reportsetup::delete_header_layout,
            crate::reportsetup::move_header_layout,
            crate::reportsetup::set_dood_codes,
            crate::reportsetup::set_prep_wrap,
            crate::reportsetup::preferences,
            crate::reportsetup::set_preferences,
            crate::reports::print_report,
            crate::export::export_report,
            crate::export::export_budget,
            crate::diffmerge::what_changed,
            crate::diffmerge::merge_pick_file,
            crate::diffmerge::merge_preview,
            crate::diffmerge::merge_apply,
            crate::outside::outside_check,
            crate::outside::outside_resolve,
            crate::outside::backup_list,
            crate::outside::revert_to_backup,
            crate::plan::import_script_tags,
            crate::plan::revise_pick,
            crate::plan::revise_apply,
            crate::plan::timeline_data,
            crate::plan::set_call_time,
            crate::plan::set_shot,
            crate::plan::set_long_day,
            crate::plan::push_past_wrap,
            crate::plan::locations_data,
            crate::plan::set_location,
            crate::plan::rest_data,
            crate::plan::set_turnaround,
            crate::plan::locks_data,
            crate::plan::lock_schedule,
            crate::plan::delete_lock,
            crate::plan::drift_data,
            crate::plan::export_drift,
            crate::plan::pin_strips,
            crate::plan::suggest_order,
            crate::plan::apply_suggested_order,
            crate::plan::export_call_sheet,
            crate::plan::call_sheet_data,
            crate::plan::shoot_day_count,
            safety_app::set_autosave,
            safety_app::open_pending,
            safety_app::open_recent,
            safety_app::recovery_list,
            safety_app::open_recovered,
            safety_app::discard_recovered,
            crate::windows::close_window,
            crate::split::board_view,
            crate::split::add_sheet_image,
            crate::split::remove_sheet_image,
            crate::split::sheet_image,
            crate::breakdown::set_sheet_field_order,
            crate::events::move_event_to,
            crate::flagcolors::set_red_flag_type_color,
            crate::templates::save_template,
            crate::templates::new_from_template,
            crate::help::open_sample,
            crate::help::user_guide,
            crate::help::host_info,
            crate::help::menu_table,
            crate::help::whats_new,
            crate::help::notices,
            crate::help::open_link,
            crate::updater::update_status,
            crate::updater::update_restart,
            crate::updater::update_whats_new,
            crate::windows::new_window,
            crate::windows::open_in_new_window,
        ])
        .setup(|app| {
            // The data folder, Open Recent, the Autosave check, a file from the command
            // line and the autosave timer (src/safety_app.rs).
            safety_app::init(app.handle());
            // Updates: the settings, What's New once after one, the timer (src/updater.rs).
            crate::updater::init(app.handle());
            // `--check-for-updates` (the launcher's switch): Help > Check for Updates.
            if safety_app::launch_from_args(std::env::args_os().skip(1)).check_for_updates {
                let handle = app.handle().clone();
                std::thread::spawn(move || crate::updater::check_now(&handle, true));
            }
            if let Some(w) = app.get_webview_window(MAIN) {
                let state = crate::docs::doc_of(&w);
                let _ = w.set_title(&state.title());
            }
            crate::windows::refresh_menu(app.handle());
            Ok(())
        })
        .build(context)
        .expect("error while building the app")
        .run(|app, event| match event {
            // macOS hands over double-clicked and Open With files here (Windows and Linux
            // pass them on the command line).
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Opened { urls } => safety_app::on_opened(
                app,
                urls.iter().filter_map(|u| u.to_file_path().ok()).collect(),
            ),
            tauri::RunEvent::Exit => safety_app::on_exit(app),
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn only_bundled_ui_urls_are_allowed() {
        let ok = |s: &str| is_app_url(&s.parse().unwrap());
        assert!(ok("tauri://localhost/index.html"));
        assert!(ok("http://tauri.localhost/index.html"));
        assert!(!ok("https://example.com/"));
        assert!(!ok("file:///etc/passwd"));
        assert!(!ok("tauri://evil/"));
        assert!(!ok("http://tauri.localhost.example.com/"));
    }

    #[test]
    fn file_names() {
        assert_eq!(
            safe_file_stem("Harbor Light: Part 2/3"),
            "Harbor Light  Part 2 3"
        );
        assert_eq!(safe_file_stem(" .. "), "Untitled");
        assert_eq!(
            with_extension(PathBuf::from("/x/harbor"), session::EXTENSION),
            PathBuf::from("/x/harbor.mtsc")
        );
        assert_eq!(
            with_extension(PathBuf::from("/x/harbor.MTSC"), session::EXTENSION),
            PathBuf::from("/x/harbor.MTSC")
        );
        assert_eq!(
            with_extension(PathBuf::from("/x/v1.2"), session::EXTENSION),
            PathBuf::from("/x/v1.2.mtsc")
        );
    }

    #[test]
    fn title_and_questions_follow_the_voice() {
        let mut s = Session::new();
        assert_eq!(window_title(&s), "Untitled - Method Schedule");
        s.edit(|x| {
            x.title = "Changed".into();
            Ok(())
        })
        .unwrap();
        assert_eq!(window_title(&s), "Untitled (edited) - Method Schedule");
        let q = unsaved_question("Harbor.mtsc");
        let r = recovered_question("Harbor.mtsc");
        let (a, b, c) = unsaved_buttons();
        for text in [q.as_str(), r.as_str(), a, b, c] {
            assert!(!text.contains('\u{2014}') && !text.contains('!'), "{text}");
        }
    }

    /// The commands registered with `generate_handler!`, the commands build.rs lists (which
    /// turns access control on for them), the commands `capabilities/main.json` grants the
    /// main window, and the commands the page's scripts invoke are all the same set. The
    /// capability grants nothing else but listening for events: no file system, shell or
    /// dialog access from the page.
    #[test]
    fn commands_handlers_capabilities_and_scripts_agree() {
        const BUILD_RS: &str = include_str!("../build.rs");
        const APP_RS: &str = include_str!("app.rs");
        const CAPABILITY: &str = include_str!("../capabilities/main.json");
        let between = |text: &'static str, start: &str, end: &str| -> &'static str {
            let from = text.split(start).nth(1).expect("start marker");
            from.split(end).next().expect("end marker")
        };
        let code_lines = |text: &'static str| {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with("//"))
        };
        let listed: BTreeSet<String> = code_lines(between(BUILD_RS, "const COMMANDS", "];"))
            .filter_map(|l| Some(l.strip_prefix('"')?.strip_suffix("\",")?.to_string()))
            .collect();
        let handled: BTreeSet<String> = code_lines(between(APP_RS, "generate_handler![", "])"))
            .map(|l| {
                l.trim_end_matches(',')
                    .rsplit("::")
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert!(listed.len() >= 10, "build.rs's list wasn't read");
        assert_eq!(listed, handled, "build.rs and generate_handler! differ");

        let cap: serde_json::Value = serde_json::from_str(CAPABILITY).unwrap();
        assert_eq!(cap["identifier"], MAIN);
        // The main window and every schedule window opened after it (src/windows.rs).
        assert_eq!(
            cap["windows"],
            serde_json::json!([MAIN, crate::windows::LABEL_GLOB])
        );
        let mut granted = BTreeSet::new();
        for p in cap["permissions"].as_array().unwrap() {
            let p = p.as_str().expect("permissions are plain names");
            if let Some(cmd) = p.strip_prefix("allow-") {
                assert!(granted.insert(cmd.replace('-', "_")), "{p} twice");
            } else {
                assert_eq!(
                    p, "core:event:allow-listen",
                    "the main window is granted {p}"
                );
            }
        }
        assert_eq!(
            granted, listed,
            "capabilities/main.json and build.rs differ"
        );

        let invoked = invoked_by_scripts();
        assert_eq!(invoked, listed, "the page's scripts and the grant differ");
    }

    /// Each page hears the events sent to its own window and the app-wide ones, so the
    /// page never names a window: it listens on the label Tauri gives it (decisions.md,
    /// entry 88). A fixed label made `doc-*` windows deaf and sent `main`'s events to
    /// every window.
    #[test]
    fn the_page_never_names_a_window() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/js");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "js") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for bad in ["'main'", "\"main\"", "target: '", "target: \""] {
                assert!(
                    !text.contains(bad),
                    "{}: names a window ({bad})",
                    path.display()
                );
            }
            // `doc-1`, `doc-*` (an element id like `doc-file` is fine).
            for part in text.split("doc-").skip(1) {
                assert!(
                    !part.starts_with(|c: char| c.is_ascii_digit() || c == '*'),
                    "{}: names a window (doc-{})",
                    path.display(),
                    &part[..part.len().min(3)]
                );
            }
        }
        let api = include_str!("../ui/js/api.js");
        assert!(api.contains("getCurrentWebviewWindow().label"));
        assert!(api.contains("{ target: label }"));
    }

    /// The DOM's own `replaceChildren` and `append` write a `null` argument as the text
    /// "null" (the page's `el` skips it, they don't), so no argument of theirs may end in
    /// `: null` or `: undefined`, as `cond ? el(...) : null` would.
    #[test]
    fn dom_calls_never_get_a_null_child() {
        /// The text of the call's arguments starting at `from`, to its closing paren.
        fn args(text: &str, from: usize) -> &str {
            let (mut depth, mut quote, mut skip) = (1, None, false);
            for (i, c) in text[from..].char_indices() {
                if skip {
                    skip = false;
                } else if let Some(q) = quote {
                    if c == '\\' {
                        skip = true;
                    } else if c == q {
                        quote = None;
                    }
                } else if matches!(c, '\'' | '"' | '`') {
                    quote = Some(c);
                } else if matches!(c, '(' | '[' | '{') {
                    depth += 1;
                } else if matches!(c, ')' | ']' | '}') {
                    depth -= 1;
                    if depth == 0 {
                        return &text[from..from + i];
                    }
                }
            }
            &text[from..]
        }
        /// The call's top-level arguments.
        fn split(args: &str) -> Vec<&str> {
            let (mut out, mut depth, mut quote, mut skip, mut start) = (vec![], 0, None, false, 0);
            for (i, c) in args.char_indices() {
                if skip {
                    skip = false;
                } else if let Some(q) = quote {
                    if c == '\\' {
                        skip = true;
                    } else if c == q {
                        quote = None;
                    }
                } else if matches!(c, '\'' | '"' | '`') {
                    quote = Some(c);
                } else if matches!(c, '(' | '[' | '{') {
                    depth += 1;
                } else if matches!(c, ')' | ']' | '}') {
                    depth -= 1;
                } else if c == ',' && depth == 0 {
                    out.push(&args[start..i]);
                    start = i + 1;
                }
            }
            out.push(&args[start..]);
            out
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/js");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "js") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for call in [".replaceChildren(", ".append(", ".prepend("] {
                for (at, _) in text.match_indices(call) {
                    for arg in split(args(&text, at + call.len())) {
                        let arg = arg.trim();
                        assert!(
                            !(arg.ends_with(": null") || arg.ends_with(": undefined")),
                            "{}: {call} gets a child that can be null: {arg}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    /// Every command name the page's scripts pass to `invoke('...'`, from every file in
    /// `ui/js`.
    fn invoked_by_scripts() -> BTreeSet<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/js");
        let mut out = BTreeSet::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "js") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for part in text.split("invoke(").skip(1) {
                let Some(rest) = part.strip_prefix('\'') else {
                    // The definition in api.js, `invoke(cmd, ...)`, and nothing else.
                    assert!(
                        part.starts_with("cmd,") && path.ends_with("api.js"),
                        "{}: invoke( with a computed name",
                        path.display()
                    );
                    continue;
                };
                let name = &rest[..rest.find('\'').unwrap()];
                out.insert(name.to_string());
            }
        }
        out
    }
}
