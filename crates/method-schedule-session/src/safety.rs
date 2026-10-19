//! The part of keeping files safe that needs no file system (decisions 179 to 182): what a
//! file's content was when the session last read or wrote it, the autosave timing rule,
//! and recovery keys. Reading, writing and comparing files on disk are the desktop's
//! (`method-schedule-app`'s `safety`), as in Method Budget.

use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::clock;

/// Quiet time after the last change before autosave writes.
pub const AUTOSAVE_QUIET: Duration = Duration::from_secs(3);
/// Longest autosave waits while changes keep coming.
pub const AUTOSAVE_MAX_WAIT: Duration = Duration::from_secs(30);

fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Whether an autosave is due: quiet for [`AUTOSAVE_QUIET`] since `last_change`, or
/// [`AUTOSAVE_MAX_WAIT`] since `pending_since`, the first change not yet written anywhere.
/// All three are milliseconds on [`clock::mono_ms`].
pub fn autosave_due(now: u64, last_change: u64, pending_since: Option<u64>) -> bool {
    let waited = |t: u64| now.saturating_sub(t);
    waited(last_change) >= ms(AUTOSAVE_QUIET)
        || pending_since.is_some_and(|t| waited(t) >= ms(AUTOSAVE_MAX_WAIT))
}

/// A 64-bit FNV-1a hash of file content: enough to tell versions apart.
pub fn content_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A stable 64-bit FNV-1a hash of a path, as 16 hex digits. Stable across runs and Rust
/// versions (std's hasher is not), so a file's backups stay in one folder.
pub fn path_hash(path: &Path) -> String {
    format!(
        "{:016x}",
        content_hash(path.as_os_str().to_string_lossy().as_bytes())
    )
}

/// A fresh recovery key for an open schedule: unique, so two copies of the app (or an old
/// crash's copy of the same file) never write over each other. It starts with the path's
/// hash, or `untitled`.
pub fn new_recovery_key(path: Option<&Path>) -> String {
    let base = path.map(path_hash).unwrap_or_else(|| "untitled".into());
    format!("{base}-{}", clock::unique())
}

/// Keys name files in the recovery folder, and the web view hands them back (to open or
/// discard an entry), so only plain names are accepted.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 96
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The content of a file as the app last read or wrote it, to notice another program
/// (a sync service, another computer, another app) changing it since: its size, its
/// modification time and a hash of its content (decisions.md, entry 179). The desktop
/// makes and checks stamps against files (`method-schedule-app`'s `safety::StampAt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskStamp {
    pub len: u64,
    pub hash: u64,
    /// When the file was last modified, as read just after this session read or wrote it.
    /// Only a shortcut for the desktop's quick check: the content decides.
    pub modified: Option<SystemTime>,
}

/// How a file on disk compares with its [`DiskStamp`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskCheck {
    /// The content we last read or wrote.
    Same,
    /// Different content, or a file we couldn't read.
    Changed,
    /// The file is gone (moved or deleted); a save puts it back.
    Missing,
}

impl DiskStamp {
    pub fn of(bytes: &[u8]) -> DiskStamp {
        DiskStamp {
            len: bytes.len() as u64,
            hash: content_hash(bytes),
            modified: None,
        }
    }

    /// Whether `bytes` are the content this stamp was made from.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.len && content_hash(bytes) == self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_waits_for_quiet_or_the_longest_wait() {
        assert!(!autosave_due(1_000, 0, Some(0)));
        assert!(autosave_due(3_000, 0, Some(0)));
        assert!(!autosave_due(29_999, 29_000, Some(0)));
        assert!(autosave_due(30_000, 29_000, Some(0)));
        assert!(
            !autosave_due(0, 5, None),
            "a clock read before the change waits"
        );
    }

    #[test]
    fn keys_are_unique_plain_names() {
        let a = new_recovery_key(Some(Path::new("/x/film.mtsc")));
        let b = new_recovery_key(Some(Path::new("/x/film.mtsc")));
        assert_ne!(a, b);
        assert!(a.starts_with(&path_hash(Path::new("/x/film.mtsc"))));
        assert!(new_recovery_key(None).starts_with("untitled-"));
        assert!(valid_key(&a) && !valid_key("../x") && !valid_key(""));
    }

    #[test]
    fn a_stamp_matches_only_its_bytes() {
        let s = DiskStamp::of(b"one");
        assert!(s.matches(b"one") && !s.matches(b"two") && !s.matches(b"one "));
    }
}
