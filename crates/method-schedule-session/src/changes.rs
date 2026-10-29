//! What Changed without a window: the engine's diff of two schedules, grouped for the
//! page (decisions.md, the diff and merge entries). The desktop picks the older schedule
//! from a file or the disk; a host hands it in.

use std::collections::BTreeMap;

use serde::Serialize;

use method_schedule_core::diff::{diff, Change, Diff, Subject};
use method_schedule_core::merge::{merge_with, Merged, Side};
use method_schedule_core::model::{CategoryId, ElementId, SheetId, StripKind};
use method_schedule_core::Schedule;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeDto {
    /// `added`, `removed`, `changed`, `moved`.
    pub kind: &'static str,
    /// `title`, `category`, `element`, `sheet`, `board`, `sub_board`, `calendar`, `strip`.
    pub subject: &'static str,
    pub text: String,
    /// Where the page can show it (decision 235): set only in What Changed, and only for a
    /// sheet, element (or its red flag), scheduled strip or calendar date (decision 237)
    /// still in the window's schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub go: Option<GoDto>,
}

/// A place to show, in the shape of a Find result: `kind` `sheet` (with `sheet` and the
/// first changed `field`), `element` (with `category`, `element`), `strip` (with
/// `board`, `sub_board`, `strip`) or `event`, a calendar date (with `board`, `sub_board`,
/// `date`), the kind Find gives an event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GoDto {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_board: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strip: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// Where change `c` can be shown in `here`, the schedule in the window.
fn go(here: &Schedule, c: &Change) -> Option<GoDto> {
    let none = GoDto {
        kind: "",
        sheet: None,
        field: None,
        category: None,
        element: None,
        board: None,
        sub_board: None,
        strip: None,
        date: None,
    };
    match c.subject {
        Subject::Sheet(id) => here.sheet(id).map(|_| GoDto {
            kind: "sheet",
            sheet: Some(id.0),
            field: c.fields.first().map(|f| f.field.to_string()),
            ..none
        }),
        Subject::Element(id) | Subject::RedFlag(id, _) => here.element(id).map(|e| GoDto {
            kind: "element",
            category: Some(e.category.0),
            element: Some(id.0),
            ..none
        }),
        Subject::Strip(b, id) => {
            let sb = here.board(b)?.sub_boards.iter().find(|sb| {
                sb.strips
                    .iter()
                    .any(|x| x.id == id && x.kind != StripKind::DayBreak)
            })?;
            Some(GoDto {
                kind: "strip",
                board: Some(b.0),
                sub_board: Some(sb.id.0),
                strip: Some(id.0),
                ..none
            })
        }
        Subject::Calendar(b, sb, Some(d)) => {
            here.board(b)?.sub_boards.iter().find(|x| x.id == sb)?;
            Some(GoDto {
                kind: "event",
                board: Some(b.0),
                sub_board: Some(sb.0),
                date: Some(d.to_string()),
                ..none
            })
        }
        _ => None,
    }
}

/// A merge conflict kept for review (decision 238), with where it is when that is still in
/// the schedule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReviewDto {
    pub key: String,
    pub what: String,
    pub base: Option<String>,
    pub ours: String,
    pub theirs: String,
    /// `ours` or `theirs`.
    pub taken: &'static str,
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub go: Option<GoDto>,
}

/// The merge conflicts kept for review, oldest first.
pub fn review_view(s: &Schedule) -> Vec<ReviewDto> {
    s.plan
        .conflicts
        .iter()
        .map(|c| ReviewDto {
            key: c.key.clone(),
            what: c.what.clone(),
            base: c.base.clone(),
            ours: c.ours.clone(),
            theirs: c.theirs.clone(),
            taken: c.taken.key(),
            date: c.date.map(|d| d.to_string()),
            go: go_key(s, &c.key),
        })
        .collect()
}

/// Where a merge conflict's key (`sheet:12:set`, `element:5:name`, `red_flag:7:dates`,
/// `category:2:name`) is in `s`: a sheet, an element or a category still there.
fn go_key(s: &Schedule, key: &str) -> Option<GoDto> {
    let mut parts = key.split(':');
    let kind = parts.next()?;
    let id: u32 = parts.next()?.parse().ok()?;
    let rest = parts.next();
    let none = GoDto {
        kind: "",
        sheet: None,
        field: None,
        category: None,
        element: None,
        board: None,
        sub_board: None,
        strip: None,
        date: None,
    };
    let element = |e: &method_schedule_core::model::Element| GoDto {
        kind: "element",
        category: Some(e.category.0),
        element: Some(e.id.0),
        ..none.clone()
    };
    match kind {
        "sheet" => s.sheet(SheetId(id)).map(|_| GoDto {
            kind: "sheet",
            sheet: Some(id),
            field: rest.map(str::to_string),
            ..none.clone()
        }),
        "element" => s.element(ElementId(id)).map(element),
        "red_flag" => s
            .elements
            .iter()
            .find(|e| e.red_flags.iter().any(|f| f.id.0 == id))
            .map(element),
        "category" => s.category(CategoryId(id)).map(|c| GoDto {
            kind: "category",
            category: Some(c.id.0),
            ..none.clone()
        }),
        _ => None,
    }
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
    grouped(d, None)
}

/// [`groups`], each change with where to show it in `here` (decision 235).
fn grouped(d: &Diff, here: Option<&Schedule>) -> Vec<GroupDto> {
    let mut out: Vec<GroupDto> = Vec::new();
    for c in &d.changes {
        let g = c.subject.group();
        let dto = ChangeDto {
            kind: c.kind.key(),
            subject: c.subject.key(),
            text: c.text.clone(),
            go: here.and_then(|s| go(s, c)),
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
        groups: grouped(&d, Some(newer)),
    }
}

// Merging (the desktop's File > Merge Changes From, and the browser host's merge_preview
// and merge_apply, decision 239).

/// Most picks a merge takes (one per conflict).
const MAX_PICKS: usize = 50_000;
/// Longest conflict key the page may send back.
const MAX_KEY: usize = 400;

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

/// A merge conflict as the page lists it.
pub fn conflict_dto(c: &method_schedule_core::merge::Conflict) -> ConflictDto {
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

/// What a finished merge from `theirs` says, `unchanged` when it changed nothing.
pub fn merge_notice(theirs: &str, m: &Merged, unchanged: bool) -> String {
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
            " {n} {word}: {} kept yours, {t} took theirs, listed under File > Merge \
             Conflicts to Review until you mark them done.",
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

/// Merges `theirs` into `ours` against `base` with `picks` (the rest keep ours), keeping
/// every conflict in the schedule for review until marked done (decision 238).
pub fn merge_into(
    base: &Schedule,
    ours: &mut Schedule,
    theirs: &Schedule,
    picks: &BTreeMap<String, Side>,
) -> Merged {
    let mut m = merge_with(base, ours, theirs, Side::Ours, picks);
    method_schedule_core::plan::keep_conflicts(
        &mut m.schedule,
        &m.conflicts,
        crate::clock::today(),
    );
    *ours = m.schedule.clone();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::model::{Place, Sheet};

    /// Decision 235: What Changed points at what is still in the window, and only there.
    #[test]
    fn what_changed_says_where_each_change_is() {
        let mut older = Schedule::new("ZTEST Go");
        let b = older.boards[0].id;
        let sb = older.boards[0].sub_boards[0].id;
        let cast = older.cast().unwrap();
        let ann = older.add_element(cast, "ANN").unwrap();
        let gone = older.add_element(cast, "BO").unwrap();
        let h = older.add_sheet(Sheet {
            scenes: "1".into(),
            set: "PIER".into(),
            ..Sheet::default()
        });
        let mut newer = older.clone();
        newer.sheets[0].set = "NET LOFT".into();
        newer.rename_element(ann, "ANNA").unwrap();
        newer.delete_element(gone, false).unwrap();
        let st = newer.boards[0].boneyard[0].id;
        newer
            .move_strips(
                b,
                &[st],
                Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .unwrap();
        let day = method_schedule_core::Date::from_ymd(2027, 3, 3).unwrap();
        newer.boards[0].sub_boards[0].calendar.holidays.push(day);
        newer.boards[0].sub_boards[0].calendar.weekly_off.clear();
        let w = compare(&older, &newer, "a".into(), "b".into());
        let all: Vec<&ChangeDto> = w.groups.iter().flat_map(|g| &g.changes).collect();
        let find = |subject: &str, kind: &str| {
            all.iter()
                .find(|c| c.subject == subject && c.kind == kind)
                .unwrap_or_else(|| panic!("no {kind} {subject} in {all:?}"))
        };
        let sheet = find("sheet", "changed").go.as_ref().unwrap();
        assert_eq!(
            (sheet.kind, sheet.sheet, sheet.field.as_deref()),
            ("sheet", Some(h.0), Some("set"))
        );
        let el = find("element", "changed").go.as_ref().unwrap();
        assert_eq!(
            (el.kind, el.element, el.category),
            ("element", Some(ann.0), Some(cast.0))
        );
        assert!(find("element", "removed").go.is_none(), "not in the window");
        let strip = find("strip", "moved").go.as_ref().unwrap();
        assert_eq!(
            (strip.kind, strip.sub_board, strip.strip),
            ("strip", Some(sb.0), Some(st.0))
        );
        // Decision 237: a date on a calendar goes to that date; the weekly days off don't.
        let cal: Vec<_> = all.iter().filter(|c| c.subject == "calendar").collect();
        assert_eq!(cal.len(), 2, "{cal:?}");
        let date = cal.iter().find_map(|c| c.go.as_ref()).unwrap();
        assert_eq!(
            (date.kind, date.sub_board, date.date.as_deref()),
            ("event", Some(sb.0), Some("2027-03-03"))
        );
        assert_eq!(cal.iter().filter(|c| c.go.is_some()).count(), 1);
        // Decision 238: merge conflicts kept for review link to what is still there.
        let mut kept = newer.clone();
        let keys = [
            format!("sheet:{}:set", h.0),
            format!("element:{}:name", gone.0),
            format!("category:{}:name", cast.0),
        ];
        let conflicts: Vec<_> = keys
            .iter()
            .map(|key| method_schedule_core::merge::Conflict {
                key: key.clone(),
                what: "x".into(),
                base: None,
                ours: "a".into(),
                theirs: "b".into(),
                taken: method_schedule_core::merge::Side::Ours,
                text: String::new(),
            })
            .collect();
        method_schedule_core::plan::keep_conflicts(&mut kept, &conflicts, None);
        let r = review_view(&kept);
        assert_eq!(r.len(), 3);
        let g = r[0].go.as_ref().unwrap();
        assert_eq!(
            (g.kind, g.sheet, g.field.as_deref()),
            ("sheet", Some(h.0), Some("set"))
        );
        assert!(r[1].go.is_none(), "deleted");
        assert_eq!(r[2].go.as_ref().unwrap().kind, "category");
        method_schedule_core::plan::settle_conflicts(&mut kept, Some(&keys[..1]));
        assert_eq!(review_view(&kept).len(), 2);
        // The merge's list has no links: leaving it would drop the picks.
        assert!(groups(&diff(&older, &newer))
            .iter()
            .flat_map(|g| &g.changes)
            .all(|c| c.go.is_none()));
    }
}
