//! Crash recovery as tests (decisions.md, entry 200): what docs/APP.md "Saving and
//! recovery" promises, checked against the saving code itself rather than by hand. A
//! process killed mid-edit (a real `kill -9` of a child process running the same code),
//! a disk that fills up in the middle of a save, a locked or read-only file or folder,
//! and a file changed outside the app followed by a crash. In every case the file on disk
//! is the old one or the complete new one, and the work is in a recovery copy the next
//! run offers and reopens.
//!
//! What stays a hand check: killing the real app window (its timer, its menus and the
//! Recovered schedules dialog drawn), in scripts/crash-check.sh.

use std::io::{self, Write};

/// A writer standing in for a disk with `room` bytes left: it takes that much, then fails
/// as a full disk does (`ENOSPC` on Unix). Used by `safety::fill` while
/// [`crate::safety::DISK_FULL_IN`] names the folder being written.
pub(crate) struct FullDisk<'a, W: Write> {
    inner: &'a mut W,
    room: usize,
}

impl<'a, W: Write> FullDisk<'a, W> {
    pub(crate) fn new(inner: &'a mut W, room: usize) -> FullDisk<'a, W> {
        FullDisk { inner, room }
    }
}

fn disk_full() -> io::Error {
    #[cfg(unix)]
    return io::Error::from_raw_os_error(28);
    #[cfg(not(unix))]
    io::Error::from(io::ErrorKind::StorageFull)
}

impl<W: Write> Write for FullDisk<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.room == 0 {
            return Err(disk_full());
        }
        let n = self.inner.write(&buf[..buf.len().min(self.room)])?;
        self.room -= n;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use crate::app::AppState;
    use crate::safety::tests::temp_dir;
    use crate::safety::{write_atomically, Access, DISK_FULL_IN};
    use crate::safety_app::{autosave, open_recovery, save, Autosaved, Safety};

    /// The environment variable that turns [`crash_child`] on, naming its folder.
    const CHILD_DIR: &str = "METHOD_SCHEDULE_CRASH_DIR";

    fn rename(state: &AppState, title: &str) {
        state
            .lock()
            .edit(|x| {
                x.title = title.into();
                Ok(())
            })
            .unwrap();
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// Writes into `dir` fail after `room` bytes until the guard goes.
    struct DiskFull;
    impl DiskFull {
        fn in_folder(dir: &Path, room: usize) -> DiskFull {
            DISK_FULL_IN.with(|d| *d.borrow_mut() = Some((dir.to_path_buf(), room)));
            DiskFull
        }
    }
    impl Drop for DiskFull {
        fn drop(&mut self) {
            DISK_FULL_IN.with(|d| *d.borrow_mut() = None);
        }
    }

    /// The other half of [`a_process_killed_mid_edit_leaves_copies_the_next_run_reopens`]:
    /// run in a child process, it edits two schedules (one never saved, one saved with
    /// Autosave off), lets autosave keep them in recovery copies, says it is ready and
    /// waits to be killed. Does nothing in a normal test run.
    #[test]
    #[ignore]
    fn crash_child() {
        let Some(dir) = std::env::var_os(CHILD_DIR).map(PathBuf::from) else {
            return;
        };
        let safety = Safety::new(Some(&dir.join("data")));
        safety.set_autosave(false);
        let untitled = AppState::default();
        rename(&untitled, "Never saved");
        let filed = AppState::default();
        save(&filed, &safety, &dir.join("film.mtsc")).unwrap();
        rename(&filed, "Edited, not saved");
        for state in [&untitled, &filed] {
            assert_eq!(
                autosave(
                    state,
                    &safety,
                    method_schedule_session::clock::mono_ms(),
                    true
                ),
                Autosaved::Recovery
            );
        }
        // Mid-edit: one more change not yet in any copy when the kill comes.
        rename(&filed, "Lost with the crash");
        fs::write(dir.join("ready"), b"").unwrap();
        let until = Instant::now() + Duration::from_secs(60);
        while Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::process::exit(3);
    }

    #[test]
    fn a_process_killed_mid_edit_leaves_copies_the_next_run_reopens() {
        let dir = temp_dir("crash-kill");
        let data = dir.join("data");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "safety_crash::tests::crash_child",
                "--exact",
                "--ignored",
                "--test-threads=1",
            ])
            .env(CHILD_DIR, &dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(60);
        while !dir.join("ready").exists() {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("the child ended early: {status}");
            }
            assert!(Instant::now() < until, "the child never got ready");
            std::thread::sleep(Duration::from_millis(20));
        }
        // SIGKILL on Unix (TerminateProcess on Windows): no destructors, no exit hook.
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        let recovery = data.join("Recovery");
        assert_eq!(
            files_in(&recovery)
                .iter()
                .filter(|n| n.ends_with(".lock"))
                .count(),
            1,
            "a killed run leaves its lock file, unlocked"
        );
        let on_disk = fs::read_to_string(dir.join("film.mtsc")).unwrap();

        // The next run: tidies the dead lock away and offers both copies.
        let safety = Safety::new(Some(&data));
        assert_eq!(
            files_in(&recovery)
                .iter()
                .filter(|n| n.ends_with(".lock"))
                .count(),
            1,
            "only the new run's"
        );
        let list = safety.recovery_list();
        let mut titles: Vec<&str> = list.iter().map(|e| e.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, ["Edited, not saved", "Never saved"]);
        for entry in &list {
            let state = AppState::default();
            open_recovery(&state, &safety, &entry.key).unwrap();
            let s = state.lock();
            assert!(s.is_dirty(), "reopens unsaved");
            assert_eq!(s.schedule().title, entry.title);
            if entry.name == "film.mtsc" {
                assert_eq!(s.path(), Some(dir.join("film.mtsc").as_path()));
                assert!(s.needs_overwrite_confirm(), "the first Save asks");
                assert!(!s.can_autosave_to_file());
            } else {
                assert_eq!(entry.name, "Untitled");
                assert!(s.path().is_none());
            }
        }
        // Autosave was off: the file is as it was saved, never half written.
        assert_eq!(fs::read_to_string(dir.join("film.mtsc")).unwrap(), on_disk);
        assert!(method_schedule_format::from_str(&on_disk).is_ok());
        drop(safety);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_full_disk_mid_save_leaves_the_old_file_whole() {
        let dir = temp_dir("crash-full");
        let docs = dir.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let data = dir.join("data");
        let path = docs.join("film.mtsc");

        // The writer itself: the disk fills after a few bytes, or before any.
        for room in [0, 7, 4096] {
            write_atomically(&path, b"the old version", Access::Document).unwrap();
            let _full = DiskFull::in_folder(&docs, room);
            let big = vec![b'x'; 10_000];
            let e = write_atomically(&path, &big, Access::Document).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::StorageFull, "{e}");
            assert_eq!(fs::read(&path).unwrap(), b"the old version");
            assert_eq!(files_in(&docs), ["film.mtsc"], "no temporary file left");
        }

        // A save, then autosave, on a full disk.
        let state = AppState::default();
        {
            let safety = Safety::new(Some(&data));
            save(&state, &safety, &path).unwrap();
            let saved = fs::read_to_string(&path).unwrap();
            rename(&state, "Written on a full disk");
            let full = DiskFull::in_folder(&docs, 100);
            let msg = save(&state, &safety, &path).unwrap_err();
            assert!(
                msg.starts_with("Couldn't save film.mtsc: the disk is full."),
                "{msg}"
            );
            assert!(msg.contains("Save As") && !msg.contains('!'), "{msg}");
            assert_eq!(fs::read_to_string(&path).unwrap(), saved);
            assert!(state.lock().is_dirty(), "still unsaved");
            let Autosaved::Failed(said) = autosave(
                &state,
                &safety,
                method_schedule_session::clock::mono_ms(),
                true,
            ) else {
                panic!("expected a failed autosave");
            };
            assert!(said.contains("the disk is full"), "{said}");
            assert!(
                state.lock().has_recovery(),
                "the work is in a recovery copy"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), saved);
            assert_eq!(files_in(&docs), ["film.mtsc"]);
            drop(full);
            // The app dies here, without a clean close.
        }
        let safety = Safety::new(Some(&data));
        let list = safety.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Written on a full disk");
        let next = AppState::default();
        open_recovery(&next, &safety, &list[0].key).unwrap();
        // With room again, the save goes through and the copy goes.
        save(&next, &safety, &path).unwrap();
        assert!(!next.lock().has_recovery());
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("Written on a full disk"));
        drop(safety);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    fn set_mode(p: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_locked_or_read_only_file_is_refused_and_the_work_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("crash-locked");
        let docs = dir.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let data = dir.join("data");
        let path = docs.join("film.mtsc");
        let state = AppState::default();
        {
            let safety = Safety::new(Some(&data));
            save(&state, &safety, &path).unwrap();
            let saved = fs::read_to_string(&path).unwrap();
            set_mode(&path, 0o444);
            assert!(crate::safety::is_locked(&path));
            rename(&state, "Mine, kept");
            let msg = save(&state, &safety, &path).unwrap_err();
            assert_eq!(
                msg,
                "Couldn't save film.mtsc: the file or its folder is locked or read-only. \
                 Nothing was written. Use Save As to keep your changes in another file."
            );
            let Autosaved::Failed(said) = autosave(
                &state,
                &safety,
                method_schedule_session::clock::mono_ms(),
                true,
            ) else {
                panic!("expected a failed autosave");
            };
            assert!(said.contains("locked or read-only"), "{said}");
            assert!(state.lock().has_recovery());
            assert_eq!(fs::read_to_string(&path).unwrap(), saved, "untouched");
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o444,
                "still read-only"
            );

            // A folder that can't be written to: Save As there is refused in the same words.
            let shut = dir.join("shut");
            fs::create_dir_all(&shut).unwrap();
            set_mode(&shut, 0o555);
            let msg = save(&state, &safety, &shut.join("copy.mtsc")).unwrap_err();
            assert!(msg.contains("locked or read-only"), "{msg}");
            assert!(files_in(&shut).is_empty());
            set_mode(&shut, 0o755);

            // Save As somewhere else works, and autosave carries on there.
            let elsewhere = docs.join("film copy.mtsc");
            save(&state, &safety, &elsewhere).unwrap();
            assert!(!state.lock().has_recovery());
            assert!(state.lock().can_autosave_to_file());
        }

        // The copy outlives a crash too: a read-only file, an edit, the app dies.
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            crate::safety_app::open_path(&state, &safety, &path).unwrap();
            rename(&state, "After the crash");
            assert!(matches!(
                autosave(
                    &state,
                    &safety,
                    method_schedule_session::clock::mono_ms(),
                    true
                ),
                Autosaved::Failed(_)
            ));
        }
        let safety = Safety::new(Some(&data));
        let list = safety.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "After the crash");
        let next = AppState::default();
        open_recovery(&next, &safety, &list[0].key).unwrap();
        assert_eq!(next.lock().schedule().title, "After the crash");
        drop(safety);

        // Locked in the Finder (the user immutable flag) counts as locked as well.
        #[cfg(target_os = "macos")]
        {
            set_mode(&path, 0o644);
            let locked = std::process::Command::new("chflags")
                .arg("uchg")
                .arg(&path)
                .status()
                .is_ok_and(|s| s.success());
            if locked {
                assert!(crate::safety::is_locked(&path));
                assert!(write_atomically(&path, b"x", Access::Document).is_err());
                let _ = std::process::Command::new("chflags")
                    .arg("nouchg")
                    .arg(&path)
                    .status();
                assert!(!crate::safety::is_locked(&path));
            }
        }
        set_mode(&path, 0o644);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_changed_outside_then_a_crash_keeps_both_versions() {
        // Decisions 179 to 182: autosave never writes over a file changed elsewhere. Here
        // the app dies before the user decides; both versions survive.
        let dir = temp_dir("crash-outside");
        let data = dir.join("data");
        let path = dir.join("film.mtsc");
        {
            let safety = Safety::new(Some(&data));
            let state = AppState::default();
            save(&state, &safety, &path).unwrap();
            fs::write(&path, "their version").unwrap();
            rename(&state, "Mine");
            assert!(matches!(
                autosave(
                    &state,
                    &safety,
                    method_schedule_session::clock::mono_ms(),
                    true
                ),
                Autosaved::Conflict(_)
            ));
            // The app dies with the question unanswered.
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), "their version");
        let safety = Safety::new(Some(&data));
        let list = safety.recovery_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "film.mtsc");
        let state = AppState::default();
        open_recovery(&state, &safety, &list[0].key).unwrap();
        {
            let s = state.lock();
            assert_eq!(s.schedule().title, "Mine");
            assert_eq!(s.path(), Some(path.as_path()));
            assert!(
                s.needs_overwrite_confirm(),
                "nothing written over theirs unasked"
            );
        }
        // Autosave leaves theirs alone; the copy is kept until a Save.
        assert_eq!(
            autosave(
                &state,
                &safety,
                method_schedule_session::clock::mono_ms(),
                true
            ),
            Autosaved::Nothing
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "their version");
        drop(safety);
        fs::remove_dir_all(&dir).unwrap();
    }
}
