//! The desktop's half of an open schedule: reading and writing its file and checking the
//! file for outside changes (decisions 179 to 182). The session itself, with undo and
//! redo, is `method-schedule-session`'s, re-exported here under its old name.

use std::path::Path;

use method_schedule_core::{fountain, Schedule};

use crate::safety::{self, DiskCheck, DiskStamp, StampAt};

pub use method_schedule_session::session::*;

/// What only the desktop does with a [`Session`]: its file.
pub trait SessionFiles: Sized {
    /// Reads a schedule file and starts a fresh session on it. A template (decisions.md,
    /// entry 85) starts a new, untitled schedule instead, so the template stays as it is.
    fn open(path: &Path) -> Result<Self, String>;

    /// Writes the schedule to `path` atomically, then remembers the path and marks the
    /// schedule saved.
    ///
    /// The outside-change guard: before writing over the file this session read or last
    /// wrote, the file is read and hashed, and the write is refused when it holds anything
    /// else (another program changed it). A file that is gone is written again. Any other
    /// path is new to this session; the Save dialog has already asked about replacing it.
    fn save_to(&mut self, path: &Path) -> Result<(), SaveError>;

    /// The check when the window gets focus ([`StampAt::quick_check`]): `Same` without
    /// a file or a stamp. A file only touched (a new time, the same content) gets its new
    /// time stamped, so the next check needs no read.
    fn disk_quick_check(&mut self) -> DiskCheck;

    /// Whether the schedule's file holds something other than what this session last read
    /// or wrote (another program changed it). Gone, or never read here, is not a change.
    fn changed_on_disk(&self) -> bool;
}

impl SessionFiles for Session {
    fn open(path: &Path) -> Result<Session, String> {
        let (schedule, bytes) = read_schedule(path)?;
        if std::str::from_utf8(&bytes).is_ok_and(method_schedule_format::is_template) {
            return Ok(crate::templates::session_from(&schedule));
        }
        Ok(Session::opened(
            schedule,
            path.to_path_buf(),
            DiskStamp::at(&bytes, path),
        ))
    }

    fn save_to(&mut self, path: &Path) -> Result<(), SaveError> {
        if let (Some(current), Some(stamp)) = (self.path(), self.disk_stamp()) {
            if safety::same_file(current, path) && stamp.check(path) == DiskCheck::Changed {
                return Err(SaveError::ChangedOnDisk(path.to_path_buf()));
            }
        }
        let text = self.file_text();
        safety::write_atomically(path, text.as_bytes(), safety::Access::Document).map_err(|e| {
            SaveError::Io(format!(
                "Couldn't save {}: {}",
                file_name(path),
                safety::write_problem(&e)
            ))
        })?;
        self.mark_saved(path.to_path_buf(), DiskStamp::at(text.as_bytes(), path));
        Ok(())
    }

    fn disk_quick_check(&mut self) -> DiskCheck {
        let (Some(path), Some(stamp)) = (self.path(), self.disk_stamp()) else {
            return DiskCheck::Same;
        };
        let out = stamp.quick_check(path);
        if out == DiskCheck::Same {
            let now = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            if now != stamp.modified {
                self.restamp(DiskStamp {
                    modified: now,
                    ..stamp
                });
            }
        }
        out
    }

    fn changed_on_disk(&self) -> bool {
        match (self.path(), self.disk_stamp()) {
            (Some(path), Some(stamp)) => stamp.check(path) == DiskCheck::Changed,
            _ => false,
        }
    }
}

/// Reads a schedule file strictly (size-limited, UTF-8, the format's reader and
/// `Schedule::check`), with the bytes it held. Errors start with the file's name.
pub fn read_schedule(path: &Path) -> Result<(Schedule, Vec<u8>), String> {
    let bytes = safety::read_limited(path).map_err(|e| format!("{}: {e}", file_name(path)))?;
    let schedule = parse_schedule(&bytes, path)?;
    Ok((schedule, bytes))
}

/// Reads a screenplay: Final Draft (`.fdx`) by its extension, Fountain otherwise, at the
/// default lines per page.
pub fn read_script(path: &Path) -> Result<fountain::ImportedScript, String> {
    read_script_with(path, fountain::LINES_PER_PAGE)
}

/// Reads a screenplay as [`read_script`] does, a Fountain one at `lines_per_page` lines a
/// page (the user's setting, decision 195). A Final Draft file keeps its own lengths.
pub fn read_script_with(
    path: &Path,
    lines_per_page: u32,
) -> Result<fountain::ImportedScript, String> {
    let bytes = safety::read_limited(path).map_err(|e| format!("{}: {e}", file_name(path)))?;
    parse_script(bytes, path, lines_per_page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::tests::temp_dir;
    use std::fs;

    #[allow(dead_code)]
    fn harbor_light() -> fountain::ImportedScript {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        read_script(&path).unwrap()
    }
    #[test]
    fn save_open_round_trip_and_dirty_flag() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("harbor.mtsc");
        let mut s = Session::new();
        let added = s.import_script(&harbor_light()).unwrap();
        assert!(added > 3);
        assert_eq!(
            s.schedule().title,
            "Harbor Light",
            "untouched: takes the title"
        );
        assert!(s.is_dirty());
        s.save_to(&path).unwrap();
        assert!(!s.is_dirty());
        assert_eq!(s.display_name(), "harbor.mtsc");
        // Undo after saving is an unsaved change; redo returns to the saved state.
        s.undo();
        assert!(s.is_dirty());
        s.redo();
        assert!(!s.is_dirty());

        let opened = Session::open(&path).unwrap();
        assert_eq!(opened.schedule(), s.schedule());
        assert!(!opened.is_dirty() && !opened.can_undo());
        // A second import adds to the schedule and keeps its title.
        let mut again = opened;
        again.import_script(&harbor_light()).unwrap();
        assert_eq!(again.schedule().title, "Harbor Light");
        assert_eq!(again.schedule().sheets.len(), 2 * added);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_changed_on_disk_is_not_overwritten() {
        let dir = temp_dir("guard");
        let path = dir.join("guard.mtsc");
        let mut s = Session::new();
        s.save_to(&path).unwrap();
        s.edit(|x| {
            x.title = "Mine".into();
            Ok(())
        })
        .unwrap();
        // Saving over our own last write is fine.
        s.save_to(&path).unwrap();

        // Another program writes the file.
        let mut theirs = Session::open(&path).unwrap();
        theirs
            .edit(|x| {
                x.title = "Theirs".into();
                Ok(())
            })
            .unwrap();
        let their_text = method_schedule_format::to_string(theirs.schedule());
        fs::write(&path, &their_text).unwrap();

        s.edit(|x| {
            x.title = "Mine again".into();
            Ok(())
        })
        .unwrap();
        let err = s.save_to(&path).unwrap_err();
        assert!(matches!(err, SaveError::ChangedOnDisk(_)));
        assert!(err.to_string().contains("Save As"), "{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), their_text, "untouched");
        assert!(s.is_dirty(), "still unsaved");

        // Save As to another file works, and that file is guarded from then on.
        let other = dir.join("mine.mtsc");
        s.save_to(&other).unwrap();
        assert!(!s.is_dirty());
        assert_eq!(
            Session::open(&other).unwrap().schedule().title,
            "Mine again"
        );

        // A file that was deleted is simply written again.
        fs::remove_file(&other).unwrap();
        s.save_to(&other).unwrap();
        assert!(other.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn opening_refuses_what_isnt_a_schedule() {
        let dir = temp_dir("refuse");
        let path = dir.join("not.mtsc");
        fs::write(&path, "{\"hello\": 1}").unwrap();
        let err = Session::open(&path).err().unwrap();
        assert!(err.starts_with("not.mtsc: "), "{err}");
        assert!(Session::open(&dir.join("missing.mtsc")).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scripts_are_read_by_extension() {
        let dir = temp_dir("scripts");
        let fdx = dir.join("tiny.FDX");
        fs::write(
            &fdx,
            "<?xml version=\"1.0\"?><FinalDraft><Content>\
             <Paragraph Type=\"Scene Heading\"><Text>INT. LOFT - NIGHT</Text></Paragraph>\
             </Content></FinalDraft>",
        )
        .unwrap();
        let script = read_script(&fdx).unwrap();
        assert_eq!(script.scenes.len(), 1);
        assert_eq!(script.scenes[0].set, "LOFT");
        // Not XML: refused as a Final Draft file, with its name.
        fs::write(&fdx, "INT. LOFT - NIGHT").unwrap();
        assert!(read_script(&fdx).unwrap_err().starts_with("tiny.FDX: "));
        fs::remove_dir_all(&dir).unwrap();
    }
}
