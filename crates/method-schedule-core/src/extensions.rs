//! Additions to a schedule file that this version doesn't know (decision 224): an
//! `extensions` object at the file's top level, on the schedule and on each category, red
//! flag type, element, sheet, board and sub-board, kept as read and written back
//! untouched. Another app (or a newer Method Schedule) puts what it needs there, and a
//! file it wrote still opens here; the strict reader refuses every other unknown key.
//!
//! Ids are never reused across kinds, so an object's additions are kept by its id. Each is
//! the object's JSON text, compact; method-schedule-format reads and writes it.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extensions {
    /// The file's top level, beside `format` and `version`.
    pub file: Option<String>,
    /// The schedule's.
    pub schedule: Option<String>,
    /// Each object's, by its id.
    pub objects: BTreeMap<u32, String>,
}

impl Extensions {
    pub fn is_empty(&self) -> bool {
        self.file.is_none() && self.schedule.is_none() && self.objects.is_empty()
    }

    /// Object `id`'s additions.
    pub fn of(&self, id: u32) -> Option<&str> {
        self.objects.get(&id).map(String::as_str)
    }

    /// Both sides' additions, `ours` first where both have the same object's (a merge).
    pub fn union(ours: &Extensions, theirs: &Extensions) -> Extensions {
        let mut objects = theirs.objects.clone();
        objects.extend(ours.objects.clone());
        Extensions {
            file: ours.file.clone().or_else(|| theirs.file.clone()),
            schedule: ours.schedule.clone().or_else(|| theirs.schedule.clone()),
            objects,
        }
    }
}
