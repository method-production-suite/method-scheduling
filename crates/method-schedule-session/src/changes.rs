//! What Changed without a window: the engine's diff of two schedules, grouped for the
//! page (decisions.md, the diff and merge entries). The desktop picks the older schedule
//! from a file or the disk; a host hands it in.

use serde::Serialize;

use method_schedule_core::diff::{diff, Diff};
use method_schedule_core::Schedule;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeDto {
    /// `added`, `removed`, `changed`, `moved`.
    pub kind: &'static str,
    /// `title`, `category`, `element`, `sheet`, `board`, `sub_board`, `calendar`, `strip`.
    pub subject: &'static str,
    pub text: String,
}

/// Changes under one heading: Schedule, Categories, Elements, Sheets, Boards, Calendars,
/// Strips (the engine's order).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GroupDto {
    pub group: &'static str,
    pub changes: Vec<ChangeDto>,
}

/// The changes of a diff, grouped under their headings in the engine's order.
pub fn groups(d: &Diff) -> Vec<GroupDto> {
    let mut out: Vec<GroupDto> = Vec::new();
    for c in &d.changes {
        let g = c.subject.group();
        let dto = ChangeDto {
            kind: c.kind.key(),
            subject: c.subject.key(),
            text: c.text.clone(),
        };
        match out.last_mut() {
            Some(last) if last.group == g => last.changes.push(dto),
            _ => out.push(GroupDto {
                group: g,
                changes: vec![dto],
            }),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WhatChangedDto {
    /// The older side: `"other.mtsc"`, `"harbor.mtsc as saved on disk"`.
    pub from: String,
    /// The schedule in the window.
    pub to: String,
    pub count: usize,
    pub groups: Vec<GroupDto>,
}

/// What changed from `older` to `newer`.
pub fn compare(older: &Schedule, newer: &Schedule, from: String, to: String) -> WhatChangedDto {
    let d = diff(older, newer);
    WhatChangedDto {
        from,
        to,
        count: d.changes.len(),
        groups: groups(&d),
    }
}
