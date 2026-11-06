//! Planning data the schedule keeps for the features beyond the reference app
//! (docs/research/mms-feature-inventory.md, "Competitor features MMS lacks";
//! decisions.md, the entries for IMP-04 and BYD-01 to BYD-09): where locations are on the map, each shoot day's call
//! time and the long day limit, turnaround minimums per category, schedule locks, pinned
//! strips and scenes a script revision omitted.
//!
//! All of it is saved in the schedule file (docs/FORMAT.md, `plan`) and left out while it
//! is empty, so older files read and write back byte for byte. References to strips,
//! sheets, categories and elements that no longer exist are ignored wherever the data is
//! read, never an error: deleting a strip must never fail because a call time named it.
//! [`tidy`] drops them; the plan's own edits call it.

use std::collections::HashSet;

use crate::model::{
    same_name, BoardId, CategoryId, ElementId, Error, Schedule, SheetId, StripId, StripKind,
    SubBoardId,
};
use crate::solar::DaylightSaving;
use crate::Date;

/// The long day limit when none is set: twelve hours from call to wrap.
pub const DEFAULT_LONG_DAY: u32 = 12 * 60;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Map positions for the sheets' location names, in the order added.
    pub locations: Vec<GeoLocation>,
    /// Each shoot day's call time, by the day break that closes the day.
    pub call_times: Vec<CallTime>,
    /// The day length (call to wrap, in minutes) past which a day is called long. `None`:
    /// [`DEFAULT_LONG_DAY`].
    pub long_day: Option<u32>,
    /// The shortest rest, wrap to next call, for elements of a category.
    pub turnarounds: Vec<Turnaround>,
    /// Saved snapshots of where every scene was, to report drift against.
    pub locks: Vec<Lock>,
    /// Strips the strip order suggestion leaves in place.
    pub pinned: Vec<StripId>,
    /// Sheets a script revision no longer has. Kept, not deleted.
    pub omitted: Vec<SheetId>,
    /// What each shoot day was when it was shot: its date, call and wrap (decision 224;
    /// written by a production report, such as Method Day's).
    pub days_shot: Vec<DayShot>,
    /// What was shot of each scene on each day: eighths, and whether that finished it.
    pub scenes_shot: Vec<SceneShot>,
    /// Calls and wraps of their own for single elements (a cast member called later than
    /// the company, say) on a shoot day; the rest checks use them (decision 231).
    pub person_calls: Vec<PersonCall>,
    /// Each script draft a revision merged, oldest first (decision 233).
    pub drafts: Vec<Draft>,
    /// Merge conflicts settled one way and not yet looked at, kept with the file until
    /// someone marks them done (decision 238).
    pub conflicts: Vec<ReviewConflict>,
    /// Each crew department's call, set once against the company's call (decision 242), in
    /// the order added.
    pub crew_calls: Vec<CrewCall>,
    /// Crew calls for one shoot day only (decision 243): a department's call that differs
    /// that day, or a department called that day alone. Kept by the day break, in the order
    /// added.
    pub day_crew_calls: Vec<DayCrewCall>,
    /// How to get to a location (decision 245): its address, directions and a map picture.
    /// By the sheets' location name, in the order added; a location needn't be on the map.
    pub location_guides: Vec<LocationGuide>,
}

/// How to get to a location, for the call sheet (decision 245). Unique by name, without
/// regard to case; at least one of the other fields is set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocationGuide {
    pub name: String,
    pub address: String,
    /// Several lines are kept.
    pub directions: String,
    /// A picture file the person chose, stored as a path as an element's images are: the
    /// schedule file never holds the picture, and the app never fetches one.
    pub map: Option<String>,
}

/// The most characters in a location's address, and in its directions.
pub const MAX_GUIDE_TEXT: usize = 2000;

/// What a location guide must be, for the setter, the file reader and the check alike.
pub fn guide_ok(g: &LocationGuide) -> Result<(), String> {
    if g.name.trim().is_empty() || g.name != g.name.trim() {
        return Err("a location's directions need its name, with no spaces around it".to_string());
    }
    for (what, text) in [("address", &g.address), ("directions", &g.directions)] {
        if text != text.trim() {
            return Err(format!("the {what} of {:?} has spaces around it", g.name));
        }
        if text.chars().count() > MAX_GUIDE_TEXT {
            return Err(format!(
                "the {what} of {:?} is longer than {MAX_GUIDE_TEXT} characters",
                g.name
            ));
        }
    }
    if g.map.as_deref().is_some_and(|m| m.trim().is_empty()) {
        return Err(format!("the map of {:?} has an empty path", g.name));
    }
    if g.address.is_empty() && g.directions.is_empty() && g.map.is_none() {
        return Err(format!(
            "the directions of {:?} hold nothing: leave them out",
            g.name
        ));
    }
    Ok(())
}

/// A department's call on one shoot day only (decision 243), against that day's company
/// call as in [`CrewCall`]. Unique by day and department, without regard to case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayCrewCall {
    /// The day break that closes the day, as in [`CallTime`].
    pub day: StripId,
    pub department: String,
    pub offset: i32,
}

/// A department's call as it holds on one day: [`Plan::crew_on`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrewOnDay {
    pub department: String,
    pub offset: i32,
    /// Set for this day only, not the every-day call.
    pub this_day: bool,
}

/// A crew department's call (decision 242): so long before (negative) or after the
/// company's call on every shoot day, or with it (0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrewCall {
    /// One line, unique without regard to case, at most [`MAX_DEPARTMENT`] characters.
    pub department: String,
    /// Minutes from the company's call, [`MAX_CREW_OFFSET`] either way at most.
    pub offset: i32,
}

/// The furthest a crew call may be from the company's: twelve hours.
pub const MAX_CREW_OFFSET: i32 = 12 * 60;
/// The longest department name, in characters.
pub const MAX_DEPARTMENT: usize = 100;
/// The most crew calls a schedule keeps.
pub const MAX_CREW_CALLS: usize = 100;

impl CrewCall {
    /// The department's call on a day called at `call`: minutes after that day's midnight,
    /// below 0 the day before.
    pub fn on(&self, call: u32) -> i32 {
        call as i32 + self.offset
    }
}

/// A merge conflict kept for review (decision 238): what [`crate::merge::Conflict`] said,
/// with the date of the merge. `ours` is the copy merged into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewConflict {
    /// The merge's key: `sheet:12:set`, `element:5:name`. Unique in the list.
    pub key: String,
    pub what: String,
    pub base: Option<String>,
    pub ours: String,
    pub theirs: String,
    /// The side the merge took.
    pub taken: crate::merge::Side,
    pub date: Option<Date>,
}

impl ReviewConflict {
    /// In plain words, as the merge said it, with the side taken.
    pub fn text(&self) -> String {
        let was = self
            .base
            .as_ref()
            .map_or(String::new(), |b| format!(", was {b}"));
        format!(
            "{}: ours {}, theirs {}{was}; {} kept",
            self.what,
            self.ours,
            self.theirs,
            self.taken.key()
        )
    }
}

/// The most conflicts the review list keeps.
pub const MAX_REVIEW_CONFLICTS: usize = 10_000;

/// A script draft merged by a revision (decision 233): its color, when, and what it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// The revision color, `Blue`, `Pink`, ...; unique without regard to case.
    pub color: String,
    pub date: Option<Date>,
    pub changed: Vec<SheetId>,
    pub added: Vec<SheetId>,
    pub omitted: Vec<SheetId>,
}

/// The usual order of revision colors, the original draft first.
pub const REVISION_COLORS: [&str; 9] = [
    "White",
    "Blue",
    "Pink",
    "Yellow",
    "Green",
    "Goldenrod",
    "Buff",
    "Salmon",
    "Cherry",
];

/// The color of revision `n` (1 for the first after the original White): Blue, Pink and on
/// to Cherry, then Double White, Double Blue and on, then Triple, then `White 4`.
pub fn draft_color(n: usize) -> String {
    let c = REVISION_COLORS[n % REVISION_COLORS.len()];
    match n / REVISION_COLORS.len() {
        0 => c.to_string(),
        1 => format!("Double {c}"),
        2 => format!("Triple {c}"),
        r => format!("{c} {}", r + 1),
    }
}

/// One element's own call and wrap on a shoot day (decision 231). Each part is left out
/// (`None`) to follow the company's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersonCall {
    /// The day break that closes the day, as in [`CallTime`].
    pub day: StripId,
    pub element: ElementId,
    /// Their call, minutes after midnight, 0 to 1439.
    pub call: Option<u32>,
    /// Their wrap, minutes after the call day's midnight, up to [`MAX_WRAP`]; after their
    /// call when both are known.
    pub wrap: Option<u32>,
}

/// A shoot day as shot. Each part is left unknown (`None`) until reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayShot {
    /// The day break that closes the day, as in [`CallTime`].
    pub day: StripId,
    /// The date it was shot; may differ from the planned date.
    pub date: Option<Date>,
    /// The actual call, minutes after midnight, 0 to 1439.
    pub call: Option<u32>,
    /// The actual wrap, minutes after the call day's midnight, up to 2880 (past midnight
    /// is past 1440). After the call when both are known.
    pub wrap: Option<u32>,
}

/// The longest a wrap may run: the end of the next day.
pub const MAX_WRAP: u32 = 48 * 60;

/// Part of a scene shot on one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneShot {
    pub sheet: SheetId,
    /// The day break of the day it was shot on, which need not be the day it is planned on.
    pub day: StripId,
    /// How much of the scene was shot that day.
    pub eighths: crate::Eighths,
    /// This day finished the scene. At most one entry per scene.
    pub complete: bool,
}

/// A location name with its place on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeoLocation {
    /// Matched to a sheet's location without regard to case.
    pub name: String,
    /// Millionths of a degree, north positive.
    pub latitude: i32,
    /// Millionths of a degree, east positive.
    pub longitude: i32,
    /// Minutes ahead of UTC: the standard offset when `daylight_saving` has a rule, else
    /// the offset on the shoot dates.
    pub utc_offset: i32,
    /// When the clocks are an hour ahead (decision 193); `None`: never.
    pub daylight_saving: Option<DaylightSaving>,
    /// Dates with a UTC offset of their own, typed from the year's announcement where the
    /// clocks follow a lunar calendar or a decree (decision 244). In date order, not
    /// overlapping, [`MAX_OFFSET_PERIODS`] at most; they win over `daylight_saving`.
    pub periods: Vec<OffsetPeriod>,
}

/// A run of dates with a UTC offset of its own (decision 244): Morocco's clocks during
/// Ramadan, say, or a year's daylight saving set by decree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffsetPeriod {
    /// The first date, and the last (the same for one day).
    pub from: Date,
    pub to: Date,
    /// Minutes ahead of UTC on those dates, 14 hours either way at most.
    pub offset: i32,
}

/// The most offset periods one location keeps.
pub const MAX_OFFSET_PERIODS: usize = 100;

impl GeoLocation {
    /// The UTC offset in effect on `date`: a typed period's where one covers the date,
    /// else the standard one, an hour more while the daylight saving rule is in effect.
    pub fn offset_on(&self, date: Date) -> i32 {
        if let Some(p) = self.periods.iter().find(|p| p.from <= date && date <= p.to) {
            return p.offset;
        }
        match self.daylight_saving {
            Some(rule) if rule.in_effect(date) => self.utc_offset + 60,
            _ => self.utc_offset,
        }
    }
}

/// Whether offset periods may be kept as they are: each ends on or after its start, its
/// offset within 14 hours, in date order without overlapping, [`MAX_OFFSET_PERIODS`] at
/// most.
pub fn periods_ok(periods: &[OffsetPeriod]) -> Result<(), String> {
    if periods.len() > MAX_OFFSET_PERIODS {
        return Err(format!(
            "a location keeps {MAX_OFFSET_PERIODS} periods of its own offset at most"
        ));
    }
    for (i, p) in periods.iter().enumerate() {
        if p.to < p.from {
            return Err(format!("the period from {} ends before it starts", p.from));
        }
        // unsigned_abs: i32::MIN has no i32 absolute value.
        if p.offset.unsigned_abs() > 14 * 60 {
            return Err(format!(
                "the period from {} has a UTC offset past 14 hours",
                p.from
            ));
        }
        if i > 0 && p.from <= periods[i - 1].to {
            return Err(format!(
                "the period from {} overlaps the one before or is out of order",
                p.from
            ));
        }
    }
    Ok(())
}

/// Offset periods as typed: entries split by `;` or line breaks, each `2027-02-07 to
/// 2027-03-14 +00:00` (or one date and an offset), put in date order. Empty for none.
pub fn parse_offset_periods(text: &str) -> Result<Vec<OffsetPeriod>, String> {
    let mut out = Vec::new();
    for entry in text
        .split([';', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        let words: Vec<&str> = entry.split_whitespace().collect();
        let (from, to, offset) = match words.as_slice() {
            [from, to_word, to, offset] if to_word.eq_ignore_ascii_case("to") => {
                (*from, *to, *offset)
            }
            [day, offset] => (*day, *day, *offset),
            _ => {
                return Err(format!(
                    "{entry:?} is not a period such as 2027-02-07 to 2027-03-14 +00:00"
                ))
            }
        };
        let date = |x: &str| {
            x.parse::<Date>()
                .map_err(|_| format!("{x:?} is not a date such as 2027-02-07"))
        };
        out.push(OffsetPeriod {
            from: date(from)?,
            to: date(to)?,
            offset: parse_offset(offset)?,
        });
    }
    out.sort_by_key(|p| p.from);
    periods_ok(&out)?;
    Ok(out)
}

/// Offset periods as [`parse_offset_periods`] reads them back.
pub fn offset_periods_text(periods: &[OffsetPeriod]) -> String {
    periods
        .iter()
        .map(|p| {
            if p.from == p.to {
                format!("{} {}", p.from, offset_text(p.offset))
            } else {
                format!("{} to {} {}", p.from, p.to, offset_text(p.offset))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallTime {
    /// The day break that closes the day.
    pub day: StripId,
    /// Minutes after midnight, 0 to 1439.
    pub minutes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turnaround {
    pub category: CategoryId,
    /// The shortest rest in minutes.
    pub minutes: u32,
}

/// Where every scene of a board was when the lock was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    /// Unique without regard to case.
    pub name: String,
    pub board: BoardId,
    /// When it was made, if the caller knew the date.
    pub made: Option<Date>,
    /// Every scheduled scene, in board order.
    pub scenes: Vec<LockedScene>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockedScene {
    pub sheet: SheetId,
    pub sub_board: SubBoardId,
    /// The shoot day number in its sub-board, from 1.
    pub day: u32,
    /// `None` when the sub-board had no start date.
    pub date: Option<Date>,
    /// The cast on the scene then.
    pub cast: Vec<ElementId>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        *self == Plan::default()
    }

    pub fn location(&self, name: &str) -> Option<&GeoLocation> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        self.locations.iter().find(|l| same_name(&l.name, name))
    }

    /// Adds a location or replaces the one with the same name.
    pub fn set_location(&mut self, loc: GeoLocation) -> Result<(), Error> {
        let name = loc.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if loc.latitude.unsigned_abs() > 90_000_000 {
            return Err(refuse("a latitude is 90 degrees north or south at most"));
        }
        if loc.longitude.unsigned_abs() > 180_000_000 {
            return Err(refuse("a longitude is 180 degrees east or west at most"));
        }
        if loc.utc_offset.unsigned_abs() > 14 * 60 {
            return Err(refuse("a UTC offset is 14 hours at most"));
        }
        periods_ok(&loc.periods).map_err(|e| refuse(&e))?;
        let loc = GeoLocation { name, ..loc };
        match self
            .locations
            .iter_mut()
            .find(|l| same_name(&l.name, &loc.name))
        {
            Some(l) => *l = loc,
            None => self.locations.push(loc),
        }
        Ok(())
    }

    /// Removes a location by name. False when there was none.
    pub fn remove_location(&mut self, name: &str) -> bool {
        let before = self.locations.len();
        self.locations.retain(|l| !same_name(&l.name, name));
        self.locations.len() != before
    }

    /// How to get to a location, by name without regard to case (decision 245).
    pub fn location_guide(&self, name: &str) -> Option<&LocationGuide> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        self.location_guides
            .iter()
            .find(|g| same_name(&g.name, name))
    }

    /// Adds a location's guide or replaces the one with the same name; one with nothing in
    /// it is taken off. Text is trimmed, a blank map path is no map.
    pub fn set_location_guide(&mut self, g: LocationGuide) -> Result<(), Error> {
        let g = LocationGuide {
            name: g.name.trim().to_string(),
            address: g.address.trim().to_string(),
            directions: g.directions.trim().to_string(),
            map: g.map.filter(|m| !m.trim().is_empty()),
        };
        if g.name.is_empty() {
            return Err(Error::EmptyName);
        }
        let at = self
            .location_guides
            .iter()
            .position(|o| same_name(&o.name, &g.name));
        if g.address.is_empty() && g.directions.is_empty() && g.map.is_none() {
            if let Some(at) = at {
                self.location_guides.remove(at);
            }
            return Ok(());
        }
        guide_ok(&g).map_err(|e| refuse(&e))?;
        match at {
            Some(at) => self.location_guides[at] = g,
            None => self.location_guides.push(g),
        }
        Ok(())
    }

    pub fn call_time(&self, day: StripId) -> Option<u32> {
        self.call_times
            .iter()
            .find(|c| c.day == day)
            .map(|c| c.minutes)
    }

    pub fn long_day(&self) -> u32 {
        self.long_day.unwrap_or(DEFAULT_LONG_DAY)
    }

    pub fn turnaround(&self, category: CategoryId) -> Option<u32> {
        self.turnarounds
            .iter()
            .find(|t| t.category == category)
            .map(|t| t.minutes)
    }

    pub fn lock(&self, name: &str) -> Option<&Lock> {
        self.locks.iter().find(|l| same_name(&l.name, name))
    }

    pub fn is_pinned(&self, strip: StripId) -> bool {
        self.pinned.contains(&strip)
    }

    pub fn is_omitted(&self, sheet: SheetId) -> bool {
        self.omitted.contains(&sheet)
    }

    /// The newest draft that changed or added `sheet` (decision 233).
    pub fn revised_in(&self, sheet: SheetId) -> Option<&Draft> {
        self.drafts
            .iter()
            .rev()
            .find(|d| d.changed.contains(&sheet) || d.added.contains(&sheet))
    }

    /// Element `element`'s own call and wrap on the day closed by `day`, if set.
    pub fn person_call(&self, day: StripId, element: ElementId) -> Option<&PersonCall> {
        self.person_calls
            .iter()
            .find(|x| x.day == day && x.element == element)
    }

    /// The crew call of `department`, if set (decision 242).
    pub fn crew_call(&self, department: &str) -> Option<&CrewCall> {
        self.crew_calls
            .iter()
            .find(|x| same_name(&x.department, department))
    }

    /// The crew calls on the day closed by `day` (decision 243): each every-day call in its
    /// order, with that day's own offset where one is set, then the departments called that
    /// day alone.
    pub fn crew_on(&self, day: StripId) -> Vec<CrewOnDay> {
        let own = |name: &str| {
            self.day_crew_calls
                .iter()
                .find(|x| x.day == day && same_name(&x.department, name))
        };
        let mut out: Vec<CrewOnDay> = self
            .crew_calls
            .iter()
            .map(|x| match own(&x.department) {
                Some(d) => CrewOnDay {
                    department: x.department.clone(),
                    offset: d.offset,
                    this_day: true,
                },
                None => CrewOnDay {
                    department: x.department.clone(),
                    offset: x.offset,
                    this_day: false,
                },
            })
            .collect();
        for d in self.day_crew_calls.iter().filter(|x| x.day == day) {
            if self.crew_call(&d.department).is_none() {
                out.push(CrewOnDay {
                    department: d.department.clone(),
                    offset: d.offset,
                    this_day: true,
                });
            }
        }
        out
    }

    /// The day closed by `day` as shot, if reported.
    pub fn day_shot(&self, day: StripId) -> Option<&DayShot> {
        self.days_shot.iter().find(|d| d.day == day)
    }

    /// Everything reported shot of a scene, day by day in the order written.
    pub fn shot_of(&self, sheet: SheetId) -> impl Iterator<Item = &SceneShot> {
        self.scenes_shot.iter().filter(move |x| x.sheet == sheet)
    }

    /// Whether a scene is reported finished.
    pub fn is_complete(&self, sheet: SheetId) -> bool {
        self.shot_of(sheet).any(|x| x.complete)
    }

    /// Eighths reported shot of a scene, over every day.
    pub fn eighths_shot(&self, sheet: SheetId) -> crate::Eighths {
        crate::Eighths(self.shot_of(sheet).map(|x| x.eighths.0).sum())
    }
}

fn refuse(why: &str) -> Error {
    Error::Broken(why.to_string())
}

/// The longest lock name, in characters.
pub const MAX_LOCK_NAME: usize = 200;

/// What `Schedule::check` asks of the plan: what the setters and the file reader refuse
/// is refused here too, so a merge or a tool can't leave it behind. A call time, pin or
/// omitted mark naming something gone is left to [`tidy`].
pub(crate) fn check(s: &Schedule) -> Result<(), Error> {
    let p = &s.plan;
    let broken = |why: String| Err(Error::Broken(format!("plan: {why}")));
    for (i, l) in p.locations.iter().enumerate() {
        if l.name.trim().is_empty() || l.name != l.name.trim() {
            return broken("a location has no name or spaces around it".to_string());
        }
        if p.locations[..i].iter().any(|o| same_name(&o.name, &l.name)) {
            return broken(format!("location {:?} is listed twice", l.name));
        }
        if l.latitude.unsigned_abs() > 90_000_000
            || l.longitude.unsigned_abs() > 180_000_000
            || l.utc_offset.unsigned_abs() > 14 * 60
        {
            return broken(format!(
                "location {:?} is off the map or its UTC offset past 14 hours",
                l.name
            ));
        }
        if let Err(why) = periods_ok(&l.periods) {
            return broken(format!("location {:?}: {why}", l.name));
        }
    }
    for (i, g) in p.location_guides.iter().enumerate() {
        guide_ok(g).or_else(broken)?;
        if p.location_guides[..i]
            .iter()
            .any(|o| same_name(&o.name, &g.name))
        {
            return broken(format!("the directions of {:?} are listed twice", g.name));
        }
    }
    let mut days = HashSet::new();
    for c in &p.call_times {
        if c.minutes >= 24 * 60 {
            return broken(format!("call time {} is past 23:59", c.minutes));
        }
        if !days.insert(c.day) {
            return broken(format!("day {} has two call times", c.day.0));
        }
    }
    let mut shot_days = HashSet::new();
    for d in &p.days_shot {
        if !shot_days.insert(d.day) {
            return broken(format!("day {} is reported shot twice", d.day.0));
        }
        if d.call.is_some_and(|m| m >= 24 * 60) {
            return broken(format!("day {}'s actual call is past 23:59", d.day.0));
        }
        if d.wrap.is_some_and(|m| m == 0 || m > MAX_WRAP)
            || matches!((d.call, d.wrap), (Some(c), Some(w)) if w <= c)
        {
            return broken(format!(
                "day {}'s actual wrap is out of range or not after its call",
                d.day.0
            ));
        }
    }
    let mut people = HashSet::new();
    for x in &p.person_calls {
        if !people.insert((x.day, x.element)) {
            return broken(format!(
                "element {} has two calls of its own on day {}",
                x.element.0, x.day.0
            ));
        }
        if x.call.is_none() && x.wrap.is_none() {
            return broken(format!(
                "element {}'s call of its own on day {} is empty",
                x.element.0, x.day.0
            ));
        }
        if x.call.is_some_and(|m| m >= 24 * 60) {
            return broken(format!("element {}'s call is past 23:59", x.element.0));
        }
        if x.wrap.is_some_and(|m| m == 0 || m > MAX_WRAP)
            || matches!((x.call, x.wrap), (Some(c), Some(w)) if w <= c)
        {
            return broken(format!(
                "element {}'s wrap is out of range or not after its call",
                x.element.0
            ));
        }
    }
    let mut shots = HashSet::new();
    let mut finished = HashSet::new();
    for x in &p.scenes_shot {
        if !shots.insert((x.sheet, x.day)) {
            return broken(format!(
                "sheet {} is reported shot twice on day {}",
                x.sheet.0, x.day.0
            ));
        }
        if x.eighths.0 > crate::model::MAX_SHEET_EIGHTHS {
            return broken(format!("sheet {} reports too many eighths shot", x.sheet.0));
        }
        if x.complete && !finished.insert(x.sheet) {
            return broken(format!("sheet {} is reported finished twice", x.sheet.0));
        }
    }
    if p.long_day.is_some_and(|m| m == 0 || m > 24 * 60) {
        return broken("the long day limit is out of range".to_string());
    }
    if p.crew_calls.len() > MAX_CREW_CALLS {
        return broken(format!("more than {MAX_CREW_CALLS} crew calls"));
    }
    for (i, x) in p.crew_calls.iter().enumerate() {
        if let Err(why) = department_ok(&x.department) {
            return broken(why);
        }
        if x.offset.unsigned_abs() > MAX_CREW_OFFSET.unsigned_abs() {
            return broken(format!(
                "the crew call of {:?} is out of range",
                x.department
            ));
        }
        if p.crew_calls[..i]
            .iter()
            .any(|o| same_name(&o.department, &x.department))
        {
            return broken(format!("{:?} has two crew calls", x.department));
        }
    }
    for (i, x) in p.day_crew_calls.iter().enumerate() {
        if let Err(why) = department_ok(&x.department) {
            return broken(why);
        }
        if x.offset.unsigned_abs() > MAX_CREW_OFFSET.unsigned_abs() {
            return broken(format!(
                "the crew call of {:?} on day {} is out of range",
                x.department, x.day.0
            ));
        }
        let before = &p.day_crew_calls[..i];
        if before
            .iter()
            .any(|o| o.day == x.day && same_name(&o.department, &x.department))
        {
            return broken(format!(
                "{:?} has two crew calls on day {}",
                x.department, x.day.0
            ));
        }
        if before.iter().filter(|o| o.day == x.day).count() >= MAX_CREW_CALLS {
            return broken(format!(
                "day {} has more than {MAX_CREW_CALLS} crew calls of its own",
                x.day.0
            ));
        }
    }
    let mut cats = HashSet::new();
    for t in &p.turnarounds {
        if t.minutes == 0 || t.minutes > 48 * 60 || !cats.insert(t.category) {
            return broken(format!(
                "category {} has a turnaround out of range or two",
                t.category.0
            ));
        }
    }
    for (i, l) in p.locks.iter().enumerate() {
        if l.name.trim().is_empty()
            || l.name.chars().count() > MAX_LOCK_NAME
            || l.name.chars().any(char::is_control)
        {
            return broken(format!(
                "a lock name is one line of 1 to {MAX_LOCK_NAME} characters"
            ));
        }
        if p.locks[..i].iter().any(|o| same_name(&o.name, &l.name)) {
            return broken(format!("two locks are named {:?}", l.name));
        }
    }
    if p.conflicts.len() > MAX_REVIEW_CONFLICTS {
        return broken(format!(
            "more than {MAX_REVIEW_CONFLICTS} merge conflicts to review"
        ));
    }
    for (i, c) in p.conflicts.iter().enumerate() {
        if c.key.trim().is_empty() || c.key.chars().any(char::is_control) {
            return broken("a merge conflict's key is one line".to_string());
        }
        if p.conflicts[..i].iter().any(|o| o.key == c.key) {
            return broken(format!("two merge conflicts are {:?}", c.key));
        }
    }
    for (i, d) in p.drafts.iter().enumerate() {
        if d.color.trim().is_empty()
            || d.color.chars().count() > MAX_LOCK_NAME
            || d.color.chars().any(char::is_control)
        {
            return broken(format!(
                "a draft color is one line of 1 to {MAX_LOCK_NAME} characters"
            ));
        }
        if p.drafts[..i].iter().any(|o| same_name(&o.color, &d.color)) {
            return broken(format!("two drafts are {:?}", d.color));
        }
    }
    // Pins are on scene strips only; a pin on a strip that is gone waits for tidy.
    let kinds: crate::model::IdMap<StripId, &StripKind> = s
        .boards
        .iter()
        .flat_map(|b| {
            b.sub_boards
                .iter()
                .flat_map(|sb| &sb.strips)
                .chain(&b.boneyard)
        })
        .map(|st| (st.id, &st.kind))
        .collect();
    let mut pins = HashSet::new();
    for x in &p.pinned {
        if !pins.insert(*x) {
            return broken(format!("strip {} is pinned twice", x.0));
        }
        if kinds
            .get(x)
            .is_some_and(|k| !matches!(k, StripKind::Sheet(_)))
        {
            return broken(format!("strip {} is pinned but is not a scene", x.0));
        }
    }
    let mut omitted = HashSet::new();
    if let Some(x) = p.omitted.iter().find(|x| !omitted.insert(**x)) {
        return broken(format!("sheet {} is marked omitted twice", x.0));
    }
    Ok(())
}

/// Every day break on any board.
/// Every day break, parked ones in a Boneyard too: a parked break keeps its call time
/// for when it comes back.
fn day_breaks(s: &Schedule) -> HashSet<StripId> {
    s.boards
        .iter()
        .flat_map(|b| {
            b.sub_boards
                .iter()
                .flat_map(|sb| &sb.strips)
                .chain(&b.boneyard)
        })
        .filter(|st| st.kind == StripKind::DayBreak)
        .map(|st| st.id)
        .collect()
}

/// Drops references to things that are gone: call times on day breaks no longer on the
/// board (a parked one keeps its), turnarounds of deleted categories, pins on strips that are gone, omitted
/// marks on deleted sheets, and locks of deleted boards. Locked scenes stay as recorded.
pub fn tidy(s: &mut Schedule) {
    let breaks = day_breaks(s);
    let strips: HashSet<StripId> = s
        .boards
        .iter()
        .flat_map(|b| {
            b.sub_boards
                .iter()
                .flat_map(|sb| &sb.strips)
                .chain(&b.boneyard)
        })
        .map(|st| st.id)
        .collect();
    let sheets: HashSet<SheetId> = s.sheets.iter().map(|h| h.id).collect();
    let elements: HashSet<ElementId> = s.elements.iter().map(|e| e.id).collect();
    let cats: HashSet<CategoryId> = s.categories.iter().map(|c| c.id).collect();
    let boards: HashSet<BoardId> = s.boards.iter().map(|b| b.id).collect();
    let p = &mut s.plan;
    p.call_times.retain(|c| breaks.contains(&c.day));
    p.days_shot.retain(|d| breaks.contains(&d.day));
    p.person_calls
        .retain(|x| breaks.contains(&x.day) && elements.contains(&x.element));
    p.day_crew_calls.retain(|x| breaks.contains(&x.day));
    p.scenes_shot
        .retain(|x| breaks.contains(&x.day) && sheets.contains(&x.sheet));
    p.turnarounds.retain(|t| cats.contains(&t.category));
    p.pinned.retain(|x| strips.contains(x));
    p.omitted.retain(|x| sheets.contains(x));
    p.locks.retain(|l| boards.contains(&l.board));
    // A draft stays in the history; only sheets since deleted leave it.
    for d in &mut p.drafts {
        for l in [&mut d.changed, &mut d.added, &mut d.omitted] {
            l.retain(|x| sheets.contains(x));
        }
    }
}

/// Sets or clears (`None`) the call time of the day closed by `day`.
pub fn set_call_time(s: &mut Schedule, day: StripId, minutes: Option<u32>) -> Result<(), Error> {
    if !day_breaks(s).contains(&day) {
        return Err(refuse(
            "a call time belongs to a shoot day: name its day break",
        ));
    }
    if minutes.is_some_and(|m| m >= 24 * 60) {
        return Err(refuse("a call time is 23:59 at the latest"));
    }
    let p = &mut s.plan;
    p.call_times.retain(|c| c.day != day);
    if let Some(minutes) = minutes {
        p.call_times.push(CallTime { day, minutes });
    }
    tidy(s);
    Ok(())
}

/// What has been shot of one scene, every day together (M5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShotSoFar {
    pub eighths: crate::Eighths,
    /// A day finished the scene.
    pub complete: bool,
}

impl Plan {
    /// What has been shot of `sheet`, or `None` while nothing is reported.
    pub fn shot_so_far(&self, sheet: SheetId) -> Option<ShotSoFar> {
        let mut out: Option<ShotSoFar> = None;
        for x in self.shot_of(sheet) {
            let o = out.get_or_insert_with(ShotSoFar::default);
            o.eighths = crate::Eighths(o.eighths.0 + x.eighths.0);
            o.complete |= x.complete;
        }
        out
    }
}

/// What a scene of `pages` still has to shoot: nothing once finished, else its length less
/// what was shot, never below nothing.
pub fn eighths_left(pages: crate::Eighths, shot: Option<ShotSoFar>) -> crate::Eighths {
    match shot {
        Some(x) if x.complete => crate::Eighths::ZERO,
        Some(x) => crate::Eighths(pages.0.saturating_sub(x.eighths.0)),
        None => pages,
    }
}

/// Records what was shot of `sheet` on the day `day` closes: `eighths` of it, and whether
/// that day finished it (another day's finish is cleared then). Nothing shot and not
/// finished takes the day's entry away.
pub fn record_shot(
    s: &mut Schedule,
    sheet: SheetId,
    day: StripId,
    eighths: crate::Eighths,
    complete: bool,
) -> Result<(), Error> {
    if s.sheet(sheet).is_none() {
        return Err(Error::NoSuchSheet(sheet));
    }
    if !day_breaks(s).contains(&day) {
        return Err(refuse("a scene is shot on a shoot day: name its day break"));
    }
    if eighths.0 > crate::model::MAX_SHEET_EIGHTHS {
        return Err(refuse("a scene is 999 pages at the most"));
    }
    let p = &mut s.plan;
    p.scenes_shot
        .retain(|x| !(x.sheet == sheet && x.day == day));
    if complete {
        for x in p.scenes_shot.iter_mut().filter(|x| x.sheet == sheet) {
            x.complete = false;
        }
    }
    if eighths.0 > 0 || complete {
        p.scenes_shot.push(SceneShot {
            sheet,
            day,
            eighths,
            complete,
        });
    }
    tidy(s);
    Ok(())
}

/// Adds `conflicts`, from a merge on `date`, to the review list (decision 238): one with a
/// key already listed takes its place. Returns how many were added.
pub fn keep_conflicts(
    s: &mut Schedule,
    conflicts: &[crate::merge::Conflict],
    date: Option<Date>,
) -> usize {
    let p = &mut s.plan;
    for c in conflicts {
        p.conflicts.retain(|x| x.key != c.key);
        p.conflicts.push(ReviewConflict {
            key: c.key.clone(),
            what: c.what.clone(),
            base: c.base.clone(),
            ours: c.ours.clone(),
            theirs: c.theirs.clone(),
            taken: c.taken,
            date,
        });
    }
    let over = p.conflicts.len().saturating_sub(MAX_REVIEW_CONFLICTS);
    p.conflicts.drain(..over);
    conflicts.len().min(MAX_REVIEW_CONFLICTS)
}

/// Marks the conflicts with these keys done: they leave the review list. Every key when
/// `keys` is none. Returns how many left.
pub fn settle_conflicts(s: &mut Schedule, keys: Option<&[String]>) -> usize {
    let p = &mut s.plan;
    let before = p.conflicts.len();
    match keys {
        Some(k) => p.conflicts.retain(|x| !k.contains(&x.key)),
        None => p.conflicts.clear(),
    }
    before - p.conflicts.len()
}

/// Sets element `element`'s own call and wrap on the day closed by `day` (decision 231):
/// minutes after midnight, the wrap up to [`MAX_WRAP`]; both `None` follows the company's
/// again.
pub fn set_person_call(
    s: &mut Schedule,
    day: StripId,
    element: ElementId,
    call: Option<u32>,
    wrap: Option<u32>,
) -> Result<(), Error> {
    if !day_breaks(s).contains(&day) {
        return Err(refuse("a call belongs to a shoot day: name its day break"));
    }
    if s.element(element).is_none() {
        return Err(Error::NoSuchElement(element));
    }
    if call.is_some_and(|m| m >= 24 * 60) {
        return Err(refuse("a call time is 23:59 at the latest"));
    }
    if wrap.is_some_and(|m| m == 0 || m > MAX_WRAP) {
        return Err(refuse("a wrap is by the end of the next day at the latest"));
    }
    if matches!((call, wrap), (Some(c), Some(w)) if w <= c) {
        return Err(refuse("a wrap comes after the call"));
    }
    let p = &mut s.plan;
    p.person_calls
        .retain(|x| !(x.day == day && x.element == element));
    if call.is_some() || wrap.is_some() {
        p.person_calls.push(PersonCall {
            day,
            element,
            call,
            wrap,
        });
    }
    tidy(s);
    Ok(())
}

/// Whether a department name may be kept as it is: one line of 1 to [`MAX_DEPARTMENT`]
/// characters with no spaces around it.
pub fn department_ok(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name != name.trim()
        || name.chars().count() > MAX_DEPARTMENT
        || name.chars().any(char::is_control)
    {
        return Err(format!(
            "a department is one line of 1 to {MAX_DEPARTMENT} characters"
        ));
    }
    Ok(())
}

/// Sets the crew call of `department` (decision 242), minutes from the company's call, or
/// removes it with `None`. A new department goes last; one already there keeps its place
/// and takes the name as typed now.
pub fn set_crew_call(s: &mut Schedule, department: &str, offset: Option<i32>) -> Result<(), Error> {
    let name = department.trim();
    department_ok(name).map_err(|e| refuse(&e))?;
    let p = &mut s.plan;
    let Some(offset) = offset else {
        p.crew_calls.retain(|x| !same_name(&x.department, name));
        return Ok(());
    };
    // unsigned_abs: i32::MIN has no i32 absolute value.
    if offset.unsigned_abs() > MAX_CREW_OFFSET.unsigned_abs() {
        return Err(refuse("a crew call is within 12:00 of the company's call"));
    }
    let full = p.crew_calls.len() >= MAX_CREW_CALLS;
    match p
        .crew_calls
        .iter_mut()
        .find(|x| same_name(&x.department, name))
    {
        Some(x) => {
            x.department = name.to_string();
            x.offset = offset;
        }
        None if full => return Err(refuse("a schedule keeps 100 crew calls at most")),
        None => p.crew_calls.push(CrewCall {
            department: name.to_string(),
            offset,
        }),
    }
    Ok(())
}

/// Sets `department`'s call on the day closed by `day` only (decision 243), minutes from
/// that day's company call, or removes it with `None` so the day follows the every-day
/// call again (or, for a department without one, no longer calls it). One already set
/// keeps its place and takes the name as typed now.
pub fn set_day_crew_call(
    s: &mut Schedule,
    day: StripId,
    department: &str,
    offset: Option<i32>,
) -> Result<(), Error> {
    if !day_breaks(s).contains(&day) {
        return Err(refuse("a call belongs to a shoot day: name its day break"));
    }
    let name = department.trim();
    department_ok(name).map_err(|e| refuse(&e))?;
    let p = &mut s.plan;
    let Some(offset) = offset else {
        p.day_crew_calls
            .retain(|x| !(x.day == day && same_name(&x.department, name)));
        return Ok(());
    };
    if offset.unsigned_abs() > MAX_CREW_OFFSET.unsigned_abs() {
        return Err(refuse("a crew call is within 12:00 of the company's call"));
    }
    let full = p.day_crew_calls.iter().filter(|x| x.day == day).count() >= MAX_CREW_CALLS;
    match p
        .day_crew_calls
        .iter_mut()
        .find(|x| x.day == day && same_name(&x.department, name))
    {
        Some(x) => {
            x.department = name.to_string();
            x.offset = offset;
        }
        None if full => {
            return Err(refuse(
                "a shoot day keeps 100 crew calls of its own at most",
            ))
        }
        None => p.day_crew_calls.push(DayCrewCall {
            day,
            department: name.to_string(),
            offset,
        }),
    }
    tidy(s);
    Ok(())
}

/// Sets the long day limit in minutes, or back to the default with `None`.
pub fn set_long_day(s: &mut Schedule, minutes: Option<u32>) -> Result<(), Error> {
    if let Some(m) = minutes {
        if m == 0 || m > 24 * 60 {
            return Err(refuse(
                "a long day limit is more than 0:00 and 24:00 at most",
            ));
        }
    }
    s.plan.long_day = minutes.filter(|m| *m != DEFAULT_LONG_DAY);
    Ok(())
}

/// Sets or clears (`None`) the turnaround minimum for a category.
pub fn set_turnaround(
    s: &mut Schedule,
    category: CategoryId,
    minutes: Option<u32>,
) -> Result<(), Error> {
    s.category(category)
        .ok_or(Error::NoSuchCategory(category))?;
    if minutes.is_some_and(|m| m == 0 || m > 48 * 60) {
        return Err(refuse("a turnaround is more than 0:00 and 48:00 at most"));
    }
    let p = &mut s.plan;
    match minutes {
        Some(minutes) => match p.turnarounds.iter_mut().find(|t| t.category == category) {
            Some(t) => t.minutes = minutes,
            None => p.turnarounds.push(Turnaround { category, minutes }),
        },
        None => p.turnarounds.retain(|t| t.category != category),
    }
    tidy(s);
    Ok(())
}

/// Pins or unpins strips for the strip order suggestion. Day breaks and banners never
/// move there anyway, so only scene strips are pinned.
pub fn set_pinned(s: &mut Schedule, strips: &[StripId], pinned: bool) -> Result<usize, Error> {
    let scenes: HashSet<StripId> = s
        .boards
        .iter()
        .flat_map(|b| {
            b.sub_boards
                .iter()
                .flat_map(|sb| &sb.strips)
                .chain(&b.boneyard)
        })
        .filter(|st| matches!(st.kind, StripKind::Sheet(_)))
        .map(|st| st.id)
        .collect();
    let mut n = 0;
    for &id in strips {
        if !scenes.contains(&id) {
            return Err(Error::NoSuchStrip(id));
        }
        let has = s.plan.pinned.contains(&id);
        if pinned && !has {
            s.plan.pinned.push(id);
            n += 1;
        } else if !pinned && has {
            s.plan.pinned.retain(|x| *x != id);
            n += 1;
        }
    }
    tidy(s);
    Ok(n)
}

// ---------------------------------------------------------------------------
// Reading what people type (the app, the command line and the MCP server share these)

/// Degrees as typed: `34.0522`, `-118.2437`, or with a compass letter (`34.0522 N`,
/// `118.2437W`). Up to six decimals; returns millionths. `max` is 90 or 180.
pub fn parse_degrees(text: &str, max: i32) -> Result<i32, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a number of degrees such as 34.0522 or -118.2437");
    let (body, sign) = match t.chars().last().map(|c| c.to_ascii_uppercase()) {
        Some('N') if max == 90 => (&t[..t.len() - 1], 1),
        Some('S') if max == 90 => (&t[..t.len() - 1], -1),
        Some('E') if max == 180 => (&t[..t.len() - 1], 1),
        Some('W') if max == 180 => (&t[..t.len() - 1], -1),
        _ => (t, 1),
    };
    let body = body.trim();
    let (neg, digits) = match body.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, body.strip_prefix('+').unwrap_or(body)),
    };
    if neg && sign < 0 {
        return Err(bad());
    }
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && frac.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
        || whole.len() > 3
    {
        return Err(bad());
    }
    if frac.len() > 6 {
        return Err(format!("{t:?} has more than six decimals"));
    }
    let w: i32 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| bad())?
    };
    let f: i32 = format!("{frac:0<6}").parse().map_err(|_| bad())?;
    let v = w * 1_000_000 + f;
    if v > max * 1_000_000 {
        return Err(format!("{t:?} is past {max} degrees"));
    }
    Ok(if neg || sign < 0 { -v } else { v })
}

/// Millionths of a degree as text: `34.0522`, `-118.2437`, `0`.
pub fn degrees_text(v: i32) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.unsigned_abs();
    let frac = format!("{:06}", a % 1_000_000);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{}", a / 1_000_000)
    } else {
        format!("{sign}{}.{frac}", a / 1_000_000)
    }
}

/// A UTC offset as typed: `-8`, `+5:30`, `-08:00`, `UTC-7`, `0`. Returns minutes.
pub fn parse_offset(text: &str) -> Result<i32, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a UTC offset such as -8, +5:30 or -08:00");
    let body = t
        .strip_prefix("UTC")
        .or_else(|| t.strip_prefix("utc"))
        .or_else(|| t.strip_prefix("GMT"))
        .unwrap_or(t)
        .trim();
    if body.is_empty() {
        return Ok(0);
    }
    let (neg, rest) = match body.as_bytes()[0] {
        b'-' => (true, &body[1..]),
        b'+' => (false, &body[1..]),
        _ => (false, body),
    };
    let (h, m) = rest.split_once(':').unwrap_or((rest, "0"));
    let ok = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit());
    if !ok(h) || !ok(m) {
        return Err(bad());
    }
    let (h, m): (i32, i32) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if m >= 60 || h * 60 + m > 14 * 60 {
        return Err(bad());
    }
    let v = h * 60 + m;
    Ok(if neg { -v } else { v })
}

/// Minutes ahead of UTC as text: `-08:00`, `+05:30`, `+00:00`.
pub fn offset_text(minutes: i32) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let a = minutes.unsigned_abs();
    format!("{sign}{:02}:{:02}", a / 60, a % 60)
}

/// A daylight saving rule as typed or stored: `none` (or nothing) for none, else a
/// rule's name (`us_canada`, `eu`, `australia`, `new_zealand`, `israel`, `egypt`,
/// `chile`), ignoring case.
pub fn parse_daylight_saving(text: &str) -> Result<Option<DaylightSaving>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    DaylightSaving::from_name(t).map(Some).ok_or_else(|| {
        format!(
            "{t:?} is not a daylight saving rule: none, us_canada, eu, australia, new_zealand, israel, egypt or chile"
        )
    })
}

/// A clock time as typed, `6:30`, `06:30` or `18:45`, as minutes after midnight.
pub fn parse_clock(text: &str) -> Result<u32, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a time such as 6:30 or 18:45");
    let (h, m) = t.split_once(':').ok_or_else(bad)?;
    let digits =
        |s: &str, n: usize| (1..=n).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(h, 2) || m.len() != 2 || !digits(m, 2) {
        return Err(bad());
    }
    let (h, m): (u32, u32) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// Minutes after midnight as a 24-hour clock, `06:30`. Past midnight wraps (`24:10` reads
/// `00:10`); before it too.
pub fn clock_text(minutes: i32) -> String {
    let m = minutes.rem_euclid(24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// A length of time as typed: whole hours (`12`) or `h:mm` (`10:30`). Minutes.
pub fn parse_hours(text: &str) -> Result<u32, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a length of time such as 12 or 10:30");
    let (h, m) = t.split_once(':').unwrap_or((t, "00"));
    let digits = |s: &str| !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(h) || m.len() != 2 || !digits(m) {
        return Err(bad());
    }
    let (h, m): (u32, u32) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// Minutes as `h:mm`.
pub fn hours_text(minutes: u32) -> String {
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

/// A crew call's offset from the company's call as typed: `-1:30` or `-1` before it,
/// `+0:15` or `0:15` after, `0` with it (decision 242). Minutes.
pub fn parse_crew_offset(text: &str) -> Result<i32, String> {
    let t = text.trim();
    let bad = || format!("{t:?} is not a time before or after the call such as -1:30, 0 or +0:15");
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let m = parse_hours(rest).map_err(|_| bad())? as i32;
    if m > MAX_CREW_OFFSET {
        return Err(format!("{t:?} is more than 12:00 from the call"));
    }
    Ok(if neg { -m } else { m })
}

/// An offset from the call as typed back: `-1:30`, `0:00`, `+0:15`.
pub fn crew_offset_text(minutes: i32) -> String {
    let a = hours_text(minutes.unsigned_abs());
    match minutes.signum() {
        -1 => format!("-{a}"),
        1 => format!("+{a}"),
        _ => a,
    }
}

/// An offset from the call in words: `1:30 before the call`, `with the call`.
pub fn crew_offset_words(minutes: i32) -> String {
    let a = hours_text(minutes.unsigned_abs());
    match minutes.signum() {
        -1 => format!("{a} before the call"),
        1 => format!("{a} after the call"),
        _ => "with the call".to_string(),
    }
}

/// What changed in the plan between two schedules, in plain phrases (for What Changed).
pub fn changes(a: &Plan, b: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for l in &b.locations {
        match a.location(&l.name) {
            None => out.push(format!("location {:?} placed on the map", l.name)),
            Some(old) if old != l => out.push(format!("location {:?} moved on the map", l.name)),
            _ => {}
        }
    }
    for l in &a.locations {
        if b.location(&l.name).is_none() {
            out.push(format!("location {:?} taken off the map", l.name));
        }
    }
    for g in &b.location_guides {
        if a.location_guide(&g.name) != Some(g) {
            out.push(format!("directions to {:?} changed", g.name));
        }
    }
    for g in &a.location_guides {
        if b.location_guide(&g.name).is_none() {
            out.push(format!("directions to {:?} removed", g.name));
        }
    }
    if a.call_times != b.call_times {
        out.push("call times changed".to_string());
    }
    if a.days_shot != b.days_shot {
        out.push("days as shot changed".to_string());
    }
    if a.person_calls != b.person_calls {
        out.push("calls of their own changed".to_string());
    }
    if a.scenes_shot != b.scenes_shot {
        out.push("scenes shot changed".to_string());
    }
    if a.long_day != b.long_day {
        out.push(format!(
            "long day limit {} -> {}",
            hours_text(a.long_day()),
            hours_text(b.long_day())
        ));
    }
    if a.turnarounds != b.turnarounds {
        out.push("turnaround minimums changed".to_string());
    }
    if a.crew_calls != b.crew_calls {
        out.push("crew calls changed".to_string());
    }
    if a.day_crew_calls != b.day_crew_calls {
        out.push("crew calls for one day changed".to_string());
    }
    for l in &b.locks {
        if a.lock(&l.name).is_none() {
            out.push(format!("schedule lock {:?} made", l.name));
        }
    }
    for l in &a.locks {
        if b.lock(&l.name).is_none() {
            out.push(format!("schedule lock {:?} deleted", l.name));
        }
    }
    if a.pinned != b.pinned {
        out.push("pinned strips changed".to_string());
    }
    if a.drafts != b.drafts {
        out.push("script drafts changed".to_string());
    }
    if a.conflicts != b.conflicts {
        out.push("merge conflicts to review changed".to_string());
    }
    if a.omitted != b.omitted {
        out.push("omitted scenes changed".to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crew_calls_are_set_against_the_call() {
        assert_eq!(parse_crew_offset("-1:30"), Ok(-90));
        assert_eq!(parse_crew_offset("-1"), Ok(-60));
        assert_eq!(parse_crew_offset("0"), Ok(0));
        assert_eq!(parse_crew_offset("+0:15"), Ok(15));
        assert_eq!(parse_crew_offset("0:15"), Ok(15));
        assert_eq!(parse_crew_offset(" -12:00 "), Ok(-720));
        for bad in ["", "-", "12:01", "1:5", "-1:60", "ten", "--1"] {
            assert!(parse_crew_offset(bad).is_err(), "{bad:?}");
        }
        assert_eq!(crew_offset_text(-90), "-1:30");
        assert_eq!(crew_offset_text(0), "0:00");
        assert_eq!(crew_offset_text(15), "+0:15");
        assert_eq!(crew_offset_words(-90), "1:30 before the call");
        assert_eq!(crew_offset_words(0), "with the call");

        let mut s = Schedule::new("T");
        set_crew_call(&mut s, "Camera", Some(0)).unwrap();
        set_crew_call(&mut s, " Hair and makeup ", Some(-90)).unwrap();
        set_crew_call(&mut s, "camera", Some(-30)).unwrap();
        let names: Vec<_> = s
            .plan
            .crew_calls
            .iter()
            .map(|x| x.department.as_str())
            .collect();
        assert_eq!(
            names,
            ["camera", "Hair and makeup"],
            "kept in place, renamed as typed"
        );
        assert_eq!(s.plan.crew_call("CAMERA").unwrap().on(7 * 60), 390);
        assert_eq!(s.plan.crew_call("Hair and makeup").unwrap().on(60), -30);
        assert!(set_crew_call(&mut s, "Grip", Some(721)).is_err());
        assert!(set_crew_call(&mut s, "Grip", Some(i32::MIN)).is_err());
        assert!(set_crew_call(&mut s, "Grip", Some(i32::MAX)).is_err());
        assert!(set_crew_call(&mut s, "", Some(0)).is_err());
        assert!(set_crew_call(&mut s, "Grip\nElectric", Some(0)).is_err());
        assert!(check(&s).is_ok());
        let mut bad = s.clone();
        bad.plan.crew_calls[1].department = "Camera".to_string();
        assert!(check(&bad).is_err(), "two of one department");
        for v in [i32::MIN, i32::MAX, -721] {
            let mut bad = s.clone();
            bad.plan.crew_calls[0].offset = v;
            assert!(check(&bad).is_err(), "{v}");
        }
        set_crew_call(&mut s, "Camera", None).unwrap();
        assert_eq!(s.plan.crew_calls.len(), 1);
        for i in 1..MAX_CREW_CALLS {
            set_crew_call(&mut s, &format!("Department {i}"), Some(0)).unwrap();
        }
        assert!(set_crew_call(&mut s, "One too many", Some(0)).is_err());
    }

    #[test]
    fn crew_calls_for_one_day() {
        let mut s = Schedule::new("T");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let at = |index| crate::model::Place {
            sub_board: Some(sb),
            index,
        };
        let d1 = s.insert_strip(b, StripKind::DayBreak, at(0)).unwrap();
        let d2 = s.insert_strip(b, StripKind::DayBreak, at(1)).unwrap();
        set_crew_call(&mut s, "Camera", Some(-30)).unwrap();
        set_crew_call(&mut s, "Hair and makeup", Some(-90)).unwrap();
        set_day_crew_call(&mut s, d1, "hair and makeup", Some(-120)).unwrap();
        set_day_crew_call(&mut s, d1, "Stunts", Some(60)).unwrap();
        let on = |s: &Schedule, d| -> Vec<(String, i32, bool)> {
            s.plan
                .crew_on(d)
                .into_iter()
                .map(|x| (x.department, x.offset, x.this_day))
                .collect()
        };
        let row = |n: &str, o, t| (n.to_string(), o, t);
        assert_eq!(
            on(&s, d1),
            [
                row("Camera", -30, false),
                row("Hair and makeup", -120, true),
                row("Stunts", 60, true)
            ],
            "the every-day name, that day's offset, then the day's own"
        );
        assert_eq!(
            on(&s, d2),
            [
                row("Camera", -30, false),
                row("Hair and makeup", -90, false)
            ]
        );
        assert!(check(&s).is_ok());
        assert!(set_day_crew_call(&mut s, d1, "Grip", Some(721)).is_err());
        assert!(set_day_crew_call(&mut s, d1, "Grip", Some(i32::MIN)).is_err());
        assert!(set_day_crew_call(&mut s, StripId(9999), "Grip", Some(0)).is_err());
        let mut bad = s.clone();
        bad.plan.day_crew_calls[1].department = "HAIR AND MAKEUP".to_string();
        assert!(check(&bad).is_err(), "two of one department on a day");
        let mut bad = s.clone();
        bad.plan.day_crew_calls[0].offset = i32::MIN;
        assert!(check(&bad).is_err());
        // Removing the day's call follows the every-day one again.
        set_day_crew_call(&mut s, d1, "Hair and makeup", None).unwrap();
        assert_eq!(on(&s, d1)[1], row("Hair and makeup", -90, false));
        // A deleted day break takes its calls with it.
        s.delete_strip(b, d1).unwrap();
        tidy(&mut s);
        assert!(s.plan.day_crew_calls.is_empty());
        for i in 0..MAX_CREW_CALLS {
            set_day_crew_call(&mut s, d2, &format!("Department {i}"), Some(0)).unwrap();
        }
        assert!(set_day_crew_call(&mut s, d2, "One too many", Some(0)).is_err());
        assert!(check(&s).is_ok());
    }

    #[test]
    fn a_location_guide_is_found_by_name_and_goes_when_empty() {
        let mut p = Plan::default();
        let guide = |address: &str, map: Option<&str>| LocationGuide {
            name: " Pier ".into(),
            address: address.into(),
            directions: String::new(),
            map: map.map(str::to_string),
        };
        p.set_location_guide(guide("  1 Harbor Road ", None))
            .unwrap();
        let g = p
            .location_guide("PIER")
            .expect("found without regard to case");
        assert_eq!(
            (g.name.as_str(), g.address.as_str()),
            ("Pier", "1 Harbor Road")
        );
        // Replaced, not listed twice; a map alone is enough.
        p.set_location_guide(guide("", Some("images/pier.png")))
            .unwrap();
        assert_eq!(p.location_guides.len(), 1);
        assert_eq!(p.location_guides[0].map.as_deref(), Some("images/pier.png"));
        // Too long is refused and changes nothing; nothing in it takes it off.
        let long = "x".repeat(MAX_GUIDE_TEXT + 1);
        assert!(p.set_location_guide(guide(&long, None)).is_err());
        assert_eq!(p.location_guides.len(), 1);
        p.set_location_guide(guide(" ", Some(" "))).unwrap();
        assert!(p.location_guides.is_empty());
        assert!(
            guide_ok(&guide("", None)).is_err(),
            "the reader refuses an empty one"
        );
    }

    #[test]
    fn offset_periods_win_over_the_rule() {
        let d = |x: &str| x.parse::<Date>().unwrap();
        let p = parse_offset_periods(
            "2027-03-28 to 2027-03-29 +00:00; 2027-02-07 to 2027-03-14 +00:00",
        )
        .unwrap();
        assert_eq!(p[0].from, d("2027-02-07"), "put in date order");
        assert_eq!(
            offset_periods_text(&p),
            "2027-02-07 to 2027-03-14 +00:00; 2027-03-28 to 2027-03-29 +00:00"
        );
        assert_eq!(
            parse_offset_periods("2027-05-01 +02:00").unwrap()[0].to,
            d("2027-05-01")
        );
        assert_eq!(parse_offset_periods(" ").unwrap(), []);
        for bad in [
            "2027-03-14 to 2027-02-07 +00:00",
            "2027-02-07 to 2027-03-14 +15:00",
            "2027-02-07 to 2027-03-14 +00:00; 2027-03-14 +01:00",
            "2027-02-07 until 2027-03-14 +00:00",
            "2027-02-30 +00:00",
            "Ramadan +00:00",
        ] {
            assert!(parse_offset_periods(bad).is_err(), "{bad:?}");
        }
        // Morocco: +01:00 all year, +00:00 through Ramadan (dates typed for 2027).
        let mut s = Schedule::new("T");
        s.plan
            .set_location(GeoLocation {
                name: "Medina".into(),
                latitude: 34_020_000,
                longitude: -6_830_000,
                utc_offset: 60,
                daylight_saving: None,
                periods: p.clone(),
            })
            .unwrap();
        let m = s.plan.location("medina").unwrap();
        assert_eq!(m.offset_on(d("2027-02-06")), 60);
        assert_eq!(m.offset_on(d("2027-02-07")), 0);
        assert_eq!(m.offset_on(d("2027-03-14")), 0);
        assert_eq!(m.offset_on(d("2027-03-15")), 60);
        // A period wins over the yearly rule.
        let mut eu = m.clone();
        eu.daylight_saving = Some(DaylightSaving::Eu);
        assert_eq!(eu.offset_on(d("2027-03-28")), 0);
        assert_eq!(eu.offset_on(d("2027-04-05")), 120);
        assert!(check(&s).is_ok());
        let mut bad = s.clone();
        bad.plan.locations[0].periods[0].offset = i32::MIN;
        assert!(check(&bad).is_err());
        let mut bad = s.clone();
        bad.plan.locations[0].periods.reverse();
        assert!(check(&bad).is_err(), "out of order");
        let mut over = m.clone();
        over.periods = (0..=MAX_OFFSET_PERIODS as i32)
            .map(|i| OffsetPeriod {
                from: Date(i * 2),
                to: Date(i * 2),
                offset: 0,
            })
            .collect();
        assert!(s.plan.set_location(over).is_err());
    }

    #[test]
    fn degrees_read_and_write() {
        assert_eq!(parse_degrees("34.0522", 90), Ok(34_052_200));
        assert_eq!(parse_degrees("-118.2437", 180), Ok(-118_243_700));
        assert_eq!(parse_degrees("118.2437 W", 180), Ok(-118_243_700));
        assert_eq!(parse_degrees("33.8688s", 90), Ok(-33_868_800));
        assert_eq!(parse_degrees("+0.5", 90), Ok(500_000));
        assert_eq!(parse_degrees("90", 90), Ok(90_000_000));
        assert!(parse_degrees("91", 90).is_err());
        assert!(parse_degrees("34.0522 E", 90).is_err());
        assert!(parse_degrees("1.1234567", 90).is_err());
        assert!(parse_degrees("abc", 90).is_err());
        assert!(parse_degrees("", 90).is_err());
        assert!(parse_degrees("-5 S", 90).is_err());
        assert_eq!(degrees_text(34_052_200), "34.0522");
        assert_eq!(degrees_text(-118_243_700), "-118.2437");
        assert_eq!(degrees_text(0), "0");
        assert_eq!(degrees_text(-500_000), "-0.5");
    }

    #[test]
    fn offsets_clocks_and_hours() {
        assert_eq!(parse_offset("-8"), Ok(-480));
        assert_eq!(parse_offset("+5:30"), Ok(330));
        assert_eq!(parse_offset("UTC-07:00"), Ok(-420));
        assert_eq!(parse_offset("0"), Ok(0));
        assert!(parse_offset("+15").is_err());
        assert!(parse_offset("5:75").is_err());
        assert_eq!(offset_text(-480), "-08:00");
        assert_eq!(offset_text(330), "+05:30");
        assert_eq!(parse_clock("6:30"), Ok(390));
        assert_eq!(parse_clock("18:45"), Ok(1125));
        assert!(parse_clock("24:00").is_err());
        assert!(parse_clock("6").is_err());
        assert!(parse_clock("6:5").is_err());
        assert_eq!(clock_text(1450), "00:10");
        assert_eq!(clock_text(-10), "23:50");
        assert_eq!(parse_hours("12"), Ok(720));
        assert_eq!(parse_hours("10:30"), Ok(630));
        assert!(parse_hours("10:3").is_err());
        assert_eq!(hours_text(630), "10:30");
    }

    #[test]
    fn plan_edits_and_tidy() {
        let mut s = Schedule::new("ZTEST");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let br = s
            .insert_strip(
                b,
                StripKind::DayBreak,
                crate::model::Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .unwrap();
        set_call_time(&mut s, br, Some(390)).unwrap();
        assert_eq!(s.plan.call_time(br), Some(390));
        assert!(set_call_time(&mut s, StripId(9999), Some(1)).is_err());
        assert!(set_call_time(&mut s, br, Some(1440)).is_err());
        let cast = s.cast().unwrap();
        set_turnaround(&mut s, cast, Some(720)).unwrap();
        assert_eq!(s.plan.turnaround(cast), Some(720));
        set_long_day(&mut s, Some(720)).unwrap();
        assert_eq!(s.plan.long_day, None);
        s.plan
            .set_location(GeoLocation {
                name: " Pier ".into(),
                latitude: 1,
                longitude: 2,
                utc_offset: 0,
                daylight_saving: None,
                periods: Vec::new(),
            })
            .unwrap();
        assert_eq!(s.plan.location("PIER").unwrap().name, "Pier");
        assert!(s
            .plan
            .set_location(GeoLocation {
                name: "x".into(),
                latitude: 91_000_000,
                longitude: 0,
                utc_offset: 0,
                daylight_saving: None,
                periods: Vec::new(),
            })
            .is_err());
        s.delete_strip(b, br).unwrap();
        tidy(&mut s);
        assert!(s.plan.call_times.is_empty());
        set_turnaround(&mut s, cast, None).unwrap();
        assert!(s.plan.turnarounds.is_empty());
        assert!(!s.plan.is_empty());
        assert!(s.plan.remove_location("pier"));
        assert!(s.plan.is_empty());
    }

    /// Review: deleting what the plan points at once left the reference in the file
    /// (a lock of a deleted board then failed in Drift). Every deletion tidies now.
    #[test]
    fn deleting_what_the_plan_points_at_tidies_it() {
        let mut s = crate::sample::sample(30);
        let b = s.boards[0].id;
        let br = breaks(&s)[0];
        set_call_time(&mut s, br, Some(390)).unwrap();
        s.delete_strip(b, br).unwrap();
        assert!(
            s.plan.call_times.is_empty(),
            "a deleted day break's call time"
        );

        let extra = s.add_category("ZTEST Rigging");
        set_turnaround(&mut s, extra, Some(600)).unwrap();
        s.delete_category(extra).unwrap();
        assert!(
            s.plan.turnarounds.is_empty(),
            "a deleted category's turnaround"
        );

        let (first, second) = (s.sheets[0].id, s.sheets[1].id);
        let strip_of = |s: &Schedule, h: SheetId| {
            s.boards[0].sub_boards[0]
                .strips
                .iter()
                .find(|x| x.kind == StripKind::Sheet(h))
                .unwrap()
                .id
        };
        let pin = strip_of(&s, second);
        s.plan.pinned.push(pin);
        s.plan.omitted.push(second);
        s.merge_sheets(first, &[second]).unwrap();
        assert!(
            s.plan.pinned.is_empty() && s.plan.omitted.is_empty(),
            "merged away"
        );
        let third = s.sheets[1].id;
        s.plan.omitted.push(third);
        s.delete_sheet(third).unwrap();
        assert!(s.plan.omitted.is_empty(), "a deleted sheet's omitted mark");

        // A sub-board's day breaks go with it; a board's locks with the board.
        let other = s.add_sub_board(b, "ZTEST Unit 2", None).unwrap();
        let at = crate::model::Place {
            sub_board: Some(other),
            index: 0,
        };
        let br2 = s.insert_strip(b, StripKind::DayBreak, at).unwrap();
        set_call_time(&mut s, br2, Some(400)).unwrap();
        s.delete_sub_board(b, other).unwrap();
        assert!(
            s.plan.call_times.is_empty(),
            "a deleted sub-board's call times"
        );
        let rain = s.duplicate_board(b, "ZTEST Rain").unwrap();
        crate::drift::lock(&mut s, rain, "White", None).unwrap();
        s.delete_board(rain).unwrap();
        assert!(s.plan.locks.is_empty(), "a deleted board's lock");
        s.check().unwrap();
    }

    fn breaks(s: &Schedule) -> Vec<StripId> {
        s.boards[0].sub_boards[0]
            .strips
            .iter()
            .filter(|x| x.kind == StripKind::DayBreak)
            .map(|x| x.id)
            .collect()
    }

    /// Review: another call time edit once dropped a parked day break's call time.
    #[test]
    fn a_parked_day_break_keeps_its_call_time() {
        let (mut s, board, sub, _) = crate::timeline::tests::split_two_days();
        let b = breaks(&s);
        set_call_time(&mut s, b[0], Some(390)).unwrap();
        let place = |sub_board, index| crate::model::Place { sub_board, index };
        s.move_strips(board, &[b[0]], place(None, 0)).unwrap();
        set_call_time(&mut s, b[1], Some(420)).unwrap();
        s.move_strips(board, &[b[0]], place(Some(sub), 1)).unwrap();
        assert_eq!(s.plan.call_time(b[0]), Some(390));
        assert!(s.check().is_ok());
    }

    /// Review: automatic day breaks once made every break anew, dropping call times.
    #[test]
    fn automatic_day_breaks_keep_call_times_in_order() {
        use crate::days::{auto_day_breaks, remove_day_breaks, Limit};
        let (mut s, board, sub, _) = crate::timeline::tests::split_two_days();
        let first = breaks(&s)[0];
        set_call_time(&mut s, first, Some(390)).unwrap();
        auto_day_breaks(&mut s, board, sub, Limit::Pages(crate::Eighths(16))).unwrap();
        assert_eq!(breaks(&s)[0], first);
        let t = crate::timeline::timelines(&s, board, sub).unwrap();
        assert_eq!(
            t[0].call,
            Some(390),
            "re-breaking dropped day 1's call time"
        );
        remove_day_breaks(&mut s, board, sub).unwrap();
        assert!(s.plan.call_times.is_empty(), "removed with their breaks");
    }

    #[test]
    fn a_persons_own_call_is_checked_and_dropped_with_its_day_or_element() {
        use crate::days::remove_day_breaks;
        let (mut s, board, sub, _) = crate::timeline::tests::split_two_days();
        let d1 = breaks(&s)[0];
        let cast = s.cast().unwrap();
        let el = s.add_element(cast, "ANN").unwrap();
        set_person_call(&mut s, d1, el, Some(7 * 60), Some(18 * 60)).unwrap();
        assert_eq!(s.plan.person_call(d1, el).unwrap().call, Some(7 * 60));
        s.check().unwrap();
        // Refused: not a day break, a wrap before the call, a call past midnight.
        let not_a_break = s.boards[0].sub_boards[0].strips[0].id;
        assert!(set_person_call(&mut s, not_a_break, el, Some(60), None).is_err());
        assert!(set_person_call(&mut s, d1, el, Some(600), Some(500)).is_err());
        assert!(set_person_call(&mut s, d1, el, Some(1440), None).is_err());
        // Setting again replaces it; both empty takes it away.
        set_person_call(&mut s, d1, el, Some(8 * 60), None).unwrap();
        assert_eq!(s.plan.person_calls.len(), 1);
        set_person_call(&mut s, d1, el, None, None).unwrap();
        assert!(s.plan.person_calls.is_empty());
        // Two for one element and day, or an empty one, don't pass check.
        let mut bad = s.clone();
        let x = PersonCall {
            day: d1,
            element: el,
            call: Some(60),
            wrap: None,
        };
        bad.plan.person_calls = vec![x, x];
        assert!(bad.check().is_err());
        bad.plan.person_calls = vec![PersonCall { call: None, ..x }];
        assert!(bad.check().is_err());
        // Its day gone: it goes too.
        set_person_call(&mut s, d1, el, Some(60), None).unwrap();
        remove_day_breaks(&mut s, board, sub).unwrap();
        assert!(s.plan.person_calls.is_empty());
    }

    #[test]
    fn a_scene_shot_over_two_days_adds_up_and_finishes_once() {
        let (mut s, _, _, _) = crate::timeline::tests::split_two_days();
        let (d1, d2) = (breaks(&s)[0], breaks(&s)[1]);
        let sheet = s.sheets[0].id;
        let pages = s.sheet(sheet).unwrap().pages;
        assert_eq!(s.plan.shot_so_far(sheet), None);
        assert_eq!(eighths_left(pages, None), pages);
        record_shot(&mut s, sheet, d1, crate::Eighths(3), false).unwrap();
        record_shot(&mut s, sheet, d2, crate::Eighths(2), true).unwrap();
        let so_far = s.plan.shot_so_far(sheet).unwrap();
        assert_eq!(
            so_far,
            ShotSoFar {
                eighths: crate::Eighths(5),
                complete: true
            }
        );
        assert_eq!(eighths_left(pages, Some(so_far)), crate::Eighths::ZERO);
        // Finishing it on the first day instead clears the second day's finish.
        record_shot(&mut s, sheet, d1, crate::Eighths(3), true).unwrap();
        assert_eq!(s.plan.scenes_shot.iter().filter(|x| x.complete).count(), 1);
        s.check().unwrap();
        // Not finished, part shot: what's left is the rest, never below nothing.
        let part = ShotSoFar {
            eighths: crate::Eighths(2),
            complete: false,
        };
        assert_eq!(
            eighths_left(crate::Eighths(6), Some(part)),
            crate::Eighths(4)
        );
        let over = ShotSoFar {
            eighths: crate::Eighths(9),
            complete: false,
        };
        assert_eq!(
            eighths_left(crate::Eighths(6), Some(over)),
            crate::Eighths::ZERO
        );
        // Nothing shot and not finished takes the entry away; a strip that isn't a day
        // break is refused.
        record_shot(&mut s, sheet, d1, crate::Eighths::ZERO, false).unwrap();
        record_shot(&mut s, sheet, d2, crate::Eighths::ZERO, false).unwrap();
        assert!(s.plan.scenes_shot.is_empty());
        let not_a_break = s.boards[0].sub_boards[0].strips[0].id;
        assert!(record_shot(&mut s, sheet, not_a_break, crate::Eighths(1), false).is_err());
    }

    #[test]
    fn what_was_shot_is_checked_counted_and_dropped_with_its_day() {
        use crate::days::remove_day_breaks;
        let (mut s, board, sub, _) = crate::timeline::tests::split_two_days();
        let (d1, d2) = (breaks(&s)[0], breaks(&s)[1]);
        let sheet = s.sheets[0].id;
        s.plan.days_shot.push(DayShot {
            day: d1,
            date: None,
            call: Some(400),
            wrap: Some(1460),
        });
        for (day, eighths, complete) in [(d1, 3, false), (d2, 2, true)] {
            s.plan.scenes_shot.push(SceneShot {
                sheet,
                day,
                eighths: crate::Eighths(eighths),
                complete,
            });
        }
        s.check().unwrap();
        assert_eq!(s.plan.eighths_shot(sheet), crate::Eighths(5));
        assert!(s.plan.is_complete(sheet));
        assert_eq!(s.plan.day_shot(d1).and_then(|d| d.wrap), Some(1460));
        // Finished twice, or a wrap before its call: refused.
        let mut bad = s.clone();
        bad.plan.scenes_shot[0].complete = true;
        assert!(bad.check().is_err());
        let mut bad = s.clone();
        bad.plan.days_shot[0].wrap = Some(300);
        assert!(bad.check().is_err());
        // The day breaks go, and what was shot on them goes with them.
        remove_day_breaks(&mut s, board, sub).unwrap();
        assert!(s.plan.days_shot.is_empty() && s.plan.scenes_shot.is_empty());
    }
}
