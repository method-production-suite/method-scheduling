//! The Calendar tab's edits on the sub-board the page shows, as plain functions on a
//! [`Schedule`]: what a date is (working, day off, holiday, company travel), the weekly
//! days off, and events. Each validates what the page sent; `Session::edit` makes it one
//! undo step, and the board and the day out of days follow the reflowed dates.

use serde::Deserialize;

use method_schedule_core::model::{
    event_color, DayKind, ElementId, Event, EventKind, EVENT_COLORS,
};
use method_schedule_core::{Date, Schedule, Weekday};

use crate::board::{sub_board, EditResult};
use crate::elements::say;
use crate::sheet::MAX_NAME;

/// The longest event, in days (the engine's limit, decision 21).
pub use method_schedule_core::model::MAX_EVENT_DAYS;

/// The kinds a date can be set to, as the page sends them.
pub const DAY_KINDS: &[&str] = &["work", "off", "holiday", "move"];

/// An event as the page sends it: `kind` is a key of [`EventKind::keys`] (`other` with
/// `label` saying what kind); dates as typed (see [`typed_date`]); `color` a key of
/// [`EVENT_COLORS`], or none.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub name: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub elements: Vec<u32>,
    #[serde(default)]
    pub color: Option<String>,
}

/// A date as typed in the app's date fields (decision 49): `2027-03-01`, or month, day
/// and year with slashes, `3/1/2027` or `03/01/2027`. The year has four digits; the
/// month and day one or two (two in the dashed form). `None` for anything else,
/// including dates that don't exist (`2/30/2027`). The app shows a date back as
/// `2027-03-01`, the form every other screen uses.
pub fn typed_date(text: &str) -> Option<Date> {
    let t = text.trim();
    if let Ok(d) = t.parse::<Date>() {
        return Some(d);
    }
    let mut parts = t.split('/');
    let (m, d, y) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let digits = |p: &str, lo: usize, hi: usize| {
        (lo..=hi).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_digit())
    };
    if !(digits(m, 1, 2) && digits(d, 1, 2) && digits(y, 4, 4)) {
        return None;
    }
    Date::from_ymd(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
}

/// The sentence for a date field that doesn't hold a date.
pub fn not_a_date(what: &str, text: &str) -> String {
    format!(
        "{what} {:?} is not a date. Type it as 3/1/2027 or 2027-03-01.",
        text.trim()
    )
}

/// `value` as a date ([`typed_date`]), or the sentence saying it isn't one.
pub fn parse_date(what: &str, value: &str) -> EditResult<Date> {
    typed_date(value).ok_or_else(|| not_a_date(what, value))
}

/// `mon` to `sun`.
pub fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

/// Makes `date` a working date, a day off, a holiday or a company travel day (`work`,
/// `off`, `holiday`, `move`). Later shoot days move to the next working dates.
pub fn set_day_kind(s: &mut Schedule, sub: u32, date: &str, kind: &str) -> EditResult {
    let d = parse_date("The date", date)?;
    let kind = match kind {
        "work" => DayKind::Work,
        "off" => DayKind::Off,
        "holiday" => DayKind::Holiday,
        "move" => DayKind::Move,
        _ => return Err("A date is working, a day off, a holiday or company travel.".into()),
    };
    let (b, sb) = sub_board(s, sub)?;
    s.calendar_mut(b, sb).map_err(say)?.set_day_kind(d, kind);
    Ok(())
}

/// Sets the weekly days off from their names (`mon` to `sun`). At least one weekday has
/// to stay working.
pub fn set_weekly_off(s: &mut Schedule, sub: u32, days: &[String]) -> EditResult {
    let mut off = Vec::new();
    for name in days {
        let w = Weekday::ALL
            .iter()
            .copied()
            .find(|w| weekday_name(*w) == name)
            .ok_or_else(|| format!("{name:?} is not a weekday."))?;
        off.push(w);
    }
    if off.len() >= 7 && Weekday::ALL.iter().all(|w| off.contains(w)) {
        return Err("Keep at least one working day in the week.".into());
    }
    let (b, sb) = sub_board(s, sub)?;
    s.calendar_mut(b, sb)
        .map_err(say)?
        .set_weekly_off(&off)
        .map_err(say)
}

fn event_from(s: &Schedule, input: &EventInput) -> EditResult<Event> {
    let label = input.label.trim();
    let Some(kind) = EventKind::from_key(input.kind.as_str(), label) else {
        return Err(
            "An event is travel, a fitting, a rehearsal, unavailable, a camera test, post-production work or other."
                .into(),
        );
    };
    let name = input.name.trim();
    if name.chars().count() > MAX_NAME || label.chars().count() > MAX_NAME {
        return Err(format!("A name is at most {MAX_NAME} characters."));
    }
    let from = parse_date("From", &input.from)?;
    let to = parse_date("To", &input.to)?;
    if to < from {
        return Err("The event ends before it starts.".into());
    }
    if to.0 - from.0 >= MAX_EVENT_DAYS {
        return Err(format!(
            "An event is at most {MAX_EVENT_DAYS} days long. Check the years."
        ));
    }
    let color = match input
        .color
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        None => None,
        Some(c) => Some(event_color(c).ok_or_else(|| {
            format!(
                "An event's color is one of {}.",
                EVENT_COLORS
                    .iter()
                    .map(|(_, n, _)| n.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?),
    };
    let mut elements = Vec::new();
    for &id in &input.elements {
        let e = s
            .element(ElementId(id))
            .ok_or("One of those elements is no longer in the schedule.")?;
        if !elements.contains(&e.id) {
            elements.push(e.id);
        }
    }
    Ok(Event {
        name: name.to_string(),
        kind,
        from,
        to,
        elements,
        color,
    })
}

/// Adds an event (`at` is `None`) or replaces event `at`. Returns its index.
pub fn save_event(
    s: &mut Schedule,
    sub: u32,
    at: Option<usize>,
    input: &EventInput,
) -> EditResult<usize> {
    let event = event_from(s, input)?;
    let (b, sb) = sub_board(s, sub)?;
    if at.is_some_and(|i| i >= s.calendar_mut(b, sb).map_or(0, |c| c.events.len())) {
        return Err("That event is no longer on the calendar.".into());
    }
    s.set_event(b, sb, at, event).map_err(say)
}

/// The Calendar's View menu (decision 111): hides the switches named in `hidden`
/// (`CalendarShow` keys), showing the rest.
pub fn set_calendar_hidden(s: &mut Schedule, hidden: &[String]) -> EditResult {
    use method_schedule_core::model::CalendarShow;
    let mut out = Vec::new();
    for k in hidden {
        out.push(
            CalendarShow::from_key(k)
                .ok_or_else(|| format!("{k:?} is not something the calendar shows."))?,
        );
    }
    s.set_calendar_hidden(&out);
    Ok(())
}

/// Deletes event `at`.
pub fn delete_event(s: &mut Schedule, sub: u32, at: usize) -> EditResult {
    let (b, sb) = sub_board(s, sub)?;
    if at >= s.calendar_mut(b, sb).map_or(0, |c| c.events.len()) {
        return Err("That event is no longer on the calendar.".into());
    }
    s.delete_event(b, sb, at).map_err(say)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::session::{read_script, Session};
    use std::path::Path;

    /// Behavior note 23: an empty date field hints the form, written as letters, never a
    /// sample date; the weeks view's dates keep no blank line above their strips.
    #[test]
    fn date_hints_are_forms_and_weeks_cells_are_tight() {
        const DATES_JS: &str = include_str!("../../method-schedule-app/ui/js/dates.js");
        const CSS: &str = include_str!("../../method-schedule-app/ui/styles.css");
        let hint = DATES_JS
            .split("export function datePlaceholder()")
            .nth(1)
            .and_then(|x| x.split('}').next())
            .expect("datePlaceholder");
        assert!(hint.contains("'M/D/YYYY'") && hint.contains("'YYYY-MM-DD'"));
        assert!(!hint.chars().any(|c| c.is_ascii_digit()), "{hint}");
        assert!(CSS.contains(".cal-month.cal-span .cal-day { min-height: 0; }"));
        assert!(CSS.contains(".cal-month.cal-span td { height: 48px; }"));
    }

    #[test]
    fn dates_are_typed_month_first_with_slashes_or_year_first_with_dashes() {
        let march_1: Date = "2027-03-01".parse().unwrap();
        for ok in [
            "2027-03-01",
            "3/1/2027",
            "03/01/2027",
            " 3/01/2027 ",
            "03/1/2027",
        ] {
            assert_eq!(typed_date(ok), Some(march_1), "{ok}");
        }
        assert_eq!(
            typed_date("2/29/2028"),
            "2028-02-29".parse().ok(),
            "a leap day"
        );
        for bad in [
            "",
            "3/1/27",
            "3-1-2027",
            "2027-3-1",
            "2027/03/01",
            "1/3/2027/1",
            "13/1/2027",
            "2/29/2027",
            "0/1/2027",
            "3/0/2027",
            "3/32/2027",
            "March 1 2027",
            "3/1/2027x",
            "+3/1/2027",
            "3//2027",
        ] {
            assert_ne!(typed_date(bad), Some(march_1), "{bad}");
            if bad != "2/29/2027" {
                assert_eq!(typed_date(bad), None, "{bad}");
            }
        }
        assert_eq!(typed_date("2/29/2027"), None, "no such day");
        let e = parse_date("From", "3/1/27").unwrap_err();
        assert_eq!(
            e,
            "From \"3/1/27\" is not a date. Type it as 3/1/2027 or 2027-03-01."
        );
        assert!(!e.contains('!') && !e.contains('\u{2014}'));
    }

    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        let mut s = Session::new();
        s.import_script(&read_script(&path).unwrap()).unwrap();
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        let ids: Vec<u32> = s.schedule().boards[0]
            .boneyard
            .iter()
            .map(|x| x.id.0)
            .collect();
        s.edit(|x| board::move_strips(x, x.boards[0].id.0, &ids, Some(sb), 0))
            .unwrap();
        s.edit(|x| board::auto_day_breaks(x, sb, "1/8")).unwrap();
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-01")))
            .unwrap();
        s
    }

    fn dates(s: &Session) -> Vec<Option<String>> {
        crate::view::board(s.schedule(), s.schedule().boards[0].id)
            .unwrap()
            .sub_boards[0]
            .days
            .iter()
            .map(|d| d.date.clone())
            .collect()
    }

    fn input(kind: &str, from: &str, to: &str, elements: Vec<u32>) -> EventInput {
        EventInput {
            kind: kind.into(),
            label: String::new(),
            name: "Wardrobe".into(),
            from: from.into(),
            to: to.into(),
            elements,
            color: None,
        }
    }

    #[test]
    fn marking_dates_reflows_the_board_one_step_each() {
        let mut s = harbor();
        let before = dates(&s);
        assert_eq!(before[1].as_deref(), Some("2027-03-02"));
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-02", "holiday"))
            .unwrap();
        assert_eq!(dates(&s)[1].as_deref(), Some("2027-03-03"));
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-02", "move"))
            .unwrap();
        assert_eq!(dates(&s)[1].as_deref(), Some("2027-03-03"));
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-06", "work"))
            .unwrap();
        assert!(dates(&s).contains(&Some("2027-03-06".to_string())));
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-02", "work"))
            .unwrap();
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-06", "off"))
            .unwrap();
        assert_eq!(dates(&s), before);
        // Each was one step: four undos bring back the holiday.
        for _ in 0..4 {
            s.undo();
        }
        assert_eq!(dates(&s)[1].as_deref(), Some("2027-03-03"));
        let cal = &s.schedule().boards[0].sub_boards[0].calendar;
        assert_eq!(cal.holidays.len(), 1);
        // Refusals change nothing.
        let snapshot = s.schedule().clone();
        assert!(s
            .edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-02-30", "off"))
            .is_err());
        assert!(s
            .edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-02", "rest"))
            .is_err());
        assert_eq!(s.schedule(), &snapshot);
        // Marking a date what it already is: no step.
        let undo_len_before = s.can_redo();
        s.edit(|x| set_day_kind(x, x.boards[0].sub_boards[0].id.0, "2027-03-01", "work"))
            .unwrap();
        assert_eq!(s.can_redo(), undo_len_before);
        assert_eq!(s.schedule(), &snapshot);
    }

    /// CAL-06 (behavior note 24): a strip dropped on a date's drop place goes to the end
    /// of that day's strips, from an earlier day or a later one; the day it left stays,
    /// and each drop is one undo step.
    #[test]
    fn a_strip_dropped_on_a_date_goes_to_the_end_of_that_day() {
        let mut s = harbor();
        let sub = s.schedule().boards[0].sub_boards[0].id.0;
        let board = s.schedule().boards[0].id.0;
        let month = |s: &Session| {
            crate::view::calendar_view(s.schedule(), sub, Some(2027), Some(3)).unwrap()
        };
        let on = |c: &crate::view::CalendarDto, date: &str| {
            c.weeks
                .iter()
                .flatten()
                .find(|d| d.date == date)
                .unwrap()
                .clone()
        };
        let before = month(&s);
        let snapshot = s.schedule().clone();
        let days_before = dates(&s).len();
        let first = on(&before, "2027-03-01").strips[0].strip;
        let third = on(&before, "2027-03-03");
        let at = third.drop_at.unwrap();
        // Forward: from day 1 to the end of day 3.
        s.edit(|x| board::move_strips(x, board, &[first], Some(sub), at))
            .unwrap();
        let after = month(&s);
        let day3: Vec<u32> = on(&after, "2027-03-03")
            .strips
            .iter()
            .map(|x| x.strip)
            .collect();
        assert_eq!(day3.last(), Some(&first));
        assert_eq!(day3.len(), third.strips.len() + 1);
        assert_eq!(dates(&s).len(), days_before, "the day it left stays");
        // Back: from day 3 to the end of day 1.
        let at = on(&after, "2027-03-01").drop_at.unwrap();
        s.edit(|x| board::move_strips(x, board, &[first], Some(sub), at))
            .unwrap();
        let back = month(&s);
        assert_eq!(
            on(&back, "2027-03-01").strips.last().map(|x| x.strip),
            Some(first)
        );
        s.undo();
        s.undo();
        assert_eq!(s.schedule(), &snapshot);
    }

    #[test]
    fn the_view_switches_are_kept_in_the_schedule() {
        use method_schedule_core::model::CalendarShow;
        let mut s = harbor();
        let sub = s.schedule().boards[0].sub_boards[0].id.0;
        let all = crate::view::calendar_view(s.schedule(), sub, Some(2027), Some(3)).unwrap();
        assert!(all.hidden.is_empty());
        assert_eq!(all.shows.len(), CalendarShow::ALL.len());
        s.edit(|x| set_calendar_hidden(x, &["strips".into(), "pages".into(), "pages".into()]))
            .unwrap();
        assert_eq!(
            s.schedule().calendar_hidden,
            [CalendarShow::Pages, CalendarShow::Strips]
        );
        let v = crate::view::calendar_view(s.schedule(), sub, Some(2027), Some(3)).unwrap();
        assert_eq!(v.hidden, ["pages", "strips"]);
        assert!(s
            .edit(|x| set_calendar_hidden(x, &["weather".into()]))
            .is_err());
        // All on again writes nothing; Undo brings the pick back, one step each.
        s.edit(|x| set_calendar_hidden(x, &[])).unwrap();
        assert!(s.schedule().calendar_hidden.is_empty());
        s.undo();
        assert_eq!(s.schedule().calendar_hidden.len(), 2);
        s.undo();
        assert!(s.schedule().calendar_hidden.is_empty());
    }

    #[test]
    fn the_weeks_view_runs_through_the_schedule() {
        let mut s = harbor();
        let sub = s.schedule().boards[0].sub_boards[0].id.0;
        let mut late = input("visual_effects", "2027-06-01", "2027-06-03", vec![]);
        late.name = "Plates".into();
        s.edit(|x| save_event(x, sub, None, &late)).unwrap();
        let opts = crate::view::CalOpts {
            span: true,
            ..Default::default()
        };
        let v =
            crate::view::calendar_view_with(s.schedule(), sub, Some(2027), Some(3), &opts).unwrap();
        let days: Vec<_> = v.weeks.iter().flatten().collect();
        assert!(v.span && days.iter().all(|d| d.in_month));
        assert_eq!(
            days.first().unwrap().date,
            "2027-02-28",
            "the Sunday before the 1st"
        );
        assert!(days.iter().any(|d| d.date == "2027-06-03"));
        assert_eq!(days.len() % 7, 0);
        assert!(v.weeks.len() > 6 && v.title.starts_with("February 2027 to June 2027"));
        // The shoot days carry their scene counts.
        let first = days.iter().find(|d| d.shoot_day == Some(1)).unwrap();
        assert_eq!(first.scenes as usize, first.strips.len());
        assert_eq!(v.outside_events, 0);
        // The month view is as before: six weeks.
        let m = crate::view::calendar_view(s.schedule(), sub, Some(2027), Some(3)).unwrap();
        assert!(!m.span && m.weeks.len() == 6 && m.title == "March 2027");
        assert_eq!(m.outside_events, 0);
        // CAL-14: an event years away (a mistyped year) is counted, not drawn, and the
        // weeks still start at the shoot.
        let mut typo = input("fitting", "2025-03-02", "2025-03-03", vec![]);
        typo.name = "Typo".into();
        s.edit(|x| save_event(x, sub, None, &typo)).unwrap();
        let mut far = input("visual_effects", "2029-06-01", "2029-06-02", vec![]);
        far.name = "Far".into();
        s.edit(|x| save_event(x, sub, None, &far)).unwrap();
        let v =
            crate::view::calendar_view_with(s.schedule(), sub, Some(2027), Some(3), &opts).unwrap();
        let days: Vec<_> = v.weeks.iter().flatten().collect();
        assert_eq!(v.outside_events, 2);
        assert_eq!(days.first().unwrap().date, "2027-02-28");
        assert!(days.iter().any(|d| d.date == "2027-06-03"), "the near one");
        assert!(days.iter().any(|d| d.shoot_day == Some(1)));
        assert!(!days.iter().any(|d| d.date.starts_with("2025")));
        // An event just within 26 weeks of the shoot still widens it.
        let mut near = input("rehearsal", "2026-09-07", "2026-09-07", vec![]);
        near.name = "Near".into();
        s.edit(|x| save_event(x, sub, None, &near)).unwrap();
        let v =
            crate::view::calendar_view_with(s.schedule(), sub, Some(2027), Some(3), &opts).unwrap();
        assert_eq!(v.outside_events, 2);
        assert_eq!(v.weeks[0][0].date, "2026-09-06");
    }

    #[test]
    fn the_weekly_days_off() {
        let mut s = harbor();
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        s.edit(|x| board::set_start_date(x, sb, Some("2027-03-04")))
            .unwrap();
        assert_eq!(dates(&s)[2].as_deref(), Some("2027-03-08"));
        s.edit(|x| set_weekly_off(x, x.boards[0].sub_boards[0].id.0, &["sun".into()]))
            .unwrap();
        assert_eq!(
            dates(&s)[2].as_deref(),
            Some("2027-03-06"),
            "a six-day week"
        );
        let snapshot = s.schedule().clone();
        let all: Vec<String> = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
            .map(String::from)
            .to_vec();
        let err = s
            .edit(|x| set_weekly_off(x, x.boards[0].sub_boards[0].id.0, &all))
            .unwrap_err();
        assert!(err.contains("at least one"), "{err}");
        assert!(s
            .edit(|x| set_weekly_off(x, x.boards[0].sub_boards[0].id.0, &["Sunday".into()]))
            .is_err());
        assert_eq!(s.schedule(), &snapshot);
        // One undo step brings the five-day week back, and redo the six-day one.
        assert!(s.undo());
        assert_eq!(dates(&s)[2].as_deref(), Some("2027-03-08"));
        assert!(s.redo());
        assert_eq!(s.schedule(), &snapshot);
    }

    #[test]
    fn events_are_saved_checked_and_deleted() {
        let mut s = harbor();
        let kit = s
            .schedule()
            .elements
            .iter()
            .find(|e| e.name == "KIT")
            .unwrap()
            .id
            .0;
        let i = s
            .edit(|x| {
                save_event(
                    x,
                    x.boards[0].sub_boards[0].id.0,
                    None,
                    &input("fitting", "2027-02-25", "2027-02-26", vec![kit, kit]),
                )
            })
            .unwrap();
        assert_eq!(i, 0);
        let ev = s.schedule().boards[0].sub_boards[0].calendar.events[0].clone();
        assert_eq!(ev.kind, EventKind::Fitting);
        assert_eq!(ev.elements, [ElementId(kit)]);
        // The fitting counts in the day out of days, before the start.
        let grid = crate::view::dood_grid(s.schedule(), crate::view::first_sub(s.schedule()), None)
            .unwrap();
        assert_eq!(grid.dates[0].date, "2027-02-25");
        let mut other = input("other", "2027-03-01", "2027-03-01", vec![]);
        other.label = " Camera test ".into();
        s.edit(|x| save_event(x, x.boards[0].sub_boards[0].id.0, Some(0), &other))
            .unwrap();
        assert_eq!(
            s.schedule().boards[0].sub_boards[0].calendar.events[0].kind,
            EventKind::Other("Camera test".into())
        );
        // A post-production kind and a color (decision 109): shown, never counted.
        let mut vfx = input("visual_effects", "2027-03-01", "2027-03-02", vec![kit]);
        vfx.color = Some("teal".into());
        s.edit(|x| save_event(x, x.boards[0].sub_boards[0].id.0, None, &vfx))
            .unwrap();
        let saved = s.schedule().boards[0].sub_boards[0].calendar.events[1].clone();
        assert_eq!(
            saved.kind,
            EventKind::Post(method_schedule_core::model::PostKind::VisualEffects)
        );
        assert_eq!(saved.color, Some(4));
        assert_eq!(saved.kind.code(), None);
        let cal = crate::view::calendar_view(
            s.schedule(),
            s.schedule().boards[0].sub_boards[0].id.0,
            Some(2027),
            Some(3),
        )
        .unwrap();
        assert_eq!(cal.events[1].color, Some("teal"));
        assert!(!cal.events[1].counts);
        assert!(cal
            .event_kinds
            .iter()
            .any(|(k, l)| *k == "sound_mixing" && *l == "Sound mixing"));
        assert_eq!(cal.event_colors.len(), 10);
        let mut bad_color = vfx.clone();
        bad_color.color = Some("mauve".into());
        let err = s
            .edit(|x| save_event(x, x.boards[0].sub_boards[0].id.0, None, &bad_color))
            .unwrap_err();
        assert!(err.contains("teal"), "{err}");
        s.edit(|x| delete_event(x, x.boards[0].sub_boards[0].id.0, 1))
            .unwrap();
        let snapshot = s.schedule().clone();
        for bad in [
            input("party", "2027-03-01", "2027-03-01", vec![]),
            input("travel", "2027-03-02", "2027-03-01", vec![]),
            input("travel", "2027-03-01", "2207-03-01", vec![]),
            input("travel", "3/1/27", "2027-03-01", vec![]),
            input("travel", "2027-03-01", "2027-03-01", vec![999_999]),
        ] {
            let err = s
                .edit(|x| save_event(x, x.boards[0].sub_boards[0].id.0, None, &bad))
                .unwrap_err();
            assert!(err.ends_with('.') && !err.contains('!'), "{err}");
        }
        assert!(s
            .edit(|x| save_event(
                x,
                x.boards[0].sub_boards[0].id.0,
                Some(3),
                &input("travel", "2027-03-01", "2027-03-01", vec![])
            ))
            .is_err());
        assert!(s
            .edit(|x| delete_event(x, x.boards[0].sub_boards[0].id.0, 3))
            .is_err());
        assert_eq!(s.schedule(), &snapshot);
        s.edit(|x| delete_event(x, x.boards[0].sub_boards[0].id.0, 0))
            .unwrap();
        assert!(s.schedule().boards[0].sub_boards[0]
            .calendar
            .events
            .is_empty());
        s.undo();
        assert_eq!(s.schedule(), &snapshot);
    }
}
