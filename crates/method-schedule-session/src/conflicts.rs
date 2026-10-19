//! The Conflicts tab (decisions.md, entries 28, 34 and 43): one element on one date in two
//! or more sub-boards of the board shown, and a travel day, fitting or rehearsal on a date
//! the element works on the same sub-board. The engine works them out
//! (`method_schedule_core::conflicts`); this names every element, sub-board and sheet, and
//! finds each sheet's strip so the page can go to it (ui/js/conflicts.js).
//!
//! Event clashes across sub-boards and undated sub-boards travel apart: they are
//! informational, never conflicts (the engine's module notes say why).

use serde::Serialize;

use method_schedule_core::conflicts::{conflicts, Booking};
use method_schedule_core::model::{Board, BoardId, ElementId, StripKind, SubBoardId};
use method_schedule_core::{Date, Schedule};

use crate::view::{weekday, NamedDto};

/// Most conflicts (and, apart, event clashes) listed; the counts say how many in all.
pub const MAX_LISTED: usize = 1000;

#[derive(Clone, Debug, Serialize)]
pub struct ConflictsDto {
    pub board: u32,
    pub board_name: String,
    /// Every conflict on the board: both lists below.
    pub count: usize,
    /// One element on one date in two or more sub-boards.
    pub conflicts: Vec<ConflictDto>,
    /// Events on work days (conflicts too).
    pub on_work: Vec<OnWorkDto>,
    /// Every event clash on the board (informational).
    pub clash_count: usize,
    pub event_clashes: Vec<ClashDto>,
    /// Sub-boards with no dates (no start date, or no working day), so they took no part.
    pub undated: Vec<NamedDto>,
    /// True when either list was cut at [`MAX_LISTED`].
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConflictDto {
    pub element: u32,
    pub name: String,
    pub category: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub weekday: &'static str,
    /// One per sub-board, in the board's order. At least two.
    pub bookings: Vec<BookingDto>,
}

/// A travel day, fitting or rehearsal on a date the element works on the same sub-board.
#[derive(Clone, Debug, Serialize)]
pub struct OnWorkDto {
    pub element: u32,
    pub name: String,
    pub category: String,
    pub date: String,
    pub weekday: &'static str,
    pub sub_board: u32,
    pub sub_board_name: String,
    /// The shoot day, counted in the sub-board.
    pub day: u32,
    /// The event's place in the sub-board's calendar.
    pub index: usize,
    /// The event's name, or its kind (`Fitting`) when it has none.
    pub event: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BookingDto {
    pub sub_board: u32,
    pub sub_board_name: String,
    /// The shoot day, counted in the sub-board.
    pub day: u32,
    pub sheets: Vec<SheetRefDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SheetRefDto {
    pub sheet: u32,
    /// `Scene 12`, or `A sheet with no scene number`.
    pub scene: String,
    /// The sheet's strip on the sub-board, to select on the board.
    pub strip: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClashDto {
    pub element: u32,
    pub name: String,
    pub category: String,
    pub date: String,
    pub weekday: &'static str,
    pub events: Vec<EventRefDto>,
    /// The element's strip work that date, any sub-board.
    pub bookings: Vec<BookingDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventRefDto {
    pub sub_board: u32,
    pub sub_board_name: String,
    /// The event's place in the sub-board's calendar.
    pub index: usize,
    pub name: String,
}

/// What the lists call event `index` of sub-board `sub`: its name, or its kind in words
/// when it has none ([`method_schedule_core::model::Event::title`]).
fn event_title(b: &Board, sub: SubBoardId, index: usize, name: &str) -> String {
    b.sub_boards
        .iter()
        .find(|x| x.id == sub)
        .and_then(|x| x.calendar.events.get(index))
        .map_or_else(|| name.to_string(), |e| e.title())
}

fn sub_name(b: &Board, id: SubBoardId) -> String {
    b.sub_boards
        .iter()
        .find(|x| x.id == id)
        .map(|x| x.name.clone())
        .unwrap_or_default()
}

fn booking(s: &Schedule, b: &Board, bk: &Booking) -> BookingDto {
    let sb = b.sub_boards.iter().find(|x| x.id == bk.sub_board);
    BookingDto {
        sub_board: bk.sub_board.0,
        sub_board_name: sub_name(b, bk.sub_board),
        day: bk.day,
        sheets: bk
            .sheets
            .iter()
            .map(|&sheet| {
                let scenes = s.sheet(sheet).map_or("", |h| h.scenes.trim());
                SheetRefDto {
                    sheet: sheet.0,
                    scene: if scenes.is_empty() {
                        "A sheet with no scene number".to_string()
                    } else {
                        format!("Scene {scenes}")
                    },
                    strip: sb.and_then(|sb| {
                        sb.strips
                            .iter()
                            .find(|st| st.kind == StripKind::Sheet(sheet))
                            .map(|st| st.id.0)
                    }),
                }
            })
            .collect(),
    }
}

fn names(s: &Schedule, el: ElementId) -> (String, String) {
    let e = s.element(el);
    (
        e.map(|e| e.name.clone()).unwrap_or_default(),
        e.and_then(|e| s.category(e.category))
            .map(|c| c.name.clone())
            .unwrap_or_default(),
    )
}

fn day(d: Date) -> (String, &'static str) {
    (d.to_string(), weekday(d))
}

/// How many conflicts board `board` has (0 when it is gone), for the tab's badge. It
/// goes with every view, so a board that can't have any isn't worked out (about 1 ms
/// saved per edit at 1,500 strips): one with no dated sub-board, or with one and no
/// travel, fitting or rehearsal on its calendar.
pub fn count(s: &Schedule, board: BoardId) -> usize {
    let Some(b) = s.board(board) else { return 0 };
    let dated: Vec<_> = b.sub_boards.iter().filter(|x| x.start.is_some()).collect();
    let counted_events = dated
        .iter()
        .any(|x| x.calendar.events.iter().any(|e| e.kind.code().is_some()));
    if dated.len() < 2 && !counted_events {
        return 0;
    }
    conflicts(s, board).map_or(0, |c| c.count())
}

/// The Conflicts tab for board `board`.
pub fn conflicts_view(s: &Schedule, board: u32) -> Result<ConflictsDto, String> {
    let b = crate::view::board_ref(s, board)?;
    let found = conflicts(s, b.id).map_err(|e| e.to_string())?;
    let list = found
        .conflicts
        .iter()
        .take(MAX_LISTED)
        .map(|c| {
            let (name, category) = names(s, c.element);
            let (date, weekday) = day(c.date);
            ConflictDto {
                element: c.element.0,
                name,
                category,
                date,
                weekday,
                bookings: c.bookings.iter().map(|bk| booking(s, b, bk)).collect(),
            }
        })
        .collect();
    let on_work = found
        .event_on_work
        .iter()
        .take(MAX_LISTED)
        .map(|c| {
            let (name, category) = names(s, c.element);
            let (date, weekday) = day(c.date);
            OnWorkDto {
                element: c.element.0,
                name,
                category,
                date,
                weekday,
                sub_board: c.sub_board.0,
                sub_board_name: sub_name(b, c.sub_board),
                day: c.day,
                index: c.event,
                event: event_title(b, c.sub_board, c.event, &c.name),
            }
        })
        .collect();
    let clashes = found
        .event_clashes
        .iter()
        .take(MAX_LISTED)
        .map(|c| {
            let (name, category) = names(s, c.element);
            let (date, weekday) = day(c.date);
            ClashDto {
                element: c.element.0,
                name,
                category,
                date,
                weekday,
                events: c
                    .events
                    .iter()
                    .map(|e| EventRefDto {
                        sub_board: e.sub_board.0,
                        sub_board_name: sub_name(b, e.sub_board),
                        index: e.index,
                        name: event_title(b, e.sub_board, e.index, &e.name),
                    })
                    .collect(),
                bookings: c.bookings.iter().map(|bk| booking(s, b, bk)).collect(),
            }
        })
        .collect();
    Ok(ConflictsDto {
        board: b.id.0,
        board_name: b.name.clone(),
        count: found.count(),
        conflicts: list,
        on_work,
        clash_count: found.event_clashes.len(),
        event_clashes: clashes,
        undated: found
            .undated
            .iter()
            .map(|&id| NamedDto {
                id: id.0,
                name: sub_name(b, id),
            })
            .collect(),
        truncated: found.conflicts.len() > MAX_LISTED
            || found.event_on_work.len() > MAX_LISTED
            || found.event_clashes.len() > MAX_LISTED,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::model::{Event, EventKind, Place, Sheet, SheetId, StripId};

    /// Main from Mon 2027-03-01 with ADA on day 1 (scene 1) and day 2 (scene 2, with BEN);
    /// Unit B from Tue 2027-03-02 with ADA and BEN on its day 1 (scene 3) and ADA on day 2
    /// (scene 4); Unit C with no start date. A fitting for ADA on Unit B on Mon 2027-03-01.
    fn three_units() -> (Schedule, u32) {
        let mut s = Schedule::new("ZTEST Conflicts app");
        let cast = s.category_named("Cast").unwrap();
        let ada = s.add_element(cast, "ADA").unwrap();
        let ben = s.add_element(cast, "BEN").unwrap();
        let sheets: Vec<SheetId> = [vec![ada], vec![ada, ben], vec![ada, ben], vec![ada]]
            .into_iter()
            .enumerate()
            .map(|(i, elements)| {
                s.add_sheet(Sheet {
                    scenes: (i + 1).to_string(),
                    elements,
                    ..Sheet::default()
                })
            })
            .collect();
        let d = |x: &str| -> Date { x.parse().unwrap() };
        let b = s.boards[0].id;
        let main = s.boards[0].sub_boards[0].id;
        let unit = s.add_sub_board(b, "Unit B", Some(d("2027-03-02"))).unwrap();
        s.add_sub_board(b, "Unit C", None).unwrap();
        s.boards[0].sub_boards[0].start = Some(d("2027-03-01"));
        for (i, sheet) in sheets.iter().enumerate() {
            let sub = if i < 2 { main } else { unit };
            let strip: StripId = s.boards[0]
                .boneyard
                .iter()
                .find(|x| x.kind == StripKind::Sheet(*sheet))
                .unwrap()
                .id;
            let end = s.boards[0]
                .sub_boards
                .iter()
                .find(|x| x.id == sub)
                .unwrap()
                .strips
                .len();
            s.move_strips(
                b,
                &[strip],
                Place {
                    sub_board: Some(sub),
                    index: end,
                },
            )
            .unwrap();
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sub),
                    index: end + 1,
                },
            )
            .unwrap();
        }
        // ADA works on Main on Mon Mar 1 and has a fitting on Unit B that day: a clash.
        s.boards[0].sub_boards[1].calendar.events.push(Event {
            name: "Fitting".into(),
            kind: EventKind::Fitting,
            from: d("2027-03-01"),
            to: d("2027-03-01"),
            elements: vec![ada],
            color: None,
        });
        s.check().unwrap();
        (s, b.0)
    }

    #[test]
    fn conflicts_name_their_element_sub_boards_and_strips() {
        let (s, b) = three_units();
        let c = conflicts_view(&s, b).unwrap();
        assert_eq!(c.board_name, "Board 1");
        assert_eq!(c.count, 2, "ADA and BEN on Tue 2027-03-02");
        assert_eq!(c.conflicts.len(), 2);
        let ada = &c.conflicts[0];
        assert_eq!((ada.name.as_str(), ada.category.as_str()), ("ADA", "Cast"));
        assert_eq!((ada.date.as_str(), ada.weekday), ("2027-03-02", "Tue"));
        let subs: Vec<(&str, u32)> = ada
            .bookings
            .iter()
            .map(|bk| (bk.sub_board_name.as_str(), bk.day))
            .collect();
        assert_eq!(subs, [("Main", 2), ("Unit B", 1)]);
        assert_eq!(ada.bookings[0].sheets[0].scene, "Scene 2");
        // Each sheet's strip is on its sub-board.
        for bk in &ada.bookings {
            let sb = s.boards[0]
                .sub_boards
                .iter()
                .find(|x| x.id.0 == bk.sub_board)
                .unwrap();
            for sh in &bk.sheets {
                let st = sb
                    .strips
                    .iter()
                    .find(|x| Some(x.id.0) == sh.strip)
                    .expect("the strip is on the sub-board");
                assert_eq!(st.kind, StripKind::Sheet(SheetId(sh.sheet)));
            }
        }
        // Apart: the clash and the undated sub-board.
        assert_eq!(c.clash_count, 1);
        let clash = &c.event_clashes[0];
        assert_eq!(
            (clash.name.as_str(), clash.date.as_str()),
            ("ADA", "2027-03-01")
        );
        assert_eq!(clash.events[0].sub_board_name, "Unit B");
        assert_eq!(clash.events[0].name, "Fitting");
        assert_eq!(clash.bookings.len(), 1);
        assert_eq!(c.undated.len(), 1);
        assert_eq!(c.undated[0].name, "Unit C");
        assert!(!c.truncated);
        assert!(c.on_work.is_empty());
        assert_eq!(count(&s, s.boards[0].id), 2);
    }

    #[test]
    fn an_event_on_a_work_day_is_a_conflict() {
        // Behavior note 17, A4 and A5. Travel with no name for ADA on Main's day 1.
        let (mut s, b) = three_units();
        let d = |x: &str| -> Date { x.parse().unwrap() };
        let ada = s.elements[0].id;
        s.boards[0].sub_boards[0].calendar.events.push(Event {
            name: String::new(),
            kind: EventKind::Travel,
            from: d("2027-03-01"),
            to: d("2027-03-01"),
            elements: vec![ada],
            color: None,
        });
        let c = conflicts_view(&s, b).unwrap();
        assert_eq!(c.count, 3);
        assert_eq!(c.on_work.len(), 1);
        let w = &c.on_work[0];
        assert_eq!(
            (
                w.name.as_str(),
                w.date.as_str(),
                w.sub_board_name.as_str(),
                w.day
            ),
            ("ADA", "2027-03-01", "Main", 1)
        );
        assert_eq!((w.event.as_str(), w.index), ("Travel", 0));
        // The nameless event is called by its kind in the clashes too, never an empty link.
        let names: Vec<&str> = c.event_clashes[0]
            .events
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["Travel", "Fitting"]);
        assert_eq!(count(&s, s.boards[0].id), 3);
        // With one sub-board, the badge still counts it.
        s.boards[0].sub_boards[1].start = None;
        assert_eq!(count(&s, s.boards[0].id), 1);
    }

    #[test]
    fn the_view_carries_the_count_and_a_missing_board_is_refused() {
        let (s, b) = three_units();
        let mut session = crate::session::Session::new();
        session
            .edit(|x| {
                *x = s.clone();
                Ok(())
            })
            .unwrap();
        assert_eq!(crate::view::view(&session, None).unwrap().conflicts, 2);
        assert_eq!(
            conflicts_view(&s, b + 1000).unwrap_err(),
            "That board is no longer in the schedule."
        );
        assert_eq!(count(&s, BoardId(b + 1000)), 0);
        // No conflicts on a schedule with one sub-board.
        let one = method_schedule_core::sample::sample(20);
        assert_eq!(conflicts_view(&one, one.boards[0].id.0).unwrap().count, 0);
    }
}
