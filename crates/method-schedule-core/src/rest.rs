//! Rest periods (decisions.md, BYD-09): a category can have a turnaround minimum, the
//! shortest rest between one day's wrap and the next working day's call for its
//! elements. Each element of such a category is checked across every sub-board of a
//! board, from the timelines ([`crate::timeline`]): for each two dates it works in a row,
//! the rest is the time from the earlier day's wrap to the later day's call: an element's
//! own call and wrap where the plan has them (decision 231), else the company's. Company
//! times can only make a rest look shorter than it is, so the check errs on the side of a
//! warning.
//!
//! Warnings only, like conflicts: nothing is blocked. A pair is skipped when a day has no
//! date or no call time; `unchecked` counts those days. Two bookings on one date are a
//! conflict ([`crate::conflicts`]), not a rest problem, and are skipped here.

use std::collections::BTreeMap;

use crate::model::{BoardId, CategoryId, ElementId, Error, Schedule, SubBoardId};
use crate::plan::hours_text;
use crate::timeline::{timelines, EntryKind};
use crate::Date;

/// One day's work for an element, as the rest check sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shift {
    pub sub_board: SubBoardId,
    pub day: u32,
    pub date: Date,
    /// Minutes after the date's midnight.
    pub call: i32,
    pub wrap: i32,
}

/// A rest shorter than the element's category minimum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortRest {
    pub element: ElementId,
    pub category: CategoryId,
    pub from: Shift,
    pub to: Shift,
    /// Minutes from wrap to call (may be negative when days overlap).
    pub rest: i32,
    pub minimum: u32,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestReport {
    pub short: Vec<ShortRest>,
    /// Work days of checked elements that had no date or no call time.
    pub unchecked: usize,
}

/// Every rest on `board` shorter than its category's turnaround minimum, by element in
/// category then element order, each element's in date order.
pub fn rest_report(s: &Schedule, board: BoardId) -> Result<RestReport, Error> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let mut out = RestReport::default();
    if s.plan.turnarounds.is_empty() {
        return Ok(out);
    }
    // Element to its shifts.
    let mut work: BTreeMap<ElementId, Vec<Shift>> = BTreeMap::new();
    let (sheets, elements) = (s.sheet_index(), s.element_index());
    for sb in &b.sub_boards {
        for day in timelines(s, board, sb.id)? {
            let mut els: Vec<ElementId> = Vec::new();
            for e in &day.entries {
                let EntryKind::Scene(id) = e.kind else {
                    continue;
                };
                if let Some(h) = sheets.get(&id) {
                    for el in &h.elements {
                        let checked = elements
                            .get(el)
                            .is_some_and(|x| s.plan.turnaround(x.category).is_some());
                        if checked && !els.contains(el) {
                            els.push(*el);
                        }
                    }
                }
            }
            for el in els {
                // Their own call and wrap where set (decision 231), else the company's.
                let own = s.plan.person_call(day.day, el);
                let call = own.and_then(|x| x.call).or(day.call);
                let wrap = own
                    .and_then(|x| x.wrap)
                    .map(|w| w as i32)
                    .or(day.call.map(|_| day.wrap));
                match (day.date, call, wrap) {
                    (Some(date), Some(call), Some(wrap)) => {
                        work.entry(el).or_default().push(Shift {
                            sub_board: sb.id,
                            day: day.number,
                            date,
                            call: call as i32,
                            wrap,
                        })
                    }
                    _ => out.unchecked += 1,
                }
            }
        }
    }
    let cat_place = |c: CategoryId| s.categories.iter().position(|x| x.id == c);
    let mut els: Vec<ElementId> = work.keys().copied().collect();
    let rank: std::collections::HashMap<ElementId, usize> = s
        .elements
        .iter()
        .enumerate()
        .map(|(i, x)| (x.id, i))
        .collect();
    els.sort_by_key(|e| {
        let el = elements.get(e);
        (el.and_then(|x| cat_place(x.category)), rank.get(e).copied())
    });
    for el in els {
        let Some(e) = elements.get(&el).copied() else {
            continue;
        };
        let Some(minimum) = s.plan.turnaround(e.category) else {
            continue;
        };
        let mut shifts = work.remove(&el).unwrap_or_default();
        shifts.sort_by_key(|x| (x.date, x.call));
        for pair in shifts.windows(2) {
            let (a, c) = (pair[0], pair[1]);
            if a.date == c.date {
                continue;
            }
            let rest = (c.date.0 - a.date.0) * 24 * 60 + c.call - a.wrap;
            if rest >= minimum as i32 {
                continue;
            }
            let text = format!(
                "{} rests {} between {} (wrap {}) and {} (call {}), under the {} turnaround.",
                e.name,
                signed_hours(rest),
                a.date,
                crate::plan::clock_text(a.wrap),
                c.date,
                crate::plan::clock_text(c.call),
                hours_text(minimum)
            );
            out.short.push(ShortRest {
                element: el,
                category: e.category,
                from: a,
                to: c,
                rest,
                minimum,
                text,
            });
        }
    }
    Ok(out)
}

fn signed_hours(m: i32) -> String {
    if m < 0 {
        format!("-{}", hours_text(m.unsigned_abs()))
    } else {
        hours_text(m as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{set_call_time, set_turnaround};
    use crate::timeline::tests::la;

    #[test]
    fn a_short_rest_is_reported() {
        let (mut s, b, _) = la();
        let cast = s.cast().unwrap();
        // No minimums: nothing to check.
        assert!(rest_report(&s, b).unwrap().short.is_empty());
        set_turnaround(&mut s, cast, Some(12 * 60)).unwrap();
        // Day 1 (Thu 2024-06-20) wraps 20:00; day 2 (Fri) calls 6:00: 10 hours.
        let r = rest_report(&s, b).unwrap();
        assert_eq!(r.short.len(), 1, "{r:?}");
        let x = &r.short[0];
        assert_eq!(x.rest, 600);
        assert_eq!(
            x.text,
            "ANN rests 10:00 between 2024-06-20 (wrap 20:00) and 2024-06-21 (call 06:00), \
             under the 12:00 turnaround."
        );
        // BO works only day 2.
        assert!(r
            .short
            .iter()
            .all(|x| s.element(x.element).unwrap().name == "ANN"));
        // A later call clears it.
        let d2 = s.plan.call_times[1].day;
        set_call_time(&mut s, d2, Some(8 * 60)).unwrap();
        assert!(rest_report(&s, b).unwrap().short.is_empty());
        // No call time on day 1: unchecked, not reported.
        let d1 = s.plan.call_times[0].day;
        set_call_time(&mut s, d1, None).unwrap();
        set_call_time(&mut s, d2, Some(6 * 60)).unwrap();
        let r = rest_report(&s, b).unwrap();
        assert!(r.short.is_empty());
        assert_eq!(r.unchecked, 1);
    }

    /// Decision 231: an element's own call and wrap replace the company's in its rest.
    #[test]
    fn a_persons_own_call_and_wrap_count() {
        use crate::plan::set_person_call;
        let (mut s, b, _) = la();
        let cast = s.cast().unwrap();
        set_turnaround(&mut s, cast, Some(12 * 60)).unwrap();
        let ann = s.element_named(cast, "ANN").unwrap();
        let (d1, d2) = (s.plan.call_times[0].day, s.plan.call_times[1].day);
        // ANN is released at 17:00 on day 1: 13 hours to the 6:00 call, no warning.
        set_person_call(&mut s, d1, ann, None, Some(17 * 60)).unwrap();
        assert!(rest_report(&s, b).unwrap().short.is_empty());
        // Called at 5:00 on day 2 too: 12 hours, the minimum, still fine.
        set_person_call(&mut s, d2, ann, Some(5 * 60), None).unwrap();
        assert!(rest_report(&s, b).unwrap().short.is_empty());
        // Back to the company's wrap on day 1: 9 hours to her 5:00 call.
        set_person_call(&mut s, d1, ann, None, None).unwrap();
        let r = rest_report(&s, b).unwrap();
        assert_eq!(r.short.len(), 1);
        assert_eq!(r.short[0].rest, 9 * 60);
        assert!(
            r.short[0].text.contains("(call 05:00)"),
            "{}",
            r.short[0].text
        );
        // A day with no company call is checked when she has a call of her own.
        crate::plan::set_call_time(&mut s, d1, None).unwrap();
        set_person_call(&mut s, d1, ann, Some(7 * 60), Some(19 * 60)).unwrap();
        let r = rest_report(&s, b).unwrap();
        assert_eq!(r.short.len(), 1, "{r:?}");
        assert_eq!(r.short[0].rest, 10 * 60);
    }
}
