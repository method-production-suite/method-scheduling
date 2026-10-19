//! Conflict marks on the board's strips and the calendar's dates (decisions.md, entry
//! 83): `method_schedule_core::conflicts` for the board, as the words each mark's title
//! and accessible name give. An element on one date in two sub-boards marks its strips
//! on each and the date on each sub-board's calendar; a counted event on a work day marks
//! that day's strips with the element and the date.

use std::collections::HashMap;

use method_schedule_core::conflicts::conflicts;
use method_schedule_core::days::days;
use method_schedule_core::model::{BoardId, SheetId, StripKind, SubBoardId};
use method_schedule_core::{Date, Schedule};

/// The marks for one board.
#[derive(Debug, Default)]
pub struct ConflictMarks {
    /// Strip id to the conflicts on it, in words.
    pub strips: HashMap<u32, Vec<String>>,
    /// (sub-board id, date) to the conflicts on it, in words.
    pub dates: HashMap<(u32, Date), Vec<String>>,
}

fn push(v: &mut Vec<String>, text: String) {
    if !v.contains(&text) {
        v.push(text);
    }
}

/// The marks for `board`. Empty when it has no conflicts, or the engine can't work them
/// out (a board that is gone).
pub fn marks(s: &Schedule, board: BoardId) -> ConflictMarks {
    let mut out = ConflictMarks::default();
    let Ok(found) = conflicts(s, board) else {
        return out;
    };
    if found.count() == 0 {
        return out;
    }
    let Some(b) = s.board(board) else {
        return out;
    };
    let name = |sub: SubBoardId| {
        b.sub_boards
            .iter()
            .find(|x| x.id == sub)
            .map_or(String::new(), |x| x.name.clone())
    };
    let element = |id| s.element(id).map_or(String::new(), |e| e.name.clone());
    // The strips of a sheet on a sub-board's shoot day.
    let mut days_of: HashMap<SubBoardId, Vec<Vec<u32>>> = HashMap::new();
    for sb in &b.sub_boards {
        if let Ok(t) = days(s, board, sb.id) {
            days_of.insert(
                sb.id,
                t.days
                    .iter()
                    .map(|d| d.strips.iter().map(|x| x.0).collect())
                    .collect(),
            );
        }
    }
    let strips_for = |sub: SubBoardId, day: u32, sheet: Option<SheetId>| -> Vec<u32> {
        let Some(sb) = b.sub_boards.iter().find(|x| x.id == sub) else {
            return Vec::new();
        };
        let on_day = days_of
            .get(&sub)
            .and_then(|d| d.get(day.saturating_sub(1) as usize))
            .cloned()
            .unwrap_or_default();
        sb.strips
            .iter()
            .filter(|st| on_day.contains(&st.id.0))
            .filter(|st| match (&st.kind, sheet) {
                (StripKind::Sheet(h), Some(want)) => *h == want,
                (StripKind::Sheet(_), None) => true,
                _ => false,
            })
            .map(|st| st.id.0)
            .collect()
    };
    for c in &found.conflicts {
        let who = element(c.element);
        for bk in &c.bookings {
            let others: Vec<String> = c
                .bookings
                .iter()
                .filter(|o| o.sub_board != bk.sub_board)
                .map(|o| format!("{} day {}", name(o.sub_board), o.day))
                .collect();
            let text = format!("{who} on {} also on {}", c.date, others.join(", "));
            for sheet in &bk.sheets {
                for st in strips_for(bk.sub_board, bk.day, Some(*sheet)) {
                    push(out.strips.entry(st).or_default(), text.clone());
                }
            }
            push(out.dates.entry((bk.sub_board.0, c.date)).or_default(), text);
        }
    }
    for w in &found.event_on_work {
        let ev = b
            .sub_boards
            .iter()
            .find(|x| x.id == w.sub_board)
            .and_then(|x| x.calendar.events.get(w.event));
        let text = format!(
            "{} at {} on a work day, {}",
            element(w.element),
            ev.map_or_else(|| w.name.clone(), |e| e.title()),
            w.date
        );
        // The day's strips that list the element.
        for st in strips_for(w.sub_board, w.day, None) {
            let lists = b
                .sub_boards
                .iter()
                .flat_map(|x| &x.strips)
                .find(|x| x.id.0 == st)
                .and_then(|x| match x.kind {
                    StripKind::Sheet(h) => s.sheet(h),
                    _ => None,
                })
                .is_some_and(|h| h.elements.contains(&w.element));
            if lists {
                push(out.strips.entry(st).or_default(), text.clone());
            }
        }
        if ev.is_some() {
            push(out.dates.entry((w.sub_board.0, w.date)).or_default(), text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::model::{Event, EventKind, Place, Sheet};

    #[test]
    fn a_conflict_marks_strips_and_dates_on_both_sub_boards() {
        let mut s = Schedule::new("ZTEST");
        let cast = s.cast().unwrap();
        let kit = s.add_element(cast, "KIT").unwrap();
        let b = s.boards[0].id;
        let main = s.boards[0].sub_boards[0].id;
        let unit = s.add_sub_board(b, "Unit B", None).unwrap();
        let sheet = |s: &mut Schedule, scenes: &str| {
            s.add_sheet(Sheet {
                scenes: scenes.into(),
                elements: vec![kit],
                ..Sheet::default()
            })
        };
        let a = sheet(&mut s, "1");
        let c = sheet(&mut s, "2");
        let strip_of = |s: &Schedule, h: SheetId| {
            s.boards[0]
                .boneyard
                .iter()
                .find(|x| x.kind == StripKind::Sheet(h))
                .unwrap()
                .id
        };
        for (h, sub) in [(a, main), (c, unit)] {
            let st = strip_of(&s, h);
            s.move_strips(
                b,
                &[st],
                Place {
                    sub_board: Some(sub),
                    index: 0,
                },
            )
            .unwrap();
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sub),
                    index: 1,
                },
            )
            .unwrap();
        }
        let day: Date = "2027-03-01".parse().unwrap();
        for sub in [main, unit] {
            s.boards[0]
                .sub_boards
                .iter_mut()
                .find(|x| x.id == sub)
                .unwrap()
                .start = Some(day);
        }
        let m = marks(&s, b);
        assert_eq!(m.strips.len(), 2, "{m:?}");
        // On the board's strips, unless the strip designer's switch hides them
        // (decision 136).
        let marked = |s: &Schedule| {
            crate::view::board(s, b)
                .unwrap()
                .sub_boards
                .iter()
                .flat_map(|x| x.days.iter().flat_map(|d| &d.strips))
                .filter(|st| !st.conflicts.is_empty())
                .count()
        };
        assert_eq!(marked(&s), 2);
        let mut hidden = s.clone();
        hidden.boards[0].strip_layout.hide_conflicts = true;
        assert_eq!(marked(&hidden), 0);
        let on_main = &m.dates[&(main.0, day)];
        assert_eq!(
            on_main,
            &vec!["KIT on 2027-03-01 also on Unit B day 1".to_string()]
        );
        assert!(m.dates.contains_key(&(unit.0, day)));
        // The Breakdown tab shows the sheet's conflicts under its head (EVT-06).
        let sheet = crate::view::sheet_view(&s, b.0, Some(c.0), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            sheet.conflicts,
            ["KIT on 2027-03-01 also on Main day 1".to_string()]
        );

        // A fitting on the work day marks it too; one with no name is called by its kind.
        s.set_event(
            b,
            main,
            None,
            Event {
                name: String::new(),
                kind: EventKind::Fitting,
                from: day,
                to: day,
                elements: vec![kit],
                color: None,
            },
        )
        .unwrap();
        let m = marks(&s, b);
        assert!(m.dates[&(main.0, day)]
            .iter()
            .any(|t| t == "KIT at Fitting on a work day, 2027-03-01"));
        // No conflicts, no marks.
        let quiet = Schedule::new("ZTEST");
        assert!(marks(&quiet, quiet.boards[0].id).strips.is_empty());
    }
}
