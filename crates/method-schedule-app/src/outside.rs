//! Changes made outside the app, settled in the app, and File > Revert to Backup
//! (docs/APP.md, "Saving and recovery"; decisions.md, entries 179 to 182).
//!
//! - **The check.** When the window gets focus, and before Save, the schedule's file is
//!   compared with the version this window last read or wrote (size, modification time,
//!   content hash: [`DiskStamp::quick_check`]). A file changed by another program pauses
//!   autosave into it (the guard in src/session.rs already refuses every write) and the
//!   page is offered what [`decide`] says: Load Theirs or Keep Mine when this window has
//!   nothing unsaved; Merge, Keep Mine or Load Theirs (after a confirmation) when both
//!   changed. Later leaves it paused, as before, and the check doesn't ask again about
//!   the same version.
//! - **Merge** reuses Merge Changes From (src/diffmerge.rs): theirs is the file on disk,
//!   the base the version last read here.
//! - **Revert to Backup** lists the file's rolling backups and makes the one picked the
//!   schedule, as one undo step in this window.
//!
//! Paths never come from the page: the file is the window's own, a backup is named by its
//! file name and checked against the backups folder's listing.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, WebviewWindow};

use method_schedule_core::diff::diff;

use crate::app::{finish, AppState, CmdResult};
use crate::diffmerge::{utc_minutes, Theirs};
use crate::docs::Doc;
use crate::safety::StampAt;
use crate::safety::{self, content_hash, DiskCheck, DiskStamp};
use crate::safety_app::{self, Safety};
use crate::session::SessionFiles;
use crate::session::{parse_schedule, read_schedule};
use crate::view::View;

/// Event to a window when it gets focus: the page asks for the check (`outside_check`).
pub const OUTSIDE_CHECK: &str = "outside-check";

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

// ---------------------------------------------------------------------------
// The decision

/// One way to settle an outside change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// The version on disk becomes the schedule (one undo step).
    LoadTheirs,
    /// The schedule in the window is saved over the version on disk (backed up first).
    KeepMine,
    /// The version on disk is merged into the schedule in the window, against the
    /// version both started from.
    Merge,
}

/// What to offer: the choices in the order the page shows them, the one Return takes,
/// and whether Load Theirs asks again first (it throws away unsaved changes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub choices: Vec<Choice>,
    pub default: Choice,
    pub confirm_load: bool,
}

/// What the check does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing to settle: the file holds what this window last read or wrote, or it is
    /// gone (a save writes it again).
    Nothing,
    Ask(Offer),
}

/// The decision, from the file on disk (`disk`; `theirs_readable`: it reads as a
/// schedule) and this window (`dirty`: unsaved changes; `has_base`: the version last
/// read here is kept, so a merge has its base). Never an action that writes over the
/// outside change without the user choosing it.
pub fn decide(disk: DiskCheck, theirs_readable: bool, dirty: bool, has_base: bool) -> Action {
    use Choice::*;
    if disk != DiskCheck::Changed {
        return Action::Nothing;
    }
    let (choices, default) = match (theirs_readable, dirty, has_base) {
        // Theirs can't be loaded or merged; keeping yours (theirs into the backups) or
        // putting it off are all there is.
        (false, _, _) => (vec![KeepMine], KeepMine),
        (true, false, _) => (vec![LoadTheirs, KeepMine], LoadTheirs),
        (true, true, true) => (vec![Merge, KeepMine, LoadTheirs], Merge),
        (true, true, false) => (vec![KeepMine, LoadTheirs], KeepMine),
    };
    Action::Ask(Offer {
        choices,
        default,
        confirm_load: dirty,
    })
}

// ---------------------------------------------------------------------------
// The check, without Tauri

/// An outside change found, for the page.
#[derive(Clone, Debug, Serialize)]
pub struct OutsideDto {
    /// The file's name.
    pub name: String,
    /// The hash of the version found, as text: the page sends it back with the choice, so
    /// a version changed again since is never settled unseen.
    pub version: String,
    /// When the file was last changed, `2026-10-11 14:03 UTC`, or empty.
    pub changed_at: String,
    pub dirty: bool,
    #[serde(flatten)]
    pub offer: Offer,
    /// Why the version on disk can't be loaded or merged, when it can't.
    pub problem: Option<String>,
    /// Merge's token for the version on disk (Merge Changes From's), when Merge is offered.
    pub token: Option<u64>,
}

/// What the check found: the page's part, and theirs for Merge.
pub struct Found {
    pub dto: OutsideDto,
    pub theirs: Option<Theirs>,
}

/// The check. `None` when there is nothing to settle, or (unless `force`, as before Save)
/// when the user put off this same version. A change found pauses autosave into the file.
pub fn check(state: &AppState, force: bool) -> Option<Found> {
    let mut s = state.lock();
    let path = s.path()?.to_path_buf();
    if s.disk_quick_check() != DiskCheck::Changed {
        return None;
    }
    let bytes = match safety::read_limited(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            // Can't be read: a change as far as the guard is concerned.
            s.set_conflict(true);
            let hash = 0;
            if !force && s.put_off_hash() == Some(hash) {
                return None;
            }
            return Some(found(
                &s,
                &path,
                hash,
                Err(format!("{}: {e}", file_name(&path))),
            ));
        }
    };
    let hash = content_hash(&bytes);
    s.set_conflict(true);
    if !force && s.put_off_hash() == Some(hash) {
        return None;
    }
    let stamp = DiskStamp::at(&bytes, &path);
    let theirs = parse_schedule(&bytes, &path).map(|schedule| Theirs {
        name: format!("{} on disk", file_name(&path)),
        path: path.clone(),
        schedule: Arc::new(schedule),
        modified: stamp.modified,
        disk: Some(stamp),
    });
    Some(found(&s, &path, hash, theirs))
}

fn found(
    s: &crate::session::Session,
    path: &Path,
    hash: u64,
    theirs: Result<Theirs, String>,
) -> Found {
    let dirty = s.is_dirty();
    let has_base = s.last_read().is_some();
    let Action::Ask(offer) = decide(DiskCheck::Changed, theirs.is_ok(), dirty, has_base) else {
        unreachable!("a change always asks");
    };
    let changed_at = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(utc_minutes)
        .unwrap_or_default();
    let (theirs, problem) = match theirs {
        Ok(t) => (Some(t), None),
        Err(e) => (
            None,
            Some(format!(
                "The version on disk can't be read as a schedule ({e}), so it can't be loaded \
                 or merged."
            )),
        ),
    };
    Found {
        dto: OutsideDto {
            name: s.display_name(),
            version: hash.to_string(),
            changed_at,
            dirty,
            offer,
            problem,
            token: None,
        },
        theirs,
    }
}

fn changed_again(name: &str) -> String {
    format!("{name} changed again on disk just now. Look at it again before choosing.")
}

/// The file now: `None` when it is gone, else its bytes, refused when they aren't the
/// version `version` the user saw.
fn read_seen(path: &Path, version: u64) -> Result<Option<Vec<u8>>, String> {
    match safety::read_limited(path) {
        Ok(b) if content_hash(&b) == version => Ok(Some(b)),
        Ok(_) => Err(changed_again(&file_name(path))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", file_name(path))),
    }
}

/// Keep Mine: the version on disk the user saw (`version`) goes into the backups, and the
/// schedule in the window is saved over it. Returns the notice.
pub fn keep_mine(state: &AppState, safety: &Safety, version: u64) -> Result<String, String> {
    let path = {
        let _w = safety.write_lock();
        let mut s = state.lock();
        let path = s
            .path()
            .ok_or("This schedule has no file yet.")?
            .to_path_buf();
        if let Some(bytes) = read_seen(&path, version)? {
            let theirs = parse_schedule(&bytes, &path).ok().map(Arc::new);
            s.took_disk(DiskStamp::at(&bytes, &path), theirs);
        }
        path
    };
    // An explicit save: it backs up what is on disk (theirs) first, and the guard still
    // refuses a file changed again since.
    safety_app::save(state, safety, &path)?;
    Ok(format!(
        "Kept yours and saved it into {}. The version changed outside the app is in the \
         backups (File > Revert to Backup).",
        file_name(&path)
    ))
}

/// Load Theirs: the version on disk the user saw (`version`) becomes the schedule, as one
/// undo step. Returns the notice.
pub fn load_theirs(state: &AppState, safety: &Safety, version: u64) -> Result<String, String> {
    let _w = safety.write_lock();
    let mut s = state.lock();
    let path = s
        .path()
        .ok_or("This schedule has no file yet.")?
        .to_path_buf();
    let bytes = read_seen(&path, version)?
        .ok_or_else(|| format!("{} is no longer on disk.", file_name(&path)))?;
    let theirs = parse_schedule(&bytes, &path)?;
    let had_changes = s.is_dirty();
    s.load_disk(DiskStamp::at(&bytes, &path), theirs)?;
    if s.recovery_stale() {
        safety.remove_recovery(&mut s);
    }
    Ok(if had_changes {
        format!(
            "Loaded {} as it is on disk. Undo brings your changes back.",
            file_name(&path)
        )
    } else {
        format!("Loaded {} as it is on disk.", file_name(&path))
    })
}

/// Later: the user put off this version; autosave into the file stays paused.
pub fn put_off(state: &AppState, version: u64) {
    state.lock().put_off(version);
}

// ---------------------------------------------------------------------------
// Revert to Backup, without Tauri

/// One backup, for the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackupDto {
    /// Its file name in the backups folder: the page names it by this.
    pub name: String,
    /// `Backup from 2026-10-11 14:03 UTC`.
    pub label: String,
    /// How many changes reverting to it makes, or `None` when it can't be read.
    pub changes: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackupsDto {
    /// The schedule's file name.
    pub file: String,
    /// Newest first.
    pub backups: Vec<BackupDto>,
}

fn backup_label(p: &Path) -> String {
    let when = std::fs::metadata(p)
        .and_then(|m| m.modified())
        .map(utc_minutes)
        .unwrap_or_default();
    format!("Backup from {when}")
}

fn own_path(state: &AppState) -> Result<PathBuf, String> {
    state.lock().path().map(PathBuf::from).ok_or_else(|| {
        "This schedule hasn't been saved to a file yet, so it has no backups.".to_string()
    })
}

/// The schedule file's backups, newest first, each with how many changes reverting to it
/// makes. Refused for a schedule never saved.
pub fn backups(state: &AppState, safety: &Safety) -> Result<BackupsDto, String> {
    let path = own_path(state)?;
    let now = state.lock().schedule().clone();
    let backups = safety
        .backups_of(&path)
        .iter()
        .rev()
        .map(|p| BackupDto {
            name: file_name(p),
            label: backup_label(p),
            changes: read_schedule(p)
                .ok()
                .map(|(b, _)| diff(&now, &b).changes.len()),
        })
        .collect();
    Ok(BackupsDto {
        file: file_name(&path),
        backups,
    })
}

/// Makes backup `name` the schedule, as one undo step in this window. The file as it is
/// now goes into the backups first (unless it is the same as the newest one). Returns the
/// notice.
/// Backup `name` of the open schedule's file (its file name in the backups folder), read,
/// with its label (`Backup from 2026-10-11 14:03 UTC`).
pub fn read_backup(
    state: &AppState,
    safety: &Safety,
    name: &str,
) -> Result<(method_schedule_core::Schedule, String), String> {
    let path = own_path(state)?;
    let picked = safety
        .backups_of(&path)
        .into_iter()
        .find(|p| file_name(p) == name)
        .ok_or("That backup is no longer in the backups folder.")?;
    let (backup, _) = read_schedule(&picked)?;
    Ok((backup, backup_label(&picked)))
}

pub fn revert(state: &AppState, safety: &Safety, name: &str) -> Result<String, String> {
    let path = own_path(state)?;
    let (backup, label) = read_backup(state, safety, name)?;
    if !state.lock().changed_on_disk() {
        safety.back_up_file(&path);
    }
    let mut s = state.lock();
    let before = s.schedule().clone();
    s.edit(|x| {
        *x = backup;
        Ok(())
    })?;
    if *s.schedule() == before {
        return Ok(format!(
            "Nothing to revert: the schedule is the same as the {}.",
            lower_first(&label)
        ));
    }
    Ok(format!(
        "Reverted to the {}. Undo takes it back.",
        lower_first(&label)
    ))
}

pub(crate) fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Commands (registered in app.rs, build.rs and capabilities/main.json)

/// The check (when the window gets focus; `force` before Save asks again about a version
/// put off). `None` when there is nothing to settle.
#[tauri::command]
pub fn outside_check(state: Doc, force: bool) -> CmdResult<Option<OutsideDto>> {
    let Some(f) = check(&state, force) else {
        return Ok(None);
    };
    let mut dto = f.dto;
    if let (Some(t), true) = (f.theirs, dto.offer.choices.contains(&Choice::Merge)) {
        dto.token = Some(state.merge_files.add(t));
    }
    Ok(Some(dto))
}

/// Settles the outside change the page was shown (`version`): `keep_mine`, `load_theirs`
/// or `later`. Merge goes through `merge_preview` and `merge_apply` with the token.
#[tauri::command]
pub fn outside_resolve<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    choice: String,
    version: String,
) -> CmdResult<View> {
    let version: u64 = version
        .parse()
        .map_err(|_| "That isn't a version of the file.".to_string())?;
    let safety = window.state::<Safety>();
    let notice = match choice.as_str() {
        "keep_mine" => Some(keep_mine(&state, &safety, version)?),
        "load_theirs" => Some(load_theirs(&state, &safety, version)?),
        "later" => {
            put_off(&state, version);
            None
        }
        _ => return Err("Unknown choice.".into()),
    };
    if notice.is_some() {
        safety_app::refresh_menu(window.app_handle());
    }
    finish(&window, &state.lock(), notice)
}

/// File > Revert to Backup: the backups to pick from.
#[tauri::command]
pub fn backup_list<R: Runtime>(window: WebviewWindow<R>, state: Doc) -> CmdResult<BackupsDto> {
    backups(&state, &window.state::<Safety>())
}

/// Makes backup `name` the schedule, as one undo step.
#[tauri::command]
pub fn revert_to_backup<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    name: String,
) -> CmdResult<View> {
    let notice = revert(&state, &window.state::<Safety>(), &name)?;
    finish(&window, &state.lock(), Some(notice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffmerge::{BaseChoice, MergeInputs};
    use crate::safety::tests::temp_dir;
    use crate::safety_app::{autosave, save, Autosaved};
    use std::collections::BTreeMap;
    use std::fs;

    fn later() -> u64 {
        method_schedule_session::clock::mono_ms()
            + safety::AUTOSAVE_QUIET.as_millis() as u64
            + 1_000
    }

    fn rename(state: &AppState, title: &str) {
        state
            .lock()
            .edit(|x| {
                x.title = title.into();
                Ok(())
            })
            .unwrap();
    }

    /// The file, written by "another program": the schedule as saved, with `edit` done.
    fn write_theirs(path: &Path, edit: impl FnOnce(&mut method_schedule_core::Schedule)) {
        let (mut s, _) = read_schedule(path).unwrap();
        edit(&mut s);
        fs::write(path, method_schedule_format::to_string(&s)).unwrap();
    }

    #[test]
    fn the_decision_for_every_disk_and_window_state() {
        use Choice::*;
        // Nothing to settle: the same content, or a file gone (a save writes it again).
        for disk in [DiskCheck::Same, DiskCheck::Missing] {
            for (r, d, b) in [
                (true, true, true),
                (false, false, false),
                (true, false, true),
            ] {
                assert_eq!(decide(disk, r, d, b), Action::Nothing);
            }
        }
        let ask = |choices: Vec<Choice>, default, confirm_load| {
            Action::Ask(Offer {
                choices,
                default,
                confirm_load,
            })
        };
        let changed = DiskCheck::Changed;
        // Only theirs changed: Load Theirs first, nothing of yours to lose.
        assert_eq!(
            decide(changed, true, false, true),
            ask(vec![LoadTheirs, KeepMine], LoadTheirs, false)
        );
        assert_eq!(
            decide(changed, true, false, false),
            ask(vec![LoadTheirs, KeepMine], LoadTheirs, false)
        );
        // Both changed: Merge first; Load Theirs asks again.
        assert_eq!(
            decide(changed, true, true, true),
            ask(vec![Merge, KeepMine, LoadTheirs], Merge, true)
        );
        // Both changed, no base kept: no merge.
        assert_eq!(
            decide(changed, true, true, false),
            ask(vec![KeepMine, LoadTheirs], KeepMine, true)
        );
        // Theirs isn't a schedule: only Keep Mine (or Later).
        assert_eq!(
            decide(changed, false, true, true),
            ask(vec![KeepMine], KeepMine, true)
        );
        assert_eq!(
            decide(changed, false, false, true),
            ask(vec![KeepMine], KeepMine, false)
        );
    }

    #[test]
    fn a_clean_window_loads_theirs_and_autosave_never_wrote_over_it() {
        let dir = temp_dir("outside-clean");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        assert!(check(&state, false).is_none(), "nothing changed");

        write_theirs(&path, |s| s.title = "Theirs".into());
        // Nothing unsaved here: autosave has nothing to write.
        assert_eq!(autosave(&state, &safety, later(), true), Autosaved::Nothing);
        let f = check(&state, false).expect("the change is found");
        assert!(!f.dto.dirty);
        assert_eq!(f.dto.offer.default, Choice::LoadTheirs);
        assert!(f.dto.problem.is_none() && f.theirs.is_some());
        assert!(state.lock().in_conflict(), "autosave into it is paused");

        let version: u64 = f.dto.version.parse().unwrap();
        // A version changed again since is refused.
        assert!(load_theirs(&state, &safety, version + 1)
            .unwrap_err()
            .contains("changed again"));
        let notice = load_theirs(&state, &safety, version).unwrap();
        assert!(!notice.contains('!'), "{notice}");
        {
            let s = state.lock();
            assert_eq!(s.schedule().title, "Theirs");
            assert!(!s.is_dirty() && !s.in_conflict());
        }
        assert!(check(&state, false).is_none(), "settled");
        // One undo step: Undo brings yours back, unsaved.
        assert!(state.lock().undo());
        assert!(state.lock().is_dirty());
        assert_ne!(state.lock().schedule().title, "Theirs");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keep_mine_backs_theirs_up_and_saves_over_it() {
        let dir = temp_dir("outside-keep");
        let data = dir.join("data");
        let safety = Safety::new(Some(&data));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        rename(&state, "Mine");
        write_theirs(&path, |s| s.title = "Theirs".into());
        // Autosave never writes over it, and the check finds both changed.
        assert!(matches!(
            autosave(&state, &safety, later(), false),
            Autosaved::Conflict(_)
        ));
        assert!(fs::read_to_string(&path).unwrap().contains("Theirs"));
        let f = check(&state, false).unwrap();
        assert!(f.dto.dirty);
        assert_eq!(f.dto.offer.default, Choice::Merge);
        assert!(f.dto.offer.confirm_load);

        // Later: not asked again about the same version, unless forced (Save).
        put_off(&state, f.dto.version.parse().unwrap());
        assert!(check(&state, false).is_none());
        assert!(state.lock().in_conflict(), "still paused");
        let f = check(&state, true).expect("Save asks again");

        let notice = keep_mine(&state, &safety, f.dto.version.parse().unwrap()).unwrap();
        assert!(notice.contains("Revert to Backup"), "{notice}");
        assert_eq!(read_schedule(&path).unwrap().0.title, "Mine");
        assert!(!state.lock().is_dirty() && !state.lock().in_conflict());
        // Theirs is the newest backup.
        let backups = safety.backups_of(&path);
        let newest = backups.last().unwrap();
        assert_eq!(read_schedule(newest).unwrap().0.title, "Theirs");
        // Autosave carries on into the file.
        rename(&state, "Mine 2");
        assert_eq!(autosave(&state, &safety, later(), false), Autosaved::File);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn merging_the_file_on_disk_uses_the_version_last_read_as_the_base() {
        let dir = temp_dir("outside-merge");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        state
            .lock()
            .edit(|x| {
                *x = method_schedule_core::sample::sample(20);
                Ok(())
            })
            .unwrap();
        save(&state, &safety, &path).unwrap();
        // Yours renames the schedule; theirs renames an element.
        rename(&state, "Mine");
        let el = state.lock().schedule().elements[1].id;
        write_theirs(&path, |s| s.rename_element(el, "THEIR NAME").unwrap());
        let f = check(&state, false).unwrap();
        assert_eq!(f.dto.offer.choices[0], Choice::Merge);
        let theirs = f.theirs.unwrap();
        assert!(theirs.name.ends_with("on disk"));
        let inputs = MergeInputs::gather(&state, &safety, theirs).unwrap();
        let p = inputs.preview(&BaseChoice::Auto, &BTreeMap::new());
        assert_eq!(p.base, "last_read", "{}", p.base_reason);
        assert!(p.problem.is_none() && p.conflicts.is_empty());
        assert!(
            p.bases.iter().all(|b| b.id != "disk"),
            "the file on disk is theirs, not a base"
        );
        assert_eq!(p.bases[0].id, "last_read");
        crate::diffmerge::apply(&state, &inputs, &BaseChoice::Auto, &BTreeMap::new()).unwrap();
        {
            let s = state.lock();
            assert_eq!(s.schedule().title, "Mine", "yours kept");
            assert!(
                s.schedule().elements.iter().any(|e| e.name == "THEIR NAME"),
                "theirs merged in"
            );
            assert!(s.is_dirty() && !s.in_conflict(), "settled, to be saved");
        }
        // Autosave now writes the merge into the file.
        assert_eq!(autosave(&state, &safety, later(), false), Autosaved::File);
        let (on_disk, _) = read_schedule(&path).unwrap();
        assert_eq!(on_disk.title, "Mine");
        assert!(on_disk.elements.iter().any(|e| e.name == "THEIR NAME"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_is_not_a_schedule_offers_only_keep_mine() {
        let dir = temp_dir("outside-bad");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        save(&state, &safety, &path).unwrap();
        fs::write(&path, "not a schedule").unwrap();
        let f = check(&state, false).unwrap();
        assert_eq!(f.dto.offer.choices, vec![Choice::KeepMine]);
        assert!(f.dto.problem.is_some() && f.theirs.is_none());
        let v: u64 = f.dto.version.parse().unwrap();
        assert!(load_theirs(&state, &safety, v).is_err());
        keep_mine(&state, &safety, v).unwrap();
        assert!(read_schedule(&path).is_ok());
        // A file gone is no outside change: a save writes it again.
        fs::remove_file(&path).unwrap();
        assert!(check(&state, true).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_changed_compares_with_a_chosen_backup() {
        // M4: Revert to Backup's list has What Changed, read through read_backup.
        let dir = temp_dir("wc-backup");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        let path = dir.join("film.mtsc");
        rename(&state, "One");
        save(&state, &safety, &path).unwrap();
        rename(&state, "Two");
        save(&state, &safety, &path).unwrap();
        let name = backups(&state, &safety).unwrap().backups[0].name.clone();
        let (older, label) = read_backup(&state, &safety, &name).unwrap();
        assert_eq!(older.title, "One");
        assert!(label.starts_with("Backup from "));
        let dto = crate::diffmerge::compare(
            &older,
            state.lock().schedule(),
            format!("the {}", lower_first(&label)),
            "film.mtsc as it is now".into(),
        );
        assert_eq!(dto.count, 1, "the title");
        assert!(dto.from.starts_with("the backup from "));
        assert!(read_backup(&state, &safety, "nope.mtsc").is_err());
    }

    #[test]
    fn revert_to_backup_is_one_undo_step_and_keeps_the_current_file() {
        let dir = temp_dir("revert");
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        assert!(backups(&state, &safety).is_err(), "never saved");
        let path = dir.join("film.mtsc");
        rename(&state, "One");
        save(&state, &safety, &path).unwrap();
        rename(&state, "Two");
        save(&state, &safety, &path).unwrap();
        rename(&state, "Three");
        save(&state, &safety, &path).unwrap();
        let list = backups(&state, &safety).unwrap();
        assert_eq!(list.file, "film.mtsc");
        assert_eq!(list.backups.len(), 2, "One and Two, newest first");
        assert!(list.backups[0].label.starts_with("Backup from "));
        assert_eq!(list.backups[0].changes, Some(1));
        let oldest = list.backups[1].name.clone();
        assert!(revert(&state, &safety, "nope.mtsc").is_err());
        let notice = revert(&state, &safety, &oldest).unwrap();
        assert!(notice.contains("Undo"), "{notice}");
        {
            let s = state.lock();
            assert_eq!(s.schedule().title, "One");
            assert!(s.is_dirty(), "reverted, to be saved");
        }
        // The file as it was (Three) went into the backups first.
        let newest = safety.backups_of(&path).last().cloned().unwrap();
        assert_eq!(read_schedule(&newest).unwrap().0.title, "Three");
        assert!(state.lock().undo());
        assert_eq!(state.lock().schedule().title, "Three");
        assert!(!state.lock().is_dirty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
