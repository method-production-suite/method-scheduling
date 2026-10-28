//! What changed between two schedules, in plain words (roadmap phase 3; docs/MERGE.md;
//! decisions.md, entry 25). Ported from Method Budget's diff, same design.
//!
//! [`diff`] compares an older schedule `a` with a newer one `b` by stable id, never by
//! position (docs/FORMAT.md): categories, red flag types, elements, red flags, sheets,
//! boards, sub-boards and strips each by their own id. Events have no ids yet, so they compare by value: an edited
//! event reads as one removed and one added.
//!
//! The result lists every change in a fixed order: the title, categories, elements,
//! sheets, then board by board (the board, its sub-boards with their calendars, then its
//! strips in board order), each list in `b`'s order with what went last. Strips that
//! changed place are reported one by one: a strip that went to another sub-board or the
//! Boneyard is moved, and so is one that changed place in its own list when it falls
//! outside a longest run of strips that kept their order, so one strip dragged up doesn't
//! mark every strip it passed.
//!
//! Not listed: `next_id` and the order of the element and sheet lists, which are
//! bookkeeping (creation order), not content.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::hash::Hash;

use crate::model::{
    Board, BoardId, Calendar, Category, CategoryId, Element, ElementId, Event, EventKind,
    PropertyValue, Quantity, RedFlag, RedFlagId, RedFlagTypeId, Schedule, Sheet, SheetId, Strip,
    StripId, StripKind, SubBoard, SubBoardId,
};
use crate::{Date, Weekday};

// ---------------------------------------------------------------------------
// What a diff is

/// What happened to the thing a [`Change`] is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    /// Fields changed in place.
    Changed,
    /// Changed place: a strip on the board, a category or sub-board in its list. Field
    /// changes may come with it.
    Moved,
}

impl ChangeKind {
    /// `"added"`, `"removed"`, `"changed"`, `"moved"`.
    pub fn key(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Removed => "removed",
            ChangeKind::Changed => "changed",
            ChangeKind::Moved => "moved",
        }
    }
}

/// What a change is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    Title,
    /// The production info, the report list or the day out of days codes.
    Setup,
    /// The strip colors (decision 60).
    StripColors,
    /// The layout library (decision 78).
    Layouts,
    Category(CategoryId),
    RedFlagType(RedFlagTypeId),
    Element(ElementId),
    /// A red flag, with the element it is on (in the newer schedule when it is there).
    RedFlag(ElementId, RedFlagId),
    Sheet(SheetId),
    Board(BoardId),
    SubBoard(BoardId, SubBoardId),
    /// A date or an event on a sub-board's calendar, or its weekly days off, with the
    /// date it is about (an event's first day; none for the weekly days off).
    Calendar(BoardId, SubBoardId, Option<crate::Date>),
    Strip(BoardId, StripId),
}

impl Subject {
    /// The heading a list groups this change under.
    pub fn group(&self) -> &'static str {
        match self {
            Subject::Title | Subject::Setup | Subject::StripColors | Subject::Layouts => "Schedule",
            Subject::Category(_) => "Categories",
            Subject::RedFlagType(_) => "Red flag types",
            Subject::Element(_) => "Elements",
            Subject::RedFlag(..) => "Red flags",
            Subject::Sheet(_) => "Sheets",
            Subject::Board(_) | Subject::SubBoard(..) => "Boards",
            Subject::Calendar(..) => "Calendars",
            Subject::Strip(..) => "Strips",
        }
    }

    /// `"title"`, `"category"`...: for programs.
    pub fn key(&self) -> &'static str {
        match self {
            Subject::Title => "title",
            Subject::Setup => "setup",
            Subject::StripColors => "strip_colors",
            Subject::Layouts => "layouts",
            Subject::Category(_) => "category",
            Subject::RedFlagType(_) => "red_flag_type",
            Subject::Element(_) => "element",
            Subject::RedFlag(..) => "red_flag",
            Subject::Sheet(_) => "sheet",
            Subject::Board(_) => "board",
            Subject::SubBoard(..) => "sub_board",
            Subject::Calendar(..) => "calendar",
            Subject::Strip(..) => "strip",
        }
    }
}

/// One field's change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    /// The file-format key (`"set"`, `"eighths"`, `"holds"`).
    pub field: &'static str,
    pub before: String,
    pub after: String,
    /// The phrase: `set "LOFT" -> "NET LOFT"`, `elements +KIT -TOVE`.
    pub text: String,
}

/// One change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub subject: Subject,
    /// Field by field, for changed (and moved) things.
    pub fields: Vec<FieldChange>,
    /// The whole change in one plain sentence.
    pub text: String,
}

/// Everything that changed from one schedule to another.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub changes: Vec<Change>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The diff as text for a terminal: changes grouped under their headings.
    pub fn to_text(&self) -> String {
        if self.changes.is_empty() {
            return "No changes.\n".to_string();
        }
        let n = self.changes.len();
        let mut out = format!("{n} change{}\n", if n == 1 { "" } else { "s" });
        let mut last = "";
        for c in &self.changes {
            let g = c.subject.group();
            if g != last {
                let _ = writeln!(out, "\n{g}");
                last = g;
            }
            let _ = writeln!(out, "  {}", c.text);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Words

/// Text quoted, flattened to one line and cut at 60 characters.
pub(crate) fn quoted(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cut: String = flat.chars().take(60).collect();
    if cut.chars().count() < flat.chars().count() {
        format!("\"{cut}...\"")
    } else {
        format!("\"{cut}\"")
    }
}

pub(crate) fn on_off(b: bool) -> String {
    if b { "on" } else { "off" }.to_string()
}

/// Property values in words: `none`, or `Height "1.85 m", Agent "Ruth"`.
pub(crate) fn properties_text(v: &[PropertyValue]) -> String {
    if v.is_empty() {
        return "none".to_string();
    }
    v.iter()
        .map(|p| format!("{} {}", p.name, quoted(&p.value)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Sheet quantities in words: `none`, or `element 12: 35, element 14: 2 not daily`.
pub(crate) fn quantities_text(v: &[Quantity]) -> String {
    if v.is_empty() {
        return "none".to_string();
    }
    v.iter()
        .map(|q| {
            format!(
                "element {}: {}{}",
                q.element.0,
                q.count,
                if q.daily { "" } else { " not daily" }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn names_text(v: &[String]) -> String {
    if v.is_empty() {
        "none".to_string()
    } else {
        v.iter().map(|x| quoted(x)).collect::<Vec<_>>().join(", ")
    }
}

fn opt<T>(v: Option<T>, f: impl Fn(T) -> String) -> String {
    v.map_or("none".to_string(), f)
}

/// Minutes as `h:mm`.
pub(crate) fn hm(m: u32) -> String {
    format!("{}:{:02}", m / 60, m % 60)
}

/// Minutes after midnight as `hh:mm`.
fn clock(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub(crate) fn weekday_name(w: Weekday) -> &'static str {
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

pub(crate) fn weekdays_text(ws: &[Weekday]) -> String {
    if ws.is_empty() {
        return "none".to_string();
    }
    ws.iter()
        .map(|w| weekday_name(*w))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How a person names a sheet: `scene 12`, else its heading (`Sheet::title`), never the
/// sheet's id (decision 162). The set is left out: a change to it is often what is told.
pub(crate) fn sheet_name(sh: &Sheet) -> String {
    let scenes = sh.scenes.trim();
    if scenes.is_empty() {
        sh.title()
    } else {
        format!("scene {scenes}")
    }
}

/// A sheet no schedule has any more, for sentences.
pub(crate) const GONE_SHEET: &str = "a scene no longer in the schedule";

/// Capitalized for the start of a sentence.
pub(crate) fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// An event in words: `fitting "Wardrobe" on 2027-03-08 (BRUNO)`.
pub(crate) fn event_text(ev: &Event, name_of: impl Fn(ElementId) -> String) -> String {
    let kind = match &ev.kind {
        EventKind::Travel => "travel".to_string(),
        EventKind::Fitting => "fitting".to_string(),
        EventKind::Rehearsal => "rehearsal".to_string(),
        EventKind::Unavailable => "unavailable".to_string(),
        EventKind::CameraTest => "camera test".to_string(),
        EventKind::Other(l) if l.trim().is_empty() => "other event".to_string(),
        EventKind::Other(l) => format!("other event {}", quoted(l)),
        EventKind::Post(k) => k.label().to_lowercase(),
    };
    let name = if ev.name.trim().is_empty() {
        String::new()
    } else {
        format!(" {}", quoted(&ev.name))
    };
    let when = if ev.from == ev.to {
        format!("on {}", ev.from)
    } else {
        format!("{} to {}", ev.from, ev.to)
    };
    let who = if ev.elements.is_empty() {
        String::new()
    } else {
        format!(
            " ({})",
            ev.elements
                .iter()
                .map(|e| name_of(*e))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    // A color shows only when set (decision 109).
    let color = ev
        .color
        .and_then(|c| crate::model::EVENT_COLORS.get(usize::from(c)))
        .map_or(String::new(), |(_, n, _)| format!(", {}", n.to_lowercase()));
    format!("{kind}{name} {when}{who}{color}")
}

/// What a calendar date is marked as, in the reader's order of precedence (FORMAT.md):
/// a company travel day, then a holiday, a day off, a weekly day off marked back to
/// working. `None`: not marked, as the working week has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Mark {
    Move,
    Holiday,
    Off,
    Work,
}

impl Mark {
    pub(crate) fn of(c: &Calendar, d: Date) -> Option<Mark> {
        if c.moves.contains(&d) {
            Some(Mark::Move)
        } else if c.holidays.contains(&d) {
            Some(Mark::Holiday)
        } else if c.off_days.contains(&d) {
            Some(Mark::Off)
        } else if c.work_days.contains(&d) {
            Some(Mark::Work)
        } else {
            None
        }
    }

    pub(crate) fn name(m: Option<Mark>) -> &'static str {
        match m {
            Some(Mark::Move) => "company travel day",
            Some(Mark::Holiday) => "holiday",
            Some(Mark::Off) => "day off",
            Some(Mark::Work) => "marked back to working",
            None => "as the working week has it",
        }
    }

    pub(crate) fn list(c: &mut Calendar, m: Mark) -> &mut Vec<Date> {
        match m {
            Mark::Move => &mut c.moves,
            Mark::Holiday => &mut c.holidays,
            Mark::Off => &mut c.off_days,
            Mark::Work => &mut c.work_days,
        }
    }
}

/// Every date any of the calendar's lists names.
pub(crate) fn marked_dates(c: &Calendar) -> BTreeSet<Date> {
    c.moves
        .iter()
        .chain(&c.holidays)
        .chain(&c.off_days)
        .chain(&c.work_days)
        .copied()
        .collect()
}

// ---------------------------------------------------------------------------
// Order

/// The elements of `b` that moved relative to `a`: those outside a longest common
/// subsequence of the two (only elements in both count). Elements are unique, so this is
/// a longest increasing subsequence of positions. Method Budget's `moved_in`.
pub(crate) fn moved_in<K: Copy + Eq + Hash + Ord>(a: &[K], b: &[K]) -> BTreeSet<K> {
    let pos: HashMap<K, usize> = a.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let seq: Vec<(K, usize)> = b
        .iter()
        .filter_map(|k| pos.get(k).map(|p| (*k, *p)))
        .collect();
    // Patience sorting with back links.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; seq.len()];
    for (i, (_, p)) in seq.iter().enumerate() {
        let at = tails.partition_point(|&t| seq[t].1 < *p);
        if at > 0 {
            prev[i] = Some(tails[at - 1]);
        }
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut keep = BTreeSet::new();
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        keep.insert(seq[i].0);
        cur = prev[i];
    }
    seq.iter()
        .map(|(k, _)| *k)
        .filter(|k| !keep.contains(k))
        .collect()
}

// ---------------------------------------------------------------------------
// Fields

struct Field {
    key: &'static str,
    label: &'static str,
    value: String,
}

fn f(key: &'static str, label: &'static str, value: String) -> Field {
    Field { key, label, value }
}

fn field_changes(a: &[Field], b: &[Field]) -> Vec<FieldChange> {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.value != y.value)
        .map(|(x, y)| FieldChange {
            field: x.key,
            before: x.value.clone(),
            after: y.value.clone(),
            text: format!("{} {} -> {}", x.label, x.value, y.value),
        })
        .collect()
}

/// A list of ids changed as `+KIT -TOVE`, or `reordered` when only the order moved.
fn list_change(
    key: &'static str,
    label: &'static str,
    a: &[ElementId],
    b: &[ElementId],
    name: &impl Fn(ElementId) -> String,
) -> Option<FieldChange> {
    if a == b {
        return None;
    }
    let all = |l: &[ElementId]| l.iter().map(|e| name(*e)).collect::<Vec<_>>().join(", ");
    let mut parts: Vec<String> = b
        .iter()
        .filter(|e| !a.contains(e))
        .map(|e| format!("+{}", name(*e)))
        .collect();
    parts.extend(
        a.iter()
            .filter(|e| !b.contains(e))
            .map(|e| format!("-{}", name(*e))),
    );
    if parts.is_empty() {
        parts.push("reordered".to_string());
    }
    Some(FieldChange {
        field: key,
        before: all(a),
        after: all(b),
        text: format!("{label} {}", parts.join(" ")),
    })
}

fn category_fields(c: &Category) -> Vec<Field> {
    vec![
        f("name", "name", quoted(&c.name)),
        f(
            "auto_board_id",
            "automatic board IDs",
            on_off(c.auto_board_id),
        ),
        f(
            "exclude_from_board",
            "left off the board",
            on_off(c.exclude_from_board),
        ),
        f("holds", "default holds", on_off(c.defaults.holds)),
        f("drops", "default drops", on_off(c.defaults.drops)),
        f(
            "min_drop_gap",
            "default minimum drop gap",
            days_text(c.defaults.min_drop_gap),
        ),
        f(
            "daily_quantity",
            "use in daily quantity",
            on_off(c.daily_quantity),
        ),
        f("properties", "properties", names_text(&c.properties)),
        f(
            "multi_line",
            "properties of several lines",
            names_text(&c.multi_line),
        ),
    ]
}

pub(crate) fn days_text(n: u32) -> String {
    format!("{n} day{}", if n == 1 { "" } else { "s" })
}

fn element_fields(e: &Element, cat_name: &impl Fn(CategoryId) -> String) -> Vec<Field> {
    vec![
        f("category", "category", cat_name(e.category)),
        f("name", "name", quoted(&e.name)),
        f("board_id", "board ID", opt(e.board_id.as_deref(), quoted)),
        f(
            "board_id_locked",
            "board ID lock",
            on_off(e.board_id_locked),
        ),
        f(
            "exclude_from_board",
            "left off the board",
            on_off(e.exclude_from_board),
        ),
        f("holds", "holds", on_off(e.rules.holds)),
        f("drops", "drops", on_off(e.rules.drops)),
        f(
            "min_drop_gap",
            "minimum drop gap",
            days_text(e.rules.min_drop_gap),
        ),
        f("notes", "notes", quoted(&e.notes)),
        f("properties", "properties", properties_text(&e.properties)),
        f("images", "images", names_text(&e.images)),
    ]
}

fn sheet_fields(s: &Sheet) -> Vec<Field> {
    vec![
        f("number", "sheet number", quoted(&s.number)),
        f("scenes", "scenes", quoted(&s.scenes)),
        f("int_ext", "I/E", quoted(&s.int_ext)),
        f("set", "set", quoted(&s.set)),
        f("time_of_day", "time of day", quoted(&s.time_of_day)),
        f("sunrise", "sunrise", opt(s.sunrise, clock)),
        f("sunset", "sunset", opt(s.sunset, clock)),
        f("eighths", "pages", s.pages.to_string()),
        f(
            "shoot_minutes",
            "shoot time",
            opt(s.shoot_time, |m| hm(m.0)),
        ),
        f("synopsis", "synopsis", quoted(&s.synopsis)),
        f("location", "location", quoted(&s.location)),
        f("script_pages", "script pages", quoted(&s.script_pages)),
        f("script_day", "script day", quoted(&s.script_day)),
        f(
            "screen_seconds",
            "screen time",
            opt(s.screen_time, |x| format!("{}:{:02}", x.0 / 60, x.0 % 60)),
        ),
        f("unit", "unit", quoted(&s.unit)),
        f("sequence", "sequence", quoted(&s.sequence)),
        f("episode", "episode", quoted(&s.episode)),
        f("comments", "comments", quoted(&s.comments)),
        f("quantities", "quantities", quantities_text(&s.quantities)),
        f("images", "images", names_text(&s.images)),
    ]
}

/// The header field order in words: `default`, or the keys joined by commas.
/// What the Calendar hides in words: `nothing hidden`, or `hides pages, time`.
pub(crate) fn calendar_hidden_text(v: &[crate::model::CalendarShow]) -> String {
    if v.is_empty() {
        "everything shown".to_string()
    } else {
        format!(
            "hides {}",
            v.iter().map(|c| c.key()).collect::<Vec<_>>().join(", ")
        )
    }
}

/// A calendar strip's fields in words: `scene, set`, or `default`.
pub(crate) fn calendar_fields_text(v: &[crate::model::StripField]) -> String {
    if v.is_empty() {
        "default".to_string()
    } else {
        v.iter().map(|f| f.name()).collect::<Vec<_>>().join(", ")
    }
}

pub(crate) fn field_order_text(v: &[&'static str]) -> String {
    if v.is_empty() {
        "default".to_string()
    } else {
        v.join(", ")
    }
}

/// A red flag type's color in words: `#rrggbb`, or `none`.
pub(crate) fn color_text(c: Option<crate::model::Rgb>) -> String {
    c.map_or("none".to_string(), |c| c.hex())
}

fn strip_fields(st: &Strip) -> Vec<Field> {
    match &st.kind {
        StripKind::Banner(b) => vec![
            f("text", "text", quoted(&b.text)),
            f("minutes", "time", hm(b.duration.0)),
            f("style", "style", b.style.text()),
        ],
        _ => Vec::new(),
    }
}

fn sentence(head: &str, fields: &[FieldChange]) -> String {
    let parts: Vec<&str> = fields.iter().map(|c| c.text.as_str()).collect();
    format!("{head}: {}", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Names from either side

/// Looks things up in the newer schedule, then the older one, so a change can name what
/// went too.
pub(crate) struct Names<'a> {
    pub(crate) a: &'a Schedule,
    pub(crate) b: &'a Schedule,
}

impl Names<'_> {
    pub(crate) fn category(&self, id: CategoryId) -> String {
        self.b
            .category(id)
            .or_else(|| self.a.category(id))
            .map_or(format!("category {}", id.0), |c| quoted(&c.name))
    }

    pub(crate) fn element(&self, id: ElementId) -> String {
        self.b
            .element(id)
            .or_else(|| self.a.element(id))
            .map_or(format!("element {}", id.0), |e| e.name.clone())
    }

    /// `Element "ADA" (Cast)`.
    pub(crate) fn element_title(&self, id: ElementId) -> String {
        match self.b.element(id).or_else(|| self.a.element(id)) {
            Some(e) => {
                let cat = self
                    .b
                    .category(e.category)
                    .or_else(|| self.a.category(e.category))
                    .map_or(String::new(), |c| format!(" ({})", c.name));
                format!("Element {}{cat}", quoted(&e.name))
            }
            None => format!("Element {}", id.0),
        }
    }

    pub(crate) fn sheet(&self, id: SheetId) -> String {
        self.b
            .sheet(id)
            .or_else(|| self.a.sheet(id))
            .map_or(GONE_SHEET.to_string(), sheet_name)
    }

    pub(crate) fn board(&self, id: BoardId) -> String {
        self.b
            .board(id)
            .or_else(|| self.a.board(id))
            .map_or(format!("board {}", id.0), |b| quoted(&b.name))
    }

    pub(crate) fn sub_board(&self, board: BoardId, id: SubBoardId) -> String {
        let find = |s: &Schedule| {
            s.board(board)
                .and_then(|b| b.sub_boards.iter().find(|x| x.id == id))
                .map(|x| quoted(&x.name))
        };
        find(self.b)
            .or_else(|| find(self.a))
            .unwrap_or(format!("sub-board {}", id.0))
    }

    /// `"Main" (Board "Board 1")`.
    pub(crate) fn sub_board_title(&self, board: BoardId, id: SubBoardId) -> String {
        format!(
            "Sub-board {} (board {})",
            self.sub_board(board, id),
            self.board(board)
        )
    }

    /// A list on a board: `Main` or `the Boneyard`.
    pub(crate) fn list(&self, board: BoardId, list: Option<SubBoardId>) -> String {
        match list {
            Some(id) => self.sub_board(board, id),
            None => "the Boneyard".to_string(),
        }
    }

    /// A strip as a person names it: `scene 12`, `banner "Move"`, `day break`.
    pub(crate) fn strip(&self, st: &Strip) -> String {
        match &st.kind {
            StripKind::Sheet(id) => self.sheet(*id),
            StripKind::Banner(b) => format!("banner {}", quoted(&b.text)),
            StripKind::DayBreak => "day break".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Where strips are

/// Every strip of a board with its list (a sub-board, `None` for the Boneyard) and index.
pub(crate) fn strip_places(b: &Board) -> HashMap<StripId, (Option<SubBoardId>, usize, &Strip)> {
    let mut out = HashMap::new();
    for sb in &b.sub_boards {
        for (i, st) in sb.strips.iter().enumerate() {
            out.insert(st.id, (Some(sb.id), i, st));
        }
    }
    for (i, st) in b.boneyard.iter().enumerate() {
        out.insert(st.id, (None, i, st));
    }
    out
}

/// The strips of one list of a board.
pub(crate) fn list_of(b: &Board, list: Option<SubBoardId>) -> &[Strip] {
    match list {
        None => &b.boneyard,
        Some(id) => b
            .sub_boards
            .iter()
            .find(|x| x.id == id)
            .map_or(&[][..], |x| &x.strips[..]),
    }
}

/// Where strip `i` of a list is, in words: `position 5, day 3`, `position 5, not
/// scheduled`, or (a day break) `position 5, closing day 3`.
fn position(strips: &[Strip], i: usize, sub_board: bool) -> String {
    if !sub_board {
        return format!("position {}", i + 1);
    }
    let breaks_before = strips[..i]
        .iter()
        .filter(|s| s.kind == StripKind::DayBreak)
        .count();
    let day = breaks_before + 1;
    if strips[i].kind == StripKind::DayBreak {
        return format!("position {}, closing day {day}", i + 1);
    }
    if strips[i..].iter().any(|s| s.kind == StripKind::DayBreak) {
        format!("position {}, day {day}", i + 1)
    } else {
        format!("position {}, not scheduled", i + 1)
    }
}

// ---------------------------------------------------------------------------
// The diff

/// What changed from `a` (older) to `b` (newer).
pub fn diff(a: &Schedule, b: &Schedule) -> Diff {
    let names = Names { a, b };
    let mut out = Vec::new();
    if a.title != b.title {
        let fc = FieldChange {
            field: "title",
            before: quoted(&a.title),
            after: quoted(&b.title),
            text: format!("title {} -> {}", quoted(&a.title), quoted(&b.title)),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Title,
            text: format!("Title {} -> {}", fc.before, fc.after),
            fields: vec![fc],
        });
    }
    setup(a, b, &mut out);
    if a.layouts != b.layouts {
        let fc = FieldChange {
            field: "layouts",
            before: a.layouts.text(a),
            after: b.layouts.text(b),
            text: format!("layouts {} -> {}", a.layouts.text(a), b.layouts.text(b)),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Layouts,
            text: format!("Layouts {} -> {}", fc.before, fc.after),
            fields: vec![fc],
        });
    }
    if a.strip_colors != b.strip_colors {
        let fc = FieldChange {
            field: "strip_colors",
            before: a.strip_colors.text(),
            after: b.strip_colors.text(),
            text: format!(
                "strip colors {} -> {}",
                a.strip_colors.text(),
                b.strip_colors.text()
            ),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::StripColors,
            text: format!("Strip colors {} -> {}", fc.before, fc.after),
            fields: vec![fc],
        });
    }
    plan(a, b, &mut out);
    if a.calendar_fields != b.calendar_fields {
        let (before, after) = (
            calendar_fields_text(&a.calendar_fields),
            calendar_fields_text(&b.calendar_fields),
        );
        let text = format!("calendar strip fields {before} -> {after}");
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "calendar_fields",
                before,
                after,
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    if a.calendar_hidden != b.calendar_hidden {
        let (before, after) = (
            calendar_hidden_text(&a.calendar_hidden),
            calendar_hidden_text(&b.calendar_hidden),
        );
        let text = format!("calendar view {before} -> {after}");
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "calendar_hidden",
                before,
                after,
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    if a.sheet_looks != b.sheet_looks {
        let (before, after) = (a.sheet_looks.text(), b.sheet_looks.text());
        let text = format!("sheet fields {before} -> {after}");
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "sheet_looks",
                before,
                after,
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    if a.sheet_fields != b.sheet_fields {
        let (before, after) = (
            field_order_text(&a.sheet_fields),
            field_order_text(&b.sheet_fields),
        );
        let text = format!("sheet field order {before} -> {after}");
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "sheet_fields",
                before,
                after,
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    categories(&names, &mut out);
    red_flag_types(&names, &mut out);
    elements(&names, &mut out);
    red_flags(&names, &mut out);
    sheets(&names, &mut out);
    boards(&names, &mut out);
    Diff { changes: out }
}

/// The production info, the report list and the day out of days codes (crate::setup).
fn setup(a: &Schedule, b: &Schedule, out: &mut Vec<Change>) {
    for field in crate::setup::Production::FIELDS {
        let (x, y) = (
            a.production.get(field).unwrap_or_default(),
            b.production.get(field).unwrap_or_default(),
        );
        if x == y {
            continue;
        }
        let label = match field {
            "company" => "production company".to_string(),
            other => crate::setup::Production::label(other).to_lowercase(),
        };
        let fc = FieldChange {
            field,
            before: quoted(x),
            after: quoted(y),
            text: format!("{label} {} -> {}", quoted(x), quoted(y)),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            text: upper_first(&fc.text),
            fields: vec![fc],
        });
    }
    for text in crate::setup::report_changes(&a.report_list(), &b.report_list()) {
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "reports",
                before: String::new(),
                after: String::new(),
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    for key in crate::setup::DoodCodes::KEYS {
        let (x, y) = (
            a.dood_codes.get(key).unwrap_or_default(),
            b.dood_codes.get(key).unwrap_or_default(),
        );
        if x == y {
            continue;
        }
        let text = format!(
            "day out of days {} code {} -> {}",
            crate::setup::key_label(key),
            quoted(x),
            quoted(y)
        );
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "dood_codes",
                before: quoted(x),
                after: quoted(y),
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
    let (ha, hb) = (a.header_list(), b.header_list());
    for h in &hb {
        match ha.iter().find(|x| x.id == h.id) {
            None => header_change(out, format!("header layout {:?} added", h.name)),
            Some(x) if x != h => header_change(
                out,
                format!(
                    "header layout {} -> {}",
                    quoted(&x.text()),
                    quoted(&h.text())
                ),
            ),
            Some(_) => {}
        }
    }
    for x in ha.iter().filter(|x| !hb.iter().any(|h| h.id == x.id)) {
        header_change(out, format!("header layout {:?} removed", x.name));
    }
}

fn header_change(out: &mut Vec<Change>, text: String) {
    out.push(Change {
        kind: ChangeKind::Changed,
        subject: Subject::Setup,
        fields: vec![FieldChange {
            field: "header_layouts",
            before: String::new(),
            after: String::new(),
            text: text.clone(),
        }],
        text: upper_first(&text),
    });
}

/// The planning data (crate::plan), one plain phrase per change.
fn plan(a: &Schedule, b: &Schedule, out: &mut Vec<Change>) {
    for text in crate::plan::changes(&a.plan, &b.plan) {
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: Subject::Setup,
            fields: vec![FieldChange {
                field: "plan",
                before: String::new(),
                after: String::new(),
                text: text.clone(),
            }],
            text: upper_first(&text),
        });
    }
}

fn categories(n: &Names, out: &mut Vec<Change>) {
    let ids = |s: &Schedule| s.categories.iter().map(|c| c.id).collect::<Vec<_>>();
    let moved = moved_in(&ids(n.a), &ids(n.b));
    let pos = |s: &Schedule, id| s.categories.iter().position(|c| c.id == id).unwrap() + 1;
    for c in &n.b.categories {
        let head = format!("Category {}", quoted(&c.name));
        let Some(old) = n.a.category(c.id) else {
            out.push(Change {
                kind: ChangeKind::Added,
                subject: Subject::Category(c.id),
                fields: Vec::new(),
                text: format!("{head} added"),
            });
            continue;
        };
        let fields = field_changes(&category_fields(old), &category_fields(c));
        if moved.contains(&c.id) {
            let mv = format!(
                "moved from position {} to {}",
                pos(n.a, c.id),
                pos(n.b, c.id)
            );
            let text = if fields.is_empty() {
                format!("{head} {mv}")
            } else {
                format!("{}; {mv}", sentence(&head, &fields))
            };
            out.push(Change {
                kind: ChangeKind::Moved,
                subject: Subject::Category(c.id),
                fields,
                text,
            });
        } else if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject: Subject::Category(c.id),
                text: sentence(&head, &fields),
                fields,
            });
        }
    }
    for c in &n.a.categories {
        if n.b.category(c.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::Category(c.id),
                fields: Vec::new(),
                text: format!("Category {} removed", quoted(&c.name)),
            });
        }
    }
}

fn elements(n: &Names, out: &mut Vec<Change>) {
    let cat = |id| n.category(id);
    let name = |id| n.element(id);
    for e in &n.b.elements {
        let head = n.element_title(e.id);
        let Some(old) = n.a.element(e.id) else {
            out.push(Change {
                kind: ChangeKind::Added,
                subject: Subject::Element(e.id),
                fields: Vec::new(),
                text: format!("{head} added"),
            });
            continue;
        };
        let mut fields = field_changes(&element_fields(old, &cat), &element_fields(e, &cat));
        fields.extend(list_change("links", "links", &old.links, &e.links, &name));
        if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject: Subject::Element(e.id),
                text: sentence(&head, &fields),
                fields,
            });
        }
    }
    for e in &n.a.elements {
        if n.b.element(e.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::Element(e.id),
                fields: Vec::new(),
                text: format!("{} removed", n.element_title(e.id)),
            });
        }
    }
}

fn red_flag_types(n: &Names, out: &mut Vec<Change>) {
    let ids = |s: &Schedule| s.red_flag_types.iter().map(|c| c.id).collect::<Vec<_>>();
    let moved = moved_in(&ids(n.a), &ids(n.b));
    let pos = |s: &Schedule, id| s.red_flag_types.iter().position(|c| c.id == id).unwrap() + 1;
    for t in &n.b.red_flag_types {
        let head = format!("Red flag type {}", quoted(&t.name));
        let Some(old) = n.a.red_flag_type(t.id) else {
            out.push(Change {
                kind: ChangeKind::Added,
                subject: Subject::RedFlagType(t.id),
                fields: Vec::new(),
                text: format!("{head} added"),
            });
            continue;
        };
        let fields = field_changes(
            &[
                f("name", "name", quoted(&old.name)),
                f("color", "color", color_text(old.color)),
                f("builtin", "built in", builtin_text(old.builtin.as_ref())),
            ],
            &[
                f("name", "name", quoted(&t.name)),
                f("color", "color", color_text(t.color)),
                f("builtin", "built in", builtin_text(t.builtin.as_ref())),
            ],
        );
        if moved.contains(&t.id) {
            let mv = format!(
                "moved from position {} to {}",
                pos(n.a, t.id),
                pos(n.b, t.id)
            );
            let text = if fields.is_empty() {
                format!("{head} {mv}")
            } else {
                format!("{}; {mv}", sentence(&head, &fields))
            };
            out.push(Change {
                kind: ChangeKind::Moved,
                subject: Subject::RedFlagType(t.id),
                fields,
                text,
            });
        } else if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject: Subject::RedFlagType(t.id),
                text: sentence(&head, &fields),
                fields,
            });
        }
    }
    for t in &n.a.red_flag_types {
        if n.b.red_flag_type(t.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::RedFlagType(t.id),
                fields: Vec::new(),
                text: format!("Red flag type {} removed", quoted(&t.name)),
            });
        }
    }
}

/// Whether a red flag type is built in, and as which event kind (decision 113): `no`, or
/// the kind's key (`travel`).
pub(crate) fn builtin_text(k: Option<&EventKind>) -> String {
    k.map_or_else(|| "no".to_string(), |k| k.key().to_string())
}

/// A red flag type's name for a sentence, from either side.
fn type_name(n: &Names, id: RedFlagTypeId) -> String {
    n.b.red_flag_type(id)
        .or_else(|| n.a.red_flag_type(id))
        .map_or(format!("red flag type {}", id.0), |t| quoted(&t.name))
}

fn red_flag_fields(n: &Names, e: ElementId, fl: &RedFlag) -> Vec<Field> {
    vec![
        f("element", "element", {
            let t = n.element_title(e);
            t.strip_prefix("Element ").unwrap_or(&t).to_string()
        }),
        f("type", "type", type_name(n, fl.kind)),
        f("dates", "dates", fl.dates.text()),
        f("note", "note", quoted(&fl.note)),
        f("hard", "hard", on_off(fl.hard)),
    ]
}

/// Red flags by id, wherever they are: element by element in `b`'s order, then what went.
fn red_flags(n: &Names, out: &mut Vec<Change>) {
    // `Red flag "Unavailable" (2027-03-02 to 2027-03-04) on "ADA" (Cast)`.
    let head = |e: ElementId, fl: &RedFlag| {
        let title = n.element_title(e);
        let on = title.strip_prefix("Element ").unwrap_or(&title);
        format!(
            "Red flag {} ({}) on {on}",
            type_name(n, fl.kind),
            fl.dates.text()
        )
    };
    for e in &n.b.elements {
        for fl in &e.red_flags {
            let Some((old_e, old)) = n.a.red_flag(fl.id) else {
                out.push(Change {
                    kind: ChangeKind::Added,
                    subject: Subject::RedFlag(e.id, fl.id),
                    fields: Vec::new(),
                    text: format!("{} added", head(e.id, fl)),
                });
                continue;
            };
            let fields = field_changes(
                &red_flag_fields(n, old_e, old),
                &red_flag_fields(n, e.id, fl),
            );
            if !fields.is_empty() {
                out.push(Change {
                    kind: ChangeKind::Changed,
                    subject: Subject::RedFlag(e.id, fl.id),
                    text: sentence(&head(old_e, old), &fields),
                    fields,
                });
            }
        }
    }
    for e in &n.a.elements {
        for fl in &e.red_flags {
            if n.b.red_flag(fl.id).is_none() {
                out.push(Change {
                    kind: ChangeKind::Removed,
                    subject: Subject::RedFlag(e.id, fl.id),
                    fields: Vec::new(),
                    text: format!("{} removed", head(e.id, fl)),
                });
            }
        }
    }
}

fn sheets(n: &Names, out: &mut Vec<Change>) {
    let name = |id| n.element(id);
    for s in &n.b.sheets {
        let head = upper_first(&sheet_name(s));
        let Some(old) = n.a.sheet(s.id) else {
            out.push(Change {
                kind: ChangeKind::Added,
                subject: Subject::Sheet(s.id),
                fields: Vec::new(),
                text: format!("{head} added"),
            });
            continue;
        };
        let mut fields = field_changes(&sheet_fields(old), &sheet_fields(s));
        fields.extend(list_change(
            "elements",
            "elements",
            &old.elements,
            &s.elements,
            &name,
        ));
        if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject: Subject::Sheet(s.id),
                text: sentence(&head, &fields),
                fields,
            });
        }
    }
    for s in &n.a.sheets {
        if n.b.sheet(s.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::Sheet(s.id),
                fields: Vec::new(),
                text: format!("{} removed", upper_first(&sheet_name(s))),
            });
        }
    }
}

fn boards(n: &Names, out: &mut Vec<Change>) {
    let ids = |s: &Schedule| s.boards.iter().map(|b| b.id).collect::<Vec<_>>();
    let moved = moved_in(&ids(n.a), &ids(n.b));
    let pos = |s: &Schedule, id| s.boards.iter().position(|b| b.id == id).unwrap() + 1;
    for b in &n.b.boards {
        let head = format!("Board {}", quoted(&b.name));
        let Some(old) = n.a.board(b.id) else {
            out.push(Change {
                kind: ChangeKind::Added,
                subject: Subject::Board(b.id),
                fields: Vec::new(),
                text: format!("{head} added"),
            });
            continue;
        };
        let fields = field_changes(
            &[
                f("name", "name", quoted(&old.name)),
                f(
                    "strip_layout",
                    "strip layout",
                    quoted(&old.strip_layout.text(n.a)),
                ),
                f("day_look", "day breaks", old.day_look.text()),
                f("uses", "layouts", old.uses.text(&n.a.layouts)),
            ],
            &[
                f("name", "name", quoted(&b.name)),
                f(
                    "strip_layout",
                    "strip layout",
                    quoted(&b.strip_layout.text(n.b)),
                ),
                f("day_look", "day breaks", b.day_look.text()),
                f("uses", "layouts", b.uses.text(&n.b.layouts)),
            ],
        );
        if moved.contains(&b.id) {
            let mv = format!(
                "moved from position {} to {}",
                pos(n.a, b.id),
                pos(n.b, b.id)
            );
            let text = if fields.is_empty() {
                format!("{head} {mv}")
            } else {
                format!("{}; {mv}", sentence(&head, &fields))
            };
            out.push(Change {
                kind: ChangeKind::Moved,
                subject: Subject::Board(b.id),
                fields,
                text,
            });
        } else if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject: Subject::Board(b.id),
                text: sentence(&head, &fields),
                fields,
            });
        }
        sub_boards(n, old, b, out);
        strips(n, old, b, out);
    }
    for b in &n.a.boards {
        if n.b.board(b.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::Board(b.id),
                fields: Vec::new(),
                text: format!("Board {} removed", quoted(&b.name)),
            });
        }
    }
}

fn sub_boards(n: &Names, a: &Board, b: &Board, out: &mut Vec<Change>) {
    let ids = |x: &Board| x.sub_boards.iter().map(|s| s.id).collect::<Vec<_>>();
    let moved = moved_in(&ids(a), &ids(b));
    let pos = |x: &Board, id| x.sub_boards.iter().position(|s| s.id == id).unwrap() + 1;
    let find = |x: &Board, id| x.sub_boards.iter().find(|s| s.id == id).cloned();
    for sb in &b.sub_boards {
        let head = n.sub_board_title(b.id, sb.id);
        let subject = Subject::SubBoard(b.id, sb.id);
        let Some(old) = find(a, sb.id) else {
            let start = sb
                .start
                .map_or(String::new(), |d| format!(", starting {d}"));
            out.push(Change {
                kind: ChangeKind::Added,
                subject,
                fields: Vec::new(),
                text: format!("{head} added{start}"),
            });
            continue;
        };
        let fields = field_changes(
            &[
                f("name", "name", quoted(&old.name)),
                f("start", "start", opt(old.start, |d| d.to_string())),
            ],
            &[
                f("name", "name", quoted(&sb.name)),
                f("start", "start", opt(sb.start, |d| d.to_string())),
            ],
        );
        if moved.contains(&sb.id) {
            let mv = format!("moved from position {} to {}", pos(a, sb.id), pos(b, sb.id));
            let text = if fields.is_empty() {
                format!("{head} {mv}")
            } else {
                format!("{}; {mv}", sentence(&head, &fields))
            };
            out.push(Change {
                kind: ChangeKind::Moved,
                subject,
                fields,
                text,
            });
        } else if !fields.is_empty() {
            out.push(Change {
                kind: ChangeKind::Changed,
                subject,
                text: sentence(&head, &fields),
                fields,
            });
        }
        calendar(n, b.id, &old, sb, out);
    }
    for sb in &a.sub_boards {
        if find(b, sb.id).is_none() {
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::SubBoard(b.id, sb.id),
                fields: Vec::new(),
                text: format!("{} removed", n.sub_board_title(b.id, sb.id)),
            });
        }
    }
}

fn calendar(n: &Names, board: BoardId, a: &SubBoard, b: &SubBoard, out: &mut Vec<Change>) {
    let head = format!(
        "{} calendar (board {})",
        n.sub_board(board, b.id),
        n.board(board)
    );
    let at = |d: Option<crate::Date>| Subject::Calendar(board, b.id, d);
    let (ca, cb) = (&a.calendar, &b.calendar);
    if ca.weekly_off != cb.weekly_off {
        let fc = FieldChange {
            field: "weekly_off",
            before: weekdays_text(&ca.weekly_off),
            after: weekdays_text(&cb.weekly_off),
            text: format!(
                "weekly days off {} -> {}",
                weekdays_text(&ca.weekly_off),
                weekdays_text(&cb.weekly_off)
            ),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: at(None),
            text: format!("{head}: {}", fc.text),
            fields: vec![fc],
        });
    }
    for (field, label, x, y) in [
        ("prep_start", "prep start", ca.prep_start, cb.prep_start),
        ("wrap_end", "wrap end", ca.wrap_end, cb.wrap_end),
    ] {
        if x == y {
            continue;
        }
        let show = |d: Option<crate::Date>| d.map_or("none".to_string(), |d| d.to_string());
        let fc = FieldChange {
            field,
            before: show(x),
            after: show(y),
            text: format!("{label} {} -> {}", show(x), show(y)),
        };
        out.push(Change {
            kind: ChangeKind::Changed,
            subject: at(y.or(x)),
            text: format!("{head}: {}", fc.text),
            fields: vec![fc],
        });
    }
    let mut dates = marked_dates(ca);
    dates.extend(marked_dates(cb));
    for d in dates {
        let (ma, mb) = (Mark::of(ca, d), Mark::of(cb, d));
        if ma == mb {
            continue;
        }
        let fc = FieldChange {
            field: "date",
            before: Mark::name(ma).to_string(),
            after: Mark::name(mb).to_string(),
            text: format!("{d} {} -> {}", Mark::name(ma), Mark::name(mb)),
        };
        let (kind, text) = match (ma, mb) {
            (None, Some(m)) => (
                ChangeKind::Added,
                format!("{head}: {} {d} added", Mark::name(Some(m))),
            ),
            (Some(m), None) => (
                ChangeKind::Removed,
                format!("{head}: {} {d} removed", Mark::name(Some(m))),
            ),
            _ => (ChangeKind::Changed, format!("{head}: {}", fc.text)),
        };
        out.push(Change {
            kind,
            subject: at(Some(d)),
            fields: vec![fc],
            text,
        });
    }
    let name = |id| n.element(id);
    let (gone, new) = event_changes(&ca.events, &cb.events);
    for ev in new {
        out.push(Change {
            kind: ChangeKind::Added,
            subject: at(Some(ev.from)),
            fields: Vec::new(),
            text: format!("{head}: event {} added", event_text(ev, name)),
        });
    }
    for ev in gone {
        out.push(Change {
            kind: ChangeKind::Removed,
            subject: at(Some(ev.from)),
            fields: Vec::new(),
            text: format!("{head}: event {} removed", event_text(ev, name)),
        });
    }
}

/// Events compared by value, as many times as each appears: those only in `a` and those
/// only in `b`, each in its own list's order.
pub(crate) fn event_changes<'a>(
    a: &'a [Event],
    b: &'a [Event],
) -> (Vec<&'a Event>, Vec<&'a Event>) {
    let unmatched = |x: &'a [Event], y: &'a [Event]| {
        let mut used = vec![false; y.len()];
        let mut out = Vec::new();
        for ev in x {
            match (0..y.len()).find(|&j| !used[j] && y[j] == *ev) {
                Some(j) => used[j] = true,
                None => out.push(ev),
            }
        }
        out
    };
    (unmatched(a, b), unmatched(b, a))
}

fn strips(n: &Names, a: &Board, b: &Board, out: &mut Vec<Change>) {
    let pa = strip_places(a);
    let pb = strip_places(b);
    let board = n.board(b.id);
    let lists: Vec<Option<SubBoardId>> = b
        .sub_boards
        .iter()
        .map(|s| Some(s.id))
        .chain(std::iter::once(None))
        .collect();
    for list in lists {
        let now = list_of(b, list);
        let before = list_of(a, list);
        let ids = |l: &[Strip]| l.iter().map(|s| s.id).collect::<Vec<_>>();
        let moved = moved_in(&ids(before), &ids(now));
        let here = n.list(b.id, list);
        for (i, st) in now.iter().enumerate() {
            let what = n.strip(st);
            let subject = Subject::Strip(b.id, st.id);
            let at_b = position(now, i, list.is_some());
            let Some(&(old_list, j, old)) = pa.get(&st.id) else {
                out.push(Change {
                    kind: ChangeKind::Added,
                    subject,
                    fields: Vec::new(),
                    text: format!("Board {board}: {what} added on {here} at {at_b}"),
                });
                continue;
            };
            let mut fields = field_changes(&strip_fields(old), &strip_fields(st));
            if old.kind != st.kind && fields.is_empty() {
                fields.push(FieldChange {
                    field: "kind",
                    before: n.strip(old),
                    after: what.clone(),
                    text: format!("{} -> {what}", n.strip(old)),
                });
            }
            let old_strips = list_of(a, old_list);
            let at_a = position(old_strips, j, old_list.is_some());
            let mv = if old_list != list {
                Some(format!(
                    "moved from {} ({at_a}) to {here} ({at_b})",
                    n.list(b.id, old_list)
                ))
            } else if moved.contains(&st.id) {
                Some(format!("moved on {here} from {at_a} to {at_b}"))
            } else {
                None
            };
            let head = format!("Board {board}: {what}");
            match mv {
                Some(mv) => {
                    let text = if fields.is_empty() {
                        format!("{head} {mv}")
                    } else {
                        format!("{}; {mv}", sentence(&head, &fields))
                    };
                    out.push(Change {
                        kind: ChangeKind::Moved,
                        subject,
                        fields,
                        text,
                    });
                }
                None if !fields.is_empty() => out.push(Change {
                    kind: ChangeKind::Changed,
                    subject,
                    text: sentence(&head, &fields),
                    fields,
                }),
                None => {}
            }
        }
    }
    // What went, in the older board's order. A sheet strip that went with its sheet is
    // said by the sheet's own change.
    let lists_a: Vec<Option<SubBoardId>> = a
        .sub_boards
        .iter()
        .map(|s| Some(s.id))
        .chain(std::iter::once(None))
        .collect();
    for list in lists_a {
        let before = list_of(a, list);
        for (i, st) in before.iter().enumerate() {
            if pb.contains_key(&st.id) {
                continue;
            }
            if let StripKind::Sheet(sh) = st.kind {
                if n.b.sheet(sh).is_none() {
                    continue;
                }
            }
            out.push(Change {
                kind: ChangeKind::Removed,
                subject: Subject::Strip(b.id, st.id),
                fields: Vec::new(),
                text: format!(
                    "Board {board}: {} removed from {} ({})",
                    n.strip(st),
                    n.list(a.id, list),
                    position(before, i, list.is_some())
                ),
            });
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Banner, DayKind, Minutes, Place};
    use crate::testgen::Rng;
    use crate::{fountain, Eighths};

    /// Three scenes on Main with a day break after each, starting 2027-03-01.
    pub(crate) fn sample() -> Schedule {
        let mut s = Schedule::new("ZTEST Lantern Creek");
        let script = fountain::parse(
            "INT. CABIN - DAY #1#\n\nADA\nHi.\n\nBEN\nHey.\n\nEXT. CREEK - NIGHT #2#\n\nADA\nCold.\n\nINT. CABIN - NIGHT #3#\n\nCARL\nWho's there?\n",
        );
        let cast = s.category_named("Cast").unwrap();
        s.import_script(&script, cast).unwrap();
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
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
        crate::days::auto_day_breaks(&mut s, b, sb, crate::days::Limit::Pages(Eighths(1))).unwrap();
        s.boards[0].sub_boards[0].start = "2027-03-01".parse().ok();
        s
    }

    fn texts(d: &Diff) -> Vec<String> {
        d.changes.iter().map(|c| c.text.clone()).collect()
    }

    #[test]
    fn a_strip_color_change_is_a_schedule_change() {
        let a = sample();
        let mut b = a.clone();
        b.strip_colors.rules.push(crate::model::StripColorRule {
            int_ext: "EXT".into(),
            time_of_day: String::new(),
            color: crate::model::Rgb(0xfff4bf),
            text: None,
        });
        let d = diff(&a, &b);
        assert_eq!(
            texts(&d),
            ["Strip colors the usual colors -> EXT / any #fff4bf"]
        );
        assert_eq!(d.changes[0].subject.group(), "Schedule");
    }

    #[test]
    fn a_strip_layout_change_is_a_board_change() {
        let a = sample();
        let mut b = a.clone();
        let id = b.boards[0].id;
        let mut layout = crate::model::StripLayout::default();
        layout.columns.truncate(2);
        b.set_strip_layout(id, layout).unwrap();
        let d = diff(&a, &b);
        assert_eq!(
            texts(&d),
            [
                "Board \"Board 1\": strip layout \"Scene, I/E, Set, Time of day, Pages, Cast IDs\" -> \"Scene, I/E\""
            ]
        );
        assert_eq!(d.changes[0].fields[0].field, "strip_layout");
    }

    #[test]
    fn nothing_changed_is_empty() {
        let s = sample();
        assert!(diff(&s, &s).is_empty());
        assert_eq!(diff(&s, &s).to_text(), "No changes.\n");
    }

    #[test]
    fn fields_are_listed_by_id() {
        let a = sample();
        let mut b = a.clone();
        b.title = "ZTEST Lantern Creek II".into();
        let cast = b.category_named("Cast").unwrap();
        let ada = b.element_named(cast, "ADA").unwrap();
        b.set_board_id(ada, Some("1")).unwrap();
        b.element_mut(ada).unwrap().rules.holds = true;
        let sh = b.sheets[0].id;
        b.sheet_mut(sh).unwrap().set = "LOG CABIN".into();
        b.sheet_mut(sh).unwrap().pages = Eighths(11);
        let carl = b.element_named(cast, "CARL").unwrap();
        b.tag(sh, carl).unwrap();
        let props = b.add_category("Props");
        b.add_element(props, "Lantern").unwrap();
        let d = diff(&a, &b);
        assert_eq!(
            texts(&d),
            [
                "Title \"ZTEST Lantern Creek\" -> \"ZTEST Lantern Creek II\"",
                "Category \"Props\" added",
                "Element \"ADA\" (Cast): board ID none -> \"1\", holds off -> on",
                "Element \"Lantern\" (Props) added",
                "Scene 1: set \"CABIN\" -> \"LOG CABIN\", pages 1/8 -> 1 3/8, elements +CARL",
            ]
        );
        assert!(!d.to_text().contains('\u{2014}'));
        // And back: removed things are named from the older side.
        let back = texts(&diff(&b, &a));
        assert!(back.contains(&"Category \"Props\" removed".to_string()));
        assert!(back.contains(&"Element \"Lantern\" (Props) removed".to_string()));
    }

    #[test]
    fn red_flags_and_their_types() {
        use crate::model::{RedFlag, RedFlagDates};
        let mut a = sample();
        let cast = a.category_named("Cast").unwrap();
        let ada = a.element_named(cast, "ADA").unwrap();
        let unavailable = a.add_red_flag_type("Unavailable").unwrap();
        let day = |s: &str| s.parse::<Date>().unwrap();
        let flag = |dates| RedFlag {
            id: RedFlagId(0),
            kind: unavailable,
            dates,
            note: String::new(),
            hard: false,
        };
        let f1 = a
            .set_red_flag(ada, flag(RedFlagDates::NotBefore(day("2027-03-10"))))
            .unwrap();
        let f2 = a
            .set_red_flag(
                ada,
                flag(RedFlagDates::Range {
                    from: day("2027-03-02"),
                    to: day("2027-03-02"),
                }),
            )
            .unwrap();
        let mut b = a.clone();
        let job = b.add_red_flag_type("Second job").unwrap();
        b.move_red_flag_type(job, 0).unwrap();
        b.set_red_flag(
            ada,
            RedFlag {
                id: f1,
                kind: unavailable,
                dates: RedFlagDates::NotAfter(day("2027-03-20")),
                note: "Wedding".into(),
                hard: true,
            },
        )
        .unwrap();
        b.delete_red_flag(f2).unwrap();
        let f3 = b
            .set_red_flag(
                ada,
                RedFlag {
                    kind: job,
                    ..flag(RedFlagDates::Range {
                        from: day("2027-03-04"),
                        to: day("2027-03-05"),
                    })
                },
            )
            .unwrap();
        let d = diff(&a, &b);
        assert_eq!(
            texts(&d),
            [
                "Red flag type \"Second job\" added",
                "Red flag \"Unavailable\" (not before 2027-03-10) on \"ADA\" (Cast): dates not before 2027-03-10 -> not after 2027-03-20, note \"\" -> \"Wedding\", hard off -> on",
                "Red flag \"Second job\" (2027-03-04 to 2027-03-05) on \"ADA\" (Cast) added",
                "Red flag \"Unavailable\" (on 2027-03-02) on \"ADA\" (Cast) removed",
            ]
        );
        assert_eq!(d.changes[1].subject, Subject::RedFlag(ada, f1));
        assert_eq!(d.changes[2].subject.key(), "red_flag");
        assert_eq!(d.changes[2].subject.group(), "Red flags");
        assert_eq!(d.changes[0].subject, Subject::RedFlagType(job));
        assert_eq!(d.changes[2].subject, Subject::RedFlag(ada, f3));
        assert_eq!(d.changes[2].kind, ChangeKind::Added);
        // Reordering types that were there on both sides is a move.
        let mut c = b.clone();
        let third = c.add_red_flag_type("Child hours").unwrap();
        let mut e = c.clone();
        e.move_red_flag_type(third, 0).unwrap();
        assert_eq!(
            texts(&diff(&c, &e)),
            ["Red flag type \"Child hours\" moved from position 3 to 1"]
        );
        // A flag that follows its element in a merge reads as moved to the survivor.
        let ben = a.element_named(cast, "BEN").unwrap();
        let mut c = a.clone();
        c.merge_elements(ben, &[ada]).unwrap();
        let moved: Vec<String> = texts(&diff(&a, &c))
            .into_iter()
            .filter(|t| t.starts_with("Red flag"))
            .collect();
        assert_eq!(
            moved,
            [
                "Red flag \"Unavailable\" (not before 2027-03-10) on \"ADA\" (Cast): element \"ADA\" (Cast) -> \"BEN\" (Cast)",
                "Red flag \"Unavailable\" (on 2027-03-02) on \"ADA\" (Cast): element \"ADA\" (Cast) -> \"BEN\" (Cast)",
            ]
        );
    }

    #[test]
    fn strip_moves_are_reported_per_strip() {
        let a = sample();
        let mut b = a.clone();
        let bid = b.boards[0].id;
        let sb = b.boards[0].sub_boards[0].id;
        // Main is 1 | 2 | 3 |. Drag scene 3 to the top.
        let s3 = b.boards[0].sub_boards[0].strips[4].id;
        b.move_strips(
            bid,
            &[s3],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let t = texts(&diff(&a, &b));
        assert_eq!(
            t,
            ["Board \"Board 1\": scene 3 moved on \"Main\" from position 5, day 3 to position 1, day 1"]
        );
        // To the Boneyard, plus a banner and a removed day break.
        let mut c = a.clone();
        let s2 = c.boards[0].sub_boards[0].strips[2].id;
        c.move_strips(
            bid,
            &[s2],
            Place {
                sub_board: None,
                index: 0,
            },
        )
        .unwrap();
        let brk = c.boards[0].sub_boards[0].strips[2].id;
        c.delete_strip(bid, brk).unwrap();
        c.insert_strip(
            bid,
            StripKind::Banner(Banner {
                text: "Company move".into(),
                duration: Minutes(30),
                ..Default::default()
            }),
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let t = texts(&diff(&a, &c));
        assert_eq!(
            t,
            [
                "Board \"Board 1\": banner \"Company move\" added on \"Main\" at position 1, day 1",
                "Board \"Board 1\": scene 2 moved from \"Main\" (position 3, day 2) to the Boneyard (position 1)",
                "Board \"Board 1\": day break removed from \"Main\" (position 4, closing day 2)",
            ]
        );
    }

    #[test]
    fn calendar_changes() {
        let a = sample();
        let mut b = a.clone();
        let cast = b.category_named("Cast").unwrap();
        let ben = b.element_named(cast, "BEN").unwrap();
        let sub = &mut b.boards[0].sub_boards[0];
        sub.start = "2027-03-08".parse().ok();
        sub.calendar
            .set_day_kind("2027-03-03".parse().unwrap(), DayKind::Holiday);
        sub.calendar
            .set_day_kind("2027-03-06".parse().unwrap(), DayKind::Work);
        sub.calendar.set_weekly_off(&[Weekday::Sun]).unwrap();
        sub.calendar.events.push(Event {
            name: "Wardrobe".into(),
            kind: EventKind::Fitting,
            from: "2027-03-02".parse().unwrap(),
            to: "2027-03-02".parse().unwrap(),
            elements: vec![ben],
            color: None,
        });
        let t = texts(&diff(&a, &b));
        assert_eq!(
            t,
            [
                "Sub-board \"Main\" (board \"Board 1\"): start 2027-03-01 -> 2027-03-08",
                "\"Main\" calendar (board \"Board 1\"): weekly days off sat, sun -> sun",
                "\"Main\" calendar (board \"Board 1\"): holiday 2027-03-03 added",
                "\"Main\" calendar (board \"Board 1\"): marked back to working 2027-03-06 added",
                "\"Main\" calendar (board \"Board 1\"): event fitting \"Wardrobe\" on 2027-03-02 (BEN) added",
            ]
        );
        let mut c = b.clone();
        c.boards[0].sub_boards[0]
            .calendar
            .set_day_kind("2027-03-03".parse().unwrap(), DayKind::Move);
        assert_eq!(
            texts(&diff(&b, &c)),
            ["\"Main\" calendar (board \"Board 1\"): 2027-03-03 holiday -> company travel day"]
        );
    }

    #[test]
    fn moved_in_keeps_the_longest_run() {
        assert_eq!(
            moved_in(&[1, 2, 3, 4, 5], &[5, 1, 2, 3, 4]),
            BTreeSet::from([5])
        );
        assert!(moved_in(&[1, 2, 3], &[1, 2, 3]).is_empty());
        assert_eq!(moved_in(&[1, 2, 3], &[3, 2, 1]).len(), 2);
    }

    /// Random edits through the engine; every one shows up in the diff, and a schedule
    /// compared with itself has none.
    #[test]
    fn property_every_edit_is_seen() {
        let mut rng = Rng::new(808);
        for case in 0..300 {
            let a = crate::merge::tests::random_schedule(&mut rng);
            assert!(diff(&a, &a).is_empty());
            let mut b = a.clone();
            for _ in 0..1 + rng.below(4) {
                crate::merge::tests::random_edit(&mut rng, &mut b);
            }
            let mut b_same = b.clone();
            b_same.next_id = a.next_id;
            let d = diff(&a, &b);
            assert_eq!(d.is_empty(), b_same == a, "case {case}: {:?}", d.to_text());
            for c in &d.changes {
                assert!(!c.text.contains('\u{2014}'), "{}", c.text);
            }
        }
    }

    /// Decision 113: adding the standard red flag types to an older schedule shows as
    /// types added, and a same-named type of its own as built in; merge carries it.
    #[test]
    fn standard_red_flag_types_show_in_diff_and_merge() {
        let mut a = sample();
        a.add_red_flag_type("Unavailable").unwrap();
        let mut b = a.clone();
        assert_eq!(b.standard_red_flag_types(), 16);
        let d = diff(&a, &b);
        let texts: Vec<&str> = d.changes.iter().map(|c| c.text.as_str()).collect();
        assert!(
            texts.contains(&"Red flag type \"Unavailable\": built in no -> unavailable"),
            "{texts:?}"
        );
        assert!(
            texts.contains(&"Red flag type \"Travel\" added"),
            "{texts:?}"
        );
        let m = crate::merge::merge(&a, &b, &a);
        assert!(m.conflicts.is_empty());
        assert_eq!(m.schedule.red_flag_types, b.red_flag_types);
    }
}
