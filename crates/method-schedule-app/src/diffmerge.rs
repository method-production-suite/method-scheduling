//! File > What Changed and File > Merge Changes From (docs/MERGE.md; decisions.md,
//! entries 25 and 26). The engine does the work (`method_schedule_core::diff`, `method_schedule_core::merge`);
//! this gathers the schedules, picks a merge base and turns the results into what the
//! page draws (ui/js/diffmerge.js). After Method Budget's `src/diffmerge.rs`, scaled down
//! to one schedule per window and no versions.
//!
//! - **What Changed** lists what changed from another schedule file (picked in an Open
//!   dialog shown here) or from this schedule's own file as it is on disk, to the
//!   schedule in the window. Read-only.
//! - **Merge Changes From** merges another file ("theirs", picked in an Open dialog shown
//!   here and kept by token) into the schedule in the window ("ours"). The base is the
//!   newest of this file's rolling backups older than both versions, or, when that can't
//!   be told, the file as it is on disk now; the page may pick another backup or the file
//!   on disk, by name, from the list this module gives it. The preview lists every
//!   conflict and what the merge changes; picks come back as `{key: "ours" | "theirs"}`.
//!   Applying is one undo step through `Session::edit`, worked out on the schedule as it
//!   is then.
//!
//! Paths never come from the page: theirs is named by token, a backup by its file name,
//! checked against the backups folder's own listing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use method_schedule_core::diff::diff;
use method_schedule_core::merge::{merge_with, Merged, Side};
use method_schedule_core::Schedule;

use crate::app::{finish, AppState, CmdResult};
use crate::docs::Doc;
use crate::safety::{self, utc_stamp};
use crate::safety_app::Safety;
use crate::session::{self, read_schedule};
use crate::view::View;

/// Most picks a merge takes (one per conflict).
const MAX_PICKS: usize = 50_000;
/// Longest conflict key the page may send back.
const MAX_KEY: usize = 400;
/// Files picked for merging kept at hand (the newest).
const FILES_KEPT: usize = 8;

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// `2026-10-11 14:03 UTC`.
pub fn utc_minutes(t: SystemTime) -> String {
    let ms = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let stamp = utc_stamp(ms);
    format!("{} UTC", stamp[..16].replace('.', ":"))
}

fn modified(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

// ---------------------------------------------------------------------------
// What the page draws

pub use method_schedule_session::changes::*;

// ---------------------------------------------------------------------------
// Files picked for merging

/// A schedule file picked as theirs, read when picked.
#[derive(Clone, Debug)]
pub struct Theirs {
    pub name: String,
    pub path: PathBuf,
    pub schedule: Arc<Schedule>,
    pub modified: Option<SystemTime>,
    /// Set when theirs is this schedule's own file, changed outside the app
    /// (src/outside.rs): what it held when read. Merging it in settles the outside change.
    pub disk: Option<safety::DiskStamp>,
}

/// The files picked for merging in one window, by token (the newest few). Each window
/// keeps its own (`AppState::merge_files`), so a token never names a file another window
/// picked.
#[derive(Default)]
pub struct MergeFiles(Mutex<(u64, Vec<(u64, Theirs)>)>);

impl MergeFiles {
    fn lock(&self) -> MutexGuard<'_, (u64, Vec<(u64, Theirs)>)> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add(&self, t: Theirs) -> u64 {
        let mut g = self.lock();
        g.0 += 1;
        let token = g.0;
        g.1.push((token, t));
        let n = g.1.len();
        if n > FILES_KEPT {
            g.1.drain(..n - FILES_KEPT);
        }
        token
    }

    pub fn get(&self, token: u64) -> Result<Theirs, String> {
        self.lock()
            .1
            .iter()
            .find(|(k, _)| *k == token)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| "That file is no longer at hand; choose it again.".to_string())
    }
}

/// Reads `path` as theirs. Refused when it is the schedule's own file.
pub fn read_theirs(path: &Path, own: Option<&Path>) -> Result<Theirs, String> {
    if own.is_some_and(|o| safety::same_file(o, path)) {
        return Err(format!(
            "{} is this schedule's own file. Choose another copy to merge from, or use What \
             Changed Since Saved to compare with it.",
            file_name(path)
        ));
    }
    let (schedule, _) = read_schedule(path)?;
    Ok(Theirs {
        name: file_name(path),
        path: path.to_path_buf(),
        schedule: Arc::new(schedule),
        modified: modified(path),
        disk: None,
    })
}

// ---------------------------------------------------------------------------
// The base

/// Which base the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BaseChoice {
    /// The newest backup older than both versions, else the file on disk.
    Auto,
    /// The schedule's file as it is on disk now.
    Disk,
    /// One of the file's backups, by its file name in the backups folder.
    Backup { name: String },
    /// The version this window last read from or wrote to its file: the base when theirs
    /// is that file, changed outside the app (decisions.md, entry 180).
    LastRead,
    /// Any schedule file, picked with Merge Changes From's picker (`merge_pick_file`),
    /// by its token: the version both copies started from when it is kept elsewhere.
    File { token: u64 },
}

/// A backup, as listed in the backups folder.
#[derive(Clone, Debug)]
pub struct Backup {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
}

/// What the automatic choice found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutoBase {
    /// `backups[i]`, with why.
    Backup(usize, String),
    /// Unclear: the file on disk, with why.
    Disk(String),
}

/// The automatic base: the newest backup (in `backups`, oldest first) made before theirs
/// was last changed. Every backup predates the schedule in the window, which has been
/// saved over each of them since. When there is no such backup, or theirs' time can't be
/// read, it is unclear which version both started from, and the file on disk is offered
/// with why.
pub fn auto_base(
    file: &str,
    backups: &[Backup],
    theirs: &str,
    theirs_modified: Option<SystemTime>,
) -> AutoBase {
    let disk = format!(
        " The base is {file} as it is on disk now. Anything saved into it after {theirs} was \
         copied will read as undone in theirs, so check what will change, or pick a backup."
    );
    let Some(t) = theirs_modified else {
        return AutoBase::Disk(format!(
            "When {theirs} was last changed can't be read, so the backup both versions \
             started from can't be told.{disk}"
        ));
    };
    if backups.is_empty() {
        return AutoBase::Disk(format!(
            "{file} has no backups yet (one is kept each time a save replaces the file), so \
             the version both started from can't be found.{disk}"
        ));
    }
    let found = backups
        .iter()
        .enumerate()
        .rev()
        .find(|(_, b)| b.modified.is_some_and(|m| m <= t));
    match found {
        Some((i, b)) => AutoBase::Backup(
            i,
            format!(
                "The newest backup of {file} from before both versions: it was made {}, \
                 {theirs} was last changed after that ({}), and this schedule has been saved \
                 since.",
                b.modified.map(utc_minutes).unwrap_or_default(),
                utc_minutes(t)
            ),
        ),
        None => AutoBase::Disk(format!(
            "Every backup of {file} is newer than {theirs} (last changed {}), so none is sure \
             to come before both versions.{disk}",
            utc_minutes(t)
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BaseOptionDto {
    /// `"disk"`, or a backup's file name.
    pub id: String,
    pub label: String,
}

/// The base a merge uses, read.
struct Base {
    id: String,
    label: String,
    schedule: Schedule,
}

/// Everything a merge needs from the window, copied out under the lock.
pub struct MergeInputs {
    pub ours: Schedule,
    pub ours_name: String,
    pub path: PathBuf,
    pub backups: Vec<Backup>,
    pub theirs: Theirs,
    /// The version last read from or written to the file here.
    pub last_read: Option<Arc<Schedule>>,
    /// A schedule file picked as the base ([`BaseChoice::File`]), with its token.
    pub picked_base: Option<(u64, Theirs)>,
}

impl MergeInputs {
    /// Copies out the open schedule and lists its backups. Refused for a schedule never
    /// saved: it has neither backups nor a file to be the base.
    pub fn gather(state: &AppState, safety: &Safety, theirs: Theirs) -> Result<Self, String> {
        let (ours, path, ours_name, last_read) = {
            let s = state.lock();
            (
                s.schedule().clone(),
                s.path().map(PathBuf::from),
                s.display_name(),
                s.last_read(),
            )
        };
        let path = path.ok_or_else(not_saved)?;
        let backups = safety
            .backups_of(&path)
            .into_iter()
            .map(|p| Backup {
                modified: modified(&p),
                path: p,
            })
            .collect();
        Ok(MergeInputs {
            ours,
            ours_name,
            path,
            backups,
            theirs,
            last_read,
            picked_base: None,
        })
    }

    /// Takes the file `base` names as its base, when it names one (from the window's
    /// picked files).
    pub fn with_base(mut self, base: &BaseChoice, files: &MergeFiles) -> Result<Self, String> {
        if let BaseChoice::File { token } = base {
            self.picked_base = Some((*token, files.get(*token)?));
        }
        Ok(self)
    }

    /// Theirs is this schedule's own file, changed outside the app.
    pub fn outside(&self) -> bool {
        self.theirs.disk.is_some()
    }

    fn last_read_label(&self) -> String {
        format!("{} as last opened or saved here", self.ours_name)
    }

    fn options(&self) -> Vec<BaseOptionDto> {
        let mut out = self.listed_options();
        // A file picked as the base, so the picker can show it.
        if let Some((token, f)) = &self.picked_base {
            out.push(BaseOptionDto {
                id: format!("file:{token}"),
                label: f.name.clone(),
            });
        }
        out
    }

    fn listed_options(&self) -> Vec<BaseOptionDto> {
        let mut out: Vec<BaseOptionDto> = self
            .backups
            .iter()
            .rev()
            .map(|b| BaseOptionDto {
                id: file_name(&b.path),
                label: format!(
                    "Backup from {}",
                    b.modified.map(utc_minutes).unwrap_or_default()
                ),
            })
            .collect();
        if self.outside() {
            // The file on disk is theirs: as the base it would hide every change of theirs.
            if self.last_read.is_some() {
                out.insert(
                    0,
                    BaseOptionDto {
                        id: "last_read".into(),
                        label: self.last_read_label(),
                    },
                );
            }
            return out;
        }
        out.push(BaseOptionDto {
            id: "disk".into(),
            label: format!("{} on disk now", self.ours_name),
        });
        out
    }

    /// The base asked for, with why it is the one (empty for one the user picked) and
    /// whether the automatic choice was clear.
    fn base(&self, choice: &BaseChoice) -> (Result<Base, String>, String, bool) {
        let last_read = match choice {
            BaseChoice::LastRead => Some("You picked this version.".to_string()),
            BaseChoice::Auto if self.outside() && self.last_read.is_some() => Some(format!(
                "The version this window last read from or saved into {}: yours and the one \
                 changed outside the app both started from it.",
                self.ours_name
            )),
            _ => None,
        };
        if let BaseChoice::File { token } = choice {
            let base = match &self.picked_base {
                Some((t, f)) if t == token => Ok(Base {
                    id: format!("file:{token}"),
                    label: f.name.clone(),
                    schedule: (*f.schedule).clone(),
                }),
                _ => Err("That file is no longer picked; pick it again.".to_string()),
            };
            return (base, "You picked this file.".into(), true);
        }
        if let Some(why) = last_read {
            let base = self
                .last_read
                .as_ref()
                .map(|s| Base {
                    id: "last_read".into(),
                    label: self.last_read_label(),
                    schedule: (**s).clone(),
                })
                .ok_or_else(|| {
                    "This window hasn't read its file, so there is no such version.".to_string()
                });
            return (base, why, true);
        }
        let (pick, why, clear) = match choice {
            BaseChoice::Auto => match auto_base(
                &self.ours_name,
                &self.backups,
                &self.theirs.name,
                self.theirs.modified,
            ) {
                AutoBase::Backup(i, why) => (Some(i), why, true),
                AutoBase::Disk(why) => (None, why, false),
            },
            BaseChoice::Disk | BaseChoice::LastRead | BaseChoice::File { .. } => {
                (None, "You picked the file on disk.".into(), true)
            }
            BaseChoice::Backup { name } => {
                let Some(i) = self
                    .backups
                    .iter()
                    .position(|b| file_name(&b.path) == *name)
                else {
                    return (
                        Err("That backup is no longer in the backups folder.".into()),
                        String::new(),
                        true,
                    );
                };
                (Some(i), "You picked this backup.".into(), true)
            }
        };
        let base = match pick {
            Some(i) => {
                let b = &self.backups[i];
                read_schedule(&b.path).map(|(schedule, _)| Base {
                    id: file_name(&b.path),
                    label: format!(
                        "Backup from {}",
                        b.modified.map(utc_minutes).unwrap_or_default()
                    ),
                    schedule,
                })
            }
            None => read_schedule(&self.path).map(|(schedule, _)| Base {
                id: "disk".into(),
                label: format!("{} on disk now", self.ours_name),
                schedule,
            }),
        };
        (base, why, clear)
    }

    /// The preview: the base used and why, every conflict and what the merge changes.
    pub fn preview(&self, choice: &BaseChoice, picks: &BTreeMap<String, Side>) -> MergeDto {
        let (base, why, clear) = self.base(choice);
        let mut out = MergeDto {
            theirs: self.theirs.name.clone(),
            ours: self.ours_name.clone(),
            base: String::new(),
            base_label: String::new(),
            base_reason: why,
            base_clear: clear,
            bases: self.options(),
            conflicts: Vec::new(),
            count: 0,
            changes: Vec::new(),
            renumbered: 0,
            fallbacks: Vec::new(),
            problem: None,
        };
        let base = match base {
            Ok(b) => b,
            Err(e) => {
                out.problem = Some(format!("The base can't be read: {e}"));
                return out;
            }
        };
        let m = merge_with(
            &base.schedule,
            &self.ours,
            &self.theirs.schedule,
            Side::Ours,
            picks,
        );
        let d = diff(&self.ours, &m.schedule);
        out.base = base.id;
        out.base_label = base.label;
        out.conflicts = m.conflicts.iter().map(conflict_dto).collect();
        out.count = d.changes.len();
        out.changes = groups(&d);
        out.renumbered = m.renumbered;
        out.fallbacks = m.fallbacks;
        out
    }

    /// The base's schedule, for applying.
    fn base_schedule(&self, choice: &BaseChoice) -> Result<Schedule, String> {
        self.base(choice)
            .0
            .map(|b| b.schedule)
            .map_err(|e| format!("The base can't be read: {e}"))
    }
}

fn not_saved() -> String {
    "Save this schedule first: a merge needs the version both copies started from, and a \
     schedule never saved has no file or backups to find it in."
        .into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConflictDto {
    pub key: String,
    pub what: String,
    pub base: Option<String>,
    pub ours: String,
    pub theirs: String,
    /// `"ours"` or `"theirs"`.
    pub taken: &'static str,
    pub text: String,
}

fn conflict_dto(c: &method_schedule_core::merge::Conflict) -> ConflictDto {
    ConflictDto {
        key: c.key.clone(),
        what: c.what.clone(),
        base: c.base.clone(),
        ours: c.ours.clone(),
        theirs: c.theirs.clone(),
        taken: c.taken.key(),
        text: c.text.clone(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MergeDto {
    /// Theirs' file name, and ours' (the window's).
    pub theirs: String,
    pub ours: String,
    /// The base used (`"disk"` or a backup's file name) and its label; empty when it
    /// can't be read.
    pub base: String,
    pub base_label: String,
    /// Why this base, in plain words.
    pub base_reason: String,
    /// False when the automatic choice couldn't tell and fell back to the file on disk.
    pub base_clear: bool,
    /// What the page may pick instead: the backups, newest first, then the file on disk.
    pub bases: Vec<BaseOptionDto>,
    pub conflicts: Vec<ConflictDto>,
    /// What the merge changes in the schedule in the window, grouped.
    pub count: usize,
    pub changes: Vec<GroupDto>,
    /// Things of theirs given new ids.
    pub renumbered: usize,
    /// Where the merge fell back to ours so the result hangs together.
    pub fallbacks: Vec<String>,
    /// Why there is nothing to show (the base can't be read).
    pub problem: Option<String>,
}

/// The page's picks: conflict key to `"ours"` or `"theirs"`.
pub fn read_picks(picks: &BTreeMap<String, String>) -> Result<BTreeMap<String, Side>, String> {
    if picks.len() > MAX_PICKS {
        return Err("Too many choices.".into());
    }
    picks
        .iter()
        .map(|(k, v)| {
            if k.len() > MAX_KEY {
                return Err("That choice isn't one of the conflicts.".to_string());
            }
            let side = Side::from_key(v).ok_or_else(|| format!("\"{v}\" isn't ours or theirs."))?;
            Ok((k.clone(), side))
        })
        .collect()
}

/// Applies the merge to the open schedule as one undo step, worked out on the schedule as
/// it is now. Returns the notice to show.
pub fn apply(
    state: &AppState,
    inputs: &MergeInputs,
    choice: &BaseChoice,
    picks: &BTreeMap<String, Side>,
) -> Result<String, String> {
    let base = inputs.base_schedule(choice)?;
    let theirs = Arc::clone(&inputs.theirs.schedule);
    let mut s = state.lock();
    let before = s.schedule().clone();
    let m: Merged = s.edit(|x| {
        let m = merge_with(&base, x, &theirs, Side::Ours, picks);
        *x = m.schedule.clone();
        Ok(m)
    })?;
    if let Some(stamp) = inputs.theirs.disk {
        // Theirs is in now: the file's version is settled, and the merge is saved into it
        // as any change is (the guard still refuses a file changed again since).
        s.took_disk(stamp, Some(theirs));
    }
    Ok(merge_notice(&inputs.theirs.name, &m, before == m.schedule))
}

fn merge_notice(theirs: &str, m: &Merged, unchanged: bool) -> String {
    if unchanged {
        return format!("Nothing to merge: this schedule already has what it takes from {theirs}.");
    }
    let n = m.conflicts.len();
    let mut out = format!("Merged the changes from {theirs}.");
    if n > 0 {
        let t = m
            .conflicts
            .iter()
            .filter(|c| c.taken == Side::Theirs)
            .count();
        let word = if n == 1 { "conflict" } else { "conflicts" };
        out.push_str(&format!(
            " {n} {word}: {} kept yours, {t} took theirs.",
            n - t
        ));
    }
    if !m.fallbacks.is_empty() {
        let f = m.fallbacks.len();
        out.push_str(&format!(
            " {f} {} kept yours so the schedule hangs together.",
            if f == 1 { "part" } else { "parts" }
        ));
    }
    out.push_str(" Undo takes the whole merge back.");
    out
}

// ---------------------------------------------------------------------------
// Commands (registered in app.rs, build.rs and capabilities/main.json)

/// Shows an Open dialog for a schedule file. `None` when cancelled. Blocks: async
/// commands only.
fn pick<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let start = state
        .lock()
        .path()
        .and_then(Path::parent)
        .map(PathBuf::from);
    let mut dialog = window
        .dialog()
        .file()
        .set_title(title)
        .add_filter("Schedule file", &[session::EXTENSION, "json"])
        .set_parent(window);
    if let Some(dir) = start {
        dialog = dialog.set_directory(dir);
    }
    match dialog.blocking_pick_file() {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|e| e.to_string()),
    }
}

/// What changed from another schedule to the open one: `against` is `"file"` (an Open
/// dialog, shown here), `"saved"` (the schedule's own file as it is on disk) or
/// `"backup"` (backup `backup` of its file, by its name in the backups folder, from Revert
/// to Backup's list). `None` when the dialog was cancelled.
#[tauri::command]
pub async fn what_changed<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    against: String,
    backup: Option<String>,
) -> CmdResult<Option<WhatChangedDto>> {
    let (path, name) = {
        let s = state.lock();
        (s.path().map(PathBuf::from), s.display_name())
    };
    let (older, from) = match against.as_str() {
        "file" => {
            let Some(p) = pick(&window, &state, "What changed since another schedule")? else {
                return Ok(None);
            };
            (read_schedule(&p)?.0, file_name(&p))
        }
        "saved" => {
            let p = path.ok_or("This schedule hasn't been saved to a file yet.")?;
            (read_schedule(&p)?.0, format!("{name} as saved on disk"))
        }
        "backup" => {
            let b = backup.ok_or("Pick a backup to compare with.")?;
            let (older, label) =
                crate::outside::read_backup(&state, &window.state::<Safety>(), &b)?;
            (
                older,
                format!("the {}", crate::outside::lower_first(&label)),
            )
        }
        _ => return Err("Unknown comparison.".into()),
    };
    let newer = state.lock().schedule().clone();
    Ok(Some(compare(
        &older,
        &newer,
        from,
        format!("{name} as it is now"),
    )))
}

#[derive(Clone, Debug, Serialize)]
pub struct PickedDto {
    pub token: u64,
    pub name: String,
}

/// Shows an Open dialog for the file to merge changes from and reads it. `None` when
/// cancelled. Refused for a schedule never saved, and for the schedule's own file.
#[tauri::command]
pub async fn merge_pick_file<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
) -> CmdResult<Option<PickedDto>> {
    let own = state.lock().path().map(PathBuf::from);
    let own = own.ok_or_else(not_saved)?;
    let Some(p) = pick(&window, &state, "Merge changes from")? else {
        return Ok(None);
    };
    let t = read_theirs(&p, Some(&own))?;
    let name = t.name.clone();
    Ok(Some(PickedDto {
        token: state.merge_files.add(t),
        name,
    }))
}

/// The merge of file `token` into the open schedule against `base`, with `picks` for its
/// conflicts: a preview, nothing changes.
#[tauri::command]
pub fn merge_preview<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    token: u64,
    base: BaseChoice,
    picks: Option<BTreeMap<String, String>>,
) -> CmdResult<MergeDto> {
    let picks = read_picks(&picks.unwrap_or_default())?;
    let inputs = MergeInputs::gather(
        &state,
        &window.state::<Safety>(),
        state.merge_files.get(token)?,
    )?
    .with_base(&base, &state.merge_files)?;
    Ok(inputs.preview(&base, &picks))
}

/// Merges file `token` into the open schedule against `base` with `picks`, as one undo
/// step. The view carries a notice saying what happened.
#[tauri::command]
pub fn merge_apply<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    token: u64,
    base: BaseChoice,
    picks: Option<BTreeMap<String, String>>,
) -> CmdResult<View> {
    let picks = read_picks(&picks.unwrap_or_default())?;
    let inputs = MergeInputs::gather(
        &state,
        &window.state::<Safety>(),
        state.merge_files.get(token)?,
    )?
    .with_base(&base, &state.merge_files)?;
    if inputs.outside() {
        // Theirs will be saved over: it goes into the backups first.
        window.state::<Safety>().back_up_file(&inputs.path);
    }
    let notice = apply(&state, &inputs, &base, &picks)?;
    let s = state.lock();
    finish(&window, &s, Some(notice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::tests::temp_dir;
    use crate::safety_app;
    use method_schedule_core::model::ElementId;
    use std::time::Duration;

    fn rename_element(s: &mut Schedule, id: ElementId, name: &str) {
        s.rename_element(id, name).unwrap();
    }

    #[test]
    fn what_changed_groups_the_engines_sentences() {
        let a = method_schedule_core::sample::sample(20);
        let mut b = a.clone();
        b.title = "The Long Haul II".into();
        let id = b.elements[0].id;
        rename_element(&mut b, id, "NEW NAME");
        let props = b.category_named("Props").unwrap();
        b.rename_category(props, "Hand Props").unwrap();
        let d = compare(&a, &b, "old.mtsc".into(), "new.mtsc as it is now".into());
        assert_eq!(d.count, 3);
        let heads: Vec<_> = d.groups.iter().map(|g| g.group).collect();
        assert_eq!(heads, ["Schedule", "Categories", "Elements"]);
        assert!(d.groups[2].changes[0].text.contains("\"NEW NAME\""));
        assert_eq!(d.groups[2].changes[0].kind, "changed");
        assert!(compare(&a, &a, String::new(), String::new())
            .groups
            .is_empty());
    }

    #[test]
    fn the_automatic_base_is_the_newest_backup_older_than_theirs() {
        let t0 = UNIX_EPOCH + Duration::from_secs(1_790_000_000);
        let at = |m: u64| Some(t0 + Duration::from_secs(m * 60));
        let b = |m: u64| Backup {
            path: PathBuf::from(format!("/b/{m}.mtsc")),
            modified: at(m),
        };
        let backups = [b(10), b(20), b(30)];
        match auto_base("film.mtsc", &backups, "copy.mtsc", at(25)) {
            AutoBase::Backup(i, why) => {
                assert_eq!(i, 1);
                assert!(why.starts_with("The newest backup of film.mtsc"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            auto_base("film.mtsc", &backups, "copy.mtsc", at(99)),
            AutoBase::Backup(2, _)
        ));
        // Unclear: offered the file on disk, saying why.
        for (list, t) in [
            (&backups[..], at(5)),
            (&[][..], at(25)),
            (&backups[..], None),
        ] {
            let AutoBase::Disk(why) = auto_base("film.mtsc", list, "copy.mtsc", t) else {
                panic!("expected the file on disk");
            };
            assert!(why.contains("film.mtsc as it is on disk now"), "{why}");
            assert!(!why.contains('\u{2014}') && !why.contains('!'), "{why}");
        }
    }

    /// A saved schedule with one backup (the version both copies start from), a copy
    /// edited elsewhere, and the window's schedule edited too.
    fn setup(name: &str) -> (PathBuf, AppState, Safety, Theirs, ElementId) {
        let dir = temp_dir(name);
        let safety = Safety::new(Some(&dir.join("data")));
        let state = AppState::default();
        state
            .lock()
            .edit(|s| {
                *s = method_schedule_core::sample::sample(20);
                Ok(())
            })
            .unwrap();
        let path = dir.join("film.mtsc");
        safety_app::save(&state, &safety, &path).unwrap();
        // The copy is made from the saved file, then edited and written elsewhere.
        let mut copy = state.lock().schedule().clone();
        let el = copy.elements[0].id;
        let first = copy.elements[1].id;
        rename_element(&mut copy, first, "THEIR NAME");
        copy.element_mut(el).unwrap().rules.min_drop_gap = 3;
        let copy_path = dir.join("copy.mtsc");
        // Ours: a save after the copy was made backs up the version the copy came from.
        state
            .lock()
            .edit(|s| {
                s.title = "Ours".into();
                s.element_mut(el).unwrap().rules.min_drop_gap = 5;
                Ok(())
            })
            .unwrap();
        safety_app::save(&state, &safety, &path).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&copy_path, method_schedule_format::to_string(&copy)).unwrap();
        let theirs = read_theirs(&copy_path, Some(&path)).unwrap();
        (dir, state, safety, theirs, el)
    }

    #[test]
    fn merging_finds_the_backup_previews_and_applies_picks_as_one_step() {
        let (dir, state, safety, theirs, el) = setup("merge-auto");
        assert_eq!(safety.backups_of(&dir.join("film.mtsc")).len(), 1);
        let inputs = MergeInputs::gather(&state, &safety, theirs.clone()).unwrap();
        let p = inputs.preview(&BaseChoice::Auto, &BTreeMap::new());
        assert!(p.base_clear, "{}", p.base_reason);
        assert!(p.base_label.starts_with("Backup from "), "{}", p.base_label);
        assert_eq!(p.bases.len(), 2, "the backup, then the file on disk");
        assert_eq!(p.bases[1].id, "disk");
        assert!(p.problem.is_none());
        // The drop gap changed both ways: one conflict, ours by default.
        assert_eq!(p.conflicts.len(), 1, "{:?}", p.conflicts);
        let c = &p.conflicts[0];
        assert_eq!(
            (c.ours.as_str(), c.theirs.as_str(), c.taken),
            ("5 days", "3 days", "ours")
        );
        // Theirs' rename comes in; ours' title stays.
        assert_eq!(p.count, 1, "{:?}", p.changes);
        assert!(p.changes[0].changes[0].text.contains("THEIR NAME"));

        // Pick theirs for the conflict and apply: one undo step.
        let picks = BTreeMap::from([(c.key.clone(), Side::Theirs)]);
        let p2 = inputs.preview(&BaseChoice::Auto, &picks);
        assert_eq!(p2.conflicts[0].taken, "theirs");
        assert_eq!(p2.count, 2);
        let notice = apply(&state, &inputs, &BaseChoice::Auto, &picks).unwrap();
        assert!(
            notice.contains("1 conflict: 0 kept yours, 1 took theirs"),
            "{notice}"
        );
        {
            let s = state.lock();
            assert_eq!(s.schedule().title, "Ours");
            assert_eq!(s.schedule().element(el).unwrap().rules.min_drop_gap, 3);
            assert!(s.schedule().check().is_ok());
        }
        assert!(state.lock().undo());
        assert_eq!(
            state
                .lock()
                .schedule()
                .element(el)
                .unwrap()
                .rules
                .min_drop_gap,
            5,
            "one undo takes the whole merge back"
        );
        // Merging again after applying changes nothing more.
        assert!(state.lock().redo());
        let again = MergeInputs::gather(&state, &safety, theirs).unwrap();
        let n = apply(&state, &again, &BaseChoice::Auto, &picks).unwrap();
        assert!(n.starts_with("Nothing to merge"), "{n}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn any_schedule_file_can_be_the_base() {
        // M4: a file picked with Merge Changes From's picker, by its token.
        let (dir, state, safety, theirs, _) = setup("merge-file-base");
        let files = MergeFiles::default();
        let token = files.add(theirs.clone());
        let choice = BaseChoice::File { token };
        let inputs = MergeInputs::gather(&state, &safety, theirs.clone())
            .unwrap()
            .with_base(&choice, &files)
            .unwrap();
        // Theirs as its own base: nothing of theirs is a change, so nothing conflicts.
        let p = inputs.preview(&choice, &BTreeMap::new());
        assert_eq!(p.base, format!("file:{token}"));
        assert_eq!(p.base_reason, "You picked this file.");
        assert!(p.conflicts.is_empty(), "{:?}", p.conflicts);
        assert!(
            p.bases.iter().any(|b| b.id == format!("file:{token}")),
            "listed to show it"
        );
        // A token never picked is refused, before anything is read.
        assert!(MergeInputs::gather(&state, &safety, theirs)
            .unwrap()
            .with_base(&BaseChoice::File { token: token + 99 }, &files)
            .is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn picking_the_base_and_refusals() {
        let (dir, state, safety, theirs, _) = setup("merge-pick");
        let inputs = MergeInputs::gather(&state, &safety, theirs).unwrap();
        // The file on disk as the base: ours equals it, so theirs' changes all come in and
        // the drop gap is no conflict.
        let p = inputs.preview(&BaseChoice::Disk, &BTreeMap::new());
        assert_eq!(p.base, "disk");
        assert!(p.conflicts.is_empty(), "{:?}", p.conflicts);
        // A backup by name; a name that isn't listed is refused.
        let name = p.bases[0].id.clone();
        let q = inputs.preview(&BaseChoice::Backup { name: name.clone() }, &BTreeMap::new());
        assert_eq!(q.base, name);
        let r = inputs.preview(
            &BaseChoice::Backup {
                name: "../../etc/passwd".into(),
            },
            &BTreeMap::new(),
        );
        assert!(r
            .problem
            .unwrap()
            .contains("no longer in the backups folder"));
        // Bad picks, the schedule's own file, a schedule never saved.
        let bad = BTreeMap::from([("k".to_string(), "mine".to_string())]);
        assert!(read_picks(&bad).is_err());
        let own = dir.join("film.mtsc");
        assert!(read_theirs(&own, Some(&own))
            .unwrap_err()
            .contains("own file"));
        let fresh = AppState::default();
        let t = read_theirs(&dir.join("copy.mtsc"), None).unwrap();
        let err = MergeInputs::gather(&fresh, &safety, t).err().unwrap();
        assert!(err.starts_with("Save this schedule first"), "{err}");
        let files = MergeFiles::default();
        assert!(files.get(1).is_err());
        // A token names a file picked in its own window only.
        let (a, b) = (AppState::default(), AppState::default());
        let token = a
            .merge_files
            .add(read_theirs(&dir.join("copy.mtsc"), None).unwrap());
        assert!(a.merge_files.get(token).is_ok());
        assert!(b.merge_files.get(token).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn times_and_sentences_follow_the_voice() {
        assert_eq!(
            utc_minutes(UNIX_EPOCH + Duration::from_secs(86_400 + 3_600 + 120)),
            "1970-01-02 01:02 UTC"
        );
        let m = Merged {
            schedule: Schedule::new("x"),
            conflicts: Vec::new(),
            renumbered: 0,
            fallbacks: vec!["a".into()],
        };
        for text in [
            merge_notice("copy.mtsc", &m, false),
            merge_notice("copy.mtsc", &m, true),
            not_saved(),
        ] {
            assert!(!text.contains('\u{2014}') && !text.contains('!'), "{text}");
        }
    }
}
