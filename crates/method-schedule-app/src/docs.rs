//! Several schedules open at once (decisions.md, entry 86): one window per schedule, each
//! with its own session (undo, clipboard, autosave and recovery), kept here by window
//! label. A command finds its schedule through [`Doc`], which takes it from the window
//! that called, so a command never acts on another window's schedule.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use tauri::{Manager, Runtime, WebviewWindow};

use crate::app::AppState;
use crate::clipboard::{self, Clip, ClipboardDto};

/// The event telling a page its window's clipboard changed (`ClipboardDto` or null).
pub const CLIPBOARD_CHANGED: &str = "clipboard-changed";

/// Every open window's schedule, by window label.
#[derive(Default)]
pub struct Docs(Mutex<BTreeMap<String, Arc<AppState>>>);

impl Docs {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Arc<AppState>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The schedule of window `label`: a new, untitled one the first time a window asks.
    pub fn get(&self, label: &str) -> Arc<AppState> {
        self.lock().entry(label.to_string()).or_default().clone()
    }

    /// Puts `state` in window `label`'s place (a new window's schedule).
    pub fn insert(&self, label: &str, state: AppState) -> Arc<AppState> {
        let state = Arc::new(state);
        self.lock().insert(label.to_string(), state.clone());
        state
    }

    /// The window has closed: its schedule goes.
    pub fn remove(&self, label: &str) -> Option<Arc<AppState>> {
        self.lock().remove(label)
    }

    /// Every window's schedule, in label order.
    pub fn all(&self) -> Vec<(String, Arc<AppState>)> {
        self.lock()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Something was cut or copied in window `from`: every other window's clipboard takes
    /// `carried` (its strips carried from that schedule, src/clipboard.rs) or is emptied
    /// (elements stay in their own window), so the app has one clipboard as the user sees
    /// it (decisions.md, entry 183). Returns the windows changed and what each now holds,
    /// for their pages. Call it without holding any window's session.
    pub fn share_clip(
        &self,
        from: &str,
        carried: Option<&Clip>,
    ) -> Vec<(String, Option<ClipboardDto>)> {
        self.all()
            .into_iter()
            .filter(|(label, _)| label != from)
            .map(|(label, st)| {
                let mut s = st.lock();
                s.set_clipboard(carried.cloned());
                (label, s.clipboard().map(clipboard::dto))
            })
            .collect()
    }

    /// The window holding the schedule saved at `path`, if one does.
    pub fn holding(&self, path: &std::path::Path) -> Option<String> {
        self.all().into_iter().find_map(|(label, st)| {
            st.lock()
                .path()
                .is_some_and(|p| crate::safety::same_file(p, path))
                .then_some(label)
        })
    }
}

/// The calling window's schedule, as a command argument: commands take `state: Doc`
/// (never Tauri's managed state of an `AppState`, which a test refuses), and use it as
/// an [`AppState`].
pub struct Doc(pub Arc<AppState>);

impl Deref for Doc {
    type Target = AppState;
    fn deref(&self) -> &AppState {
        &self.0
    }
}

impl<'de, R: Runtime> CommandArg<'de, R> for Doc {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let webview = command.message.webview();
        let docs = webview
            .try_state::<Docs>()
            .ok_or_else(|| InvokeError::from("The app has no schedules yet."))?;
        Ok(Doc(docs.get(webview.label())))
    }
}

/// Shares what was just cut or copied in `window` with every other window and tells
/// their pages (see [`Docs::share_clip`]). `carried` is [`clipboard::carry`]'s result.
pub fn share_clip<R: Runtime>(window: &WebviewWindow<R>, carried: Option<Clip>) {
    use tauri::Emitter;
    let Some(docs) = window.try_state::<Docs>() else {
        return;
    };
    for (label, dto) in docs.share_clip(window.label(), carried.as_ref()) {
        let _ = window
            .app_handle()
            .emit_to(label.as_str(), CLIPBOARD_CHANGED, dto);
    }
}

/// The schedule of `window`.
pub fn doc_of<R: Runtime>(window: &WebviewWindow<R>) -> Arc<AppState> {
    window.state::<Docs>().get(window.label())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionFiles;

    #[test]
    fn each_window_has_its_own_schedule() {
        let docs = Docs::default();
        let a = docs.get("main");
        let b = docs.insert("doc-1", AppState::default());
        a.lock()
            .edit(|x| {
                x.title = "ZTEST One".into();
                Ok(())
            })
            .unwrap();
        assert_eq!(b.lock().schedule().title, "Untitled");
        assert_eq!(docs.get("main").lock().schedule().title, "ZTEST One");
        assert_eq!(docs.all().len(), 2);
        assert!(docs.remove("doc-1").is_some());
        assert_eq!(docs.all().len(), 1);
    }

    /// Cutting or copying in one window gives every other window the strips carried,
    /// or empties its clipboard for elements; the window that cut keeps its own clip
    /// (decisions.md, entry 183).
    #[test]
    fn a_copy_is_shared_with_the_other_windows() {
        let docs = Docs::default();
        let a = docs.get("main");
        let b = docs.insert("doc-1", AppState::default());
        let c = docs.insert("doc-2", AppState::default());
        let carried = Clip::Carried {
            from: "ZTEST One".into(),
            cut: false,
            items: vec![clipboard::Carried::DayBreak],
        };
        a.lock().set_clipboard(Some(Clip::Elements {
            sheet: method_schedule_core::model::SheetId(1),
            cut: false,
            elements: vec![],
        }));
        let told = docs.share_clip("main", Some(&carried));
        assert_eq!(told.len(), 2);
        assert!(told
            .iter()
            .all(|(l, d)| l != "main" && d.as_ref().is_some_and(|d| d.kind == "strips")));
        assert_eq!(b.lock().clipboard(), Some(&carried));
        assert_eq!(c.lock().clipboard(), Some(&carried));
        assert!(matches!(a.lock().clipboard(), Some(Clip::Elements { .. })));
        let told = docs.share_clip("doc-1", None);
        assert!(told.iter().all(|(_, d)| d.is_none()));
        assert!(a.lock().clipboard().is_none() && c.lock().clipboard().is_none());
        assert_eq!(b.lock().clipboard(), Some(&carried));
    }

    /// A file already open in a window is found by its path, however the path is
    /// spelled, so opening it again brings that window to the front (decisions.md, entry
    /// 86); an untitled window and another file are not it.
    #[test]
    fn the_window_holding_a_file_is_found() {
        let dir = std::env::temp_dir().join(format!("ms-docs-holding-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (one, two) = (dir.join("one.mtsc"), dir.join("two.mtsc"));
        let docs = Docs::default();
        docs.get("main");
        let b = docs.insert("doc-1", AppState::default());
        b.lock().save_to(&one).unwrap();
        assert_eq!(docs.holding(&one).as_deref(), Some("doc-1"));
        let spelled = dir.join(".").join("one.mtsc");
        assert_eq!(docs.holding(&spelled).as_deref(), Some("doc-1"));
        std::fs::write(&two, b"x").unwrap();
        assert_eq!(docs.holding(&two), None);
        docs.remove("doc-1");
        assert_eq!(docs.holding(&one), None, "a closed window holds nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What is one window's alone: its title as last set (read without asking the
    /// window), the close question, the autosave problem said and the files picked for
    /// merging.
    #[test]
    fn each_window_keeps_its_own_title_close_question_and_tokens() {
        let docs = Docs::default();
        let a = docs.get("main");
        let b = docs.insert("doc-1", AppState::default());
        assert_eq!(a.title(), "Untitled - Method Schedule");
        assert!(a.start_closing());
        assert!(!a.start_closing(), "a second close click asks nothing");
        assert!(b.start_closing(), "another window's close is its own");
        a.stop_closing();
        assert!(a.start_closing());
        *b.autosave_error.lock().unwrap() = Some("disk full".into());
        assert!(a.autosave_error.lock().unwrap().is_none());
        assert!(b.merge_files.get(1).is_err());
        assert_eq!(
            crate::windows::open_elsewhere("harbor.mtsc"),
            "harbor.mtsc is open in another window."
        );
    }

    /// Commands take the calling window's schedule: a command written with Tauri's
    /// managed state of an `AppState` would act on no window's schedule at all.
    #[test]
    fn no_command_takes_the_old_single_schedule() {
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let needle = ["State<'_, ", "AppState>"].concat();
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !text.contains(&needle),
                    "{}: take `state: Doc` (src/docs.rs) instead",
                    path.display()
                );
            }
        }
    }
}
