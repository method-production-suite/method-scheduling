//! Find and replace: a typed search over one schedule, and replacing what was found in
//! the places picked.
//!
//! A search looks at text the schedule holds: sheet header fields (one, or all of them),
//! element names, category names, banner text and event names. Options are match case,
//! whole word, a field filter, and a scope: the whole schedule or one sub-board.
//! Results ([`Hit`]) are typed places with a short context string, in board order.
//!
//! [`replace`] changes only the places it is given, and only when every one of them still
//! matches the search; it works on a copy and returns the new schedule, or a [`Refusal`]
//! with nothing changed. It refuses a rename that would make two element names (or two
//! category names) the same ignoring case, a name left empty, and a line break put into
//! a one-line field. The cast category can be renamed like any other (behavior note 17,
//! A10). The result always passes
//! `Schedule::check`.
//!
//! An advanced search adds conditions on sheet fields ([`Condition`]: contains, is,
//! starts with, is empty, greater or less than, and their opposites), joined by AND or
//! OR. With conditions, only the sheets that meet them are searched, and with no text,
//! each such sheet is one result (decision 77).
//!
//! The text matching is Method Budget's (its `find.rs`, decisions entry 26 there): case
//! folding a character at a time, and a whole word means the match doesn't run on into
//! letters, digits or `_` on either side.

use std::collections::HashSet;
use std::fmt;

use crate::model::{
    natural_cmp, same_name, BoardId, CategoryId, ElementId, Error, Schedule, Sheet, SheetId,
    StripId, StripKind, SubBoardId,
};

/// The free-text header fields of a breakdown sheet, in the sheet's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SheetField {
    Number,
    Scenes,
    IntExt,
    Set,
    TimeOfDay,
    Synopsis,
    Location,
    ScriptPages,
    ScriptDay,
    Unit,
    Sequence,
    Episode,
    Comments,
}

impl SheetField {
    pub const ALL: [SheetField; 13] = [
        SheetField::Number,
        SheetField::Scenes,
        SheetField::IntExt,
        SheetField::Set,
        SheetField::TimeOfDay,
        SheetField::Synopsis,
        SheetField::Location,
        SheetField::ScriptPages,
        SheetField::ScriptDay,
        SheetField::Unit,
        SheetField::Sequence,
        SheetField::Episode,
        SheetField::Comments,
    ];

    /// The field's name in the file format, the CLI and the MCP tools.
    pub fn key(self) -> &'static str {
        match self {
            SheetField::Number => "number",
            SheetField::Scenes => "scenes",
            SheetField::IntExt => "int_ext",
            SheetField::Set => "set",
            SheetField::TimeOfDay => "time_of_day",
            SheetField::Synopsis => "synopsis",
            SheetField::Location => "location",
            SheetField::ScriptPages => "script_pages",
            SheetField::ScriptDay => "script_day",
            SheetField::Unit => "unit",
            SheetField::Sequence => "sequence",
            SheetField::Episode => "episode",
            SheetField::Comments => "comments",
        }
    }

    pub fn parse(key: &str) -> Option<SheetField> {
        SheetField::ALL.into_iter().find(|f| f.key() == key)
    }

    /// A one-value field: the reference app's pick lists (set, location, I/E, time of day,
    /// unit, sequence, episode, script day; `reports::HeaderField`). Its value is a whole,
    /// shared with other sheets, so a replace leaves it trimmed and a blank one unlinks it
    /// (decision 189).
    pub fn one_value(self) -> bool {
        matches!(
            self,
            SheetField::Set
                | SheetField::Location
                | SheetField::IntExt
                | SheetField::TimeOfDay
                | SheetField::Unit
                | SheetField::Sequence
                | SheetField::Episode
                | SheetField::ScriptDay
        )
    }

    /// Synopsis and comments may run over several lines; every other field is one line.
    pub fn multi_line(self) -> bool {
        matches!(self, SheetField::Synopsis | SheetField::Comments)
    }

    pub fn get(self, h: &Sheet) -> &str {
        match self {
            SheetField::Number => &h.number,
            SheetField::Scenes => &h.scenes,
            SheetField::IntExt => &h.int_ext,
            SheetField::Set => &h.set,
            SheetField::TimeOfDay => &h.time_of_day,
            SheetField::Synopsis => &h.synopsis,
            SheetField::Location => &h.location,
            SheetField::ScriptPages => &h.script_pages,
            SheetField::ScriptDay => &h.script_day,
            SheetField::Unit => &h.unit,
            SheetField::Sequence => &h.sequence,
            SheetField::Episode => &h.episode,
            SheetField::Comments => &h.comments,
        }
    }

    fn get_mut(self, h: &mut Sheet) -> &mut String {
        match self {
            SheetField::Number => &mut h.number,
            SheetField::Scenes => &mut h.scenes,
            SheetField::IntExt => &mut h.int_ext,
            SheetField::Set => &mut h.set,
            SheetField::TimeOfDay => &mut h.time_of_day,
            SheetField::Synopsis => &mut h.synopsis,
            SheetField::Location => &mut h.location,
            SheetField::ScriptPages => &mut h.script_pages,
            SheetField::ScriptDay => &mut h.script_day,
            SheetField::Unit => &mut h.unit,
            SheetField::Sequence => &mut h.sequence,
            SheetField::Episode => &mut h.episode,
            SheetField::Comments => &mut h.comments,
        }
    }
}

/// Which text a search looks at. No filter (`Query::field` is `None`) means all of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFilter {
    /// Sheet header fields: one, or every one (`None`).
    Sheet(Option<SheetField>),
    Element,
    Category,
    Banner,
    Event,
}

impl FieldFilter {
    /// `sheet` (every header field), a header field's key, `element`, `category`,
    /// `banner` or `event`.
    pub fn parse(key: &str) -> Option<FieldFilter> {
        Some(match key {
            "sheet" => FieldFilter::Sheet(None),
            "element" => FieldFilter::Element,
            "category" => FieldFilter::Category,
            "banner" => FieldFilter::Banner,
            "event" => FieldFilter::Event,
            k => FieldFilter::Sheet(Some(SheetField::parse(k)?)),
        })
    }

    /// Every key [`FieldFilter::parse`] takes.
    pub fn keys() -> Vec<&'static str> {
        let mut v = vec!["sheet"];
        v.extend(SheetField::ALL.iter().map(|f| f.key()));
        v.extend(["element", "category", "banner", "event"]);
        v
    }
}

/// Where a search looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// Every sheet, element, category, and every board's banners and events.
    #[default]
    Schedule,
    /// One sub-board: the sheets and banners in its strips (scheduled or not), its
    /// calendar's events, and the elements on those sheets and events with their
    /// categories.
    SubBoard {
        board: BoardId,
        sub_board: SubBoardId,
    },
    /// One board's Boneyard: the sheets of its unscheduled strips, and the elements on
    /// those sheets with their categories.
    Boneyard { board: BoardId },
}

/// A search.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// What to look for. Empty finds nothing.
    pub text: String,
    /// Capitals must match.
    pub match_case: bool,
    /// The match must not run on into letters, digits or `_` on either side.
    pub whole_word: bool,
    /// Which text to look at; `None` is all of it.
    pub field: Option<FieldFilter>,
    pub scope: Scope,
    /// Conditions a sheet must meet to be searched (an advanced search). With any, only
    /// sheet header fields are searched.
    pub conditions: Vec<Condition>,
    /// How the conditions join: every one (AND) or any one (OR).
    pub any: bool,
}

/// A field a condition looks at: a text header field, or a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondField {
    Text(SheetField),
    /// The page length, in eighths.
    Pages,
    /// Shoot time, in minutes.
    ShootTime,
    /// Screen time, in seconds.
    ScreenTime,
}

impl CondField {
    /// A text header field's key, `pages`, `shoot_time` or `screen_time`.
    pub fn parse(key: &str) -> Option<CondField> {
        Some(match key {
            "pages" => CondField::Pages,
            "shoot_time" => CondField::ShootTime,
            "screen_time" => CondField::ScreenTime,
            k => CondField::Text(SheetField::parse(k)?),
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            CondField::Text(f) => f.key(),
            CondField::Pages => "pages",
            CondField::ShootTime => "shoot_time",
            CondField::ScreenTime => "screen_time",
        }
    }

    /// Every key [`CondField::parse`] takes.
    pub fn keys() -> Vec<&'static str> {
        let mut v: Vec<&'static str> = SheetField::ALL.iter().map(|f| f.key()).collect();
        v.extend(["pages", "shoot_time", "screen_time"]);
        v
    }

    /// The number a numeric field holds on `h`: `None` when it is empty.
    fn number(self, h: &Sheet) -> Option<u32> {
        match self {
            CondField::Text(_) => None,
            CondField::Pages => (h.pages.0 > 0).then_some(h.pages.0),
            CondField::ShootTime => h.shoot_time.map(|m| m.0),
            CondField::ScreenTime => h.screen_time.map(|s| s.0),
        }
    }
}

/// How a condition compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Contains,
    NotContains,
    Is,
    IsNot,
    StartsWith,
    Empty,
    NotEmpty,
    /// Greater than: a number, or text compared naturally (scene 12 is after 9).
    Greater,
    /// Less than, the same way.
    Less,
}

impl Op {
    pub const KEYS: [&'static str; 9] = [
        "contains",
        "not_contains",
        "is",
        "is_not",
        "starts_with",
        "empty",
        "not_empty",
        "greater",
        "less",
    ];

    pub fn parse(key: &str) -> Option<Op> {
        Some(match key {
            "contains" => Op::Contains,
            "not_contains" => Op::NotContains,
            "is" => Op::Is,
            "is_not" => Op::IsNot,
            "starts_with" => Op::StartsWith,
            "empty" => Op::Empty,
            "not_empty" => Op::NotEmpty,
            "greater" => Op::Greater,
            "less" => Op::Less,
            _ => return None,
        })
    }

    /// Whether the operator needs a value.
    pub fn takes_value(self) -> bool {
        !matches!(self, Op::Empty | Op::NotEmpty)
    }
}

/// One row of an advanced search: `field op value`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub field: CondField,
    pub op: Op,
    /// As typed (trimmed); for a numeric field, read into `number`.
    pub value: String,
    number: Option<u32>,
}

/// A number typed for a numeric field: pages as `2`, `3/8` or `1 3/8` (in eighths), a
/// shoot time as `h:mm` and a screen time as `m:ss` (or a whole number of minutes or
/// seconds). Integers only.
fn read_number(field: CondField, v: &str) -> Option<u32> {
    let colon = |s: &str| -> Option<u32> {
        match s.split_once(':') {
            Some((a, b)) if b.len() == 2 => {
                let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
                (b < 60).then(|| a.checked_mul(60)?.checked_add(b))?
            }
            Some(_) => None,
            None if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().ok(),
            None => None,
        }
    };
    match field {
        CondField::Text(_) => None,
        CondField::Pages => v.parse::<crate::Eighths>().ok().map(|e| e.0),
        CondField::ShootTime | CondField::ScreenTime => colon(v),
    }
}

impl Condition {
    /// A condition, its value checked: a numeric field needs a number it can read
    /// ([`read_number`]) for every operator that takes a value, and Contains, Starts
    /// with and their like don't apply to numbers. The error is a plain sentence.
    pub fn new(field: CondField, op: Op, value: &str) -> Result<Condition, String> {
        let value = value.trim().to_string();
        let mut number = None;
        if let CondField::Text(_) = field {
            if op.takes_value() && value.is_empty() && op != Op::Is && op != Op::IsNot {
                return Err("type a value for each condition, or pick is empty".to_string());
            }
        } else if op.takes_value() {
            if matches!(op, Op::Contains | Op::NotContains | Op::StartsWith) {
                return Err(format!(
                    "{} is a number: use is, is not, greater than or less than",
                    field.key()
                ));
            }
            number = Some(read_number(field, &value).ok_or_else(|| match field {
                CondField::Pages => format!("{value:?} is not a page length like 2 or 1 3/8"),
                CondField::ShootTime => format!("{value:?} is not a shoot time like 1:30"),
                _ => format!("{value:?} is not a screen time like 2:05"),
            })?);
        }
        Ok(Condition {
            field,
            op,
            value,
            number,
        })
    }

    /// Whether sheet `h` meets the condition. Text compares trimmed, ignoring case
    /// unless `match_case`.
    pub fn meets(&self, h: &Sheet, match_case: bool) -> bool {
        match self.field {
            CondField::Text(f) => {
                let have = f.get(h).trim();
                let fold = |s: &str| {
                    if match_case {
                        s.to_string()
                    } else {
                        s.to_lowercase()
                    }
                };
                let (x, v) = (fold(have), fold(&self.value));
                match self.op {
                    Op::Contains => x.contains(&v),
                    Op::NotContains => !x.contains(&v),
                    Op::Is => x == v,
                    Op::IsNot => x != v,
                    Op::StartsWith => x.starts_with(&v),
                    Op::Empty => x.is_empty(),
                    Op::NotEmpty => !x.is_empty(),
                    Op::Greater => !x.is_empty() && natural_cmp(have, &self.value).is_gt(),
                    Op::Less => !x.is_empty() && natural_cmp(have, &self.value).is_lt(),
                }
            }
            field => {
                let have = field.number(h);
                let want = self.number.unwrap_or(0);
                match self.op {
                    Op::Empty => have.is_none(),
                    Op::NotEmpty => have.is_some(),
                    // An empty number meets no comparison, as an empty text meets no
                    // Greater or Less: only Empty finds it.
                    Op::Is => have.is_some_and(|n| n == want),
                    Op::IsNot => have.is_some_and(|n| n != want),
                    Op::Greater => have.is_some_and(|n| n > want),
                    Op::Less => have.is_some_and(|n| n < want),
                    _ => false,
                }
            }
        }
    }
}

impl Query {
    /// Whether sheet `h` meets the conditions (true when there are none).
    pub fn sheet_qualifies(&self, h: &Sheet) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        let mut each = self.conditions.iter().map(|c| c.meets(h, self.match_case));
        if self.any {
            each.any(|x| x)
        } else {
            each.all(|x| x)
        }
    }
}

impl Query {
    /// A search for `text` everywhere, ignoring case.
    pub fn new(text: &str) -> Query {
        Query {
            text: text.to_string(),
            ..Query::default()
        }
    }
}

/// A place a search matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Sheet {
        sheet: SheetId,
        field: SheetField,
    },
    Element(ElementId),
    Category(CategoryId),
    /// A banner strip on a sub-board (banners never sit in the Boneyard).
    Banner {
        board: BoardId,
        sub_board: SubBoardId,
        strip: StripId,
    },
    /// Event `index` of a sub-board's calendar.
    Event {
        board: BoardId,
        sub_board: SubBoardId,
        index: usize,
    },
}

impl Location {
    /// The kind of place: `sheet`, `element`, `category`, `banner` or `event`.
    pub fn kind(&self) -> &'static str {
        match self {
            Location::Sheet { .. } => "sheet",
            Location::Element(_) => "element",
            Location::Category(_) => "category",
            Location::Banner { .. } => "banner",
            Location::Event { .. } => "event",
        }
    }

    /// The field: a header field's key, or `name` or `text`.
    pub fn field(&self) -> &'static str {
        match self {
            Location::Sheet { field, .. } => field.key(),
            Location::Banner { .. } => "text",
            _ => "name",
        }
    }

    fn multi_line(&self) -> bool {
        matches!(self, Location::Sheet { field, .. } if field.multi_line())
    }
}

/// One result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub location: Location,
    /// What the field holds, whole.
    pub text: String,
    /// A short line naming the place, with the matched text in view: `Scene 12 set:
    /// CABIN`, `Cast element: ADA`.
    pub context: String,
}

// ---- matching text (Method Budget's matcher)

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Matcher {
    text: String,
    needle: Vec<char>,
    fold: bool,
    whole: bool,
}

impl Matcher {
    fn new(q: &Query) -> Option<Matcher> {
        if q.text.is_empty() {
            return None;
        }
        Some(Matcher {
            text: q.text.clone(),
            needle: q.text.chars().collect(),
            fold: !q.match_case,
            whole: q.whole_word,
        })
    }

    /// Where the needle ends if it starts at byte `at`.
    fn match_at(&self, hay: &str, at: usize) -> Option<usize> {
        let mut it = hay[at..].char_indices();
        for &n in &self.needle {
            let (_, h) = it.next()?;
            if h != n && !(self.fold && h.to_lowercase().eq(n.to_lowercase())) {
                return None;
            }
        }
        Some(at + it.next().map_or(hay.len() - at, |(i, _)| i))
    }

    fn bounded(&self, hay: &str, start: usize, end: usize) -> bool {
        if !self.whole {
            return true;
        }
        let first = self.needle.first().copied().is_some_and(is_word);
        let last = self.needle.last().copied().is_some_and(is_word);
        let before = hay[..start].chars().next_back().is_some_and(is_word);
        let after = hay[end..].chars().next().is_some_and(is_word);
        !((first && before) || (last && after))
    }

    /// Byte ranges of every match in `hay`, left to right, not overlapping.
    fn ranges(&self, hay: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < hay.len() {
            let start = if self.fold {
                at
            } else {
                match hay[at..].find(&self.text) {
                    Some(i) => at + i,
                    None => break,
                }
            };
            if let Some(end) = self.match_at(hay, start) {
                if self.bounded(hay, start, end) {
                    out.push((start, end));
                    at = end;
                    continue;
                }
            }
            at = start + hay[start..].chars().next().map_or(1, char::len_utf8);
        }
        out
    }
}

/// `hay` with each range replaced by `with`.
fn splice(hay: &str, ranges: &[(usize, usize)], with: &str) -> String {
    let mut out = String::with_capacity(hay.len() + ranges.len() * with.len());
    let mut from = 0;
    for &(s, e) in ranges {
        out.push_str(&hay[from..s]);
        out.push_str(with);
        from = e;
    }
    out.push_str(&hay[from..]);
    out
}

/// Longest context value shown, in characters.
const CONTEXT_CHARS: usize = 80;

/// `value` on one line, cut to [`CONTEXT_CHARS`] around the first match.
fn excerpt(value: &str, first: (usize, usize)) -> String {
    let flat: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let chars: Vec<char> = flat.chars().collect();
    if chars.len() <= CONTEXT_CHARS {
        return flat;
    }
    // Control characters are one byte each and became one space, so byte offsets map to
    // the same char offsets in `flat` as in `value`.
    let start_char = value[..first.0].chars().count();
    let from = start_char
        .saturating_sub(20)
        .min(chars.len() - CONTEXT_CHARS);
    let to = from + CONTEXT_CHARS;
    let mut out = String::new();
    if from > 0 {
        out.push_str("...");
    }
    out.extend(&chars[from..to]);
    if to < chars.len() {
        out.push_str("...");
    }
    out
}

// ---- the walk

/// The text at `loc`, or `None` when the place is gone.
fn text_at<'a>(s: &'a Schedule, loc: &Location) -> Option<&'a str> {
    match *loc {
        Location::Sheet { sheet, field } => s.sheet(sheet).map(|h| field.get(h)),
        Location::Element(id) => s.element(id).map(|e| e.name.as_str()),
        Location::Category(id) => s.category(id).map(|c| c.name.as_str()),
        Location::Banner {
            board,
            sub_board,
            strip,
        } => s
            .board(board)?
            .sub_boards
            .iter()
            .find(|x| x.id == sub_board)?
            .strips
            .iter()
            .find(|x| x.id == strip)
            .and_then(|x| match &x.kind {
                StripKind::Banner(b) => Some(b.text.as_str()),
                _ => None,
            }),
        Location::Event {
            board,
            sub_board,
            index,
        } => s
            .board(board)?
            .sub_boards
            .iter()
            .find(|x| x.id == sub_board)?
            .calendar
            .events
            .get(index)
            .map(|e| e.name.as_str()),
    }
}

/// The words naming a place, as its context line starts: `Scene 12 set`, `Cast element`,
/// `Category`, `Banner on Main`, `Event on Unit B`.
pub fn label(s: &Schedule, loc: &Location) -> String {
    let sub_name = |board: BoardId, sub: SubBoardId| {
        s.board(board)
            .and_then(|b| b.sub_boards.iter().find(|x| x.id == sub))
            .map_or(String::new(), |x| x.name.clone())
    };
    match *loc {
        Location::Sheet { sheet, field } => {
            // Never the sheet's id (decision 162): its heading when it has no number.
            match s.sheet(sheet) {
                Some(h) if !h.scenes.trim().is_empty() => {
                    format!("Scene {} {}", h.scenes.trim(), field.key())
                }
                Some(h) => format!("{} {}", h.title(), field.key()),
                None => format!("A scene no longer in the schedule {}", field.key()),
            }
        }
        Location::Element(id) => {
            let cat = s
                .element(id)
                .and_then(|e| s.category(e.category))
                .map_or("", |c| c.name.as_str());
            format!("{cat} element")
        }
        Location::Category(_) => "Category".to_string(),
        Location::Banner {
            board, sub_board, ..
        } => format!("Banner on {}", sub_name(board, sub_board)),
        Location::Event {
            board, sub_board, ..
        } => format!("Event on {}", sub_name(board, sub_board)),
    }
}

/// Every place the filter and scope cover, in board order: for each board and each of
/// its sub-boards, the strips in order (a sheet's header fields the first time the sheet
/// is met, banners) and then the sub-board's events; then sheets met only in a
/// Boneyard; then categories in order, each followed by its elements.
fn places(s: &Schedule, q: &Query) -> Result<Vec<Location>, Error> {
    // With conditions, only the header fields of the sheets that meet them.
    let advanced = !q.conditions.is_empty();
    let want = |f: FieldFilter| match (q.field, f) {
        _ if advanced && !matches!(f, FieldFilter::Sheet(_)) => false,
        (None, _) => true,
        (Some(FieldFilter::Sheet(None)), FieldFilter::Sheet(_)) => true,
        (Some(a), b) => a == b,
    };
    let sheet_fields: Vec<SheetField> = SheetField::ALL
        .into_iter()
        .filter(|f| want(FieldFilter::Sheet(Some(*f))))
        .collect();
    let mut out = Vec::new();
    let mut seen_sheets: HashSet<SheetId> = HashSet::new();
    let mut push_sheet = |out: &mut Vec<Location>, id: SheetId| {
        if seen_sheets.insert(id) && s.sheet(id).is_some_and(|h| q.sheet_qualifies(h)) {
            out.extend(
                sheet_fields
                    .iter()
                    .map(|&field| Location::Sheet { sheet: id, field }),
            );
        }
    };

    let subs: Vec<(BoardId, &crate::model::SubBoard)> = match q.scope {
        Scope::Schedule => s
            .boards
            .iter()
            .flat_map(|b| b.sub_boards.iter().map(move |sb| (b.id, sb)))
            .collect(),
        Scope::SubBoard { board, sub_board } => {
            let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
            let sb = b
                .sub_boards
                .iter()
                .find(|x| x.id == sub_board)
                .ok_or(Error::NoSuchSubBoard(sub_board))?;
            vec![(board, sb)]
        }
        Scope::Boneyard { board } => {
            s.board(board).ok_or(Error::NoSuchBoard(board))?;
            Vec::new()
        }
    };
    for (board, sb) in &subs {
        for st in &sb.strips {
            match &st.kind {
                StripKind::Sheet(id) => push_sheet(&mut out, *id),
                StripKind::Banner(_) if want(FieldFilter::Banner) => out.push(Location::Banner {
                    board: *board,
                    sub_board: sb.id,
                    strip: st.id,
                }),
                _ => {}
            }
        }
        if want(FieldFilter::Event) {
            out.extend((0..sb.calendar.events.len()).map(|index| Location::Event {
                board: *board,
                sub_board: sb.id,
                index,
            }));
        }
    }

    // Elements in scope: all of them, or those on the sub-board's sheets and events.
    let in_scope: Option<HashSet<ElementId>> = match q.scope {
        Scope::Schedule => {
            for b in &s.boards {
                for st in &b.boneyard {
                    if let StripKind::Sheet(id) = st.kind {
                        push_sheet(&mut out, id);
                    }
                }
            }
            for h in &s.sheets {
                push_sheet(&mut out, h.id);
            }
            None
        }
        Scope::SubBoard { .. } => {
            let mut els = HashSet::new();
            for (_, sb) in &subs {
                for st in &sb.strips {
                    if let StripKind::Sheet(id) = st.kind {
                        if let Some(h) = s.sheet(id) {
                            els.extend(h.elements.iter().copied());
                        }
                    }
                }
                for ev in &sb.calendar.events {
                    els.extend(ev.elements.iter().copied());
                }
            }
            Some(els)
        }
        Scope::Boneyard { board } => {
            let mut els = HashSet::new();
            for st in s
                .board(board)
                .map(|b| b.boneyard.as_slice())
                .unwrap_or_default()
            {
                if let StripKind::Sheet(id) = st.kind {
                    push_sheet(&mut out, id);
                    if let Some(h) = s.sheet(id) {
                        els.extend(h.elements.iter().copied());
                    }
                }
            }
            Some(els)
        }
    };
    let element_in = |id: ElementId| in_scope.as_ref().is_none_or(|set| set.contains(&id));
    for c in &s.categories {
        let els: Vec<ElementId> = s
            .elements
            .iter()
            .filter(|e| e.category == c.id && element_in(e.id))
            .map(|e| e.id)
            .collect();
        if want(FieldFilter::Category) && (in_scope.is_none() || !els.is_empty()) {
            out.push(Location::Category(c.id));
        }
        if want(FieldFilter::Element) {
            out.extend(els.into_iter().map(Location::Element));
        }
    }
    Ok(out)
}

/// Every place `q` matches, in board order (see [`Scope`] for what each scope covers).
/// An empty search finds nothing. Refused only when the scope's board or sub-board is
/// missing.
pub fn find(s: &Schedule, q: &Query) -> Result<Vec<Hit>, Error> {
    let Some(m) = Matcher::new(q) else {
        // Still refuse a scope that isn't there.
        let locs = places(s, q)?;
        if q.conditions.is_empty() {
            return Ok(Vec::new());
        }
        // An advanced search with no text: each sheet that meets the conditions, once,
        // at the first condition's text field (else the first field searched).
        let first = q.conditions.iter().find_map(|c| match c.field {
            CondField::Text(f) => Some(f),
            _ => None,
        });
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for loc in locs {
            let Location::Sheet { sheet, field } = loc else {
                continue;
            };
            if !seen.insert(sheet) {
                continue;
            }
            let location = Location::Sheet {
                sheet,
                field: first.unwrap_or(field),
            };
            let text = text_at(s, &location).unwrap_or_default().to_string();
            out.push(Hit {
                location,
                context: format!("{}: {}", label(s, &location), excerpt(&text, (0, 0))),
                text,
            });
        }
        return Ok(out);
    };
    let mut out = Vec::new();
    for loc in places(s, q)? {
        let Some(text) = text_at(s, &loc) else {
            continue;
        };
        let ranges = m.ranges(text);
        let Some(&first) = ranges.first() else {
            continue;
        };
        out.push(Hit {
            location: loc,
            text: text.to_string(),
            context: format!("{}: {}", label(s, &loc), excerpt(text, first)),
        });
    }
    Ok(out)
}

// ---- replacing

/// One place a replace changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub location: Location,
    pub before: String,
    pub after: String,
}

/// What [`replace`] made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replaced {
    /// The schedule with the replacements made. Passes `Schedule::check`.
    pub schedule: Schedule,
    /// The places whose text changed, in the order picked. Empty when the replacement is
    /// what every place held already.
    pub changes: Vec<Change>,
}

/// Why a replace changed nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The search is empty.
    NothingToFind,
    /// No place was picked.
    NothingPicked,
    /// A picked place doesn't match the search (any more): search again.
    Stale(Location),
    /// The scope's board or sub-board is missing.
    Scope(Error),
    /// Two elements of one category, or two categories, would share a name (ignoring
    /// case).
    Collision {
        /// The category name, or `None` for category names.
        category: Option<String>,
        name: String,
    },
    /// An element or category name would be left empty.
    EmptyName(Location),
    /// A line break or other control character in a one-line field.
    OneLine(Location),
    /// The result wouldn't pass `Schedule::check`.
    Broken(Error),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NothingToFind => f.write_str("type what to find first"),
            Refusal::NothingPicked => f.write_str("pick one or more results to replace"),
            Refusal::Stale(_) => f.write_str(
                "some of the picked results no longer match (the schedule changed); search again",
            ),
            Refusal::Scope(e) => write!(f, "{e}"),
            Refusal::Collision {
                category: Some(c),
                name,
            } => write!(f, "{name:?} would be in {c} twice"),
            Refusal::Collision {
                category: None,
                name,
            } => write!(f, "there would be two categories called {name:?}"),
            Refusal::EmptyName(l) => write!(f, "a {} name can't be empty", l.kind()),
            Refusal::OneLine(l) => write!(
                f,
                "{} {} is one line: the replacement can't hold a line break",
                l.kind(),
                l.field()
            ),
            Refusal::Broken(e) => write!(f, "the result wouldn't hang together: {e}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Replaces what `q` matches by `with` in each of `picks` (results of [`find`] with the
/// same query), on a copy of `s`. Each picked place must still match. Element and
/// category names are trimmed after the replacement (as renaming trims them), and so is
/// a one-value header field's value ([`SheetField::one_value`]): one that comes out blank
/// is empty, so the value is unlinked from the sheet rather than kept as spaces (decision
/// 189). A place whose text comes out the same is not a change. Refused, with nothing changed, as
/// [`Refusal`] lists.
pub fn replace(
    s: &Schedule,
    q: &Query,
    with: &str,
    picks: &[Location],
) -> Result<Replaced, Refusal> {
    let m = Matcher::new(q).ok_or(Refusal::NothingToFind)?;
    if picks.is_empty() {
        return Err(Refusal::NothingPicked);
    }
    let found: HashSet<Location> = find(s, q)
        .map_err(Refusal::Scope)?
        .into_iter()
        .map(|h| h.location)
        .collect();
    let has_control = |multi: bool| {
        with.chars()
            .any(|c| c.is_control() && !(multi && (c == '\n' || c == '\t')))
    };

    // Work out every change first.
    let mut seen = HashSet::new();
    let mut changes = Vec::new();
    for loc in picks {
        if !seen.insert(*loc) {
            continue;
        }
        if !found.contains(loc) {
            return Err(Refusal::Stale(*loc));
        }
        let before = text_at(s, loc).ok_or(Refusal::Stale(*loc))?;
        let mut after = splice(before, &m.ranges(before), with);
        if after == before {
            continue;
        }
        if has_control(loc.multi_line()) {
            return Err(Refusal::OneLine(*loc));
        }
        match loc {
            Location::Element(_) | Location::Category(_) => {
                after = after.trim().to_string();
                if after.is_empty() {
                    return Err(Refusal::EmptyName(*loc));
                }
                if after == before {
                    continue;
                }
            }
            Location::Sheet { field, .. } if field.one_value() => {
                after = after.trim().to_string();
                if after == before {
                    continue;
                }
            }
            _ => {}
        }
        changes.push(Change {
            location: *loc,
            before: before.to_string(),
            after,
        });
    }

    // Then make them on a copy.
    let mut out = s.clone();
    for c in &changes {
        match c.location {
            Location::Sheet { sheet, field } => {
                if let Some(h) = out.sheet_mut(sheet) {
                    *field.get_mut(h) = c.after.clone();
                }
            }
            Location::Element(id) => {
                if let Some(e) = out.element_mut(id) {
                    e.name = c.after.clone();
                }
            }
            Location::Category(id) => {
                if let Some(cat) = out.category_mut(id) {
                    cat.name = c.after.clone();
                }
            }
            Location::Banner {
                board,
                sub_board,
                strip,
            } => {
                let st = out
                    .boards
                    .iter_mut()
                    .filter(|b| b.id == board)
                    .flat_map(|b| b.sub_boards.iter_mut())
                    .filter(|x| x.id == sub_board)
                    .flat_map(|x| x.strips.iter_mut())
                    .find(|x| x.id == strip);
                if let Some(StripKind::Banner(b)) = st.map(|x| &mut x.kind) {
                    b.text = c.after.clone();
                }
            }
            Location::Event {
                board,
                sub_board,
                index,
            } => {
                let ev = out
                    .boards
                    .iter_mut()
                    .filter(|b| b.id == board)
                    .flat_map(|b| b.sub_boards.iter_mut())
                    .find(|x| x.id == sub_board)
                    .and_then(|x| x.calendar.events.get_mut(index));
                if let Some(ev) = ev {
                    ev.name = c.after.clone();
                }
            }
        }
    }

    // Names stay unique ignoring case: elements within their category, and categories.
    let renamed: HashSet<CategoryId> = changes
        .iter()
        .filter_map(|c| match c.location {
            Location::Element(id) => out.element(id).map(|e| e.category),
            _ => None,
        })
        .collect();
    for cat in renamed {
        // The same comparison as `same_name`.
        let mut names: HashSet<String> = HashSet::new();
        for e in out.elements.iter().filter(|e| e.category == cat) {
            if !names.insert(e.name.trim().to_ascii_lowercase()) {
                return Err(Refusal::Collision {
                    category: out.category(cat).map(|c| c.name.clone()),
                    name: e.name.clone(),
                });
            }
        }
    }
    if changes
        .iter()
        .any(|c| matches!(c.location, Location::Category(_)))
    {
        for (i, c) in out.categories.iter().enumerate() {
            if out.categories[..i]
                .iter()
                .any(|o| same_name(&o.name, &c.name))
            {
                return Err(Refusal::Collision {
                    category: None,
                    name: c.name.clone(),
                });
            }
        }
    }
    out.check().map_err(Refusal::Broken)?;
    Ok(Replaced {
        schedule: out,
        changes,
    })
}

/// Every result of `q`, to replace them all.
pub fn all(s: &Schedule, q: &Query) -> Result<Vec<Location>, Error> {
    Ok(find(s, q)?.into_iter().map(|h| h.location).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::tests::random_schedule;
    use crate::model::{Banner, Event, EventKind, Minutes, Place};
    use crate::testgen::Rng;
    use crate::Date;

    /// Main with sheets 1 (CABIN, ADA and BEN), 2 (CREEK, ADA) and a banner; Unit B with
    /// sheet 3 (CABIN NIGHT, CARL) and a fitting; sheet 4 in the Boneyard.
    fn fixture() -> (Schedule, BoardId, SubBoardId, SubBoardId) {
        let mut s = Schedule::new("ZTEST Find");
        let cast = s.category_named("Cast").unwrap();
        let props = s.add_category("Props");
        let ada = s.add_element(cast, "ADA").unwrap();
        let ben = s.add_element(cast, "BEN").unwrap();
        let carl = s.add_element(cast, "CARL").unwrap();
        s.add_element(props, "Lantern").unwrap();
        let sheet = |n: &str, set: &str, els: Vec<ElementId>| Sheet {
            scenes: n.to_string(),
            int_ext: "INT".into(),
            set: set.to_string(),
            time_of_day: "DAY".into(),
            synopsis: format!("Ada walks to the {set}."),
            elements: els,
            ..Sheet::default()
        };
        let a = s.add_sheet(sheet("1", "CABIN", vec![ada, ben]));
        let b2 = s.add_sheet(sheet("2", "CREEK", vec![ada]));
        let c = s.add_sheet(sheet("3", "CABIN NIGHT", vec![carl]));
        s.add_sheet(sheet("4", "CABINET", vec![]));
        let b = s.boards[0].id;
        let main = s.boards[0].sub_boards[0].id;
        let unit = s.add_sub_board(b, "Unit B", None).unwrap();
        let ids: Vec<StripId> = s.boards[0].boneyard[..2].iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(main),
                index: 0,
            },
        )
        .unwrap();
        let _ = (a, b2);
        let third = s.boards[0]
            .boneyard
            .iter()
            .find(|x| x.kind == StripKind::Sheet(c))
            .unwrap()
            .id;
        s.move_strips(
            b,
            &[third],
            Place {
                sub_board: Some(unit),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            StripKind::Banner(Banner {
                text: "Cabin move".into(),
                duration: Minutes(30),
                ..Default::default()
            }),
            Place {
                sub_board: Some(main),
                index: 1,
            },
        )
        .unwrap();
        let d: Date = "2027-03-01".parse().unwrap();
        s.set_event(
            b,
            unit,
            None,
            Event {
                name: "Carl cabin fitting".into(),
                kind: EventKind::Fitting,
                from: d,
                to: d,
                elements: vec![carl],
                color: None,
            },
        )
        .unwrap();
        s.check().unwrap();
        (s, b, main, unit)
    }

    fn contexts(hits: &[Hit]) -> Vec<String> {
        hits.iter().map(|h| h.context.clone()).collect()
    }

    #[test]
    fn finds_in_board_order_with_context() {
        let (s, ..) = fixture();
        let hits = find(&s, &Query::new("cabin")).unwrap();
        assert_eq!(
            contexts(&hits),
            [
                "Scene 1 set: CABIN",
                "Scene 1 synopsis: Ada walks to the CABIN.",
                "Banner on Main: Cabin move",
                "Scene 3 set: CABIN NIGHT",
                "Scene 3 synopsis: Ada walks to the CABIN NIGHT.",
                "Event on Unit B: Carl cabin fitting",
                "Scene 4 set: CABINET",
                "Scene 4 synopsis: Ada walks to the CABINET.",
            ]
        );
        assert_eq!(hits[0].text, "CABIN");
        assert!(matches!(hits[2].location, Location::Banner { .. }));
    }

    /// A blank replace in a one-value header field (a pick list in the reference app)
    /// unlinks the value: the field comes out empty, not spaces, and the value leaves the
    /// header lists once no sheet has it; what is left of a partly blanked value is
    /// trimmed. A free-text field keeps its spaces (decision 189, E-S32).
    #[test]
    fn a_blank_replace_unlinks_a_one_value_field() {
        let (s, ..) = fixture();
        let set = |text: &str| Query {
            text: text.into(),
            field: Some(FieldFilter::Sheet(Some(SheetField::Set))),
            ..Query::default()
        };
        // Whole word: CREEK, the whole of scene 2's set.
        let mut creek = set("CREEK");
        creek.whole_word = true;
        let picks = all(&s, &creek).unwrap();
        let r = replace(&s, &creek, " ", &picks).unwrap();
        let two = r.schedule.sheets.iter().find(|h| h.scenes == "2").unwrap();
        assert_eq!(two.set, "", "unlinked, not a space");
        let hc = crate::reports::header_category(&r.schedule, crate::reports::HeaderField::Set);
        assert!(hc.schedule.element_named(hc.category, "CREEK").is_none());
        assert!(hc
            .schedule
            .elements
            .iter()
            .filter(|e| e.category == hc.category)
            .all(|e| !e.name.trim().is_empty()));
        // Part of a value blanked: the rest, trimmed.
        let mut night = set("NIGHT");
        night.whole_word = true;
        let picks = all(&s, &night).unwrap();
        let r = replace(&s, &night, "", &picks).unwrap();
        let three = r.schedule.sheets.iter().find(|h| h.scenes == "3").unwrap();
        assert_eq!(three.set, "CABIN");
        assert_eq!(r.changes[0].after, "CABIN");
        // The synopsis is free text: its spaces stay as the replace leaves them.
        let mut syn = Query::new("the");
        syn.field = Some(FieldFilter::Sheet(Some(SheetField::Synopsis)));
        syn.whole_word = true;
        let picks = all(&s, &syn).unwrap();
        let r = replace(&s, &syn, "", &picks).unwrap();
        assert!(
            r.changes[0].after.contains("to  "),
            "{:?}",
            r.changes[0].after
        );
        assert!(SheetField::ALL
            .iter()
            .filter(|f| f.one_value())
            .all(|f| crate::reports::HeaderField::from_key(f.key()).is_some()));
        assert_eq!(
            SheetField::ALL.iter().filter(|f| f.one_value()).count(),
            crate::reports::HeaderField::ALL.len()
        );
    }

    #[test]
    fn options_narrow_the_search() {
        let (s, b, main, unit) = fixture();
        let q = |text: &str, f: Option<FieldFilter>| Query {
            text: text.into(),
            field: f,
            ..Query::default()
        };
        // Whole word skips CABINET; match case skips the banner and event.
        let mut word = q("CABIN", Some(FieldFilter::Sheet(Some(SheetField::Set))));
        word.whole_word = true;
        assert_eq!(find(&s, &word).unwrap().len(), 2);
        let mut case = q("CABIN", None);
        case.match_case = true;
        let found = find(&s, &case).unwrap();
        assert!(found.iter().all(|h| h.text.contains("CABIN")), "{found:?}");
        assert_eq!(found.len(), 6);
        // Field filters.
        assert_eq!(
            find(&s, &q("ada", Some(FieldFilter::Element)))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            find(&s, &q("ada", Some(FieldFilter::Sheet(None))))
                .unwrap()
                .len(),
            4,
            "the synopses"
        );
        assert_eq!(
            contexts(&find(&s, &q("prop", Some(FieldFilter::Category))).unwrap()),
            ["Category: Props"]
        );
        assert_eq!(
            contexts(&find(&s, &q("lan", None)).unwrap()),
            ["Props element: Lantern"]
        );
        // Scope: one sub-board, its elements and their categories only.
        let mut sub = q("a", Some(FieldFilter::Element));
        sub.scope = Scope::SubBoard {
            board: b,
            sub_board: unit,
        };
        assert_eq!(contexts(&find(&s, &sub).unwrap()), ["Cast element: CARL"]);
        sub.field = None;
        sub.text = "cabin".into();
        let hits = find(&s, &sub).unwrap();
        assert_eq!(hits.len(), 3, "{hits:?}");
        sub.scope = Scope::SubBoard {
            board: b,
            sub_board: main,
        };
        sub.text = "props".into();
        assert!(find(&s, &sub).unwrap().is_empty(), "no prop on Main");
        sub.scope = Scope::SubBoard {
            board: b,
            sub_board: SubBoardId(9999),
        };
        assert!(find(&s, &sub).is_err());
        // The Boneyard: scene 4 (CABINET) only, and no element (it has none).
        sub.scope = Scope::Boneyard { board: b };
        sub.text = "cabin".into();
        assert_eq!(
            contexts(&find(&s, &sub).unwrap()),
            [
                "Scene 4 set: CABINET",
                "Scene 4 synopsis: Ada walks to the CABINET."
            ]
        );
        sub.text = "ada".into();
        sub.field = Some(FieldFilter::Element);
        assert!(
            find(&s, &sub).unwrap().is_empty(),
            "Ada works only on scheduled scenes"
        );
        sub.field = None;
        sub.scope = Scope::Boneyard {
            board: BoardId(9999),
        };
        assert!(find(&s, &sub).is_err());
        assert!(find(&s, &Query::new("")).unwrap().is_empty());
    }

    #[test]
    fn whole_word_and_case_match_like_the_budget() {
        let m = |text: &str, case: bool, word: bool| Matcher {
            text: text.into(),
            needle: text.chars().collect(),
            fold: !case,
            whole: word,
        };
        assert_eq!(m("ab", false, false).ranges("xABab"), [(1, 3), (3, 5)]);
        assert_eq!(m("ab", true, false).ranges("xABab"), [(3, 5)]);
        assert!(m("ab", false, true).ranges("xab ab_").is_empty());
        assert_eq!(m("ab", false, true).ranges("ab, AB"), [(0, 2), (4, 6)]);
        assert_eq!(m("é", false, false).ranges("CAFÉ"), [(3, 5)]);
    }

    #[test]
    fn long_text_is_cut_around_the_match() {
        let long = format!("{}needle{}", "x".repeat(100), "y".repeat(100));
        let at = long.find("needle").unwrap();
        let e = excerpt(&long, (at, at + 6));
        assert!(e.starts_with("...") && e.ends_with("..."), "{e}");
        assert!(e.contains("needle"));
        assert_eq!(excerpt("a\nb", (0, 1)), "a b");
    }

    #[test]
    fn replaces_picked_places_only() {
        let (s, ..) = fixture();
        let q = Query::new("cabin");
        let hits = find(&s, &q).unwrap();
        let r = replace(&s, &q, "Lodge", &[hits[0].location, hits[2].location]).unwrap();
        assert_eq!(r.changes.len(), 2);
        assert_eq!(r.schedule.sheets[0].set, "Lodge");
        assert_eq!(r.schedule.sheets[0].synopsis, "Ada walks to the CABIN.");
        assert_eq!(r.changes[1].before, "Cabin move");
        assert_eq!(r.changes[1].after, "Lodge move");
        r.schedule.check().unwrap();
        // All of them, then nothing left to find.
        let r = replace(&s, &q, "Lodge", &all(&s, &q).unwrap()).unwrap();
        assert_eq!(r.changes.len(), 8);
        assert!(find(&r.schedule, &q).unwrap().is_empty());
        // A pick that no longer matches is stale.
        assert!(matches!(
            replace(&r.schedule, &q, "x", &[hits[0].location]),
            Err(Refusal::Stale(_))
        ));
        assert_eq!(
            replace(&s, &Query::new(""), "x", &[hits[0].location]),
            Err(Refusal::NothingToFind)
        );
        assert_eq!(replace(&s, &q, "x", &[]), Err(Refusal::NothingPicked));
    }

    #[test]
    fn refuses_what_would_break_a_name_or_field() {
        let (s, ..) = fixture();
        // BEN -> ADA collides in Cast, ignoring case.
        let q = Query::new("BEN");
        let r = replace(&s, &q, "ada", &all(&s, &q).unwrap());
        assert_eq!(
            r,
            Err(Refusal::Collision {
                category: Some("Cast".into()),
                name: "ada".into()
            })
        );
        // ...but the same name in another category is fine.
        let q = Query::new("Lantern");
        let r = replace(&s, &q, "ADA", &all(&s, &q).unwrap()).unwrap();
        assert_eq!(r.changes.len(), 1);
        // Empty names, the cast category renamed, category collisions.
        let q = Query::new("CARL");
        let picks: Vec<Location> = find(&s, &q)
            .unwrap()
            .into_iter()
            .map(|h| h.location)
            .filter(|l| matches!(l, Location::Element(_)))
            .collect();
        assert!(matches!(
            replace(&s, &q, "  ", &picks),
            Err(Refusal::EmptyName(_))
        ));
        let q = Query::new("Cast");
        let r = replace(&s, &q, "Actors", &all(&s, &q).unwrap()).unwrap();
        let cast = r.schedule.cast().unwrap();
        assert_eq!(r.schedule.category(cast).unwrap().name, "Actors");
        let q = Query::new("Props");
        assert!(matches!(
            replace(&s, &q, "cast", &all(&s, &q).unwrap()),
            Err(Refusal::Collision { category: None, .. })
        ));
        // A line break goes into a synopsis but not a set.
        let q = Query {
            text: "CREEK".into(),
            match_case: true,
            ..Query::default()
        };
        let hits = find(&s, &q).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(matches!(
            replace(&s, &q, "A\nB", &[hits[0].location]),
            Err(Refusal::OneLine(_))
        ));
        let r = replace(&s, &q, "A\nB", &[hits[1].location]).unwrap();
        assert_eq!(r.schedule.sheets[1].synopsis, "Ada walks to the A\nB.");
        // Names are trimmed like a rename.
        let q = Query::new("Lantern");
        let r = replace(&s, &q, " Lamp ", &all(&s, &q).unwrap()).unwrap();
        assert_eq!(r.changes[0].after, "Lamp");
    }

    /// Find, then replace with the same text, changes nothing; replace never breaks check.
    #[test]
    fn property_same_text_is_a_no_op_and_check_always_passes() {
        const WORDS: &[&str] = &["A", "a", "ADA", "BEN", "1", "Fit", "Props", "e", "Car"];
        const WITH: &[&str] = &["", "ADA", "x", "Ben", " ", "Lamp", "Cast", "\n"];
        let mut rng = Rng::new(0xf1d);
        let (mut same_runs, mut made, mut refused) = (0, 0, 0);
        for _ in 0..300 {
            let s = random_schedule(&mut rng);
            let text = WORDS[rng.below(WORDS.len() as u64) as usize];
            let mut q = Query::new(text);
            q.match_case = rng.below(2) == 0;
            q.whole_word = rng.below(2) == 0;
            q.field = match rng.below(4) {
                0 => None,
                1 => Some(FieldFilter::Element),
                2 => Some(FieldFilter::Sheet(None)),
                _ => Some(FieldFilter::Category),
            };
            let picks = all(&s, &q).unwrap();
            if picks.is_empty() {
                continue;
            }
            // The same text, matching case: nothing changes.
            let mut same = q.clone();
            same.match_case = true;
            let same_picks = all(&s, &same).unwrap();
            if !same_picks.is_empty() {
                let r = replace(&s, &same, text, &same_picks).unwrap();
                assert!(r.changes.is_empty(), "{text}: {:?}", r.changes);
                assert_eq!(r.schedule, s);
                same_runs += 1;
            }
            // Any replacement, any subset: refused, or a schedule that passes check.
            let with = WITH[rng.below(WITH.len() as u64) as usize];
            let some: Vec<Location> = picks
                .iter()
                .copied()
                .filter(|_| rng.below(2) == 0)
                .collect();
            for p in [&picks, &some] {
                match replace(&s, &q, with, p) {
                    Ok(r) => {
                        r.schedule.check().unwrap();
                        assert_eq!(r.schedule.sheets.len(), s.sheets.len());
                        made += usize::from(!r.changes.is_empty());
                    }
                    Err(_) => refused += 1,
                }
            }
        }
        // The run exercised every outcome.
        assert!(
            same_runs > 20 && made > 20 && refused > 5,
            "{same_runs} {made} {refused}"
        );
    }

    #[test]
    fn advanced_find_conditions_and_or() {
        let mut s = Schedule::new("ZTEST");
        let mk = |s: &mut Schedule, scenes: &str, set: &str, eighths: u32, mins: Option<u32>| {
            s.add_sheet(Sheet {
                scenes: scenes.into(),
                set: set.into(),
                pages: crate::Eighths(eighths),
                shoot_time: mins.map(crate::model::Minutes),
                ..Sheet::default()
            })
        };
        let a = mk(&mut s, "9", "KITCHEN", 12, Some(90));
        let b = mk(&mut s, "12", "Kitchen Garden", 4, None);
        let c = mk(&mut s, "20", "CABIN", 20, Some(30));
        let cond = |f: &str, op: &str, v: &str| {
            Condition::new(CondField::parse(f).unwrap(), Op::parse(op).unwrap(), v).unwrap()
        };
        let sheets = |s: &Schedule, q: &Query| -> Vec<SheetId> {
            let mut v: Vec<SheetId> = find(s, q)
                .unwrap()
                .into_iter()
                .filter_map(|h| match h.location {
                    Location::Sheet { sheet, .. } => Some(sheet),
                    _ => None,
                })
                .collect();
            v.dedup();
            v
        };
        let q = |conds: Vec<Condition>, any: bool| Query {
            conditions: conds,
            any,
            ..Query::default()
        };
        assert_eq!(
            sheets(&s, &q(vec![cond("set", "starts_with", "kitchen")], false)),
            [a, b]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("set", "is", "kitchen")], false)),
            [a]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("set", "is_not", "kitchen")], false)),
            [b, c]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("set", "not_contains", "kitchen")], false)),
            [c]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("pages", "greater", "1 4/8")], false)),
            [c]
        );
        assert_eq!(sheets(&s, &q(vec![cond("pages", "less", "1")], false)), [b]);
        assert_eq!(
            sheets(&s, &q(vec![cond("shoot_time", "empty", "")], false)),
            [b]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("shoot_time", "greater", "1:00")], false)),
            [a]
        );
        assert_eq!(
            sheets(&s, &q(vec![cond("scenes", "greater", "10")], false)),
            [b, c]
        );
        // AND and OR.
        let two = vec![
            cond("set", "contains", "kitchen"),
            cond("pages", "greater", "1"),
        ];
        assert_eq!(sheets(&s, &q(two.clone(), false)), [a]);
        assert_eq!(sheets(&s, &q(two, true)), [a, b, c]);
        // With text, only the qualifying sheets are searched, and only sheet fields.
        let cat = s.add_category("Props");
        s.add_element(cat, "Kitchen knife").unwrap();
        let mut with = q(vec![cond("pages", "greater", "1")], false);
        with.text = "kitchen".into();
        let hits = find(&s, &with).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].location,
            Location::Sheet {
                sheet: a,
                field: SheetField::Set
            }
        );
        // Bad values are refused in words.
        for (f, op, v) in [
            ("pages", "greater", "lots"),
            ("shoot_time", "is", "90m"),
            ("pages", "contains", "1"),
            ("set", "contains", ""),
        ] {
            assert!(
                Condition::new(CondField::parse(f).unwrap(), Op::parse(op).unwrap(), v).is_err(),
                "{f} {op} {v}"
            );
        }
        // The sheet number is a field like the others.
        s.sheet_mut(c).unwrap().number = "7".into();
        assert_eq!(sheets(&s, &q(vec![cond("number", "is", "7")], false)), [c]);
    }

    /// Review: an empty shoot time once was 0 for Is, Is Not and Less but empty for
    /// Greater.
    #[test]
    fn an_empty_number_meets_only_empty() {
        let none = Sheet::default();
        let timed = Sheet {
            shoot_time: Some(crate::model::Minutes(30)),
            ..Sheet::default()
        };
        for (op, v, want_none, want_timed) in [
            ("is", "0:00", false, false),
            ("is", "0:30", false, true),
            ("is_not", "0:30", false, false),
            ("is_not", "1:00", false, true),
            ("less", "1:00", false, true),
            ("greater", "0:10", false, true),
            ("empty", "", true, false),
            ("not_empty", "", false, true),
        ] {
            let c = Condition::new(CondField::ShootTime, Op::parse(op).unwrap(), v).unwrap();
            assert_eq!(c.meets(&none, false), want_none, "{op} {v} on none");
            assert_eq!(c.meets(&timed, false), want_timed, "{op} {v} on 0:30");
        }
    }
}
