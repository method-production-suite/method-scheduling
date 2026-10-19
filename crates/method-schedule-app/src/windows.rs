//! Windows: one per schedule (decisions.md, entry 86), File > New Window and Open in New
//! Window, Close Window (entry 79), and the Window menu that lists them. Each window's
//! schedule lives in src/docs.rs under the window's label; the main window is `main`,
//! the others `doc-1`, `doc-2` and so on, all granted the same commands
//! (capabilities/main.json).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tauri::menu::{MenuItem, MenuItemKind};
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

use crate::app::{AppState, CmdResult, MAIN};
use crate::docs::Docs;
use crate::safety_app::{self, Safety};
use crate::session::SessionFiles;
use crate::session::{self, Session};

/// The labels of the windows opened after the main one, as the capability names them.
pub const LABEL_GLOB: &str = "doc-*";
/// The Window menu's list of windows (Windows and Linux; macOS lists them itself).
pub const WINDOW_LIST: &str = "window.list";
/// A Window menu item: `window.show.<label>`.
pub const SHOW_PREFIX: &str = "window.show.";

static NEXT: AtomicU32 = AtomicU32::new(1);

/// The window that has focus, else the main one, else any.
pub fn focused<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let all = app.webview_windows();
    all.values()
        .find(|w| w.is_focused().unwrap_or(false))
        .cloned()
        .or_else(|| all.get(MAIN).cloned())
        .or_else(|| all.values().next().cloned())
}

/// File > Close Window from the menu: closes the focused window, which asks about
/// unsaved changes first (the window's close request, src/app.rs).
pub fn close_focused<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = focused(app) {
        let _ = w.close();
    }
}

/// How far a new window sits below and right of the window it opens from, in points.
pub const CASCADE: f64 = 26.0;

/// Where a new window's top left goes: [`CASCADE`] points below and right of the window it
/// opens from (`from`, its top left), so it never hides that window exactly. When that
/// would push the new window (`size`) past the screen's right or bottom edge (`screen`:
/// its top left and size), it starts again at the screen's top left, one step in. All in
/// points.
pub fn cascade(from: (f64, f64), size: (f64, f64), screen: (f64, f64, f64, f64)) -> (f64, f64) {
    let (sx, sy, sw, sh) = screen;
    let (x, y) = (from.0 + CASCADE, from.1 + CASCADE);
    if x + size.0 > sx + sw || y + size.1 > sy + sh {
        (sx + CASCADE, sy + CASCADE)
    } else {
        (x, y)
    }
}

/// Where a window opened from `from` goes ([`cascade`]), when its place and screen are
/// known.
fn cascade_from<R: Runtime>(from: &WebviewWindow<R>, size: (f64, f64)) -> Option<(f64, f64)> {
    let scale = from.scale_factor().ok()?;
    let at = from.outer_position().ok()?.to_logical::<f64>(scale);
    let screen = from.current_monitor().ok().flatten().map_or(
        (0.0, 0.0, f64::MAX / 4.0, f64::MAX / 4.0),
        |m| {
            let p = m.position().to_logical::<f64>(m.scale_factor());
            let s = m.size().to_logical::<f64>(m.scale_factor());
            (p.x, p.y, s.width, s.height)
        },
    );
    Some(cascade((at.x, at.y), size, screen))
}

/// Opens a new window holding `session`, cascaded from `from` (the window it opens from)
/// when there is one.
pub fn open_window<R: Runtime>(
    app: &AppHandle<R>,
    session: Session,
    from: Option<&WebviewWindow<R>>,
) -> tauri::Result<WebviewWindow<R>> {
    let label = format!("doc-{}", NEXT.fetch_add(1, Ordering::SeqCst));
    let state = AppState::with(session);
    let title = state.title();
    let docs = app.state::<Docs>();
    docs.insert(&label, state);
    let size = (1180.0, 800.0);
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(760.0, 520.0);
    if let Some((x, y)) = from.and_then(|w| cascade_from(w, size)) {
        builder = builder.position(x, y);
    }
    let built = builder.build();
    if built.is_err() {
        // No window, so no schedule kept for it (another Open would find its file here).
        docs.remove(&label);
    }
    refresh_menu(app);
    built
}

/// A window has closed: its schedule goes. Changes the user didn't throw away are kept
/// (into the file with Autosave on, else a recovery copy), as on quitting.
pub fn closed<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let (Some(docs), Some(safety)) = (app.try_state::<Docs>(), app.try_state::<Safety>()) {
        if let Some(state) = docs.remove(label) {
            safety_app::leaving(&state, &safety);
        }
    }
    refresh_menu(app);
}

/// The windows in the Window menu's order: the main one first, then by when they opened,
/// each with the title last set (kept with its schedule, so no window is asked and no
/// session is locked).
fn listed<R: Runtime>(app: &AppHandle<R>) -> Vec<(String, String)> {
    let open = app.webview_windows();
    let mut v: Vec<(u32, String, String)> = app
        .state::<Docs>()
        .all()
        .into_iter()
        .filter(|(label, _)| open.contains_key(label))
        .map(|(label, state)| {
            let rank = label
                .strip_prefix("doc-")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            (rank, label, state.title())
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, l, t)| (l, t)).collect()
}

/// Brings the Window menu's list up to date (each window's title, with "(edited)" for
/// unsaved changes). macOS keeps its own list in its Window menu. The work runs on the
/// main thread once it is free, so this never waits for it and is safe from anywhere,
/// under a session's lock too (decisions.md, entry 88).
pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>) {
    if cfg!(target_os = "macos") {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || rebuild_menu(&handle));
}

fn rebuild_menu<R: Runtime>(app: &AppHandle<R>) {
    let Some(menu) = app.menu() else {
        return;
    };
    let Some(MenuItemKind::Submenu(list)) = menu
        .items()
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.id() == WINDOW_LIST)
    else {
        return;
    };
    let _ = (|| -> tauri::Result<()> {
        while list.remove_at(0)?.is_some() {}
        for (label, title) in listed(app) {
            let id = format!("{SHOW_PREFIX}{label}");
            list.append(&MenuItem::with_id(app, id, title, true, None::<&str>)?)?;
        }
        Ok(())
    })();
}

/// Window menu items: brings that window to the front. False for any other id.
pub fn handle_menu<R: Runtime>(app: &AppHandle<R>, id: &str) -> bool {
    let Some(label) = id.strip_prefix(SHOW_PREFIX) else {
        return false;
    };
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    true
}

/// File > Close Window from its key on Windows and Linux (the page answers app keys
/// there): closes this window through the same question.
#[tauri::command]
pub fn close_window<R: Runtime>(window: WebviewWindow<R>) {
    let _ = window.close();
}

/// File > New Window: a new window with a new, untitled schedule. Async, as a window
/// made from a synchronous command can hang on Windows.
#[tauri::command]
pub async fn new_window<R: Runtime>(window: WebviewWindow<R>) -> CmdResult<()> {
    open_window(window.app_handle(), Session::new(), Some(&window))
        .map(|_| ())
        .map_err(|e| format!("A new window couldn't be opened: {e}"))
}

/// The open window other than `except` holding the schedule saved at `path`. Never
/// called under a session's lock (it locks every window's in turn).
pub fn holder<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
    except: &str,
) -> Option<WebviewWindow<R>> {
    app.state::<Docs>()
        .holding(path)
        .filter(|label| label != except)
        .and_then(|label| app.get_webview_window(&label))
}

/// The window holding the schedule saved at `path`, other than `except`: brought to the
/// front. True when there is one (false when no window is open for it).
pub fn focus_holder<R: Runtime>(app: &AppHandle<R>, path: &Path, except: &str) -> bool {
    let Some(w) = holder(app, path, except) else {
        return false;
    };
    let _ = w.set_focus();
    true
}

/// Refuses `path` when another window has it open: saving or opening it here would leave
/// two windows writing one file.
pub fn refuse_if_open_elsewhere<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
    here: &str,
) -> Result<(), String> {
    if holder(app, path, here).is_none() {
        return Ok(());
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    Err(open_elsewhere(&name))
}

pub(crate) fn open_elsewhere(name: &str) -> String {
    format!("{name} is open in another window.")
}

/// File > Open in New Window…: an Open dialog; the schedule opens in a window of its own
/// (a file already open in a window brings that window to the front). False when
/// cancelled.
#[tauri::command]
pub async fn open_in_new_window<R: Runtime>(window: WebviewWindow<R>) -> CmdResult<bool> {
    let start = crate::docs::doc_of(&window)
        .lock()
        .path()
        .and_then(Path::parent)
        .map(PathBuf::from);
    let mut dialog = window
        .dialog()
        .file()
        .set_title("Open in a new window")
        .add_filter(
            "Schedule file",
            &[session::EXTENSION, "json", crate::templates::EXTENSION],
        )
        .set_parent(&window);
    if let Some(dir) = start {
        dialog = dialog.set_directory(dir);
    }
    let Some(picked) = dialog.blocking_pick_file() else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let app = window.app_handle();
    if focus_holder(app, &path, "") {
        return Ok(true);
    }
    let next = Session::open(&path)?;
    if next.path().is_some() {
        app.state::<Safety>().remember(&path);
        safety_app::refresh_menu(app);
    }
    open_window(app, next, Some(&window))
        .map_err(|e| format!("A new window couldn't be opened: {e}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_windows_cascade_and_start_over_at_the_edge() {
        let screen = (0.0, 25.0, 1728.0, 1092.0);
        let size = (1180.0, 800.0);
        assert_eq!(cascade((100.0, 60.0), size, screen), (126.0, 86.0));
        // Each from the last: never on top of the one before.
        let a = cascade((0.0, 25.0), size, screen);
        let b = cascade(a, size, screen);
        assert!(a != (0.0, 25.0) && b != a);
        // Past the right or bottom edge: back to the top left, one step in.
        assert_eq!(cascade((540.0, 60.0), size, screen), (26.0, 51.0));
        assert_eq!(cascade((100.0, 300.0), size, screen), (26.0, 51.0));
        // A second screen to the left of the first.
        let left = (-1920.0, 0.0, 1920.0, 1080.0);
        assert_eq!(cascade((-1900.0, 10.0), size, left), (-1874.0, 36.0));
    }

    /// A window's getters wait for the main thread, so a command holding a schedule's
    /// lock that asked one could deadlock against the main thread waiting for that lock
    /// (decisions.md, entry 88). The title is kept with the schedule instead.
    #[test]
    fn nothing_asks_a_window_for_its_title() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for bad in [["window", ".title()"].concat(), ["w", ".title()"].concat()] {
                assert!(!text.contains(&bad), "{}: {bad}", path.display());
            }
        }
    }

    #[test]
    fn labels_match_the_capability() {
        const CAPABILITY: &str = include_str!("../capabilities/main.json");
        let cap: serde_json::Value = serde_json::from_str(CAPABILITY).unwrap();
        let windows: Vec<&str> = cap["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w.as_str())
            .collect();
        assert_eq!(windows, [MAIN, LABEL_GLOB]);
        let label = format!("doc-{}", 7);
        assert!(label.starts_with(LABEL_GLOB.trim_end_matches('*')));
    }
}
