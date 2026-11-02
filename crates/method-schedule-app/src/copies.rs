//! Conflict copies a sync service leaves beside the schedule's file (docs/APP.md, "Saving
//! and recovery"; decisions.md, entry 241).
//!
//! When two computers change one file in a shared folder before either has the other's
//! version, the sync service keeps one as the file and saves the other beside it under a
//! new name: `film (conflicted copy 2026-10-30).mtsc`, `film 2.mtsc`, `film (1).mtsc`,
//! `film.sync-conflict-20261030-140312-ABC.mtsc`. Nothing in the app would ever look at
//! that copy, so its changes would be lost without anyone knowing. The window looks for
//! such copies of its own file (when it gets focus, and when its file changes) and offers
//! to merge each into the schedule with Merge Changes From, against the automatic base
//! (the newest backup from before the copy was last changed).
//!
//! The folder is only listed; a copy is read when the user chooses to merge it, and is
//! named by its file name, checked against the listing (paths never come from the page).
//! A copy merged or dismissed isn't offered again in this window until it changes.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use serde::Serialize;

use crate::app::{AppState, CmdResult};
use crate::diffmerge::{read_theirs, utc_minutes, PickedDto, Theirs};
use crate::docs::Doc;

/// At most this many copies are listed, newest first.
const LISTED: usize = 20;

/// Whether `other` (a file name) is the kind of name a sync service gives a conflict copy
/// of `own` (the schedule's file name). Both must be schedule files (`.mtsc`, any case).
/// The name after `own`'s stem is one of: ` (… conflict …)`, ` 2` to ` 99`, ` (1)` to
/// ` (99)`, or `.sync-conflict-…`.
pub fn is_copy_of(own: &str, other: &str) -> bool {
    let (Some(stem), Some(other_stem)) = (schedule_stem(own), schedule_stem(other)) else {
        return false;
    };
    let Some(rest) = other_stem.strip_prefix(stem) else {
        return false;
    };
    let number = |s: &str| {
        !s.is_empty()
            && s.len() <= 2
            && s.bytes().all(|b| b.is_ascii_digit())
            && !s.starts_with('0')
    };
    if let Some(n) = rest.strip_prefix(' ') {
        if number(n) {
            return n != "1";
        }
    }
    if let Some(inner) = rest.strip_prefix(" (").and_then(|r| r.strip_suffix(')')) {
        return number(inner) || inner.to_lowercase().contains("conflict");
    }
    rest.starts_with(".sync-conflict-")
}

/// A schedule file name's stem: `None` when it isn't a `.mtsc` file.
fn schedule_stem(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    let (stem, ext) = name.split_at(dot);
    (!stem.is_empty() && ext.eq_ignore_ascii_case(".mtsc")).then_some(stem)
}

/// The conflict copies of the file at `own` in its folder, newest first (at most
/// [`LISTED`]), each with when it was last changed. Empty when the folder can't be listed.
pub fn find(own: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let (Some(dir), Some(own_name)) = (own.parent(), own.file_name()) else {
        return Vec::new();
    };
    let own_name = own_name.to_string_lossy();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<_> = entries
        .flatten()
        .filter(|e| is_copy_of(&own_name, &e.file_name().to_string_lossy()))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| (e.path(), meta.modified().ok()))
        })
        .collect();
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found.truncate(LISTED);
    found
}

/// The copies merged or dismissed in one window, each with when it was last changed then:
/// one changed since is offered again.
#[derive(Default)]
pub struct Settled(Mutex<Vec<(PathBuf, Option<SystemTime>)>>);

impl Settled {
    fn lock(&self) -> MutexGuard<'_, Vec<(PathBuf, Option<SystemTime>)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn holds(&self, path: &Path, modified: Option<SystemTime>) -> bool {
        self.lock().iter().any(|(p, m)| p == path && *m == modified)
    }

    fn add(&self, path: PathBuf, modified: Option<SystemTime>) {
        let mut g = self.lock();
        g.retain(|(p, _)| *p != path);
        g.push((path, modified));
    }
}

/// A conflict copy, for the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CopyDto {
    /// Its file name: the page names it by this.
    pub name: String,
    /// When it was last changed, `2026-10-30 14:03 UTC`, or empty.
    pub changed_at: String,
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The copies to offer: the conflict copies of the schedule's file, less those settled
/// here and unchanged since. Empty for a schedule never saved.
pub fn offered(state: &AppState) -> Vec<CopyDto> {
    let Some(own) = state.lock().path().map(PathBuf::from) else {
        return Vec::new();
    };
    find(&own)
        .into_iter()
        .filter(|(p, m)| !state.copies.holds(p, *m))
        .map(|(p, m)| CopyDto {
            name: name_of(&p),
            changed_at: m.map(utc_minutes).unwrap_or_default(),
        })
        .collect()
}

/// Copy `name` of the schedule's file, found in its folder now, with when it was last
/// changed.
fn listed(state: &AppState, name: &str) -> Result<(PathBuf, Option<SystemTime>), String> {
    let own = state
        .lock()
        .path()
        .map(PathBuf::from)
        .ok_or("This schedule hasn't been saved to a file yet.")?;
    find(&own)
        .into_iter()
        .find(|(p, _)| name_of(p) == name)
        .ok_or_else(|| format!("{name} is no longer beside {}.", name_of(&own)))
}

/// Reads copy `name` as theirs, for Merge Changes From.
pub fn read(state: &AppState, name: &str) -> Result<Theirs, String> {
    let (path, _) = listed(state, name)?;
    let own = state.lock().path().map(PathBuf::from);
    read_theirs(&path, own.as_deref())
}

/// Copy `name` is settled here (merged, or dismissed) as it is now.
pub fn settle(state: &AppState, name: &str) -> Result<(), String> {
    let (path, modified) = listed(state, name)?;
    state.copies.add(path, modified);
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands (registered in app.rs, build.rs and capabilities/main.json)

/// The conflict copies of the schedule's file to offer, newest first.
#[tauri::command]
pub fn copies_found(state: Doc) -> CmdResult<Vec<CopyDto>> {
    Ok(offered(&state))
}

/// Reads copy `name` for Merge Changes From: its token, for `merge_preview` and
/// `merge_apply` (base `{kind: "auto"}`).
#[tauri::command]
pub fn copy_merge(state: Doc, name: String) -> CmdResult<PickedDto> {
    let t = read(&state, &name)?;
    let name = t.name.clone();
    Ok(PickedDto {
        token: state.merge_files.add(t),
        name,
    })
}

/// Copy `name` is merged or dismissed: not offered again in this window until it changes.
#[tauri::command]
pub fn copy_settle(state: Doc, name: String) -> CmdResult<()> {
    settle(&state, &name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffmerge::{apply, BaseChoice, MergeInputs};
    use crate::safety::tests::temp_dir;
    use crate::safety_app::{save, Safety};
    use crate::session::read_schedule;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn the_names_sync_services_give_conflict_copies() {
        for other in [
            "film (conflicted copy).mtsc",
            "film (Alex's conflicted copy 2026-10-30).mtsc",
            "film (conflicted copy 2026-10-30 140312).mtsc",
            "film (Case Conflict).mtsc",
            "film 2.mtsc",
            "film 12.mtsc",
            "film (1).mtsc",
            "film (2).mtsc",
            "film.sync-conflict-20261030-140312-ABCDEFG.mtsc",
            "film (conflicted copy).MTSC",
        ] {
            assert!(is_copy_of("film.mtsc", other), "{other}");
        }
        for other in [
            "film.mtsc",
            "film 1.mtsc",
            "film 02.mtsc",
            "film 100.mtsc",
            "film reshoots.mtsc",
            "film-2.mtsc",
            "film (director's cut).mtsc",
            "film (conflicted copy).mtst",
            "film (conflicted copy).mtsc.json",
            "films 2.mtsc",
            "other 2.mtsc",
            "film (0).mtsc",
            "film ().mtsc",
        ] {
            assert!(!is_copy_of("film.mtsc", other), "{other}");
        }
        assert!(!is_copy_of("film", "film 2.mtsc"), "own must be a schedule");
        assert!(is_copy_of("My Film.mtsc", "My Film 2.mtsc"));
    }

    fn sample() -> method_schedule_core::Schedule {
        method_schedule_core::sample::sample(20)
    }

    #[test]
    fn copies_are_found_newest_first_and_settled_until_they_change() {
        let dir = temp_dir("copies-find");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        assert!(offered(&state).is_empty(), "never saved: no folder");
        let path = dir.join("film.mtsc");
        state
            .lock()
            .edit(|x| {
                *x = sample();
                Ok(())
            })
            .unwrap();
        save(&state, &safety, &path).unwrap();
        let text = fs::read(&path).unwrap();
        fs::write(dir.join("film 2.mtsc"), &text).unwrap();
        fs::write(dir.join("film reshoots.mtsc"), &text).unwrap();
        fs::create_dir(dir.join("film (1).mtsc")).unwrap();
        // Newer by its time.
        let newer = dir.join("film (conflicted copy).mtsc");
        fs::write(&newer, &text).unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(&newer)
            .unwrap()
            .set_modified(later)
            .unwrap();
        let names: Vec<_> = offered(&state).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["film (conflicted copy).mtsc", "film 2.mtsc"]);

        settle(&state, "film 2.mtsc").unwrap();
        let names: Vec<_> = offered(&state).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["film (conflicted copy).mtsc"]);
        // Changed since: offered again.
        fs::File::options()
            .write(true)
            .open(dir.join("film 2.mtsc"))
            .unwrap()
            .set_modified(later + std::time::Duration::from_secs(60))
            .unwrap();
        assert_eq!(offered(&state).len(), 2);

        // Only a listed copy is read, by its name.
        assert!(read(&state, "film reshoots.mtsc").is_err());
        assert!(read(&state, "../film 2.mtsc").is_err());
        assert!(settle(&state, "film.mtsc").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_conflict_copy_merges_in_against_the_backup_from_before_both() {
        let dir = temp_dir("copies-merge");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        state
            .lock()
            .edit(|x| {
                *x = sample();
                Ok(())
            })
            .unwrap();
        save(&state, &safety, &path).unwrap();
        let el = state.lock().schedule().elements[1].id;
        // The other computer renames an element; the sync service keeps its version as
        // a copy. This one renames the schedule and saves (the backup is the version both
        // started from).
        let (mut theirs, _) = read_schedule(&path).unwrap();
        theirs.rename_element(el, "THEIR NAME").unwrap();
        let copy = dir.join("film (conflicted copy).mtsc");
        fs::write(&copy, method_schedule_format::to_string(&theirs)).unwrap();
        // The backup must come before the copy for the automatic base to pick it.
        let backup_time = SystemTime::now() - std::time::Duration::from_secs(120);
        state
            .lock()
            .edit(|x| {
                x.title = "Mine".into();
                Ok(())
            })
            .unwrap();
        save(&state, &safety, &path).unwrap();
        for b in safety.backups_of(&path) {
            fs::File::options()
                .write(true)
                .open(&b)
                .unwrap()
                .set_modified(backup_time)
                .unwrap();
        }

        let t = read(&state, "film (conflicted copy).mtsc").unwrap();
        assert!(t.disk.is_none(), "a copy is never the file on disk");
        let inputs = MergeInputs::gather(&state, &safety, t).unwrap();
        let p = inputs.preview(&BaseChoice::Auto, &BTreeMap::new());
        assert!(p.problem.is_none(), "{:?}", p.problem);
        assert!(p.conflicts.is_empty(), "{:?}", p.conflicts);
        assert_ne!(p.base, "disk", "{}", p.base_reason);
        apply(&state, &inputs, &BaseChoice::Auto, &BTreeMap::new()).unwrap();
        let s = state.lock();
        assert_eq!(s.schedule().title, "Mine", "yours kept");
        assert!(
            s.schedule().elements.iter().any(|e| e.name == "THEIR NAME"),
            "the copy's change merged in"
        );
        drop(s);
        settle(&state, "film (conflicted copy).mtsc").unwrap();
        assert!(offered(&state).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
