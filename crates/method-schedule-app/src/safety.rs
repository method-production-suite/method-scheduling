//! Keeping work safe: atomic writes, the outside-change guard, autosave timing, recovery
//! copies, rolling backups, the app's settings and the recent files list.
//!
//! Ported from Method Budget's `src/safety.rs` (same owner, same license), scaled down to
//! one schedule per window. Everything here is plain files in a folder handed in by the
//! caller (the app's local data folder in the app, a temporary folder in tests), so it is
//! testable without a window. The Tauri side is `src/safety_app.rs`; the behavior is in
//! docs/APP.md ("Saving and recovery").
//!
//! The app's local data folder:
//!
//! ```text
//! Recovery/<key>.mtsc        the schedule as it was at the last autosave
//! Recovery/<key>.json         what it is: original path, title, time, owner
//! Recovery/owner-<id>.lock    held (locked) by each running copy, removed on a clean exit
//! Backups/<path hash>/        the last few versions of one file, as they were on disk
//!                             before each save replaced them, with source.json
//! recent.json                 File > Open Recent
//! settings.json               the Autosave switch
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub use method_schedule_session::safety::{
    autosave_due, content_hash, new_recovery_key, path_hash, valid_key, DiskCheck, DiskStamp,
    AUTOSAVE_MAX_WAIT, AUTOSAVE_QUIET,
};

/// Schedule and script files larger than this are refused before they are read, so a stray
/// huge file can't stall the app. The same limit as the command line.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// File > Open Recent shows this many files.
pub const RECENT_LIMIT: usize = 10;
/// Backups kept per file.
pub const BACKUPS_KEPT: usize = 5;
/// Autosaves into a file back up its previous version at most this often (an explicit
/// Save always does, and so does the first write into a file in a session).
pub const BACKUP_GAP: Duration = Duration::from_secs(10 * 60);
/// The extension recovery copies and backups are written with (the schedule file's).
const EXT: &str = "mtsc";

// ---------------------------------------------------------------------------
// Names

fn unique() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!(
        "{:x}-{nanos:x}-{:x}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `2026-10-09 14.03.22.123` (UTC) for backup file names: sorts in time order and is
/// legal in file names everywhere. Integers only.
pub fn utc_stamp(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}.{:02}.{:02}.{:03}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60,
        ms % 1000
    )
}

// ---------------------------------------------------------------------------
// Writing files

/// Who may read a file written by [`write_atomically`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Only this user (0600 on Unix): recovery copies, backups, settings, the recent list.
    Private,
    /// A schedule file: keeps the permissions of the file it replaces (a new file gets the
    /// usual ones).
    Document,
}

fn create_new(path: &Path, access: Access) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    if access == Access::Private {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = access;
    o.open(path)
}

/// Creates a folder only this user can open (0700 on Unix).
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)?;
    #[cfg(unix)]
    {
        // A folder that already existed is closed up too. Only ever called on the app's
        // own folders.
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Whether the file at `path` is locked against changes: read-only (no write permission
/// for anyone), or locked in the Finder (the user or system immutable flag, macOS). A file
/// that isn't there isn't locked.
pub fn is_locked(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        // UF_IMMUTABLE (the Finder's Locked) and SF_IMMUTABLE.
        if meta.st_flags() & (0x0000_0002 | 0x0002_0000) != 0 {
            return true;
        }
    }
    meta.is_file() && meta.permissions().readonly()
}

/// A write that failed, in the words a save message uses after "Couldn't save NAME: "
/// (decisions.md, entry 200). A full disk and a locked or read-only file or folder say
/// what happened and that the file is as it was; anything else is the system's own words.
pub fn write_problem(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => {
            "the disk is full. Nothing was written, so the file is as it was. Free some space, \
             or use Save As to save somewhere else."
                .to_string()
        }
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "the file or its folder is locked or read-only. Nothing was written. Use Save As \
             to keep your changes in another file."
                .to_string()
        }
        _ => e.to_string(),
    }
}

// Tests only: writes into this folder fail after this many bytes, as on a full disk
// (src/safety_crash.rs).
#[cfg(test)]
thread_local! {
    pub(crate) static DISK_FULL_IN: std::cell::RefCell<Option<(PathBuf, usize)>> =
        const { std::cell::RefCell::new(None) };
}

/// Writes `bytes` into the temporary file `f` beside `path`.
fn fill(f: &mut File, path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if let Some((dir, room)) = DISK_FULL_IN.with(|d| d.borrow().clone()) {
        if path.parent() == Some(dir.as_path()) {
            return crate::safety_crash::FullDisk::new(f, room).write_all(bytes);
        }
    }
    let _ = path;
    f.write_all(bytes)
}

/// Writes `bytes` to `path` so that `path` is always either the old file or the complete
/// new one: write a temporary file in the same folder, flush it to disk, then rename it
/// over `path` (and flush the folder, on Unix). A failed write leaves the old file alone
/// and no temporary file behind. A document keeps the permissions of the file it replaces,
/// and a document that is locked or read-only ([`is_locked`]) is refused before anything
/// is written: renaming over it would work on Unix, and the lock is the user's say.
pub fn write_atomically(path: &Path, bytes: &[u8], access: Access) -> io::Result<()> {
    if access == Access::Document && is_locked(path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the file is locked or read-only",
        ));
    }
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "schedule".into());
    let tmp = dir.join(format!(".{name}.{}.tmp", unique()));
    let result = (|| {
        let mut f = create_new(&tmp, access)?;
        fill(&mut f, path, bytes)?;
        f.sync_all()?;
        drop(f);
        if access == Access::Document {
            if let Ok(meta) = fs::metadata(path) {
                // Best effort: a file on a share may refuse.
                let _ = fs::set_permissions(&tmp, meta.permissions());
            }
        }
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return result;
    }
    #[cfg(unix)]
    if let Ok(d) = File::open(&dir) {
        // The rename itself reaches the disk. Best effort: some file systems can't sync a
        // folder.
        let _ = d.sync_all();
    }
    Ok(())
}

/// A hidden temporary name in `path`'s folder, ending in `.ext`, for a file another writer
/// fills (the system's print operation, writing a PDF) before [`rename_into_place`] puts
/// it at `path`. Together they make that write atomic too.
pub fn temp_beside(path: &Path, ext: &str) -> PathBuf {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    dir.join(format!(".{name}.{}.tmp.{ext}", unique()))
}

/// Renames the finished temporary file `tmp` over `path` and flushes the folder (on Unix),
/// so `path` is always either the old file or the complete new one. On failure the
/// temporary file is removed.
pub fn rename_into_place(tmp: &Path, path: &Path) -> io::Result<()> {
    if let Err(e) = fs::rename(tmp, path) {
        let _ = fs::remove_file(tmp);
        return Err(e);
    }
    #[cfg(unix)]
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Reads a file of at most [`MAX_FILE_BYTES`].
pub fn read_limited(path: &Path) -> io::Result<Vec<u8>> {
    let meta = fs::metadata(path)?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(io::Error::other("the file is larger than 64 MB"));
    }
    fs::read(path)
}

/// Whether two paths name the same file: the same path, or the same file once links and
/// `..` are resolved (only files that exist can be resolved).
pub fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (fs::canonicalize(a), fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
}

/// Makes and checks [`DiskStamp`]s against files: the desktop's half of the stamp, whose
/// data lives in the session crate.
pub trait StampAt {
    /// The stamp of `bytes`, just read from or written to `path`, with the file's time.
    fn at(bytes: &[u8], path: &Path) -> DiskStamp;
    /// Reads `path` and compares its content with this stamp. Always reads and hashes:
    /// a write is refused on what the file holds, not on its size or time.
    fn check(&self, path: &Path) -> DiskCheck;
    /// [`StampAt::check`] for the check when the window gets focus: a file whose size
    /// and modification time are both as stamped is taken as the same without reading
    /// it, a different size is a change without reading it, and anything else (a new
    /// time, no time to compare) is read and hashed. Never used to allow a write.
    fn quick_check(&self, path: &Path) -> DiskCheck;
}

impl StampAt for DiskStamp {
    fn at(bytes: &[u8], path: &Path) -> DiskStamp {
        DiskStamp {
            modified: fs::metadata(path).and_then(|m| m.modified()).ok(),
            ..DiskStamp::of(bytes)
        }
    }

    fn check(&self, path: &Path) -> DiskCheck {
        match fs::read(path) {
            Ok(bytes) if self.matches(&bytes) => DiskCheck::Same,
            Ok(_) => DiskCheck::Changed,
            Err(e) if e.kind() == io::ErrorKind::NotFound => DiskCheck::Missing,
            Err(_) => DiskCheck::Changed,
        }
    }

    fn quick_check(&self, path: &Path) -> DiskCheck {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return DiskCheck::Missing,
            Err(_) => return DiskCheck::Changed,
        };
        if meta.len() != self.len {
            return DiskCheck::Changed;
        }
        match (self.modified, meta.modified().ok()) {
            (Some(a), Some(b)) if a == b => DiskCheck::Same,
            _ => self.check(path),
        }
    }
}

// ---------------------------------------------------------------------------
// Recovery copies

/// The sidecar next to each recovery copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryMeta {
    pub key: String,
    /// Where the schedule lives on disk, `None` for one never saved.
    pub path: Option<String>,
    pub title: String,
    pub saved_at_ms: u64,
    /// The lock file of the copy of the app that wrote it.
    pub owner: String,
}

/// What [`RecoveryStore::list`] finds: a recovery copy left by a copy of the app that is
/// no longer running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecoveryEntry {
    pub key: String,
    /// The original file's name, or "Untitled". Never its folder: the page shows this.
    pub name: String,
    pub title: String,
    pub saved_at_ms: u64,
}

/// The recovery folder. Each running copy of the app holds an exclusive lock on its own
/// `owner-*.lock` file for as long as it runs, so a listing can tell a crashed copy's
/// files (lock free) from a running copy's (lock held). A clean exit ([`close`]) unlocks
/// and removes the lock file; a crash leaves an unlocked one, which the next start tidies
/// away. Only a lock some running copy actually holds counts as live: a lock file nobody
/// holds is a dead copy's.
///
/// [`close`]: RecoveryStore::close
#[derive(Debug)]
pub struct RecoveryStore {
    dir: PathBuf,
    owner: String,
    /// Held open (and locked) until [`RecoveryStore::close`] or the store is dropped.
    /// `None` once closed, or when locks aren't supported here.
    lock: Mutex<Option<File>>,
    /// Set by [`RecoveryStore::close`].
    closed: std::sync::atomic::AtomicBool,
}

impl Drop for RecoveryStore {
    fn drop(&mut self) {
        self.close();
    }
}

impl RecoveryStore {
    /// Opens (creating if needed) the recovery folder `dir` and takes this copy's lock.
    pub fn open(dir: &Path) -> io::Result<RecoveryStore> {
        create_private_dir(dir)?;
        let owner = format!("owner-{}.lock", unique());
        let file = create_new(&dir.join(&owner), Access::Private)?;
        // If locks aren't supported here (some network folders), recovery still works;
        // only the running-copy check is lost.
        let lock = file.try_lock().is_ok().then_some(file);
        Ok(RecoveryStore {
            dir: dir.to_path_buf(),
            owner,
            lock: Mutex::new(lock),
            closed: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// This copy is exiting cleanly: releases its lock and removes its lock file, so
    /// nothing of it is left in the folder. Recovery copies it wrote stay (they are offered
    /// at the next start, as a dead copy's). Safe to call more than once.
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let held = self.lock.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(f) = held {
            // Unlock before removing: Windows can't delete a file that is still open.
            let _ = f.unlock();
            drop(f);
        }
        let _ = remove_if_present(&self.dir.join(&self.owner));
    }

    /// This copy's lock file name.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    fn schedule_file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.{EXT}"))
    }

    fn meta_file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Writes (or replaces) the recovery copy for `key`: the schedule first, then its
    /// sidecar, each atomically, readable only by this user.
    pub fn write(&self, key: &str, text: &str, path: Option<&Path>, title: &str) -> io::Result<()> {
        if !valid_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bad recovery key",
            ));
        }
        write_atomically(&self.schedule_file(key), text.as_bytes(), Access::Private)?;
        let meta = RecoveryMeta {
            key: key.to_string(),
            path: path.and_then(Path::to_str).map(str::to_string),
            title: title.to_string(),
            saved_at_ms: now_ms(),
            owner: self.owner.clone(),
        };
        self.write_meta(&meta)
    }

    fn write_meta(&self, meta: &RecoveryMeta) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(meta).map_err(io::Error::other)?;
        write_atomically(&self.meta_file(&meta.key), &json, Access::Private)
    }

    /// Deletes the recovery copy for `key`, if there is one.
    pub fn remove(&self, key: &str) -> io::Result<()> {
        if !valid_key(key) {
            return Ok(());
        }
        // Sidecar first: a schedule file without one is never listed.
        remove_if_present(&self.meta_file(key))?;
        remove_if_present(&self.schedule_file(key))
    }

    fn read_meta(&self, key: &str) -> Option<RecoveryMeta> {
        let text = fs::read_to_string(self.meta_file(key)).ok()?;
        let meta: RecoveryMeta = serde_json::from_str(&text).ok()?;
        (meta.key == key).then_some(meta)
    }

    /// The schedule text and sidecar for `key`, if it is listed (left by a copy of the app
    /// that is no longer running).
    pub fn read(&self, key: &str) -> io::Result<(String, RecoveryMeta)> {
        let missing =
            || io::Error::new(io::ErrorKind::NotFound, "That recovered schedule is gone.");
        if !valid_key(key) || !self.list().iter().any(|e| e.key == key) {
            return Err(missing());
        }
        let meta = self.read_meta(key).ok_or_else(missing)?;
        let bytes = read_limited(&self.schedule_file(key))?;
        let text = String::from_utf8(bytes).map_err(|_| missing())?;
        Ok((text, meta))
    }

    /// Makes `key` this copy's own (after opening it), so another copy of the app starting
    /// now doesn't offer it too.
    pub fn adopt(&self, key: &str) -> io::Result<()> {
        let mut meta = self
            .read_meta(key)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        meta.owner = self.owner.clone();
        self.write_meta(&meta)
    }

    /// Is the copy of the app that holds `owner` still running? Only when some process
    /// holds the lock on its file right now. The check takes a *shared* lock, so two
    /// copies checking the same dead copy's file at once never see each other's check as
    /// a live owner (an owner holds its lock exclusively). A missing file, a name that
    /// isn't a lock file, or a lock that can't be tested all count as not running.
    fn owner_running(&self, owner: &str) -> bool {
        if owner == self.owner {
            return !self.closed.load(Ordering::SeqCst);
        }
        if !owner.starts_with("owner-") || !owner.ends_with(".lock") || owner.contains(['/', '\\'])
        {
            return false;
        }
        let Ok(f) = File::open(self.dir.join(owner)) else {
            return false;
        };
        matches!(f.try_lock_shared(), Err(fs::TryLockError::WouldBlock))
    }

    /// Recovery copies left by copies of the app that are no longer running, newest first.
    pub fn list(&self) -> Vec<RecoveryEntry> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out: Vec<RecoveryEntry> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let key = name.strip_suffix(".json")?;
                if !valid_key(key) {
                    return None;
                }
                let meta = self.read_meta(key)?;
                if self.owner_running(&meta.owner) || !self.schedule_file(key).is_file() {
                    return None;
                }
                let name = meta
                    .path
                    .as_deref()
                    .and_then(|p| Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into());
                Some(RecoveryEntry {
                    key: meta.key,
                    name,
                    title: meta.title,
                    saved_at_ms: meta.saved_at_ms,
                })
            })
            .collect();
        out.sort_by(|a, b| b.saved_at_ms.cmp(&a.saved_at_ms).then(a.key.cmp(&b.key)));
        out
    }

    /// Removes lock files of copies that are no longer running, and stray temporary
    /// files. Recovery copies themselves are only removed by the user's choice or a save.
    pub fn tidy(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for e in entries.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            let stale_owner =
                name.starts_with("owner-") && name.ends_with(".lock") && !self.owner_running(&name);
            let stray_tmp = name.starts_with('.')
                && name.ends_with(".tmp")
                && !name.contains(&format!(".{:x}-", std::process::id()));
            if stale_owner || stray_tmp {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Backups

/// The last [`BACKUPS_KEPT`] versions of each schedule file, as they were on disk before a
/// save replaced them: `Backups/<path hash>/<name> <time> UTC.mtsc`, with a `source.json`
/// naming the file.
#[derive(Debug)]
pub struct Backups {
    dir: PathBuf,
}

impl Backups {
    pub fn new(dir: &Path) -> Backups {
        Backups {
            dir: dir.to_path_buf(),
        }
    }

    /// The folder holding `document`'s backups.
    pub fn folder(&self, document: &Path) -> PathBuf {
        self.dir.join(path_hash(document))
    }

    /// `document`'s backups, oldest first.
    pub fn list(&self, document: &Path) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.folder(document)) else {
            return Vec::new();
        };
        let mut v: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == EXT))
            .collect();
        // Names end in a fixed-width UTC time, after the same file name.
        v.sort();
        v
    }

    /// Copies `document` as it is on disk now into its backups, before a save replaces
    /// it, and keeps only the newest [`BACKUPS_KEPT`]. Nothing to do when the file doesn't
    /// exist yet, when the newest backup is the same, or when the newest backup is younger
    /// than `min_gap`.
    pub fn back_up(
        &self,
        document: &Path,
        min_gap: Option<Duration>,
    ) -> io::Result<Option<PathBuf>> {
        let existing = self.list(document);
        if let (Some(gap), Some(newest)) = (min_gap, existing.last()) {
            let age = fs::metadata(newest)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok());
            if age.is_some_and(|a| a < gap) {
                return Ok(None);
            }
        }
        let bytes = match read_limited(document) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if let Some(newest) = existing.last() {
            if fs::read(newest).is_ok_and(|b| b == bytes) {
                return Ok(None);
            }
        }
        let folder = self.folder(document);
        create_private_dir(&folder)?;
        let source = serde_json::json!({ "path": document.to_string_lossy() });
        write_atomically(
            &folder.join("source.json"),
            source.to_string().as_bytes(),
            Access::Private,
        )?;
        let stem = document
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "schedule".into());
        let mut ms = now_ms();
        let name = |ms| format!("{stem} {} UTC.{EXT}", utc_stamp(ms));
        let mut target = folder.join(name(ms));
        while target.exists() {
            ms += 1;
            target = folder.join(name(ms));
        }
        write_atomically(&target, &bytes, Access::Private)?;
        let all = self.list(document);
        for old in all.iter().take(all.len().saturating_sub(BACKUPS_KEPT)) {
            let _ = fs::remove_file(old);
        }
        Ok(Some(target))
    }
}

// ---------------------------------------------------------------------------
// Settings

/// App-wide preferences, in `settings.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Save into the schedule's file a few seconds after each change (schedules with a
    /// file; others get a recovery copy). On unless turned off.
    pub autosave: bool,
    /// The week start, date format, calendar strip fields (decision 59) and Fountain lines
    /// per page (decision 195): the user's, never the schedule's.
    pub prefs: crate::reportsetup::Prefs,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            autosave: true,
            prefs: crate::reportsetup::Prefs::default(),
        }
    }
}

/// The most a small app file (`settings.json`, `recent.json`) may hold; a bigger one is
/// read as missing.
pub const MAX_SMALL_FILE: u64 = 256 * 1024;

/// The text of a small app file, or `None` when it is missing, unreadable or bigger than
/// [`MAX_SMALL_FILE`].
fn read_small(file: &Path) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    fs::File::open(file)
        .ok()?
        .take(MAX_SMALL_FILE + 1)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() as u64 <= MAX_SMALL_FILE).then_some(text)
}

impl Settings {
    /// Reads `file`; missing or unreadable means the defaults. Each setting is read on
    /// its own, so one that is wrong (by hand, or from a later version) falls back to its
    /// default and leaves the others as they were: Autosave stays off when it was off.
    pub fn load(file: &Path) -> Settings {
        read_small(file)
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .map(|v| Settings::from_value(&v))
            .unwrap_or_default()
    }

    fn from_value(v: &serde_json::Value) -> Settings {
        let mut out = Settings::default();
        if let Some(on) = v.get("autosave").and_then(serde_json::Value::as_bool) {
            out.autosave = on;
        }
        let Some(p) = v.get("prefs") else {
            return out;
        };
        let text = |k: &str| p.get(k).and_then(|x| x.as_str()).map(str::to_string);
        // Each preference is taken when it passes the same checks as one saved from
        // the app (`Prefs::checked`); else it stays at its default.
        let mut take = |set: &dyn Fn(&mut crate::reportsetup::Prefs)| {
            let mut next = out.prefs.clone();
            set(&mut next);
            if let Ok(ok) = next.checked() {
                out.prefs = ok;
            }
        };
        if let Some(w) = text("week_start") {
            take(&|x| x.week_start = w.clone());
        }
        if let Some(d) = text("date_format") {
            take(&|x| x.date_format = d.clone());
        }
        if let Some(list) = p.get("calendar_fields").and_then(|x| x.as_array()) {
            let fields: Option<Vec<String>> = list
                .iter()
                .map(|f| f.as_str().map(str::to_string))
                .collect();
            if let Some(fields) = fields {
                take(&|x| x.calendar_fields = fields.clone());
            }
        }
        if let Some(n) = p.get("fountain_lines").and_then(serde_json::Value::as_u64) {
            if let Ok(n) = u32::try_from(n) {
                take(&|x| x.fountain_lines = n);
            }
        }
        if let Some(t) = text("theme") {
            take(&|x| x.theme = t.clone());
        }
        out
    }

    pub fn save(&self, file: &Path) -> io::Result<()> {
        if let Some(dir) = file.parent() {
            create_private_dir(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomically(file, &bytes, Access::Private)
    }
}

// ---------------------------------------------------------------------------
// Recent files

#[derive(Default, Serialize, Deserialize)]
struct RecentJson {
    files: Vec<String>,
}

/// File > Open Recent: the last [`RECENT_LIMIT`] schedules opened or saved, newest first,
/// kept in `recent.json` (when there is a file to keep it in).
#[derive(Debug, Default)]
pub struct RecentFiles {
    file: Option<PathBuf>,
    list: Vec<PathBuf>,
}

impl RecentFiles {
    /// Reads the list from `file`; a missing or unreadable file is an empty list.
    pub fn load(file: Option<PathBuf>) -> RecentFiles {
        let list = file
            .as_deref()
            .and_then(read_small)
            .and_then(|t| serde_json::from_str::<RecentJson>(&t).ok())
            .unwrap_or_default()
            .files
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .take(RECENT_LIMIT)
            .collect();
        RecentFiles { file, list }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.list
    }

    /// Puts `path` at the top (once), keeping at most [`RECENT_LIMIT`].
    pub fn add(&mut self, path: &Path) -> io::Result<()> {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.list.retain(|p| *p != path);
        self.list.insert(0, path);
        self.list.truncate(RECENT_LIMIT);
        self.save()
    }

    /// Drops files that no longer exist. True if any were dropped.
    pub fn prune(&mut self) -> bool {
        let before = self.list.len();
        self.list.retain(|p| p.is_file());
        let changed = self.list.len() != before;
        if changed {
            let _ = self.save();
        }
        changed
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.list.clear();
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            create_private_dir(dir)?;
        }
        let json = RecentJson {
            files: self
                .list
                .iter()
                .filter_map(|p| p.to_str().map(str::to_string))
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&json).map_err(io::Error::other)?;
        write_atomically(file, &bytes, Access::Private)
    }

    /// Menu labels: each file's name, with its folder's name when two share a name. `&` is
    /// doubled (menus read a single one as a mnemonic).
    pub fn labels(&self) -> Vec<String> {
        let name = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned())
        };
        self.list
            .iter()
            .map(|p| {
                let n = name(p);
                let shared = self.list.iter().filter(|q| name(q) == n).count() > 1;
                let label = match p.parent().and_then(Path::file_name) {
                    Some(dir) if shared => format!("{n} ({})", dir.to_string_lossy()),
                    _ => n,
                };
                label.replace('&', "&&")
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh, empty folder under the system's temporary folder.
    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("method-schedule-app-{name}-{}", unique()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temporary_file() {
        let dir = temp_dir("atomic");
        let path = dir.join("a.mtsc");
        write_atomically(&path, b"one", Access::Document).unwrap();
        write_atomically(&path, b"two", Access::Document).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap()).collect();
        assert_eq!(names.len(), 1, "only the file itself is left");
        // A folder that doesn't exist fails and leaves nothing behind.
        assert!(write_atomically(&dir.join("missing/b.mtsc"), b"x", Access::Document).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_filled_elsewhere_lands_whole() {
        let dir = temp_dir("beside");
        let path = dir.join("Report.pdf");
        fs::write(&path, b"old").unwrap();
        let tmp = temp_beside(&path, "pdf");
        assert_eq!(tmp.parent(), Some(dir.as_path()));
        let name = tmp.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(".Report.pdf.") && name.ends_with(".tmp.pdf"),
            "{name}"
        );
        assert_ne!(temp_beside(&path, "pdf"), tmp, "each name is new");
        fs::write(&tmp, b"new").unwrap();
        rename_into_place(&tmp, &path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(!tmp.exists());
        // A rename that fails leaves the old file and no temporary file.
        let tmp = temp_beside(&path, "pdf");
        fs::write(&tmp, b"newer").unwrap();
        assert!(rename_into_place(&tmp, &dir.join("missing/Report.pdf")).is_err());
        assert!(!tmp.exists());
        assert_eq!(fs::read(&path).unwrap(), b"new");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stamps_tell_versions_apart() {
        let dir = temp_dir("stamp");
        let path = dir.join("a.mtsc");
        fs::write(&path, b"version one").unwrap();
        let stamp = DiskStamp::of(b"version one");
        assert_eq!(stamp.check(&path), DiskCheck::Same);
        fs::write(&path, b"version two").unwrap();
        assert_eq!(stamp.check(&path), DiskCheck::Changed);
        fs::remove_file(&path).unwrap();
        assert_eq!(stamp.check(&path), DiskCheck::Missing);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_quick_check_trusts_size_and_time_and_reads_otherwise() {
        let dir = temp_dir("stamp-quick");
        let path = dir.join("a.mtsc");
        fs::write(&path, b"version one").unwrap();
        let stamp = DiskStamp::at(b"version one", &path);
        assert!(stamp.modified.is_some());
        assert_eq!(stamp.quick_check(&path), DiskCheck::Same);
        // Another size: a change, without reading.
        fs::write(&path, b"version three").unwrap();
        assert_eq!(stamp.quick_check(&path), DiskCheck::Changed);
        // The same size and a new time: read and hashed.
        fs::write(&path, b"version two").unwrap();
        let later = SystemTime::now() + Duration::from_secs(5);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(stamp.quick_check(&path), DiskCheck::Changed);
        // Only touched (a new time, the same content): the same.
        fs::write(&path, b"version one").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(stamp.quick_check(&path), DiskCheck::Same);
        fs::remove_file(&path).unwrap();
        assert_eq!(stamp.quick_check(&path), DiskCheck::Missing);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hash_is_stable() {
        // FNV-1a test vectors: stable across runs and Rust versions.
        assert_eq!(content_hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(content_hash(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn private_files_and_documents_keep_their_permissions() {
        let dir = temp_dir("perms");
        let path = dir.join("film.mtsc");
        write_atomically(&path, b"one", Access::Document).unwrap();
        // A failed write (the target is a folder) leaves no temporary file.
        let folder = dir.join("taken");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("x"), b"x").unwrap();
        assert!(write_atomically(&folder, b"two", Access::Document).is_err());
        assert_eq!(names(&dir), ["film.mtsc", "taken"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let private = dir.join("private.json");
            write_atomically(&private, b"{}", Access::Private).unwrap();
            assert_eq!(
                fs::metadata(&private).unwrap().permissions().mode() & 0o777,
                0o600
            );
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            write_atomically(&path, b"three", Access::Document).unwrap();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keys_and_names() {
        let a = new_recovery_key(Some(Path::new("/x/film.mtsc")));
        let b = new_recovery_key(Some(Path::new("/x/film.mtsc")));
        assert_ne!(a, b, "every open schedule gets its own key");
        assert!(a.starts_with(&path_hash(Path::new("/x/film.mtsc"))));
        assert!(new_recovery_key(None).starts_with("untitled-"));
        assert!(valid_key(&a) && valid_key(&new_recovery_key(None)));
        for bad in ["", "../x", "a/b", "A", "x.json", "a\\b", &"a".repeat(97)] {
            assert!(!valid_key(bad), "{bad}");
        }
        assert_ne!(
            path_hash(Path::new("/x/a.mtsc")),
            path_hash(Path::new("/x/b.mtsc"))
        );
        assert_eq!(utc_stamp(0), "1970-01-01 00.00.00.000");
        assert_eq!(utc_stamp(1_791_554_602_123), "2026-10-09 14.03.22.123");
        assert_eq!(utc_stamp(951_782_400_000), "2000-02-29 00.00.00.000");
    }

    #[test]
    fn recovery_copies_are_listed_only_after_their_writer_is_gone() {
        let dir = temp_dir("recovery");
        let running = RecoveryStore::open(&dir).unwrap();
        let k1 = new_recovery_key(Some(Path::new("/films/a.mtsc")));
        let k2 = new_recovery_key(None);
        running
            .write(&k1, "{a}", Some(Path::new("/films/a.mtsc")), "Film A")
            .unwrap();
        running.write(&k2, "{b}", None, "Untitled feature").unwrap();
        running
            .write(&k1, "{a2}", Some(Path::new("/films/a.mtsc")), "Film A")
            .unwrap();

        // Another copy starting now doesn't offer a running copy's files.
        let other = RecoveryStore::open(&dir).unwrap();
        assert!(other.list().is_empty());
        assert!(other.read(&k1).is_err());
        assert!(running.list().is_empty(), "nor its own");

        // The writer stops (a crash): now they are offered, newest first.
        drop(running);
        let listed = other.list();
        assert_eq!(listed.len(), 2);
        let a = listed.iter().find(|e| e.key == k1).unwrap();
        assert_eq!((a.name.as_str(), a.title.as_str()), ("a.mtsc", "Film A"));
        let b = listed.iter().find(|e| e.key == k2).unwrap();
        assert_eq!(b.name, "Untitled");
        assert!(listed[0].saved_at_ms >= listed[1].saved_at_ms);
        let (text, meta) = other.read(&k1).unwrap();
        assert_eq!(text, "{a2}");
        assert_eq!(meta.path.as_deref(), Some("/films/a.mtsc"));

        // Opening one makes it the opener's: a third copy doesn't see it.
        other.adopt(&k1).unwrap();
        let third = RecoveryStore::open(&dir).unwrap();
        assert_eq!(
            third.list().iter().map(|e| &e.key).collect::<Vec<_>>(),
            [&k2]
        );

        // Discarding removes both files; removing twice is fine.
        third.remove(&k2).unwrap();
        third.remove(&k2).unwrap();
        assert!(third.list().is_empty());
        assert!(!dir.join(format!("{k2}.mtsc")).exists());

        // Tidying removes the dead copy's lock and keeps the living ones'.
        third.tidy();
        let locks = names(&dir)
            .into_iter()
            .filter(|n| n.ends_with(".lock"))
            .count();
        assert_eq!(locks, 2, "{:?}", names(&dir));

        // Junk is ignored, and keys from outside are checked.
        fs::write(dir.join("not-json.json"), "nope").unwrap();
        assert!(third.list().is_empty());
        assert!(third.read("../escape").is_err());
        assert!(third.write("../escape", "x", None, "x").is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let f = dir.join(format!("{k1}.mtsc"));
            assert_eq!(fs::metadata(f).unwrap().permissions().mode() & 0o777, 0o600);
        }
        drop((other, third));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stale_lock_is_never_taken_for_a_running_copy() {
        let dir = temp_dir("stale-lock");
        let store = RecoveryStore::open(&dir).unwrap();
        // A copy that died: its lock file is still there, but nobody holds it.
        let dead = "owner-dead-1.lock";
        fs::write(dir.join(dead), b"").unwrap();
        let key = new_recovery_key(None);
        fs::write(dir.join(format!("{key}.mtsc")), "{x}").unwrap();
        let meta = RecoveryMeta {
            key: key.clone(),
            path: None,
            title: "Left behind".into(),
            saved_at_ms: 1,
            owner: dead.into(),
        };
        fs::write(
            dir.join(format!("{key}.json")),
            serde_json::to_vec(&meta).unwrap(),
        )
        .unwrap();
        assert_eq!(store.list().len(), 1, "a dead copy's work is offered");

        // Another copy checking the same file at this moment holds a shared lock on it;
        // that must not make the dead copy look alive.
        let checker = File::open(dir.join(dead)).unwrap();
        checker.try_lock_shared().unwrap();
        assert_eq!(store.list().len(), 1);
        drop(checker);

        // A live copy's exclusive lock does count, and only while it is held.
        let live = RecoveryStore::open(&dir).unwrap();
        let mut meta2 = meta.clone();
        meta2.key = new_recovery_key(None);
        meta2.owner = live.owner().to_string();
        fs::write(dir.join(format!("{}.mtsc", meta2.key)), "{y}").unwrap();
        fs::write(
            dir.join(format!("{}.json", meta2.key)),
            serde_json::to_vec(&meta2).unwrap(),
        )
        .unwrap();
        assert_eq!(store.list().len(), 1, "the live copy's work is not offered");
        live.close();
        assert_eq!(store.list().len(), 2, "closed: now it is");

        // Names that aren't lock files never count as running.
        for bad in ["owner-x.json", "../owner-x.lock", "other.lock"] {
            assert!(!store.owner_running(bad), "{bad}");
        }

        // Tidying removes the dead lock and keeps this copy's own.
        store.tidy();
        let locks: Vec<String> = names(&dir)
            .into_iter()
            .filter(|n| n.ends_with(".lock"))
            .collect();
        assert_eq!(locks, [store.owner().to_string()]);
        store.close();
        assert!(!names(&dir).iter().any(|n| n.ends_with(".lock")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn backups_keep_the_last_few_distinct_versions() {
        let dir = temp_dir("backups");
        let doc = dir.join("work").join("film.mtsc");
        let backups = Backups::new(&dir.join("Backups"));
        // Nothing on disk yet: nothing to back up.
        assert_eq!(backups.back_up(&doc, None).unwrap(), None);
        fs::create_dir_all(doc.parent().unwrap()).unwrap();
        for i in 0..8 {
            fs::write(&doc, format!("version {i}")).unwrap();
            assert!(backups.back_up(&doc, None).unwrap().is_some());
            // Saving again without changes adds nothing.
            assert_eq!(backups.back_up(&doc, None).unwrap(), None);
        }
        let kept: Vec<String> = backups
            .list(&doc)
            .iter()
            .map(|p| fs::read_to_string(p).unwrap())
            .collect();
        let want: Vec<String> = (3..8).map(|i| format!("version {i}")).collect();
        assert_eq!(kept, want);
        let first = backups.list(&doc)[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            first.starts_with("film ") && first.ends_with(" UTC.mtsc"),
            "{first}"
        );
        let source = fs::read_to_string(backups.folder(&doc).join("source.json")).unwrap();
        assert!(source.contains("film.mtsc"));
        // Only schedule files count as backups.
        fs::write(backups.folder(&doc).join("notes.txt"), "x").unwrap();
        assert_eq!(backups.list(&doc).len(), BACKUPS_KEPT);

        // Autosaves back up only when the newest backup is old enough.
        fs::write(&doc, "version 8").unwrap();
        assert_eq!(backups.back_up(&doc, Some(BACKUP_GAP)).unwrap(), None);
        assert!(backups
            .back_up(&doc, Some(Duration::ZERO))
            .unwrap()
            .is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn settings_default_to_autosave_on_and_are_kept() {
        let dir = temp_dir("settings");
        let file = dir.join("data").join("settings.json");
        assert!(Settings::load(&file).autosave);
        Settings {
            autosave: false,
            ..Settings::default()
        }
        .save(&file)
        .unwrap();
        assert!(!Settings::load(&file).autosave);
        fs::write(&file, "{}").unwrap();
        assert!(
            Settings::load(&file).autosave,
            "missing fields are defaults"
        );
        fs::write(&file, "garbage").unwrap();
        assert!(Settings::load(&file).autosave);

        // One bad setting leaves the others: Autosave stays off.
        fs::write(
            &file,
            r#"{"autosave": false, "prefs": {"week_start": "tuesday", "date_format": "us",
                "calendar_fields": ["scene", "nonsense"]}}"#,
        )
        .unwrap();
        let s = Settings::load(&file);
        assert!(!s.autosave);
        assert_eq!(s.prefs.week_start, "sun", "bad: the default");
        assert_eq!(s.prefs.date_format, "us", "good: kept");
        assert_eq!(
            s.prefs.calendar_fields,
            crate::reportsetup::Prefs::default().calendar_fields
        );
        fs::write(
            &file,
            r#"{"autosave": "no", "prefs": {"week_start": "mon"}}"#,
        )
        .unwrap();
        let s = Settings::load(&file);
        assert!(s.autosave, "not a switch: the default");
        assert_eq!(s.prefs.week_start, "mon");
        // Fountain lines per page (decision 195): 56 unless set, 40 to 70.
        assert_eq!(s.prefs.fountain_lines, 56);
        for (n, read) in [("55", 55), ("40", 40), ("70", 70), ("39", 56), ("71", 56)] {
            fs::write(&file, format!(r#"{{"prefs": {{"fountain_lines": {n}}}}}"#)).unwrap();
            assert_eq!(Settings::load(&file).prefs.fountain_lines, read, "{n}");
        }
        fs::write(&file, r#"{"prefs": {"fountain_lines": "55"}}"#).unwrap();
        assert_eq!(Settings::load(&file).prefs.fountain_lines, 56);
        // The look (decision 208): one the app has, else the default.
        assert_eq!(Settings::load(&file).prefs.theme, "paper");
        fs::write(&file, r#"{"prefs": {"theme": "ledger"}}"#).unwrap();
        assert_eq!(Settings::load(&file).prefs.theme, "ledger");
        fs::write(&file, r#"{"prefs": {"theme": "neon"}}"#).unwrap();
        assert_eq!(Settings::load(&file).prefs.theme, "paper");
        fs::write(&file, r#"{"autosave": false, "prefs": 7}"#).unwrap();
        assert!(!Settings::load(&file).autosave);

        // Too big to be ours: read as missing.
        let mut huge = String::from(r#"{"autosave": false, "pad": ""#);
        huge.push_str(&"x".repeat(MAX_SMALL_FILE as usize));
        huge.push_str(r#""}"#);
        fs::write(&file, huge).unwrap();
        assert!(Settings::load(&file).autosave);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recent_files_are_ordered_deduped_capped_pruned_and_kept() {
        let dir = temp_dir("recent");
        let store = dir.join("recent.json");
        let file = |n: &str| {
            let p = dir.join(n);
            fs::write(&p, "{}").unwrap();
            fs::canonicalize(p).unwrap()
        };
        let mut r = RecentFiles::load(Some(store.clone()));
        assert!(r.paths().is_empty());
        let a = file("a.mtsc");
        let b = file("b.mtsc");
        r.add(&a).unwrap();
        r.add(&b).unwrap();
        r.add(&a).unwrap();
        assert_eq!(r.paths(), [a.clone(), b.clone()], "newest first, once each");
        assert_eq!(RecentFiles::load(Some(store.clone())).paths(), r.paths());
        for i in 0..12 {
            r.add(&file(&format!("f{i:02}.mtsc"))).unwrap();
        }
        assert_eq!(r.paths().len(), RECENT_LIMIT);
        assert!(!r.paths().contains(&a));
        fs::remove_file(dir.join("f11.mtsc")).unwrap();
        assert!(r.prune());
        assert!(!r.prune());
        assert_eq!(
            RecentFiles::load(Some(store.clone())).paths().len(),
            RECENT_LIMIT - 1
        );
        r.clear().unwrap();
        assert!(RecentFiles::load(Some(store.clone())).paths().is_empty());
        fs::write(&store, "not json").unwrap();
        assert!(RecentFiles::load(Some(store)).paths().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recent_labels_tell_same_named_files_apart() {
        let r = RecentFiles {
            file: None,
            list: vec![
                PathBuf::from("/films/One/board.mtsc"),
                PathBuf::from("/films/Two/board.mtsc"),
                PathBuf::from("/films/Rock & Roll.mtsc"),
            ],
        };
        assert_eq!(
            r.labels(),
            ["board.mtsc (One)", "board.mtsc (Two)", "Rock && Roll.mtsc"]
        );
    }
}
