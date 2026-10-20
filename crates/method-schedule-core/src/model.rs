//! The schedule: categories, elements, breakdown sheets and boards.
//!
//! Everything carries a stable id, so files diff and merge by identity. Every board holds
//! every sheet exactly once, in a sub-board or in its Boneyard (`Schedule::check`).

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::fountain::{ImportedScript, IntExt};
use crate::{Date, Eighths, Weekday};

macro_rules! id_type {
    ($($name:ident),*) => {$(
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);
    )*};
}
id_type!(
    CategoryId,
    ElementId,
    SheetId,
    BoardId,
    SubBoardId,
    StripId,
    RedFlagTypeId,
    RedFlagId
);

/// A quick hasher for the ids above (one `u32` each), for the indexes a board view or a
/// check builds every edit (decision 190). The ids are the schedule's own, not anyone's
/// input to collide on purpose, so the standard hasher's guard is not needed here. Bytes
/// of anything else are folded in a word at a time.
#[derive(Clone, Copy, Default)]
pub struct IdHasher(u64);

impl IdHasher {
    const K: u64 = 0x517c_c1b7_2722_0a95;

    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::K);
    }
}

impl std::hash::Hasher for IdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Builds [`IdHasher`]s.
pub type IdHash = std::hash::BuildHasherDefault<IdHasher>;
/// A map keyed by ids, with [`IdHasher`].
pub type IdMap<K, V> = HashMap<K, V, IdHash>;
/// A set of ids, with [`IdHasher`].
pub type IdSet<T> = HashSet<T, IdHash>;

/// Hold and drop switches for the day out of days. Held per element; a category's copy is
/// the default for elements created in it. Off by default, minimum gap 1 (D10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DoodRules {
    pub holds: bool,
    pub drops: bool,
    /// The shortest gap, in calendar days, that may become a drop.
    pub min_drop_gap: u32,
}

impl Default for DoodRules {
    fn default() -> DoodRules {
        DoodRules {
            holds: false,
            drops: false,
            min_drop_gap: 1,
        }
    }
}

/// Which dates a sub-board shoots on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calendar {
    /// Weekdays that are never shoot days. Saturday and Sunday by default.
    pub weekly_off: Vec<Weekday>,
    /// Dates marked back to working although they fall on a weekly day off.
    pub work_days: Vec<Date>,
    /// Single days off.
    pub off_days: Vec<Date>,
    pub holidays: Vec<Date>,
    /// Company travel days. Like holidays they take no shoot day number.
    pub moves: Vec<Date>,
    pub events: Vec<Event>,
    /// The first day of prep, for the budget exchange (optional, decided by the user).
    pub prep_start: Option<Date>,
    /// The last day of wrap, for the budget exchange (optional).
    pub wrap_end: Option<Date>,
}

impl Default for Calendar {
    fn default() -> Calendar {
        Calendar {
            weekly_off: vec![Weekday::Sat, Weekday::Sun],
            work_days: Vec::new(),
            off_days: Vec::new(),
            holidays: Vec::new(),
            moves: Vec::new(),
            events: Vec::new(),
            prep_start: None,
            wrap_end: None,
        }
    }
}

/// What an event means for the day out of days.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Counts as worked; shows TR.
    Travel,
    /// Counts as worked; shows FT.
    Fitting,
    /// Counts as worked; shows R (code not yet seen in the real app).
    Rehearsal,
    /// Dates the element can't work: shown on the calendar, counted nowhere and never
    /// shown on the day out of days (behavior note 17, A8).
    Unavailable,
    /// A camera test: shown on the calendar, counted nowhere (behavior note 17, A8).
    CameraTest,
    /// Any other kind: shown on the calendar, counted nowhere.
    Other(String),
    /// Post-production work (behavior note 22): shown on the calendar, never counted in
    /// the day out of days, never a conflict.
    Post(PostKind),
}

/// The kinds of post-production work an event can be (behavior note 22; our own list of
/// generic kinds, decision 109; color correction from behavior note 27, decision 163).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PostKind {
    PostProduction,
    VisualEffects,
    Dubbing,
    SyncSound,
    MusicRecording,
    PictureEditing,
    SoundEditing,
    SoundMixing,
    TitlesGraphics,
    Trailer,
    ColorCorrection,
}

impl PostKind {
    pub const ALL: [PostKind; 11] = [
        PostKind::PostProduction,
        PostKind::VisualEffects,
        PostKind::Dubbing,
        PostKind::SyncSound,
        PostKind::MusicRecording,
        PostKind::PictureEditing,
        PostKind::SoundEditing,
        PostKind::SoundMixing,
        PostKind::TitlesGraphics,
        PostKind::Trailer,
        PostKind::ColorCorrection,
    ];

    /// The kind's key in the file and over MCP: `post_production`, `visual_effects`...
    pub fn key(self) -> &'static str {
        match self {
            PostKind::PostProduction => "post_production",
            PostKind::VisualEffects => "visual_effects",
            PostKind::Dubbing => "dubbing",
            PostKind::SyncSound => "sync_sound",
            PostKind::MusicRecording => "music_recording",
            PostKind::PictureEditing => "picture_editing",
            PostKind::SoundEditing => "sound_editing",
            PostKind::SoundMixing => "sound_mixing",
            PostKind::TitlesGraphics => "titles_graphics",
            PostKind::Trailer => "trailer",
            PostKind::ColorCorrection => "color_correction",
        }
    }

    /// The kind in words: `Visual effects`.
    pub fn label(self) -> &'static str {
        match self {
            PostKind::PostProduction => "Post-production",
            PostKind::VisualEffects => "Visual effects",
            PostKind::Dubbing => "Dubbing",
            PostKind::SyncSound => "Sync sound",
            PostKind::MusicRecording => "Music recording",
            PostKind::PictureEditing => "Picture editing",
            PostKind::SoundEditing => "Sound editing",
            PostKind::SoundMixing => "Sound mixing",
            PostKind::TitlesGraphics => "Titles and graphics",
            PostKind::Trailer => "Trailer",
            PostKind::ColorCorrection => "Color correction",
        }
    }

    pub fn from_key(key: &str) -> Option<PostKind> {
        PostKind::ALL.into_iter().find(|k| k.key() == key)
    }
}

/// The ten colors an event can take (behavior note 22; our own palette, decision 109):
/// key in the file, name in words, and the color.
pub const EVENT_COLORS: [(&str, &str, Rgb); 10] = [
    ("red", "Red", Rgb(0xd64545)),
    ("orange", "Orange", Rgb(0xe8833a)),
    ("yellow", "Yellow", Rgb(0xe0b62c)),
    ("green", "Green", Rgb(0x4c9a52)),
    ("teal", "Teal", Rgb(0x2a9d8f)),
    ("blue", "Blue", Rgb(0x3b73c4)),
    ("indigo", "Indigo", Rgb(0x5a55b5)),
    ("purple", "Purple", Rgb(0x9050b0)),
    ("pink", "Pink", Rgb(0xd0598f)),
    ("brown", "Brown", Rgb(0x8a6142)),
];

/// The index into [`EVENT_COLORS`] of the color keyed `key`.
pub fn event_color(key: &str) -> Option<u8> {
    EVENT_COLORS
        .iter()
        .position(|(k, _, _)| *k == key)
        .map(|i| i as u8)
}

impl EventKind {
    /// The day out of days code, for kinds that count.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            EventKind::Travel => Some("TR"),
            EventKind::Fitting => Some("FT"),
            EventKind::Rehearsal => Some("R"),
            EventKind::Unavailable
            | EventKind::CameraTest
            | EventKind::Other(_)
            | EventKind::Post(_) => None,
        }
    }

    /// The kind's name in the file and over MCP: `travel`, `fitting`, `rehearsal`,
    /// `unavailable`, `camera_test`, `other`, or a post-production kind's key
    /// ([`PostKind::key`]).
    pub fn key(&self) -> &'static str {
        match self {
            EventKind::Travel => "travel",
            EventKind::Fitting => "fitting",
            EventKind::Rehearsal => "rehearsal",
            EventKind::Unavailable => "unavailable",
            EventKind::CameraTest => "camera_test",
            EventKind::Other(_) => "other",
            EventKind::Post(k) => k.key(),
        }
    }

    /// The kind from its key, with `label` saying what an `other` event is. `None` for
    /// an unknown key.
    pub fn from_key(key: &str, label: &str) -> Option<EventKind> {
        Some(match key {
            "travel" => EventKind::Travel,
            "fitting" => EventKind::Fitting,
            "rehearsal" => EventKind::Rehearsal,
            "unavailable" => EventKind::Unavailable,
            "camera_test" => EventKind::CameraTest,
            "other" => EventKind::Other(label.to_string()),
            _ => EventKind::Post(PostKind::from_key(key)?),
        })
    }

    /// The kind in words: `Travel`, `Camera test`, or what an `other` event is called
    /// (`Other` when it has no label).
    pub fn label(&self) -> String {
        match self {
            EventKind::Travel => "Travel".to_string(),
            EventKind::Fitting => "Fitting".to_string(),
            EventKind::Rehearsal => "Rehearsal".to_string(),
            EventKind::Unavailable => "Unavailable".to_string(),
            EventKind::CameraTest => "Camera test".to_string(),
            EventKind::Other(l) if l.trim().is_empty() => "Other".to_string(),
            EventKind::Other(l) => l.clone(),
            EventKind::Post(k) => k.label().to_string(),
        }
    }

    /// Every kind's key, in the order the app lists them: the counting kinds, unavailable,
    /// camera test, the post-production kinds, then other.
    pub fn keys() -> Vec<&'static str> {
        let mut out = vec![
            "travel",
            "fitting",
            "rehearsal",
            "unavailable",
            "camera_test",
        ];
        out.extend(PostKind::ALL.iter().map(|k| k.key()));
        out.push("other");
        out
    }

    /// The kinds a new schedule's built-in red flag types match, in their order: every
    /// kind but `other` (behavior note 23; decision 113). Red flag types and event kinds
    /// are one word list, so the two never drift apart.
    pub fn builtin_flag_kinds() -> Vec<EventKind> {
        let mut out = vec![
            EventKind::Travel,
            EventKind::Fitting,
            EventKind::Rehearsal,
            EventKind::Unavailable,
            EventKind::CameraTest,
        ];
        out.extend(PostKind::ALL.iter().map(|k| EventKind::Post(*k)));
        out
    }
}

/// Something elements do on a run of dates outside the strips: travel, a fitting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub kind: EventKind,
    pub from: Date,
    /// Inclusive.
    pub to: Date,
    pub elements: Vec<ElementId>,
    /// An index into [`EVENT_COLORS`], or none (the kind's usual look).
    pub color: Option<u8>,
}

/// The most days one event covers, `from` and `to` included. Every day of an event is a
/// column in the day out of days, so a slip of the year (2207 for 2027) can't make one
/// thousands of days long (decision 21).
pub const MAX_EVENT_DAYS: i32 = 366;

/// The most red flags one [`Schedule::add_red_flags`] adds (elements times runs of
/// dates), so a slip with every element and every other date checked can't bury the
/// schedule under hundreds of thousands of flags.
pub const MAX_RED_FLAGS_AT_ONCE: usize = 10_000;

impl Event {
    /// What a list calls the event: its name, or its kind in words when it has none
    /// (`Travel`, `Camera test`), so a nameless event is never an empty line or link.
    pub fn title(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            self.kind.label()
        } else {
            name.to_string()
        }
    }
}

/// What one date is on a sub-board's calendar. A date is exactly one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayKind {
    /// A working date: a shoot day.
    Work,
    /// A weekly day off or a single day off.
    Off,
    Holiday,
    /// A company travel day.
    Move,
}

impl Calendar {
    pub fn is_holiday(&self, d: Date) -> bool {
        self.holidays.contains(&d)
    }

    /// A weekly day off (unless marked back to working) or a single day off. Holidays are
    /// separate.
    pub fn is_off(&self, d: Date) -> bool {
        (self.weekly_off.contains(&d.weekday()) && !self.work_days.contains(&d))
            || self.off_days.contains(&d)
    }

    /// What `d` is: a company travel day, a holiday, a day off, or a working date.
    pub fn day_kind(&self, d: Date) -> DayKind {
        if self.is_move(d) {
            DayKind::Move
        } else if self.is_holiday(d) {
            DayKind::Holiday
        } else if self.is_off(d) {
            DayKind::Off
        } else {
            DayKind::Work
        }
    }

    /// Makes `d` a working date, a day off, a holiday or a company travel day, dropping
    /// whatever it was. On a weekly day off, working means marked back to working; on a
    /// working weekday, a day off is a single day off. Lists stay sorted, each date once.
    pub fn set_day_kind(&mut self, d: Date, kind: DayKind) {
        for list in [
            &mut self.work_days,
            &mut self.off_days,
            &mut self.holidays,
            &mut self.moves,
        ] {
            list.retain(|x| *x != d);
        }
        let weekly = self.weekly_off.contains(&d.weekday());
        let list = match kind {
            DayKind::Work if weekly => Some(&mut self.work_days),
            DayKind::Off if !weekly => Some(&mut self.off_days),
            DayKind::Holiday => Some(&mut self.holidays),
            DayKind::Move => Some(&mut self.moves),
            _ => None,
        };
        if let Some(list) = list {
            let at = list.partition_point(|x| *x < d);
            list.insert(at, d);
        }
    }

    /// Sets the weekly days off (Monday first, each once). Refused when it would leave no
    /// working weekday. Dates already marked keep their marks.
    pub fn set_weekly_off(&mut self, days: &[Weekday]) -> Result<(), Error> {
        let off: Vec<Weekday> = Weekday::ALL
            .iter()
            .copied()
            .filter(|w| days.contains(w))
            .collect();
        if off.len() == Weekday::ALL.len() {
            return Err(Error::Broken(
                "the week needs at least one working day".to_string(),
            ));
        }
        self.weekly_off = off;
        Ok(())
    }

    pub fn is_move(&self, d: Date) -> bool {
        self.moves.contains(&d)
    }

    /// What is wrong with the calendar's marks, if anything: every weekday off (no shoot
    /// day at all), the weekly days off out of order or given twice, or a mark list
    /// (back to working, days off, holidays, company travel) out of date order, with a
    /// date twice, or sharing a date with another list. A date is exactly one kind
    /// (decision 21), and [`Calendar::set_day_kind`] and
    /// [`Calendar::set_weekly_off`] keep it so.
    pub fn problem(&self) -> Option<String> {
        if self.weekly_off.len() >= Weekday::ALL.len() {
            return Some("the week needs at least one working day".to_string());
        }
        let place = |w: &Weekday| Weekday::ALL.iter().position(|x| x == w);
        if !self
            .weekly_off
            .windows(2)
            .all(|p| place(&p[0]) < place(&p[1]))
        {
            return Some("the weekly days off are out of order or given twice".to_string());
        }
        let lists = [
            ("dates back to working", &self.work_days),
            ("days off", &self.off_days),
            ("holidays", &self.holidays),
            ("company travel days", &self.moves),
        ];
        for (name, list) in lists {
            if !list.windows(2).all(|p| p[0] < p[1]) {
                return Some(format!("its {name} are out of order or list a date twice"));
            }
        }
        for (i, (a, x)) in lists.iter().enumerate() {
            for (b, y) in &lists[i + 1..] {
                if let Some(d) = x.iter().find(|d| y.binary_search(d).is_ok()) {
                    return Some(format!("{d} is in both its {a} and its {b}"));
                }
            }
        }
        None
    }

    pub fn is_shoot_day(&self, d: Date) -> bool {
        !self.is_off(d) && !self.is_holiday(d) && !self.is_move(d)
    }

    /// The date of shoot day `n` (from 1): the nth shoot day on or after `start`. `None`
    /// when the week has no shoot days at all.
    pub fn date_of_day(&self, start: Date, n: u32) -> Option<Date> {
        if Weekday::ALL.iter().all(|w| self.weekly_off.contains(w)) || n == 0 {
            return None;
        }
        let mut d = start;
        let mut left = n;
        loop {
            if self.is_shoot_day(d) {
                left -= 1;
                if left == 0 {
                    return Some(d);
                }
            }
            d = d.add_days(1);
        }
    }

    /// The dates of shoot days asked for in rising order, each walked on from the last
    /// rather than from `start` again (decision 190): the same dates as
    /// [`Calendar::date_of_day`], in time linear in the board's days.
    pub fn day_dates(&self, start: Date) -> DayDates<'_> {
        DayDates {
            calendar: self,
            start,
            last: None,
        }
    }
}

/// [`Calendar::day_dates`].
pub struct DayDates<'a> {
    calendar: &'a Calendar,
    start: Date,
    last: Option<(u32, Date)>,
}

impl DayDates<'_> {
    /// The date of shoot day `n`, as [`Calendar::date_of_day`] gives it.
    pub fn date(&mut self, n: u32) -> Option<Date> {
        let found = match self.last {
            Some((m, d)) if n > m => self.calendar.date_of_day(d.add_days(1), n - m),
            Some((m, d)) if n == m => Some(d),
            _ => self.calendar.date_of_day(self.start, n),
        };
        if let Some(d) = found {
            self.last = Some((n, d));
        }
        found
    }
}

/// The name a new schedule gives its cast category. The cast category is built in (the
/// board shows its board IDs; script import fills it): it can't be moved or deleted, but
/// it can be renamed (behavior note 17, A10), so the engine knows it by
/// [`Category::cast`], never by name.
pub const CAST_NAME: &str = "Cast";

/// Our own default category list for a new schedule, after the cast (inventory section
/// 4): each name, whether new elements are numbered automatically, and whether
/// quantities count in the daily totals. Ordered as a breakdown is read: people first,
/// then what is in front of the camera, then what makes the picture, then the rest. Not
/// any other product's list; informed by the groups of the open category id proposal
/// (interop-formats.md, section 4).
/// The properties the cast category starts with in a new schedule (behavior note 24:
/// the reference app's cast comes with contact and deal fields; our own names), each with
/// whether it takes several lines (decision 119).
pub const CAST_PROPERTIES: &[(&str, bool)] = &[
    ("Full name", false),
    ("Address", true),
    ("Phone", false),
    ("Agent", false),
    ("Agent phone", false),
    ("Rate", false),
    ("Rate per", false),
    ("Deal notes", true),
];

pub const DEFAULT_CATEGORIES: &[(&str, bool, bool)] = &[
    ("Background", false, true),
    ("Stand-ins", false, true),
    ("Stunts", true, false),
    ("Hand props", false, false),
    ("Set dressing", false, false),
    ("Wardrobe", false, false),
    ("Hair and makeup", false, false),
    ("Picture vehicles", false, false),
    ("Animals", false, true),
    ("Practical effects", false, false),
    ("Visual effects", false, false),
    ("Camera", false, false),
    ("Grip and electric", false, false),
    ("Sound and music", false, false),
    ("Special equipment", false, false),
    ("Extra crew", false, true),
    ("Safety", false, false),
    ("Other", false, false),
];

/// The longest custom property name, in characters.
pub const MAX_PROPERTY_NAME: usize = 60;
/// The longest board ID one sets, in characters.
pub const MAX_BOARD_ID: usize = 12;

/// Whether `b` is a board ID one may set: 1 to [`MAX_BOARD_ID`] ASCII letters and
/// digits, like `12` or `12A` (inventory section 4).
pub fn board_id_ok(b: &str) -> bool {
    (1..=MAX_BOARD_ID).contains(&b.len()) && b.bytes().all(|c| c.is_ascii_alphanumeric())
}

/// The most pages one sheet holds, in eighths: 999 pages, far past any scene, so that
/// totals of many sheets never come near the end of a number.
pub const MAX_SHEET_EIGHTHS: u32 = 999 * 8;
/// The longest shoot time of a sheet and duration of a banner: a day (decision 50).
pub const MAX_SHOOT_MINUTES: u32 = 24 * 60;
/// The longest screen time of a sheet: a day of running time.
pub const MAX_SCREEN_SECONDS: u32 = 24 * 60 * 60;
/// The largest quantity of an element on a sheet.
pub const MAX_QUANTITY: u32 = 99_999;
/// The highest `next_id` a schedule may have: ids stay far from the end of a number, so
/// adding never overflows.
pub const MAX_NEXT_ID: u32 = 1_000_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    /// New elements get the next board ID: one more than the highest whole number.
    pub auto_board_id: bool,
    pub exclude_from_board: bool,
    pub defaults: DoodRules,
    /// The built-in cast category, whatever its name. At most one per schedule.
    pub cast: bool,
    /// Its elements' quantities on each sheet add up in the daily totals; off, each
    /// element counts once a day (inventory section 3).
    pub daily_quantity: bool,
    /// Custom element properties: the names of the fields each of its elements may fill
    /// in, in order, unique ignoring case.
    pub properties: Vec<String>,
    /// The properties whose values take several lines (decision 119), by their names in
    /// `properties`, in that order; the others take one line.
    pub multi_line: Vec<String>,
}

impl Category {
    /// A category the app relies on, which can't be moved or deleted: the cast category.
    pub fn is_built_in(&self) -> bool {
        self.cast
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub id: ElementId,
    pub category: CategoryId,
    pub name: String,
    /// Cast numbers and the like. Letters allowed; unique within the category.
    pub board_id: Option<String>,
    pub board_id_locked: bool,
    /// Left off the board on its own, whatever its category says (behavior note 21):
    /// excluded when this or its category's setting is on.
    pub exclude_from_board: bool,
    pub rules: DoodRules,
    pub notes: String,
    pub links: Vec<ElementId>,
    /// Dates the element can't (or would rather not) work, in the order added.
    pub red_flags: Vec<RedFlag>,
    /// Values of its category's custom properties, in the category's order; a property
    /// with no value is left out.
    pub properties: Vec<PropertyValue>,
    /// Image files, as paths: relative to the schedule file's folder, or absolute. The
    /// images themselves stay outside the schedule file.
    pub images: Vec<String>,
}

/// One custom property's value on an element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyValue {
    /// A property of the element's category.
    pub name: String,
    pub value: String,
}

/// How many of an element a sheet needs (35 background), and whether that number adds
/// to the day's total for its category (inventory section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantity {
    /// An element on the sheet.
    pub element: ElementId,
    /// At least 1.
    pub count: u32,
    /// Counts in the daily totals. Off for a group already counted on another sheet that
    /// day.
    pub daily: bool,
}

/// A kind of red flag, from the schedule's own list (`Schedule::red_flag_types`): what a
/// person calls the limit ("Unavailable", "Second job", "Child hours"). Global across
/// boards. A type may have a color its marks are drawn in. New schedules start with a
/// built-in type for each event kind but `other` ([`EventKind::builtin_flag_kinds`];
/// behavior note 23, decision 113): those can be renamed and recolored, never deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedFlagType {
    pub id: RedFlagTypeId,
    /// Unique ignoring case.
    pub name: String,
    /// The color its marks are drawn in; `None` draws them in the usual red.
    pub color: Option<Rgb>,
    /// For a built-in type, the event kind whose word it is (never `other`). The type
    /// and the kind stay apart: a red flag warns, an event books the element and counts
    /// (decision 113).
    pub builtin: Option<EventKind>,
}

/// The dates a red flag rules out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedFlagDates {
    /// From `from` to `to`, both included. One date when they are equal.
    Range { from: Date, to: Date },
    /// Every date before this one: the element can't start work until it.
    NotBefore(Date),
    /// Every date after this one: the element is gone after it.
    NotAfter(Date),
}

impl RedFlagDates {
    /// Whether the flag rules out `d`.
    pub fn covers(&self, d: Date) -> bool {
        match *self {
            RedFlagDates::Range { from, to } => from <= d && d <= to,
            RedFlagDates::NotBefore(x) => d < x,
            RedFlagDates::NotAfter(x) => d > x,
        }
    }

    /// In words: `on 2027-03-02`, `2027-03-02 to 2027-03-04`, `not before 2027-03-10`.
    pub fn text(&self) -> String {
        match *self {
            RedFlagDates::Range { from, to } if from == to => format!("on {from}"),
            RedFlagDates::Range { from, to } => format!("{from} to {to}"),
            RedFlagDates::NotBefore(x) => format!("not before {x}"),
            RedFlagDates::NotAfter(x) => format!("not after {x}"),
        }
    }
}

/// An availability limit on one element: a red flag (inventory section 9; decision 33).
/// A warning only: nothing is ever blocked. `hard` says how firm the limit is, for people
/// and lists; the engine treats both the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedFlag {
    /// Stable. `RedFlagId(0)` asks `Schedule::set_red_flag` for a new one.
    pub id: RedFlagId,
    pub kind: RedFlagTypeId,
    pub dates: RedFlagDates,
    pub note: String,
    /// A hard limit (the element can't work then) rather than a soft one (it would rather
    /// not).
    pub hard: bool,
}

/// Minutes, for shoot time and banner durations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Minutes(pub u32);

/// Seconds, for screen time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seconds(pub u32);

/// A breakdown sheet. Every header field is optional; pick-list fields (I/E, time of day)
/// are free text whose values the app offers again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheet {
    pub id: SheetId,
    /// The sheet number as typed (`12`, `12A`): an index of the user's own, apart from
    /// the scene numbers (decision 77). Sheets can be stepped through by it.
    pub number: String,
    /// Scene numbers as written: `12`, `12A`, `12-14`, `12, 15`.
    pub scenes: String,
    pub int_ext: String,
    pub set: String,
    pub time_of_day: String,
    /// Minutes after midnight.
    pub sunrise: Option<u32>,
    pub sunset: Option<u32>,
    pub pages: Eighths,
    pub shoot_time: Option<Minutes>,
    pub synopsis: String,
    pub location: String,
    pub script_pages: String,
    pub script_day: String,
    pub screen_time: Option<Seconds>,
    pub unit: String,
    pub sequence: String,
    pub episode: String,
    pub comments: String,
    pub elements: Vec<ElementId>,
    /// Quantities of elements on the sheet. An element with none is one, counted daily.
    pub quantities: Vec<Quantity>,
    /// Image paths for the scene, as an element's are: relative to the schedule file's
    /// folder or absolute. The images stay outside the schedule file.
    pub images: Vec<String>,
}

impl Sheet {
    /// The scene heading as a script writes it: `INT. KITCHEN - DAY`, from I/E, set and
    /// time of day (any of them may be empty; empty when all are).
    pub fn heading(&self) -> String {
        let place = self.place();
        match (place.is_empty(), self.time_of_day.trim()) {
            (_, "") => place,
            (true, tod) => tod.to_string(),
            (false, tod) => format!("{place} - {tod}"),
        }
    }

    /// `INT. KITCHEN`: I/E (a plain word takes a full stop) and set.
    fn place(&self) -> String {
        let ie = self.int_ext.trim();
        let ie = if !ie.is_empty() && ie.chars().all(|c| c.is_ascii_alphabetic()) {
            format!("{ie}.")
        } else {
            ie.to_string()
        };
        [ie.as_str(), self.set.trim()]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// How a person knows the sheet, for messages (BYD-01, decision 162): `Scene 26, INT.
    /// KITCHEN` (`Scenes 3, 3A, ...` for several), else its heading, else its sheet
    /// number as typed (`Sheet 12A`), never the sheet's id.
    pub fn title(&self) -> String {
        let scenes = self.scenes.trim();
        if !scenes.is_empty() {
            let word = if scenes.contains([',', '-']) {
                "Scenes"
            } else {
                "Scene"
            };
            let place = self.place();
            return if place.is_empty() {
                format!("{word} {scenes}")
            } else {
                format!("{word} {scenes}, {place}")
            };
        }
        let heading = self.heading();
        if !heading.is_empty() {
            heading
        } else if !self.number.trim().is_empty() {
            format!("Sheet {}", self.number.trim())
        } else {
            "A scene with no number or heading".to_string()
        }
    }

    /// How many of `element` the sheet needs, and whether it counts daily: its quantity,
    /// or one, counted.
    pub fn quantity(&self, element: ElementId) -> (u32, bool) {
        self.quantities
            .iter()
            .find(|q| q.element == element)
            .map_or((1, true), |q| (q.count, q.daily))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Banner {
    pub text: String,
    /// Counts toward the day's time.
    pub duration: Minutes,
    /// How it is drawn (decision 65).
    pub style: BannerStyle,
}

/// A banner's look (inventory section 5, decisions 65 and 87): bold text, a color from a
/// fixed palette (none: the board's banner color), one or two lines high, a text size
/// step and where the text sits (`Auto` is the left).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BannerStyle {
    pub bold: bool,
    pub color: Option<BannerColor>,
    pub tall: bool,
    pub size: TextSize,
    pub align: Align,
    /// The text's font family (decision 187): the board's own, or one of a few safe ones.
    pub font: BannerFont,
}

/// A banner's font family (decision 187): a short list every system has a face for, so
/// a schedule looks the same on any machine and on paper. `Usual` is the board's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BannerFont {
    #[default]
    Usual,
    Serif,
    Sans,
    Mono,
}

impl BannerFont {
    pub const ALL: [BannerFont; 4] = [
        BannerFont::Usual,
        BannerFont::Serif,
        BannerFont::Sans,
        BannerFont::Mono,
    ];

    /// `usual`, `serif`, `sans`, `mono`: the file's and the page's names.
    pub fn name(self) -> &'static str {
        match self {
            BannerFont::Usual => "usual",
            BannerFont::Serif => "serif",
            BannerFont::Sans => "sans",
            BannerFont::Mono => "mono",
        }
    }

    pub fn from_name(name: &str) -> Option<BannerFont> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    /// For sentences: `serif`, `sans serif`, `monospaced`; empty for the usual.
    pub fn text(self) -> &'static str {
        match self {
            BannerFont::Usual => "",
            BannerFont::Serif => "serif",
            BannerFont::Sans => "sans serif",
            BannerFont::Mono => "monospaced",
        }
    }
}

impl BannerStyle {
    pub fn is_default(&self) -> bool {
        *self == BannerStyle::default()
    }

    /// `bold, red, two lines`, or `plain`: for sentences.
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.bold {
            parts.push("bold".into());
        }
        if let Some(c) = self.color {
            parts.push(c.text());
        }
        if self.tall {
            parts.push("two lines".into());
        }
        match self.size {
            TextSize::Small => parts.push("small text".into()),
            TextSize::Large => parts.push("large text".into()),
            TextSize::Normal => {}
        }
        match self.align {
            Align::Center => parts.push("centered".into()),
            Align::Right => parts.push("on the right".into()),
            Align::Auto | Align::Left => {}
        }
        if self.font != BannerFont::Usual {
            parts.push(self.font.text().into());
        }
        if parts.is_empty() {
            "plain".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// A banner's color: one of nine named colors, which the app draws lighter or darker to
/// suit light and dark, or any color by value (`#rrggbb`), drawn as it is (decision 154).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BannerColor {
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
    Blue,
    Teal,
    Green,
    Gray,
    Rgb(Rgb),
}

impl BannerColor {
    /// The named colors, in the palette's order.
    pub const NAMED: [BannerColor; 9] = [
        BannerColor::Yellow,
        BannerColor::Orange,
        BannerColor::Red,
        BannerColor::Pink,
        BannerColor::Purple,
        BannerColor::Blue,
        BannerColor::Teal,
        BannerColor::Green,
        BannerColor::Gray,
    ];

    /// A named color's name; `None` for a color by value.
    pub fn name(self) -> Option<&'static str> {
        Some(match self {
            BannerColor::Yellow => "yellow",
            BannerColor::Orange => "orange",
            BannerColor::Red => "red",
            BannerColor::Pink => "pink",
            BannerColor::Purple => "purple",
            BannerColor::Blue => "blue",
            BannerColor::Teal => "teal",
            BannerColor::Green => "green",
            BannerColor::Gray => "gray",
            BannerColor::Rgb(_) => return None,
        })
    }

    /// As the file, the page and MCP write it: the name, or `#rrggbb` in lowercase.
    pub fn text(self) -> String {
        match self {
            BannerColor::Rgb(c) => c.hex(),
            named => named.name().unwrap_or_default().to_string(),
        }
    }

    /// A name, or `#rrggbb` in lowercase only, so a file writes back byte for byte.
    pub fn from_text(text: &str) -> Option<BannerColor> {
        if text.starts_with('#') {
            if text.bytes().any(|b| b.is_ascii_uppercase()) {
                return None;
            }
            return Rgb::parse(text).map(BannerColor::Rgb);
        }
        BannerColor::NAMED
            .into_iter()
            .find(|c| c.name() == Some(text))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StripKind {
    Sheet(SheetId),
    Banner(Banner),
    DayBreak,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strip {
    pub id: StripId,
    pub kind: StripKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubBoard {
    pub id: SubBoardId,
    pub name: String,
    pub start: Option<Date>,
    pub calendar: Calendar,
    pub strips: Vec<Strip>,
}

/// One arrangement of every sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    /// What its scene strips show (decision 39).
    pub strip_layout: StripLayout,
    /// What its day breaks show and whether a shoot day strip heads each day (decisions 67, 148).
    pub day_look: DayLook,
    /// The named layouts it picked instead of its own (decision 78).
    pub uses: crate::layouts::LayoutUse,
    pub sub_boards: Vec<SubBoard>,
    /// Unscheduled strips. Has no start date.
    pub boneyard: Vec<Strip>,
}

impl Board {
    /// Every scene strip in board order (each sub-board's strips in turn, then the
    /// Boneyard), with its sheet and whether it is that sheet's first strip: a later one
    /// is a part of a scene spanning days (decision 76).
    pub fn sheet_parts(&self) -> impl Iterator<Item = (&Strip, SheetId, bool)> + '_ {
        let mut seen = IdSet::default();
        self.sub_boards
            .iter()
            .flat_map(|sb| &sb.strips)
            .chain(&self.boneyard)
            .filter_map(move |st| match st.kind {
                StripKind::Sheet(id) => Some((st, id, seen.insert(id))),
                _ => None,
            })
    }
}

/// The sheets by id, built once where many lookups follow ([`Schedule::sheet_index`]).
pub type SheetIndex<'a> = IdMap<SheetId, &'a Sheet>;
/// The elements by id ([`Schedule::element_index`]).
pub type ElementIndex<'a> = IdMap<ElementId, &'a Element>;

/// Where strips go: a sub-board, or the Boneyard when `sub_board` is `None`, at `index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub sub_board: Option<SubBoardId>,
    pub index: usize,
}

/// A header field a sheet merge changed on the survivor, as text before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinedField {
    /// The field's name as the file writes it: `scenes`, `pages`, `set`...
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

/// A value a sheet merge left out: another sheet's value for a one-value field the
/// survivor kept (or filled from an earlier sheet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedValue {
    /// The merged-away sheet it came from.
    pub sheet: SheetId,
    /// The field's name as the file writes it.
    pub field: &'static str,
    pub value: String,
}

/// What [`Schedule::merge_sheets`] did (decision 35).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetMerge {
    pub survivor: SheetId,
    /// The sheets merged in and deleted, in the order given.
    pub merged: Vec<SheetId>,
    /// The survivor's fields that changed, in the file's field order.
    pub combined: Vec<CombinedField>,
    /// Elements the survivor gained, in the order added.
    pub elements_added: Vec<ElementId>,
    /// Values that didn't survive, by field in the file's order, then the order given.
    pub dropped: Vec<DroppedValue>,
}

/// `h:mm` for minutes after midnight or a shoot time.
fn hm_text(m: u32) -> String {
    format!("{}:{:02}", m / 60, m % 60)
}

/// For [`Schedule::merge_sheets`]: a one-value field's survivor (`before`'s value, else
/// the first of `sheets`, in the order given, that has one); every other value among `rest` is
/// pushed to `dropped`.
fn keep_one<T: PartialEq>(
    field: &'static str,
    before: &Sheet,
    sheets: &[&Sheet],
    rest: &[&Sheet],
    get: impl Fn(&Sheet) -> Option<T>,
    show: impl Fn(&T) -> String,
    dropped: &mut Vec<DroppedValue>,
) -> Option<T> {
    let kept = get(before).or_else(|| sheets.iter().find_map(|h| get(h)));
    for h in rest {
        if let Some(v) = get(h) {
            if Some(&v) != kept.as_ref() {
                dropped.push(DroppedValue {
                    sheet: h.id,
                    field,
                    value: show(&v),
                });
            }
        }
    }
    kept
}

/// `m:ss` for a screen time.
fn ms_text(s: u32) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

/// A header field strips can be sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Scene,
    IntExt,
    Set,
    TimeOfDay,
    Location,
    Pages,
    ScriptDay,
    Unit,
    Sequence,
    Episode,
}

impl SortField {
    pub const ALL: [SortField; 10] = [
        SortField::Scene,
        SortField::IntExt,
        SortField::Set,
        SortField::TimeOfDay,
        SortField::Location,
        SortField::Pages,
        SortField::ScriptDay,
        SortField::Unit,
        SortField::Sequence,
        SortField::Episode,
    ];

    /// The field's name as the file and the app's commands write it.
    pub fn name(self) -> &'static str {
        match self {
            SortField::Scene => "scene",
            SortField::IntExt => "int_ext",
            SortField::Set => "set",
            SortField::TimeOfDay => "time_of_day",
            SortField::Location => "location",
            SortField::Pages => "pages",
            SortField::ScriptDay => "script_day",
            SortField::Unit => "unit",
            SortField::Sequence => "sequence",
            SortField::Episode => "episode",
        }
    }

    pub fn from_name(name: &str) -> Option<SortField> {
        SortField::ALL.into_iter().find(|f| f.name() == name)
    }

    fn text(self, h: &Sheet) -> &str {
        match self {
            SortField::Scene => &h.scenes,
            SortField::IntExt => &h.int_ext,
            SortField::Set => &h.set,
            SortField::TimeOfDay => &h.time_of_day,
            SortField::Location => &h.location,
            SortField::ScriptDay => &h.script_day,
            SortField::Unit => &h.unit,
            SortField::Sequence => &h.sequence,
            SortField::Episode => &h.episode,
            SortField::Pages => "",
        }
    }
}

/// Sort keys at most, as in the reference app (behavior note 24: four rules; decision 122).
pub const MAX_SORT_KEYS: usize = 4;

/// One sort key: a field, ascending unless `descending`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortKey {
    pub field: SortField,
    pub descending: bool,
}

impl SortKey {
    /// How two sheets compare on this key. Empty text comes last either way; a page
    /// length of 0 is a length like any other.
    pub fn compare(&self, a: &Sheet, b: &Sheet) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let o = if self.field == SortField::Pages {
            a.pages.cmp(&b.pages)
        } else {
            let (x, y) = (self.field.text(a).trim(), self.field.text(b).trim());
            match (x.is_empty(), y.is_empty()) {
                (true, true) => return Ordering::Equal,
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                _ => natural_cmp(x, y),
            }
        };
        if self.descending {
            o.reverse()
        } else {
            o
        }
    }
}

/// Compares text the way people number scenes: runs of digits by their value, other runs
/// ignoring case, a number before text at the same place, and a shorter string first
/// when one runs out. So `2 < 9 < 10 < 10A < 10B < 11 < A12`. Engine check E-S4.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    fn runs(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut digit = None;
        for (i, c) in s.char_indices() {
            let d = c.is_ascii_digit();
            if digit.is_some_and(|x| x != d) {
                out.push((digit.unwrap(), &s[start..i]));
                start = i;
            }
            digit = Some(d);
        }
        if let Some(d) = digit {
            out.push((d, &s[start..]));
        }
        out
    }
    let (ra, rb) = (runs(a), runs(b));
    for ((da, x), (db, y)) in ra.iter().zip(&rb) {
        let o = match (da, db) {
            (true, true) => {
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                x.len().cmp(&y.len()).then_with(|| x.cmp(y))
            }
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => x.to_lowercase().cmp(&y.to_lowercase()),
        };
        if o.is_ne() {
            return o;
        }
    }
    ra.len().cmp(&rb.len())
}

/// A field a strip can show (the strip layout, decision 39): each sheet field the board
/// knows, plus the cast's board IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StripField {
    Scene,
    IntExt,
    Set,
    TimeOfDay,
    Pages,
    /// The board IDs (or names, where an element has none) of the cast on the sheet.
    Cast,
    Location,
    ScriptDay,
    Unit,
    Sequence,
    Episode,
    ShootTime,
    Synopsis,
    /// How many cast members the sheet has (decision 66).
    CastCount,
    /// How many elements the sheet has, every category on the board (decision 66).
    ElementCount,
    /// The sheet's elements other than the cast, every category on the board, by board
    /// ID or name (the calendar strip's default, behavior note 19; decision 102).
    Elements,
    /// The sheet's elements of the column's one category, by name or board ID (the
    /// column's switch), as `LAMP, ROPE` (decision 194).
    CategoryElements,
    /// An empty cell, to hold a place (the strip designer, decision 87).
    Spacer,
    /// The column's own text, the same on every strip (decision 87).
    Text,
}

impl StripField {
    /// Every field, in the order a strip layout dialog offers them.
    pub const ALL: [StripField; 19] = [
        StripField::Scene,
        StripField::IntExt,
        StripField::Set,
        StripField::TimeOfDay,
        StripField::Pages,
        StripField::Cast,
        StripField::Location,
        StripField::ScriptDay,
        StripField::Unit,
        StripField::Sequence,
        StripField::Episode,
        StripField::ShootTime,
        StripField::Synopsis,
        StripField::CastCount,
        StripField::ElementCount,
        StripField::Elements,
        StripField::CategoryElements,
        StripField::Spacer,
        StripField::Text,
    ];

    /// The fields that show something of the sheet: every field but the spacer, custom
    /// text and a category's elements (which needs a layout's category pick, decision
    /// 194). Reports and the calendar pick their columns from these.
    pub const SHEET: [StripField; 16] = [
        StripField::Scene,
        StripField::IntExt,
        StripField::Set,
        StripField::TimeOfDay,
        StripField::Pages,
        StripField::Cast,
        StripField::Location,
        StripField::ScriptDay,
        StripField::Unit,
        StripField::Sequence,
        StripField::Episode,
        StripField::ShootTime,
        StripField::Synopsis,
        StripField::CastCount,
        StripField::ElementCount,
        StripField::Elements,
    ];

    /// The name the file and the app use.
    pub fn name(self) -> &'static str {
        match self {
            StripField::Scene => "scene",
            StripField::IntExt => "int_ext",
            StripField::Set => "set",
            StripField::TimeOfDay => "time_of_day",
            StripField::Pages => "pages",
            StripField::Cast => "cast",
            StripField::Location => "location",
            StripField::ScriptDay => "script_day",
            StripField::Unit => "unit",
            StripField::Sequence => "sequence",
            StripField::Episode => "episode",
            StripField::ShootTime => "shoot_time",
            StripField::Synopsis => "synopsis",
            StripField::CastCount => "cast_count",
            StripField::ElementCount => "element_count",
            StripField::Elements => "elements",
            StripField::CategoryElements => "category_elements",
            StripField::Spacer => "spacer",
            StripField::Text => "text",
        }
    }

    pub fn from_name(name: &str) -> Option<StripField> {
        StripField::ALL.into_iter().find(|f| f.name() == name)
    }

    /// A short label for column headings and sentences.
    pub fn label(self) -> &'static str {
        match self {
            StripField::Scene => "Scene",
            StripField::IntExt => "I/E",
            StripField::Set => "Set",
            StripField::TimeOfDay => "Time of day",
            StripField::Pages => "Pages",
            StripField::Cast => "Cast IDs",
            StripField::Location => "Location",
            StripField::ScriptDay => "Script day",
            StripField::Unit => "Unit",
            StripField::Sequence => "Sequence",
            StripField::Episode => "Episode",
            StripField::ShootTime => "Shoot time",
            StripField::Synopsis => "Synopsis",
            StripField::CastCount => "Cast count",
            StripField::ElementCount => "Element count",
            StripField::Elements => "Elements",
            StripField::CategoryElements => "Category elements",
            StripField::Spacer => "Spacer",
            StripField::Text => "Text",
        }
    }

    /// Whether a layout may show it more than once (the spacer and custom text).
    pub fn repeats(self) -> bool {
        matches!(self, StripField::Spacer | StripField::Text)
    }

    /// Whether a report's own columns or the calendar can show it: something of the sheet
    /// (not the spacer or custom text) that needs no category pick (not a category's
    /// elements, decision 194).
    pub fn of_sheet(self) -> bool {
        !self.repeats() && self != StripField::CategoryElements
    }

    /// Whether a strip layout's column of this field may pick a category: the element
    /// count counts it (decision 147), a category's elements lists it (decision 194).
    pub fn picks_category(self) -> bool {
        matches!(
            self,
            StripField::ElementCount | StripField::CategoryElements
        )
    }

    /// The width a field gets when a layout doesn't say.
    pub fn default_width(self) -> StripWidth {
        match self {
            StripField::Scene
            | StripField::IntExt
            | StripField::Pages
            | StripField::ScriptDay
            | StripField::Unit
            | StripField::Sequence
            | StripField::Episode
            | StripField::ShootTime
            | StripField::CastCount
            | StripField::ElementCount
            | StripField::Spacer => StripWidth::Narrow,
            StripField::TimeOfDay | StripField::Text => StripWidth::Medium,
            StripField::Set
            | StripField::Cast
            | StripField::Location
            | StripField::Elements
            | StripField::CategoryElements => StripWidth::Wide,
            StripField::Synopsis => StripWidth::Widest,
        }
    }
}

/// A strip column's width, in a few fixed steps. The app draws them; the engine only
/// keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StripWidth {
    Narrow,
    Medium,
    Wide,
    Widest,
}

impl StripWidth {
    pub const ALL: [StripWidth; 4] = [
        StripWidth::Narrow,
        StripWidth::Medium,
        StripWidth::Wide,
        StripWidth::Widest,
    ];

    pub fn name(self) -> &'static str {
        match self {
            StripWidth::Narrow => "narrow",
            StripWidth::Medium => "medium",
            StripWidth::Wide => "wide",
            StripWidth::Widest => "widest",
        }
    }

    pub fn from_name(name: &str) -> Option<StripWidth> {
        StripWidth::ALL.into_iter().find(|w| w.name() == name)
    }
}

/// How a cell's text sits in it. `Auto` is the field's usual way: numbers to the right,
/// everything else to the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Align {
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

impl Align {
    pub const ALL: [Align; 4] = [Align::Auto, Align::Left, Align::Center, Align::Right];

    pub fn name(self) -> &'static str {
        match self {
            Align::Auto => "auto",
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }

    pub fn from_name(name: &str) -> Option<Align> {
        Align::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// A text size step (the report options' sizes), used by strip layouts and banners too
/// (decision 87): a few fixed steps, so strips keep their fixed line height.
pub use crate::setup::TextSize;

/// The most lines a strip has (decision 87).
pub const MAX_STRIP_LINES: u8 = 4;

/// The longest custom text a strip column holds, in characters.
pub const MAX_COLUMN_TEXT: usize = 100;

/// One field of a strip layout: where it sits, how wide, and how its text looks.
///
/// The first line's fields are the strip's columns, one each, `width` wide. A field on a
/// later line sits under them, left to right after the one before it on that line,
/// covering `span` of those columns (a merged cell); its own width is kept but not used
/// (decision 87).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StripColumn {
    pub field: StripField,
    pub width: StripWidth,
    /// Which line of the strip: 0 for the first, up to [`MAX_STRIP_LINES`] - 1 (decisions
    /// 78 and 87).
    pub line: u8,
    /// How many of the first line's columns it covers: 1 on the first line.
    pub span: u8,
    pub align: Align,
    pub bold: bool,
    pub italic: bool,
    /// A custom text field's text (empty for every other field).
    pub text: String,
    /// The category an element count counts (the strip designer's pick, BRD-18, behavior
    /// note 27, decision 147), shown as `Props (2)`; none: every element, as a bare
    /// number. Only on [`StripField::ElementCount`], and on
    /// [`StripField::CategoryElements`], the category it lists (none: an empty cell;
    /// decision 194).
    pub category: Option<CategoryId>,
    /// A category's elements by board ID (an element without one by name) rather than by
    /// name (decision 194). Only on [`StripField::CategoryElements`].
    pub board_ids: bool,
    /// Line styles (decision 185): a line down the cell's right edge, between it and the
    /// next column, and a rule along its foot, under its line.
    pub rule_after: bool,
    pub rule_under: bool,
}

impl StripColumn {
    /// `field` at its default width, on the first line, plain.
    pub fn of(field: StripField) -> StripColumn {
        StripColumn {
            field,
            width: field.default_width(),
            line: 0,
            span: 1,
            align: Align::Auto,
            bold: false,
            italic: false,
            text: String::new(),
            category: None,
            board_ids: false,
            rule_after: false,
            rule_under: false,
        }
    }

    /// `field` on line `line` (0 for the first), covering `span` columns.
    pub fn on_line(field: StripField, line: u8, span: u8) -> StripColumn {
        StripColumn {
            line,
            span,
            ..StripColumn::of(field)
        }
    }

    /// `wide, line 2, spans 3, centered, bold`: what isn't the column's default, in words.
    /// The category an element count counts is named from `s` (decision 162).
    fn look_text(&self, s: &Schedule) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.width != self.field.default_width() {
            parts.push(self.width.name().to_string());
        }
        if self.line > 0 {
            parts.push(format!("line {}", self.line + 1));
        }
        if self.span > 1 {
            parts.push(format!("spans {}", self.span));
        }
        if self.align != Align::Auto {
            parts.push(self.align.name().to_string());
        }
        if self.bold {
            parts.push("bold".to_string());
        }
        if self.italic {
            parts.push("italic".to_string());
        }
        if self.rule_after {
            parts.push("line after".to_string());
        }
        if self.rule_under {
            parts.push("rule under".to_string());
        }
        let verb = if self.field == StripField::CategoryElements {
            "listing"
        } else {
            "counting"
        };
        if let Some(c) = self.category {
            parts.push(match s.category(c) {
                Some(cat) => format!("{verb} {}", cat.name),
                None => format!("{verb} a deleted category"),
            });
        }
        if self.board_ids {
            parts.push("by board ID".to_string());
        }
        parts.join(", ")
    }
}

/// Which fields a board's scene strips show, where and how (the strip designer,
/// decisions 39, 78 and 87): up to four lines, the first line's fields making the
/// columns, later lines' fields covering one or more of them. The board, the one-line
/// report and the printed board use it. At least one field on the first line; each sheet
/// field once (spacers and custom text may repeat).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StripLayout {
    pub columns: Vec<StripColumn>,
    /// The strips' text size step.
    pub size: TextSize,
    /// The strip designer's switches for the marks on strips (BRD-14, behavior note 25,
    /// decision 136): off hides the red flag or the conflict mark on this layout's strips.
    /// Both show by default.
    pub hide_red_flags: bool,
    pub hide_conflicts: bool,
}

impl Default for StripLayout {
    /// What the board has always shown: scene, I/E, set, time of day, pages and cast IDs.
    fn default() -> StripLayout {
        StripLayout::of(
            [
                StripField::Scene,
                StripField::IntExt,
                StripField::Set,
                StripField::TimeOfDay,
                StripField::Pages,
                StripField::Cast,
            ]
            .into_iter()
            .map(StripColumn::of)
            .collect(),
        )
    }
}

impl StripLayout {
    /// A layout of `columns` at the usual text size.
    pub fn of(columns: Vec<StripColumn>) -> StripLayout {
        StripLayout {
            columns,
            size: TextSize::Normal,
            hide_red_flags: false,
            hide_conflicts: false,
        }
    }

    pub fn is_default(&self) -> bool {
        *self == StripLayout::default()
    }

    /// Whether the columns are the default ones (the text size aside).
    pub fn default_columns(&self) -> bool {
        self.columns == StripLayout::default().columns
    }

    /// The categories its element counts count, every column's.
    pub fn counted(&self) -> impl Iterator<Item = CategoryId> + '_ {
        self.columns.iter().filter_map(|c| c.category)
    }

    /// Drops the pick of a category that is no longer in `keep`: that element count
    /// counts every element again (decision 147).
    pub fn tidy_counts(&mut self, keep: &HashSet<CategoryId>) {
        for c in &mut self.columns {
            if c.category.is_some_and(|x| !keep.contains(&x)) {
                c.category = None;
            }
        }
    }

    /// How many lines a strip takes: one past the last line any field is on.
    pub fn lines(&self) -> u8 {
        self.columns.iter().map(|c| c.line + 1).max().unwrap_or(1)
    }

    /// Why the layout can't be used: no field on the first line, a sheet field twice, a
    /// line past the fourth, a span on the first line, a later line wider than the
    /// first, or a custom text too long, on two lines or on another field.
    pub fn problem(&self) -> Option<String> {
        if self.columns.is_empty() {
            return Some("a strip layout needs at least one field".to_string());
        }
        let first = self.columns.iter().filter(|c| c.line == 0).count();
        if first == 0 {
            return Some("a strip layout needs a field on its first line".to_string());
        }
        for (i, c) in self.columns.iter().enumerate() {
            if !c.field.repeats() && self.columns[..i].iter().any(|o| o.field == c.field) {
                return Some(format!("the strip layout shows {} twice", c.field.label()));
            }
            if c.line >= MAX_STRIP_LINES {
                return Some(format!("a strip has at most {MAX_STRIP_LINES} lines"));
            }
            if c.span == 0 {
                return Some(format!("{} covers at least one column", c.field.label()));
            }
            if c.line == 0 && c.span != 1 {
                return Some(format!(
                    "{} on the first line is one column wide",
                    c.field.label()
                ));
            }
            if c.field != StripField::Text && !c.text.is_empty() {
                return Some(format!("{} has no text of its own", c.field.label()));
            }
            if !c.field.picks_category() && c.category.is_some() {
                return Some(format!("{} counts no category", c.field.label()));
            }
            if c.field != StripField::CategoryElements && c.board_ids {
                return Some(format!("{} has no board ID switch", c.field.label()));
            }
            if c.text.chars().count() > MAX_COLUMN_TEXT {
                return Some(format!(
                    "a strip's custom text is at most {MAX_COLUMN_TEXT} characters"
                ));
            }
            if c.text.contains(['\n', '\r']) {
                return Some("a strip's custom text is one line".to_string());
            }
        }
        for line in 1..MAX_STRIP_LINES {
            let wide: usize = self
                .columns
                .iter()
                .filter(|c| c.line == line)
                .map(|c| usize::from(c.span))
                .sum();
            if wide > first {
                return Some(format!(
                    "line {} covers {wide} columns but the first line has {first}",
                    line + 1
                ));
            }
        }
        None
    }

    /// `Scene, I/E, Set (wide), Synopsis (line 2, spans 3, italic), ...`: each field, with
    /// what isn't its default. For sentences (What Changed, merges); categories are named
    /// from `s`.
    pub fn text(&self, s: &Schedule) -> String {
        let mut out = self
            .columns
            .iter()
            .map(|c| {
                let name = if c.field == StripField::Text {
                    format!("Text {:?}", c.text)
                } else {
                    c.field.label().to_string()
                };
                let look = c.look_text(s);
                if look.is_empty() {
                    name
                } else {
                    format!("{name} ({look})")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        if self.size != TextSize::Normal {
            out.push_str(&format!("; {} text", self.size.name()));
        }
        if self.hide_red_flags {
            out.push_str("; no red flag marks");
        }
        if self.hide_conflicts {
            out.push_str("; no conflict marks");
        }
        out
    }
}

/// What a board's day breaks show besides the day's number and totals, and whether a
/// shoot day strip (the day's number of the sub-board's days and its full date) heads
/// each day (inventory section 6, decisions 67 and 148).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DayLook {
    pub shoot_day_strip: bool,
    /// The day's screen time.
    pub screen_time: bool,
    /// The earliest sunrise and the latest sunset of the day's sheets.
    pub sunrise: bool,
    pub sunset: bool,
    /// The day's element quantities: each category that uses daily quantities, its total
    /// for the day (decisions 63 and 87).
    pub quantity: bool,
    /// Text every day break of the board shows; empty for none.
    pub text: String,
}

/// The longest day break text, in characters.
pub const MAX_DAY_TEXT: usize = 200;

impl DayLook {
    pub fn is_default(&self) -> bool {
        *self == DayLook::default()
    }

    pub fn problem(&self) -> Option<String> {
        if self.text.chars().count() > MAX_DAY_TEXT {
            return Some(format!(
                "a day break's text is at most {MAX_DAY_TEXT} characters"
            ));
        }
        if self.text.contains(['\n', '\r']) {
            return Some("a day break's text is one line".to_string());
        }
        None
    }

    /// `shoot day strip, screen time, sunrise, sunset, "text"` or `totals only`: for
    /// sentences.
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (on, what) in [
            (self.shoot_day_strip, "shoot day strip"),
            (self.screen_time, "screen time"),
            (self.sunrise, "sunrise"),
            (self.sunset, "sunset"),
            (self.quantity, "quantities"),
        ] {
            if on {
                parts.push(what.to_string());
            }
        }
        if !self.text.is_empty() {
            parts.push(format!("{:?}", self.text));
        }
        if parts.is_empty() {
            "totals only".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// A color, `0xRRGGBB` (decision 60). The file writes it as `#rrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb(pub u32);

impl Rgb {
    pub const MAX: u32 = 0x00FF_FFFF;

    /// `#rrggbb`, lowercase.
    pub fn hex(self) -> String {
        format!("#{:06x}", self.0 & Rgb::MAX)
    }

    /// `#rrggbb` (either case); anything else is `None`.
    pub fn parse(text: &str) -> Option<Rgb> {
        let h = text.strip_prefix('#')?;
        if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(h, 16).ok().map(Rgb)
    }

    /// Black or white, whichever reads better on this color: the integer luma with
    /// weights 299, 587 and 114 thousandths, black from half way up.
    pub fn ink(self) -> Rgb {
        let (r, g, b) = ((self.0 >> 16) & 255, (self.0 >> 8) & 255, self.0 & 255);
        if 299 * r + 587 * g + 114 * b >= 128 * 1000 {
            Rgb(0)
        } else {
            Rgb(Rgb::MAX)
        }
    }
}

/// One strip color: scene strips whose I/E and time of day are these values (compared
/// trimmed, ignoring case; an empty value matches any) are drawn in `color`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StripColorRule {
    pub int_ext: String,
    pub time_of_day: String,
    pub color: Rgb,
    /// The pair's own text color (behavior note 22, decision 108): when set, it wins over
    /// the scene text color; none follows that, else black or white.
    pub text: Option<Rgb>,
}

impl StripColorRule {
    pub fn matches(&self, sheet: &Sheet) -> bool {
        let fits = |rule: &str, value: &str| rule.trim().is_empty() || same_name(rule, value);
        fits(&self.int_ext, &sheet.int_ext) && fits(&self.time_of_day, &sheet.time_of_day)
    }

    /// `EXT / NIGHT`, with `any` for an empty value.
    pub fn label(&self) -> String {
        let v = |x: &str| {
            if x.trim().is_empty() {
                "any".to_string()
            } else {
                x.trim().to_string()
            }
        };
        format!("{} / {}", v(&self.int_ext), v(&self.time_of_day))
    }
}

/// The colors the user set for strips (inventory section 5, decision 60): scene strip
/// colors by I/E and time of day, the first matching rule winning (none matching: the
/// usual convention), and a text color per strip type (none: black or white, whichever
/// reads on the strip). Kept with the schedule, so every board shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct StripColors {
    pub rules: Vec<StripColorRule>,
    pub scene_text: Option<Rgb>,
    pub banner_text: Option<Rgb>,
    pub break_text: Option<Rgb>,
}

/// The most strip color rules a schedule keeps.
pub const MAX_COLOR_RULES: usize = 200;

impl StripColors {
    pub fn is_default(&self) -> bool {
        *self == StripColors::default()
    }

    /// The color the first matching rule gives a sheet's strip.
    pub fn color_of(&self, sheet: &Sheet) -> Option<Rgb> {
        self.rule_for(sheet).map(|r| r.color)
    }

    /// The first rule that matches a sheet's strip.
    pub fn rule_for(&self, sheet: &Sheet) -> Option<&StripColorRule> {
        self.rules.iter().find(|r| r.matches(sheet))
    }

    /// The text color the user set for a sheet's strip: the matching pair's own, else the
    /// scene text color; `None` when neither is set (black or white, whichever reads).
    pub fn text_of(&self, sheet: &Sheet) -> Option<Rgb> {
        self.rule_for(sheet)
            .and_then(|r| r.text)
            .or(self.scene_text)
    }

    /// Why these colors can't be used: too many rules, a color out of range, a value too
    /// long, or two rules for the same values.
    pub fn problem(&self) -> Option<String> {
        if self.rules.len() > MAX_COLOR_RULES {
            return Some(format!("at most {MAX_COLOR_RULES} strip colors"));
        }
        let all = self
            .rules
            .iter()
            .flat_map(|r| [Some(r.color), r.text])
            .flatten()
            .chain(self.scene_text)
            .chain(self.banner_text)
            .chain(self.break_text);
        for c in all {
            if c.0 > Rgb::MAX {
                return Some(format!("color {} is out of range", c.0));
            }
        }
        for (i, r) in self.rules.iter().enumerate() {
            if r.int_ext.chars().count() > 200 || r.time_of_day.chars().count() > 200 {
                return Some("a strip color's value is at most 200 characters".to_string());
            }
            if self.rules[..i].iter().any(|o| {
                same_name(&o.int_ext, &r.int_ext) && same_name(&o.time_of_day, &r.time_of_day)
            }) {
                return Some(format!("two strip colors for {}", r.label()));
            }
        }
        None
    }

    /// `EXT / NIGHT #1f3524, ...; scene text #000000`: for sentences (What Changed,
    /// merges).
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = self
            .rules
            .iter()
            .map(|r| match r.text {
                Some(t) => format!("{} {} text {}", r.label(), r.color.hex(), t.hex()),
                None => format!("{} {}", r.label(), r.color.hex()),
            })
            .collect();
        for (what, c) in [
            ("scene text", self.scene_text),
            ("banner text", self.banner_text),
            ("day break text", self.break_text),
        ] {
            if let Some(c) = c {
                parts.push(format!("{what} {}", c.hex()));
            }
        }
        if parts.is_empty() {
            "the usual colors".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NoSuchCategory(CategoryId),
    NoSuchElement(ElementId),
    NoSuchSheet(SheetId),
    NoSuchBoard(BoardId),
    NoSuchSubBoard(SubBoardId),
    NoSuchStrip(StripId),
    NoSuchRedFlagType(RedFlagTypeId),
    NoSuchRedFlag(RedFlagId),
    DuplicateName(String),
    DuplicateBoardId(String),
    DuplicateCategory(String),
    /// A name that is empty once trimmed.
    EmptyName,
    /// A built-in category can't be renamed, moved or deleted.
    BuiltIn(String),
    /// A category that still holds elements can't be deleted.
    NotEmpty(String),
    /// An element linked to others can't be deleted without unlinking it first.
    Linked(String),
    /// A red flag type still used by red flags can't be deleted.
    InUse(String),
    /// A built-in red flag type can be renamed and recolored but not deleted.
    BuiltInType(String),
    /// What `check` found wrong.
    Broken(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSuchCategory(id) => write!(f, "no category {}", id.0),
            Error::NoSuchElement(id) => write!(f, "no element {}", id.0),
            Error::NoSuchSheet(id) => write!(f, "no sheet {}", id.0),
            Error::NoSuchBoard(id) => write!(f, "no board {}", id.0),
            Error::NoSuchSubBoard(id) => write!(f, "no sub-board {}", id.0),
            Error::NoSuchStrip(id) => write!(f, "no strip {}", id.0),
            Error::NoSuchRedFlagType(id) => write!(f, "no red flag type {}", id.0),
            Error::NoSuchRedFlag(id) => write!(f, "no red flag {}", id.0),
            Error::DuplicateName(n) => write!(f, "{n:?} is already in this category"),
            Error::DuplicateBoardId(n) => {
                write!(f, "board ID {n:?} is already used in this category")
            }
            Error::DuplicateCategory(n) => write!(f, "there is already a category {n:?}"),
            Error::EmptyName => f.write_str("a name can't be empty"),
            Error::BuiltIn(n) => write!(f, "{n} is a built-in category"),
            Error::NotEmpty(n) => write!(f, "{n} still has elements"),
            Error::Linked(n) => write!(f, "{n:?} is linked to other elements"),
            Error::InUse(n) => write!(f, "red flag type {n:?} is still in use"),
            Error::BuiltInType(n) => write!(f, "red flag type {n:?} is built in"),
            Error::Broken(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub title: String,
    pub categories: Vec<Category>,
    pub elements: Vec<Element>,
    pub sheets: Vec<Sheet>,
    pub boards: Vec<Board>,
    /// The red flag types, in display order.
    pub red_flag_types: Vec<RedFlagType>,
    /// The production info report headers show (crate::setup).
    pub production: crate::setup::Production,
    /// The report list with each report's options, empty while it is the default
    /// (`Schedule::report_list`).
    pub reports: Vec<crate::setup::ReportSetup>,
    /// The day out of days code letters.
    pub dood_codes: crate::setup::DoodCodes,
    /// The report header layouts, empty while they are the three the list starts with
    /// (`Schedule::header_list`, decision 99).
    pub header_layouts: Vec<crate::headers::HeaderDesign>,
    /// Strip colors and text colors the user set (decision 60).
    pub strip_colors: StripColors,
    /// The named layouts boards and the one-line report pick from (decision 78).
    pub layouts: crate::layouts::Library,
    /// Planning data for the features beyond the reference app (crate::plan).
    pub plan: crate::plan::Plan,
    /// The order the Breakdown tab lays the sheet's header fields out in, as keys of
    /// [`SHEET_FIELDS`]; empty while it is the default. Fields left out follow in the
    /// default order ([`Schedule::sheet_field_order`]; decision 77).
    pub sheet_fields: Vec<&'static str>,
    /// Which header fields the Breakdown tab hides, and the widths set (the breakdown
    /// designer, decision 101); empty while nothing is set.
    pub sheet_looks: SheetLooks,
    /// What a strip shows on a Calendar date, in order (the calendar strip designer, part
    /// of the schedule per behavior note 19; decision 102); empty while it is
    /// [`DEFAULT_CALENDAR_FIELDS`].
    pub calendar_fields: Vec<StripField>,
    /// What the Calendar hides (its View menu, behavior note 22; decision 111), in
    /// [`CalendarShow::ALL`] order; empty while it shows everything.
    pub calendar_hidden: Vec<CalendarShow>,
    /// The next id to hand out. Ids are never reused, across all kinds.
    pub next_id: u32,
    /// What the file held under `extensions` keys, written back untouched (decision 224).
    pub extensions: crate::extensions::Extensions,
}

/// Names compare without regard to case. Engine check: confirm in the real app.
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// `head` then `tail`, `head` cut so the whole is at most `limit` characters.
pub(crate) fn fit_name(head: &str, tail: &str, limit: usize) -> String {
    let keep = limit.saturating_sub(tail.chars().count());
    let head: String = head.chars().take(keep).collect();
    format!("{}{tail}", head.trim_end())
}

impl Schedule {
    /// An empty schedule with one board, one sub-board and a Cast category. Cast does not
    /// number new elements automatically, matching what a script import does (D10).
    pub fn new(title: &str) -> Schedule {
        let mut s = Schedule {
            title: title.to_string(),
            categories: Vec::new(),
            elements: Vec::new(),
            sheets: Vec::new(),
            boards: Vec::new(),
            red_flag_types: Vec::new(),
            production: Default::default(),
            reports: Vec::new(),
            dood_codes: Default::default(),
            header_layouts: Vec::new(),
            strip_colors: StripColors::default(),
            layouts: Default::default(),
            plan: Default::default(),
            sheet_fields: Vec::new(),
            sheet_looks: SheetLooks::default(),
            calendar_fields: Vec::new(),
            calendar_hidden: Vec::new(),
            next_id: 1,
            extensions: Default::default(),
        };
        s.cast_or_add();
        s.add_board("Board 1", None);
        s
    }

    /// A new schedule as the app, the command line and MCP make one: [`Schedule::new`]
    /// with our default categories after the cast ([`DEFAULT_CATEGORIES`]).
    pub fn with_default_categories(title: &str) -> Schedule {
        let mut s = Schedule::new(title);
        for (name, auto, daily) in DEFAULT_CATEGORIES {
            let id = s.add_category(name);
            let c = s.category_mut(id).expect("just added");
            c.auto_board_id = *auto;
            c.daily_quantity = *daily;
        }
        // The built-in red flag types (behavior note 23; decision 113).
        s.standard_red_flag_types();
        // The cast's starter properties (behavior note 24; decision 119).
        let cast = s.cast().expect("a new schedule has the cast");
        for (name, multi) in CAST_PROPERTIES {
            s.add_property(cast, name).expect("distinct names");
            if *multi {
                let at = s.category(cast).unwrap().properties.len() - 1;
                s.set_property_multi_line(cast, at, true)
                    .expect("just added");
            }
        }
        s
    }

    /// Whether an element is left off the board: it shows on no strip and never conflicts
    /// (decisions 28, 62 and 105; engine check E-E3). Its own setting decides; the
    /// category's is the default a new element gets (behavior note 24; decision 120).
    pub fn excluded(&self, element: ElementId) -> bool {
        self.element(element)
            .is_some_and(|e| self.element_excluded(e))
    }

    /// [`Schedule::excluded`] for an element in hand.
    pub fn element_excluded(&self, e: &Element) -> bool {
        e.exclude_from_board
    }

    /// A new id. Saturates rather than wrapping: `check` refuses `next_id` past
    /// [`MAX_NEXT_ID`], far below where it would stop.
    pub(crate) fn fresh(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    // ---- categories and elements

    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }

    pub fn category_mut(&mut self, id: CategoryId) -> Option<&mut Category> {
        self.categories.iter_mut().find(|c| c.id == id)
    }

    pub fn category_named(&self, name: &str) -> Option<CategoryId> {
        self.categories
            .iter()
            .find(|c| same_name(&c.name, name))
            .map(|c| c.id)
    }

    /// Adds a category, or returns the one already called `name`.
    pub fn add_category(&mut self, name: &str) -> CategoryId {
        if let Some(id) = self.category_named(name) {
            return id;
        }
        let id = CategoryId(self.fresh());
        self.categories.push(Category {
            id,
            name: name.trim().to_string(),
            auto_board_id: false,
            exclude_from_board: false,
            defaults: DoodRules::default(),
            cast: false,
            daily_quantity: false,
            properties: Vec::new(),
            multi_line: Vec::new(),
        });
        id
    }

    /// The built-in cast category, whatever it is called now.
    pub fn cast(&self) -> Option<CategoryId> {
        self.categories.iter().find(|c| c.cast).map(|c| c.id)
    }

    /// The cast category, made when the schedule has none: a category named [`CAST_NAME`]
    /// becomes it, else a new one goes first.
    pub fn cast_or_add(&mut self) -> CategoryId {
        if let Some(id) = self.cast() {
            return id;
        }
        if let Some(id) = self.category_named(CAST_NAME) {
            self.category_mut(id).unwrap().cast = true;
            return id;
        }
        let id = CategoryId(self.fresh());
        self.categories.insert(
            0,
            Category {
                id,
                name: CAST_NAME.to_string(),
                auto_board_id: false,
                exclude_from_board: false,
                defaults: DoodRules::default(),
                cast: true,
                daily_quantity: false,
                properties: Vec::new(),
                multi_line: Vec::new(),
            },
        );
        id
    }

    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn element_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    pub fn element_named(&self, category: CategoryId, name: &str) -> Option<ElementId> {
        self.elements
            .iter()
            .find(|e| e.category == category && same_name(&e.name, name))
            .map(|e| e.id)
    }

    /// Adds an element with its category's default rules and, when the category numbers
    /// automatically, the next board ID: one more than the highest whole number in use.
    pub fn add_element(&mut self, category: CategoryId, name: &str) -> Result<ElementId, Error> {
        let cat = self
            .category(category)
            .ok_or(Error::NoSuchCategory(category))?
            .clone();
        if name.trim().is_empty() {
            return Err(Error::EmptyName);
        }
        if self.element_named(category, name).is_some() {
            return Err(Error::DuplicateName(name.trim().to_string()));
        }
        let board_id = cat
            .auto_board_id
            .then(|| self.free_board_id(category).to_string());
        let id = ElementId(self.fresh());
        self.elements.push(Element {
            id,
            category,
            name: name.trim().to_string(),
            board_id,
            board_id_locked: false,
            // The category's setting is a default for new elements (decision 120).
            exclude_from_board: cat.exclude_from_board,
            rules: cat.defaults,
            notes: String::new(),
            links: Vec::new(),
            red_flags: Vec::new(),
            properties: Vec::new(),
            images: Vec::new(),
        });
        Ok(id)
    }

    /// One more than the highest whole-number board ID in `category`, or 1 with none
    /// (behavior note 17, A14: a gap below the highest is not filled). IDs with anything
    /// but digits in them (`12A`) are ignored; `07` counts as 7.
    fn free_board_id(&self, category: CategoryId) -> u64 {
        let used: HashSet<&str> = self
            .elements
            .iter()
            .filter(|e| e.category == category)
            .filter_map(|e| e.board_id.as_deref())
            .collect();
        let highest = used
            .iter()
            .filter(|b| !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()))
            .map(|b| b.parse::<u64>().unwrap_or(u64::MAX))
            .max()
            .unwrap_or(0);
        // Past the largest number a board ID can hold, the lowest free one serves.
        match highest.checked_add(1) {
            Some(n) => n,
            None => (1..)
                .find(|n: &u64| !used.contains(n.to_string().as_str()))
                .unwrap(),
        }
    }

    /// Sets or clears an element's board ID: letters and digits ([`board_id_ok`]), unique
    /// in its category.
    pub fn set_board_id(
        &mut self,
        element: ElementId,
        board_id: Option<&str>,
    ) -> Result<(), Error> {
        let el = self.element(element).ok_or(Error::NoSuchElement(element))?;
        if let Some(b) = board_id {
            if !board_id_ok(b) {
                return Err(Error::Broken(format!(
                    "a board ID is letters and digits, at most {MAX_BOARD_ID}, like 12 or \
                     12A; {b:?} is not"
                )));
            }
            let taken = self.elements.iter().any(|e| {
                e.id != element && e.category == el.category && e.board_id.as_deref() == Some(b)
            });
            if taken {
                return Err(Error::DuplicateBoardId(b.to_string()));
            }
        }
        self.element_mut(element).unwrap().board_id = board_id.map(str::to_string);
        Ok(())
    }

    // ---- editing categories and elements (the Elements tab)

    /// Renames a category, the cast category too (behavior note 17, A10). Names are
    /// unique ignoring case.
    pub fn rename_category(&mut self, id: CategoryId, name: &str) -> Result<(), Error> {
        self.category(id).ok_or(Error::NoSuchCategory(id))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if self
            .categories
            .iter()
            .any(|c| c.id != id && same_name(&c.name, name))
        {
            return Err(Error::DuplicateCategory(name.to_string()));
        }
        self.category_mut(id).unwrap().name = name.to_string();
        Ok(())
    }

    /// Moves a user category to place `to` among the user categories (from 0; past the
    /// end means last). Built-in categories keep their places.
    pub fn move_category(&mut self, id: CategoryId, to: usize) -> Result<(), Error> {
        let cat = self.category(id).ok_or(Error::NoSuchCategory(id))?;
        if cat.is_built_in() {
            return Err(Error::BuiltIn(cat.name.clone()));
        }
        let slots: Vec<usize> = (0..self.categories.len())
            .filter(|&i| !self.categories[i].is_built_in())
            .collect();
        let mut user: Vec<Category> = slots.iter().map(|&i| self.categories[i].clone()).collect();
        let from = user.iter().position(|c| c.id == id).unwrap();
        let moving = user.remove(from);
        user.insert(to.min(user.len()), moving);
        for (slot, c) in slots.into_iter().zip(user) {
            self.categories[slot] = c;
        }
        Ok(())
    }

    /// Deletes a user category that holds no elements.
    pub fn delete_category(&mut self, id: CategoryId) -> Result<(), Error> {
        let cat = self.category(id).ok_or(Error::NoSuchCategory(id))?;
        if cat.is_built_in() {
            return Err(Error::BuiltIn(cat.name.clone()));
        }
        if self.elements.iter().any(|e| e.category == id) {
            return Err(Error::NotEmpty(cat.name.clone()));
        }
        self.categories.retain(|c| c.id != id);
        self.tidy_report_filters();
        crate::plan::tidy(self);
        Ok(())
    }

    /// Gives every element of `category` the category's default hold and drop rules (the
    /// explicit "apply to existing elements"; defaults otherwise reach new elements only,
    /// inventory section 4). Returns how many elements changed.
    pub fn apply_defaults(&mut self, category: CategoryId) -> Result<usize, Error> {
        let c = self
            .category(category)
            .ok_or(Error::NoSuchCategory(category))?;
        let (rules, excluded) = (c.defaults, c.exclude_from_board);
        let mut changed = 0;
        for e in self.elements.iter_mut().filter(|e| e.category == category) {
            if e.rules != rules || e.exclude_from_board != excluded {
                e.rules = rules;
                // Left off the board is a default too since decision 120.
                e.exclude_from_board = excluded;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Renames an element. Names are unique in the category, ignoring case.
    pub fn rename_element(&mut self, id: ElementId, name: &str) -> Result<(), Error> {
        let el = self.element(id).ok_or(Error::NoSuchElement(id))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if self
            .elements
            .iter()
            .any(|e| e.id != id && e.category == el.category && same_name(&e.name, name))
        {
            return Err(Error::DuplicateName(name.to_string()));
        }
        self.element_mut(id).unwrap().name = name.to_string();
        Ok(())
    }

    /// Whether an element links to others or others link to it.
    pub fn is_linked(&self, id: ElementId) -> bool {
        self.element(id).is_some_and(|e| !e.links.is_empty())
            || self.elements.iter().any(|e| e.links.contains(&id))
    }

    /// Replaces each of `gone` by `with` in `list`, keeping the first entry of each id.
    fn replace_ids(list: &mut Vec<ElementId>, gone: &HashSet<ElementId>, with: ElementId) {
        let mut seen = HashSet::new();
        list.retain_mut(|e| {
            if gone.contains(e) {
                *e = with;
            }
            seen.insert(*e)
        });
    }

    /// Merges `others` into `survivor`, all of one category: every sheet and event that
    /// listed any of them lists the survivor once, in the place of the first, links and
    /// red flags follow, and the others are deleted. The survivor keeps its own name, board ID and rules.
    pub fn merge_elements(
        &mut self,
        survivor: ElementId,
        others: &[ElementId],
    ) -> Result<(), Error> {
        let keep = self
            .element(survivor)
            .ok_or(Error::NoSuchElement(survivor))?;
        let category = keep.category;
        let mut gone = HashSet::new();
        for &o in others {
            let e = self.element(o).ok_or(Error::NoSuchElement(o))?;
            if o == survivor {
                continue;
            }
            if e.category != category {
                return Err(Error::Broken(
                    "only elements of one category can be merged".to_string(),
                ));
            }
            gone.insert(o);
        }
        if gone.is_empty() {
            return Ok(());
        }
        let mut inherited: Vec<ElementId> = self
            .elements
            .iter()
            .filter(|e| gone.contains(&e.id))
            .flat_map(|e| e.links.iter().copied())
            .collect();
        let mut flags: Vec<RedFlag> = self
            .elements
            .iter()
            .filter(|e| gone.contains(&e.id))
            .flat_map(|e| e.red_flags.iter().cloned())
            .collect();
        for sheet in &mut self.sheets {
            Self::replace_ids(&mut sheet.elements, &gone, survivor);
            // A quantity follows its element; the first one met stays.
            let mut seen = HashSet::new();
            sheet.quantities.retain_mut(|q| {
                if gone.contains(&q.element) {
                    q.element = survivor;
                }
                seen.insert(q.element)
            });
        }
        for b in &mut self.boards {
            for sb in &mut b.sub_boards {
                for ev in &mut sb.calendar.events {
                    Self::replace_ids(&mut ev.elements, &gone, survivor);
                }
            }
        }
        self.elements.retain(|e| !gone.contains(&e.id));
        for e in &mut self.elements {
            if e.id == survivor {
                e.links.append(&mut inherited);
                e.red_flags.append(&mut flags);
            }
            let me = e.id;
            Self::replace_ids(&mut e.links, &gone, survivor);
            e.links.retain(|l| *l != me);
        }
        for r in &mut self.reports {
            Self::replace_ids(&mut r.options.elements, &gone, survivor);
        }
        // A lock's cast follows too, so drift since the lock sees one actor, not one
        // dropped and one added.
        for l in &mut self.plan.locks {
            for sc in &mut l.scenes {
                Self::replace_ids(&mut sc.cast, &gone, survivor);
            }
        }
        Ok(())
    }

    /// Moves an element to another category. Its name must be free there (ignoring case),
    /// and so must its board ID: the real app refuses the move otherwise (behavior note 17,
    /// A12). It keeps its ID and lock; one without an ID keeps none, also in a category
    /// that numbers automatically: that setting is for new elements only (behavior note
    /// 27, E-L7, decision 140). Rules, links and sheets stay; custom property values are
    /// lost.
    pub fn move_element(&mut self, id: ElementId, to: CategoryId) -> Result<(), Error> {
        let el = self.element(id).ok_or(Error::NoSuchElement(id))?.clone();
        self.category(to).ok_or(Error::NoSuchCategory(to))?;
        if el.category == to {
            return Ok(());
        }
        if self.element_named(to, &el.name).is_some() {
            return Err(Error::DuplicateName(el.name.clone()));
        }
        let id_free = |b: &str| {
            !self
                .elements
                .iter()
                .any(|e| e.category == to && e.board_id.as_deref() == Some(b))
        };
        if let Some(b) = el.board_id.as_deref().filter(|b| !id_free(b)) {
            return Err(Error::DuplicateBoardId(b.to_string()));
        }
        let locked = el.board_id_locked && el.board_id.is_some();
        let e = self.element_mut(id).unwrap();
        e.category = to;
        e.board_id_locked = locked;
        // Properties belong to the category, so their values are lost (inventory
        // section 4; the manual over the older video).
        e.properties.clear();
        Ok(())
    }

    /// Renumbers a category's board IDs from `start`, walking `order` (every element of
    /// the category once, as the Elements tab lists them). Locked elements keep their IDs,
    /// and so do IDs with anything but digits in them (`12A`, `B`); the numbers those use
    /// are skipped. Every other element, with or without an ID, gets the next free number.
    pub fn renumber(
        &mut self,
        category: CategoryId,
        start: u32,
        order: &[ElementId],
    ) -> Result<(), Error> {
        self.category(category)
            .ok_or(Error::NoSuchCategory(category))?;
        let mine: HashSet<ElementId> = self
            .elements
            .iter()
            .filter(|e| e.category == category)
            .map(|e| e.id)
            .collect();
        let listed: HashSet<ElementId> = order.iter().copied().collect();
        if listed != mine || listed.len() != order.len() {
            return Err(Error::Broken(
                "the order must list every element of the category once".to_string(),
            ));
        }
        let numeric = |b: &str| !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit());
        let kept =
            |e: &Element| e.board_id_locked || e.board_id.as_deref().is_some_and(|b| !numeric(b));
        let taken: HashSet<u64> = self
            .elements
            .iter()
            .filter(|e| e.category == category && kept(e))
            .filter_map(|e| e.board_id.as_deref())
            .filter(|b| numeric(b))
            .filter_map(|b| b.parse::<u64>().ok())
            .collect();
        let mut n = u64::from(start);
        let mut fresh = Vec::new();
        for id in order {
            let e = self.element(*id).unwrap();
            if kept(e) {
                continue;
            }
            while taken.contains(&n) {
                n += 1;
            }
            fresh.push((*id, n.to_string()));
            n += 1;
        }
        for (id, b) in fresh {
            self.element_mut(id).unwrap().board_id = Some(b);
        }
        Ok(())
    }

    /// Deletes an element: it leaves every sheet and event. A linked element is refused
    /// unless `unlink` is set, which drops its links both ways first.
    pub fn delete_element(&mut self, id: ElementId, unlink: bool) -> Result<(), Error> {
        let el = self.element(id).ok_or(Error::NoSuchElement(id))?;
        if self.is_linked(id) && !unlink {
            return Err(Error::Linked(el.name.clone()));
        }
        self.elements.retain(|e| e.id != id);
        for e in &mut self.elements {
            e.links.retain(|l| *l != id);
        }
        for sheet in &mut self.sheets {
            sheet.elements.retain(|e| *e != id);
        }
        for b in &mut self.boards {
            for sb in &mut b.sub_boards {
                for ev in &mut sb.calendar.events {
                    ev.elements.retain(|e| *e != id);
                }
            }
        }
        self.tidy_report_filters();
        for sheet in &mut self.sheets {
            sheet.quantities.retain(|q| q.element != id);
        }
        Ok(())
    }

    // ---- custom properties (inventory section 4)

    fn property_name(name: &str) -> Result<String, Error> {
        let t = name.trim();
        if t.is_empty() {
            return Err(Error::EmptyName);
        }
        if t.chars().count() > MAX_PROPERTY_NAME || t.chars().any(char::is_control) {
            return Err(Error::Broken(format!(
                "a property name is one line of at most {MAX_PROPERTY_NAME} characters"
            )));
        }
        Ok(t.to_string())
    }

    /// Adds a custom property to a category, at the end. Names are unique in the
    /// category, ignoring case.
    pub fn add_property(&mut self, category: CategoryId, name: &str) -> Result<(), Error> {
        let name = Self::property_name(name)?;
        let c = self
            .category_mut(category)
            .ok_or(Error::NoSuchCategory(category))?;
        if c.properties.iter().any(|p| same_name(p, &name)) {
            return Err(Error::DuplicateName(name));
        }
        c.properties.push(name);
        Ok(())
    }

    /// Renames property `at` of a category; its elements' values follow.
    pub fn rename_property(
        &mut self,
        category: CategoryId,
        at: usize,
        name: &str,
    ) -> Result<(), Error> {
        let name = Self::property_name(name)?;
        let c = self
            .category_mut(category)
            .ok_or(Error::NoSuchCategory(category))?;
        let old = c
            .properties
            .get(at)
            .cloned()
            .ok_or_else(|| Error::Broken("that property is no longer there".to_string()))?;
        if c.properties
            .iter()
            .enumerate()
            .any(|(i, p)| i != at && same_name(p, &name))
        {
            return Err(Error::DuplicateName(name));
        }
        c.properties[at] = name.clone();
        for m in c.multi_line.iter_mut().filter(|m| **m == old) {
            *m = name.clone();
        }
        for e in self.elements.iter_mut().filter(|e| e.category == category) {
            for v in e.properties.iter_mut().filter(|v| v.name == old) {
                v.name = name.clone();
            }
        }
        Ok(())
    }

    /// Moves property `at` of a category to place `to`; its elements' values follow the
    /// order.
    pub fn move_property(
        &mut self,
        category: CategoryId,
        at: usize,
        to: usize,
    ) -> Result<(), Error> {
        let c = self
            .category_mut(category)
            .ok_or(Error::NoSuchCategory(category))?;
        if at >= c.properties.len() || to >= c.properties.len() {
            return Err(Error::Broken(
                "that property is no longer there".to_string(),
            ));
        }
        let p = c.properties.remove(at);
        c.properties.insert(to, p);
        let order = c.properties.clone();
        c.multi_line
            .sort_by_key(|m| order.iter().position(|p| p == m));
        for e in self.elements.iter_mut().filter(|e| e.category == category) {
            e.properties
                .sort_by_key(|v| order.iter().position(|p| *p == v.name));
        }
        Ok(())
    }

    /// Deletes property `at` of a category, and every element's value of it.
    pub fn delete_property(&mut self, category: CategoryId, at: usize) -> Result<(), Error> {
        let c = self
            .category_mut(category)
            .ok_or(Error::NoSuchCategory(category))?;
        if at >= c.properties.len() {
            return Err(Error::Broken(
                "that property is no longer there".to_string(),
            ));
        }
        let old = c.properties.remove(at);
        c.multi_line.retain(|m| *m != old);
        for e in self.elements.iter_mut().filter(|e| e.category == category) {
            e.properties.retain(|v| v.name != old);
        }
        Ok(())
    }

    /// Whether property `at` of a category takes several lines (decision 119). Values
    /// already set stay as they are either way.
    pub fn set_property_multi_line(
        &mut self,
        category: CategoryId,
        at: usize,
        on: bool,
    ) -> Result<(), Error> {
        let c = self
            .category_mut(category)
            .ok_or(Error::NoSuchCategory(category))?;
        let name = c
            .properties
            .get(at)
            .cloned()
            .ok_or_else(|| Error::Broken("that property is no longer there".to_string()))?;
        c.multi_line.retain(|m| *m != name);
        if on {
            c.multi_line.push(name);
            let order = c.properties.clone();
            c.multi_line
                .sort_by_key(|m| order.iter().position(|p| p == m));
        }
        Ok(())
    }

    /// Whether a category's property `name` (exactly as listed) takes several lines.
    pub fn property_multi_line(&self, category: CategoryId, name: &str) -> bool {
        self.category(category)
            .is_some_and(|c| c.multi_line.iter().any(|m| m == name))
    }

    /// Sets an element's value of one of its category's properties (named ignoring
    /// case); an empty value clears it. Values are kept in the category's order.
    pub fn set_property_value(
        &mut self,
        element: ElementId,
        name: &str,
        value: &str,
    ) -> Result<(), Error> {
        let e = self.element(element).ok_or(Error::NoSuchElement(element))?;
        let order = self
            .category(e.category)
            .ok_or(Error::NoSuchCategory(e.category))?
            .properties
            .clone();
        let name = order
            .iter()
            .find(|p| same_name(p, name))
            .cloned()
            .ok_or_else(|| Error::Broken(format!("the category has no property {name:?}")))?;
        let e = self.element_mut(element).unwrap();
        e.properties.retain(|v| v.name != name);
        let value = value.trim();
        if !value.is_empty() {
            e.properties.push(PropertyValue {
                name,
                value: value.to_string(),
            });
            e.properties
                .sort_by_key(|v| order.iter().position(|p| *p == v.name));
        }
        Ok(())
    }

    // ---- linked elements (inventory section 4; behavior note 04; engine check E-L2)

    /// The element and every element it reaches through links, each once: its own links,
    /// their links, and so on through every level (behavior note 25: actor to prop to
    /// prop all arrive), depth first in link order. A cycle stops at an element already
    /// met, so A to B to A gives A, B.
    pub fn link_chain(&self, element: ElementId) -> Vec<ElementId> {
        let mut out = vec![element];
        let mut stack = vec![(element, 0usize)];
        while let Some((id, i)) = stack.pop() {
            let Some(next) = self.element(id).and_then(|e| e.links.get(i)).copied() else {
                continue;
            };
            stack.push((id, i + 1));
            if !out.contains(&next) {
                out.push(next);
                stack.push((next, 0));
            }
        }
        out
    }

    /// Puts an element on a sheet with every element its links reach (`link_chain`),
    /// each once, in that order: adding the main element adds its linked ones, theirs
    /// too (behavior notes 04 and 25). Returns the elements newly on the sheet.
    pub fn tag_with_links(
        &mut self,
        sheet: SheetId,
        element: ElementId,
    ) -> Result<Vec<ElementId>, Error> {
        self.element(element).ok_or(Error::NoSuchElement(element))?;
        self.sheet(sheet).ok_or(Error::NoSuchSheet(sheet))?;
        let mut added = Vec::new();
        for id in self.link_chain(element) {
            if self.sheet(sheet).is_some_and(|h| !h.elements.contains(&id)) {
                self.tag(sheet, id)?;
                added.push(id);
            }
        }
        Ok(added)
    }

    /// Sets the elements an element links to: adding it to a sheet from now on adds them
    /// too. Each once, never itself. With `retroactive`, every sheet that already holds it
    /// gets every element its links now reach (`link_chain`); without, nothing already on
    /// a sheet changes,
    /// and removing a link never takes anything off a sheet (behavior note 04). Returns
    /// how many sheets gained an element.
    pub fn set_links(
        &mut self,
        element: ElementId,
        links: &[ElementId],
        retroactive: bool,
    ) -> Result<usize, Error> {
        self.element(element).ok_or(Error::NoSuchElement(element))?;
        let mut list = Vec::new();
        for &l in links {
            self.element(l).ok_or(Error::NoSuchElement(l))?;
            if l == element {
                return Err(Error::Broken("an element can't link to itself".to_string()));
            }
            if !list.contains(&l) {
                list.push(l);
            }
        }
        self.element_mut(element).unwrap().links = list;
        if !retroactive {
            return Ok(0);
        }
        let list: Vec<ElementId> = self.link_chain(element).into_iter().skip(1).collect();
        let mut changed = 0;
        for sheet in self
            .sheets
            .iter_mut()
            .filter(|h| h.elements.contains(&element))
        {
            let before = sheet.elements.len();
            for l in &list {
                if !sheet.elements.contains(l) {
                    sheet.elements.push(*l);
                }
            }
            if sheet.elements.len() > before {
                changed += 1;
            }
        }
        Ok(changed)
    }

    // ---- quantities (inventory section 3)

    /// Sets how many of `element` a sheet needs and whether that counts in the daily
    /// totals. The element must be on the sheet; a count of 1, counted daily, is the
    /// default and leaves no entry.
    pub fn set_quantity(
        &mut self,
        sheet: SheetId,
        element: ElementId,
        count: u32,
        daily: bool,
    ) -> Result<(), Error> {
        self.element(element).ok_or(Error::NoSuchElement(element))?;
        if count == 0 || count > MAX_QUANTITY {
            return Err(Error::Broken(format!(
                "a quantity is from 1 to {MAX_QUANTITY}"
            )));
        }
        let h = self.sheet_mut(sheet).ok_or(Error::NoSuchSheet(sheet))?;
        if !h.elements.contains(&element) {
            return Err(Error::Broken(
                "that element is not on the sheet".to_string(),
            ));
        }
        let at = h.quantities.iter().position(|q| q.element == element);
        let q = Quantity {
            element,
            count,
            daily,
        };
        match (at, count == 1 && daily) {
            (Some(i), true) => {
                h.quantities.remove(i);
            }
            (Some(i), false) => h.quantities[i] = q,
            (None, true) => {}
            (None, false) => {
                // Kept in the order the sheet lists its elements.
                h.quantities.push(q);
                let order = h.elements.clone();
                h.quantities
                    .sort_by_key(|q| order.iter().position(|e| *e == q.element));
            }
        }
        Ok(())
    }

    // ---- red flags (decision 33)

    pub fn red_flag_type(&self, id: RedFlagTypeId) -> Option<&RedFlagType> {
        self.red_flag_types.iter().find(|t| t.id == id)
    }

    pub fn red_flag_type_named(&self, name: &str) -> Option<RedFlagTypeId> {
        self.red_flag_types
            .iter()
            .find(|t| same_name(&t.name, name))
            .map(|t| t.id)
    }

    /// Adds a red flag type at the end of the list, or returns the one already called
    /// `name` (ignoring case).
    pub fn add_red_flag_type(&mut self, name: &str) -> Result<RedFlagTypeId, Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if let Some(id) = self.red_flag_type_named(name) {
            return Ok(id);
        }
        let id = RedFlagTypeId(self.fresh());
        self.red_flag_types.push(RedFlagType {
            id,
            name: name.to_string(),
            color: None,
            builtin: None,
        });
        Ok(id)
    }

    /// Adds the built-in red flag types the schedule lacks, one per event kind but
    /// `other`, at the end of the list in the kinds' order, named as the kind is
    /// (`Travel`, `Camera test`). A type of the schedule's own already called so (ignoring
    /// case) becomes that built-in type instead, keeping its name, color and flags.
    /// Returns how many types it added or marked; nothing changes when none are missing.
    pub fn standard_red_flag_types(&mut self) -> usize {
        let mut changed = 0;
        for kind in EventKind::builtin_flag_kinds() {
            if self
                .red_flag_types
                .iter()
                .any(|t| t.builtin.as_ref() == Some(&kind))
            {
                continue;
            }
            let name = kind.label();
            changed += 1;
            if let Some(t) = self
                .red_flag_types
                .iter_mut()
                .find(|t| t.builtin.is_none() && same_name(&t.name, &name))
            {
                t.builtin = Some(kind);
                continue;
            }
            let id = RedFlagTypeId(self.fresh());
            self.red_flag_types.push(RedFlagType {
                id,
                name,
                color: None,
                builtin: Some(kind),
            });
        }
        changed
    }

    /// The built-in red flag types the schedule lacks (see
    /// [`Schedule::standard_red_flag_types`]).
    pub fn missing_standard_red_flag_types(&self) -> usize {
        EventKind::builtin_flag_kinds()
            .iter()
            .filter(|k| {
                !self
                    .red_flag_types
                    .iter()
                    .any(|t| t.builtin.as_ref() == Some(*k))
            })
            .count()
    }

    /// Renames a red flag type. Names are unique ignoring case.
    pub fn rename_red_flag_type(&mut self, id: RedFlagTypeId, name: &str) -> Result<(), Error> {
        self.red_flag_type(id).ok_or(Error::NoSuchRedFlagType(id))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if self
            .red_flag_types
            .iter()
            .any(|t| t.id != id && same_name(&t.name, name))
        {
            return Err(Error::DuplicateName(name.to_string()));
        }
        let t = self.red_flag_types.iter_mut().find(|t| t.id == id).unwrap();
        t.name = name.to_string();
        Ok(())
    }

    /// Moves a red flag type to place `to` in the list (from 0; past the end means last).
    pub fn move_red_flag_type(&mut self, id: RedFlagTypeId, to: usize) -> Result<(), Error> {
        let from = self
            .red_flag_types
            .iter()
            .position(|t| t.id == id)
            .ok_or(Error::NoSuchRedFlagType(id))?;
        let t = self.red_flag_types.remove(from);
        let to = to.min(self.red_flag_types.len());
        self.red_flag_types.insert(to, t);
        Ok(())
    }

    /// Deletes a red flag type no red flag uses. A built-in type is never deleted.
    pub fn delete_red_flag_type(&mut self, id: RedFlagTypeId) -> Result<(), Error> {
        let t = self.red_flag_type(id).ok_or(Error::NoSuchRedFlagType(id))?;
        if t.builtin.is_some() {
            return Err(Error::BuiltInType(t.name.clone()));
        }
        if self
            .elements
            .iter()
            .flat_map(|e| &e.red_flags)
            .any(|f| f.kind == id)
        {
            return Err(Error::InUse(t.name.clone()));
        }
        self.red_flag_types.retain(|t| t.id != id);
        Ok(())
    }

    /// A red flag and the element it is on.
    pub fn red_flag(&self, id: RedFlagId) -> Option<(ElementId, &RedFlag)> {
        self.elements
            .iter()
            .find_map(|e| e.red_flags.iter().find(|f| f.id == id).map(|f| (e.id, f)))
    }

    /// Adds a red flag to `element` (when `flag.id` is `RedFlagId(0)`) or replaces the
    /// flag with that id, which must be on `element`. Refused for a missing element, type
    /// or flag, or a range that ends before it starts. Returns the flag's id.
    pub fn set_red_flag(
        &mut self,
        element: ElementId,
        mut flag: RedFlag,
    ) -> Result<RedFlagId, Error> {
        self.element(element).ok_or(Error::NoSuchElement(element))?;
        self.red_flag_type(flag.kind)
            .ok_or(Error::NoSuchRedFlagType(flag.kind))?;
        if let RedFlagDates::Range { from, to } = flag.dates {
            if to < from {
                return Err(Error::Broken(
                    "a red flag can't end before it starts".to_string(),
                ));
            }
        }
        if flag.id == RedFlagId(0) {
            flag.id = RedFlagId(self.fresh());
            let id = flag.id;
            self.element_mut(element).unwrap().red_flags.push(flag);
            return Ok(id);
        }
        let id = flag.id;
        match self.red_flag(id) {
            None => Err(Error::NoSuchRedFlag(id)),
            Some((on, _)) if on != element => Err(Error::Broken(format!(
                "red flag {} is on another element",
                id.0
            ))),
            Some(_) => {
                let e = self.element_mut(element).unwrap();
                let slot = e.red_flags.iter_mut().find(|f| f.id == id).unwrap();
                *slot = flag;
                Ok(id)
            }
        }
    }

    /// Adds one red flag of type `kind` for each element and each of `dates` (several
    /// elements and dates at once, behavior note 23), all with the same note and
    /// firmness. All or nothing: refused for a missing element or type, an element given
    /// twice, no elements or no dates, more than [`MAX_RED_FLAGS_AT_ONCE`] flags, or a
    /// range that ends before it starts. Returns the new flags' ids, element by element.
    pub fn add_red_flags(
        &mut self,
        elements: &[ElementId],
        kind: RedFlagTypeId,
        dates: &[RedFlagDates],
        note: &str,
        hard: bool,
    ) -> Result<Vec<RedFlagId>, Error> {
        if elements.is_empty() || dates.is_empty() {
            return Err(Error::Broken(
                "red flags need at least one element and one date".to_string(),
            ));
        }
        if elements.len().saturating_mul(dates.len()) > MAX_RED_FLAGS_AT_ONCE {
            return Err(Error::Broken(format!(
                "at most {MAX_RED_FLAGS_AT_ONCE} red flags at once"
            )));
        }
        self.red_flag_type(kind)
            .ok_or(Error::NoSuchRedFlagType(kind))?;
        for (i, e) in elements.iter().enumerate() {
            self.element(*e).ok_or(Error::NoSuchElement(*e))?;
            if elements[..i].contains(e) {
                return Err(Error::Broken(format!("element {} is given twice", e.0)));
            }
        }
        for d in dates {
            if let RedFlagDates::Range { from, to } = *d {
                if to < from {
                    return Err(Error::Broken(
                        "a red flag can't end before it starts".to_string(),
                    ));
                }
            }
        }
        let mut out = Vec::with_capacity(elements.len() * dates.len());
        for e in elements {
            for d in dates {
                out.push(self.set_red_flag(
                    *e,
                    RedFlag {
                        id: RedFlagId(0),
                        kind,
                        dates: *d,
                        note: note.to_string(),
                        hard,
                    },
                )?);
            }
        }
        Ok(out)
    }

    /// Deletes several red flags at once. All or nothing: refused when one is missing.
    pub fn delete_red_flags(&mut self, ids: &[RedFlagId]) -> Result<(), Error> {
        for id in ids {
            self.red_flag(*id).ok_or(Error::NoSuchRedFlag(*id))?;
        }
        for e in &mut self.elements {
            e.red_flags.retain(|f| !ids.contains(&f.id));
        }
        Ok(())
    }

    /// Deletes a red flag.
    pub fn delete_red_flag(&mut self, id: RedFlagId) -> Result<(), Error> {
        let (element, _) = self.red_flag(id).ok_or(Error::NoSuchRedFlag(id))?;
        self.element_mut(element)
            .unwrap()
            .red_flags
            .retain(|f| f.id != id);
        Ok(())
    }

    // ---- calendars and events

    /// A sub-board's calendar, to change.
    pub fn calendar_mut(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
    ) -> Result<&mut Calendar, Error> {
        self.board_mut(board)?
            .sub_boards
            .iter_mut()
            .find(|x| x.id == sub)
            .map(|x| &mut x.calendar)
            .ok_or(Error::NoSuchSubBoard(sub))
    }

    /// Adds an event to a sub-board's calendar (`at` is `None`) or replaces event `at`.
    /// Refused when it ends before it starts, covers more than [`MAX_EVENT_DAYS`] days or
    /// names a missing element; elements are kept once each, in order. Returns the
    /// event's index.
    pub fn set_event(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
        at: Option<usize>,
        mut event: Event,
    ) -> Result<usize, Error> {
        if event.to < event.from {
            return Err(Error::Broken(
                "an event can't end before it starts".to_string(),
            ));
        }
        if event.to.0 - event.from.0 >= MAX_EVENT_DAYS {
            return Err(Error::Broken(format!(
                "an event covers at most {MAX_EVENT_DAYS} days"
            )));
        }
        if event
            .color
            .is_some_and(|c| usize::from(c) >= EVENT_COLORS.len())
        {
            return Err(Error::Broken("an event has one of ten colors".to_string()));
        }
        if let Some(e) = event.elements.iter().find(|e| self.element(**e).is_none()) {
            return Err(Error::NoSuchElement(*e));
        }
        let mut seen = HashSet::new();
        event.elements.retain(|e| seen.insert(*e));
        let cal = self.calendar_mut(board, sub)?;
        match at {
            None => {
                cal.events.push(event);
                Ok(cal.events.len() - 1)
            }
            Some(i) if i < cal.events.len() => {
                cal.events[i] = event;
                Ok(i)
            }
            Some(_) => Err(Error::Broken("that event is no longer there".to_string())),
        }
    }

    /// Deletes event `at` from a sub-board's calendar.
    pub fn delete_event(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
        at: usize,
    ) -> Result<(), Error> {
        let cal = self.calendar_mut(board, sub)?;
        if at >= cal.events.len() {
            return Err(Error::Broken("that event is no longer there".to_string()));
        }
        cal.events.remove(at);
        Ok(())
    }

    // ---- sheets

    pub fn sheet(&self, id: SheetId) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    /// Every sheet by id, for many lookups.
    pub fn sheet_index(&self) -> SheetIndex<'_> {
        self.sheets.iter().map(|h| (h.id, h)).collect()
    }

    /// Every element by id, for many lookups.
    pub fn element_index(&self) -> ElementIndex<'_> {
        self.elements.iter().map(|e| (e.id, e)).collect()
    }

    pub fn sheet_mut(&mut self, id: SheetId) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    /// Adds a sheet (its `id` is replaced) at the end of every board's Boneyard.
    pub fn add_sheet(&mut self, mut sheet: Sheet) -> SheetId {
        let id = SheetId(self.fresh());
        sheet.id = id;
        self.sheets.push(sheet);
        for b in 0..self.boards.len() {
            let strip = Strip {
                id: StripId(self.fresh()),
                kind: StripKind::Sheet(id),
            };
            self.boards[b].boneyard.push(strip);
        }
        id
    }

    /// Adds an element to a sheet once.
    pub fn tag(&mut self, sheet: SheetId, element: ElementId) -> Result<(), Error> {
        self.element(element).ok_or(Error::NoSuchElement(element))?;
        let s = self.sheet_mut(sheet).ok_or(Error::NoSuchSheet(sheet))?;
        if !s.elements.contains(&element) {
            s.elements.push(element);
        }
        Ok(())
    }

    /// Takes an element off a sheet. An element that isn't on it changes nothing; the
    /// element itself stays in the schedule.
    pub fn untag(&mut self, sheet: SheetId, element: ElementId) -> Result<(), Error> {
        let s = self.sheet_mut(sheet).ok_or(Error::NoSuchSheet(sheet))?;
        s.elements.retain(|e| *e != element);
        s.quantities.retain(|q| q.element != element);
        Ok(())
    }

    /// Adds a sheet (its `id` is replaced) like [`Schedule::add_sheet`], except that on
    /// `board` its strip goes to `at` (`at.index` is clamped to the list's end). Every other
    /// board gets it at the end of its Boneyard. Refused, with nothing changed, when the
    /// board or sub-board doesn't exist.
    pub fn add_sheet_at(
        &mut self,
        sheet: Sheet,
        board: BoardId,
        at: Place,
    ) -> Result<SheetId, Error> {
        let b = self.board(board).ok_or(Error::NoSuchBoard(board))?;
        if let Some(sub) = at.sub_board {
            if !b.sub_boards.iter().any(|x| x.id == sub) {
                return Err(Error::NoSuchSubBoard(sub));
            }
        }
        let id = self.add_sheet(sheet);
        let b = self.board_mut(board)?;
        let strip = b.boneyard.pop().expect("add_sheet puts the strip last");
        let list = strips_mut(b, at.sub_board)?;
        list.insert(at.index.min(list.len()), strip);
        Ok(id)
    }

    /// Deletes a sheet and its strip on every board. Its elements stay in the schedule
    /// (other sheets may use them); day breaks and banners around it stay where they are.
    pub fn delete_sheet(&mut self, sheet: SheetId) -> Result<(), Error> {
        let i = self
            .sheets
            .iter()
            .position(|s| s.id == sheet)
            .ok_or(Error::NoSuchSheet(sheet))?;
        self.sheets.remove(i);
        let gone = |st: &Strip| st.kind == StripKind::Sheet(sheet);
        for b in &mut self.boards {
            b.boneyard.retain(|st| !gone(st));
            for sb in &mut b.sub_boards {
                sb.strips.retain(|st| !gone(st));
            }
        }
        crate::plan::tidy(self);
        Ok(())
    }

    /// Merges breakdown sheets into one: "merge strips" (inventory section 5, engine check
    /// E-S3, decisions 35 and 46). `survivor` keeps its id and its place on every board;
    /// the `others` are deleted and their strips leave every board (banners and day breaks
    /// around them stay where they are). "In order" means the order given: the survivor
    /// first, then `others` as listed (the app passes the order the strips were selected
    /// in, as the reference app joins them; note 18). The survivor's fields become:
    ///
    /// - `scenes`, `synopsis` and `comments`: each sheet's non-empty text in order, a
    ///   repeat left out, joined by `", "` (scenes) or a line break (the other two);
    /// - `eighths` summed; `shoot_minutes` and `screen_seconds` summed over the sheets
    ///   that have one (none when no sheet has one);
    /// - `elements`: every sheet's elements, in order, each once, each with the quantity
    ///   the first sheet in order gives it;
    /// - every other field (`int_ext`, `set`, `time_of_day`, `sunrise`, `sunset`,
    ///   `location`, `script_pages`, `script_day`, `unit`, `sequence`, `episode`): the
    ///   survivor's value, or, when it has none, the first value in order; any other
    ///   value is reported as dropped.
    ///
    /// Refused, with nothing changed: an unknown sheet, no others, the survivor among the
    /// others, a sheet given twice, a sum past `u32`.
    pub fn merge_sheets(
        &mut self,
        survivor: SheetId,
        others: &[SheetId],
    ) -> Result<SheetMerge, Error> {
        let before = self
            .sheet(survivor)
            .ok_or(Error::NoSuchSheet(survivor))?
            .clone();
        if others.is_empty() {
            return Err(Error::Broken(
                "merging needs at least two sheets".to_string(),
            ));
        }
        let mut seen = HashSet::new();
        for &o in others {
            self.sheet(o).ok_or(Error::NoSuchSheet(o))?;
            if o == survivor {
                return Err(Error::Broken(format!(
                    "sheet {} can't be merged with itself",
                    o.0
                )));
            }
            if !seen.insert(o) {
                return Err(Error::Broken(format!("sheet {} is given twice", o.0)));
            }
        }
        let all: Vec<SheetId> = std::iter::once(survivor)
            .chain(others.iter().copied())
            .collect();
        let order: std::collections::HashMap<SheetId, usize> =
            all.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let sheets: Vec<&Sheet> = all
            .iter()
            .map(|id| self.sheet(*id).expect("checked above"))
            .collect();
        let too_large = || Error::Broken("the merged total is too large".to_string());

        let mut after = before.clone();
        let join = |get: fn(&Sheet) -> &String, sep: &str| -> String {
            let mut parts: Vec<&str> = Vec::new();
            for h in &sheets {
                let t = get(h).as_str();
                if !t.trim().is_empty() && !parts.contains(&t) {
                    parts.push(t);
                }
            }
            parts.join(sep)
        };
        after.scenes = join(|h| &h.scenes, ", ");
        after.synopsis = join(|h| &h.synopsis, "\n");
        after.comments = join(|h| &h.comments, "\n");
        let mut pages = Eighths::ZERO;
        for h in &sheets {
            pages = pages.checked_add(h.pages).ok_or_else(too_large)?;
        }
        if pages.0 > MAX_SHEET_EIGHTHS {
            return Err(Error::Broken(
                "the merged sheet would have more than 999 pages".to_string(),
            ));
        }
        after.pages = pages;
        let sum = |vals: Vec<Option<u32>>| -> Result<Option<u32>, Error> {
            let mut total = None;
            for v in vals.into_iter().flatten() {
                total = Some(total.unwrap_or(0u32).checked_add(v).ok_or_else(too_large)?);
            }
            Ok(total)
        };
        after.shoot_time =
            sum(sheets.iter().map(|h| h.shoot_time.map(|m| m.0)).collect())?.map(Minutes);
        after.screen_time =
            sum(sheets.iter().map(|h| h.screen_time.map(|s| s.0)).collect())?.map(Seconds);
        if after.shoot_time.is_some_and(|m| m.0 > MAX_SHOOT_MINUTES)
            || after.screen_time.is_some_and(|x| x.0 > MAX_SCREEN_SECONDS)
        {
            return Err(Error::Broken(
                "the merged sheet's time would be longer than a day: shorten a sheet first"
                    .to_string(),
            ));
        }
        let mut elements = Vec::new();
        for h in &sheets {
            for e in &h.elements {
                if !elements.contains(e) {
                    elements.push(*e);
                }
            }
        }
        // A quantity comes from the first sheet in order that gives one.
        let mut quantities: Vec<Quantity> = Vec::new();
        for e in &elements {
            if let Some(q) = sheets
                .iter()
                .find_map(|h| h.quantities.iter().find(|q| q.element == *e))
            {
                quantities.push(*q);
            }
        }
        after.elements = elements;
        after.quantities = quantities;
        // Images: every sheet's, in order, each once.
        let mut images: Vec<String> = Vec::new();
        for h in &sheets {
            for im in &h.images {
                if !images.contains(im) {
                    images.push(im.clone());
                }
            }
        }
        after.images = images;

        // One-value fields: the survivor's, else the first in order.
        let mut dropped = Vec::new();
        let rest: Vec<&Sheet> = sheets
            .iter()
            .copied()
            .filter(|h| h.id != survivor)
            .collect();
        let mut text = |field: &'static str, get: fn(&Sheet) -> &String| -> String {
            let some = |h: &Sheet| Some(get(h).clone()).filter(|t| !t.trim().is_empty());
            keep_one(
                field,
                &before,
                &sheets,
                &rest,
                some,
                |t| t.clone(),
                &mut dropped,
            )
            .unwrap_or_default()
        };
        after.number = text("number", |h| &h.number);
        after.int_ext = text("int_ext", |h| &h.int_ext);
        after.set = text("set", |h| &h.set);
        after.time_of_day = text("time_of_day", |h| &h.time_of_day);
        after.location = text("location", |h| &h.location);
        after.script_pages = text("script_pages", |h| &h.script_pages);
        after.script_day = text("script_day", |h| &h.script_day);
        after.unit = text("unit", |h| &h.unit);
        after.sequence = text("sequence", |h| &h.sequence);
        after.episode = text("episode", |h| &h.episode);
        let show = |m: &u32| hm_text(*m);
        after.sunrise = keep_one(
            "sunrise",
            &before,
            &sheets,
            &rest,
            |h| h.sunrise,
            show,
            &mut dropped,
        );
        after.sunset = keep_one(
            "sunset",
            &before,
            &sheets,
            &rest,
            |h| h.sunset,
            show,
            &mut dropped,
        );

        // What changed on the survivor, in the file's field order.
        type Show = fn(&Sheet) -> String;
        let fields: [(&'static str, Show); 19] = [
            ("number", |h| h.number.clone()),
            ("scenes", |h| h.scenes.clone()),
            ("int_ext", |h| h.int_ext.clone()),
            ("set", |h| h.set.clone()),
            ("time_of_day", |h| h.time_of_day.clone()),
            ("sunrise", |h| h.sunrise.map(hm_text).unwrap_or_default()),
            ("sunset", |h| h.sunset.map(hm_text).unwrap_or_default()),
            ("eighths", |h| h.pages.to_string()),
            ("shoot_minutes", |h| {
                h.shoot_time.map(|m| hm_text(m.0)).unwrap_or_default()
            }),
            ("synopsis", |h| h.synopsis.clone()),
            ("location", |h| h.location.clone()),
            ("script_pages", |h| h.script_pages.clone()),
            ("script_day", |h| h.script_day.clone()),
            ("screen_seconds", |h| {
                h.screen_time.map(|s| ms_text(s.0)).unwrap_or_default()
            }),
            ("unit", |h| h.unit.clone()),
            ("sequence", |h| h.sequence.clone()),
            ("episode", |h| h.episode.clone()),
            ("comments", |h| h.comments.clone()),
            ("images", |h| h.images.join(", ")),
        ];
        let mut combined = Vec::new();
        for (field, show) in fields {
            let (b, a) = (show(&before), show(&after));
            if b != a {
                combined.push(CombinedField {
                    field,
                    before: b,
                    after: a,
                });
            }
        }
        let elements_added: Vec<ElementId> = after
            .elements
            .iter()
            .filter(|e| !before.elements.contains(e))
            .copied()
            .collect();
        let field_rank = |f: &str| fields.iter().position(|(n, _)| *n == f).unwrap_or(0);
        dropped.sort_by_key(|d: &DroppedValue| {
            (
                field_rank(d.field),
                order.get(&d.sheet).copied().unwrap_or(usize::MAX),
            )
        });
        let merged: Vec<SheetId> = all.iter().copied().filter(|id| *id != survivor).collect();

        *self.sheet_mut(survivor).expect("checked above") = after;
        let gone: HashSet<SheetId> = merged.iter().copied().collect();
        self.sheets.retain(|h| !gone.contains(&h.id));
        let is_gone = |st: &Strip| matches!(st.kind, StripKind::Sheet(id) if gone.contains(&id));
        for b in &mut self.boards {
            b.boneyard.retain(|st| !is_gone(st));
            for sb in &mut b.sub_boards {
                sb.strips.retain(|st| !is_gone(st));
            }
        }
        crate::plan::tidy(self);
        Ok(SheetMerge {
            survivor,
            merged,
            combined,
            elements_added,
            dropped,
        })
    }

    /// Copies a sheet, every field and element, to a new sheet: "duplicate to split a
    /// scene" (inventory section 3, decision 35). On every board where the original sits
    /// on a sub-board, the copy's strip goes right after it; on a board where it is in the
    /// Boneyard, at the Boneyard's end. Returns the new sheet's id.
    pub fn duplicate_sheet(&mut self, sheet: SheetId) -> Result<SheetId, Error> {
        let copy = self.sheet(sheet).ok_or(Error::NoSuchSheet(sheet))?.clone();
        let places: Vec<Option<Place>> = self
            .boards
            .iter()
            .map(|b| self.sheet_place(b.id, sheet))
            .collect();
        let id = self.add_sheet(copy);
        for (b, place) in self.boards.iter_mut().zip(places) {
            if let Some(Place {
                sub_board: Some(sub),
                index,
            }) = place
            {
                let strip = b.boneyard.pop().expect("add_sheet puts the strip last");
                let list = strips_mut(b, Some(sub))?;
                list.insert(index + 1, strip);
            }
        }
        Ok(id)
    }

    /// Where `sheet`'s strip is on `board`: its sub-board (`None`: the Boneyard) and index.
    pub fn sheet_place(&self, board: BoardId, sheet: SheetId) -> Option<Place> {
        let b = self.board(board)?;
        let lists = b
            .sub_boards
            .iter()
            .map(|sb| (Some(sb.id), &sb.strips))
            .chain(std::iter::once((None, &b.boneyard)));
        for (sub_board, list) in lists {
            if let Some(index) = list
                .iter()
                .position(|st| st.kind == StripKind::Sheet(sheet))
            {
                return Some(Place { sub_board, index });
            }
        }
        None
    }

    /// `board`'s sheets in board order: each sub-board's strips in turn, then the
    /// Boneyard. Empty when there is no such board.
    pub fn sheets_in_board_order(&self, board: BoardId) -> Vec<SheetId> {
        let Some(b) = self.board(board) else {
            return Vec::new();
        };
        // A scene spanning days is listed at its first part (decision 76).
        b.sheet_parts()
            .filter(|(_, _, first)| *first)
            .map(|(_, id, _)| id)
            .collect()
    }

    /// A sheet's strips on `board` in board order: one, or its parts when the scene spans
    /// days (decision 76).
    pub fn sheet_strips(&self, board: BoardId, sheet: SheetId) -> Vec<StripId> {
        let Some(b) = self.board(board) else {
            return Vec::new();
        };
        b.sub_boards
            .iter()
            .flat_map(|sb| &sb.strips)
            .chain(&b.boneyard)
            .filter(|st| st.kind == StripKind::Sheet(sheet))
            .map(|st| st.id)
            .collect()
    }

    /// The sheets with more than one strip on `board`: scenes spanning days.
    pub fn split_sheets(&self, board: BoardId) -> HashSet<SheetId> {
        self.board(board).map_or_else(HashSet::new, |b| {
            b.sheet_parts()
                .filter(|(_, _, first)| !*first)
                .map(|(_, id, _)| id)
                .collect()
        })
    }

    /// Adds a part to a scheduled scene so it spans days (inventory section 6, "multiple
    /// day breaks", decision 76): another strip of the same sheet at `at`, on a sub-board.
    /// The sheet's pages and times count on its first part only; its elements work on
    /// every part's day. Refused in the Boneyard, and for a sheet in the Boneyard.
    pub fn add_part(
        &mut self,
        board: BoardId,
        sheet: SheetId,
        at: Place,
    ) -> Result<StripId, Error> {
        if self.sheet(sheet).is_none() {
            return Err(Error::NoSuchSheet(sheet));
        }
        if at.sub_board.is_none() {
            return Err(Error::Broken(
                "a scene's parts go on a sub-board, not in the Boneyard".to_string(),
            ));
        }
        if self
            .sheet_place(board, sheet)
            .is_some_and(|p| p.sub_board.is_none())
        {
            return Err(Error::Broken(
                "schedule the scene on a sub-board before splitting it".to_string(),
            ));
        }
        let id = StripId(self.fresh());
        let list = strips_mut(self.board_mut(board)?, at.sub_board)?;
        list.insert(
            at.index.min(list.len()),
            Strip {
                id,
                kind: StripKind::Sheet(sheet),
            },
        );
        Ok(id)
    }

    /// Brings in an imported script: one sheet per scene, speaking characters as elements
    /// of `cast` matched by name, unknown names created. New sheets land at the end of
    /// every Boneyard. Returns the new sheets in script order.
    pub fn import_script(
        &mut self,
        script: &ImportedScript,
        cast: CategoryId,
    ) -> Result<Vec<SheetId>, Error> {
        self.category(cast).ok_or(Error::NoSuchCategory(cast))?;
        let mut out = Vec::new();
        for scene in &script.scenes {
            let int_ext = match scene.int_ext {
                IntExt::Int => "INT",
                IntExt::Ext => "EXT",
                IntExt::IntExt => "INT/EXT",
                IntExt::ExtInt => "EXT/INT",
                IntExt::Unknown => "",
            };
            let x = &scene.extra;
            let sheet = self.add_sheet(Sheet {
                scenes: scene.number.clone().unwrap_or_default(),
                int_ext: int_ext.to_string(),
                set: scene.set.clone(),
                time_of_day: scene.time_of_day.clone(),
                pages: scene.length,
                script_pages: x.script_page.clone(),
                script_day: x.story_day.clone(),
                synopsis: x.synopsis.clone(),
                sequence: x.sequence.clone(),
                episode: x.episode.clone(),
                ..Sheet::default()
            });
            // Speaking cast first, then the cast who don't speak (a script scenes file).
            for name in scene.characters.iter().chain(&x.nonspeaking) {
                let el = match self.element_named(cast, name) {
                    Some(el) => el,
                    None => self.add_element(cast, name)?,
                };
                self.tag_with_links(sheet, el)?;
            }
            if x.omitted && !self.plan.omitted.contains(&sheet) {
                self.plan.omitted.push(sheet);
            }
            out.push(sheet);
        }
        Ok(out)
    }

    // ---- boards

    pub fn board(&self, id: BoardId) -> Option<&Board> {
        self.boards.iter().find(|b| b.id == id)
    }

    fn board_mut(&mut self, id: BoardId) -> Result<&mut Board, Error> {
        self.boards
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(Error::NoSuchBoard(id))
    }

    /// Adds a board with one sub-board, Main, starting `start`, and every sheet in its
    /// Boneyard. The app passes today, as the reference app does (behavior note 27,
    /// decision 153); the engine reads no clock.
    pub fn add_board(&mut self, name: &str, start: Option<Date>) -> BoardId {
        let id = BoardId(self.fresh());
        let sub = SubBoardId(self.fresh());
        let boneyard = (0..self.sheets.len())
            .map(|i| Strip {
                id: StripId(self.fresh()),
                kind: StripKind::Sheet(self.sheets[i].id),
            })
            .collect();
        self.boards.push(Board {
            id,
            name: name.to_string(),
            strip_layout: StripLayout::default(),
            day_look: DayLook::default(),
            uses: Default::default(),
            sub_boards: vec![SubBoard {
                id: sub,
                name: "Main".to_string(),
                start,
                calendar: Calendar::default(),
                strips: Vec::new(),
            }],
            boneyard,
        });
        id
    }

    /// Copies a board with its sub-boards, strip order, banners and day breaks. Every
    /// copied board, sub-board and strip gets a new id.
    pub fn duplicate_board(&mut self, board: BoardId, name: &str) -> Result<BoardId, Error> {
        let mut copy = self.board(board).ok_or(Error::NoSuchBoard(board))?.clone();
        copy.id = BoardId(self.fresh());
        copy.name = name.to_string();
        for sb in &mut copy.sub_boards {
            sb.id = SubBoardId(self.fresh());
            for st in &mut sb.strips {
                st.id = StripId(self.fresh());
            }
        }
        for st in &mut copy.boneyard {
            st.id = StripId(self.fresh());
        }
        let id = copy.id;
        self.boards.push(copy);
        Ok(id)
    }

    pub fn add_sub_board(
        &mut self,
        board: BoardId,
        name: &str,
        start: Option<Date>,
    ) -> Result<SubBoardId, Error> {
        let id = SubBoardId(self.fresh());
        self.board_mut(board)?.sub_boards.push(SubBoard {
            id,
            name: name.to_string(),
            start,
            calendar: Calendar::default(),
            strips: Vec::new(),
        });
        Ok(id)
    }

    /// Deletes a sub-board; its sheet strips go to the end of the Boneyard, its banners
    /// and day breaks go with it. The last sub-board can't be deleted.
    pub fn delete_sub_board(&mut self, board: BoardId, sub: SubBoardId) -> Result<(), Error> {
        let b = self.board_mut(board)?;
        let i = b
            .sub_boards
            .iter()
            .position(|s| s.id == sub)
            .ok_or(Error::NoSuchSubBoard(sub))?;
        if b.sub_boards.len() == 1 {
            return Err(Error::Broken(
                "a board keeps at least one sub-board".to_string(),
            ));
        }
        let gone = b.sub_boards.remove(i);
        // A scene keeps its other parts where they are; one with every part here goes to
        // the Boneyard once (decision 76).
        let mut placed: HashSet<SheetId> = b
            .sub_boards
            .iter()
            .flat_map(|s| &s.strips)
            .filter_map(|s| match s.kind {
                StripKind::Sheet(id) => Some(id),
                _ => None,
            })
            .collect();
        b.boneyard.extend(
            gone.strips
                .into_iter()
                .filter(|s| matches!(s.kind, StripKind::Sheet(id) if placed.insert(id))),
        );
        crate::plan::tidy(self);
        Ok(())
    }

    /// A board name, trimmed: not empty, and no other board (`except` aside) has it,
    /// ignoring case. Engine check E-S1: confirm uniqueness in the real app.
    pub fn board_name(&self, name: &str, except: Option<BoardId>) -> Result<String, Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if self
            .boards
            .iter()
            .any(|b| Some(b.id) != except && same_name(&b.name, name))
        {
            return Err(Error::Broken(format!("there is already a board {name:?}")));
        }
        Ok(name.to_string())
    }

    /// A sub-board name for `board`, trimmed: not empty, and no other sub-board of that
    /// board (`except` aside) has it, ignoring case. Sub-boards of different boards may
    /// share a name (a duplicated board keeps its sub-boards' names). Engine check E-S1.
    pub fn sub_board_name(
        &self,
        board: BoardId,
        name: &str,
        except: Option<SubBoardId>,
    ) -> Result<String, Error> {
        let b = self.board(board).ok_or(Error::NoSuchBoard(board))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        if b.sub_boards
            .iter()
            .any(|x| Some(x.id) != except && same_name(&x.name, name))
        {
            return Err(Error::Broken(format!(
                "board {:?} already has a sub-board {name:?}",
                b.name
            )));
        }
        Ok(name.to_string())
    }

    /// Renames a board (see [`Schedule::board_name`]).
    pub fn rename_board(&mut self, board: BoardId, name: &str) -> Result<(), Error> {
        let name = self.board_name(name, Some(board))?;
        self.board_mut(board)?.name = name;
        Ok(())
    }

    /// Moves a board to place `to` among the boards (from 0; past the end means last).
    pub fn move_board(&mut self, board: BoardId, to: usize) -> Result<(), Error> {
        let from = self
            .boards
            .iter()
            .position(|b| b.id == board)
            .ok_or(Error::NoSuchBoard(board))?;
        let b = self.boards.remove(from);
        let at = to.min(self.boards.len());
        self.boards.insert(at, b);
        Ok(())
    }

    /// Deletes a board: its arrangement, sub-boards and calendars. The sheets stay (every
    /// other board holds them). The last board can't be deleted.
    pub fn delete_board(&mut self, board: BoardId) -> Result<(), Error> {
        let at = self
            .boards
            .iter()
            .position(|b| b.id == board)
            .ok_or(Error::NoSuchBoard(board))?;
        if self.boards.len() == 1 {
            return Err(Error::Broken(
                "a schedule keeps at least one board".to_string(),
            ));
        }
        self.boards.remove(at);
        crate::plan::tidy(self);
        Ok(())
    }

    /// Renames a sub-board (see [`Schedule::sub_board_name`]).
    pub fn rename_sub_board(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
        name: &str,
    ) -> Result<(), Error> {
        let name = self.sub_board_name(board, name, Some(sub))?;
        self.sub_board_mut(board, sub)?.name = name;
        Ok(())
    }

    /// Moves a sub-board to place `to` on its board (from 0; past the end means last).
    pub fn move_sub_board(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
        to: usize,
    ) -> Result<(), Error> {
        let b = self.board_mut(board)?;
        let from = b
            .sub_boards
            .iter()
            .position(|x| x.id == sub)
            .ok_or(Error::NoSuchSubBoard(sub))?;
        let x = b.sub_boards.remove(from);
        let at = to.min(b.sub_boards.len());
        b.sub_boards.insert(at, x);
        Ok(())
    }

    /// Sets or clears a sub-board's start date.
    pub fn set_start(
        &mut self,
        board: BoardId,
        sub: SubBoardId,
        start: Option<Date>,
    ) -> Result<(), Error> {
        self.sub_board_mut(board, sub)?.start = start;
        Ok(())
    }

    fn sub_board_mut(&mut self, board: BoardId, sub: SubBoardId) -> Result<&mut SubBoard, Error> {
        self.board_mut(board)?
            .sub_boards
            .iter_mut()
            .find(|x| x.id == sub)
            .ok_or(Error::NoSuchSubBoard(sub))
    }

    /// The board a sub-board belongs to (sub-board ids are unique across the schedule).
    pub fn board_of_sub(&self, sub: SubBoardId) -> Option<BoardId> {
        self.boards
            .iter()
            .find(|b| b.sub_boards.iter().any(|x| x.id == sub))
            .map(|b| b.id)
    }

    /// Changes a banner's text and duration. Anything but a banner is refused.
    pub fn set_banner(
        &mut self,
        board: BoardId,
        strip: StripId,
        banner: Banner,
    ) -> Result<(), Error> {
        let b = self.board_mut(board)?;
        let st = b
            .sub_boards
            .iter_mut()
            .flat_map(|x| x.strips.iter_mut())
            .find(|x| x.id == strip)
            .ok_or(Error::NoSuchStrip(strip))?;
        match &mut st.kind {
            StripKind::Banner(old) => {
                *old = banner;
                Ok(())
            }
            _ => Err(Error::Broken("that strip is not a banner".to_string())),
        }
    }

    /// Sets what a board's scene strips show (decision 39). Refused, with nothing changed,
    /// for an unknown board or a layout with no field or a field twice.
    pub fn set_strip_layout(&mut self, board: BoardId, layout: StripLayout) -> Result<(), Error> {
        if let Some(why) = layout.problem() {
            return Err(Error::Broken(why));
        }
        self.counts_known(&layout)?;
        self.board_mut(board)?.strip_layout = layout;
        Ok(())
    }

    /// Refuses a strip layout whose element count counts a category the schedule lacks.
    pub(crate) fn counts_known(&self, layout: &StripLayout) -> Result<(), Error> {
        match layout.counted().find(|c| self.category(*c).is_none()) {
            Some(c) => Err(Error::NoSuchCategory(c)),
            None => Ok(()),
        }
    }

    /// Sets what a board's day breaks show (decision 67).
    pub fn set_day_look(&mut self, board: BoardId, look: DayLook) -> Result<(), Error> {
        if let Some(why) = look.problem() {
            return Err(Error::Broken(why));
        }
        self.board_mut(board)?.day_look = look;
        Ok(())
    }

    /// Sorts a sub-board's strips (or the Boneyard's, `sub` `None`) by `keys`, the first
    /// deciding first, as the reference app's sort does (inventory, section 5): banners
    /// go to the top in the order they were in, day breaks are removed (re-break after),
    /// and sheets follow in key order. Ties keep their order. Empty values come last
    /// whichever way a key sorts. Text sorts naturally ([`natural_cmp`]), so scene `9`
    /// comes before `10` and `10` before `10A`; pages sort by length.
    pub fn sort_strips(
        &mut self,
        board: BoardId,
        sub: Option<SubBoardId>,
        keys: &[SortKey],
    ) -> Result<(), Error> {
        let sheets: std::collections::HashMap<SheetId, &Sheet> =
            self.sheets.iter().map(|h| (h.id, h)).collect();
        let b = self.boards.iter().find(|b| b.id == board);
        let b = b.ok_or(Error::NoSuchBoard(board))?;
        let list = match sub {
            None => &b.boneyard,
            Some(id) => {
                &b.sub_boards
                    .iter()
                    .find(|x| x.id == id)
                    .ok_or(Error::NoSuchSubBoard(id))?
                    .strips
            }
        };
        let mut banners = Vec::new();
        // A scene spanning days moves as one block, its parts in their order (DBK-11,
        // decision 134): one entry per sheet, holding every strip of it.
        let mut scenes: Vec<(Vec<Strip>, Option<&Sheet>)> = Vec::new();
        let mut at: std::collections::HashMap<SheetId, usize> = Default::default();
        for st in list {
            match st.kind {
                StripKind::Banner(_) => banners.push(st.clone()),
                StripKind::Sheet(id) => match at.get(&id) {
                    Some(&i) => scenes[i].0.push(st.clone()),
                    None => {
                        at.insert(id, scenes.len());
                        scenes.push((vec![st.clone()], sheets.get(&id).copied()));
                    }
                },
                StripKind::DayBreak => {}
            }
        }
        scenes.sort_by(|(_, a), (_, b)| {
            let (Some(a), Some(b)) = (a, b) else {
                return std::cmp::Ordering::Equal;
            };
            keys.iter()
                .map(|k| k.compare(a, b))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        banners.extend(scenes.into_iter().flat_map(|(parts, _)| parts));
        *strips_mut(self.board_mut(board)?, sub)? = banners;
        Ok(())
    }

    /// Sets the strip colors (decision 60). Refused: what [`StripColors::problem`] finds.
    pub fn set_strip_colors(&mut self, colors: StripColors) -> Result<(), Error> {
        if let Some(why) = colors.problem() {
            return Err(Error::Broken(why));
        }
        self.strip_colors = colors;
        Ok(())
    }

    /// Inserts a banner or a day break.
    pub fn insert_strip(
        &mut self,
        board: BoardId,
        kind: StripKind,
        at: Place,
    ) -> Result<StripId, Error> {
        if matches!(kind, StripKind::Sheet(_)) {
            return Err(Error::Broken(
                "sheet strips come from add_sheet".to_string(),
            ));
        }
        if at.sub_board.is_none() && matches!(kind, StripKind::Banner(_)) {
            return Err(Error::Broken(
                "banners don't go in the Boneyard".to_string(),
            ));
        }
        let id = StripId(self.fresh());
        let list = strips_mut(self.board_mut(board)?, at.sub_board)?;
        list.insert(at.index.min(list.len()), Strip { id, kind });
        Ok(id)
    }

    /// Deletes a banner, a day break or one part of a scene spanning days (decision 76).
    /// A sheet's only strip can't be deleted from one board alone.
    pub fn delete_strip(&mut self, board: BoardId, strip: StripId) -> Result<(), Error> {
        let parted: HashSet<SheetId> = self.split_sheets(board);
        let b = self.board_mut(board)?;
        for list in
            std::iter::once(&mut b.boneyard).chain(b.sub_boards.iter_mut().map(|s| &mut s.strips))
        {
            if let Some(i) = list.iter().position(|s| s.id == strip) {
                if matches!(list[i].kind, StripKind::Sheet(id) if !parted.contains(&id)) {
                    return Err(Error::Broken(
                        "delete the sheet to remove its strip".to_string(),
                    ));
                }
                list.remove(i);
                crate::plan::tidy(self);
                return Ok(());
            }
        }
        Err(Error::NoSuchStrip(strip))
    }

    /// Moves strips, in their current board order (each sub-board in turn, then the
    /// Boneyard), to `to`. `to.index` counts positions in
    /// the destination before the move, so dropping a selection "below row 5" means
    /// index 6 whether or not the selection came from above.
    pub fn move_strips(
        &mut self,
        board: BoardId,
        strips: &[StripId],
        to: Place,
    ) -> Result<(), Error> {
        let b = self.board_mut(board)?;
        strips_mut(b, to.sub_board)?;
        let wanted: IdSet<StripId> = strips.iter().copied().collect();
        // Refuse before changing anything.
        let present: IdSet<StripId> = b
            .boneyard
            .iter()
            .chain(b.sub_boards.iter().flat_map(|s| &s.strips))
            .map(|s| s.id)
            .collect();
        // Day breaks may be parked in the Boneyard (decision 57); banners may not, and
        // neither may a scene spanning days (decision 76).
        if to.sub_board.is_none() {
            let parted: IdSet<SheetId> = b
                .sheet_parts()
                .filter(|(_, _, first)| !*first)
                .map(|(_, id, _)| id)
                .collect();
            let split = b.sub_boards.iter().flat_map(|s| &s.strips).any(|s| {
                wanted.contains(&s.id)
                    && matches!(s.kind, StripKind::Sheet(id) if parted.contains(&id))
            });
            if split {
                return Err(Error::Broken(
                    "a scene spanning days goes to the Boneyard once its parts are joined"
                        .to_string(),
                ));
            }
            let banner = b
                .boneyard
                .iter()
                .chain(b.sub_boards.iter().flat_map(|s| &s.strips))
                .any(|s| wanted.contains(&s.id) && matches!(s.kind, StripKind::Banner(_)));
            if banner {
                return Err(Error::Broken(
                    "banners don't go in the Boneyard".to_string(),
                ));
            }
        }
        if let Some(missing) = wanted.iter().find(|id| !present.contains(id)) {
            return Err(Error::NoSuchStrip(*missing));
        }
        let mut moving = Vec::new();
        let mut index = to.index;
        // Board order: each sub-board in turn, then the Boneyard.
        for (sub, list) in b
            .sub_boards
            .iter_mut()
            .map(|s| (Some(s.id), &mut s.strips))
            .chain(std::iter::once((None, &mut b.boneyard)))
        {
            let mut kept = Vec::with_capacity(list.len());
            for (i, s) in list.drain(..).enumerate() {
                if wanted.contains(&s.id) {
                    if sub == to.sub_board && i < to.index {
                        index -= 1;
                    }
                    moving.push(s);
                } else {
                    kept.push(s);
                }
            }
            *list = kept;
        }
        let dest = strips_mut(b, to.sub_board)?;
        let at = index.min(dest.len());
        dest.splice(at..at, moving);
        Ok(())
    }

    // ---- checks

    /// Confirms the schedule hangs together: ids unique, references valid, element names
    /// and board IDs unique per category, red flag type names unique, red flag dates in
    /// order, every board holding every sheet exactly once.
    pub fn check(&self) -> Result<(), Error> {
        let broken = |why: String| Err(Error::Broken(why));
        let strips: usize = self
            .boards
            .iter()
            .map(|b| b.boneyard.len() + b.sub_boards.iter().map(|s| s.strips.len()).sum::<usize>())
            .sum();
        let mut ids = IdSet::with_capacity_and_hasher(
            strips + self.sheets.len() + self.elements.len() + 64,
            IdHash::default(),
        );
        // Lookups the checks below make many times over, indexed once (decision 190).
        let element_ids: IdSet<ElementId> = self.elements.iter().map(|e| e.id).collect();
        let has_element = |id: ElementId| element_ids.contains(&id);
        let mut all_ids = self
            .categories
            .iter()
            .map(|c| c.id.0)
            .chain(self.elements.iter().map(|e| e.id.0))
            .chain(self.sheets.iter().map(|s| s.id.0))
            .chain(self.boards.iter().map(|b| b.id.0))
            .chain(self.red_flag_types.iter().map(|t| t.id.0))
            .chain(
                self.elements
                    .iter()
                    .flat_map(|e| &e.red_flags)
                    .map(|f| f.id.0),
            );
        for b in &self.boards {
            for sb in &b.sub_boards {
                if !ids.insert(sb.id.0) {
                    return broken(format!("id {} used twice", sb.id.0));
                }
            }
            for st in b
                .boneyard
                .iter()
                .chain(b.sub_boards.iter().flat_map(|s| &s.strips))
            {
                if !ids.insert(st.id.0) {
                    return broken(format!("id {} used twice", st.id.0));
                }
            }
        }
        if let Some(dup) = all_ids.find(|id| !ids.insert(*id)) {
            return broken(format!("id {dup} used twice"));
        }
        if ids.iter().any(|&id| id >= self.next_id || id == 0) {
            return broken("an id is at or past next_id".to_string());
        }
        if self.next_id > MAX_NEXT_ID {
            return broken(format!("next_id is past {MAX_NEXT_ID}"));
        }
        self.check_layouts(&mut ids)?;

        for (i, c) in self.categories.iter().enumerate() {
            if c.name.trim().is_empty() {
                return broken("a category has no name".to_string());
            }
            if self.categories[..i]
                .iter()
                .any(|o| same_name(&o.name, &c.name))
            {
                return broken(format!("category {:?} appears twice", c.name));
            }
        }
        if self.categories.iter().filter(|c| c.cast).count() > 1 {
            return broken("more than one cast category".to_string());
        }
        for c in &self.categories {
            for (i, p) in c.properties.iter().enumerate() {
                if p.trim().is_empty() || *p != p.trim() {
                    return broken(format!("category {:?} has a property with no name", c.name));
                }
                if p.chars().count() > MAX_PROPERTY_NAME || p.chars().any(char::is_control) {
                    return broken(format!(
                        "category {:?} has a property name longer than \
                         {MAX_PROPERTY_NAME} characters or of more than one line",
                        c.name
                    ));
                }
                if c.properties[..i].iter().any(|o| same_name(o, p)) {
                    return broken(format!("category {:?} has property {p:?} twice", c.name));
                }
            }
            let mut last = None;
            for m in &c.multi_line {
                let at = c.properties.iter().position(|p| p == m);
                if at.is_none() || last >= at {
                    return broken(format!(
                        "category {:?} marks {m:?} as several lines, but it isn't one of its \
                         properties or is out of their order",
                        c.name
                    ));
                }
                last = at;
            }
        }
        for (i, t) in self.red_flag_types.iter().enumerate() {
            if t.name.trim().is_empty() {
                return broken("a red flag type has no name".to_string());
            }
            if self.red_flag_types[..i]
                .iter()
                .any(|o| same_name(&o.name, &t.name))
            {
                return broken(format!("red flag type {:?} appears twice", t.name));
            }
            if let Some(k) = &t.builtin {
                if matches!(k, EventKind::Other(_)) {
                    return broken(format!("red flag type {:?} is built in as other", t.name));
                }
                if self.red_flag_types[..i]
                    .iter()
                    .any(|o| o.builtin.as_ref() == Some(k))
                {
                    return broken(format!("two red flag types are built in as {}", k.key()));
                }
            }
        }
        let element_repeats = {
            let mut names = HashSet::with_capacity(self.elements.len());
            let mut board_ids = HashSet::new();
            self.elements.iter().any(|e| {
                !names.insert((e.category, e.name.trim().to_ascii_lowercase()))
                    || e.board_id
                        .as_deref()
                        .is_some_and(|b| !board_ids.insert((e.category, b)))
            })
        };
        for e in &self.elements {
            for f in &e.red_flags {
                if self.red_flag_type(f.kind).is_none() {
                    return broken(format!(
                        "red flag {} on {:?} has missing type {}",
                        f.id.0, e.name, f.kind.0
                    ));
                }
                if let RedFlagDates::Range { from, to } = f.dates {
                    if to < from {
                        return broken(format!(
                            "red flag {} on {:?} ends before it starts",
                            f.id.0, e.name
                        ));
                    }
                }
            }
            if self.category(e.category).is_none() {
                return broken(format!("element {:?} has no category", e.name));
            }
            // Every pair is compared only when some pair repeats, so the message names the
            // same element as before; the sets find whether one does.
            for other in self.elements.iter().filter(|_| element_repeats) {
                if other.id < e.id && other.category == e.category {
                    if same_name(&other.name, &e.name) {
                        return broken(format!("element {:?} appears twice in a category", e.name));
                    }
                    if other.board_id.is_some() && other.board_id == e.board_id {
                        return broken(format!("board ID {:?} used twice", e.board_id));
                    }
                }
            }
            if let Some(l) = e.links.iter().find(|l| !has_element(**l)) {
                return broken(format!(
                    "element {:?} links to missing element {}",
                    e.name, l.0
                ));
            }
            if e.links.contains(&e.id) || repeats(&e.links) {
                return broken(format!(
                    "element {:?} links to itself or to one element twice",
                    e.name
                ));
            }
            let props = &self.category(e.category).expect("checked above").properties;
            let mut last = None;
            for (i, v) in e.properties.iter().enumerate() {
                let Some(at) = props.iter().position(|p| *p == v.name) else {
                    return broken(format!(
                        "element {:?} has a value for {:?}, which its category doesn't have",
                        e.name, v.name
                    ));
                };
                // Values are kept in the category's order, as set_property_value keeps them.
                if last.is_some_and(|l| at < l) {
                    return broken(format!(
                        "element {:?} has its property values out of the category's order",
                        e.name
                    ));
                }
                last = Some(at);
                if v.value.trim().is_empty() || e.properties[..i].iter().any(|o| o.name == v.name) {
                    return broken(format!(
                        "element {:?} has an empty or repeated value for {:?}",
                        e.name, v.name
                    ));
                }
            }
            for (i, im) in e.images.iter().enumerate() {
                if im.trim().is_empty() || e.images[..i].contains(im) {
                    return broken(format!(
                        "element {:?} has an empty or repeated image path",
                        e.name
                    ));
                }
            }
        }
        for s in &self.sheets {
            if let Some(el) = s.elements.iter().find(|el| !has_element(**el)) {
                return broken(format!("sheet {} lists missing element {}", s.id.0, el.0));
            }
            if repeats(&s.elements) {
                return broken(format!("sheet {} lists an element twice", s.id.0));
            }
            for (i, q) in s.quantities.iter().enumerate() {
                if !s.elements.contains(&q.element) {
                    return broken(format!(
                        "sheet {} has a quantity for element {}, which isn't on it",
                        s.id.0, q.element.0
                    ));
                }
                if q.count == 0 || s.quantities[..i].iter().any(|o| o.element == q.element) {
                    return broken(format!(
                        "sheet {} has a zero or repeated quantity for element {}",
                        s.id.0, q.element.0
                    ));
                }
                if q.count > MAX_QUANTITY {
                    return broken(format!(
                        "sheet {} has a quantity past {MAX_QUANTITY}",
                        s.id.0
                    ));
                }
            }
            if s.pages.0 > MAX_SHEET_EIGHTHS {
                return broken(format!("sheet {} has more than 999 pages", s.id.0));
            }
            if s.shoot_time.is_some_and(|m| m.0 > MAX_SHOOT_MINUTES) {
                return broken(format!(
                    "sheet {} has a shoot time longer than a day",
                    s.id.0
                ));
            }
            if s.screen_time.is_some_and(|x| x.0 > MAX_SCREEN_SECONDS) {
                return broken(format!(
                    "sheet {} has a screen time longer than a day",
                    s.id.0
                ));
            }
            for (i, im) in s.images.iter().enumerate() {
                if im.trim().is_empty() || s.images[..i].contains(im) {
                    return broken(format!(
                        "sheet {} has an empty or repeated image path",
                        s.id.0
                    ));
                }
            }
        }
        if let Some(why) = self.sheet_looks.problem() {
            return broken(why);
        }
        if !self.calendar_fields.is_empty() {
            if let Some(why) = calendar_fields_problem(&self.calendar_fields) {
                return broken(why);
            }
        }
        if self.calendar_hidden.windows(2).any(|w| w[0] >= w[1]) {
            return broken("the calendar's hidden items are out of order or twice".to_string());
        }
        for (i, f) in self.sheet_fields.iter().enumerate() {
            if !SHEET_FIELDS.contains(f) || self.sheet_fields[..i].contains(f) {
                return broken(format!(
                    "the sheet field order names {f:?}, which is unknown or given twice"
                ));
            }
        }
        for ev in self
            .boards
            .iter()
            .flat_map(|b| &b.sub_boards)
            .flat_map(|sb| &sb.calendar.events)
        {
            if ev.to < ev.from {
                return broken(format!("event {:?} ends before it starts", ev.name));
            }
            if let Some(el) = ev.elements.iter().find(|el| !has_element(**el)) {
                return broken(format!(
                    "event {:?} lists missing element {}",
                    ev.name, el.0
                ));
            }
            if repeats(&ev.elements) {
                return broken(format!("event {:?} lists an element twice", ev.name));
            }
            if ev
                .color
                .is_some_and(|c| usize::from(c) >= EVENT_COLORS.len())
            {
                return broken(format!("event {:?} has a color out of range", ev.name));
            }
        }
        crate::setup::check_reports(self, &self.reports)?;
        if !self.header_layouts.is_empty() {
            crate::headers::check_headers(&self.header_layouts)?;
        }
        crate::plan::check(self)?;
        if let Some(why) = self.dood_codes.problem() {
            return broken(format!("day out of days codes: {why}"));
        }
        for sb in self.boards.iter().flat_map(|b| &b.sub_boards) {
            if let (Some(p), Some(w)) = (sb.calendar.prep_start, sb.calendar.wrap_end) {
                if w < p {
                    return broken(format!(
                        "sub-board {:?} wraps before its prep starts",
                        sb.name
                    ));
                }
            }
            if let Some(why) = sb.calendar.problem() {
                return broken(format!("sub-board {:?}: {why}", sb.name));
            }
        }
        let sheet_ids: IdSet<SheetId> = self.sheets.iter().map(|s| s.id).collect();
        if self.boards.is_empty() {
            return broken("the schedule has no board".to_string());
        }
        if let Some(why) = self.strip_colors.problem() {
            return broken(why);
        }
        for b in &self.boards {
            if b.sub_boards.is_empty() {
                return broken(format!("board {:?} has no sub-board", b.name));
            }
            if let Some(why) = b.strip_layout.problem() {
                return broken(format!("board {:?}: {why}", b.name));
            }
            if let Some(c) = b
                .strip_layout
                .counted()
                .find(|c| self.category(*c).is_none())
            {
                return broken(format!(
                    "board {:?}: its element count counts a missing category {}",
                    b.name, c.0
                ));
            }
            if let Some(why) = b.day_look.problem() {
                return broken(format!("board {:?}: {why}", b.name));
            }
            if b.boneyard
                .iter()
                .any(|st| matches!(st.kind, StripKind::Banner(_)))
            {
                return broken(format!("board {:?} has a banner in its Boneyard", b.name));
            }
            if b.sub_boards.iter().flat_map(|s| &s.strips).any(
                |st| matches!(&st.kind, StripKind::Banner(x) if x.duration.0 > MAX_SHOOT_MINUTES),
            ) {
                return broken(format!("board {:?} has a banner longer than a day", b.name));
            }
            // A sheet is on every board; several strips of one sheet are its parts, a scene
            // spanning days (decision 76), and never sit in the Boneyard.
            let mut seen = IdSet::with_capacity_and_hasher(sheet_ids.len(), IdHash::default());
            for st in b.sub_boards.iter().flat_map(|s| &s.strips) {
                if let StripKind::Sheet(id) = st.kind {
                    if !sheet_ids.contains(&id) {
                        return broken(format!(
                            "board {:?} has a strip for missing sheet {}",
                            b.name, id.0
                        ));
                    }
                    seen.insert(id);
                }
            }
            for st in &b.boneyard {
                if let StripKind::Sheet(id) = st.kind {
                    if !sheet_ids.contains(&id) {
                        return broken(format!(
                            "board {:?} has a strip for missing sheet {}",
                            b.name, id.0
                        ));
                    }
                    if !seen.insert(id) {
                        return broken(format!(
                            "board {:?} has sheet {} in the Boneyard and elsewhere",
                            b.name, id.0
                        ));
                    }
                }
            }
            if seen.len() != sheet_ids.len() {
                return broken(format!("board {:?} is missing sheets", b.name));
            }
        }
        Ok(())
    }
}

/// Whether a list of ids holds one twice. A short list (a sheet's elements, as a rule) is
/// compared pair by pair, which needs no set.
fn repeats<T: Copy + Eq + std::hash::Hash>(xs: &[T]) -> bool {
    if xs.len() <= 16 {
        return xs.iter().enumerate().any(|(i, x)| xs[..i].contains(x));
    }
    let mut seen = IdSet::with_capacity_and_hasher(xs.len(), IdHash::default());
    !xs.iter().all(|x| seen.insert(*x))
}

fn strips_mut(b: &mut Board, sub: Option<SubBoardId>) -> Result<&mut Vec<Strip>, Error> {
    match sub {
        None => Ok(&mut b.boneyard),
        Some(id) => b
            .sub_boards
            .iter_mut()
            .find(|s| s.id == id)
            .map(|s| &mut s.strips)
            .ok_or(Error::NoSuchSubBoard(id)),
    }
}

/// The breakdown sheet's header fields, by the keys the app and the file use, in the
/// order the Breakdown tab lays them out by default: three rows, as the reference app
/// does (decision 53), the sheet number first (decision 77). `pages` is whole pages plus
/// eighths.
pub const SHEET_FIELDS: [&str; 18] = [
    "number",
    "scenes",
    "int_ext",
    "set",
    "time_of_day",
    "shoot_time",
    "synopsis",
    "location",
    "pages",
    "script_pages",
    "script_day",
    "screen_time",
    "unit",
    "sequence",
    "episode",
    "comments",
    "sunrise",
    "sunset",
];

/// What a calendar strip shows until the schedule says otherwise (behavior note 19): the
/// scene number, the set, the elements and the cast IDs.
pub const DEFAULT_CALENDAR_FIELDS: [StripField; 4] = [
    StripField::Scene,
    StripField::Set,
    StripField::Elements,
    StripField::Cast,
];

/// The most fields a calendar strip shows.
pub const MAX_CALENDAR_FIELDS: usize = 13;

/// What the Calendar's View menu turns on and off (behavior note 22; decision 111). All
/// on by default; the schedule keeps the ones turned off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CalendarShow {
    /// Shoot day and day off labels (`Day 3`, `Off`).
    DayLabels,
    /// The day's pages.
    Pages,
    /// The day's estimated time.
    Time,
    /// How many scenes the day holds.
    Scenes,
    Events,
    /// The day's scene strips.
    Strips,
    /// The day's banners.
    Banners,
    RedFlags,
}

impl CalendarShow {
    pub const ALL: [CalendarShow; 8] = [
        CalendarShow::DayLabels,
        CalendarShow::Pages,
        CalendarShow::Time,
        CalendarShow::Scenes,
        CalendarShow::Events,
        CalendarShow::Strips,
        CalendarShow::Banners,
        CalendarShow::RedFlags,
    ];

    /// The key in the file and on the page: `day_labels`, `pages`, `time`, `scenes`,
    /// `events`, `strips`, `banners`, `red_flags`.
    pub fn key(self) -> &'static str {
        match self {
            CalendarShow::DayLabels => "day_labels",
            CalendarShow::Pages => "pages",
            CalendarShow::Time => "time",
            CalendarShow::Scenes => "scenes",
            CalendarShow::Events => "events",
            CalendarShow::Strips => "strips",
            CalendarShow::Banners => "banners",
            CalendarShow::RedFlags => "red_flags",
        }
    }

    /// In words, for the View menu.
    pub fn label(self) -> &'static str {
        match self {
            CalendarShow::DayLabels => "Shoot day and day off labels",
            CalendarShow::Pages => "Page counts",
            CalendarShow::Time => "Estimated time",
            CalendarShow::Scenes => "Scenes per day",
            CalendarShow::Events => "Events",
            CalendarShow::Strips => "Strips",
            CalendarShow::Banners => "Banners",
            CalendarShow::RedFlags => "Red flags",
        }
    }

    pub fn from_key(key: &str) -> Option<CalendarShow> {
        CalendarShow::ALL.into_iter().find(|c| c.key() == key)
    }
}

impl Schedule {
    /// Whether the Calendar shows `what` (decision 111).
    pub fn calendar_shows(&self, what: CalendarShow) -> bool {
        !self.calendar_hidden.contains(&what)
    }

    /// Sets what the Calendar hides; kept in [`CalendarShow::ALL`] order, each once.
    pub fn set_calendar_hidden(&mut self, hidden: &[CalendarShow]) {
        self.calendar_hidden = CalendarShow::ALL
            .into_iter()
            .filter(|c| hidden.contains(c))
            .collect();
    }
}

impl Schedule {
    /// The fields a calendar strip shows: the schedule's, else the default.
    pub fn calendar_strip_fields(&self) -> Vec<StripField> {
        if self.calendar_fields.is_empty() {
            DEFAULT_CALENDAR_FIELDS.to_vec()
        } else {
            self.calendar_fields.clone()
        }
    }

    /// Sets the calendar strip's fields: one to [`MAX_CALENDAR_FIELDS`] sheet fields, each
    /// once. The default is stored as empty, so it writes nothing.
    pub fn set_calendar_fields(&mut self, fields: Vec<StripField>) -> Result<(), Error> {
        if let Some(why) = calendar_fields_problem(&fields) {
            return Err(Error::Broken(why));
        }
        self.calendar_fields = if fields == DEFAULT_CALENDAR_FIELDS {
            Vec::new()
        } else {
            fields
        };
        Ok(())
    }
}

/// What is wrong with a calendar strip's fields, if anything.
pub fn calendar_fields_problem(fields: &[StripField]) -> Option<String> {
    if fields.is_empty() {
        return Some("a calendar strip shows at least one field".to_string());
    }
    if fields.len() > MAX_CALENDAR_FIELDS {
        return Some(format!(
            "a calendar strip shows {MAX_CALENDAR_FIELDS} fields at most"
        ));
    }
    if let Some(f) = fields.iter().find(|f| !f.of_sheet()) {
        return Some(format!("{} is for strip layouts only", f.label()));
    }
    if fields
        .iter()
        .enumerate()
        .any(|(i, f)| fields[..i].contains(f))
    {
        return Some("a calendar strip field is given twice".to_string());
    }
    None
}

/// A header field's width on the Breakdown tab (the breakdown designer, decision 101).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldWidth {
    Narrow,
    Medium,
    Wide,
}

impl FieldWidth {
    pub const ALL: [FieldWidth; 3] = [FieldWidth::Narrow, FieldWidth::Medium, FieldWidth::Wide];

    pub fn name(self) -> &'static str {
        match self {
            FieldWidth::Narrow => "narrow",
            FieldWidth::Medium => "medium",
            FieldWidth::Wide => "wide",
        }
    }

    pub fn from_name(name: &str) -> Option<FieldWidth> {
        Self::ALL.into_iter().find(|w| w.name() == name)
    }
}

/// How the Breakdown tab shows the sheet's header fields beyond their order: the fields
/// it hides and the widths set, each by key of [`SHEET_FIELDS`] in that list's order. A
/// field with no width set keeps its usual one. Hiding never touches a sheet's values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SheetLooks {
    pub hidden: Vec<&'static str>,
    pub widths: Vec<(&'static str, FieldWidth)>,
    /// How many of the shown fields each row of the sheet holds, in order (decision
    /// 186); fields past them make one more row. Empty: the usual six, five and the rest.
    pub rows: Vec<u8>,
}

impl SheetLooks {
    pub fn is_default(&self) -> bool {
        self.hidden.is_empty() && self.widths.is_empty() && self.rows.is_empty()
    }

    /// These looks with rows of `lengths` (empty for the usual rows), checked as
    /// [`SheetLooks::problem`] checks them.
    pub fn with_rows(self, lengths: &[u8]) -> Result<SheetLooks, Error> {
        let out = SheetLooks {
            rows: lengths.to_vec(),
            ..self
        };
        match out.rows_problem() {
            Some(why) => Err(Error::Broken(why)),
            None => Ok(out),
        }
    }

    /// Each row holds at least one field, and all of them together no more than a
    /// sheet has.
    fn rows_problem(&self) -> Option<String> {
        let total: usize = self.rows.iter().map(|n| usize::from(*n)).sum();
        if self.rows.contains(&0) {
            return Some("a sheet row holds at least one field".into());
        }
        if total > SHEET_FIELDS.len() {
            return Some(format!(
                "the sheet rows hold {total} fields, more than the {} a sheet has",
                SHEET_FIELDS.len()
            ));
        }
        None
    }

    pub fn width(&self, key: &str) -> Option<FieldWidth> {
        self.widths.iter().find(|(k, _)| *k == key).map(|(_, w)| *w)
    }

    /// For sentences: `hides unit, sunrise; set wide`, or `default`.
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        if !self.hidden.is_empty() {
            parts.push(format!("hides {}", self.hidden.join(", ")));
        }
        if !self.widths.is_empty() {
            parts.push(
                self.widths
                    .iter()
                    .map(|(k, w)| format!("{k} {}", w.name()))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !self.rows.is_empty() {
            parts.push(format!(
                "rows of {}",
                self.rows
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if parts.is_empty() {
            "default".to_string()
        } else {
            parts.join("; ")
        }
    }

    /// Built from keys as typed: each a sheet field, each once. Kept in
    /// [`SHEET_FIELDS`] order so one set of looks has one form.
    pub fn read(hidden: &[String], widths: &[(String, String)]) -> Result<SheetLooks, Error> {
        let key = |k: &str| {
            sheet_field_key(k.trim())
                .ok_or_else(|| Error::Broken(format!("{k:?} is not a sheet field")))
        };
        let mut h: Vec<&'static str> = Vec::new();
        for k in hidden {
            let k = key(k)?;
            if h.contains(&k) {
                return Err(Error::Broken(format!("{k:?} is hidden twice")));
            }
            h.push(k);
        }
        let mut w: Vec<(&'static str, FieldWidth)> = Vec::new();
        for (k, v) in widths {
            let k = key(k)?;
            let v = FieldWidth::from_name(v.trim())
                .ok_or_else(|| Error::Broken(format!("{v:?} is not a field width")))?;
            if w.iter().any(|(x, _)| *x == k) {
                return Err(Error::Broken(format!("{k:?} has two widths")));
            }
            w.push((k, v));
        }
        let at = |k: &str| SHEET_FIELDS.iter().position(|x| *x == k).unwrap_or(0);
        h.sort_by_key(|k| at(k));
        w.sort_by_key(|(k, _)| at(k));
        Ok(SheetLooks {
            hidden: h,
            widths: w,
            rows: Vec::new(),
        })
    }

    /// What `check` asks: known keys, each once, in [`SHEET_FIELDS`] order.
    pub(crate) fn problem(&self) -> Option<String> {
        let at = |k: &str| SHEET_FIELDS.iter().position(|x| *x == k);
        let ordered = |v: Vec<Option<usize>>| {
            v.iter().all(Option::is_some) && v.windows(2).all(|p| p[0] < p[1])
        };
        if !ordered(self.hidden.iter().map(|k| at(k)).collect()) {
            return Some("the hidden sheet fields are unknown, repeated or out of order".into());
        }
        if !ordered(self.widths.iter().map(|(k, _)| at(k)).collect()) {
            return Some("the sheet field widths are unknown, repeated or out of order".into());
        }
        self.rows_problem()
    }
}

/// A header field's key from [`SHEET_FIELDS`], as the static string itself.
pub fn sheet_field_key(key: &str) -> Option<&'static str> {
    SHEET_FIELDS.iter().copied().find(|k| *k == key)
}

/// The order Previous and Next step through a board's sheets (inventory section 3;
/// decision 77): the board's own order, or by sheet number or scene number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetOrder {
    /// Each sub-board's strips in turn, then the Boneyard.
    #[default]
    Board,
    /// By sheet number, naturally (9 before 10, 10 before 10A); sheets without one last.
    Number,
    /// By scene number, naturally; sheets without one last.
    Scene,
}

impl SheetOrder {
    pub fn parse(key: &str) -> Option<SheetOrder> {
        match key {
            "board" => Some(SheetOrder::Board),
            "number" => Some(SheetOrder::Number),
            "scene" => Some(SheetOrder::Scene),
            _ => None,
        }
    }
}

impl Schedule {
    /// The header fields in the order the Breakdown tab shows them: the order set (fields
    /// it leaves out follow in the default order), else [`SHEET_FIELDS`].
    pub fn sheet_field_order(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self.sheet_fields.clone();
        for f in SHEET_FIELDS {
            if !out.contains(&f) {
                out.push(f);
            }
        }
        out
    }

    /// Sets the header field order. Every key must be one of [`SHEET_FIELDS`], each once;
    /// fields left out follow in the default order. The default order is stored as
    /// empty, so a schedule set back to it writes nothing.
    pub fn set_sheet_field_order(&mut self, keys: &[String]) -> Result<(), Error> {
        let mut out: Vec<&'static str> = Vec::new();
        for k in keys {
            let key = sheet_field_key(k.trim())
                .ok_or_else(|| Error::Broken(format!("{k:?} is not a sheet field")))?;
            if out.contains(&key) {
                return Err(Error::Broken(format!("{key:?} is given twice")));
            }
            out.push(key);
        }
        for f in SHEET_FIELDS {
            if !out.contains(&f) {
                out.push(f);
            }
        }
        self.sheet_fields = if out == SHEET_FIELDS { Vec::new() } else { out };
        Ok(())
    }

    /// A board's sheets in `order`. Sorted by a number, sheets keep board order among
    /// equal numbers, and those without one come last.
    pub fn sheets_in_order(&self, board: BoardId, order: SheetOrder) -> Vec<SheetId> {
        let mut ids = self.sheets_in_board_order(board);
        let key: fn(&Sheet) -> &str = match order {
            SheetOrder::Board => return ids,
            SheetOrder::Number => |h| h.number.trim(),
            SheetOrder::Scene => |h| h.scenes.trim(),
        };
        ids.sort_by(|a, b| {
            let (x, y) = (
                self.sheet(*a).map_or("", key),
                self.sheet(*b).map_or("", key),
            );
            match (x.is_empty(), y.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => natural_cmp(x, y),
            }
        });
        ids
    }

    /// This schedule as a template (inventory section 1; decision 77): its categories,
    /// elements and their settings, red flag types, strip colors and layouts, report
    /// list, production info, code letters and sheet field order, and its boards and
    /// sub-boards with their calendars, but no sheets, no strips, no events and no red
    /// flags. Of the plan it keeps the map locations, the long day and the turnarounds,
    /// and drops what points at strips and sheets: locks, call times, pinned strips and
    /// omitted sheets. The title is kept.
    pub fn to_template(&self) -> Schedule {
        let mut t = self.clone();
        // A template, and every schedule started from one, is new: it gets the built-in
        // red flag types it lacks (decision 113).
        t.standard_red_flag_types();
        t.sheets.clear();
        t.plan.locks.clear();
        t.plan.call_times.clear();
        t.plan.days_shot.clear();
        t.plan.scenes_shot.clear();
        t.plan.pinned.clear();
        t.plan.omitted.clear();
        for e in &mut t.elements {
            e.red_flags.clear();
        }
        for b in &mut t.boards {
            b.boneyard.clear();
            for sb in &mut b.sub_boards {
                sb.strips.clear();
                sb.calendar.events.clear();
            }
        }
        t.tidy_report_filters();
        t
    }

    /// Why this schedule can't be kept as a template, if it can't: a template holds no
    /// sheets, strips, events or red flags, and no plan that points at them (locks, call
    /// times, pinned strips, omitted scenes). See [`Schedule::to_template`].
    pub fn template_problem(&self) -> Option<&'static str> {
        let subs = || self.boards.iter().flat_map(|b| &b.sub_boards);
        if !self.sheets.is_empty() {
            Some("a template holds no sheets")
        } else if self.boards.iter().any(|b| !b.boneyard.is_empty())
            || subs().any(|sb| !sb.strips.is_empty())
        {
            Some("a template holds no strips")
        } else if subs().any(|sb| !sb.calendar.events.is_empty()) {
            Some("a template holds no events")
        } else if self.elements.iter().any(|e| !e.red_flags.is_empty()) {
            Some("a template holds no red flags")
        } else if !(self.plan.locks.is_empty()
            && self.plan.call_times.is_empty()
            && self.plan.pinned.is_empty()
            && self.plan.omitted.is_empty())
        {
            Some("a template holds no locks, call times, pinned strips or omitted scenes")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    /// BYD-01 (decision 162): a sheet in messages is its scene number and set, else its
    /// heading, else its own sheet number; never its id.
    #[test]
    fn a_sheet_is_named_as_a_person_knows_it() {
        let sheet = |scenes: &str, ie: &str, set: &str, tod: &str, number: &str| Sheet {
            id: SheetId(237),
            scenes: scenes.into(),
            int_ext: ie.into(),
            set: set.into(),
            time_of_day: tod.into(),
            number: number.into(),
            ..Sheet::default()
        };
        assert_eq!(
            sheet("26", "INT", "KITCHEN", "DAY", "").title(),
            "Scene 26, INT. KITCHEN"
        );
        assert_eq!(
            sheet("3, 3A", "EXT", "PIER", "", "").title(),
            "Scenes 3, 3A, EXT. PIER"
        );
        assert_eq!(sheet("12", "", "", "", "").title(), "Scene 12");
        assert_eq!(
            sheet("", "INT/EXT", "CAR", "NIGHT", "").title(),
            "INT/EXT CAR - NIGHT"
        );
        assert_eq!(sheet("", "", "", "", "12A").title(), "Sheet 12A");
        let none = sheet("", "", "", "", "").title();
        assert!(!none.contains("237"), "{none}");
        assert_eq!(
            sheet("", "INT", "HALL", "DAY", "").heading(),
            "INT. HALL - DAY"
        );
    }

    /// Decision 101: hidden fields and widths read strictly, kept in one order, checked,
    /// diffed and merged as one value.
    #[test]
    fn sheet_looks_hide_and_size_fields() {
        let k = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let a = Schedule::new("ZTEST Looks");
        let mut b = a.clone();
        b.sheet_looks =
            SheetLooks::read(&k(&["sunset", "number"]), &[("set".into(), "wide".into())]).unwrap();
        assert_eq!(b.sheet_looks.hidden, ["number", "sunset"]);
        assert!(b.check().is_ok());
        assert!(SheetLooks::read(&k(&["unit", "unit"]), &[]).is_err());
        assert!(SheetLooks::read(&k(&["budget"]), &[]).is_err());
        assert!(SheetLooks::read(&[], &[("set".into(), "huge".into())]).is_err());
        let mut bad = b.clone();
        bad.sheet_looks.hidden.reverse();
        assert!(bad.check().is_err());
        let d = crate::diff::diff(&a, &b);
        assert_eq!(
            d.changes[0].text,
            "Sheet fields default -> hides number, sunset; set wide"
        );
        let m = crate::merge::merge(&a, &a, &b);
        assert_eq!(m.schedule.sheet_looks, b.sheet_looks);
        assert!(b.to_template().sheet_looks == b.sheet_looks);
    }
    use super::*;
    use crate::fountain;
    use crate::testgen::Rng;

    fn sample() -> (Schedule, BoardId, SubBoardId) {
        let mut s = Schedule::new("ZTEST Lantern Creek");
        let script = fountain::parse(
            "INT. CABIN - DAY #1#\n\nADA\nHi.\n\nBEN\nHey.\n\nEXT. CREEK - NIGHT #2#\n\nADA\nCold.\n\nINT. CABIN - NIGHT #3#\n\nCARL\nWho's there?\n",
        );
        let cast = s.category_named("Cast").unwrap();
        s.import_script(&script, cast).unwrap();
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        (s, b, sb)
    }

    #[test]
    fn adding_a_main_element_adds_its_linked_ones() {
        // Behavior notes 04 and 25, E-L2: a link from A to B adds B with A, and B's
        // links too, through every level; removing a link takes nothing off;
        // retroactive reaches sheets that already hold A.
        let (mut s, _, _) = sample();
        let cast = s.cast().unwrap();
        let props = s.add_category("Hand props");
        let (ada, ben, carl) = (
            s.element_named(cast, "ADA").unwrap(),
            s.element_named(cast, "BEN").unwrap(),
            s.element_named(cast, "CARL").unwrap(),
        );
        let lamp = s.add_element(props, "Lamp").unwrap();
        let rope = s.add_element(props, "Rope").unwrap();
        s.set_links(carl, &[lamp, lamp], false).unwrap();
        assert_eq!(s.element(carl).unwrap().links, [lamp]);
        s.set_links(lamp, &[rope], false).unwrap();
        assert!(s.set_links(carl, &[carl], false).is_err());
        let h1 = s.sheets[0].id; // ADA, BEN
        assert_eq!(s.tag_with_links(h1, carl).unwrap(), [carl, lamp, rope]);
        assert_eq!(s.tag_with_links(h1, carl).unwrap(), []);
        s.untag(h1, rope).unwrap();
        // Retroactive: ADA's sheets gain the link; without, nothing changes.
        s.set_links(ada, &[rope], false).unwrap();
        assert!(!s.sheet(h1).unwrap().elements.contains(&rope));
        // Sheet 1 (ADA, BEN) gains the rope; sheet 2 (ADA) the rope and BEN.
        assert_eq!(s.set_links(ada, &[rope, ben], true).unwrap(), 2);
        assert!(s.sheet(h1).unwrap().elements.contains(&rope));
        // Removing the link leaves the sheet alone.
        s.set_links(ada, &[], true).unwrap();
        assert!(s.sheet(h1).unwrap().elements.contains(&rope));
        s.check().unwrap();
    }

    #[test]
    fn linked_elements_chain_through_every_level_and_stop_at_a_cycle() {
        // Behavior note 25: actor to prop, prop to a second prop; adding the actor adds
        // both props. A cycle (rope back to the actor) adds nothing twice.
        let (mut s, _, _) = sample();
        let cast = s.cast().unwrap();
        let props = s.add_category("Hand props");
        let carl = s.element_named(cast, "CARL").unwrap();
        let lamp = s.add_element(props, "Lamp").unwrap();
        let rope = s.add_element(props, "Rope").unwrap();
        let wick = s.add_element(props, "Wick").unwrap();
        s.set_links(carl, &[lamp], false).unwrap();
        s.set_links(lamp, &[rope, wick], false).unwrap();
        s.set_links(rope, &[carl, lamp], false).unwrap();
        assert_eq!(s.link_chain(carl), [carl, lamp, rope, wick]);
        assert_eq!(s.link_chain(rope), [rope, carl, lamp, wick]);
        let h = s.add_sheet(Sheet::default());
        assert_eq!(s.tag_with_links(h, carl).unwrap(), [carl, lamp, rope, wick]);
        // Retroactive reaches the whole chain on sheets already holding the element.
        let h2 = s.add_sheet(Sheet::default());
        s.tag(h2, wick).unwrap();
        let tove = s.add_element(props, "Tinder").unwrap();
        s.set_links(tove, &[], false).unwrap();
        s.set_links(wick, &[tove], false).unwrap();
        let h3 = s.add_sheet(Sheet::default());
        s.tag(h3, carl).unwrap();
        assert!(s.set_links(carl, &[lamp], true).unwrap() >= 1);
        let on = &s.sheet(h3).unwrap().elements;
        for e in [carl, lamp, rope, wick, tove] {
            assert!(on.contains(&e));
        }
        s.check().unwrap();
    }

    #[test]
    fn a_new_schedule_s_cast_has_starter_properties_of_one_or_several_lines() {
        // Behavior note 24; decision 119. Schedule::new stays bare.
        assert!(Schedule::new("x").categories[0].properties.is_empty());
        let mut s = Schedule::with_default_categories("ZTEST Lantern Creek");
        let cast = s.cast().unwrap();
        let c = s.category(cast).unwrap();
        let names: Vec<&str> = CAST_PROPERTIES.iter().map(|(n, _)| *n).collect();
        assert_eq!(c.properties, names);
        assert_eq!(c.multi_line, ["Address", "Deal notes"]);
        assert!(s.property_multi_line(cast, "Address"));
        assert!(!s.property_multi_line(cast, "Phone"));
        // The marks follow a rename, a move and a delete.
        s.rename_property(cast, 1, "Home address").unwrap();
        s.move_property(cast, 7, 0).unwrap();
        assert_eq!(
            s.category(cast).unwrap().multi_line,
            ["Deal notes", "Home address"]
        );
        s.delete_property(cast, 0).unwrap();
        s.set_property_multi_line(cast, 0, true).unwrap();
        assert_eq!(
            s.category(cast).unwrap().multi_line,
            ["Full name", "Home address"]
        );
        s.set_property_multi_line(cast, 0, false).unwrap();
        assert!(s.set_property_multi_line(cast, 99, true).is_err());
        s.check().unwrap();
        let mut t = s.clone();
        t.category_mut(cast).unwrap().multi_line = vec!["Nope".into()];
        assert!(t.check().is_err());
    }

    #[test]
    fn custom_properties_belong_to_the_category() {
        let (mut s, _, _) = sample();
        let cast = s.cast().unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        s.add_property(cast, "Agent").unwrap();
        s.add_property(cast, "Height").unwrap();
        assert!(s.add_property(cast, " agent ").is_err(), "ignoring case");
        assert!(s.add_property(cast, "  ").is_err());
        s.set_property_value(ada, "HEIGHT", "1.70 m").unwrap();
        s.set_property_value(ada, "Agent", "Ruth Varga").unwrap();
        let names = |s: &Schedule| -> Vec<String> {
            s.element(ada)
                .unwrap()
                .properties
                .iter()
                .map(|v| v.name.clone())
                .collect()
        };
        assert_eq!(names(&s), ["Agent", "Height"], "the category's order");
        assert!(s.set_property_value(ada, "Weight", "x").is_err());
        s.rename_property(cast, 1, "Height (m)").unwrap();
        assert_eq!(names(&s), ["Agent", "Height (m)"]);
        s.move_property(cast, 1, 0).unwrap();
        assert_eq!(names(&s), ["Height (m)", "Agent"]);
        s.set_property_value(ada, "agent", "").unwrap();
        assert_eq!(names(&s), ["Height (m)"]);
        s.delete_property(cast, 0).unwrap();
        assert!(names(&s).is_empty());
        // A move to another category loses the values.
        s.set_property_value(ada, "Agent", "Ruth").unwrap();
        let other = s.add_category("Stand-ins");
        s.move_element(ada, other).unwrap();
        assert!(names(&s).is_empty());
        s.check().unwrap();
    }

    #[test]
    fn quantities_stay_with_their_elements() {
        let (mut s, _, _) = sample();
        let cast = s.cast().unwrap();
        let (ada, ben) = (
            s.element_named(cast, "ADA").unwrap(),
            s.element_named(cast, "BEN").unwrap(),
        );
        let h1 = s.sheets[0].id;
        let h3 = s.sheets[2].id; // CARL only
        assert!(
            s.set_quantity(h3, ada, 2, true).is_err(),
            "not on the sheet"
        );
        assert!(s.set_quantity(h1, ada, 0, true).is_err());
        s.set_quantity(h1, ben, 4, false).unwrap();
        s.set_quantity(h1, ada, 3, true).unwrap();
        let q = |s: &Schedule| s.sheet(h1).unwrap().quantities.clone();
        assert_eq!(q(&s)[0].element, ada, "in the sheet's order");
        assert_eq!(s.sheet(h1).unwrap().quantity(ben), (4, false));
        s.set_quantity(h1, ada, 1, true).unwrap();
        assert_eq!(q(&s).len(), 1, "back to the default leaves no entry");
        s.untag(h1, ben).unwrap();
        assert!(q(&s).is_empty());
        s.tag(h1, ben).unwrap();
        s.set_quantity(h1, ben, 4, true).unwrap();
        s.merge_elements(ada, &[ben]).unwrap();
        assert_eq!(s.sheet(h1).unwrap().quantity(ada), (4, true));
        s.check().unwrap();
        s.delete_element(ada, true).unwrap();
        assert!(q(&s).is_empty());
        s.check().unwrap();
    }

    #[test]
    fn a_new_schedule_lists_our_default_categories() {
        let s = Schedule::with_default_categories("ZTEST");
        s.check().unwrap();
        assert!(s.categories[0].cast);
        let names: Vec<&str> = s.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names.len(), DEFAULT_CATEGORIES.len() + 1);
        assert!(
            s.categories
                .iter()
                .find(|c| c.name == "Background")
                .unwrap()
                .daily_quantity
        );
        assert!(!s.categories[0].daily_quantity, "cast off by default");
    }

    fn sheet_order(s: &Schedule, b: BoardId, sub: Option<SubBoardId>) -> Vec<String> {
        let board = s.board(b).unwrap();
        let list = match sub {
            None => &board.boneyard,
            Some(id) => &board.sub_boards.iter().find(|x| x.id == id).unwrap().strips,
        };
        list.iter()
            .map(|st| match &st.kind {
                StripKind::Sheet(id) => s.sheet(*id).unwrap().scenes.clone(),
                StripKind::Banner(b) => format!("[{}]", b.text),
                StripKind::DayBreak => "|".to_string(),
            })
            .collect()
    }

    #[test]
    fn import_creates_sheets_and_matches_cast_by_name() {
        let (s, b, _) = sample();
        s.check().unwrap();
        assert_eq!(s.sheets.len(), 3);
        assert_eq!(sheet_order(&s, b, None), ["1", "2", "3"]);
        let names: Vec<_> = s
            .elements
            .iter()
            .map(|e| (e.name.as_str(), e.board_id.as_deref()))
            .collect();
        // Import leaves board IDs empty, as the real app does (D10).
        assert_eq!(names, [("ADA", None), ("BEN", None), ("CARL", None)]);
        assert_eq!(s.sheets[1].elements, [s.elements[0].id]);
        assert_eq!(s.sheets[0].int_ext, "INT");
        assert_eq!(s.sheets[1].time_of_day, "NIGHT");
    }

    #[test]
    fn reimport_reuses_elements_and_adds_sheets() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("cast").unwrap();
        let again = fountain::parse("INT. CABIN - DAY #1#\n\nada\nHi.\n");
        s.import_script(&again, cast).unwrap();
        s.check().unwrap();
        assert_eq!(s.sheets.len(), 4);
        assert_eq!(s.elements.len(), 3);
    }

    #[test]
    fn element_names_and_board_ids_are_unique_per_category() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        assert_eq!(
            s.add_element(cast, " ada "),
            Err(Error::DuplicateName("ada".to_string()))
        );
        assert_eq!(s.add_element(cast, "  "), Err(Error::EmptyName));
        let ada = s.element_named(cast, "ADA").unwrap();
        let carl = s.element_named(cast, "CARL").unwrap();
        s.set_board_id(ada, Some("1")).unwrap();
        s.set_board_id(carl, Some("3")).unwrap();
        s.category_mut(cast).unwrap().auto_board_id = true;
        let props = s.add_category("Props");
        let lamp = s.add_element(props, "ADA").unwrap();
        assert_eq!(s.element(lamp).unwrap().board_id, None);
        let ben = s.element_named(cast, "BEN").unwrap();
        assert_eq!(
            s.set_board_id(ben, Some("1")),
            Err(Error::DuplicateBoardId("1".to_string()))
        );
        // Letters and digits only, 1 to 12 of them, whoever sets it (the app, MCP).
        for bad in ["", " 2", "2 A", "2-A", "1234567890123", "2\n", "é"] {
            assert!(
                matches!(s.set_board_id(ben, Some(bad)), Err(Error::Broken(_))),
                "{bad:?}"
            );
        }
        assert_eq!(s.element(ben).unwrap().board_id, None);
        s.set_board_id(ben, Some("123456789012")).unwrap();
        s.set_board_id(ben, Some("2A")).unwrap();
        // One more than the highest whole number goes to the next new cast member; the
        // gap at 2 stays and `2A` doesn't count (behavior note 17, A14).
        let dee = s.add_element(cast, "DEE").unwrap();
        assert_eq!(s.element(dee).unwrap().board_id.as_deref(), Some("4"));
        s.set_board_id(carl, Some("09")).unwrap();
        let eve = s.add_element(cast, "EVE").unwrap();
        assert_eq!(s.element(eve).unwrap().board_id.as_deref(), Some("10"));
        // With no whole-number ID in the category, 1.
        s.category_mut(props).unwrap().auto_board_id = true;
        s.set_board_id(lamp, Some("L")).unwrap();
        let rope = s.add_element(props, "ROPE").unwrap();
        assert_eq!(s.element(rope).unwrap().board_id.as_deref(), Some("1"));
        s.check().unwrap();
    }

    #[test]
    fn category_defaults_apply_to_new_elements_only() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        s.category_mut(cast).unwrap().defaults = DoodRules {
            holds: true,
            drops: true,
            min_drop_gap: 10,
        };
        assert_eq!(
            s.element(s.element_named(cast, "ADA").unwrap())
                .unwrap()
                .rules,
            DoodRules::default()
        );
        let dee = s.add_element(cast, "DEE").unwrap();
        assert!(s.element(dee).unwrap().rules.holds);
    }

    #[test]
    fn moves_down_and_up_within_a_list() {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1", "2", "3"]);
        // Drop scene 1 below scene 3 (index 3 counts the list before the move).
        s.move_strips(
            b,
            &[ids[0]],
            Place {
                sub_board: Some(sb),
                index: 3,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["2", "3", "1"]);
        // And back to the top.
        s.move_strips(
            b,
            &[ids[0]],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1", "2", "3"]);
        // Two strips at once keep their board order.
        s.move_strips(
            b,
            &[ids[2], ids[0]],
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["2", "1", "3"]);
        s.check().unwrap();
    }

    #[test]
    fn banners_day_breaks_and_sub_boards() {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            StripKind::DayBreak,
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        let banner = Banner {
            text: "Move to creek".to_string(),
            duration: Minutes(30),
            ..Default::default()
        };
        s.insert_strip(
            b,
            StripKind::Banner(banner),
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["1", "2", "[Move to creek]", "|", "3"]
        );
        assert!(s.delete_strip(b, ids[0]).is_err());

        let unit2 = s
            .add_sub_board(b, "Second unit", "2027-03-08".parse().ok())
            .unwrap();
        s.move_strips(
            b,
            &[ids[2]],
            Place {
                sub_board: Some(unit2),
                index: 0,
            },
        )
        .unwrap();
        s.delete_sub_board(b, unit2).unwrap();
        assert_eq!(sheet_order(&s, b, None), ["3"]);
        s.check().unwrap();
    }

    #[test]
    fn duplicated_board_is_independent() {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let copy = s.duplicate_board(b, "Rain plan").unwrap();
        s.check().unwrap();
        let csb = s.board(copy).unwrap().sub_boards[0].id;
        let cids: Vec<StripId> = s.board(copy).unwrap().sub_boards[0]
            .strips
            .iter()
            .map(|x| x.id)
            .collect();
        s.move_strips(
            copy,
            &[cids[0]],
            Place {
                sub_board: Some(csb),
                index: 3,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1", "2", "3"]);
        assert_eq!(sheet_order(&s, copy, Some(csb)), ["2", "3", "1"]);
        // A sheet added later lands in every board's Boneyard.
        s.add_sheet(Sheet {
            scenes: "4".to_string(),
            ..Sheet::default()
        });
        assert_eq!(sheet_order(&s, copy, None), ["4"]);
        s.check().unwrap();
    }

    #[test]
    fn sheets_are_inserted_in_place_untagged_and_deleted() {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids[..2],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            StripKind::DayBreak,
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        let rain = s.duplicate_board(b, "Rain plan").unwrap();
        // After scene 1 on the sub-board; at the end of the other board's Boneyard.
        let first = s.sheets[0].id;
        let at = s.sheet_place(b, first).unwrap();
        assert_eq!(
            at,
            Place {
                sub_board: Some(sb),
                index: 0
            }
        );
        let new = s
            .add_sheet_at(
                Sheet {
                    scenes: "1A".to_string(),
                    ..Sheet::default()
                },
                b,
                Place {
                    sub_board: Some(sb),
                    index: at.index + 1,
                },
            )
            .unwrap();
        s.check().unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1", "1A", "2", "|"]);
        assert_eq!(sheet_order(&s, rain, None), ["3", "1A"]);
        let order: Vec<String> = s
            .sheets_in_board_order(b)
            .iter()
            .map(|id| s.sheet(*id).unwrap().scenes.clone())
            .collect();
        assert_eq!(order, ["1", "1A", "2", "3"]);
        // A place past the end lands at the end; a missing sub-board changes nothing.
        let before = s.clone();
        let missing = Place {
            sub_board: Some(SubBoardId(999_999)),
            index: 0,
        };
        assert_eq!(
            s.add_sheet_at(Sheet::default(), b, missing),
            Err(Error::NoSuchSubBoard(SubBoardId(999_999)))
        );
        assert_eq!(s, before);

        // Untagging: the element stays in the schedule.
        let ada = s
            .element_named(s.category_named("Cast").unwrap(), "ADA")
            .unwrap();
        s.untag(first, ada).unwrap();
        s.untag(first, ada).unwrap();
        assert!(!s.sheet(first).unwrap().elements.contains(&ada));
        assert!(s.element(ada).is_some());
        assert!(s.untag(SheetId(999_999), ada).is_err());

        // Deleting a sheet takes its strip off every board and keeps the day break.
        s.delete_sheet(new).unwrap();
        s.check().unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1", "2", "|"]);
        assert_eq!(sheet_order(&s, rain, None), ["3"]);
        assert_eq!(s.delete_sheet(new), Err(Error::NoSuchSheet(new)));
        assert_eq!(s.sheet_place(b, new), None);
        assert_eq!(
            s.sheets_in_board_order(BoardId(999_999)),
            Vec::<SheetId>::new()
        );
    }

    fn ids_of(s: &Schedule, cat: CategoryId) -> Vec<(String, Option<String>)> {
        let mut v: Vec<_> = s
            .elements
            .iter()
            .filter(|e| e.category == cat)
            .map(|e| (e.name.clone(), e.board_id.clone()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn categories_are_renamed_reordered_and_deleted_but_built_ins_stay() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        assert_eq!(s.cast(), Some(cast));
        let props = s.add_category("Props");
        let wardrobe = s.add_category("Wardrobe");
        let vehicles = s.add_category("Vehicles");
        // The cast category can be renamed (behavior note 17, A10) and stays the cast.
        s.rename_category(cast, "People").unwrap();
        assert_eq!(s.cast(), Some(cast));
        assert_eq!(s.cast_or_add(), cast);
        assert_eq!(
            s.move_category(cast, 2),
            Err(Error::BuiltIn("People".into()))
        );
        assert_eq!(
            s.delete_category(cast),
            Err(Error::BuiltIn("People".into()))
        );
        // A user category may take the old name; it is not the cast.
        let other = s.add_category("Cast");
        assert_ne!(other, cast);
        assert_eq!(s.cast(), Some(cast));
        s.delete_category(other).unwrap();
        s.rename_category(cast, "Cast").unwrap();
        assert_eq!(
            s.rename_category(props, " WARDROBE "),
            Err(Error::DuplicateCategory("WARDROBE".to_string()))
        );
        assert_eq!(s.rename_category(props, "  "), Err(Error::EmptyName));
        // A change of case of its own name is fine.
        s.rename_category(props, " props ").unwrap();
        s.rename_category(props, "Props").unwrap();
        let names =
            |s: &Schedule| -> Vec<String> { s.categories.iter().map(|c| c.name.clone()).collect() };
        // Places count among user categories; Cast stays first.
        s.move_category(vehicles, 0).unwrap();
        assert_eq!(names(&s), ["Cast", "Vehicles", "Props", "Wardrobe"]);
        s.move_category(vehicles, 99).unwrap();
        assert_eq!(names(&s), ["Cast", "Props", "Wardrobe", "Vehicles"]);
        // Only an empty category goes.
        let lamp = s.add_element(props, "Lamp").unwrap();
        assert_eq!(
            s.delete_category(props),
            Err(Error::NotEmpty("Props".into()))
        );
        s.delete_element(lamp, false).unwrap();
        s.delete_category(props).unwrap();
        s.delete_category(wardrobe).unwrap();
        assert_eq!(names(&s), ["Cast", "Vehicles"]);
        s.check().unwrap();
    }

    #[test]
    fn defaults_apply_to_existing_elements_only_when_asked() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        let rules = DoodRules {
            holds: true,
            drops: true,
            min_drop_gap: 10,
        };
        s.category_mut(cast).unwrap().defaults = rules;
        assert!(s.elements.iter().all(|e| e.rules == DoodRules::default()));
        assert_eq!(s.apply_defaults(cast), Ok(3));
        assert!(s.elements.iter().all(|e| e.rules == rules));
        assert_eq!(s.apply_defaults(cast), Ok(0));
    }

    #[test]
    fn elements_are_renamed_uniquely_ignoring_case() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        assert_eq!(
            s.rename_element(ada, "ben"),
            Err(Error::DuplicateName("ben".into()))
        );
        assert_eq!(s.rename_element(ada, " "), Err(Error::EmptyName));
        s.rename_element(ada, " Ada ").unwrap();
        assert_eq!(s.element(ada).unwrap().name, "Ada");
        // The same name in another category is fine.
        let props = s.add_category("Props");
        let ben = s.add_element(props, "Lamp").unwrap();
        s.rename_element(ben, "BEN").unwrap();
        s.check().unwrap();
    }

    #[test]
    fn merging_keeps_one_entry_per_sheet_and_event() {
        let (mut s, b, sb) = sample();
        let cast = s.category_named("Cast").unwrap();
        let (ada, ben, carl) = (
            s.element_named(cast, "ADA").unwrap(),
            s.element_named(cast, "BEN").unwrap(),
            s.element_named(cast, "CARL").unwrap(),
        );
        let props = s.add_category("Props");
        let lamp = s.add_element(props, "Lamp").unwrap();
        s.element_mut(lamp).unwrap().links = vec![ben];
        s.element_mut(carl).unwrap().links = vec![ben];
        s.boards[0].sub_boards[0].calendar.events.push(Event {
            name: "Fitting".into(),
            kind: EventKind::Fitting,
            from: "2027-03-01".parse().unwrap(),
            to: "2027-03-01".parse().unwrap(),
            elements: vec![ben, ada],
            color: None,
        });
        // Sheet 1 has ADA and BEN; sheet 2 ADA; sheet 3 CARL. Scheduled and locked.
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
        crate::days::auto_day_breaks(&mut s, b, sb, crate::days::Limit::Pages(Eighths(1))).unwrap();
        crate::drift::lock(&mut s, b, "white", None).unwrap();
        assert!(s.merge_elements(ada, &[lamp]).is_err(), "another category");
        s.merge_elements(ada, &[ben, carl, ada]).unwrap();
        // The lock's cast follows: no actor drifted, none dropped.
        let d = crate::drift::drift(&s, "white").unwrap();
        assert_eq!(d.changed().count(), 0, "{d:?}");
        assert_eq!(d.actors.len(), 1);
        s.check().unwrap();
        assert_eq!(s.sheets[0].elements, [ada]);
        assert_eq!(s.sheets[1].elements, [ada]);
        assert_eq!(s.sheets[2].elements, [ada], "CARL's sheet now lists ADA");
        assert_eq!(s.boards[0].sub_boards[0].calendar.events[0].elements, [ada]);
        assert_eq!(ids_of(&s, cast), [("ADA".to_string(), None)]);
        // Links to the merged ones point at the survivor; it doesn't link to itself.
        assert_eq!(s.element(lamp).unwrap().links, [ada]);
        assert!(s.element(ada).unwrap().links.is_empty());
    }

    #[test]
    fn moving_an_element_keeps_its_board_id_or_is_refused() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        let extras = s.add_category("Extras");
        let ada = s.element_named(cast, "ADA").unwrap();
        let ben = s.element_named(cast, "BEN").unwrap();
        s.set_board_id(ada, Some("1")).unwrap();
        s.element_mut(ada).unwrap().board_id_locked = true;
        s.set_board_id(ben, Some("2")).unwrap();
        let taken = s.add_element(extras, "Crowd").unwrap();
        s.set_board_id(taken, Some("2")).unwrap();
        s.move_element(ada, extras).unwrap();
        assert_eq!(s.element(ada).unwrap().category, extras);
        assert_eq!(s.element(ada).unwrap().board_id.as_deref(), Some("1"));
        assert!(s.element(ada).unwrap().board_id_locked);
        let before = s.clone();
        assert_eq!(
            s.move_element(ben, extras),
            Err(Error::DuplicateBoardId("2".into())),
            "2 is taken there"
        );
        assert_eq!(s, before);
        s.set_board_id(ben, None).unwrap();
        s.move_element(ben, extras).unwrap();
        assert_eq!(s.element(ben).unwrap().board_id, None);
        // A category that numbers automatically gives none: the setting is for new
        // elements only (behavior note 27, E-L7).
        s.category_mut(cast).unwrap().auto_board_id = true;
        s.move_element(ben, cast).unwrap();
        assert_eq!(s.element(ben).unwrap().board_id, None);
        assert!(!s.element(ben).unwrap().board_id_locked);
        // The name must be free there.
        s.add_element(extras, "carl").unwrap();
        let carl = s.element_named(cast, "CARL").unwrap();
        assert_eq!(
            s.move_element(carl, extras),
            Err(Error::DuplicateName("CARL".into()))
        );
        // Sheets don't change.
        assert!(s.sheets[0].elements.contains(&ben));
        s.check().unwrap();
    }

    #[test]
    fn renumbering_skips_locked_numbers_and_leaves_letters_alone() {
        let mut s = Schedule::new("ZTEST");
        let cast = s.category_named("Cast").unwrap();
        let mut ids = Vec::new();
        for (name, id, locked) in [
            ("A", Some("7"), false),
            ("B", Some("2"), true),
            ("C", None, false),
            ("D", Some("12A"), false),
            ("E", Some("3"), false),
            ("F", Some("04"), true),
        ] {
            let e = s.add_element(cast, name).unwrap();
            s.set_board_id(e, id).unwrap();
            s.element_mut(e).unwrap().board_id_locked = locked;
            ids.push(e);
        }
        s.renumber(cast, 1, &ids).unwrap();
        s.check().unwrap();
        let got: Vec<Option<&str>> = ids
            .iter()
            .map(|id| s.element(*id).unwrap().board_id.as_deref())
            .collect();
        // A 1, B locked 2, C 3, D keeps 12A, E 5 (4 is F's, locked as 04), F keeps 04.
        assert_eq!(
            got,
            [
                Some("1"),
                Some("2"),
                Some("3"),
                Some("12A"),
                Some("5"),
                Some("04")
            ]
        );
        // From another start, in another order.
        let order = [ids[4], ids[2], ids[0], ids[1], ids[3], ids[5]];
        s.renumber(cast, 10, &order).unwrap();
        let e = |i: usize| s.element(ids[i]).unwrap().board_id.clone().unwrap();
        assert_eq!((e(4), e(2), e(0)), ("10".into(), "11".into(), "12".into()));
        // The order must list the category exactly.
        assert!(s.renumber(cast, 1, &ids[..3]).is_err());
        assert!(s.renumber(cast, 1, &[ids[0]; 6]).is_err());
    }

    #[test]
    fn deleting_an_element_takes_it_off_sheets_and_events_unless_linked() {
        let (mut s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        let ben = s.element_named(cast, "BEN").unwrap();
        s.boards[0].sub_boards[0].calendar.events.push(Event {
            name: String::new(),
            kind: EventKind::Travel,
            from: "2027-03-01".parse().unwrap(),
            to: "2027-03-02".parse().unwrap(),
            elements: vec![ada, ben],
            color: None,
        });
        s.element_mut(ben).unwrap().links = vec![ada];
        assert_eq!(
            s.delete_element(ada, false),
            Err(Error::Linked("ADA".into()))
        );
        assert!(s.is_linked(ada) && s.is_linked(ben));
        s.delete_element(ada, true).unwrap();
        s.check().unwrap();
        assert!(s.element(ada).is_none());
        assert!(s.sheets.iter().all(|h| !h.elements.contains(&ada)));
        assert_eq!(s.boards[0].sub_boards[0].calendar.events[0].elements, [ben]);
        assert!(s.element(ben).unwrap().links.is_empty());
        assert!(!s.is_linked(ben));
        s.delete_element(ben, false).unwrap();
        assert_eq!(s.delete_element(ben, false), Err(Error::NoSuchElement(ben)));
        s.check().unwrap();
    }

    #[test]
    fn check_refuses_dangling_and_doubled_references() {
        let (s, _, _) = sample();
        let cast = s.category_named("Cast").unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        let mut t = s.clone();
        t.sheets[1].elements.push(ada);
        assert!(t.check().is_err(), "an element twice on a sheet");
        let mut t = s.clone();
        let ev = Event {
            name: String::new(),
            kind: EventKind::Fitting,
            from: "2027-03-02".parse().unwrap(),
            to: "2027-03-01".parse().unwrap(),
            elements: vec![ada],
            color: None,
        };
        t.boards[0].sub_boards[0].calendar.events.push(ev.clone());
        assert!(t.check().is_err(), "ends before it starts");
        let mut t = s.clone();
        t.boards[0].sub_boards[0].calendar.events.push(Event {
            to: ev.from,
            elements: vec![ElementId(999_999)],
            ..ev.clone()
        });
        assert!(t.check().is_err(), "a missing element");
        let mut t = s.clone();
        t.categories.push(Category {
            id: CategoryId(t.next_id),
            name: "CAST".into(),
            auto_board_id: false,
            exclude_from_board: false,
            defaults: DoodRules::default(),
            cast: false,
            daily_quantity: false,
            properties: Vec::new(),
            multi_line: Vec::new(),
        });
        t.next_id += 1;
        assert!(t.check().is_err(), "two categories of one name");
        let mut t = s.clone();
        let props = t.add_category("Props");
        t.category_mut(props).unwrap().cast = true;
        assert!(t.check().is_err(), "two cast categories");
    }

    #[test]
    fn property_random_element_edits_keep_the_schedule_whole() {
        let mut rng = Rng::new(4242);
        for round in 0..120 {
            let mut s = Schedule::new("ZTEST");
            let mut cats = vec![s.category_named("Cast").unwrap()];
            for name in ["Props", "Extras", "Vehicles"] {
                if rng.below(2) == 0 {
                    cats.push(s.add_category(name));
                }
            }
            for i in 0..(rng.below(8) + 2) {
                let cat = cats[rng.below(cats.len() as u64) as usize];
                let e = s.add_element(cat, &format!("E{i}")).unwrap();
                if rng.below(3) == 0 {
                    let _ = s.set_board_id(e, Some(&(rng.below(9) + 1).to_string()));
                }
                s.element_mut(e).unwrap().board_id_locked = rng.below(4) == 0;
            }
            for i in 0..(rng.below(6) + 1) {
                let els: Vec<ElementId> = s
                    .elements
                    .iter()
                    .map(|e| e.id)
                    .filter(|_| rng.below(2) == 0)
                    .collect();
                s.add_sheet(Sheet {
                    scenes: i.to_string(),
                    elements: els,
                    ..Sheet::default()
                });
            }
            let first = s.elements[0].id;
            s.boards[0].sub_boards[0].calendar.events.push(Event {
                name: "e".into(),
                kind: EventKind::Rehearsal,
                from: "2027-03-01".parse().unwrap(),
                to: "2027-03-03".parse().unwrap(),
                elements: vec![first],
                color: None,
            });
            s.check().unwrap();
            for step in 0..30 {
                if s.elements.is_empty() {
                    break;
                }
                let pick = |rng: &mut Rng, s: &Schedule| {
                    s.elements[rng.below(s.elements.len() as u64) as usize].id
                };
                let before = s.clone();
                let r = match rng.below(12) {
                    0 => {
                        let survivor = pick(&mut rng, &s);
                        let cat = s.element(survivor).unwrap().category;
                        let others: Vec<ElementId> = s
                            .elements
                            .iter()
                            .filter(|e| e.category == cat && rng.below(2) == 0)
                            .map(|e| e.id)
                            .collect();
                        let had: Vec<bool> = s
                            .sheets
                            .iter()
                            .map(|h| {
                                h.elements
                                    .iter()
                                    .any(|e| *e == survivor || others.contains(e))
                            })
                            .collect();
                        let r = s.merge_elements(survivor, &others);
                        if r.is_ok() {
                            for (h, had) in s.sheets.iter().zip(had) {
                                assert_eq!(h.elements.contains(&survivor), had);
                                assert!(!others
                                    .iter()
                                    .any(|o| *o != survivor && h.elements.contains(o)));
                            }
                        }
                        r
                    }
                    1 => {
                        let e = pick(&mut rng, &s);
                        let to = cats[rng.below(cats.len() as u64) as usize];
                        s.move_element(e, to)
                    }
                    2 => {
                        let cat = cats[rng.below(cats.len() as u64) as usize];
                        let order: Vec<ElementId> = s
                            .elements
                            .iter()
                            .filter(|e| e.category == cat)
                            .map(|e| e.id)
                            .collect();
                        let locked: Vec<(ElementId, Option<String>)> = s
                            .elements
                            .iter()
                            .filter(|e| e.category == cat && e.board_id_locked)
                            .map(|e| (e.id, e.board_id.clone()))
                            .collect();
                        let r = s.renumber(cat, rng.below(5) as u32 + 1, &order);
                        for (id, b) in locked {
                            assert_eq!(s.element(id).unwrap().board_id, b, "locked kept");
                        }
                        r
                    }
                    3 => {
                        let e = pick(&mut rng, &s);
                        let r = s.delete_element(e, rng.below(2) == 0);
                        if r.is_ok() {
                            assert!(s.sheets.iter().all(|h| !h.elements.contains(&e)));
                        }
                        r
                    }
                    4 => {
                        let e = pick(&mut rng, &s);
                        let other = pick(&mut rng, &s);
                        let mut links = s.element(e).unwrap().links.clone();
                        links.push(other);
                        s.set_links(e, &links, rng.below(2) == 0).map(|_| ())
                    }
                    8 => {
                        let cat = cats[rng.below(cats.len() as u64) as usize];
                        let name = format!("p{}", rng.below(3));
                        let _ = s.add_property(cat, &name);
                        match s.elements.iter().find(|e| e.category == cat) {
                            Some(e) => s.set_property_value(e.id, &name, "x"),
                            None => Ok(()),
                        }
                    }
                    9 => {
                        let h = rng.below(s.sheets.len() as u64) as usize;
                        let Some(&e) = s.sheets[h].elements.first() else {
                            continue;
                        };
                        let id = s.sheets[h].id;
                        s.set_quantity(id, e, rng.below(3) as u32 + 1, rng.below(2) == 0)
                    }
                    10 => {
                        let h = s.sheets[rng.below(s.sheets.len() as u64) as usize].id;
                        let e = pick(&mut rng, &s);
                        s.tag_with_links(h, e).map(|_| ())
                    }
                    11 => {
                        let h = s.sheets[rng.below(s.sheets.len() as u64) as usize].id;
                        let e = pick(&mut rng, &s);
                        s.untag(h, e)
                    }
                    5 => {
                        let e = pick(&mut rng, &s);
                        s.rename_element(e, &format!("e{}", rng.below(12)))
                    }
                    6 => {
                        let cat = cats[rng.below(cats.len() as u64) as usize];
                        s.move_category(cat, rng.below(4) as usize)
                    }
                    _ => {
                        let cat = cats[rng.below(cats.len() as u64) as usize];
                        let r = s.delete_category(cat);
                        if r.is_ok() {
                            cats.retain(|c| *c != cat);
                        }
                        r
                    }
                };
                if r.is_err() {
                    assert_eq!(s, before, "round {round} step {step}: {r:?}");
                }
                s.check()
                    .unwrap_or_else(|e| panic!("round {round} step {step}: {e}"));
                assert!(s.categories[0].cast);
                assert_eq!(s.cast(), Some(s.categories[0].id));
            }
        }
    }

    #[test]
    fn a_date_is_one_kind_and_days_reflow() {
        let mut cal = Calendar::default();
        let d = |x: &str| x.parse::<Date>().unwrap();
        let start = d("2027-03-01"); // Monday
        assert_eq!(cal.date_of_day(start, 6), Some(d("2027-03-08")));
        assert_eq!(cal.day_kind(d("2027-03-06")), DayKind::Off, "Saturday");
        // A holiday on Wednesday pushes day 3 to Thursday.
        cal.set_day_kind(d("2027-03-03"), DayKind::Holiday);
        assert_eq!(cal.date_of_day(start, 3), Some(d("2027-03-04")));
        // Saturday back to working: day 5 is Saturday.
        cal.set_day_kind(d("2027-03-06"), DayKind::Work);
        assert_eq!(cal.work_days, [d("2027-03-06")]);
        assert!(cal.is_shoot_day(d("2027-03-06")));
        assert_eq!(cal.date_of_day(start, 5), Some(d("2027-03-06")));
        // The holiday becomes company travel: one list only, never both.
        cal.set_day_kind(d("2027-03-03"), DayKind::Move);
        assert!(cal.holidays.is_empty());
        assert_eq!(cal.day_kind(d("2027-03-03")), DayKind::Move);
        // A single day off on a weekday; working again clears it.
        cal.set_day_kind(d("2027-03-02"), DayKind::Off);
        assert_eq!(cal.off_days, [d("2027-03-02")]);
        cal.set_day_kind(d("2027-03-02"), DayKind::Work);
        assert!(cal.off_days.is_empty() && cal.work_days.len() == 1);
        // Off on a weekly day off needs no mark.
        cal.set_day_kind(d("2027-03-06"), DayKind::Off);
        assert!(cal.work_days.is_empty() && cal.off_days.is_empty());
        // Marks stay sorted.
        for x in ["2027-04-02", "2027-03-30", "2027-04-01"] {
            cal.set_day_kind(d(x), DayKind::Holiday);
        }
        assert_eq!(
            cal.holidays,
            [d("2027-03-30"), d("2027-04-01"), d("2027-04-02")]
        );
    }

    /// Walking on from the last day gives the same dates as counting from the start, in any
    /// order of asking, across holidays, travel, a day off and a Saturday worked
    /// (decision 190).
    #[test]
    fn day_dates_match_date_of_day() {
        let mut cal = Calendar::default();
        let d = |x: &str| x.parse::<Date>().unwrap();
        let start = d("2027-03-01");
        cal.set_day_kind(d("2027-03-03"), DayKind::Holiday);
        cal.set_day_kind(d("2027-03-10"), DayKind::Move);
        cal.set_day_kind(d("2027-03-15"), DayKind::Off);
        cal.set_day_kind(d("2027-03-13"), DayKind::Work);
        let mut dates = cal.day_dates(start);
        for n in [0, 1, 2, 2, 3, 7, 30, 4, 31, 0, 31, 60, 1, 61] {
            assert_eq!(dates.date(n), cal.date_of_day(start, n), "day {n}");
        }
        cal.set_weekly_off(&Weekday::ALL[..6]).unwrap();
        let mut dates = cal.day_dates(start);
        for n in 1..20 {
            assert_eq!(dates.date(n), cal.date_of_day(start, n), "day {n}");
        }
    }

    /// The duplicate checks found by sets name the same element as the pairwise ones.
    #[test]
    fn check_finds_repeated_elements_and_lists() {
        let mut s = Schedule::new("t");
        let cast = s.cast().unwrap();
        let a = s.add_element(cast, "ANNA").unwrap();
        let b = s.add_element(cast, "BEN").unwrap();
        s.check().unwrap();
        let mut x = s.clone();
        x.element_mut(b).unwrap().name = " anna ".to_string();
        assert!(x
            .check()
            .unwrap_err()
            .to_string()
            .contains("element \" anna \" appears twice in a category"));
        let mut x = s.clone();
        x.element_mut(a).unwrap().board_id = Some("1".to_string());
        x.element_mut(b).unwrap().board_id = Some("1".to_string());
        assert!(x.check().unwrap_err().to_string().contains("board ID"));
        let mut x = s.clone();
        x.element_mut(a).unwrap().links = vec![b, b];
        assert!(x
            .check()
            .unwrap_err()
            .to_string()
            .contains("one element twice"));
        assert!(repeats(&[1, 2, 3, 1]));
        assert!(!repeats(&(0..40).collect::<Vec<u32>>()));
        assert!(repeats(&(0..40).chain([39]).collect::<Vec<u32>>()));
    }

    #[test]
    fn the_working_week_keeps_one_working_day() {
        let mut cal = Calendar::default();
        cal.set_weekly_off(&[Weekday::Sun, Weekday::Sat, Weekday::Sun])
            .unwrap();
        assert_eq!(cal.weekly_off, [Weekday::Sat, Weekday::Sun]);
        cal.set_weekly_off(&[Weekday::Sun]).unwrap();
        let d = |x: &str| x.parse::<Date>().unwrap();
        assert_eq!(
            cal.date_of_day(d("2027-03-01"), 6),
            Some(d("2027-03-06")),
            "a six-day week"
        );
        assert!(cal.set_weekly_off(&Weekday::ALL).is_err());
        assert_eq!(cal.weekly_off, [Weekday::Sun]);
        cal.set_weekly_off(&[]).unwrap();
        assert!(cal.weekly_off.is_empty());
    }

    #[test]
    fn events_are_added_replaced_and_deleted() {
        let (mut s, b, sb) = sample();
        let cast = s.category_named("Cast").unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        let day = |x: &str| x.parse::<Date>().unwrap();
        let ev = Event {
            name: "Fitting".into(),
            kind: EventKind::Fitting,
            from: day("2027-03-01"),
            to: day("2027-03-02"),
            elements: vec![ada, ada],
            color: None,
        };
        assert_eq!(s.set_event(b, sb, None, ev.clone()), Ok(0));
        assert_eq!(s.boards[0].sub_boards[0].calendar.events[0].elements, [ada]);
        let other = Event {
            kind: EventKind::Other("Camera test".into()),
            ..ev.clone()
        };
        assert_eq!(s.set_event(b, sb, Some(0), other.clone()), Ok(0));
        assert_eq!(
            s.boards[0].sub_boards[0].calendar.events[0].kind,
            EventKind::Other("Camera test".into())
        );
        let before = s.clone();
        let backwards = Event {
            from: day("2027-03-03"),
            ..ev.clone()
        };
        assert!(s.set_event(b, sb, None, backwards).is_err());
        // A leap year is the longest event; a day more is a slip of the year.
        let year = Event {
            from: day("2028-01-01"),
            to: day("2028-12-31"),
            ..ev.clone()
        };
        let mut long = s.clone();
        assert_eq!(long.set_event(b, sb, None, year.clone()), Ok(1));
        let too_long = Event {
            to: day("2029-01-01"),
            ..year
        };
        assert!(s.set_event(b, sb, None, too_long).is_err());
        let missing = Event {
            elements: vec![ElementId(999_999)],
            ..ev.clone()
        };
        assert_eq!(
            s.set_event(b, sb, None, missing),
            Err(Error::NoSuchElement(ElementId(999_999)))
        );
        assert!(s.set_event(b, sb, Some(5), ev.clone()).is_err());
        assert!(s
            .set_event(b, SubBoardId(999_999), None, ev.clone())
            .is_err());
        assert!(s.delete_event(b, sb, 1).is_err());
        assert_eq!(s, before);
        s.delete_event(b, sb, 0).unwrap();
        assert!(s.boards[0].sub_boards[0].calendar.events.is_empty());
        s.check().unwrap();
        // A nameless event is called by its kind.
        let nameless = Event {
            name: "  ".into(),
            ..ev.clone()
        };
        assert_eq!(nameless.title(), "Fitting");
        assert_eq!(
            Event {
                kind: EventKind::CameraTest,
                ..nameless
            }
            .title(),
            "Camera test"
        );
        assert_eq!(
            Event {
                name: " Wardrobe ".into(),
                ..ev
            }
            .title(),
            "Wardrobe"
        );
    }

    #[test]
    fn property_calendar_marks_keep_one_kind_per_date_and_days_in_order() {
        let mut rng = Rng::new(31337);
        let start: Date = "2027-03-01".parse().unwrap();
        let kinds = [DayKind::Work, DayKind::Off, DayKind::Holiday, DayKind::Move];
        for _ in 0..200 {
            let mut cal = Calendar::default();
            let mut want = std::collections::HashMap::new();
            for _ in 0..40 {
                if rng.below(8) == 0 {
                    let off: Vec<Weekday> = Weekday::ALL
                        .iter()
                        .copied()
                        .filter(|_| rng.below(3) == 0)
                        .collect();
                    let _ = cal.set_weekly_off(&off);
                    // Kinds set on dates follow the week: only explicit marks survive.
                    want.clear();
                    for d in cal
                        .work_days
                        .iter()
                        .chain(&cal.off_days)
                        .chain(&cal.holidays)
                        .chain(&cal.moves)
                    {
                        want.insert(*d, cal.day_kind(*d));
                    }
                    continue;
                }
                let d = start.add_days(rng.below(60) as i32);
                let k = kinds[rng.below(4) as usize];
                cal.set_day_kind(d, k);
                want.insert(d, k);
                for (d, k) in &want {
                    assert_eq!(cal.day_kind(*d), *k, "{d}");
                }
                // Each date in one list at most, every list sorted.
                let mut all = Vec::new();
                for list in [&cal.work_days, &cal.off_days, &cal.holidays, &cal.moves] {
                    assert!(list.windows(2).all(|w| w[0] < w[1]));
                    all.extend(list.iter().copied());
                }
                let n = all.len();
                all.sort();
                all.dedup();
                assert_eq!(all.len(), n, "a date in two lists");
                // `check` agrees: the edits never leave a calendar it refuses.
                assert_eq!(cal.problem(), None);
                // Shoot days are increasing working dates.
                let mut prev = None;
                for i in 1..=20 {
                    let Some(day) = cal.date_of_day(start, i) else {
                        break;
                    };
                    assert_eq!(cal.day_kind(day), DayKind::Work);
                    assert!(prev.is_none_or(|p| p < day));
                    prev = Some(day);
                }
            }
        }
    }

    /// A calendar `check` refuses: a date of two kinds, a date twice, marks out of order,
    /// or a week with no working day (decision 21; only a hand-edited file can hold one).
    #[test]
    fn check_refuses_a_calendar_with_a_date_of_two_kinds() {
        let d = |x: &str| -> Date { x.parse().unwrap() };
        let good = Schedule::new("ZTEST Calendar");
        let broken = |change: &dyn Fn(&mut Calendar)| {
            let mut s = good.clone();
            change(&mut s.boards[0].sub_boards[0].calendar);
            s.check().unwrap_err().to_string()
        };
        let e = broken(&|c| {
            c.holidays.push(d("2027-03-02"));
            c.moves.push(d("2027-03-02"));
        });
        assert!(
            e.contains("2027-03-02 is in both its holidays and its company travel days"),
            "{e}"
        );
        let e = broken(&|c| c.off_days = vec![d("2027-03-02"), d("2027-03-02")]);
        assert!(
            e.contains("days off are out of order or list a date twice"),
            "{e}"
        );
        let e = broken(&|c| c.holidays = vec![d("2027-03-09"), d("2027-03-02")]);
        assert!(e.contains("holidays are out of order"), "{e}");
        let e = broken(&|c| c.weekly_off = Weekday::ALL.to_vec());
        assert!(e.contains("at least one working day"), "{e}");
        let e = broken(&|c| c.weekly_off = vec![Weekday::Sun, Weekday::Sat]);
        assert!(e.contains("weekly days off are out of order"), "{e}");
        // A back to working mark on a weekday that is no longer a weekly day off stays
        // valid: set_weekly_off keeps marks already made.
        let mut s = good.clone();
        let cal = &mut s.boards[0].sub_boards[0].calendar;
        cal.set_day_kind(d("2027-03-06"), DayKind::Work);
        cal.set_weekly_off(&[Weekday::Sun]).unwrap();
        s.check().unwrap();
    }

    #[test]
    fn property_random_sheet_inserts_and_deletes_keep_every_sheet_once() {
        let mut rng = Rng::new(1999);
        for _ in 0..150 {
            let mut s = Schedule::new("ZTEST");
            let b = s.boards[0].id;
            let cast = s.category_named("Cast").unwrap();
            let el = s.add_element(cast, "ADA").unwrap();
            let mut subs = vec![None, Some(s.boards[0].sub_boards[0].id)];
            for _ in 0..rng.below(3) {
                subs.push(Some(s.add_sub_board(b, "Unit", None).unwrap()));
            }
            if rng.below(2) == 0 {
                s.duplicate_board(b, "Copy").unwrap();
            }
            for step in 0..40 {
                let sheets = s.sheets_in_board_order(b);
                match rng.below(5) {
                    0 | 1 => {
                        let to = subs[rng.below(subs.len() as u64) as usize];
                        let index = rng.below(12) as usize;
                        let id = s
                            .add_sheet_at(
                                Sheet {
                                    scenes: step.to_string(),
                                    elements: vec![el],
                                    ..Sheet::default()
                                },
                                b,
                                Place {
                                    sub_board: to,
                                    index,
                                },
                            )
                            .unwrap();
                        let at = s.sheet_place(b, id).unwrap();
                        assert_eq!(at.sub_board, to);
                    }
                    2 if !sheets.is_empty() => {
                        let gone = sheets[rng.below(sheets.len() as u64) as usize];
                        s.delete_sheet(gone).unwrap();
                        assert!(s.sheet(gone).is_none());
                        assert!(s.boards.iter().all(|x| s.sheet_place(x.id, gone).is_none()));
                    }
                    3 if !sheets.is_empty() => {
                        let id = sheets[rng.below(sheets.len() as u64) as usize];
                        s.untag(id, el).unwrap();
                    }
                    _ => {
                        let to = subs[rng.below(subs.len() as u64) as usize];
                        s.insert_strip(
                            b,
                            StripKind::DayBreak,
                            Place {
                                sub_board: to.or(subs[1]),
                                index: rng.below(12) as usize,
                            },
                        )
                        .unwrap();
                    }
                }
                s.check().unwrap();
                // Board order lists every sheet exactly once.
                let mut order = s.sheets_in_board_order(b);
                order.sort();
                let mut all: Vec<SheetId> = s.sheets.iter().map(|x| x.id).collect();
                all.sort();
                assert_eq!(order, all);
            }
        }
    }

    #[test]
    fn property_random_moves_keep_every_sheet_once() {
        let mut rng = Rng::new(2027);
        for _ in 0..200 {
            let mut s = Schedule::new("ZTEST");
            let b = s.boards[0].id;
            for i in 0..(rng.below(12) + 1) {
                s.add_sheet(Sheet {
                    scenes: i.to_string(),
                    ..Sheet::default()
                });
            }
            let mut subs = vec![None, Some(s.boards[0].sub_boards[0].id)];
            for _ in 0..rng.below(3) {
                subs.push(Some(s.add_sub_board(b, "Unit", None).unwrap()));
            }
            for _ in 0..40 {
                let board = s.board(b).unwrap();
                let all: Vec<StripId> = board
                    .boneyard
                    .iter()
                    .chain(board.sub_boards.iter().flat_map(|x| &x.strips))
                    .map(|x| x.id)
                    .collect();
                let to = subs[rng.below(subs.len() as u64) as usize];
                match rng.below(4) {
                    0 => {
                        let kind = if rng.below(2) == 0 {
                            StripKind::DayBreak
                        } else {
                            StripKind::Banner(Banner {
                                text: "b".to_string(),
                                duration: Minutes(5),
                                ..Default::default()
                            })
                        };
                        let before = s.clone();
                        let banner = matches!(kind, StripKind::Banner(_));
                        let r = s.insert_strip(
                            b,
                            kind,
                            Place {
                                sub_board: to,
                                index: rng.below(20) as usize,
                            },
                        );
                        // Only the Boneyard refuses, a banner (day breaks park there,
                        // decision 57), and a refusal changes nothing.
                        assert_eq!(r.is_err(), to.is_none() && banner);
                        if r.is_err() {
                            assert_eq!(s, before);
                        }
                    }
                    _ if !all.is_empty() => {
                        let pick: Vec<StripId> =
                            all.iter().copied().filter(|_| rng.below(3) == 0).collect();
                        let before = s.clone();
                        let r = s.move_strips(
                            b,
                            &pick,
                            Place {
                                sub_board: to,
                                index: rng.below(20) as usize,
                            },
                        );
                        if r.is_err() {
                            assert!(to.is_none(), "{r:?}");
                            assert_eq!(s, before);
                        }
                    }
                    _ => {}
                }
                s.check().unwrap();
            }
        }
    }

    #[test]
    fn moved_strips_keep_board_order_sub_boards_before_the_boneyard() {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &[ids[2]],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        // Scene 3 is on Main, scenes 1 and 2 in the Boneyard: board order is 3, 1, 2.
        s.move_strips(
            b,
            &[ids[0], ids[2], ids[1]],
            Place {
                sub_board: Some(sb),
                index: 1,
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["3", "1", "2"]);
        s.check().unwrap();
    }

    #[test]
    fn boards_and_sub_boards_are_renamed_moved_and_deleted() {
        let (mut s, b, sb) = sample();
        let rain = s.duplicate_board(b, "Rain plan").unwrap();
        assert_eq!(
            s.board_name(" rain PLAN ", None),
            Err(Error::Broken(
                "there is already a board \"rain PLAN\"".into()
            ))
        );
        assert_eq!(s.board_name("  ", None), Err(Error::EmptyName));
        s.rename_board(rain, " Wet weather ").unwrap();
        assert_eq!(s.board(rain).unwrap().name, "Wet weather");
        // Renaming to its own name in another case is fine.
        s.rename_board(rain, "WET weather").unwrap();
        assert!(s.rename_board(rain, "board 1").is_err());
        s.move_board(rain, 0).unwrap();
        assert_eq!(s.boards[0].id, rain);
        s.move_board(rain, 99).unwrap();
        assert_eq!(s.boards[1].id, rain);

        let unit = s.add_sub_board(b, "Unit B", None).unwrap();
        assert!(s.sub_board_name(b, "unit b", None).is_err());
        assert!(s.sub_board_name(rain, "Unit B", None).is_ok());
        assert!(s.rename_sub_board(b, unit, "main").is_err());
        s.rename_sub_board(b, unit, "Second unit").unwrap();
        s.move_sub_board(b, unit, 0).unwrap();
        let names: Vec<_> = s
            .board(b)
            .unwrap()
            .sub_boards
            .iter()
            .map(|x| x.name.clone())
            .collect();
        assert_eq!(names, ["Second unit", "Main"]);
        assert_eq!(s.board_of_sub(unit), Some(b));
        assert_eq!(s.board_of_sub(sb), Some(b));
        s.set_start(b, unit, "2027-03-08".parse().ok()).unwrap();
        assert!(s.set_start(rain, unit, None).is_err());

        s.delete_board(rain).unwrap();
        assert_eq!(s.boards.len(), 1);
        assert!(s.delete_board(b).is_err());
        assert_eq!(s.sheets.len(), 3);
        s.check().unwrap();
    }

    #[test]
    fn banners_are_edited_in_place() {
        let (mut s, b, sb) = sample();
        let id = s
            .insert_strip(
                b,
                StripKind::Banner(Banner {
                    text: "Lunch".into(),
                    duration: Minutes(60),
                    ..Default::default()
                }),
                Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .unwrap();
        s.set_banner(
            b,
            id,
            Banner {
                text: "Company move".into(),
                duration: Minutes(45),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(sheet_order(&s, b, Some(sb)), ["[Company move]"]);
        let sheet_strip = s.board(b).unwrap().boneyard[0].id;
        let banner = Banner {
            text: "x".into(),
            duration: Minutes(0),
            ..Default::default()
        };
        assert!(s.set_banner(b, sheet_strip, banner.clone()).is_err());
        assert_eq!(
            s.set_banner(b, StripId(9999), banner),
            Err(Error::NoSuchStrip(StripId(9999)))
        );
    }

    #[test]
    fn scene_numbers_sort_naturally() {
        use std::cmp::Ordering;
        let mut v = vec!["10A", "A1", "9", "1A", "10", "2", "1", "10b", "A12", "11"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            ["1", "1A", "2", "9", "10", "10A", "10b", "11", "A1", "A12"]
        );
        assert_eq!(natural_cmp("007", "7"), Ordering::Equal);
        assert_eq!(natural_cmp("12, 15", "12"), Ordering::Greater);
        assert_eq!(natural_cmp("Day 2", "day 10"), Ordering::Less);
        assert_eq!(natural_cmp("", "1"), Ordering::Less);
        // Digits past what a u64 holds still compare by value.
        assert_eq!(
            natural_cmp("123456789012345678901234567890", "99"),
            Ordering::Greater
        );
    }

    #[test]
    fn sorting_puts_banners_first_drops_day_breaks_and_orders_sheets() {
        let mut s = Schedule::new("ZTEST");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        for (scene, set, eighths) in [
            ("10", "BARN", 8),
            ("2", "", 3),
            ("1A", "ATTIC", 4),
            ("1", "BARN", 4),
        ] {
            let id = s.add_sheet(Sheet {
                scenes: scene.into(),
                set: set.into(),
                pages: Eighths(eighths),
                ..Sheet::default()
            });
            let strip = s
                .board(b)
                .unwrap()
                .boneyard
                .iter()
                .find(|x| x.kind == StripKind::Sheet(id))
                .unwrap()
                .id;
            let len = s.board(b).unwrap().sub_boards[0].strips.len();
            s.move_strips(
                b,
                &[strip],
                Place {
                    sub_board: Some(sb),
                    index: len,
                },
            )
            .unwrap();
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sb),
                    index: usize::MAX,
                },
            )
            .unwrap();
        }
        for (i, text) in [(2, "Lunch"), (6, "Move")] {
            s.insert_strip(
                b,
                StripKind::Banner(Banner {
                    text: text.into(),
                    duration: Minutes(30),
                    ..Default::default()
                }),
                Place {
                    sub_board: Some(sb),
                    index: i,
                },
            )
            .unwrap();
        }
        let key = |field, descending| SortKey { field, descending };
        s.sort_strips(b, Some(sb), &[key(SortField::Scene, false)])
            .unwrap();
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["[Lunch]", "[Move]", "1", "1A", "2", "10"]
        );
        // Set, then pages descending: the sheet with no set comes last.
        s.sort_strips(
            b,
            Some(sb),
            &[key(SortField::Set, false), key(SortField::Pages, true)],
        )
        .unwrap();
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["[Lunch]", "[Move]", "1A", "10", "1", "2"]
        );
        // Descending still puts the empty set last.
        s.sort_strips(b, Some(sb), &[key(SortField::Set, true)])
            .unwrap();
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["[Lunch]", "[Move]", "10", "1", "1A", "2"]
        );
        // No keys: banners up, breaks out, order kept.
        s.sort_strips(b, Some(sb), &[]).unwrap();
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["[Lunch]", "[Move]", "10", "1", "1A", "2"]
        );
        assert!(s.sort_strips(b, Some(SubBoardId(9999)), &[]).is_err());
        s.check().unwrap();
        assert_eq!(
            SortField::from_name("script_day"),
            Some(SortField::ScriptDay)
        );
        assert_eq!(SortField::from_name("cast"), None);
    }

    #[test]
    fn property_random_sorts_keep_every_sheet_once_and_order_by_the_keys() {
        let mut rng = Rng::new(4242);
        const WORDS: [&str; 7] = ["", "1", "2", "10", "10A", "A", "b"];
        for _ in 0..150 {
            let mut s = Schedule::new("ZTEST");
            let b = s.boards[0].id;
            let sb = s.boards[0].sub_boards[0].id;
            let other = s.add_sub_board(b, "Unit B", None).unwrap();
            for _ in 0..(rng.below(14) + 1) {
                let mut w = || WORDS[rng.below(WORDS.len() as u64) as usize].to_string();
                let sheet = Sheet {
                    scenes: w(),
                    int_ext: w(),
                    set: w(),
                    time_of_day: w(),
                    location: w(),
                    script_day: w(),
                    unit: w(),
                    sequence: w(),
                    episode: w(),
                    ..Sheet::default()
                };
                let pages = Eighths(rng.below(20) as u32);
                s.add_sheet(Sheet { pages, ..sheet });
            }
            // Scatter the strips over both sub-boards and the Boneyard, with breaks and
            // banners on the sub-boards.
            let all: Vec<StripId> = s.board(b).unwrap().boneyard.iter().map(|x| x.id).collect();
            for id in all {
                let to = match rng.below(3) {
                    0 => None,
                    1 => Some(sb),
                    _ => Some(other),
                };
                s.move_strips(
                    b,
                    &[id],
                    Place {
                        sub_board: to,
                        index: rng.below(20) as usize,
                    },
                )
                .unwrap();
            }
            for _ in 0..rng.below(6) {
                let to = if rng.below(2) == 0 { sb } else { other };
                let kind = if rng.below(2) == 0 {
                    StripKind::DayBreak
                } else {
                    StripKind::Banner(Banner {
                        text: "b".into(),
                        duration: Minutes(5),
                        ..Default::default()
                    })
                };
                s.insert_strip(
                    b,
                    kind,
                    Place {
                        sub_board: Some(to),
                        index: rng.below(20) as usize,
                    },
                )
                .unwrap();
            }
            let keys: Vec<SortKey> = (0..rng.below(4))
                .map(|_| SortKey {
                    field: SortField::ALL[rng.below(10) as usize],
                    descending: rng.below(2) == 0,
                })
                .collect();
            let target = match rng.below(3) {
                0 => None,
                1 => Some(sb),
                _ => Some(other),
            };
            let before = s.clone();
            let list_of = |s: &Schedule, sub: Option<SubBoardId>| -> Vec<Strip> {
                let board = s.board(b).unwrap();
                match sub {
                    None => board.boneyard.clone(),
                    Some(id) => board
                        .sub_boards
                        .iter()
                        .find(|x| x.id == id)
                        .unwrap()
                        .strips
                        .clone(),
                }
            };
            s.sort_strips(b, target, &keys).unwrap();
            s.check().unwrap();
            let old = list_of(&before, target);
            let new = list_of(&s, target);
            // The other lists are untouched.
            for sub in [None, Some(sb), Some(other)] {
                if sub != target {
                    assert_eq!(list_of(&s, sub), list_of(&before, sub));
                }
            }
            // No day breaks; banners first, in their old order; then the same sheets.
            assert!(new.iter().all(|x| x.kind != StripKind::DayBreak));
            let banners: Vec<_> = old
                .iter()
                .filter(|x| matches!(x.kind, StripKind::Banner(_)))
                .cloned()
                .collect();
            assert_eq!(&new[..banners.len()], &banners[..]);
            let mut old_sheets: Vec<StripId> = old
                .iter()
                .filter(|x| matches!(x.kind, StripKind::Sheet(_)))
                .map(|x| x.id)
                .collect();
            let rest = &new[banners.len()..];
            assert!(rest.iter().all(|x| matches!(x.kind, StripKind::Sheet(_))));
            let mut new_sheets: Vec<StripId> = rest.iter().map(|x| x.id).collect();
            // In key order, and ties keep their old order.
            let sheet = |st: &Strip| match st.kind {
                StripKind::Sheet(id) => s.sheet(id).unwrap(),
                _ => unreachable!(),
            };
            let pos = |id: StripId| old.iter().position(|x| x.id == id).unwrap();
            for w in rest.windows(2) {
                let o = keys
                    .iter()
                    .map(|k| k.compare(sheet(&w[0]), sheet(&w[1])))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal);
                assert_ne!(o, std::cmp::Ordering::Greater, "{keys:?}");
                if o.is_eq() {
                    assert!(pos(w[0].id) < pos(w[1].id));
                }
            }
            old_sheets.sort();
            new_sheets.sort();
            assert_eq!(old_sheets, new_sheets);
        }
    }

    /// The sample on Main as `1 | 2 | 3 |`, with lengths, times and text set, and a copy
    /// of the board where everything is in the Boneyard.
    fn merge_sample() -> (Schedule, BoardId, SubBoardId, [SheetId; 3]) {
        let (mut s, b, sb) = sample();
        let ids: Vec<StripId> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        for i in [3, 2, 1] {
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sb),
                    index: i,
                },
            )
            .unwrap();
        }
        s.add_board("Other", None);
        let h: Vec<SheetId> = s.sheets.iter().map(|x| x.id).collect();
        let edit =
            |s: &mut Schedule, i: usize, f: &dyn Fn(&mut Sheet)| f(s.sheet_mut(h[i]).unwrap());
        edit(&mut s, 0, &|x| {
            x.pages = Eighths(8);
            x.shoot_time = Some(Minutes(60));
            x.synopsis = "Ada meets Ben.".into();
            x.location = "Barn".into();
        });
        edit(&mut s, 1, &|x| {
            x.pages = Eighths(3);
            x.sunrise = Some(390);
            x.comments = "Rain".into();
        });
        edit(&mut s, 2, &|x| {
            x.pages = Eighths(5);
            x.shoot_time = Some(Minutes(30));
            x.synopsis = "Carl hears a noise.".into();
            x.sunrise = Some(400);
        });
        (s, b, sb, [h[0], h[1], h[2]])
    }

    #[test]
    fn merging_sheets_joins_sums_unions_and_reports_what_was_dropped() {
        let (mut s, b, sb, [one, two, three]) = merge_sample();
        let other = s.boards[1].id;
        let cast = s.category_named("Cast").unwrap();
        let (ada, ben, carl) = (
            s.element_named(cast, "ADA").unwrap(),
            s.element_named(cast, "BEN").unwrap(),
            s.element_named(cast, "CARL").unwrap(),
        );
        // Scene 3 survives (selected first), then 2, then 1: they join in that order.
        let r = s.merge_sheets(three, &[two, one]).unwrap();
        s.check().unwrap();
        assert_eq!(r.survivor, three);
        assert_eq!(r.merged, [two, one], "the order given");
        let h = s.sheet(three).unwrap();
        assert_eq!(h.scenes, "3, 2, 1");
        assert_eq!(h.pages, Eighths(16));
        assert_eq!(h.shoot_time, Some(Minutes(90)));
        assert_eq!(h.screen_time, None, "no sheet had one");
        assert_eq!(h.synopsis, "Carl hears a noise.\nAda meets Ben.");
        assert_eq!(h.comments, "Rain");
        assert_eq!(h.elements, [carl, ada, ben], "the survivor's first");
        assert_eq!(r.elements_added, [ada, ben]);
        // The survivor's own values stay; an empty one is filled from the first sheet.
        assert_eq!(
            (h.int_ext.as_str(), h.set.as_str(), h.time_of_day.as_str()),
            ("INT", "CABIN", "NIGHT")
        );
        assert_eq!(h.location, "Barn");
        assert_eq!(h.sunrise, Some(400));
        let dropped: Vec<(SheetId, &str, &str)> = r
            .dropped
            .iter()
            .map(|d| (d.sheet, d.field, d.value.as_str()))
            .collect();
        assert_eq!(
            dropped,
            [
                (two, "int_ext", "EXT"),
                (two, "set", "CREEK"),
                (one, "time_of_day", "DAY"),
                (two, "sunrise", "6:30"),
            ]
        );
        let combined: Vec<(&str, &str, &str)> = r
            .combined
            .iter()
            .map(|c| (c.field, c.before.as_str(), c.after.as_str()))
            .collect();
        assert_eq!(
            combined,
            [
                ("scenes", "3", "3, 2, 1"),
                ("eighths", "5/8", "2"),
                ("shoot_minutes", "0:30", "1:30"),
                (
                    "synopsis",
                    "Carl hears a noise.",
                    "Carl hears a noise.\nAda meets Ben."
                ),
                ("location", "", "Barn"),
                ("comments", "", "Rain"),
            ]
        );
        // The merged-away strips are gone from every board; breaks stay.
        assert_eq!(sheet_order(&s, b, Some(sb)), ["|", "|", "3, 2, 1", "|"]);
        assert_eq!(sheet_order(&s, other, None), ["3, 2, 1"]);
        assert!(s.sheet(one).is_none() && s.sheet(two).is_none());
    }

    #[test]
    fn strip_layouts_are_checked_set_and_copied_with_a_board() {
        let (mut s, b, _) = sample();
        assert!(s.boards[0].strip_layout.is_default());
        assert_eq!(
            s.boards[0].strip_layout.text(&s),
            "Scene, I/E, Set, Time of day, Pages, Cast IDs"
        );
        let layout = StripLayout::of(vec![
            StripColumn::of(StripField::Scene),
            StripColumn {
                width: StripWidth::Wide,
                ..StripColumn::of(StripField::Synopsis)
            },
            StripColumn::of(StripField::ShootTime),
        ]);
        s.set_strip_layout(b, layout.clone()).unwrap();
        s.check().unwrap();
        assert_eq!(s.boards[0].strip_layout, layout);
        assert_eq!(
            s.boards[0].strip_layout.text(&s),
            "Scene, Synopsis (wide), Shoot time"
        );
        // A duplicated board keeps it; a new board starts with the default.
        let copy = s.duplicate_board(b, "Copy").unwrap();
        assert_eq!(s.board(copy).unwrap().strip_layout, layout);
        let fresh = s.add_board("Fresh", None);
        assert!(s.board(fresh).unwrap().strip_layout.is_default());
        // Refused, nothing changed: no field, a field twice, no such board.
        let before = s.clone();
        for bad in [
            vec![],
            vec![
                StripColumn::of(StripField::Set),
                StripColumn::of(StripField::Set),
            ],
        ] {
            let e = s.set_strip_layout(b, StripLayout::of(bad)).unwrap_err();
            assert!(matches!(e, Error::Broken(_)));
        }
        assert_eq!(
            s.set_strip_layout(BoardId(999_999), StripLayout::default())
                .unwrap_err(),
            Error::NoSuchBoard(BoardId(999_999))
        );
        assert_eq!(s, before);
        // check catches a layout set behind the edits' back.
        s.boards[0].strip_layout.columns.clear();
        assert!(s.check().is_err());
        // Every field has a name that reads back, and a label.
        for f in StripField::ALL {
            assert_eq!(StripField::from_name(f.name()), Some(f));
            assert!(!f.label().is_empty());
        }
        for w in StripWidth::ALL {
            assert_eq!(StripWidth::from_name(w.name()), Some(w));
        }
    }

    #[test]
    fn the_strip_designer_has_four_lines_merged_cells_styles_and_text() {
        // Decision 87: up to four lines; the first line's fields are the columns and a
        // later line's fields cover one or more of them; spacers and custom text repeat.
        let (mut s, b, _) = sample();
        let text = |t: &str, line: u8, span: u8| StripColumn {
            text: t.to_string(),
            ..StripColumn::on_line(StripField::Text, line, span)
        };
        let layout = StripLayout {
            columns: vec![
                StripColumn {
                    bold: true,
                    align: Align::Center,
                    ..StripColumn::of(StripField::Scene)
                },
                StripColumn::of(StripField::Set),
                StripColumn::of(StripField::Spacer),
                StripColumn {
                    rule_after: true,
                    ..StripColumn::of(StripField::Pages)
                },
                StripColumn {
                    italic: true,
                    rule_under: true,
                    ..StripColumn::on_line(StripField::Synopsis, 1, 3)
                },
                StripColumn::on_line(StripField::Spacer, 1, 1),
                text("Notes", 3, 4),
            ],
            size: TextSize::Large,
            ..StripLayout::default()
        };
        assert_eq!(layout.problem(), None);
        assert_eq!(layout.lines(), 4);
        s.set_strip_layout(b, layout.clone()).unwrap();
        s.check().unwrap();
        assert_eq!(
            layout.text(&s),
            "Scene (center, bold), Set, Spacer, Pages (line after), Synopsis (line 2, spans 3, \
             italic, rule under), Spacer (line 2), Text \"Notes\" (line 4, spans 4); large text"
        );
        assert_eq!(StripLayout::default().lines(), 1);
        // Refused: a fifth line, a span on the first line, a later line wider than the
        // first, a sheet field twice, text on another field, a long or two-line text.
        let with = |c: StripColumn| {
            let mut l = layout.clone();
            l.columns.push(c);
            l
        };
        let bad = [
            with(StripColumn::on_line(StripField::Unit, 4, 1)),
            with(StripColumn::on_line(StripField::Unit, 0, 2)),
            with(StripColumn::on_line(StripField::Unit, 1, 1)),
            with(StripColumn::on_line(StripField::Set, 2, 1)),
            with(StripColumn::on_line(StripField::Unit, 2, 0)),
            with(StripColumn {
                text: "x".into(),
                ..StripColumn::on_line(StripField::Unit, 2, 1)
            }),
            with(text(&"x".repeat(MAX_COLUMN_TEXT + 1), 2, 1)),
            with(text("a\nb", 2, 1)),
        ];
        for l in bad {
            assert!(l.problem().is_some(), "{}", l.text(&s));
            assert!(s.set_strip_layout(b, l).is_err());
        }
        // A spacer and a text may repeat.
        assert_eq!(
            with(StripColumn::on_line(StripField::Spacer, 2, 2)).problem(),
            None
        );
        for a in Align::ALL {
            assert_eq!(Align::from_name(a.name()), Some(a));
        }
        for t in TextSize::ALL {
            assert_eq!(TextSize::from_name(t.name()), Some(t));
        }
        // The sheet fields reports offer leave the spacer and text out.
        assert!(StripField::SHEET.iter().all(|f| f.of_sheet()));
        assert_eq!(StripField::SHEET.len() + 3, StripField::ALL.len());
    }

    #[test]
    fn a_banner_style_says_its_size_and_alignment() {
        let style = BannerStyle {
            bold: true,
            size: TextSize::Large,
            align: Align::Center,
            ..BannerStyle::default()
        };
        assert_eq!(style.text(), "bold, large text, centered");
        assert!(!style.is_default());
        assert_eq!(
            BannerStyle {
                size: TextSize::Small,
                align: Align::Right,
                ..BannerStyle::default()
            }
            .text(),
            "small text, on the right"
        );
    }

    #[test]
    fn merging_sheets_joins_in_the_order_given_whatever_the_boards_say() {
        // Note 18: the first selected strip survives and the scenes join in selection
        // order, the survivor's first. Main and Other both order the sheets 1, 2, 3.
        let (mut s, b, sb, [one, two, three]) = merge_sample();
        let other = s.boards[1].id;
        let cast = s.category_named("Cast").unwrap();
        let ada = s.element_named(cast, "ADA").unwrap();
        let r = s.merge_sheets(one, &[three, two]).unwrap();
        s.check().unwrap();
        assert_eq!(r.merged, [three, two], "the order given");
        let h = s.sheet(one).unwrap();
        assert_eq!(h.scenes, "1, 3, 2");
        assert_eq!(h.synopsis, "Ada meets Ben.\nCarl hears a noise.");
        assert_eq!(h.elements[0], ada, "the survivor's elements first");
        // The survivor keeps its place, on every board.
        assert_eq!(sheet_order(&s, b, Some(sb)), ["1, 3, 2", "|", "|", "|"]);
        assert_eq!(sheet_order(&s, other, None), ["1, 3, 2"]);
    }

    #[test]
    fn merging_sheets_refuses_bad_requests_and_changes_nothing() {
        let (mut s, _, _, [one, two, three]) = merge_sample();
        let before = s.clone();
        let broken = |e: Error| matches!(e, Error::Broken(_));
        assert!(broken(s.merge_sheets(one, &[]).unwrap_err()));
        assert!(broken(s.merge_sheets(one, &[one]).unwrap_err()));
        assert!(broken(s.merge_sheets(one, &[two, one]).unwrap_err()));
        assert!(broken(s.merge_sheets(one, &[two, two]).unwrap_err()));
        assert_eq!(
            s.merge_sheets(SheetId(999_999), &[two]).unwrap_err(),
            Error::NoSuchSheet(SheetId(999_999))
        );
        assert_eq!(
            s.merge_sheets(one, &[three, SheetId(999_999)]).unwrap_err(),
            Error::NoSuchSheet(SheetId(999_999))
        );
        s.sheet_mut(one).unwrap().pages = Eighths(u32::MAX);
        let before_big = s.clone();
        assert!(broken(s.merge_sheets(one, &[two]).unwrap_err()));
        assert_eq!(s, before_big);
        s.sheet_mut(one).unwrap().pages = before.sheet(one).unwrap().pages;
        assert_eq!(s, before);
    }

    #[test]
    fn duplicating_a_sheet_puts_the_copy_after_it_or_at_the_boneyard_end() {
        let (mut s, b, sb, [one, two, _]) = merge_sample();
        let other = s.boards[1].id;
        let copy = s.duplicate_sheet(two).unwrap();
        s.check().unwrap();
        let (a, c) = (s.sheet(two).unwrap(), s.sheet(copy).unwrap());
        assert_ne!(copy, two);
        assert_eq!(
            Sheet {
                id: two,
                ..c.clone()
            },
            *a,
            "every field and element"
        );
        assert_eq!(
            sheet_order(&s, b, Some(sb)),
            ["1", "|", "2", "2", "|", "3", "|"]
        );
        assert_eq!(s.sheet_place(b, copy).unwrap().index, 3);
        // In the Boneyard on the other board: at its end.
        assert_eq!(sheet_order(&s, other, None), ["1", "2", "3", "2"]);
        assert_eq!(s.sheet_place(other, copy).unwrap().index, 3);
        // The last strip of a sub-board: the copy goes last.
        let ids: Vec<StripId> = s.boards[1].boneyard.iter().map(|x| x.id).collect();
        let osb = s.boards[1].sub_boards[0].id;
        s.move_strips(
            other,
            &ids[..1],
            Place {
                sub_board: Some(osb),
                index: 0,
            },
        )
        .unwrap();
        let copy2 = s.duplicate_sheet(one).unwrap();
        s.check().unwrap();
        assert_eq!(sheet_order(&s, other, Some(osb)), ["1", "1"]);
        assert_eq!(s.sheet_place(other, copy2).unwrap().index, 1);
        assert_eq!(
            s.duplicate_sheet(SheetId(999_999)).unwrap_err(),
            Error::NoSuchSheet(SheetId(999_999))
        );
    }

    #[test]
    fn property_random_merges_and_duplicates_keep_every_sheet_once_and_every_tag() {
        let mut rng = Rng::new(3535);
        for _ in 0..120 {
            let mut s = Schedule::new("ZTEST");
            let b = s.boards[0].id;
            let cast = s.category_named("Cast").unwrap();
            let els: Vec<ElementId> = (0..5)
                .map(|i| s.add_element(cast, &format!("E{i}")).unwrap())
                .collect();
            let mut subs = vec![None, Some(s.boards[0].sub_boards[0].id)];
            for _ in 0..rng.below(3) {
                subs.push(Some(s.add_sub_board(b, "Unit", None).unwrap()));
            }
            for i in 0..(rng.below(10) + 2) {
                let to = subs[rng.below(subs.len() as u64) as usize];
                let mut elements = Vec::new();
                for e in &els {
                    if rng.below(3) == 0 {
                        elements.push(*e);
                    }
                }
                s.add_sheet_at(
                    Sheet {
                        scenes: i.to_string(),
                        pages: Eighths(rng.below(20) as u32),
                        shoot_time: (rng.below(2) == 0).then(|| Minutes(rng.below(90) as u32)),
                        set: format!("SET {}", rng.below(3)),
                        elements,
                        ..Sheet::default()
                    },
                    b,
                    Place {
                        sub_board: to,
                        index: rng.below(8) as usize,
                    },
                )
                .unwrap();
                if rng.below(3) == 0 {
                    let at = subs[1 + rng.below(subs.len() as u64 - 1) as usize];
                    s.insert_strip(
                        b,
                        StripKind::DayBreak,
                        Place {
                            sub_board: at,
                            index: rng.below(8) as usize,
                        },
                    )
                    .unwrap();
                }
            }
            if rng.below(2) == 0 {
                s.duplicate_board(b, "Copy").unwrap();
            }
            for _ in 0..6 {
                let ids: Vec<SheetId> = s.sheets.iter().map(|x| x.id).collect();
                if rng.below(3) == 0 {
                    let orig = ids[rng.below(ids.len() as u64) as usize];
                    let places: Vec<Option<Place>> =
                        s.boards.iter().map(|x| s.sheet_place(x.id, orig)).collect();
                    let copy = s.duplicate_sheet(orig).unwrap();
                    s.check().unwrap();
                    assert_eq!(
                        s.sheet(copy).unwrap().elements,
                        s.sheet(orig).unwrap().elements
                    );
                    for (x, p) in s.boards.iter().zip(places) {
                        let p = p.unwrap();
                        let q = s.sheet_place(x.id, copy).unwrap();
                        assert_eq!(q.sub_board, p.sub_board);
                        if p.sub_board.is_some() {
                            assert_eq!(q.index, p.index + 1);
                        } else {
                            assert_eq!(q.index, x.boneyard.len() - 1);
                        }
                    }
                    continue;
                }
                if ids.len() < 2 {
                    continue;
                }
                let survivor = ids[rng.below(ids.len() as u64) as usize];
                let mut others = Vec::new();
                for id in &ids {
                    if *id != survivor && rng.below(3) == 0 {
                        others.push(*id);
                    }
                }
                if others.is_empty() {
                    others.push(*ids.iter().find(|id| **id != survivor).unwrap());
                }
                let before = s.clone();
                let group: Vec<&Sheet> = std::iter::once(survivor)
                    .chain(others.iter().copied())
                    .map(|id| before.sheet(id).unwrap())
                    .collect();
                let places: Vec<Option<SubBoardId>> = s
                    .boards
                    .iter()
                    .map(|x| s.sheet_place(x.id, survivor).unwrap().sub_board)
                    .collect();
                let r = s.merge_sheets(survivor, &others).unwrap();
                s.check().unwrap();
                // Every remaining sheet is on every board exactly once (check), the
                // merged ones on none, and the survivor where it was.
                for (x, sub) in s.boards.iter().zip(places) {
                    assert_eq!(s.sheet_place(x.id, survivor).unwrap().sub_board, sub);
                    let mut order = s.sheets_in_board_order(x.id);
                    order.sort();
                    let mut all: Vec<SheetId> = s.sheets.iter().map(|h| h.id).collect();
                    all.sort();
                    assert_eq!(order, all);
                }
                for o in &others {
                    assert!(s.sheet(*o).is_none());
                }
                assert_eq!(s.sheets.len(), before.sheets.len() - others.len());
                // No tag is lost, none doubled, and the sums add up.
                let h = s.sheet(survivor).unwrap();
                for g in &group {
                    for e in &g.elements {
                        assert!(h.elements.contains(e));
                    }
                }
                let distinct: HashSet<ElementId> = group
                    .iter()
                    .flat_map(|g| g.elements.iter().copied())
                    .collect();
                assert_eq!(h.elements.len(), distinct.len());
                assert_eq!(h.pages, group.iter().map(|g| g.pages).sum());
                let shoot: Vec<u32> = group
                    .iter()
                    .filter_map(|g| g.shoot_time)
                    .map(|m| m.0)
                    .collect();
                assert_eq!(
                    h.shoot_time,
                    (!shoot.is_empty()).then(|| Minutes(shoot.iter().sum()))
                );
                // Every dropped set is a value the survivor didn't keep.
                for d in &r.dropped {
                    assert_eq!(d.field, "set");
                    assert_ne!(d.value, h.set);
                }
                // Other sheets are untouched.
                for x in &s.sheets {
                    if x.id != survivor {
                        assert_eq!(Some(x), before.sheet(x.id));
                    }
                }
            }
        }
    }

    #[test]
    fn strip_colors_match_by_value_first_rule_wins_and_are_checked() {
        let rule = |ie: &str, tod: &str, c: u32| StripColorRule {
            int_ext: ie.into(),
            time_of_day: tod.into(),
            color: Rgb(c),
            text: None,
        };
        let mut s = Schedule::new("ZTEST");
        let colors = StripColors {
            rules: vec![
                rule("ext", "Night", 1),
                rule("", "NIGHT", 2),
                rule("INT", "", 3),
            ],
            ..StripColors::default()
        };
        s.set_strip_colors(colors.clone()).unwrap();
        let sheet = |ie: &str, tod: &str| Sheet {
            int_ext: ie.into(),
            time_of_day: tod.into(),
            ..Sheet::default()
        };
        let c = &s.strip_colors;
        assert_eq!(c.color_of(&sheet(" EXT ", "NIGHT")), Some(Rgb(1)));
        assert_eq!(c.color_of(&sheet("INT", "night")), Some(Rgb(2)));
        assert_eq!(c.color_of(&sheet("INT", "DAY")), Some(Rgb(3)));
        assert_eq!(c.color_of(&sheet("EXT", "DAY")), None);
        // A pair's own text color wins over the scene text color (decision 108).
        let mut inked = colors.clone();
        inked.scene_text = Some(Rgb(7));
        inked.rules[0].text = Some(Rgb(8));
        assert_eq!(inked.text_of(&sheet("EXT", "NIGHT")), Some(Rgb(8)));
        assert_eq!(inked.text_of(&sheet("INT", "NIGHT")), Some(Rgb(7)));
        assert_eq!(inked.text_of(&sheet("EXT", "DAY")), Some(Rgb(7)));
        assert!(inked.text().contains("ext / Night #000001 text #000008"));
        let mut bad_ink = inked.clone();
        bad_ink.rules[1].text = Some(Rgb(0x0100_0000));
        assert!(bad_ink.problem().is_some());
        // Refused, nothing changed: two rules for the same values, a color out of range.
        let mut twice = colors.clone();
        twice.rules.push(rule("EXT ", "night", 9));
        assert!(s.set_strip_colors(twice).is_err());
        let mut big = colors.clone();
        big.scene_text = Some(Rgb(0x0100_0000));
        assert!(s.set_strip_colors(big).is_err());
        assert_eq!(s.strip_colors, colors);
        s.check().unwrap();
        assert_eq!(Rgb::parse("#1F3524"), Some(Rgb(0x1f3524)));
        assert_eq!(Rgb(0x1f3524).hex(), "#1f3524");
        assert!(Rgb::parse("1f3524").is_none() && Rgb::parse("#1f352g").is_none());
        assert_eq!(Rgb(0xfff4bf).ink(), Rgb(0));
        assert_eq!(Rgb(0x1f3524).ink(), Rgb(Rgb::MAX));
    }

    #[test]
    fn sheet_field_order_numbers_steps_and_templates() {
        let mut s = Schedule::new("ZTEST");
        assert_eq!(s.sheet_field_order(), SHEET_FIELDS);
        s.set_sheet_field_order(&["sunset".into(), "scenes".into()])
            .unwrap();
        let order = s.sheet_field_order();
        assert_eq!(&order[..3], ["sunset", "scenes", "number"]);
        assert_eq!(order.len(), SHEET_FIELDS.len());
        assert!(s
            .set_sheet_field_order(&["scenes".into(), "scenes".into()])
            .is_err());
        assert!(s.set_sheet_field_order(&["budget".into()]).is_err());
        // Back to the default stores nothing.
        let all: Vec<String> = SHEET_FIELDS.iter().map(|x| x.to_string()).collect();
        s.set_sheet_field_order(&all).unwrap();
        assert!(s.sheet_fields.is_empty());

        // Stepping by sheet number and scene number, naturally, empties last.
        let b = s.boards[0].id;
        let add = |s: &mut Schedule, number: &str, scenes: &str| {
            s.add_sheet(Sheet {
                number: number.into(),
                scenes: scenes.into(),
                ..Sheet::default()
            })
        };
        let a = add(&mut s, "10", "3");
        let c = add(&mut s, "", "10A");
        let d = add(&mut s, "9", "");
        let e = add(&mut s, "10A", "10");
        assert_eq!(s.sheets_in_order(b, SheetOrder::Board), [a, c, d, e]);
        assert_eq!(s.sheets_in_order(b, SheetOrder::Number), [d, a, e, c]);
        assert_eq!(s.sheets_in_order(b, SheetOrder::Scene), [a, e, c, d]);
        assert_eq!(SheetOrder::parse("scene"), Some(SheetOrder::Scene));
        assert_eq!(SheetOrder::parse("x"), None);

        // Sheet images: no empty or repeated path.
        s.sheet_mut(a).unwrap().images = vec!["a.png".into(), "a.png".into()];
        assert!(s.check().is_err());
        s.sheet_mut(a).unwrap().images = vec!["a.png".into()];
        s.sheet_mut(c).unwrap().images = vec!["c.png".into(), "a.png".into()];
        s.check().unwrap();

        // Merging sheets joins images and keeps the survivor's number.
        let m = s.merge_sheets(a, &[c, d]).unwrap();
        let h = s.sheet(a).unwrap();
        assert_eq!(h.images, ["a.png", "c.png"]);
        assert_eq!(h.number, "10");
        assert!(m
            .dropped
            .iter()
            .any(|x| x.field == "number" && x.value == "9"));
        assert!(m.combined.iter().any(|x| x.field == "images"));

        // A template keeps the setup and drops the sheets, strips, events and flags.
        let t = s.to_template();
        assert!(t.sheets.is_empty());
        assert!(t.boards[0].boneyard.is_empty());
        assert_eq!(t.categories, s.categories);
        t.check().unwrap();
    }

    /// Behavior note 23 (E-E14): new schedules start with a built-in red flag type per
    /// event kind but `other`; they can be renamed and recolored, never deleted; an older
    /// schedule gets them only when asked, a same-named type of its own taking the part.
    #[test]
    fn built_in_red_flag_types_match_the_event_kinds() {
        let s = Schedule::with_default_categories("ZTEST Built-in types");
        let kinds: Vec<_> = s
            .red_flag_types
            .iter()
            .map(|t| t.builtin.clone().unwrap())
            .collect();
        assert_eq!(kinds, EventKind::builtin_flag_kinds());
        assert_eq!(kinds.len(), 16);
        let names: Vec<_> = s.red_flag_types.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            &names[..6],
            &[
                "Travel",
                "Fitting",
                "Rehearsal",
                "Unavailable",
                "Camera test",
                "Post-production"
            ]
        );
        assert_eq!(s.missing_standard_red_flag_types(), 0);
        s.check().unwrap();

        let mut s = s;
        let travel = s.red_flag_types[0].id;
        assert_eq!(
            s.delete_red_flag_type(travel),
            Err(Error::BuiltInType("Travel".into()))
        );
        s.rename_red_flag_type(travel, "On the road").unwrap();
        s.red_flag_types[0].color = Some(Rgb(0x2a9d8f));
        s.check().unwrap();
        // Renamed, it is still the travel type: asking again adds nothing.
        assert_eq!(s.standard_red_flag_types(), 0);
        assert_eq!(s.red_flag_types.len(), 16);
        // The schedule's own types still delete.
        let own = s.add_red_flag_type("Second job").unwrap();
        s.delete_red_flag_type(own).unwrap();

        // An older schedule: nothing until asked; its own Unavailable takes the part.
        let mut old = Schedule::new("ZTEST Older");
        let mine = old.add_red_flag_type("unavailable").unwrap();
        assert_eq!(old.missing_standard_red_flag_types(), 16);
        assert_eq!(old.standard_red_flag_types(), 16);
        assert_eq!(old.red_flag_types.len(), 16);
        let t = old.red_flag_type(mine).unwrap();
        assert_eq!(
            (t.name.as_str(), t.builtin.clone()),
            ("unavailable", Some(EventKind::Unavailable))
        );
        assert_eq!(old.red_flag_types[1].builtin, Some(EventKind::Travel));
        old.check().unwrap();

        // Two types built in as one kind, or as other, don't hang together.
        let mut twice = old.clone();
        twice.red_flag_types[1].builtin = Some(EventKind::Unavailable);
        assert!(twice.check().is_err());
        let mut other = old.clone();
        other.red_flag_types[1].builtin = Some(EventKind::Other(String::new()));
        assert!(other.check().is_err());
    }

    /// Behavior note 23: flags for several elements and several dates at once.
    #[test]
    fn red_flags_for_several_elements_and_dates_at_once() {
        let mut s = Schedule::with_default_categories("ZTEST Several flags");
        let cast = s.cast().unwrap();
        let ada = s.add_element(cast, "ADA").unwrap();
        let ben = s.add_element(cast, "BEN").unwrap();
        let kind = s.red_flag_types[3].id;
        let d = |x: &str| -> Date { x.parse().unwrap() };
        let picked = [
            d("2027-03-04"),
            d("2027-03-02"),
            d("2027-03-03"),
            d("2027-03-09"),
            d("2027-03-02"),
        ];
        let dates = crate::redflags::runs(&picked);
        assert_eq!(
            dates,
            vec![
                RedFlagDates::Range {
                    from: d("2027-03-02"),
                    to: d("2027-03-04")
                },
                RedFlagDates::Range {
                    from: d("2027-03-09"),
                    to: d("2027-03-09")
                },
            ]
        );
        let ids = s
            .add_red_flags(&[ada, ben], kind, &dates, "Second job", true)
            .unwrap();
        assert_eq!(ids.len(), 4);
        assert_eq!(s.element(ada).unwrap().red_flags.len(), 2);
        assert_eq!(s.element(ben).unwrap().red_flags[1].note, "Second job");
        s.check().unwrap();
        // All or nothing.
        let before = s.clone();
        assert!(s
            .add_red_flags(&[ada, ada], kind, &dates, "", true)
            .is_err());
        assert!(s
            .add_red_flags(&[ada, ElementId(9999)], kind, &dates, "", true)
            .is_err());
        assert!(s.add_red_flags(&[], kind, &dates, "", true).is_err());
        // 101 elements on 100 separate dates is over the limit for one go; 100 are not.
        let many: Vec<RedFlagDates> = (0..100)
            .map(|i| {
                let x = d("2027-03-01").add_days(i * 2);
                RedFlagDates::Range { from: x, to: x }
            })
            .collect();
        let mut wide = s.clone();
        let crowd: Vec<ElementId> = (0..101)
            .map(|i| wide.add_element(cast, &format!("EXTRA {i}")).unwrap())
            .collect();
        assert!(wide.add_red_flags(&crowd, kind, &many, "", true).is_err());
        assert_eq!(
            wide.add_red_flags(&crowd[..100], kind, &many, "", true)
                .map(|ids| ids.len()),
            Ok(MAX_RED_FLAGS_AT_ONCE)
        );
        assert!(s.add_red_flags(&[ada], kind, &[], "", true).is_err());
        assert!(s.delete_red_flags(&[ids[0], RedFlagId(9999)]).is_err());
        assert_eq!(s, before);
        s.delete_red_flags(&[ids[0], ids[3]]).unwrap();
        assert_eq!(s.element(ada).unwrap().red_flags.len(), 1);
        assert_eq!(s.element(ben).unwrap().red_flags.len(), 1);
    }

    #[test]
    fn a_template_drops_the_plan_that_points_at_strips_and_sheets() {
        use crate::plan;
        let (mut s, b, _) = sample();
        let cast = s.cast().unwrap();
        let strip = s.boards[0].boneyard[0].id;
        s.plan
            .set_location(plan::GeoLocation {
                name: "Cabin".into(),
                latitude: 1,
                longitude: 2,
                utc_offset: 0,
                daylight_saving: None,
            })
            .unwrap();
        plan::set_long_day(&mut s, Some(600)).unwrap();
        plan::set_turnaround(&mut s, cast, Some(720)).unwrap();
        plan::set_pinned(&mut s, &[strip], true).unwrap();
        s.plan.omitted.push(s.sheets[0].id);
        crate::drift::lock(&mut s, b, "First", None).unwrap();
        s.plan.call_times.push(plan::CallTime {
            day: strip,
            minutes: 420,
        });
        let t = s.to_template();
        assert_eq!(t.plan.locations, s.plan.locations);
        assert_eq!(t.plan.long_day, Some(600));
        assert_eq!(t.plan.turnarounds, s.plan.turnarounds);
        assert!(t.plan.locks.is_empty());
        assert!(t.plan.call_times.is_empty());
        assert!(t.plan.pinned.is_empty());
        assert!(t.plan.omitted.is_empty());
        t.check().unwrap();
        assert_eq!(t.template_problem(), None);
        assert_eq!(s.template_problem(), Some("a template holds no sheets"));
        let mut u = t.clone();
        u.plan.locks = s.plan.locks.clone();
        assert!(u.template_problem().is_some());
    }

    /// Review: nothing bounded a sheet's numbers or `next_id`, and check let a merge or a
    /// tool leave plan data the setters refuse.
    #[test]
    fn check_bounds_numbers_and_the_plan() {
        let mut s = Schedule::new("T");
        let h = s.add_sheet(Sheet::default());
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        s.tag(h, ann).unwrap();
        s.add_property(cast, "Size").unwrap();
        s.add_property(cast, "Hair").unwrap();
        s.set_property_value(ann, "Hair", "Red").unwrap();
        s.set_property_value(ann, "Size", "L").unwrap();
        assert!(s.check().is_ok());
        let day = {
            let b = s.boards[0].id;
            let sb = s.boards[0].sub_boards[0].id;
            let at = Place {
                sub_board: Some(sb),
                index: 0,
            };
            s.insert_strip(b, StripKind::DayBreak, at).unwrap()
        };
        let bad = |f: &dyn Fn(&mut Schedule)| {
            let mut t = s.clone();
            f(&mut t);
            t.check().is_err()
        };
        assert!(bad(&|t| t.sheets[0].pages = Eighths(MAX_SHEET_EIGHTHS + 1)));
        assert!(!bad(&|t| t.sheets[0].pages = Eighths(MAX_SHEET_EIGHTHS)));
        assert!(bad(&|t| t.sheets[0].shoot_time = Some(Minutes(24 * 60 + 1))));
        assert!(!bad(&|t| t.sheets[0].shoot_time = Some(Minutes(24 * 60))));
        assert!(bad(
            &|t| t.sheets[0].screen_time = Some(Seconds(MAX_SCREEN_SECONDS + 1))
        ));
        assert!(bad(&|t| t.next_id = MAX_NEXT_ID + 1));
        assert!(bad(&|t| t.sheets[0].quantities = vec![Quantity {
            element: ann,
            count: MAX_QUANTITY + 1,
            daily: true,
        }]));
        assert!(s
            .clone()
            .set_quantity(h, ann, MAX_QUANTITY + 1, true)
            .is_err());
        assert!(bad(&|t| t.elements[0].properties.reverse()));
        assert!(bad(&|t| t.categories[0].properties[0] = "a\nb".into()));
        assert!(bad(&|t| t.categories[0].properties[0] = "x".repeat(61)));
        // The plan.
        assert!(bad(&|t| t.plan.call_times = vec![crate::plan::CallTime {
            day,
            minutes: 24 * 60,
        }]));
        assert!(bad(&|t| t.plan.call_times = vec![
            crate::plan::CallTime { day, minutes: 60 },
            crate::plan::CallTime { day, minutes: 70 },
        ]));
        assert!(bad(&|t| t.plan.locations =
            vec![crate::plan::GeoLocation {
                name: "Pier".into(),
                latitude: 90_000_001,
                longitude: 0,
                utc_offset: 0,
                daylight_saving: None,
            }]));
        assert!(bad(&|t| t.plan.pinned = vec![day]));
        assert!(bad(&|t| {
            crate::drift::lock(t, t.boards[0].id, "L", None).unwrap();
            t.plan.locks[0].name = "x".repeat(201);
        }));
        let b = s.boards[0].id;
        assert!(crate::drift::lock(&mut s.clone(), b, &"x".repeat(201), None).is_err());
    }

    #[test]
    fn page_totals_saturate() {
        let big = Eighths(u32::MAX - 1);
        assert_eq!(big + Eighths(5), Eighths(u32::MAX));
        let mut x = big;
        x += Eighths(5);
        assert_eq!(x, Eighths(u32::MAX));
        assert_eq!([big, big].into_iter().sum::<Eighths>(), Eighths(u32::MAX));
    }

    #[test]
    fn banner_colors_by_name_or_value() {
        // Decision 154: the nine names as before, or any color as lowercase #rrggbb.
        for c in BannerColor::NAMED {
            assert_eq!(BannerColor::from_text(&c.text()), Some(c));
        }
        assert_eq!(BannerColor::from_text("teal"), Some(BannerColor::Teal));
        let c = BannerColor::from_text("#3fb8a8").unwrap();
        assert_eq!(c, BannerColor::Rgb(Rgb(0x3f_b8_a8)));
        assert_eq!((c.text().as_str(), c.name()), ("#3fb8a8", None));
        for bad in [
            "Teal", "#3FB8A8", "#3fb8a", "3fb8a8", "#3fb8a8 ", "", "none",
        ] {
            assert_eq!(BannerColor::from_text(bad), None, "{bad:?}");
        }
    }
}
