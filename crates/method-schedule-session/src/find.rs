//! Edit > Find and Edit > Replace (decisions.md, entries 28 and 34). The engine does the
//! work (`method_schedule_core::find`); this reads what the page sends, turns the results
//! into what the Find panel draws (ui/js/find.js), and makes a replace one undo step with
//! its refusals as plain sentences.
//!
//! A result travels as a key (`sheet:12:set`, `element:5`, `category:2`,
//! `banner:1:3:40`, `event:1:3:0`) and comes back the same way for Replace; every key is
//! parsed and checked here, and the engine refuses a pick that no longer matches.
//! Method Budget's `src/find.rs` is the model (at most 1,000 results listed, text cut
//! short), scaled down to the engine's typed search.

use serde::{Deserialize, Serialize};

use method_schedule_core::find::{
    self as finder, CondField, Condition, FieldFilter, Location, Op, Query, Refusal, Scope,
    SheetField,
};
use method_schedule_core::model::{BoardId, CategoryId, ElementId, SheetId, StripId, SubBoardId};
use method_schedule_core::Schedule;

/// Most results listed; the count says how many there are in all.
pub const MAX_HITS: usize = 1000;
/// Longest search text, in characters.
const MAX_TEXT: usize = 1000;
/// Longest replacement, in characters (a header field holds at most 10,000).
const MAX_WITH: usize = 10_000;
/// Most picks a replace takes.
const MAX_PICKS: usize = 100_000;
/// Longest key the page may send back.
const MAX_KEY: usize = 64;
/// Longest context shown, in characters.
const CUT: usize = 160;

/// A search as the page sends it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct QueryArg {
    pub text: String,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub whole_word: bool,
    /// A `FieldFilter` key (`sheet`, a header field, `element`, `category`, `banner`,
    /// `event`), or null for everything.
    #[serde(default)]
    pub field: Option<String>,
    /// The sub-board to search, or null for the whole schedule.
    #[serde(default)]
    pub sub_board: Option<u32>,
    /// An advanced search's rows (decision 77): `{field, op, value}`, at most
    /// [`MAX_CONDITIONS`].
    #[serde(default)]
    pub conditions: Vec<ConditionArg>,
    /// Join the rows with OR rather than AND.
    #[serde(default)]
    pub any: bool,
}

/// One row of an advanced search as the page sends it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConditionArg {
    pub field: String,
    pub op: String,
    #[serde(default)]
    pub value: String,
}

/// Most rows an advanced search takes.
pub const MAX_CONDITIONS: usize = 20;

/// What the Find panel lists.
#[derive(Clone, Debug, Serialize)]
pub struct FoundDto {
    /// Every place that matches.
    pub count: usize,
    /// The first [`MAX_HITS`], in board order.
    pub hits: Vec<HitDto>,
    pub truncated: bool,
}

/// One result, with what the page needs to go there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HitDto {
    /// The place, as Replace takes it back.
    pub key: String,
    /// `sheet`, `element`, `category`, `banner` or `event`.
    pub kind: &'static str,
    /// A header field's key, or `name` or `text`.
    pub field: &'static str,
    /// The place in words: `Scene 12, Set`, `Cast element`, `Banner on Main`.
    pub place: String,
    /// What the field holds, on one line, cut around the first match.
    pub context: String,
    pub sheet: Option<u32>,
    pub element: Option<u32>,
    pub category: Option<u32>,
    pub board: Option<u32>,
    pub sub_board: Option<u32>,
    pub strip: Option<u32>,
    /// An event's place in its sub-board's calendar.
    pub event: Option<usize>,
    /// An event's first date, `YYYY-MM-DD`.
    pub date: Option<String>,
}

/// The header fields' names as the Breakdown tab labels them.
fn field_label(f: SheetField) -> &'static str {
    match f {
        SheetField::Number => "Sheet number",
        SheetField::Scenes => "Scenes",
        SheetField::IntExt => "I/E",
        SheetField::Set => "Set",
        SheetField::TimeOfDay => "Time of day",
        SheetField::Synopsis => "Synopsis",
        SheetField::Location => "Location",
        SheetField::ScriptPages => "Script pages",
        SheetField::ScriptDay => "Script day",
        SheetField::Unit => "Unit",
        SheetField::Sequence => "Sequence",
        SheetField::Episode => "Episode",
        SheetField::Comments => "Comments",
    }
}

/// The search the page asked for, checked.
pub fn query(s: &Schedule, q: &QueryArg) -> Result<Query, String> {
    if q.text.chars().count() > MAX_TEXT {
        return Err(format!(
            "Search for at most {MAX_TEXT} characters at a time."
        ));
    }
    let field = match q.field.as_deref() {
        None | Some("") => None,
        Some(k) => Some(FieldFilter::parse(k).ok_or("That isn't a field Find knows.")?),
    };
    let scope = match q.sub_board {
        None => Scope::Schedule,
        Some(sub) => {
            let (b, sb) = crate::view::sub_ref(s, sub)?;
            Scope::SubBoard {
                board: b.id,
                sub_board: sb.id,
            }
        }
    };
    if q.conditions.len() > MAX_CONDITIONS {
        return Err(format!(
            "An advanced find takes at most {MAX_CONDITIONS} rows."
        ));
    }
    let mut conditions = Vec::with_capacity(q.conditions.len());
    for c in &q.conditions {
        if c.value.chars().count() > MAX_TEXT {
            return Err(format!(
                "Compare with at most {MAX_TEXT} characters at a time."
            ));
        }
        let field = CondField::parse(&c.field).ok_or("That isn't a field Find knows.")?;
        let op = Op::parse(&c.op).ok_or("That isn't a comparison Find knows.")?;
        conditions.push(Condition::new(field, op, &c.value).map_err(|e| upper_first(&e) + ".")?);
    }
    Ok(Query {
        text: q.text.clone(),
        match_case: q.match_case,
        whole_word: q.whole_word,
        field,
        scope,
        conditions,
        any: q.any,
    })
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// A place as a key the page sends back.
pub fn key(loc: &Location) -> String {
    match *loc {
        Location::Sheet { sheet, field } => format!("sheet:{}:{}", sheet.0, field.key()),
        Location::Element(id) => format!("element:{}", id.0),
        Location::Category(id) => format!("category:{}", id.0),
        Location::Banner {
            board,
            sub_board,
            strip,
        } => format!("banner:{}:{}:{}", board.0, sub_board.0, strip.0),
        Location::Event {
            board,
            sub_board,
            index,
        } => format!("event:{}:{}:{index}", board.0, sub_board.0),
    }
}

/// A key back as a place. Only the shape is checked here; the engine refuses a place
/// that isn't (or no longer is) a result.
pub fn parse_key(k: &str) -> Option<Location> {
    if k.len() > MAX_KEY {
        return None;
    }
    let mut parts = k.split(':');
    let kind = parts.next()?;
    let mut num = || -> Option<u32> {
        let p = parts.next()?;
        // Plain digits only: no sign, no spaces.
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    };
    let loc = match kind {
        "sheet" => {
            let sheet = SheetId(num()?);
            let field = SheetField::parse(parts.next()?)?;
            Location::Sheet { sheet, field }
        }
        "element" => Location::Element(ElementId(num()?)),
        "category" => Location::Category(CategoryId(num()?)),
        "banner" => Location::Banner {
            board: BoardId(num()?),
            sub_board: SubBoardId(num()?),
            strip: StripId(num()?),
        },
        "event" => Location::Event {
            board: BoardId(num()?),
            sub_board: SubBoardId(num()?),
            index: num()? as usize,
        },
        _ => return None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(loc)
}

fn cut(text: &str) -> String {
    if text.chars().count() <= CUT {
        return text.to_string();
    }
    let mut out: String = text.chars().take(CUT).collect();
    out.push_str("...");
    out
}

/// `Main`, or `Board 2, Main` when the schedule has more than one board.
fn sub_board_name(s: &Schedule, board: BoardId, sub: SubBoardId) -> String {
    let Some(b) = s.board(board) else {
        return String::new();
    };
    let name = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .map_or("", |x| x.name.as_str());
    if s.boards.len() > 1 {
        format!("{}, {name}", b.name)
    } else {
        name.to_string()
    }
}

/// A sheet in words: `Scene 12`, or `A sheet with no scene number`.
fn sheet_words(s: &Schedule, sheet: SheetId) -> String {
    let scenes = s.sheet(sheet).map_or("", |h| h.scenes.trim());
    if scenes.is_empty() {
        "A sheet with no scene number".to_string()
    } else {
        format!("Scene {scenes}")
    }
}

/// The place in words, as the result list shows it.
pub fn place(s: &Schedule, loc: &Location) -> String {
    match *loc {
        Location::Sheet { sheet, field } => {
            format!("{}, {}", sheet_words(s, sheet), field_label(field))
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
        } => format!("Banner on {}", sub_board_name(s, board, sub_board)),
        Location::Event {
            board,
            sub_board,
            index,
        } => {
            let from = s
                .board(board)
                .and_then(|b| b.sub_boards.iter().find(|x| x.id == sub_board))
                .and_then(|sb| sb.calendar.events.get(index))
                .map(|e| format!(", {}", e.from))
                .unwrap_or_default();
            format!("Event on {}{from}", sub_board_name(s, board, sub_board))
        }
    }
}

fn hit_dto(s: &Schedule, h: &finder::Hit) -> HitDto {
    let loc = h.location;
    // The engine's context starts with its own label; the panel shows ours beside it.
    let prefix = format!("{}: ", finder::label(s, &loc));
    let context = h.context.strip_prefix(&prefix).unwrap_or(&h.context);
    let mut d = HitDto {
        key: key(&loc),
        kind: loc.kind(),
        field: loc.field(),
        place: place(s, &loc),
        context: cut(context),
        sheet: None,
        element: None,
        category: None,
        board: None,
        sub_board: None,
        strip: None,
        event: None,
        date: None,
    };
    match loc {
        Location::Sheet { sheet, .. } => d.sheet = Some(sheet.0),
        Location::Element(id) => {
            d.element = Some(id.0);
            d.category = s.element(id).map(|e| e.category.0);
        }
        Location::Category(id) => d.category = Some(id.0),
        Location::Banner {
            board,
            sub_board,
            strip,
        } => {
            d.board = Some(board.0);
            d.sub_board = Some(sub_board.0);
            d.strip = Some(strip.0);
        }
        Location::Event {
            board,
            sub_board,
            index,
        } => {
            d.board = Some(board.0);
            d.sub_board = Some(sub_board.0);
            d.event = Some(index);
            d.date = s
                .board(board)
                .and_then(|b| b.sub_boards.iter().find(|x| x.id == sub_board))
                .and_then(|sb| sb.calendar.events.get(index))
                .map(|e| e.from.to_string());
        }
    }
    d
}

/// Every place `q` matches, the first [`MAX_HITS`] of them listed.
pub fn find(s: &Schedule, q: &QueryArg) -> Result<FoundDto, String> {
    let query = query(s, q)?;
    let hits = finder::find(s, &query).map_err(|e| e.to_string())?;
    Ok(FoundDto {
        count: hits.len(),
        truncated: hits.len() > MAX_HITS,
        hits: hits.iter().take(MAX_HITS).map(|h| hit_dto(s, h)).collect(),
    })
}

/// A refusal as a plain sentence.
pub fn refusal(s: &Schedule, r: &Refusal) -> String {
    match r {
        Refusal::NothingToFind => "Type what to find first.".to_string(),
        Refusal::NothingPicked => "Check one or more results to replace.".to_string(),
        Refusal::Stale(_) => "Nothing was replaced: some of the checked results no longer \
                              match, because the schedule changed. Check them again and \
                              choose Replace."
            .to_string(),
        Refusal::Scope(_) => "That sub-board is no longer in the schedule.".to_string(),
        Refusal::Collision {
            category: Some(c),
            name,
        } => format!("Nothing was replaced: {c} would have two elements called \"{name}\"."),
        Refusal::Collision {
            category: None,
            name,
        } => format!("Nothing was replaced: there would be two categories called \"{name}\"."),
        Refusal::EmptyName(loc) => {
            let what = if matches!(loc, Location::Category(_)) {
                "a category"
            } else {
                "an element"
            };
            format!("Nothing was replaced: {what}'s name can't be empty.")
        }
        Refusal::OneLine(loc) => format!(
            "Nothing was replaced: {} is one line, so the replacement can't hold a line break.",
            place(s, loc)
        ),
        Refusal::Broken(e) => {
            format!("Nothing was replaced: the schedule wouldn't hang together ({e}).")
        }
    }
}

/// Replaces what `q` matches by `with` in the places `picks` names, as one undo step.
/// Returns the notice for the status line.
pub fn replace(
    session: &mut crate::session::Session,
    q: &QueryArg,
    with: &str,
    picks: &[String],
) -> Result<String, String> {
    if with.chars().count() > MAX_WITH {
        return Err(format!("A replacement is at most {MAX_WITH} characters."));
    }
    if picks.len() > MAX_PICKS {
        return Err("Too many results checked at once.".to_string());
    }
    let locs = picks
        .iter()
        .map(|k| parse_key(k).ok_or("That result isn't one Find gave."))
        .collect::<Result<Vec<_>, _>>()?;
    let query = query(session.schedule(), q)?;
    let n = session.edit(|x| {
        let done = finder::replace(x, &query, with, &locs).map_err(|r| refusal(x, &r))?;
        *x = done.schedule;
        Ok(done.changes.len())
    })?;
    Ok(match n {
        0 => "Nothing changed: the checked results already read that way.".to_string(),
        1 => "Replaced in 1 place. Undo takes it back.".to_string(),
        n => format!("Replaced in {n} places. Undo takes them all back."),
    })
}

// ---------------------------------------------------------------------------
// Commands

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::{Banner, Event, EventKind, Minutes, Place, Sheet, StripKind};

    /// Main holds scenes 1 (CABIN: ADA, BEN) and 2 (CREEK: ADA) and a banner "Cabin move";
    /// Unit B holds scene 3 (CABIN NIGHT: CARL) and a fitting "Cabin fitting" for CARL;
    /// scene 4 (CABINET) is in the Boneyard. Props holds Lantern.
    fn session() -> (Session, SubBoardId) {
        let mut s = Schedule::new("ZTEST Find app");
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
        s.add_sheet(sheet("1", "CABIN", vec![ada, ben]));
        s.add_sheet(sheet("2", "CREEK", vec![ada]));
        let third = s.add_sheet(sheet("3", "CABIN NIGHT", vec![carl]));
        s.add_sheet(sheet("4", "CABINET", vec![]));
        let b = s.boards[0].id;
        let main = s.boards[0].sub_boards[0].id;
        let unit = s.add_sub_board(b, "Unit B", None).unwrap();
        let first_two: Vec<StripId> = s.boards[0].boneyard[..2].iter().map(|x| x.id).collect();
        s.move_strips(
            b,
            &first_two,
            Place {
                sub_board: Some(main),
                index: 0,
            },
        )
        .unwrap();
        let strip3 = s.boards[0]
            .boneyard
            .iter()
            .find(|x| x.kind == StripKind::Sheet(third))
            .unwrap()
            .id;
        s.move_strips(
            b,
            &[strip3],
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
        let d = |x: &str| x.parse().unwrap();
        s.boards[0].sub_boards[1].calendar.events.push(Event {
            name: "Cabin fitting".into(),
            kind: EventKind::Fitting,
            from: d("2027-03-04"),
            to: d("2027-03-04"),
            elements: vec![carl],
            color: None,
        });
        s.check().unwrap();
        let mut session = Session::new();
        session
            .edit(|x| {
                *x = s;
                Ok(())
            })
            .unwrap();
        (session, unit)
    }

    fn q(text: &str) -> QueryArg {
        QueryArg {
            text: text.into(),
            ..QueryArg::default()
        }
    }

    #[test]
    fn results_carry_their_place_context_and_where_to_go() {
        let (s, unit) = session();
        let f = find(s.schedule(), &q("cabin")).unwrap();
        let places: Vec<&str> = f.hits.iter().map(|h| h.place.as_str()).collect();
        // Board order: Main's strips (scene 1, the banner, scene 2), Unit B's strip and
        // event, then the Boneyard's scene 4.
        assert_eq!(
            places,
            [
                "Scene 1, Set",
                "Scene 1, Synopsis",
                "Banner on Main",
                "Scene 3, Set",
                "Scene 3, Synopsis",
                "Event on Unit B, 2027-03-04",
                "Scene 4, Set",
                "Scene 4, Synopsis",
            ]
        );
        assert_eq!(f.count, 8);
        assert!(!f.truncated);
        let set = &f.hits[0];
        assert_eq!(
            (set.kind, set.field, set.context.as_str()),
            ("sheet", "set", "CABIN")
        );
        assert!(set.sheet.is_some() && set.board.is_none());
        let banner = &f.hits[2];
        assert_eq!(banner.context, "Cabin move");
        assert!(banner.strip.is_some() && banner.sub_board.is_some() && banner.board.is_some());
        let event = &f.hits[5];
        assert_eq!(
            (event.event, event.date.as_deref(), event.sub_board),
            (Some(0), Some("2027-03-04"), Some(unit.0))
        );
        // Every key reads back as its place.
        for h in &f.hits {
            let loc = parse_key(&h.key).unwrap();
            assert_eq!(key(&loc), h.key);
        }

        // Match case, whole word, a field and a scope.
        let mut x = q("CABIN");
        x.match_case = true;
        x.whole_word = true;
        x.field = Some("set".into());
        let f = find(s.schedule(), &x).unwrap();
        assert_eq!(f.count, 2, "CABIN and CABIN NIGHT, not CABINET");
        x.sub_board = Some(unit.0);
        let f = find(s.schedule(), &x).unwrap();
        assert_eq!(f.hits.len(), 1);
        assert_eq!(f.hits[0].place, "Scene 3, Set");

        // Element and category names, with the category to open.
        let f = find(s.schedule(), &q("a")).unwrap();
        let ada = f.hits.iter().find(|h| h.place == "Cast element").unwrap();
        assert_eq!(ada.kind, "element");
        assert!(ada.element.is_some() && ada.category.is_some());
        assert!(f
            .hits
            .iter()
            .any(|h| h.kind == "category" && h.context == "Cast"));
    }

    #[test]
    fn what_the_page_sends_is_checked() {
        let (s, _) = session();
        let mut x = q("cabin");
        x.field = Some("budget".into());
        assert_eq!(
            find(s.schedule(), &x).unwrap_err(),
            "That isn't a field Find knows."
        );
        let mut x = q("cabin");
        x.sub_board = Some(999_999);
        assert_eq!(
            find(s.schedule(), &x).unwrap_err(),
            "That sub-board is no longer in the schedule."
        );
        assert!(find(s.schedule(), &q(&"x".repeat(MAX_TEXT + 1))).is_err());
        assert_eq!(find(s.schedule(), &q("")).unwrap().count, 0);
        for bad in [
            "",
            "sheet",
            "sheet:1",
            "sheet:1:budget",
            "sheet:-1:set",
            "sheet:+1:set",
            "element:1:2",
            "banner:1:2",
            "event:1:2:x",
            "thing:1",
            "element: 1",
        ] {
            assert_eq!(parse_key(bad), None, "{bad}");
        }
        assert!(parse_key(&format!("element:{}", "1".repeat(80))).is_none());
    }

    #[test]
    fn replace_is_one_undo_step() {
        let (mut s, _) = session();
        let f = find(s.schedule(), &q("cabin")).unwrap();
        let picks: Vec<String> = f.hits.iter().map(|h| h.key.clone()).collect();
        let before = s.schedule().clone();
        let notice = replace(&mut s, &q("cabin"), "Lodge", &picks).unwrap();
        assert_eq!(notice, "Replaced in 8 places. Undo takes them all back.");
        assert_eq!(find(s.schedule(), &q("cabin")).unwrap().count, 0);
        assert_eq!(find(s.schedule(), &q("Lodge")).unwrap().count, 8);
        assert!(s.undo());
        assert_eq!(s.schedule(), &before);

        // Only the picked ones.
        let notice = replace(&mut s, &q("cabin"), "Lodge", &picks[..1]).unwrap();
        assert_eq!(notice, "Replaced in 1 place. Undo takes it back.");
        assert_eq!(find(s.schedule(), &q("cabin")).unwrap().count, 7);

        // Replacing a match-case find with its own text changes nothing.
        let mut same = q("CREEK");
        same.match_case = true;
        let k: Vec<String> = find(s.schedule(), &same)
            .unwrap()
            .hits
            .iter()
            .map(|h| h.key.clone())
            .collect();
        let rev = s.schedule().clone();
        assert_eq!(
            replace(&mut s, &same, "CREEK", &k).unwrap(),
            "Nothing changed: the checked results already read that way."
        );
        assert_eq!(s.schedule(), &rev);
    }

    #[test]
    fn refusals_are_plain_sentences_and_change_nothing() {
        let (mut s, _) = session();
        let keys = |s: &Session, x: &QueryArg| -> Vec<String> {
            find(s.schedule(), x)
                .unwrap()
                .hits
                .iter()
                .map(|h| h.key.clone())
                .collect()
        };
        let before = s.schedule().clone();
        let mut el = q("BEN");
        el.field = Some("element".into());
        let k = keys(&s, &el);
        let mut errors = vec![replace(&mut s, &el, "ada", &k).unwrap_err()];
        assert_eq!(
            errors[0],
            "Nothing was replaced: Cast would have two elements called \"ada\"."
        );
        errors.push(replace(&mut s, &el, "  ", &k).unwrap_err());
        assert_eq!(
            errors[1],
            "Nothing was replaced: an element's name can't be empty."
        );
        let mut props = q("Props");
        props.field = Some("category".into());
        let k = keys(&s, &props);
        errors.push(replace(&mut s, &props, "cast", &k).unwrap_err());
        assert_eq!(
            errors[2],
            "Nothing was replaced: there would be two categories called \"cast\"."
        );
        let mut set = q("CREEK");
        set.field = Some("set".into());
        let k = keys(&s, &set);
        errors.push(replace(&mut s, &set, "A\nB", &k).unwrap_err());
        assert_eq!(
            errors[3],
            "Nothing was replaced: Scene 2, Set is one line, so the replacement can't hold a line break."
        );
        errors.push(replace(&mut s, &q("nothing here"), "x", &k).unwrap_err());
        assert!(errors[4].starts_with("Nothing was replaced: some of the checked results"));
        errors.push(replace(&mut s, &set, "x", &[]).unwrap_err());
        assert_eq!(errors[5], "Check one or more results to replace.");
        errors.push(replace(&mut s, &q(""), "x", &k).unwrap_err());
        assert_eq!(errors[6], "Type what to find first.");
        errors.push(replace(&mut s, &set, "x", &["sheet:x".into()]).unwrap_err());
        assert_eq!(errors[7], "That result isn't one Find gave.");
        errors.push(replace(&mut s, &set, &"x".repeat(MAX_WITH + 1), &k).unwrap_err());
        for e in &errors {
            assert!(!e.contains('\u{2014}') && !e.contains('!'), "{e}");
            assert!(e.ends_with('.'), "{e}");
        }
        assert_eq!(s.schedule(), &before);
        assert!(!s.can_undo() || s.schedule() == &before);
        // A line break is fine in a synopsis.
        let mut syn = q("CREEK");
        syn.field = Some("synopsis".into());
        let k = keys(&s, &syn);
        assert!(replace(&mut s, &syn, "creek\nbed", &k).is_ok());
    }

    #[test]
    fn at_most_max_hits_are_listed() {
        let mut s = method_schedule_core::sample::sample(1500);
        let n = s.sheets.len();
        for h in s.sheets.iter_mut() {
            h.comments = "zz".into();
        }
        let f = find(&s, &q("zz")).unwrap();
        assert_eq!(f.count, n);
        assert_eq!(f.hits.len(), MAX_HITS.min(n));
        assert_eq!(f.truncated, n > MAX_HITS);
    }

    /// The page's field list (ui/js/find.js, between its markers) is the engine's
    /// `FieldFilter::keys()` plus '' for everything, in the same order.
    #[test]
    fn the_page_offers_every_field_the_engine_takes() {
        const FIND_JS: &str = include_str!("../../method-schedule-app/ui/js/find.js");
        let list = FIND_JS
            .split("const FIELDS = [")
            .nth(1)
            .unwrap()
            .split("];")
            .next()
            .unwrap();
        let keys: Vec<&str> = list
            .lines()
            .filter_map(|l| l.trim().strip_prefix("['"))
            .map(|l| &l[..l.find('\'').unwrap()])
            .collect();
        let mut want = vec![""];
        want.extend(FieldFilter::keys());
        assert_eq!(keys, want);
        for k in &keys[1..] {
            assert!(FieldFilter::parse(k).is_some(), "{k}");
        }
    }
}
