//! Dragging an event on the Calendar to another date (decisions.md, entry 83): the event
//! keeps its length and moves to start on the date it was dropped on, as one undo step.

use method_schedule_core::Schedule;

use crate::board::EditResult;
use crate::calendar::typed_date;

/// Moves event `at` of sub-board `sub` so it starts on `to` (typed as the date fields
/// take it), keeping how many days it covers. Moving it onto its own start changes nothing.
pub fn move_event(s: &mut Schedule, sub: u32, at: usize, to: &str) -> EditResult {
    let start = typed_date(to).ok_or("Drop the event on a date.")?;
    let (b, sb) = crate::view::sub_ref(s, sub).map(|(b, sb)| (b.id, sb.id))?;
    let mut ev = s
        .board(b)
        .and_then(|x| x.sub_boards.iter().find(|x| x.id == sb))
        .and_then(|x| x.calendar.events.get(at))
        .cloned()
        .ok_or("That event is no longer on the calendar.")?;
    let shift = start.0 - ev.from.0;
    if shift == 0 {
        return Ok(());
    }
    ev.from = start;
    ev.to = ev.to.add_days(shift);
    s.set_event(b, sb, Some(at), ev)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::{Event, EventKind};

    #[test]
    fn an_event_moves_whole_as_one_step() {
        let mut s = Session::new();
        let (b, sb) = {
            let x = s.schedule();
            (x.boards[0].id, x.boards[0].sub_boards[0].id)
        };
        s.edit(|x| {
            x.set_event(
                b,
                sb,
                None,
                Event {
                    name: "Fitting".into(),
                    kind: EventKind::Fitting,
                    from: "2027-03-01".parse().unwrap(),
                    to: "2027-03-03".parse().unwrap(),
                    elements: Vec::new(),
                    color: None,
                },
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
        .unwrap();
        let steps = |s: &Session| s.schedule().boards[0].sub_boards[0].calendar.events[0].clone();
        s.edit(|x| move_event(x, sb.0, 0, "3/10/2027")).unwrap();
        let ev = steps(&s);
        assert_eq!(
            (ev.from.to_string(), ev.to.to_string()),
            ("2027-03-10".into(), "2027-03-12".into())
        );
        s.undo();
        assert_eq!(steps(&s).from.to_string(), "2027-03-01");
        // Back across a month, and errors in words.
        s.edit(|x| move_event(x, sb.0, 0, "2027-02-27")).unwrap();
        assert_eq!(steps(&s).to.to_string(), "2027-03-01");
        assert!(s.edit(|x| move_event(x, sb.0, 3, "2027-03-01")).is_err());
        assert!(s.edit(|x| move_event(x, sb.0, 0, "soon")).is_err());
        assert!(s.edit(|x| move_event(x, 9999, 0, "2027-03-01")).is_err());
    }
}
