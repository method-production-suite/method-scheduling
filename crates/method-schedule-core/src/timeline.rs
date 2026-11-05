//! The running timeline of a shoot day (decisions.md, BYD-04): from the day's call time,
//! each scene and banner in board order takes its shoot time, giving a clock time for
//! every strip, the wrap and the day's length, with warnings for a long day and for
//! exterior day scenes planned after sunset or before sunrise.
//!
//! A day with no call time still gets a timeline, counted from the call (`+1:30`); the
//! sun warnings need a call time, a date and a located scene. A scene with no shoot time
//! takes no time and is counted in `untimed`. Banners take their duration.
//!
//! What runs past a day's wrap (its call plus the long day limit) can be pushed to the
//! start of the next shoot day: [`past_wrap`] lists the strips, [`push_past_wrap`] moves
//! them (decision 192).

use crate::days::days_with;
use crate::model::{
    BoardId, Error, Place, Schedule, Sheet, SheetId, StripId, StripKind, SubBoardId,
};
use crate::plan::{clock_text, hours_text};
use crate::solar::{sun, Sun};
use crate::Date;

/// Sunrise and sunset for a day, and the location they are for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaySun {
    pub location: String,
    pub sun: Sun,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Scene(SheetId),
    Banner(String),
}

/// One strip on the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub strip: StripId,
    pub kind: EntryKind,
    /// Minutes after midnight when the day has a call time, else minutes after the call.
    pub start: i32,
    pub end: i32,
    /// For a scene: no shoot time was given, so it takes none.
    pub untimed: bool,
    /// A later part of a scene spanning days: it takes no time and counts no pages
    /// (decision 76).
    pub later: bool,
    /// What is wrong with this strip's time, in plain words.
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayTimeline {
    pub number: u32,
    pub date: Option<Date>,
    /// The day break that closes the day (call times are kept by it).
    pub day: StripId,
    /// Minutes after midnight.
    pub call: Option<u32>,
    /// Call plus the day's length: minutes after midnight (past 1440 runs into the next
    /// day), or after the call when there is no call time.
    pub wrap: i32,
    /// Shoot times and banner durations added up.
    pub length: u32,
    pub sun: Option<DaySun>,
    pub entries: Vec<Entry>,
    /// Scenes with no shoot time.
    pub untimed: usize,
    /// The day's warnings, then each strip's, in plain words.
    pub warnings: Vec<String>,
}

impl DayTimeline {
    /// A time on this day as text: a clock time with a call time, else `+h:mm`.
    pub fn time_text(&self, minutes: i32) -> String {
        time_text(self.call, minutes)
    }
}

/// A time on a day with call time `call` as text: a clock time, `(+1)` after midnight,
/// or `+h:mm` after the call when the day has no call time.
pub fn time_text(call: Option<u32>, minutes: i32) -> String {
    if call.is_some() {
        let next = if minutes >= 24 * 60 { " (+1)" } else { "" };
        format!("{}{next}", clock_text(minutes))
    } else {
        format!("+{}", hours_text(minutes.max(0) as u32))
    }
}

/// Whether a scene needs daylight: an exterior written as day, morning or afternoon.
pub fn needs_daylight(h: &Sheet) -> bool {
    let ie = h.int_ext.to_ascii_uppercase();
    let tod = h.time_of_day.to_ascii_uppercase();
    ie.contains("EXT")
        && !tod.contains("NIGHT")
        && ["DAY", "MORNING", "AFTERNOON", "NOON"]
            .iter()
            .any(|w| tod.contains(w))
}

/// The sun on `date` at the sheet's location, when the plan places it on the map.
pub fn sheet_sun(s: &Schedule, h: &Sheet, date: Date) -> Option<DaySun> {
    let loc = s.plan.location(&h.location)?;
    Some(DaySun {
        location: loc.name.clone(),
        sun: sun(date, loc.latitude, loc.longitude, loc.offset_on(date)),
    })
}

/// The day's sun: at the first scene whose location is on the map.
pub fn day_sun(s: &Schedule, sheets: &[SheetId], date: Option<Date>) -> Option<DaySun> {
    let date = date?;
    sheets
        .iter()
        .filter_map(|id| s.sheet(*id))
        .find_map(|h| sheet_sun(s, h, date))
}

/// A day break's earliest sunrise and latest sunset, in minutes after midnight
/// (decision 167): each sheet's typed time, else, with a date, the time worked out at its
/// location when the plan places that on the map. A typed time wins over the worked-out
/// one, field by field. A sun that stays up or down all day, or a time past either
/// midnight, gives nothing.
pub fn day_break_sun(
    s: &Schedule,
    sheets: &[&Sheet],
    date: Option<Date>,
) -> (Option<u32>, Option<u32>) {
    // Each location's sun is worked out once a day.
    let mut known: Vec<(&str, Option<(u32, u32)>)> = Vec::new();
    let mut worked = |h: &Sheet| -> Option<(u32, u32)> {
        let date = date?;
        let loc = s.plan.location(&h.location)?;
        if let Some((_, t)) = known.iter().find(|(n, _)| *n == loc.name) {
            return *t;
        }
        let t = match sun(date, loc.latitude, loc.longitude, loc.offset_on(date)) {
            Sun::Times { rise, set } => {
                let day = |m: i32| u32::try_from(m).ok().filter(|m| *m < 24 * 60);
                day(rise).zip(day(set))
            }
            Sun::AlwaysUp | Sun::AlwaysDown => None,
        };
        known.push((&loc.name, t));
        t
    };
    let (mut rise, mut set): (Option<u32>, Option<u32>) = (None, None);
    for h in sheets {
        let here = if h.sunrise.is_none() || h.sunset.is_none() {
            worked(h)
        } else {
            None
        };
        if let Some(r) = h.sunrise.or(here.map(|t| t.0)) {
            rise = Some(rise.map_or(r, |x| x.min(r)));
        }
        if let Some(t) = h.sunset.or(here.map(|t| t.1)) {
            set = Some(set.map_or(t, |x| x.max(t)));
        }
    }
    (rise, set)
}

/// Every shoot day's timeline on a sub-board.
pub fn timelines(s: &Schedule, board: BoardId, sub: SubBoardId) -> Result<Vec<DayTimeline>, Error> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let mut out = Vec::new();
    let index = s.sheet_index();
    for d in days_with(s, &index, board, sub)?.days {
        let date = sb
            .start
            .and_then(|st| sb.calendar.date_of_day(st, d.number));
        let call = s.plan.call_time(d.break_id);
        let base = call.map_or(0, |c| c as i32);
        let mut t = base;
        let mut entries = Vec::new();
        let mut untimed = 0;
        let mut sheets = Vec::new();
        // An omitted scene isn't shot (decision 91): not on the timeline or call sheet.
        for e in d.entries.iter().filter(|e| !e.omitted) {
            let st = &sb.strips[e.at];
            let (kind, minutes, no_time) = match &st.kind {
                StripKind::Sheet(sh) if !e.first => {
                    // A later part of a scene spanning days takes no time, as on its
                    // day break (decision 76).
                    sheets.push(*sh);
                    (EntryKind::Scene(*sh), 0, false)
                }
                StripKind::Sheet(sh) => {
                    sheets.push(*sh);
                    let h = index.get(sh);
                    let m = h.and_then(|h| h.shoot_time).map(|m| m.0);
                    (EntryKind::Scene(*sh), m.unwrap_or(0), m.is_none())
                }
                StripKind::Banner(bn) => (EntryKind::Banner(bn.text.clone()), bn.duration.0, false),
                StripKind::DayBreak => continue,
            };
            if no_time {
                untimed += 1;
            }
            let start = t;
            t += minutes as i32;
            entries.push(Entry {
                strip: st.id,
                kind,
                start,
                end: t,
                untimed: no_time,
                later: e.sheet.is_some() && !e.first,
                warnings: Vec::new(),
            });
        }
        let length = (t - base) as u32;
        let sun_today = day_sun(s, &sheets, date);
        let mut day = DayTimeline {
            number: d.number,
            date,
            day: d.break_id,
            call,
            wrap: t,
            length,
            sun: sun_today,
            entries,
            untimed,
            warnings: Vec::new(),
        };
        let limit = s.plan.long_day();
        if length > limit {
            day.warnings.push(format!(
                "Day {} runs {}, past the {} long day limit.",
                d.number,
                hours_text(length),
                hours_text(limit)
            ));
        }
        if untimed > 0 {
            let what = if untimed == 1 {
                "1 scene has no shoot time, so the timeline gives it".to_string()
            } else {
                format!("{untimed} scenes have no shoot time, so the timeline gives them")
            };
            day.warnings.push(format!("Day {}: {what} none.", d.number));
        }
        if let (Some(_), Some(date)) = (call, date) {
            for i in 0..day.entries.len() {
                let EntryKind::Scene(id) = day.entries[i].kind else {
                    continue;
                };
                let Some(h) = index.get(&id).copied() else {
                    continue;
                };
                if !needs_daylight(h) {
                    continue;
                }
                let here = sheet_sun(s, h, date).or_else(|| day.sun.clone());
                let Some(DaySun { sun, .. }) = here else {
                    continue;
                };
                let (start, end) = (day.entries[i].start, day.entries[i].end);
                let scene = scene_name(h);
                let w = match sun {
                    Sun::Times { set, .. } if end > set => Some(format!(
                        "{scene} ({} {}) ends {}, after sunset at {}.",
                        h.int_ext,
                        h.time_of_day,
                        day.time_text(end),
                        clock_text(set)
                    )),
                    Sun::Times { rise, .. } if start < rise => Some(format!(
                        "{scene} ({} {}) starts {}, before sunrise at {}.",
                        h.int_ext,
                        h.time_of_day,
                        day.time_text(start),
                        clock_text(rise)
                    )),
                    Sun::AlwaysDown => Some(format!(
                        "{scene} ({} {}) is planned on a day the sun does not rise.",
                        h.int_ext, h.time_of_day
                    )),
                    _ => None,
                };
                if let Some(w) = w {
                    day.entries[i].warnings.push(w.clone());
                    day.warnings.push(w);
                }
            }
        }
        out.push(day);
    }
    Ok(out)
}

/// What runs past a shoot day's wrap, to go to the start of the next shoot day
/// (decision 192).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastWrap {
    pub board: BoardId,
    pub sub_board: SubBoardId,
    /// The day's number, from 1.
    pub day: u32,
    pub call: Option<u32>,
    /// The latest wrap: the call plus the long day limit, in the day's minutes (after
    /// midnight with a call time, else after the call).
    pub wrap_by: i32,
    /// The strips that would move, in board order: whole strips, from the first that ends
    /// after `wrap_by` (the day's first strip always stays) to the day's break, omitted
    /// scenes between them included. Empty when nothing runs past.
    pub strips: Vec<StripId>,
    /// The scenes and banners among them as the timeline names them, in order.
    pub names: Vec<String>,
    /// The next shoot day's number, or `None` when this is the sub-board's last day.
    pub next: Option<u32>,
    /// The day break the strips would follow: the start of the next day.
    pub after: StripId,
}

impl PastWrap {
    /// One line for the command line and the app: what would move, or why nothing would.
    pub fn text(&self) -> String {
        let by = time_text(self.call, self.wrap_by);
        if self.strips.is_empty() {
            return format!("Nothing on day {} runs past {by}.", self.day);
        }
        let what = if self.names.len() == 1 {
            "1 strip runs".to_string()
        } else {
            format!("{} strips run", self.names.len())
        };
        match self.next {
            Some(n) => format!(
                "{what} past {by} on day {}: {}. They would move to the start of day {n}.",
                self.day,
                self.names.join(", ")
            ),
            None => format!(
                "{what} past {by} on day {}, the last shoot day: add a day break after it first.",
                self.day
            ),
        }
    }
}

/// What on shoot day `day` of a sub-board runs past its wrap: the call (or the start of
/// the day, without one) plus the long day limit. Changes nothing.
pub fn past_wrap(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
    day: u32,
) -> Result<PastWrap, Error> {
    past_wrap_in(s, board, sub, &timelines(s, board, sub)?, day)
}

/// [`past_wrap`] from the sub-board's timelines already worked out, so a caller showing
/// every day works them out once.
pub fn past_wrap_in(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
    all: &[DayTimeline],
    day: u32,
) -> Result<PastWrap, Error> {
    let t = all
        .iter()
        .find(|t| t.number == day)
        .ok_or_else(|| Error::Broken(format!("there is no shoot day {day}")))?;
    let wrap_by = t.call.map_or(0, |c| c as i32) + s.plan.long_day() as i32;
    let sb = s
        .board(board)
        .and_then(|b| b.sub_boards.iter().find(|x| x.id == sub))
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let at = |id: StripId| sb.strips.iter().position(|x| x.id == id);
    let mut strips = Vec::new();
    let mut names = Vec::new();
    // The day's first strip stays: a scene longer than the day can't be helped by moving.
    if let Some(i) = t.entries.iter().skip(1).position(|e| e.end > wrap_by) {
        let first = i + 1;
        if let (Some(from), Some(to)) = (at(t.entries[first].strip), at(t.day)) {
            strips = sb.strips[from..to].iter().map(|x| x.id).collect();
        }
        names = t.entries[first..]
            .iter()
            .map(|e| match &e.kind {
                EntryKind::Scene(id) => s.sheet(*id).map_or_else(String::new, scene_name),
                EntryKind::Banner(text) => text.clone(),
            })
            .collect();
    }
    Ok(PastWrap {
        board,
        sub_board: sub,
        day,
        call: t.call,
        wrap_by,
        strips,
        names,
        next: all.iter().any(|x| x.number == day + 1).then_some(day + 1),
        after: t.day,
    })
}

/// Moves what runs past shoot day `day`'s wrap to the start of the next shoot day on the
/// same sub-board, in the same order; day breaks stay. `seen` is the list [`past_wrap`]
/// gave: the move is refused when that is no longer what would move. Returns how many
/// strips moved.
pub fn push_past_wrap(
    s: &mut Schedule,
    board: BoardId,
    sub: SubBoardId,
    day: u32,
    seen: &[StripId],
) -> Result<usize, Error> {
    let p = past_wrap(s, board, sub, day)?;
    if p.strips != seen {
        return Err(Error::Broken(
            "the day changed since the preview; look at what runs past wrap again".to_string(),
        ));
    }
    if p.strips.is_empty() || p.next.is_none() {
        return Err(Error::Broken(p.text()));
    }
    let index = s
        .board(board)
        .and_then(|b| b.sub_boards.iter().find(|x| x.id == sub))
        .and_then(|sb| sb.strips.iter().position(|x| x.id == p.after))
        .ok_or(Error::NoSuchStrip(p.after))?;
    s.move_strips(
        board,
        &p.strips,
        Place {
            sub_board: Some(sub),
            index: index + 1,
        },
    )?;
    Ok(p.strips.len())
}

/// `Sc 12`, or the sheet's heading when it has no scene number (`Sheet::title`; never
/// the sheet's id, decision 162).
pub fn scene_name(h: &Sheet) -> String {
    if h.scenes.trim().is_empty() {
        h.title()
    } else {
        format!("Sc {}", h.scenes.trim())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Banner, Minutes, Place, Sheet};
    use crate::plan::{set_call_time, GeoLocation};

    /// Two days in Los Angeles in June 2024: day 1 has an exterior day scene running past
    /// sunset, day 2 a long day.
    pub(crate) fn la() -> (Schedule, BoardId, SubBoardId) {
        let mut s = Schedule::new("ZTEST Timeline");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        s.set_start(b, sb, "2024-06-20".parse().ok()).unwrap();
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        let bo = s.add_element(cast, "BO").unwrap();
        let mk = |s: &mut Schedule, n: &str, ie: &str, tod: &str, min: Option<u32>, loc: &str| {
            let id = s.add_sheet(Sheet {
                scenes: n.into(),
                int_ext: ie.into(),
                set: format!("SET {n}"),
                time_of_day: tod.into(),
                location: loc.into(),
                shoot_time: min.map(Minutes),
                ..Sheet::default()
            });
            id
        };
        let s1 = mk(&mut s, "1", "INT", "DAY", Some(600), "Stage 5");
        let s2 = mk(&mut s, "2", "EXT", "DAY", Some(120), "Pier");
        mk(&mut s, "3", "EXT", "NIGHT", None, "Pier");
        let s4 = mk(&mut s, "4", "INT", "DAY", Some(800), "Stage 5");
        s.tag(s1, ann).unwrap();
        s.tag(s2, ann).unwrap();
        s.tag(s4, ann).unwrap();
        s.tag(s4, bo).unwrap();
        let strips: Vec<StripId> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &strips,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let at = |i| Place {
            sub_board: Some(sb),
            index: i,
        };
        s.insert_strip(
            b,
            StripKind::Banner(Banner {
                text: "Lunch".into(),
                duration: Minutes(60),
                style: Default::default(),
            }),
            at(1),
        )
        .unwrap();
        let d1 = s.insert_strip(b, StripKind::DayBreak, at(4)).unwrap();
        let d2 = s.insert_strip(b, StripKind::DayBreak, at(6)).unwrap();
        set_call_time(&mut s, d1, Some(7 * 60)).unwrap();
        set_call_time(&mut s, d2, Some(6 * 60)).unwrap();
        s.plan
            .set_location(GeoLocation {
                name: "Pier".into(),
                latitude: 34_052_200,
                longitude: -118_243_700,
                utc_offset: -420,
                daylight_saving: None,
                periods: Vec::new(),
            })
            .unwrap();
        (s, b, sb)
    }

    #[test]
    fn clock_times_wrap_and_warnings() {
        let (s, b, sb) = la();
        let t = timelines(&s, b, sb).unwrap();
        assert_eq!(t.len(), 2);
        let d1 = &t[0];
        assert_eq!(d1.call, Some(420));
        // 10:00 of scene 1, an hour's lunch, 2:00 of scene 2, scene 3 with no time.
        assert_eq!(d1.length, 600 + 60 + 120);
        assert_eq!(d1.wrap, 420 + 780);
        assert_eq!(d1.time_text(d1.entries[2].start), "18:00");
        assert_eq!(d1.untimed, 1);
        // The day's sun is at the pier (stage 5 is not on the map).
        let sun = d1.sun.as_ref().unwrap();
        assert_eq!(sun.location, "Pier");
        // Scene 2 runs 18:00 to 20:00; the sun sets about 20:07: no warning yet.
        assert!(d1.entries[2].warnings.is_empty(), "{:?}", d1.warnings);
        assert!(d1.warnings.iter().any(|w| w.contains("no shoot time")));
        // Day 2: 13:20 from a 6:00 call is a long day.
        let d2 = &t[1];
        assert_eq!(d2.length, 800);
        assert!(d2.warnings[0].contains("past the 12:00 long day limit"));
    }

    #[test]
    fn exterior_day_scene_after_sunset() {
        let (mut s, b, sb) = la();
        let day1 = s.plan.call_times[0].day;
        set_call_time(&mut s, day1, Some(8 * 60 + 30)).unwrap();
        let t = timelines(&s, b, sb).unwrap();
        let e = &t[0].entries[2];
        assert_eq!(e.end, 8 * 60 + 30 + 780);
        assert!(e.warnings[0].starts_with("Sc 2 (EXT DAY) ends 21:30, after sunset at 20:0"));
        // Without a call time: relative times, no sun warnings.
        crate::plan::set_call_time(&mut s, day1, None).unwrap();
        let t = timelines(&s, b, sb).unwrap();
        assert_eq!(t[0].time_text(t[0].entries[2].start), "+11:00");
        assert!(t[0].entries[2].warnings.is_empty());
    }

    /// Decision 193: a location with a daylight saving rule keeps its standard offset;
    /// the timeline, the day breaks and the call sheet take the summer one in June.
    #[test]
    fn the_sun_follows_a_locations_daylight_saving_rule() {
        let (mut s, b, sb) = la();
        let june = timelines(&s, b, sb).unwrap()[0].sun.clone();
        let date: Option<Date> = "2024-06-20".parse().ok();
        let pier = Sheet {
            location: "Pier".into(),
            ..Sheet::default()
        };
        let typed = day_break_sun(&s, &[&pier], date);
        let mut loc = s.plan.locations[0].clone();
        loc.utc_offset = -480;
        loc.daylight_saving = Some(crate::solar::DaylightSaving::UsCanada);
        s.plan.set_location(loc.clone()).unwrap();
        assert_eq!(timelines(&s, b, sb).unwrap()[0].sun, june);
        assert_eq!(day_break_sun(&s, &[&pier], date), typed);
        let c = crate::callsheet::call_sheet(&s, b, sb, 1).unwrap();
        assert_eq!(c.sun, june.map(|x| x.sun));
        // Without the rule the standard offset is an hour behind.
        loc.daylight_saving = None;
        s.plan.set_location(loc).unwrap();
        let (rise, set) = day_break_sun(&s, &[&pier], date);
        assert_eq!(
            (rise.unwrap() + 60, set.unwrap() + 60),
            (typed.0.unwrap(), typed.1.unwrap())
        );
    }

    /// Decision 192: what runs past the wrap goes, whole strips in order, to the start of
    /// the next day; the day breaks and their call times stay.
    #[test]
    fn what_runs_past_wrap_moves_to_the_next_day() {
        let (mut s, b, sb) = la();
        let ids = |s: &Schedule| -> Vec<StripId> {
            s.boards[0].sub_boards[0]
                .strips
                .iter()
                .map(|x| x.id)
                .collect()
        };
        let before = ids(&s);
        // Day 1 calls at 7:00, so it wraps by 19:00: scene 2 (18:00 to 20:00) runs past,
        // and scene 3 after it; scene 1 and lunch stay.
        let p = past_wrap(&s, b, sb, 1).unwrap();
        assert_eq!(p.wrap_by, 19 * 60);
        assert_eq!(p.strips, vec![before[2], before[3]]);
        assert_eq!(p.names, ["Sc 2", "Sc 3"]);
        assert_eq!((p.next, p.after), (Some(2), before[4]));
        assert_eq!(
            p.text(),
            "2 strips run past 19:00 on day 1: Sc 2, Sc 3. They would move to the start of day 2."
        );
        // Without a call time the limit counts from the call: the same strips.
        let day1 = before[4];
        set_call_time(&mut s, day1, None).unwrap();
        assert_eq!(past_wrap(&s, b, sb, 1).unwrap().strips, p.strips);
        set_call_time(&mut s, day1, Some(7 * 60)).unwrap();
        // A list that is no longer what would move is refused, and nothing moves.
        assert!(push_past_wrap(&mut s, b, sb, 1, &p.strips[..1]).is_err());
        assert_eq!(ids(&s), before);
        assert_eq!(push_past_wrap(&mut s, b, sb, 1, &p.strips).unwrap(), 2);
        let after = ids(&s);
        let order = [0, 1, 4, 2, 3, 5, 6].map(|i| before[i]);
        assert_eq!(after, order);
        assert_eq!(s.plan.call_time(day1), Some(7 * 60));
        let t = timelines(&s, b, sb).unwrap();
        assert_eq!(t[0].wrap, 7 * 60 + 660);
        assert_eq!(t[1].entries[0].strip, before[2]);
        // Day 1 now wraps in time: nothing to push, and a push is refused.
        let p = past_wrap(&s, b, sb, 1).unwrap();
        assert!(p.strips.is_empty());
        assert_eq!(p.text(), "Nothing on day 1 runs past 19:00.");
        assert!(push_past_wrap(&mut s, b, sb, 1, &[]).is_err());
        // Day 2 (6:00 call) now runs past 18:00 with scene 4, but it is the last day.
        let p = past_wrap(&s, b, sb, 2).unwrap();
        assert_eq!((p.strips.clone(), p.next), (vec![before[5]], None));
        assert!(p
            .text()
            .ends_with("the last shoot day: add a day break after it first."));
        let err = push_past_wrap(&mut s, b, sb, 2, &p.strips).unwrap_err();
        assert!(err.to_string().contains("last shoot day"), "{err}");
        assert_eq!(ids(&s), order);
        assert!(past_wrap(&s, b, sb, 3).is_err());
    }

    /// Decision 192: a day's first strip stays even when it alone runs past the wrap.
    #[test]
    fn a_days_first_strip_stays() {
        let (mut s, b, sb) = la();
        let d1 = s.boards[0].sub_boards[0].strips[4].id;
        crate::plan::set_long_day(&mut s, Some(8 * 60)).unwrap();
        let p = past_wrap(&s, b, sb, 1).unwrap();
        let strips = &s.boards[0].sub_boards[0].strips;
        assert_eq!(p.strips, vec![strips[1].id, strips[2].id, strips[3].id]);
        assert_eq!(p.names[0], "Lunch");
        assert_eq!(p.after, d1);
    }

    /// One ten-hour, two-page scene on day 1 with a part on day 2 (decision 76).
    pub(crate) fn split_two_days() -> (Schedule, BoardId, SubBoardId, SheetId) {
        let mut s = Schedule::new("T");
        let sheet = s.add_sheet(Sheet {
            scenes: "1".into(),
            shoot_time: Some(Minutes(600)),
            pages: crate::Eighths(16),
            ..Sheet::default()
        });
        let board = s.boards[0].id;
        let sub = s.boards[0].sub_boards[0].id;
        let at = |index| Place {
            sub_board: Some(sub),
            index,
        };
        s.move_strips(board, &s.sheet_strips(board, sheet), at(0))
            .unwrap();
        s.insert_strip(board, StripKind::DayBreak, at(1)).unwrap();
        s.add_part(board, sheet, at(2)).unwrap();
        s.insert_strip(board, StripKind::DayBreak, at(3)).unwrap();
        s.set_start(board, sub, Some("2027-03-01".parse().unwrap()))
            .unwrap();
        (s, board, sub, sheet)
    }

    /// Review: the timeline, call sheet and rest counted a later part at full length.
    #[test]
    fn a_later_part_counts_like_its_day_break() {
        let (s, board, sub, _) = split_two_days();
        let d = crate::days::days(&s, board, sub).unwrap();
        let t = timelines(&s, board, sub).unwrap();
        assert_eq!(d.days[0].time.0, t[0].length);
        assert_eq!(d.days[1].time.0, t[1].length, "day 2");
        assert_eq!(t[1].length, 0);
        assert!(t[1].entries[0].later && !t[0].entries[0].later);
        assert_eq!(t[1].untimed, 0);
        let c = crate::callsheet::call_sheet(&s, board, sub, 2).unwrap();
        assert_eq!(c.pages, d.days[1].pages);
        assert_eq!(c.scenes.len(), 1, "the part is still on the call sheet");
        let c = crate::callsheet::call_sheet(&s, board, sub, 1).unwrap();
        assert_eq!(c.pages, crate::Eighths(16));
    }

    /// Note 27: a day break's sun came only from typed times; a located sheet now gives
    /// its worked-out times, and a typed time wins.
    #[test]
    fn day_breaks_take_the_located_sun_when_none_is_typed() {
        let (s, _, _) = la();
        let date: Option<Date> = "2024-06-20".parse().ok();
        let pier = Sheet {
            location: "Pier".into(),
            ..Sheet::default()
        };
        let stage = Sheet {
            location: "Stage 5".into(),
            ..Sheet::default()
        };
        let Sun::Times { rise, set } = sun(date.unwrap(), 34_052_200, -118_243_700, -420) else {
            panic!("the sun rises in Los Angeles");
        };
        let (rise, set) = (rise as u32, set as u32);
        assert_eq!(day_break_sun(&s, &[&pier], date), (Some(rise), Some(set)));
        // No date, or no location on the map: nothing.
        assert_eq!(day_break_sun(&s, &[&pier], None), (None, None));
        assert_eq!(day_break_sun(&s, &[&stage], date), (None, None));
        // A typed time wins, field by field.
        let typed = Sheet {
            sunrise: Some(rise + 30),
            ..pier.clone()
        };
        assert_eq!(
            day_break_sun(&s, &[&typed], date),
            (Some(rise + 30), Some(set))
        );
        // Earliest sunrise and latest sunset across the day's sheets.
        let early = Sheet {
            sunrise: Some(300),
            sunset: Some(1000),
            ..stage.clone()
        };
        assert_eq!(
            day_break_sun(&s, &[&typed, &early, &pier], date),
            (Some(300), Some(set))
        );
    }
}
