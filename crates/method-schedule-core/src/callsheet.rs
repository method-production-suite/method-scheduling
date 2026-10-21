//! Call sheet data for one shoot day (decisions.md, BYD-02; docs/CALLSHEET.md): the
//! day's scenes in board order with their clock times, the cast with their board IDs,
//! every other element by category, sunrise and sunset, and notes. Data only: a designed
//! call sheet is made elsewhere from the JSON or CSV (method_schedule_format::callsheet).

use crate::model::{BoardId, CategoryId, ElementId, Error, Schedule, SheetId, SubBoardId};
use crate::plan::{clock_text, hours_text};
use crate::reports::id_order;
use crate::solar::Sun;
use crate::timeline::{timelines, EntryKind};
use crate::{Date, Eighths};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallScene {
    pub sheet: SheetId,
    pub scenes: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    pub pages: Eighths,
    pub synopsis: String,
    pub location: String,
    /// Cast board IDs (or names without one), in board ID order.
    pub cast: Vec<String>,
    /// Clock times (minutes after midnight) with a call time; else none.
    pub start: Option<i32>,
    pub end: Option<i32>,
    pub shoot_minutes: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallCast {
    pub element: ElementId,
    pub board_id: Option<String>,
    pub name: String,
    /// The scene numbers they are in today, in board order.
    pub scenes: Vec<String>,
    /// When their first scene starts and last ends (with a call time).
    pub first: Option<i32>,
    pub last: Option<i32>,
    /// Their call and wrap: their own where they have one (decision 231), else the
    /// company's. Minutes after midnight; none without a call time.
    pub call: Option<i32>,
    pub wrap: Option<i32>,
    /// Whether either is their own.
    pub own: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallElement {
    pub element: ElementId,
    pub board_id: Option<String>,
    pub name: String,
    pub scenes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallCategory {
    pub category: CategoryId,
    pub name: String,
    pub elements: Vec<CallElement>,
}

/// A later shoot day in brief, for the advance schedule (decision 232).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvanceDay {
    pub day: u32,
    pub date: Option<Date>,
    pub call: Option<u32>,
    pub pages: Eighths,
    pub scenes: Vec<CallScene>,
}

/// How many shoot days after the call sheet's the advance schedule covers.
pub const ADVANCE_DAYS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSheet {
    pub title: String,
    pub company: String,
    pub director: String,
    pub producer: String,
    pub board: String,
    pub sub_board: String,
    pub day: u32,
    pub days: u32,
    pub date: Option<Date>,
    pub call: Option<u32>,
    /// With a call time: minutes after midnight (1440 or more is after midnight).
    pub wrap: Option<i32>,
    /// Shoot times and banners, in minutes.
    pub length: u32,
    pub pages: Eighths,
    /// Where the sunrise and sunset are for, and the times.
    pub sun_location: Option<String>,
    pub sun: Option<Sun>,
    pub scenes: Vec<CallScene>,
    pub cast: Vec<CallCast>,
    /// Categories other than the cast, in schedule order, those with elements today.
    pub elements: Vec<CallCategory>,
    /// Banners (with their times), events on the date, and the timeline's warnings.
    pub notes: Vec<String>,
    /// The next shoot days of the sub-board, up to `ADVANCE_DAYS`; none on the last day.
    pub advance: Vec<AdvanceDay>,
}

impl CallSheet {
    /// Sunrise and sunset as `HH:MM`, when known.
    pub fn sun_text(&self) -> (Option<String>, Option<String>) {
        match self.sun {
            Some(Sun::Times { rise, set }) => (Some(clock_text(rise)), Some(clock_text(set))),
            _ => (None, None),
        }
    }
}

/// An element on the day: its scene numbers, first start and last end.
type Seen = (ElementId, Vec<String>, Option<i32>, Option<i32>);

/// A scene's row on a day: its own heading, the cast by board ID, its clock times.
fn call_scene(
    s: &Schedule,
    h: &crate::model::Sheet,
    e: &crate::timeline::Entry,
    timed: bool,
) -> CallScene {
    let cast_cat = s.cast();
    let mut ids: Vec<(Option<String>, String)> = h
        .elements
        .iter()
        .filter_map(|el| s.element(*el))
        .filter(|x| Some(x.category) == cast_cat)
        .map(|x| (x.board_id.clone(), x.name.clone()))
        .collect();
    ids.sort_by(|a, b| id_order(a.0.as_deref(), b.0.as_deref()).then(a.1.cmp(&b.1)));
    CallScene {
        sheet: h.id,
        scenes: h.scenes.clone(),
        int_ext: h.int_ext.clone(),
        set: h.set.clone(),
        time_of_day: h.time_of_day.clone(),
        // A later part of a scene spanning days counts no pages (decision 76).
        pages: if e.later { Eighths::ZERO } else { h.pages },
        synopsis: h.synopsis.clone(),
        location: h.location.clone(),
        cast: ids.into_iter().map(|(id, n)| id.unwrap_or(n)).collect(),
        start: timed.then_some(e.start),
        end: timed.then_some(e.end),
        shoot_minutes: if e.later {
            None
        } else {
            h.shoot_time.map(|m| m.0)
        },
    }
}

/// A later day's scenes in brief (decision 232).
fn advance_day(s: &Schedule, t: &crate::timeline::DayTimeline) -> AdvanceDay {
    let sheets = s.sheet_index();
    let scenes: Vec<CallScene> = t
        .entries
        .iter()
        .filter_map(|e| match e.kind {
            EntryKind::Scene(id) => Some(call_scene(s, sheets.get(&id)?, e, t.call.is_some())),
            EntryKind::Banner(_) => None,
        })
        .collect();
    AdvanceDay {
        day: t.number,
        date: t.date,
        call: t.call,
        pages: scenes.iter().map(|x| x.pages).sum(),
        scenes,
    }
}

/// The call sheet data for shoot day `day` (from 1) of a sub-board.
pub fn call_sheet(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
    day: u32,
) -> Result<CallSheet, Error> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let mut all = timelines(s, board, sub)?;
    let days = all.len() as u32;
    let at = all.iter().position(|t| t.number == day).ok_or_else(|| {
        Error::Broken(format!(
            "{} has {} shoot days; there is no day {day}",
            sb.name, days
        ))
    })?;
    let advance = all[at + 1..]
        .iter()
        .take(ADVANCE_DAYS)
        .map(|t| advance_day(s, t))
        .collect();
    let t = all.swap_remove(at);
    let cast_cat = s.cast();
    let mut scenes = Vec::new();
    let mut notes = Vec::new();
    let mut pages = Eighths::ZERO;
    // Element to (scene numbers, first start, last end), in first-seen order.
    let mut seen: Vec<Seen> = Vec::new();
    let timed = t.call.is_some();
    let (sheets, elements) = (s.sheet_index(), s.element_index());
    for e in &t.entries {
        match &e.kind {
            EntryKind::Banner(text) => {
                let minutes = (e.end - e.start).max(0) as u32;
                let when = if timed {
                    format!("{} ", t.time_text(e.start))
                } else {
                    String::new()
                };
                let long = if minutes > 0 {
                    format!(" ({})", hours_text(minutes))
                } else {
                    String::new()
                };
                notes.push(format!("{when}{text}{long}"));
            }
            EntryKind::Scene(id) => {
                let Some(h) = sheets.get(id) else { continue };
                let row = call_scene(s, h, e, timed);
                pages += row.pages;
                for el in &h.elements {
                    if !elements.contains_key(el) {
                        continue;
                    }
                    let number = if h.scenes.trim().is_empty() {
                        crate::timeline::scene_name(h)
                    } else {
                        h.scenes.trim().to_string()
                    };
                    match seen.iter_mut().find(|(e, ..)| e == el) {
                        Some(row) => {
                            row.1.push(number);
                            if timed {
                                row.3 = Some(e.end);
                            }
                        }
                        None => seen.push((
                            *el,
                            vec![number],
                            timed.then_some(e.start),
                            timed.then_some(e.end),
                        )),
                    }
                }
                scenes.push(row);
            }
        }
    }
    if let Some(date) = t.date {
        for ev in &sb.calendar.events {
            if ev.from <= date && date <= ev.to {
                let who: Vec<String> = ev
                    .elements
                    .iter()
                    .filter_map(|e| elements.get(e).copied())
                    .map(|e| e.name.clone())
                    .collect();
                let name = if ev.name.trim().is_empty() {
                    "Event".to_string()
                } else {
                    ev.name.trim().to_string()
                };
                if who.is_empty() {
                    notes.push(name);
                } else {
                    notes.push(format!("{name}: {}", who.join(", ")));
                }
            }
        }
    }
    notes.extend(t.warnings.iter().cloned());
    let mut cast = Vec::new();
    let mut by_cat: Vec<CallCategory> = Vec::new();
    for (el, numbers, first, last) in seen {
        let Some(x) = elements.get(&el).copied() else {
            continue;
        };
        if Some(x.category) == cast_cat {
            let mine = s.plan.person_call(t.day, el);
            let call = mine.and_then(|m| m.call).or(t.call).map(|c| c as i32);
            let wrap = mine
                .and_then(|m| m.wrap)
                .map(|w| w as i32)
                .or(t.call.map(|_| t.wrap));
            cast.push(CallCast {
                element: el,
                board_id: x.board_id.clone(),
                name: x.name.clone(),
                scenes: numbers,
                first,
                last,
                call,
                wrap,
                own: mine.is_some(),
            });
            continue;
        }
        let item = CallElement {
            element: el,
            board_id: x.board_id.clone(),
            name: x.name.clone(),
            scenes: numbers,
        };
        match by_cat.iter_mut().find(|c| c.category == x.category) {
            Some(c) => c.elements.push(item),
            None => by_cat.push(CallCategory {
                category: x.category,
                name: s
                    .category(x.category)
                    .map_or(String::new(), |c| c.name.clone()),
                elements: vec![item],
            }),
        }
    }
    let by_id = |a: &Option<String>, an: &str, b: &Option<String>, bn: &str| {
        id_order(a.as_deref(), b.as_deref())
            .then_with(|| an.to_ascii_uppercase().cmp(&bn.to_ascii_uppercase()))
    };
    cast.sort_by(|a, b| by_id(&a.board_id, &a.name, &b.board_id, &b.name));
    for c in &mut by_cat {
        c.elements
            .sort_by(|a, b| by_id(&a.board_id, &a.name, &b.board_id, &b.name));
    }
    let place = |c: CategoryId| s.categories.iter().position(|x| x.id == c);
    by_cat.sort_by_key(|c| place(c.category));
    Ok(CallSheet {
        title: s.title.clone(),
        company: s.production.company.clone(),
        director: s.production.director.clone(),
        producer: s.production.producer.clone(),
        board: b.name.clone(),
        sub_board: sb.name.clone(),
        day,
        days,
        date: t.date,
        call: t.call,
        wrap: t.call.map(|_| t.wrap),
        length: t.length,
        pages,
        sun_location: t.sun.as_ref().map(|x| x.location.clone()),
        sun: t.sun.map(|x| x.sun),
        scenes,
        cast,
        elements: by_cat,
        notes,
        advance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::tests::la;

    #[test]
    fn a_days_call_sheet() {
        let (mut s, b, sb) = la();
        let props = s.add_category("Props");
        let net = s.add_element(props, "NET").unwrap();
        let s2 = s.sheets[1].id;
        s.tag(s2, net).unwrap();
        s.set_board_id(s.elements[0].id, Some("1")).unwrap();
        let c = call_sheet(&s, b, sb, 1).unwrap();
        assert_eq!(c.day, 1);
        assert_eq!(c.days, 2);
        assert_eq!(c.date, "2024-06-20".parse().ok());
        assert_eq!(c.call, Some(420));
        assert_eq!(c.wrap, Some(1200));
        assert_eq!(c.scenes.len(), 3);
        assert_eq!(c.scenes[1].scenes, "2");
        assert_eq!(c.scenes[1].start, Some(1080));
        assert_eq!(c.scenes[0].cast, vec!["1".to_string()]);
        assert_eq!(c.cast.len(), 1);
        assert_eq!(c.cast[0].name, "ANN");
        assert_eq!(c.cast[0].scenes, vec!["1".to_string(), "2".to_string()]);
        assert_eq!(c.cast[0].first, Some(420));
        assert_eq!(c.cast[0].last, Some(1200));
        assert_eq!(
            (c.cast[0].call, c.cast[0].wrap, c.cast[0].own),
            (Some(420), Some(1200), false)
        );
        // Her own call (decision 231) replaces the company's; the wrap stays the day's.
        let day = s.plan.call_times[0].day;
        let ann = c.cast[0].element;
        crate::plan::set_person_call(&mut s, day, ann, Some(6 * 60), None).unwrap();
        let c = call_sheet(&s, b, sb, 1).unwrap();
        assert_eq!(
            (c.cast[0].call, c.cast[0].wrap, c.cast[0].own),
            (Some(360), Some(1200), true)
        );
        assert_eq!(c.elements.len(), 1);
        assert_eq!(c.elements[0].name, "Props");
        assert_eq!(c.elements[0].elements[0].name, "NET");
        assert_eq!(c.sun_location.as_deref(), Some("Pier"));
        let (rise, set) = c.sun_text();
        assert!(rise.unwrap().starts_with("05:4"));
        assert!(set.unwrap().starts_with("20:0"));
        assert_eq!(c.notes[0], "17:00 Lunch (1:00)");
        assert!(c.notes.iter().any(|n| n.contains("no shoot time")));
        assert!(call_sheet(&s, b, sb, 3).is_err());
    }
}
