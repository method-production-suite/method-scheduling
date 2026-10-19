//! The `plan` key: planning data for the features beyond the reference app
//! (method_schedule_core::plan; docs/FORMAT.md). Left out while empty.

use serde::{Deserialize, Serialize};

use method_schedule_core::model::{BoardId, CategoryId, ElementId, SheetId, StripId, SubBoardId};
use method_schedule_core::plan::{
    CallTime, DayShot, GeoLocation, Lock, LockedScene, Plan, SceneShot, Turnaround,
    DEFAULT_LONG_DAY, MAX_WRAP,
};
use method_schedule_core::Eighths;

use method_schedule_core::solar::DaylightSaving;

use crate::{date, default_given, FormatError};

fn is_zero_i(n: &i32) -> bool {
    *n == 0
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilePlan {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    locations: Vec<FileLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    call_times: Vec<FileCallTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    long_day_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    turnarounds: Vec<FileTurnaround>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    locks: Vec<FileLock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pinned: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    days_shot: Vec<FileDayShot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scenes_shot: Vec<FileSceneShot>,
}

/// A shoot day as shot (decision 224): what a production report wrote back.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDayShot {
    day: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    call: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrap: Option<u32>,
}

/// Part of a scene shot on one day (decision 224).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSceneShot {
    sheet: u32,
    day: u32,
    eighths: u32,
    /// Left out when this day didn't finish the scene; `false` written is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    complete: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLocation {
    name: String,
    /// Millionths of a degree, north positive.
    latitude_microdeg: i32,
    /// Millionths of a degree, east positive.
    longitude_microdeg: i32,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    utc_offset_minutes: i32,
    /// A daylight saving rule's name (decision 193); left out for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    daylight_saving: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileCallTime {
    /// The day break strip that closes the day.
    day: u32,
    /// Minutes after midnight.
    minutes: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileTurnaround {
    category: u32,
    minutes: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLock {
    name: String,
    board: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    made: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scenes: Vec<FileLockedScene>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLockedScene {
    sheet: u32,
    sub_board: u32,
    day: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cast: Vec<u32>,
}

pub(crate) fn out_plan(p: &Plan) -> Option<FilePlan> {
    if p.is_empty() {
        return None;
    }
    Some(FilePlan {
        days_shot: p
            .days_shot
            .iter()
            .map(|d| FileDayShot {
                day: d.day.0,
                date: d.date.map(|x| x.to_string()),
                call: d.call,
                wrap: d.wrap,
            })
            .collect(),
        scenes_shot: p
            .scenes_shot
            .iter()
            .map(|x| FileSceneShot {
                sheet: x.sheet.0,
                day: x.day.0,
                eighths: x.eighths.0,
                complete: x.complete.then_some(true),
            })
            .collect(),
        locations: p
            .locations
            .iter()
            .map(|l| FileLocation {
                name: l.name.clone(),
                latitude_microdeg: l.latitude,
                longitude_microdeg: l.longitude,
                utc_offset_minutes: l.utc_offset,
                daylight_saving: l.daylight_saving.map(|r| r.name().to_string()),
            })
            .collect(),
        call_times: p
            .call_times
            .iter()
            .map(|c| FileCallTime {
                day: c.day.0,
                minutes: c.minutes,
            })
            .collect(),
        long_day_minutes: p.long_day,
        turnarounds: p
            .turnarounds
            .iter()
            .map(|t| FileTurnaround {
                category: t.category.0,
                minutes: t.minutes,
            })
            .collect(),
        locks: p
            .locks
            .iter()
            .map(|l| FileLock {
                name: l.name.clone(),
                board: l.board.0,
                made: l.made.map(|d| d.to_string()),
                scenes: l
                    .scenes
                    .iter()
                    .map(|sc| FileLockedScene {
                        sheet: sc.sheet.0,
                        sub_board: sc.sub_board.0,
                        day: sc.day,
                        date: sc.date.map(|d| d.to_string()),
                        cast: sc.cast.iter().map(|e| e.0).collect(),
                    })
                    .collect(),
            })
            .collect(),
        pinned: p.pinned.iter().map(|x| x.0).collect(),
        omitted: p.omitted.iter().map(|x| x.0).collect(),
    })
}

fn bad(why: String) -> FormatError {
    FormatError::BadValue(format!("plan: {why}"))
}

pub(crate) fn in_plan(f: Option<FilePlan>) -> Result<Plan, FormatError> {
    let Some(f) = f else {
        return Ok(Plan::default());
    };
    let mut p = Plan::default();
    for l in f.locations {
        let name = l.name.clone();
        if name.trim() != name {
            return Err(bad(format!(
                "location {name:?} has spaces around it; the name is written trimmed"
            )));
        }
        if p.location(&name).is_some() {
            return Err(bad(format!("location {name:?} is listed twice")));
        }
        // Only the names the writer writes: none is left out, not written.
        let daylight_saving = match l.daylight_saving.as_deref() {
            None => None,
            Some("none") => return Err(default_given("daylight_saving", "none")),
            Some(t) => Some(
                DaylightSaving::ALL
                    .into_iter()
                    .find(|r| r.name() == t)
                    .ok_or_else(|| {
                        bad(format!(
                            "location {name:?}: daylight saving {t:?} is not us_canada, eu, australia or new_zealand"
                        ))
                    })?,
            ),
        };
        p.set_location(GeoLocation {
            name: l.name,
            latitude: l.latitude_microdeg,
            longitude: l.longitude_microdeg,
            utc_offset: l.utc_offset_minutes,
            daylight_saving,
        })
        .map_err(|e| bad(format!("location {name:?}: {e}")))?;
    }
    for c in f.call_times {
        if c.minutes >= 24 * 60 {
            return Err(bad(format!("call time {} is past 23:59", c.minutes)));
        }
        if p.call_times.iter().any(|x| x.day.0 == c.day) {
            return Err(bad(format!("day {} has two call times", c.day)));
        }
        p.call_times.push(CallTime {
            day: StripId(c.day),
            minutes: c.minutes,
        });
    }
    if let Some(m) = f.long_day_minutes {
        if m == 0 || m > 24 * 60 {
            return Err(bad(format!("long day limit {m} is out of range")));
        }
        if m == DEFAULT_LONG_DAY {
            return Err(default_given("long_day_minutes", &m.to_string()));
        }
    }
    p.long_day = f.long_day_minutes;
    for t in f.turnarounds {
        if t.minutes == 0 || t.minutes > 48 * 60 {
            return Err(bad(format!("turnaround {} is out of range", t.minutes)));
        }
        if p.turnarounds.iter().any(|x| x.category.0 == t.category) {
            return Err(bad(format!("category {} has two turnarounds", t.category)));
        }
        p.turnarounds.push(Turnaround {
            category: CategoryId(t.category),
            minutes: t.minutes,
        });
    }
    for l in f.locks {
        if l.name.trim().is_empty() || p.lock(&l.name).is_some() {
            return Err(bad(format!(
                "lock name {:?} is empty or used twice",
                l.name
            )));
        }
        let scenes = l
            .scenes
            .into_iter()
            .map(|sc| {
                Ok(LockedScene {
                    sheet: SheetId(sc.sheet),
                    sub_board: SubBoardId(sc.sub_board),
                    day: sc.day,
                    date: sc.date.as_deref().map(date).transpose()?,
                    cast: sc.cast.into_iter().map(ElementId).collect(),
                })
            })
            .collect::<Result<_, FormatError>>()?;
        p.locks.push(Lock {
            name: l.name,
            board: BoardId(l.board),
            made: l.made.as_deref().map(date).transpose()?,
            scenes,
        });
    }
    for d in f.days_shot {
        if p.days_shot.iter().any(|x| x.day.0 == d.day) {
            return Err(bad(format!("day {} is reported shot twice", d.day)));
        }
        if d.date.is_none() && d.call.is_none() && d.wrap.is_none() {
            return Err(bad(format!(
                "day {} is reported shot with no date, call or wrap: leave it out",
                d.day
            )));
        }
        if d.call.is_some_and(|m| m >= 24 * 60) {
            return Err(bad(format!("day {}'s actual call is past 23:59", d.day)));
        }
        if d.wrap.is_some_and(|m| m == 0 || m > MAX_WRAP)
            || matches!((d.call, d.wrap), (Some(c), Some(w)) if w <= c)
        {
            return Err(bad(format!(
                "day {}'s actual wrap is out of range (1 to {MAX_WRAP}) or not after its call",
                d.day
            )));
        }
        p.days_shot.push(DayShot {
            day: StripId(d.day),
            date: d.date.as_deref().map(date).transpose()?,
            call: d.call,
            wrap: d.wrap,
        });
    }
    for x in f.scenes_shot {
        if x.complete == Some(false) {
            return Err(default_given("complete", "false"));
        }
        let complete = x.complete == Some(true);
        if p.scenes_shot
            .iter()
            .any(|o| o.sheet.0 == x.sheet && o.day.0 == x.day)
        {
            return Err(bad(format!(
                "sheet {} is reported shot twice on day {}",
                x.sheet, x.day
            )));
        }
        if complete
            && p.scenes_shot
                .iter()
                .any(|o| o.sheet.0 == x.sheet && o.complete)
        {
            return Err(bad(format!("sheet {} is reported finished twice", x.sheet)));
        }
        if x.eighths == 0 && !complete {
            return Err(bad(format!(
                "sheet {} is reported shot on day {} with no eighths and not finished: leave it out",
                x.sheet, x.day
            )));
        }
        if x.eighths > method_schedule_core::model::MAX_SHEET_EIGHTHS {
            return Err(bad(format!(
                "sheet {} reports too many eighths shot",
                x.sheet
            )));
        }
        p.scenes_shot.push(SceneShot {
            sheet: SheetId(x.sheet),
            day: StripId(x.day),
            eighths: Eighths(x.eighths),
            complete,
        });
    }
    p.pinned = f.pinned.into_iter().map(StripId).collect();
    p.omitted = f.omitted.into_iter().map(SheetId).collect();
    if p.is_empty() {
        return Err(default_given("plan", "{}"));
    }
    Ok(p)
}
