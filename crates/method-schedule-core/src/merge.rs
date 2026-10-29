//! Three-way merge of schedules (roadmap phase 3; docs/MERGE.md; decisions.md, entry 25).
//! Ported from Method Budget's merge, same rules.
//!
//! [`merge`] takes a common ancestor `base` and two schedules made from it, `ours` and
//! `theirs`, and returns one schedule with both sets of changes, plus the [`Conflict`]s a
//! person may want to settle. Everything is matched by id, as [`crate::diff`] matches it.
//!
//! - A change on one side only is taken. The same change on both sides is taken once.
//! - Changes to different things, or different fields of one thing, merge on their own.
//!   A sheet's elements and an element's links merge id by id, calendar dates date by
//!   date, events (which have no ids yet) by value.
//! - **Conflicts**: the same field changed to different values on both sides; something
//!   deleted on one side and changed (or newly used) on the other; both sides reordering
//!   one list differently (a sub-board's strips, the Boneyard, the categories, a board's
//!   sub-boards). Each takes [`Side::Ours`] by default and is listed; [`merge_with`]
//!   takes a pick per conflict key, and keys are stable for the same three schedules.
//! - Keeping something one side deleted keeps it as the other side has it, with the
//!   sheet tags, links and strips the deleting side took away with it.
//! - **Order** (Method Budget's rule for rows): the side that reordered a list wins;
//!   strips the other side added or moved in go right after the strip they followed
//!   there.
//! - **New ids**: each side numbers new things from its own `next_id`, so two copies
//!   adding things usually reuse ids. Categories added on both sides with one name are
//!   one category, and elements added on both sides with one name in one category are one
//!   element, and so are red flag types added on both sides with one name. Anything else
//!   added on both sides with one id is one thing only when it is identical; otherwise
//!   theirs gets a new id above every id in the three schedules.
//! - Red flag types merge like categories (by id, the name a field, the list order like
//!   the categories'); red flags merge id by id inside their element, field by field
//!   (type, dates, note, hard).
//! - The result always passes [`Schedule::check`]. A merge that wouldn't (two names that
//!   now collide, a tag on an element the other side deleted) falls back to ours for the
//!   part at fault and says so in [`Merged::fallbacks`].

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use crate::diff::{
    event_changes, list_of, marked_dates, on_off, quoted, sheet_name, strip_places, upper_first,
    weekdays_text, Mark, GONE_SHEET,
};
use crate::model::{
    same_name, Banner, Board, BoardId, Calendar, Category, CategoryId, DoodRules, Element,
    ElementId, Event, Minutes, RedFlag, RedFlagId, RedFlagType, RedFlagTypeId, Schedule, Seconds,
    Sheet, SheetId, Strip, StripId, StripKind, SubBoard, SubBoardId,
};
use crate::{Date, Eighths};

// ---------------------------------------------------------------------------
// What a merge gives back

/// One side of a merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Ours,
    Theirs,
}

impl Side {
    /// `"ours"` or `"theirs"`.
    pub fn key(self) -> &'static str {
        match self {
            Side::Ours => "ours",
            Side::Theirs => "theirs",
        }
    }

    pub fn from_key(s: &str) -> Option<Side> {
        match s {
            "ours" => Some(Side::Ours),
            "theirs" => Some(Side::Theirs),
            _ => None,
        }
    }
}

/// Something both sides changed differently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// Stable for the same three schedules: what a pick names.
    pub key: String,
    /// What it is about: `Scene 4 set`, `Element "ADA" (Cast)`.
    pub what: String,
    /// The value in the base, when it had one.
    pub base: Option<String>,
    pub ours: String,
    pub theirs: String,
    /// The side the merged schedule took.
    pub taken: Side,
    /// In plain words: `Scene 4 set: ours "LOFT", theirs "BARN", was "SHED"`.
    pub text: String,
}

/// A merged schedule.
#[derive(Clone, Debug)]
pub struct Merged {
    /// Always passes [`Schedule::check`].
    pub schedule: Schedule,
    /// In schedule order: title, categories, elements, sheets, boards.
    pub conflicts: Vec<Conflict>,
    /// Things from theirs given new ids because ours added different ones with the same
    /// ids.
    pub renumbered: usize,
    /// Where the merge fell back to ours (or left something out) so the result hangs
    /// together, in plain words.
    pub fallbacks: Vec<String>,
}

/// The production info merges field by field.
fn merge_production(
    cx: &mut Cx<'_>,
    base: &Schedule,
    ours: &Schedule,
    theirs: &Schedule,
) -> crate::setup::Production {
    let mut p = crate::setup::Production::default();
    for f in crate::setup::Production::FIELDS {
        let label = match f {
            "company" => "Production company",
            other => crate::setup::Production::label(other),
        };
        let at = |s: &Schedule| s.production.get(f).unwrap_or_default().to_string();
        let v = field(
            cx,
            &format!("production:{f}"),
            label,
            Some(&at(base)),
            &at(ours),
            &at(theirs),
            |t| quoted(t),
        );
        *p.get_mut(f).expect("a known field") = v;
    }
    p
}

/// Merges `ours` and `theirs`, both made from `base`. Conflicts keep ours.
pub fn merge(base: &Schedule, ours: &Schedule, theirs: &Schedule) -> Merged {
    merge_with(base, ours, theirs, Side::Ours, &BTreeMap::new())
}

/// [`merge`], taking `picks` (conflict key to side) where given and `default` for every
/// other conflict.
pub fn merge_with(
    base: &Schedule,
    ours: &Schedule,
    theirs: &Schedule,
    default: Side,
    picks: &BTreeMap<String, Side>,
) -> Merged {
    let mut cx = Cx {
        default,
        picks,
        conflicts: Vec::new(),
        fallbacks: Vec::new(),
    };
    let (theirs, renumbered, next) = align(base, ours, theirs);
    let m = M {
        b: base,
        o: ours,
        t: &theirs,
        refs_o: new_refs(base, ours),
        refs_t: new_refs(base, &theirs),
        gained_o: gained(base, ours),
        gained_t: gained(base, &theirs),
        used_o: type_uses(base, ours),
        used_t: type_uses(base, &theirs),
        keep: default,
    };
    let mut s = Schedule {
        title: field(
            &mut cx,
            "title",
            "Title",
            Some(&base.title),
            &ours.title,
            &theirs.title,
            |t| quoted(t),
        ),
        categories: Vec::new(),
        elements: Vec::new(),
        sheets: Vec::new(),
        boards: Vec::new(),
        red_flag_types: Vec::new(),
        production: merge_production(&mut cx, base, ours, &theirs),
        // The report list and the code letters merge as one value each, as a strip
        // layout does: one side's change wins, two different changes conflict.
        reports: field(
            &mut cx,
            "reports",
            "Report list",
            Some(&base.reports),
            &ours.reports,
            &theirs.reports,
            |r| quoted(&crate::setup::reports_text(r)),
        ),
        // The header layouts are one value, as the report list is (decision 99).
        header_layouts: field(
            &mut cx,
            "header_layouts",
            "Header layouts",
            Some(&base.header_layouts),
            &ours.header_layouts,
            &theirs.header_layouts,
            |h| quoted(&crate::headers::headers_text(h)),
        ),
        dood_codes: field(
            &mut cx,
            "dood_codes",
            "Day out of days codes",
            Some(&base.dood_codes),
            &ours.dood_codes,
            &theirs.dood_codes,
            |c| quoted(&c.text()),
        ),
        // The whole set of strip colors is one value, as a strip layout is.
        strip_colors: field(
            &mut cx,
            "strip_colors",
            "Strip colors",
            Some(&base.strip_colors),
            &ours.strip_colors,
            &theirs.strip_colors,
            |c| c.text(),
        ),
        // The layout library is one value, as the strip colors are (decision 78).
        layouts: field(
            &mut cx,
            "layouts",
            "Layouts",
            Some(&base.layouts),
            &ours.layouts,
            &theirs.layouts,
            |l| l.text(ours),
        ),
        // The planning data merges list by list, entry by entry (decision 91).
        plan: crate::plan::Plan::default(),
        // The header field order is one value too.
        sheet_fields: field(
            &mut cx,
            "sheet_fields",
            "Sheet field order",
            Some(&base.sheet_fields),
            &ours.sheet_fields,
            &theirs.sheet_fields,
            |v| crate::diff::field_order_text(v),
        ),
        // The breakdown designer's hidden fields and widths are one value (decision 101).
        sheet_looks: field(
            &mut cx,
            "sheet_looks",
            "Sheet field looks",
            Some(&base.sheet_looks),
            &ours.sheet_looks,
            &theirs.sheet_looks,
            |v| v.text(),
        ),
        // The calendar strip's fields are one value, as a strip layout is (decision 102).
        calendar_fields: field(
            &mut cx,
            "calendar_fields",
            "Calendar strip fields",
            Some(&base.calendar_fields),
            &ours.calendar_fields,
            &theirs.calendar_fields,
            |v| crate::diff::calendar_fields_text(v),
        ),
        // What the Calendar hides is one value too (decision 111).
        calendar_hidden: field(
            &mut cx,
            "calendar_hidden",
            "Calendar view",
            Some(&base.calendar_hidden),
            &ours.calendar_hidden,
            &theirs.calendar_hidden,
            |v| crate::diff::calendar_hidden_text(v),
        ),
        next_id: next,
        // Additions from other apps (decision 224): both sides', ours first. Their new
        // objects are numbered again here, so only additions to objects the base had
        // (ids below its next id; ids are never reused) are theirs to keep.
        extensions: {
            let mut kept = theirs.extensions.clone();
            kept.objects.retain(|id, _| *id < base.next_id);
            crate::extensions::Extensions::union(&ours.extensions, &kept)
        },
    };
    s.plan = m.plan(&mut cx);
    s.categories = m.categories(&mut cx);
    s.red_flag_types = m.red_flag_types(&mut cx);
    let revived = m.elements(&mut cx, &mut s);
    s.sheets = m.sheets(&mut cx);
    m.revive_tags(&mut s, &revived);
    s.boards = m.boards(&mut cx, &s);
    // A report filter may name a category or element the other side deleted.
    s.tidy_report_filters();
    tidy(&mut s);
    crate::plan::tidy(&mut s);
    repair(&mut s, &m, &mut cx.fallbacks);
    // A board that picked a layout the merged library lacks goes back to its own.
    let lib = s.layouts.clone();
    for b in &mut s.boards {
        let before = b.uses;
        for slot in [
            &mut b.uses.strips,
            &mut b.uses.day_breaks,
            &mut b.uses.shoot_day,
        ] {
            if slot.is_some_and(|id| lib.get(id).is_none()) {
                *slot = None;
            }
        }
        if b.uses != before {
            cx.fallbacks.push(format!(
                "Board {}: a layout it picked is gone, it uses its own",
                quoted(&b.name)
            ));
        }
    }
    if s.layouts.one_line.is_some_and(|id| lib.get(id).is_none()) {
        s.layouts.one_line = None;
    }
    Merged {
        schedule: s,
        conflicts: cx.conflicts,
        renumbered,
        fallbacks: cx.fallbacks,
    }
}

// ---------------------------------------------------------------------------
// Deciding

struct Cx<'p> {
    default: Side,
    picks: &'p BTreeMap<String, Side>,
    conflicts: Vec<Conflict>,
    fallbacks: Vec<String>,
}

impl Cx<'_> {
    fn decide(
        &mut self,
        key: String,
        what: String,
        base: Option<String>,
        ours: String,
        theirs: String,
    ) -> Side {
        let taken = self.picks.get(&key).copied().unwrap_or(self.default);
        let text = match &base {
            Some(b) => format!("{what}: ours {ours}, theirs {theirs}, was {b}"),
            None => format!("{what}: ours {ours}, theirs {theirs}"),
        };
        self.conflicts.push(Conflict {
            key,
            what,
            base,
            ours,
            theirs,
            taken,
            text,
        });
        taken
    }
}

/// The three-way choice for one value: `Some(value)` (none for "not there"), or `None`
/// for a conflict.
fn pick3<'a, T: PartialEq + ?Sized>(
    b: Option<&'a T>,
    o: Option<&'a T>,
    t: Option<&'a T>,
) -> Option<Option<&'a T>> {
    if o == t {
        Some(o)
    } else if o == b {
        Some(t)
    } else if t == b {
        Some(o)
    } else {
        None
    }
}

/// One field merged: the side that changed it, or a conflict. `b` is none when both
/// sides added the thing.
fn field<V: PartialEq + Clone>(
    cx: &mut Cx<'_>,
    key: &str,
    what: &str,
    b: Option<&V>,
    o: &V,
    t: &V,
    show: impl Fn(&V) -> String,
) -> V {
    match pick3(b, Some(o), Some(t)) {
        Some(v) => v.unwrap_or(o).clone(),
        None => match cx.decide(
            key.to_string(),
            what.to_string(),
            b.map(&show),
            show(o),
            show(t),
        ) {
            Side::Ours => o.clone(),
            Side::Theirs => t.clone(),
        },
    }
}

/// One field of a thing merged with [`field`], under the thing's key and label (`k` and
/// `w` make them): the side that changed it, or a conflict.
macro_rules! merge_field {
    ($cx:expr, $k:ident, $w:ident, ($b:expr, $o:expr, $t:expr), $f:ident, $key:literal, $label:literal, $show:expr) => {
        field(
            $cx,
            &$k($key),
            &$w($label),
            $b.map(|x| &x.$f),
            &$o.$f,
            &$t.$f,
            $show,
        )
    };
}

/// Whether a thing is in the merge, and from where.
enum Got<'a, T> {
    Gone,
    /// From one side only: added there, or kept there against the other side's delete.
    One(&'a T, Side),
    Both(Option<&'a T>, &'a T, &'a T),
}

/// Deleted on one side and changed (`touched`) on the other is a conflict; deleted and
/// untouched goes.
#[allow(clippy::too_many_arguments)]
fn exist<'a, T>(
    cx: &mut Cx<'_>,
    key: &str,
    what: &str,
    b: Option<&'a T>,
    o: Option<&'a T>,
    t: Option<&'a T>,
    touched_o: bool,
    touched_t: bool,
) -> Got<'a, T> {
    let changed = "changed it".to_string();
    let deleted = "deleted it".to_string();
    match (b, o, t) {
        (_, Some(o), Some(t)) => Got::Both(b, o, t),
        (None, Some(o), None) => Got::One(o, Side::Ours),
        (None, None, Some(t)) => Got::One(t, Side::Theirs),
        (Some(_), Some(o), None) => {
            if !touched_o {
                return Got::Gone;
            }
            match cx.decide(key.to_string(), what.to_string(), None, changed, deleted) {
                Side::Ours => Got::One(o, Side::Ours),
                Side::Theirs => Got::Gone,
            }
        }
        (Some(_), None, Some(t)) => {
            if !touched_t {
                return Got::Gone;
            }
            match cx.decide(key.to_string(), what.to_string(), None, deleted, changed) {
                Side::Ours => Got::Gone,
                Side::Theirs => Got::One(t, Side::Theirs),
            }
        }
        _ => Got::Gone,
    }
}

/// Merges a list in which every element is unique (Method Budget's `merge_order`): the
/// side that reordered it wins (both reordering differently is a conflict when `clash`
/// names the list, else ours wins); elements the other side (or only the base) has go
/// right after the one they follow there; elements in no list go last. Only `members`
/// are kept.
fn merge_order<K: Copy + Ord + Hash>(
    cx: &mut Cx<'_>,
    clash: Option<(&str, &str)>,
    lists: [&[K]; 3],
    members: &BTreeSet<K>,
    show: impl Fn(&[K]) -> String,
) -> Vec<K> {
    let sets: Vec<HashSet<K>> = lists.iter().map(|l| l.iter().copied().collect()).collect();
    let in_all = |k: &K| members.contains(k) && sets.iter().all(|s| s.contains(k));
    let common = |l: &[K]| -> Vec<K> { l.iter().copied().filter(|k| in_all(k)).collect() };
    let (fb, fo, ft) = (common(lists[0]), common(lists[1]), common(lists[2]));
    let (by_o, by_t) = (fo != fb, ft != fb);
    let primary = if by_o && by_t && fo != ft {
        match clash {
            Some((key, what)) => cx.decide(
                key.to_string(),
                what.to_string(),
                Some(show(&fb)),
                show(&fo),
                show(&ft),
            ),
            None => Side::Ours,
        }
    } else if by_t {
        Side::Theirs
    } else {
        Side::Ours
    };
    let (first, second) = match primary {
        Side::Ours => (lists[1], lists[2]),
        Side::Theirs => (lists[2], lists[1]),
    };
    let mut out: Vec<K> = Vec::with_capacity(members.len());
    let mut placed: HashSet<K> = HashSet::new();
    for k in first {
        if members.contains(k) && placed.insert(*k) {
            out.push(*k);
        }
    }
    for l in [second, lists[0]] {
        for (i, k) in l.iter().enumerate() {
            if !members.contains(k) || placed.contains(k) {
                continue;
            }
            let at = l[..i]
                .iter()
                .rev()
                .find(|p| placed.contains(*p))
                .and_then(|p| out.iter().position(|x| x == p))
                .map_or(0, |i| i + 1);
            out.insert(at, *k);
            placed.insert(*k);
        }
    }
    for k in members {
        if placed.insert(*k) {
            out.push(*k);
        }
    }
    out
}

/// A list of unique ids merged id by id: kept unless a side removed it, added when a
/// side added it, in merged order.
fn merge_ids<K: Copy + Ord + Hash>(cx: &mut Cx, b: Option<&[K]>, o: &[K], t: &[K]) -> Vec<K> {
    let bl = b.unwrap_or(&[]);
    let members: BTreeSet<K> = o
        .iter()
        .chain(t)
        .filter(|k| !bl.contains(k) || (o.contains(k) && t.contains(k)))
        .copied()
        .collect();
    merge_order(cx, None, [bl, o, t], &members, |_| String::new())
}

/// A list whose entries have a key (`id`) merged entry by entry: each entry is one value,
/// so one side's change, add or delete wins and two different changes to one entry
/// conflict (key `<key>:<id>`, named by `name`, its value shown by `show`). In ours'
/// order, then theirs' additions.
#[allow(clippy::too_many_arguments)]
fn merge_keyed<V: PartialEq + Clone, K: Eq + Hash + Clone + std::fmt::Display>(
    cx: &mut Cx<'_>,
    key: &str,
    b: &[V],
    o: &[V],
    t: &[V],
    id: impl Fn(&V) -> K,
    name: impl Fn(&V) -> String,
    show: impl Fn(&V) -> String,
) -> Vec<V> {
    let find = |l: &[V], k: &K| l.iter().position(|v| id(v) == *k);
    let mut keys: Vec<K> = Vec::new();
    let mut seen = HashSet::new();
    for v in o.iter().chain(t).chain(b) {
        let k = id(v);
        if seen.insert(k.clone()) {
            keys.push(k);
        }
    }
    let mut out = Vec::new();
    for k in keys {
        let (bv, ov, tv) = (
            find(b, &k).map(|i| &b[i]),
            find(o, &k).map(|i| &o[i]),
            find(t, &k).map(|i| &t[i]),
        );
        let got = match pick3(bv, ov, tv) {
            Some(v) => v,
            None => {
                let any = ov.or(tv).or(bv).expect("listed");
                let text = |v: Option<&V>| v.map_or_else(|| "deleted it".to_string(), &show);
                match cx.decide(
                    format!("{key}:{k}"),
                    name(any),
                    bv.map(&show),
                    text(ov),
                    text(tv),
                ) {
                    Side::Ours => ov,
                    Side::Theirs => tv,
                }
            }
        };
        out.extend(got.cloned());
    }
    out
}

/// Ids in first-seen order: ours, then theirs, then the base.
fn union<K: Copy + Eq + Hash>(lists: [&[K]; 3]) -> Vec<K> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for l in lists {
        for k in l {
            if seen.insert(*k) {
                out.push(*k);
            }
        }
    }
    out
}

/// A short list for an order conflict: the first few names.
fn order_text(names: Vec<String>) -> String {
    const SHOWN: usize = 8;
    let more = names.len().saturating_sub(SHOWN);
    let mut s = names.into_iter().take(SHOWN).collect::<Vec<_>>().join(", ");
    if more > 0 {
        s.push_str(&format!(" and {more} more"));
    }
    if s.is_empty() {
        "nothing".into()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// New ids on both sides

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Category,
    RedFlagType,
    Element,
    RedFlag,
    Sheet,
    Board,
    SubBoard,
    Strip,
    Layout,
}

fn kinds(s: &Schedule) -> HashMap<u32, Kind> {
    let mut m = HashMap::new();
    for c in &s.categories {
        m.insert(c.id.0, Kind::Category);
    }
    for t in &s.red_flag_types {
        m.insert(t.id.0, Kind::RedFlagType);
    }
    for e in &s.elements {
        m.insert(e.id.0, Kind::Element);
        for f in &e.red_flags {
            m.insert(f.id.0, Kind::RedFlag);
        }
    }
    for sh in &s.sheets {
        m.insert(sh.id.0, Kind::Sheet);
    }
    for b in &s.boards {
        m.insert(b.id.0, Kind::Board);
        for sb in &b.sub_boards {
            m.insert(sb.id.0, Kind::SubBoard);
            for st in &sb.strips {
                m.insert(st.id.0, Kind::Strip);
            }
        }
        for st in &b.boneyard {
            m.insert(st.id.0, Kind::Strip);
        }
    }
    for l in &s.layouts.layouts {
        m.insert(l.id.0, Kind::Layout);
    }
    m
}

fn find_sub(s: &Schedule, id: u32) -> Option<&SubBoard> {
    s.boards
        .iter()
        .flat_map(|b| &b.sub_boards)
        .find(|x| x.id.0 == id)
}

/// A sub-board with its board.
fn sub_at(s: &Schedule, id: u32) -> Option<(BoardId, &SubBoard)> {
    s.boards.iter().find_map(|b| {
        b.sub_boards
            .iter()
            .find(|x| x.id.0 == id)
            .map(|x| (b.id, x))
    })
}

/// A strip with its board and list.
fn strip_at(s: &Schedule, id: u32) -> Option<(BoardId, Option<SubBoardId>, &Strip)> {
    s.boards.iter().find_map(|b| {
        strip_places(b)
            .get(&StripId(id))
            .map(|(l, _, st)| (b.id, *l, *st))
    })
}

fn find_strip(s: &Schedule, id: u32) -> Option<&Strip> {
    strip_at(s, id).map(|x| x.2)
}

/// Whether the thing with id `id` is the same in both schedules (a board by name, a
/// sub-board on the same board without its strips, a strip on the same board and list).
fn same_object(o: &Schedule, t: &Schedule, id: u32, k: Kind) -> bool {
    match k {
        Kind::Category => o.category(CategoryId(id)) == t.category(CategoryId(id)),
        Kind::Element => o.element(ElementId(id)) == t.element(ElementId(id)),
        Kind::RedFlagType => {
            o.red_flag_type(RedFlagTypeId(id)) == t.red_flag_type(RedFlagTypeId(id))
        }
        Kind::RedFlag => o.red_flag(RedFlagId(id)) == t.red_flag(RedFlagId(id)),
        Kind::Sheet => o.sheet(SheetId(id)) == t.sheet(SheetId(id)),
        Kind::Board => {
            o.board(BoardId(id)).map(|b| &b.name) == t.board(BoardId(id)).map(|b| &b.name)
        }
        Kind::SubBoard => {
            let key =
                |(b, x): (BoardId, &SubBoard)| (b, x.name.clone(), x.start, x.calendar.clone());
            sub_at(o, id).map(key) == sub_at(t, id).map(key)
        }
        Kind::Strip => strip_at(o, id) == strip_at(t, id),
        Kind::Layout => {
            let id = crate::layouts::LayoutId(id);
            o.layouts.get(id) == t.layouts.get(id)
        }
    }
}

/// Theirs with the ids of what both sides added lined up with ours (see the module
/// notes), how many got new ids, and the next free id.
fn align(base: &Schedule, ours: &Schedule, theirs: &Schedule) -> (Schedule, usize, u32) {
    let kb = kinds(base);
    let ko = kinds(ours);
    let kt = kinds(theirs);
    let new_o: BTreeSet<u32> = ko.keys().filter(|k| !kb.contains_key(k)).copied().collect();
    let new_t: BTreeSet<u32> = kt.keys().filter(|k| !kb.contains_key(k)).copied().collect();
    let mut map: BTreeMap<u32, u32> = BTreeMap::new();
    let mut targets: HashSet<u32> = HashSet::new();
    for c in theirs.categories.iter().filter(|c| new_t.contains(&c.id.0)) {
        if let Some(oc) = ours.categories.iter().find(|x| {
            new_o.contains(&x.id.0) && same_name(&x.name, &c.name) && !targets.contains(&x.id.0)
        }) {
            map.insert(c.id.0, oc.id.0);
            targets.insert(oc.id.0);
        }
    }
    for c in theirs
        .red_flag_types
        .iter()
        .filter(|c| new_t.contains(&c.id.0))
    {
        if let Some(oc) = ours.red_flag_types.iter().find(|x| {
            new_o.contains(&x.id.0) && same_name(&x.name, &c.name) && !targets.contains(&x.id.0)
        }) {
            map.insert(c.id.0, oc.id.0);
            targets.insert(oc.id.0);
        }
    }
    for e in theirs.elements.iter().filter(|e| new_t.contains(&e.id.0)) {
        let cat = map.get(&e.category.0).copied().unwrap_or(e.category.0);
        if let Some(oe) = ours.elements.iter().find(|x| {
            new_o.contains(&x.id.0)
                && x.category.0 == cat
                && same_name(&x.name, &e.name)
                && !targets.contains(&x.id.0)
        }) {
            map.insert(e.id.0, oe.id.0);
            targets.insert(oe.id.0);
        }
    }
    let mut next = base.next_id.max(ours.next_id).max(theirs.next_id);
    let mut renumbered = 0;
    // Parents before children, so a thing whose board, category or sheet got a new id
    // gets one too.
    let order = [
        Kind::Category,
        Kind::RedFlagType,
        Kind::Element,
        Kind::RedFlag,
        Kind::Sheet,
        Kind::Board,
        Kind::SubBoard,
        Kind::Strip,
        Kind::Layout,
    ];
    for kind in order {
        for &id in new_t.iter().filter(|id| kt[*id] == kind) {
            if map.contains_key(&id) {
                continue;
            }
            let moved = |x: u32| map.get(&x).is_some_and(|n| *n != x);
            let parent_moved = match kind {
                Kind::Element => theirs.element(ElementId(id)).is_some_and(|e| {
                    moved(e.category.0)
                        || e.links.iter().any(|l| moved(l.0))
                        || e.red_flags.iter().any(|f| moved(f.kind.0))
                }),
                Kind::RedFlag => theirs
                    .red_flag(RedFlagId(id))
                    .is_some_and(|(e, f)| moved(e.0) || moved(f.kind.0)),
                Kind::Sheet => theirs
                    .sheet(SheetId(id))
                    .is_some_and(|sh| sh.elements.iter().any(|e| moved(e.0))),
                Kind::SubBoard => sub_at(theirs, id).is_some_and(|(b, _)| moved(b.0)),
                Kind::Strip => strip_at(theirs, id).is_some_and(|(b, l, st)| {
                    moved(b.0)
                        || l.is_some_and(|x| moved(x.0))
                        || matches!(st.kind, StripKind::Sheet(sh) if moved(sh.0))
                }),
                _ => false,
            };
            let clash = targets.contains(&id)
                || (new_o.contains(&id)
                    && (parent_moved || ko[&id] != kind || !same_object(ours, theirs, id, kind)));
            if clash {
                map.insert(id, next);
                next = next.saturating_add(1);
                renumbered += 1;
            }
        }
    }
    let mut t = theirs.clone();
    if map.iter().any(|(a, b)| a != b) {
        remap(&mut t, &map);
    }
    t.next_id = next;
    (t, renumbered, next)
}

/// Every id in the schedule, and every reference to one, through `map`.
fn remap(s: &mut Schedule, map: &BTreeMap<u32, u32>) {
    let m = |x: &mut u32| {
        if let Some(n) = map.get(x) {
            *x = *n;
        }
    };
    for c in &mut s.categories {
        m(&mut c.id.0);
    }
    for t in &mut s.red_flag_types {
        m(&mut t.id.0);
    }
    for e in &mut s.elements {
        m(&mut e.id.0);
        m(&mut e.category.0);
        for l in &mut e.links {
            m(&mut l.0);
        }
        for f in &mut e.red_flags {
            m(&mut f.id.0);
            m(&mut f.kind.0);
        }
    }
    for sh in &mut s.sheets {
        m(&mut sh.id.0);
        for e in &mut sh.elements {
            m(&mut e.0);
        }
        for q in &mut sh.quantities {
            m(&mut q.element.0);
        }
    }
    let strip = |st: &mut Strip| {
        m(&mut st.id.0);
        if let StripKind::Sheet(id) = &mut st.kind {
            m(&mut id.0);
        }
    };
    for l in &mut s.layouts.layouts {
        m(&mut l.id.0);
    }
    if let Some(l) = &mut s.layouts.one_line {
        m(&mut l.0);
    }
    for b in &mut s.boards {
        m(&mut b.id.0);
        for l in [
            &mut b.uses.strips,
            &mut b.uses.day_breaks,
            &mut b.uses.shoot_day,
        ]
        .into_iter()
        .flatten()
        {
            m(&mut l.0);
        }
        for sb in &mut b.sub_boards {
            m(&mut sb.id.0);
            sb.strips.iter_mut().for_each(strip);
            for ev in &mut sb.calendar.events {
                for e in &mut ev.elements {
                    m(&mut e.0);
                }
            }
        }
        b.boneyard.iter_mut().for_each(strip);
    }
    let p = &mut s.plan;
    p.call_times.iter_mut().for_each(|c| m(&mut c.day.0));
    p.days_shot.iter_mut().for_each(|d| m(&mut d.day.0));
    for x in &mut p.person_calls {
        m(&mut x.day.0);
        m(&mut x.element.0);
    }
    for x in &mut p.scenes_shot {
        m(&mut x.day.0);
        m(&mut x.sheet.0);
    }
    p.turnarounds.iter_mut().for_each(|x| m(&mut x.category.0));
    p.pinned.iter_mut().for_each(|x| m(&mut x.0));
    p.omitted.iter_mut().for_each(|x| m(&mut x.0));
    for d in &mut p.drafts {
        for l in [&mut d.changed, &mut d.added, &mut d.omitted] {
            l.iter_mut().for_each(|x| m(&mut x.0));
        }
    }
    for l in &mut p.locks {
        m(&mut l.board.0);
        for sc in &mut l.scenes {
            m(&mut sc.sheet.0);
            m(&mut sc.sub_board.0);
            sc.cast.iter_mut().for_each(|x| m(&mut x.0));
        }
    }
    for r in &mut s.reports {
        for c in &mut r.options.categories {
            m(&mut c.0);
        }
        for e in &mut r.options.elements {
            m(&mut e.0);
        }
    }
}

// ---------------------------------------------------------------------------
// What each side touched

/// Elements a side newly refers to: tagged on a sheet, put on an event, linked.
fn new_refs(base: &Schedule, x: &Schedule) -> HashSet<ElementId> {
    let mut out = HashSet::new();
    for sh in &x.sheets {
        let before = base.sheet(sh.id);
        for e in &sh.elements {
            if before.is_none_or(|b| !b.elements.contains(e)) {
                out.insert(*e);
            }
        }
    }
    for el in &x.elements {
        let before = base.element(el.id);
        for l in &el.links {
            if before.is_none_or(|b| !b.links.contains(l)) {
                out.insert(*l);
            }
        }
    }
    for b in &x.boards {
        for sb in &b.sub_boards {
            let before = base
                .board(b.id)
                .and_then(|bb| bb.sub_boards.iter().find(|y| y.id == sb.id))
                .map_or(&[][..], |y| &y.calendar.events[..]);
            let (_, new) = event_changes(before, &sb.calendar.events);
            for ev in new {
                out.extend(ev.elements.iter().copied());
            }
        }
    }
    out
}

/// Red flag types a side newly used: on a new red flag, or one given that type.
fn type_uses(base: &Schedule, x: &Schedule) -> HashSet<RedFlagTypeId> {
    x.elements
        .iter()
        .flat_map(|e| &e.red_flags)
        .filter(|f| base.red_flag(f.id).is_none_or(|(_, b)| b.kind != f.kind))
        .map(|f| f.kind)
        .collect()
}

/// Categories a side put elements in (new elements, or ones moved there).
fn gained(base: &Schedule, x: &Schedule) -> HashSet<CategoryId> {
    x.elements
        .iter()
        .filter(|e| base.element(e.id).is_none_or(|b| b.category != e.category))
        .map(|e| e.category)
        .collect()
}

// ---------------------------------------------------------------------------
// The merge

struct M<'a> {
    b: &'a Schedule,
    o: &'a Schedule,
    t: &'a Schedule,
    refs_o: HashSet<ElementId>,
    refs_t: HashSet<ElementId>,
    gained_o: HashSet<CategoryId>,
    gained_t: HashSet<CategoryId>,
    used_o: HashSet<RedFlagTypeId>,
    used_t: HashSet<RedFlagTypeId>,
    /// The side a fallback keeps: the side conflicts are settled for by default, so the
    /// two orders of a merge give the same schedule (decision 174).
    keep: Side,
}

/// A strip's list, index and strip on one side.
type At3<'s> = (Option<SubBoardId>, usize, &'s Strip);

/// Where a strip comes from: one side, or both (with the base, if it had it).
enum Which<'s> {
    One(At3<'s>),
    Both(Option<At3<'s>>, At3<'s>, At3<'s>),
}

/// An element kept against the side that deleted it.
struct Revived {
    id: ElementId,
    keeper: Side,
}

impl M<'_> {
    fn side(&self, s: Side) -> &Schedule {
        match s {
            Side::Ours => self.o,
            Side::Theirs => self.t,
        }
    }

    /// The side fallbacks keep, then the other side.
    fn kept(&self) -> &Schedule {
        self.side(self.keep)
    }

    fn other(&self) -> &Schedule {
        match self.keep {
            Side::Ours => self.t,
            Side::Theirs => self.o,
        }
    }

    /// `ours'` or `theirs'`, for the side fallbacks keep.
    fn whose(&self) -> &'static str {
        match self.keep {
            Side::Ours => "ours'",
            Side::Theirs => "theirs'",
        }
    }

    // ---- names, from whichever side has the thing

    fn category_name(&self, id: CategoryId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.category(id))
            .map_or(format!("category {}", id.0), |c| quoted(&c.name))
    }

    fn red_flag_type_name(&self, id: RedFlagTypeId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.red_flag_type(id))
            .map_or(format!("red flag type {}", id.0), |c| quoted(&c.name))
    }

    fn element_name(&self, id: ElementId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.element(id))
            .map_or(format!("element {}", id.0), |e| e.name.clone())
    }

    fn element_title(&self, id: ElementId) -> String {
        match [self.o, self.t, self.b].iter().find_map(|s| s.element(id)) {
            Some(e) => {
                let cat = [self.o, self.t, self.b]
                    .iter()
                    .find_map(|s| s.category(e.category))
                    .map_or(String::new(), |c| format!(" ({})", c.name));
                format!("Element {}{cat}", quoted(&e.name))
            }
            None => format!("Element {}", id.0),
        }
    }

    fn sheet_title(&self, id: SheetId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.sheet(id))
            .map_or(upper_first(GONE_SHEET), |s| upper_first(&sheet_name(s)))
    }

    fn sheet_text(&self, id: SheetId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.sheet(id))
            .map_or(GONE_SHEET.to_string(), sheet_name)
    }

    fn board_name(&self, id: BoardId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| s.board(id))
            .map_or(format!("board {}", id.0), |b| quoted(&b.name))
    }

    fn list_name(&self, list: Option<SubBoardId>) -> String {
        match list {
            None => "the Boneyard".to_string(),
            Some(id) => [self.o, self.t, self.b]
                .iter()
                .find_map(|s| find_sub(s, id.0))
                .map_or(format!("sub-board {}", id.0), |x| quoted(&x.name)),
        }
    }

    fn strip_name(&self, st: &Strip) -> String {
        match &st.kind {
            StripKind::Sheet(id) => [self.o, self.t, self.b]
                .iter()
                .find_map(|s| s.sheet(*id))
                .map_or(GONE_SHEET.to_string(), sheet_name),
            StripKind::Banner(b) => format!("banner {}", quoted(&b.text)),
            StripKind::DayBreak => "day break".to_string(),
        }
    }

    fn strip_name_by_id(&self, id: StripId) -> String {
        [self.o, self.t, self.b]
            .iter()
            .find_map(|s| find_strip(s, id.0))
            .map_or(format!("strip {}", id.0), |st| self.strip_name(st))
    }

    // ---- categories

    fn categories(&self, cx: &mut Cx) -> Vec<Category> {
        let ids = |s: &Schedule| s.categories.iter().map(|c| c.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(self.b), ids(self.o), ids(self.t));
        let mut got: HashMap<CategoryId, Category> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (b, o, t) = (
                self.b.category(id),
                self.o.category(id),
                self.t.category(id),
            );
            let what = format!("Category {}", self.category_name(id));
            let key = format!("category:{}", id.0);
            let touched =
                |x: Option<&Category>, gained: &HashSet<CategoryId>| x != b || gained.contains(&id);
            let c = match exist(
                cx,
                &key,
                &what,
                b,
                o,
                t,
                touched(o, &self.gained_o),
                touched(t, &self.gained_t),
            ) {
                Got::Gone => continue,
                Got::One(c, _) => c.clone(),
                Got::Both(b, o, t) => {
                    let k = |f: &str| format!("{key}:{f}");
                    let w = |l: &str| format!("{what} {l}");
                    let rules = merge_rules(
                        cx,
                        &key,
                        &format!("{what} default"),
                        b.map(|x| &x.defaults),
                        &o.defaults,
                        &t.defaults,
                    );
                    Category {
                        id,
                        name: merge_field!(cx, k, w, (b, o, t), name, "name", "name", |v| quoted(
                            v
                        )),
                        auto_board_id: merge_field!(
                            cx,
                            k,
                            w,
                            (b, o, t),
                            auto_board_id,
                            "auto_board_id",
                            "automatic board IDs",
                            |v| on_off(*v)
                        ),
                        exclude_from_board: merge_field!(
                            cx,
                            k,
                            w,
                            (b, o, t),
                            exclude_from_board,
                            "exclude_from_board",
                            "left off the board",
                            |v| on_off(*v)
                        ),
                        defaults: rules,
                        // Fixed when the category is made; both sides agree.
                        cast: o.cast,
                        daily_quantity: merge_field!(
                            cx,
                            k,
                            w,
                            (b, o, t),
                            daily_quantity,
                            "daily_quantity",
                            "use in daily quantity",
                            |v| on_off(*v)
                        ),
                        properties: merge_field!(
                            cx,
                            k,
                            w,
                            (b, o, t),
                            properties,
                            "properties",
                            "properties",
                            |v| list_text(v)
                        ),
                        multi_line: merge_field!(
                            cx,
                            k,
                            w,
                            (b, o, t),
                            multi_line,
                            "multi_line",
                            "properties of several lines",
                            |v| list_text(v)
                        ),
                    }
                }
            };
            // Only the merged properties can be of several lines, in their order.
            let mut c = c;
            let props = c.properties.clone();
            c.multi_line.retain(|m| props.contains(m));
            c.multi_line
                .sort_by_key(|m| props.iter().position(|p| p == m));
            c.multi_line.dedup();
            got.insert(id, c);
        }
        let members: BTreeSet<CategoryId> = got.keys().copied().collect();
        let order = merge_order(
            cx,
            Some(("categories:order", "Category order")),
            [&lb, &lo, &lt],
            &members,
            |l| order_text(l.iter().map(|c| self.category_name(*c)).collect()),
        );
        order.into_iter().filter_map(|id| got.remove(&id)).collect()
    }

    // ---- red flag types

    fn red_flag_types(&self, cx: &mut Cx) -> Vec<RedFlagType> {
        let ids = |s: &Schedule| s.red_flag_types.iter().map(|c| c.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(self.b), ids(self.o), ids(self.t));
        let mut got: HashMap<RedFlagTypeId, RedFlagType> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (b, o, t) = (
                self.b.red_flag_type(id),
                self.o.red_flag_type(id),
                self.t.red_flag_type(id),
            );
            let what = format!("Red flag type {}", self.red_flag_type_name(id));
            let key = format!("red_flag_type:{}", id.0);
            let touched = |x: Option<&RedFlagType>, used: &HashSet<RedFlagTypeId>| {
                x != b || used.contains(&id)
            };
            let c = match exist(
                cx,
                &key,
                &what,
                b,
                o,
                t,
                touched(o, &self.used_o),
                touched(t, &self.used_t),
            ) {
                Got::Gone => continue,
                Got::One(c, _) => c.clone(),
                Got::Both(b, o, t) => RedFlagType {
                    id,
                    name: field(
                        cx,
                        &format!("{key}:name"),
                        &format!("{what} name"),
                        b.map(|x| &x.name),
                        &o.name,
                        &t.name,
                        |v| quoted(v),
                    ),
                    color: field(
                        cx,
                        &format!("{key}:color"),
                        &format!("{what} color"),
                        b.map(|x| &x.color),
                        &o.color,
                        &t.color,
                        |v| crate::diff::color_text(*v),
                    ),
                    builtin: field(
                        cx,
                        &format!("{key}:builtin"),
                        &format!("{what} built in"),
                        b.map(|x| &x.builtin),
                        &o.builtin,
                        &t.builtin,
                        |v| crate::diff::builtin_text(v.as_ref()),
                    ),
                },
            };
            got.insert(id, c);
        }
        let members: BTreeSet<RedFlagTypeId> = got.keys().copied().collect();
        let order = merge_order(
            cx,
            Some(("red_flag_types:order", "Red flag type order")),
            [&lb, &lo, &lt],
            &members,
            |l| order_text(l.iter().map(|c| self.red_flag_type_name(*c)).collect()),
        );
        order.into_iter().filter_map(|id| got.remove(&id)).collect()
    }

    /// An element's red flags merged id by id, field by field.
    fn red_flags(
        &self,
        cx: &mut Cx,
        what: &str,
        b: Option<&[RedFlag]>,
        o: &[RedFlag],
        t: &[RedFlag],
    ) -> Vec<RedFlag> {
        let bl = b.unwrap_or(&[]);
        let ids = |l: &[RedFlag]| l.iter().map(|f| f.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(bl), ids(o), ids(t));
        fn find(l: &[RedFlag], id: RedFlagId) -> Option<&RedFlag> {
            l.iter().find(|f| f.id == id)
        }
        let mut got: HashMap<RedFlagId, RedFlag> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (fb, fo, ft) = (find(bl, id), find(o, id), find(t, id));
            let key = format!("red_flag:{}", id.0);
            // Named by its type and dates as it was, never its id (decision 162).
            let w = match fb.or(fo).or(ft) {
                Some(f) => format!(
                    "{what} red flag {} ({})",
                    self.red_flag_type_name(f.kind),
                    f.dates.text()
                ),
                None => format!("{what} red flag"),
            };
            let f = match exist(cx, &key, &w, fb, fo, ft, fo != fb, ft != fb) {
                Got::Gone => continue,
                Got::One(f, _) => f.clone(),
                Got::Both(fb, fo, ft) => {
                    let k = |x: &str| format!("{key}:{x}");
                    let l = |x: &str| format!("{w} {x}");
                    RedFlag {
                        id,
                        kind: field(
                            cx,
                            &k("type"),
                            &l("type"),
                            fb.map(|x| &x.kind),
                            &fo.kind,
                            &ft.kind,
                            |v| self.red_flag_type_name(*v),
                        ),
                        dates: field(
                            cx,
                            &k("dates"),
                            &l("dates"),
                            fb.map(|x| &x.dates),
                            &fo.dates,
                            &ft.dates,
                            |v| v.text(),
                        ),
                        note: field(
                            cx,
                            &k("note"),
                            &l("note"),
                            fb.map(|x| &x.note),
                            &fo.note,
                            &ft.note,
                            |v| quoted(v),
                        ),
                        hard: field(
                            cx,
                            &k("hard"),
                            &l("hard"),
                            fb.map(|x| &x.hard),
                            &fo.hard,
                            &ft.hard,
                            |v| on_off(*v),
                        ),
                    }
                }
            };
            got.insert(id, f);
        }
        let members: BTreeSet<RedFlagId> = got.keys().copied().collect();
        let order = merge_order(cx, None, [&lb, &lo, &lt], &members, |_| String::new());
        order.into_iter().filter_map(|id| got.remove(&id)).collect()
    }

    // ---- the plan

    /// The planning data, each list entry by entry (decision 91): a location by name, a
    /// call time by its day break, a turnaround by category, a lock by name, each one
    /// value; pins and omitted marks as sets; the long day limit as one field. Two
    /// different changes to one entry conflict, with the entry named.
    fn plan(&self, cx: &mut Cx) -> crate::plan::Plan {
        use crate::plan::{clock_text, degrees_text, hours_text, Plan};
        let (b, o, t) = (&self.b.plan, &self.o.plan, &self.t.plan);
        let fold = |n: &str| n.trim().to_lowercase();
        Plan {
            locations: merge_keyed(
                cx,
                "plan:location",
                &b.locations,
                &o.locations,
                &t.locations,
                |l| fold(&l.name),
                |l| format!("Map location {}", quoted(&l.name)),
                |l| {
                    let dst = l
                        .daylight_saving
                        .map_or(String::new(), |r| format!(", {} summer time", r.label()));
                    format!(
                        "{}, {} (UTC {}{dst})",
                        degrees_text(l.latitude),
                        degrees_text(l.longitude),
                        crate::plan::offset_text(l.utc_offset)
                    )
                },
            ),
            call_times: merge_keyed(
                cx,
                "plan:call_time",
                &b.call_times,
                &o.call_times,
                &t.call_times,
                |c| c.day.0,
                |c| {
                    format!(
                        "Call time of the day closed by {}",
                        self.strip_name_by_id(c.day)
                    )
                },
                |c| clock_text(c.minutes as i32),
            ),
            days_shot: merge_keyed(
                cx,
                "plan:day_shot",
                &b.days_shot,
                &o.days_shot,
                &t.days_shot,
                |d| d.day.0,
                |d| format!("The day closed by {} as shot", self.strip_name_by_id(d.day)),
                |d| {
                    let date = d.date.map_or("no date".to_string(), |x| x.to_string());
                    let time = |m: Option<u32>| m.map_or("?".to_string(), |m| clock_text(m as i32));
                    format!("{date}, call {}, wrap {}", time(d.call), time(d.wrap))
                },
            ),
            person_calls: merge_keyed(
                cx,
                "plan:person_call",
                &b.person_calls,
                &o.person_calls,
                &t.person_calls,
                |x| format!("{}:{}", x.element.0, x.day.0),
                |x| {
                    format!(
                        "The call of {} on the day closed by {}",
                        [self.o, self.t, self.b]
                            .iter()
                            .find_map(|s| s.element(x.element))
                            .map_or("an element that is gone".to_string(), |e| e.name.clone()),
                        self.strip_name_by_id(x.day)
                    )
                },
                |x| {
                    let time = |m: Option<u32>| {
                        m.map_or("the company's".to_string(), |m| clock_text(m as i32))
                    };
                    format!("call {}, wrap {}", time(x.call), time(x.wrap))
                },
            ),
            scenes_shot: merge_keyed(
                cx,
                "plan:scene_shot",
                &b.scenes_shot,
                &o.scenes_shot,
                &t.scenes_shot,
                |x| format!("{}:{}", x.sheet.0, x.day.0),
                |x| {
                    format!(
                        "What was shot of {} on the day closed by {}",
                        [self.o, self.t, self.b]
                            .iter()
                            .find_map(|s| s.sheet(x.sheet))
                            .map_or(GONE_SHEET.to_string(), sheet_name),
                        self.strip_name_by_id(x.day)
                    )
                },
                |x| {
                    format!(
                        "{} eighths{}",
                        x.eighths.0,
                        if x.complete { ", finished" } else { "" }
                    )
                },
            ),
            long_day: field(
                cx,
                "plan:long_day",
                "Long day limit",
                Some(&b.long_day),
                &o.long_day,
                &t.long_day,
                |v| v.map_or("the default".to_string(), hours_text),
            ),
            turnarounds: merge_keyed(
                cx,
                "plan:turnaround",
                &b.turnarounds,
                &o.turnarounds,
                &t.turnarounds,
                |x| x.category.0,
                |x| format!("Turnaround for {}", self.category_name(x.category)),
                |x| hours_text(x.minutes),
            ),
            locks: merge_keyed(
                cx,
                "plan:lock",
                &b.locks,
                &o.locks,
                &t.locks,
                |l| fold(&l.name),
                |l| format!("Schedule lock {}", quoted(&l.name)),
                |l| match l.made {
                    Some(d) => format!("{} scenes, made {d}", l.scenes.len()),
                    None => format!("{} scenes", l.scenes.len()),
                },
            ),
            drafts: merge_keyed(
                cx,
                "plan:draft",
                &b.drafts,
                &o.drafts,
                &t.drafts,
                |d| fold(&d.color),
                |d| format!("The {} draft", d.color),
                |d| match d.date {
                    Some(x) => format!("{} scenes changed, made {x}", d.changed.len()),
                    None => format!("{} scenes changed", d.changed.len()),
                },
            ),
            // Kept conflicts come and go whole: one marked done on either side stays done.
            conflicts: merge_keyed(
                cx,
                "plan:conflict",
                &b.conflicts,
                &o.conflicts,
                &t.conflicts,
                |c| c.key.clone(),
                |c| format!("The conflict to review on {}", c.what),
                |c| c.text(),
            ),
            pinned: merge_ids(cx, Some(&b.pinned), &o.pinned, &t.pinned),
            omitted: merge_ids(cx, Some(&b.omitted), &o.omitted, &t.omitted),
        }
    }

    // ---- elements

    /// Merges the elements into `s.elements`; returns those kept against a delete.
    fn elements(&self, cx: &mut Cx, s: &mut Schedule) -> Vec<Revived> {
        let ids = |s: &Schedule| s.elements.iter().map(|e| e.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(self.b), ids(self.o), ids(self.t));
        let mut got: HashMap<ElementId, Element> = HashMap::new();
        let mut revived = Vec::new();
        for id in union([&lo, &lt, &lb]) {
            let (b, o, t) = (self.b.element(id), self.o.element(id), self.t.element(id));
            let what = self.element_title(id);
            let key = format!("element:{}", id.0);
            let touched =
                |x: Option<&Element>, refs: &HashSet<ElementId>| x != b || refs.contains(&id);
            let e = match exist(
                cx,
                &key,
                &what,
                b,
                o,
                t,
                touched(o, &self.refs_o),
                touched(t, &self.refs_t),
            ) {
                Got::Gone => continue,
                Got::One(e, keeper) => {
                    if b.is_some() {
                        revived.push(Revived { id, keeper });
                    }
                    e.clone()
                }
                Got::Both(b, o, t) => self.merge_element(cx, &key, &what, b, o, t, &s.categories),
            };
            got.insert(id, e);
        }
        let members: BTreeSet<ElementId> = got.keys().copied().collect();
        let order = merge_order(cx, None, [&lb, &lo, &lt], &members, |_| String::new());
        s.elements = order.into_iter().filter_map(|id| got.remove(&id)).collect();
        revived
    }

    #[allow(clippy::too_many_arguments)]
    fn merge_element(
        &self,
        cx: &mut Cx,
        key: &str,
        what: &str,
        b: Option<&Element>,
        o: &Element,
        t: &Element,
        cats: &[Category],
    ) -> Element {
        let k = |f: &str| format!("{key}:{f}");
        let w = |l: &str| format!("{what} {l}");
        let category = merge_field!(cx, k, w, (b, o, t), category, "category", "category", |c| {
            self.category_name(*c)
        });
        // Property values under the merged category's names, so a value set on one side
        // follows a rename on the other.
        let names = |x: &Schedule| {
            x.category(category)
                .map(|c| c.properties.clone())
                .unwrap_or_default()
        };
        let merged = cats
            .iter()
            .find(|c| c.id == category)
            .map(|c| c.properties.clone())
            .unwrap_or_default();
        let nb = names(self.b);
        let (ro, rt) = (renames(&nb, &names(self.o)), renames(&nb, &names(self.t)));
        let carry = |v: &[crate::model::PropertyValue], own: &[(String, String)]| {
            v.iter()
                .map(|v| {
                    let base = own
                        .iter()
                        .find(|(_, n)| *n == v.name)
                        .map_or(&v.name, |(b, _)| b);
                    let name = [Some(base), find_rename(&ro, base), find_rename(&rt, base)]
                        .into_iter()
                        .flatten()
                        .find(|n| merged.contains(n))
                        .unwrap_or(&v.name);
                    crate::model::PropertyValue {
                        name: name.clone(),
                        value: v.value.clone(),
                    }
                })
                .collect::<Vec<_>>()
        };
        let pb = b.map(|x| carry(&x.properties, &[]));
        let (po, pt) = (carry(&o.properties, &ro), carry(&t.properties, &rt));
        let properties = field(
            cx,
            &k("properties"),
            &w("properties"),
            pb.as_ref(),
            &po,
            &pt,
            |v| crate::diff::properties_text(v),
        );
        for v in &properties {
            let was = pb.as_ref().is_some_and(|x| x.contains(v));
            if !merged.contains(&v.name) && !was {
                cx.fallbacks.push(format!(
                    "{what}: its {:?} value {:?} is left out: the property is gone",
                    v.name, v.value
                ));
            }
        }
        Element {
            id: o.id,
            category,
            name: merge_field!(cx, k, w, (b, o, t), name, "name", "name", |v| quoted(v)),
            board_id: merge_field!(cx, k, w, (b, o, t), board_id, "board_id", "board ID", |v| v
                .as_deref()
                .map_or("none".to_string(), quoted)),
            board_id_locked: merge_field!(
                cx,
                k,
                w,
                (b, o, t),
                board_id_locked,
                "board_id_locked",
                "board ID lock",
                |v| on_off(*v)
            ),
            exclude_from_board: merge_field!(
                cx,
                k,
                w,
                (b, o, t),
                exclude_from_board,
                "exclude_from_board",
                "left off the board",
                |v| on_off(*v)
            ),
            rules: merge_rules(cx, key, what, b.map(|x| &x.rules), &o.rules, &t.rules),
            notes: merge_field!(cx, k, w, (b, o, t), notes, "notes", "notes", |v| quoted(v)),
            links: merge_ids(cx, b.map(|x| &x.links[..]), &o.links, &t.links),
            red_flags: self.red_flags(
                cx,
                what,
                b.map(|x| &x.red_flags[..]),
                &o.red_flags,
                &t.red_flags,
            ),
            properties,
            images: merge_field!(cx, k, w, (b, o, t), images, "images", "images", |v| {
                list_text(v)
            }),
        }
    }

    // ---- sheets

    fn sheets(&self, cx: &mut Cx) -> Vec<Sheet> {
        let ids = |s: &Schedule| s.sheets.iter().map(|e| e.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(self.b), ids(self.o), ids(self.t));
        let mut got: HashMap<SheetId, Sheet> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (b, o, t) = (self.b.sheet(id), self.o.sheet(id), self.t.sheet(id));
            let what = self.sheet_title(id);
            let key = format!("sheet:{}", id.0);
            let sh = match exist(cx, &key, &what, b, o, t, o != b, t != b) {
                Got::Gone => continue,
                Got::One(sh, _) => sh.clone(),
                Got::Both(b, o, t) => merge_sheet(cx, &key, &what, b, o, t),
            };
            got.insert(id, sh);
        }
        let members: BTreeSet<SheetId> = got.keys().copied().collect();
        let order = merge_order(cx, None, [&lb, &lo, &lt], &members, |_| String::new());
        order.into_iter().filter_map(|id| got.remove(&id)).collect()
    }

    /// An element kept against a delete keeps the tags and links the deleting side took
    /// away with it.
    fn revive_tags(&self, s: &mut Schedule, revived: &[Revived]) {
        for r in revived {
            let keeper = self.side(r.keeper);
            for sh in &mut s.sheets {
                let had = self
                    .b
                    .sheet(sh.id)
                    .is_some_and(|x| x.elements.contains(&r.id));
                let kept = keeper
                    .sheet(sh.id)
                    .is_some_and(|x| x.elements.contains(&r.id));
                if had && kept && !sh.elements.contains(&r.id) {
                    sh.elements.push(r.id);
                }
            }
            for e in &mut s.elements {
                let had = self
                    .b
                    .element(e.id)
                    .is_some_and(|x| x.links.contains(&r.id));
                let kept = keeper
                    .element(e.id)
                    .is_some_and(|x| x.links.contains(&r.id));
                if had && kept && !e.links.contains(&r.id) {
                    e.links.push(r.id);
                }
            }
        }
    }

    // ---- boards

    fn boards(&self, cx: &mut Cx, s: &Schedule) -> Vec<Board> {
        let ids = |s: &Schedule| s.boards.iter().map(|e| e.id).collect::<Vec<_>>();
        let (lb, lo, lt) = (ids(self.b), ids(self.o), ids(self.t));
        let sheets: HashSet<SheetId> = s.sheets.iter().map(|x| x.id).collect();
        let mut got: HashMap<BoardId, Board> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (b, o, t) = (self.b.board(id), self.o.board(id), self.t.board(id));
            let what = format!("Board {}", self.board_name(id));
            let key = format!("board:{}", id.0);
            let board = match exist(cx, &key, &what, b, o, t, o != b, t != b) {
                Got::Gone => continue,
                Got::One(x, _) => x.clone(),
                Got::Both(b, o, t) => self.merge_board(cx, &key, &what, b, o, t, &sheets),
            };
            got.insert(id, board);
        }
        let members: BTreeSet<BoardId> = got.keys().copied().collect();
        let order = merge_order(cx, None, [&lb, &lo, &lt], &members, |_| String::new());
        order.into_iter().filter_map(|id| got.remove(&id)).collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn merge_board(
        &self,
        cx: &mut Cx,
        key: &str,
        what: &str,
        b: Option<&Board>,
        o: &Board,
        t: &Board,
        sheets: &HashSet<SheetId>,
    ) -> Board {
        let name = field(
            cx,
            &format!("{key}:name"),
            &format!("{what} name"),
            b.map(|x| &x.name),
            &o.name,
            &t.name,
            |v| quoted(v),
        );
        let strip_layout = field(
            cx,
            &format!("{key}:strip_layout"),
            &format!("{what} strip layout"),
            b.map(|x| &x.strip_layout),
            &o.strip_layout,
            &t.strip_layout,
            |v| v.text(self.o),
        );

        // Sub-boards, without their strips.
        let subs = |x: Option<&Board>| {
            x.map_or(Vec::new(), |x| {
                x.sub_boards.iter().map(|s| s.id).collect::<Vec<_>>()
            })
        };
        let (lb, lo, lt) = (subs(b), subs(Some(o)), subs(Some(t)));
        fn find(x: Option<&Board>, id: SubBoardId) -> Option<&SubBoard> {
            x.and_then(|x| x.sub_boards.iter().find(|s| s.id == id))
        }
        let bname = self.board_name(o.id);
        let mut got: HashMap<SubBoardId, SubBoard> = HashMap::new();
        // Sub-boards kept against the side that deleted them.
        let mut revived: HashMap<SubBoardId, Side> = HashMap::new();
        for id in union([&lo, &lt, &lb]) {
            let (sb, so, st) = (find(b, id), find(Some(o), id), find(Some(t), id));
            let sw = format!("Sub-board {} (board {bname})", self.list_name(Some(id)));
            let sk = format!("{key}:sub_board:{}", id.0);
            let sub = match exist(cx, &sk, &sw, sb, so, st, so != sb, st != sb) {
                Got::Gone => continue,
                Got::One(x, keeper) => {
                    if sb.is_some() {
                        let deleter = match keeper {
                            Side::Ours => Side::Theirs,
                            Side::Theirs => Side::Ours,
                        };
                        revived.insert(id, deleter);
                    }
                    SubBoard {
                        strips: Vec::new(),
                        ..x.clone()
                    }
                }
                Got::Both(sb, so, st) => {
                    let k = |f: &str| format!("{sk}:{f}");
                    let w = |l: &str| format!("{sw} {l}");
                    SubBoard {
                        id,
                        name: merge_field!(cx, k, w, (sb, so, st), name, "name", "name", |v| {
                            quoted(v)
                        }),
                        start: merge_field!(cx, k, w, (sb, so, st), start, "start", "start", |v| v
                            .map_or("none".to_string(), |d| d.to_string())),
                        calendar: merge_calendar(
                            cx,
                            &k("calendar"),
                            &format!("{} calendar (board {bname})", self.list_name(Some(id))),
                            sb.map(|x| &x.calendar),
                            &so.calendar,
                            &st.calendar,
                        ),
                        strips: Vec::new(),
                    }
                }
            };
            got.insert(id, sub);
        }
        let members: BTreeSet<SubBoardId> = got.keys().copied().collect();
        let order = merge_order(
            cx,
            Some((
                &format!("{key}:sub_boards:order"),
                &format!("{what} sub-board order"),
            )),
            [&lb, &lo, &lt],
            &members,
            |l| order_text(l.iter().map(|x| self.list_name(Some(*x))).collect()),
        );

        // Strips: each one's existence, its list and its content; then each list's order.
        let pb = b.map(strip_places).unwrap_or_default();
        let mut po = strip_places(o);
        let mut pt = strip_places(t);
        // A sub-board kept against a delete keeps the strips the deleting side moved out
        // or dropped with it.
        for (id, &(list, _, _)) in &pb {
            let Some(sub) = list else { continue };
            let Some(deleter) = revived.get(&sub) else {
                continue;
            };
            let side = match deleter {
                Side::Ours => &mut po,
                Side::Theirs => &mut pt,
            };
            if side.get(id).is_none_or(|(l, _, _)| l.is_none()) {
                side.insert(*id, pb[id]);
            }
        }
        let order_ids = |x: &Board| {
            x.sub_boards
                .iter()
                .flat_map(|s| &s.strips)
                .chain(&x.boneyard)
                .map(|s| s.id)
                .collect::<Vec<_>>()
        };
        let all = union([
            &order_ids(o),
            &order_ids(t),
            &b.map(order_ids).unwrap_or_default(),
        ]);
        let mut placed: HashMap<StripId, (Option<SubBoardId>, Strip)> = HashMap::new();
        // Parts of a sheet deleted on one side (decision 76), kept back in case the merge
        // would leave their sheet without a strip.
        let mut dropped: Vec<(StripId, SheetId, Option<SubBoardId>, Strip)> = Vec::new();
        for id in all {
            let (sb, so, st) = (pb.get(&id), po.get(&id), pt.get(&id));
            let any = so.or(st).or(sb).map(|x| x.2).expect("listed");
            let name = self.strip_name(any);
            let sw = format!("{what}: {name}");
            let sk = format!("{key}:strip:{}", id.0);
            let touched = |x: Option<&(Option<SubBoardId>, usize, &Strip)>| {
                x.map(|(l, _, s)| (*l, *s)) != sb.map(|(l, _, s)| (*l, *s))
            };
            let which = if let StripKind::Sheet(sheet) = any.kind {
                // A sheet strip goes with its sheet.
                if !sheets.contains(&sheet) {
                    continue;
                }
                match (so, st) {
                    (Some(x), Some(y)) => Which::Both(sb.copied(), *x, *y),
                    // A part one side deleted goes, unless the other side changed it.
                    (Some(x), None) | (None, Some(x)) if sb.is_some() => {
                        match exist(cx, &sk, &sw, sb, so, st, touched(so), touched(st)) {
                            Got::One(x, _) => Which::One(*x),
                            _ => {
                                dropped.push((id, sheet, x.0, x.2.clone()));
                                continue;
                            }
                        }
                    }
                    (Some(x), None) => Which::One(*x),
                    (None, Some(y)) => Which::One(*y),
                    (None, None) => continue,
                }
            } else {
                match exist(cx, &sk, &sw, sb, so, st, touched(so), touched(st)) {
                    Got::Gone => continue,
                    Got::One(x, _) => Which::One(*x),
                    Got::Both(b, x, y) => Which::Both(b.copied(), *x, *y),
                }
            };
            let (list, strip) = match which {
                Which::One(x) => (x.0, x.2.clone()),
                Which::Both(b, x, y) => {
                    let list = field(
                        cx,
                        &format!("{sk}:place"),
                        &format!("{sw} place"),
                        b.as_ref().map(|z| &z.0),
                        &x.0,
                        &y.0,
                        |l| self.list_name(*l),
                    );
                    let kind = match (b.map(|z| &z.2.kind), &x.2.kind, &y.2.kind) {
                        (bk, StripKind::Banner(xo), StripKind::Banner(xt)) => {
                            let bb = match bk {
                                Some(StripKind::Banner(z)) => Some(z),
                                _ => None,
                            };
                            StripKind::Banner(Banner {
                                text: field(
                                    cx,
                                    &format!("{sk}:text"),
                                    &format!("{sw} text"),
                                    bb.map(|z| &z.text),
                                    &xo.text,
                                    &xt.text,
                                    |v| quoted(v),
                                ),
                                duration: field(
                                    cx,
                                    &format!("{sk}:minutes"),
                                    &format!("{sw} time"),
                                    bb.map(|z| &z.duration),
                                    &xo.duration,
                                    &xt.duration,
                                    |v| crate::diff::hm(v.0),
                                ),
                                style: field(
                                    cx,
                                    &format!("{sk}:style"),
                                    &format!("{sw} style"),
                                    bb.map(|z| &z.style),
                                    &xo.style,
                                    &xt.style,
                                    |v| v.text(),
                                ),
                            })
                        }
                        (bk, ko, kt) => field(
                            cx,
                            &format!("{sk}:kind"),
                            &format!("{sw} kind"),
                            bk,
                            ko,
                            kt,
                            |k| match k {
                                StripKind::Sheet(_) => "a sheet strip".to_string(),
                                StripKind::Banner(_) => "a banner".to_string(),
                                StripKind::DayBreak => "a day break".to_string(),
                            },
                        ),
                    };
                    (list, Strip { id, kind })
                }
            };
            // A list that is gone: the kept side's list (ours' by default), else the
            // other's, else the Boneyard for a sheet strip; a banner or day break with
            // nowhere to go goes.
            let valid = |l: Option<SubBoardId>| match l {
                // Sheet strips and parked day breaks may sit in the Boneyard; banners not.
                None => !matches!(strip.kind, StripKind::Banner(_)),
                Some(x) => got.contains_key(&x),
            };
            let list = if valid(list) {
                list
            } else {
                let sides = match cx.default {
                    Side::Ours => [so, st],
                    Side::Theirs => [st, so],
                };
                let alt = sides
                    .map(|x| x.map(|x| x.0))
                    .into_iter()
                    .flatten()
                    .find(|l| valid(*l));
                match alt {
                    Some(l) => l,
                    None if matches!(strip.kind, StripKind::Sheet(_)) => None,
                    None => {
                        cx.fallbacks.push(format!(
                            "{what}: {name} left out: the sub-board it was on is gone"
                        ));
                        continue;
                    }
                }
            };
            placed.insert(id, (list, strip));
        }
        // A sheet keeps at least one strip on the board: each side deleting a different
        // part keeps the first.
        for (id, sheet, list, strip) in dropped {
            let has = |p: &HashMap<StripId, (Option<SubBoardId>, Strip)>| {
                p.values()
                    .any(|(_, s)| matches!(s.kind, StripKind::Sheet(x) if x == sheet))
            };
            if !has(&placed) {
                let list = list.filter(|l| got.contains_key(l));
                placed.insert(id, (list, strip));
            }
        }

        let lists = |x: Option<&Board>, l: Option<SubBoardId>| {
            x.map_or(Vec::new(), |x| {
                list_of(x, l).iter().map(|s| s.id).collect::<Vec<_>>()
            })
        };
        let mut sub_boards = Vec::new();
        let take = |cx: &mut Cx, l: Option<SubBoardId>, k: String, w: String| {
            let members: BTreeSet<StripId> = placed
                .iter()
                .filter(|(_, (x, _))| *x == l)
                .map(|(id, _)| *id)
                .collect();
            // The side that kept a sub-board against a delete has its order.
            let (lbx, lox, ltx) = (lists(b, l), lists(Some(o), l), lists(Some(t), l));
            let order = merge_order(cx, Some((&k, &w)), [&lbx, &lox, &ltx], &members, |ids| {
                order_text(ids.iter().map(|i| self.strip_name_by_id(*i)).collect())
            });
            order
                .into_iter()
                .filter_map(|id| placed.get(&id).map(|x| x.1.clone()))
                .collect::<Vec<_>>()
        };
        for id in &order {
            let mut sub = got.remove(id).expect("member");
            sub.strips = take(
                cx,
                Some(*id),
                format!("{key}:strips:{}", id.0),
                format!("{what}, {} strip order", self.list_name(Some(*id))),
            );
            sub_boards.push(sub);
        }
        let boneyard = take(
            cx,
            None,
            format!("{key}:boneyard"),
            format!("{what}, Boneyard order"),
        );
        // What the day breaks show is one value, as the strip layout is.
        let day_look = field(
            cx,
            &format!("{key}:day_look"),
            &format!("{what} day breaks"),
            b.map(|x| &x.day_look),
            &o.day_look,
            &t.day_look,
            |v| v.text(),
        );
        // The named layouts it picked are one value too (decision 78).
        let uses = field(
            cx,
            &format!("{key}:uses"),
            &format!("{what} layouts"),
            b.map(|x| &x.uses),
            &o.uses,
            &t.uses,
            |v| v.text(&self.o.layouts),
        );
        Board {
            id: o.id,
            name,
            strip_layout,
            day_look,
            uses,
            sub_boards,
            boneyard,
        }
    }
}

fn merge_rules(
    cx: &mut Cx<'_>,
    key: &str,
    what: &str,
    b: Option<&DoodRules>,
    o: &DoodRules,
    t: &DoodRules,
) -> DoodRules {
    DoodRules {
        holds: field(
            cx,
            &format!("{key}:holds"),
            &format!("{what} holds"),
            b.map(|x| &x.holds),
            &o.holds,
            &t.holds,
            |v| on_off(*v),
        ),
        drops: field(
            cx,
            &format!("{key}:drops"),
            &format!("{what} drops"),
            b.map(|x| &x.drops),
            &o.drops,
            &t.drops,
            |v| on_off(*v),
        ),
        min_drop_gap: field(
            cx,
            &format!("{key}:min_drop_gap"),
            &format!("{what} minimum drop gap"),
            b.map(|x| &x.min_drop_gap),
            &o.min_drop_gap,
            &t.min_drop_gap,
            |v| crate::diff::days_text(*v),
        ),
    }
}

fn merge_sheet(
    cx: &mut Cx<'_>,
    key: &str,
    what: &str,
    b: Option<&Sheet>,
    o: &Sheet,
    t: &Sheet,
) -> Sheet {
    let text = |cx: &mut Cx, f: &str, l: &str, get: fn(&Sheet) -> &String| {
        field(
            cx,
            &format!("{key}:{f}"),
            &format!("{what} {l}"),
            b.map(get),
            get(o),
            get(t),
            |v| quoted(v),
        )
    };
    let clock = |v: &Option<u32>| {
        v.map_or("none".to_string(), |m| {
            format!("{:02}:{:02}", m / 60, m % 60)
        })
    };
    let k = |f: &str| format!("{key}:{f}");
    let w = |l: &str| format!("{what} {l}");
    Sheet {
        id: o.id,
        number: text(cx, "number", "sheet number", |s| &s.number),
        scenes: text(cx, "scenes", "scenes", |s| &s.scenes),
        int_ext: text(cx, "int_ext", "I/E", |s| &s.int_ext),
        set: text(cx, "set", "set", |s| &s.set),
        time_of_day: text(cx, "time_of_day", "time of day", |s| &s.time_of_day),
        sunrise: merge_field!(cx, k, w, (b, o, t), sunrise, "sunrise", "sunrise", clock),
        sunset: merge_field!(cx, k, w, (b, o, t), sunset, "sunset", "sunset", clock),
        pages: merge_field!(
            cx,
            k,
            w,
            (b, o, t),
            pages,
            "eighths",
            "pages",
            |v: &Eighths| v.to_string()
        ),
        shoot_time: merge_field!(
            cx,
            k,
            w,
            (b, o, t),
            shoot_time,
            "shoot_minutes",
            "shoot time",
            |v: &Option<Minutes>| v.map_or("none".to_string(), |m| crate::diff::hm(m.0))
        ),
        synopsis: text(cx, "synopsis", "synopsis", |s| &s.synopsis),
        location: text(cx, "location", "location", |s| &s.location),
        script_pages: text(cx, "script_pages", "script pages", |s| &s.script_pages),
        script_day: text(cx, "script_day", "script day", |s| &s.script_day),
        screen_time: merge_field!(
            cx,
            k,
            w,
            (b, o, t),
            screen_time,
            "screen_seconds",
            "screen time",
            |v: &Option<Seconds>| {
                v.map_or("none".to_string(), |x| {
                    format!("{}:{:02}", x.0 / 60, x.0 % 60)
                })
            }
        ),
        unit: text(cx, "unit", "unit", |s| &s.unit),
        sequence: text(cx, "sequence", "sequence", |s| &s.sequence),
        episode: text(cx, "episode", "episode", |s| &s.episode),
        comments: text(cx, "comments", "comments", |s| &s.comments),
        elements: merge_ids(cx, b.map(|x| &x.elements[..]), &o.elements, &t.elements),
        quantities: merge_field!(
            cx,
            k,
            w,
            (b, o, t),
            quantities,
            "quantities",
            "quantities",
            |v| crate::diff::quantities_text(v)
        ),
        images: merge_field!(cx, k, w, (b, o, t), images, "images", "images", |v| {
            list_text(v)
        }),
    }
}

/// A list of names in words: `none`, or each quoted, joined by commas.
fn list_text(v: &[String]) -> String {
    if v.is_empty() {
        "none".to_string()
    } else {
        v.iter().map(|x| quoted(x)).collect::<Vec<_>>().join(", ")
    }
}

/// A calendar merged: the weekly days off as a whole, each date's mark on its own,
/// events by value.
fn merge_calendar(
    cx: &mut Cx<'_>,
    key: &str,
    what: &str,
    b: Option<&Calendar>,
    o: &Calendar,
    t: &Calendar,
) -> Calendar {
    let mut out = Calendar {
        weekly_off: field(
            cx,
            &format!("{key}:weekly_off"),
            &format!("{what} weekly days off"),
            b.map(|x| &x.weekly_off),
            &o.weekly_off,
            &t.weekly_off,
            |v| weekdays_text(v),
        ),
        work_days: Vec::new(),
        off_days: Vec::new(),
        holidays: Vec::new(),
        moves: Vec::new(),
        events: merge_events(
            cx,
            b.map_or(&[][..], |x| &x.events[..]),
            &o.events,
            &t.events,
        ),
        prep_start: field(
            cx,
            &format!("{key}:prep_start"),
            &format!("{what} prep start"),
            b.map(|x| &x.prep_start),
            &o.prep_start,
            &t.prep_start,
            |d| d.map_or("none".to_string(), |d| d.to_string()),
        ),
        wrap_end: field(
            cx,
            &format!("{key}:wrap_end"),
            &format!("{what} wrap end"),
            b.map(|x| &x.wrap_end),
            &o.wrap_end,
            &t.wrap_end,
            |d| d.map_or("none".to_string(), |d| d.to_string()),
        ),
    };
    let mut dates: BTreeSet<Date> = marked_dates(o);
    dates.extend(marked_dates(t));
    if let Some(b) = b {
        dates.extend(marked_dates(b));
    }
    for d in dates {
        let mb = b.map(|x| Mark::of(x, d));
        let m = field(
            cx,
            &format!("{key}:{d}"),
            &format!("{what} {d}"),
            mb.as_ref(),
            &Mark::of(o, d),
            &Mark::of(t, d),
            |m| Mark::name(*m).to_string(),
        );
        if let Some(m) = m {
            Mark::list(&mut out, m).push(d);
        }
    }
    out
}

/// Events merged by value: each value as many times as the side that changed its count
/// has it (ours when both did), in merged order.
fn merge_events(cx: &mut Cx, b: &[Event], o: &[Event], t: &[Event]) -> Vec<Event> {
    let mut values: Vec<&Event> = Vec::new();
    for ev in o.iter().chain(t).chain(b) {
        if !values.contains(&ev) {
            values.push(ev);
        }
    }
    let keys = |l: &[Event]| -> Vec<(usize, usize)> {
        let mut seen = vec![0usize; values.len()];
        l.iter()
            .map(|ev| {
                let v = values.iter().position(|x| *x == ev).expect("listed");
                seen[v] += 1;
                (v, seen[v] - 1)
            })
            .collect()
    };
    let (kb, ko, kt) = (keys(b), keys(o), keys(t));
    let count = |k: &[(usize, usize)], v: usize| k.iter().filter(|x| x.0 == v).count();
    let mut members = BTreeSet::new();
    for v in 0..values.len() {
        let (cb, co, ct) = (count(&kb, v), count(&ko, v), count(&kt, v));
        let n = pick3(Some(&cb), Some(&co), Some(&ct))
            .flatten()
            .copied()
            .unwrap_or(co);
        for i in 0..n {
            members.insert((v, i));
        }
    }
    merge_order(cx, None, [&kb, &ko, &kt], &members, |_| String::new())
        .into_iter()
        .map(|(v, _)| values[v].clone())
        .collect()
}

/// The properties `side` renamed from `base`, as (base name, new name): the lists the
/// same length and a name in a place that neither list has elsewhere.
fn renames(base: &[String], side: &[String]) -> Vec<(String, String)> {
    if base.len() != side.len() {
        return Vec::new();
    }
    base.iter()
        .zip(side)
        .filter(|(b, x)| {
            !same_name(b, x)
                && !side.iter().any(|s| same_name(s, b))
                && !base.iter().any(|s| same_name(s, x))
        })
        .map(|(b, x)| (b.clone(), x.clone()))
        .collect()
}

fn find_rename<'a>(r: &'a [(String, String)], base: &str) -> Option<&'a String> {
    r.iter().find(|(b, _)| b == base).map(|(_, n)| n)
}

// ---------------------------------------------------------------------------
// Making the result whole

/// Drops what can't stand after a merge and needs no question: a quantity for an
/// element no longer on its sheet (or repeated), a property value whose property the
/// element's category no longer has, a link to the element itself.
fn tidy(s: &mut Schedule) {
    for h in &mut s.sheets {
        let mut seen = HashSet::new();
        let on = h.elements.clone();
        h.quantities
            .retain(|q| on.contains(&q.element) && seen.insert(q.element));
    }
    let props: HashMap<CategoryId, Vec<String>> = s
        .categories
        .iter()
        .map(|c| (c.id, c.properties.clone()))
        .collect();
    for e in &mut s.elements {
        let mine = props.get(&e.category).cloned().unwrap_or_default();
        let mut seen = HashSet::new();
        e.properties
            .retain(|v| mine.contains(&v.name) && seen.insert(v.name.clone()));
        e.properties
            .sort_by_key(|v| mine.iter().position(|p| *p == v.name));
        let me = e.id;
        e.links.retain(|l| *l != me);
    }
}

/// Puts right what the merge left inconsistent, falling back to ours for the part at
/// fault, and says what it did. Ours whole when nothing else works.
fn repair(s: &mut Schedule, m: &M, out: &mut Vec<String>) {
    for _ in 0..4 {
        s.tidy_report_filters();
        if s.check().is_ok() {
            return;
        }
        repair_pass(s, m, out);
        tidy(s);
    }
    if let Err(e) = s.check() {
        out.push(format!(
            "The merged schedule didn't hang together ({e}), so the merge kept {} whole",
            m.whose().trim_end_matches('\'')
        ));
        let next = s.next_id;
        *s = m.kept().clone();
        s.next_id = s.next_id.max(next);
    }
}

fn repair_pass(s: &mut Schedule, m: &M, out: &mut Vec<String>) {
    // Fallbacks keep the side conflicts are settled for: ours unless the merge settles
    // for theirs (decision 174). Comments say ours for that side.
    //
    // Two categories with one name: ours' name back for the later one, else for the
    // first (decision 174), else the later one joins the first.
    let mut i = 0;
    while i < s.categories.len() {
        let c = s.categories[i].clone();
        let Some(j) = s.categories[..i]
            .iter()
            .position(|x| same_name(&x.name, &c.name))
        else {
            i += 1;
            continue;
        };
        let first = s.categories[j].id;
        let back = |k: usize| {
            let x = &s.categories[k];
            m.kept()
                .category(x.id)
                .filter(|oc| oc.name != x.name)
                .map(|oc| (k, oc.name.clone()))
        };
        match back(i).or_else(|| back(j)) {
            Some((k, name)) => {
                out.push(format!(
                    "Category {} kept {} name {}: the merged name was taken",
                    quoted(&s.categories[k].name),
                    m.whose(),
                    quoted(&name)
                ));
                s.categories[k].name = name;
                // The first's name back may meet another; look again from it.
                i = if k == i { i + 1 } else { j };
            }
            None => {
                out.push(format!(
                    "Category {} joined the category of the same name",
                    quoted(&c.name)
                ));
                for e in &mut s.elements {
                    if e.category == c.id {
                        e.category = first;
                    }
                }
                s.categories.remove(i);
            }
        }
    }
    // Two red flag types with one name: ours' name back for the later one, else for the
    // first, else the later one joins the first.
    let mut i = 0;
    while i < s.red_flag_types.len() {
        let c = s.red_flag_types[i].clone();
        let Some(j) = s.red_flag_types[..i]
            .iter()
            .position(|x| same_name(&x.name, &c.name))
        else {
            i += 1;
            continue;
        };
        let first = s.red_flag_types[j].id;
        let back = |k: usize| {
            let x = &s.red_flag_types[k];
            m.kept()
                .red_flag_type(x.id)
                .filter(|oc| oc.name != x.name)
                .map(|oc| (k, oc.name.clone()))
        };
        match back(i).or_else(|| back(j)) {
            Some((k, name)) => {
                out.push(format!(
                    "Red flag type {} kept {} name {}: the merged name was taken",
                    quoted(&s.red_flag_types[k].name),
                    m.whose(),
                    quoted(&name)
                ));
                s.red_flag_types[k].name = name;
                i = if k == i { i + 1 } else { j };
            }
            None => {
                out.push(format!(
                    "Red flag type {} joined the type of the same name",
                    quoted(&c.name)
                ));
                for f in s.elements.iter_mut().flat_map(|e| &mut e.red_flags) {
                    if f.kind == c.id {
                        f.kind = first;
                    }
                }
                s.red_flag_types.remove(i);
            }
        }
    }
    // Red flags whose type is gone keep it; a red flag on two elements stays where ours
    // has it.
    let mut kept: Vec<RedFlagTypeId> = Vec::new();
    for f in s.elements.iter().flat_map(|e| &e.red_flags) {
        if s.red_flag_type(f.kind).is_none() && !kept.contains(&f.kind) {
            kept.push(f.kind);
        }
    }
    for id in kept {
        if let Some(t) = [m.kept(), m.other(), m.b]
            .iter()
            .find_map(|x| x.red_flag_type(id))
        {
            out.push(format!(
                "Red flag type {} kept: red flags still use it",
                quoted(&t.name)
            ));
            s.red_flag_types.push(t.clone());
        }
    }
    let mut seen: HashMap<RedFlagId, ElementId> = HashMap::new();
    for e in &s.elements {
        for f in &e.red_flags {
            let ours_there = m.kept().red_flag(f.id).is_some_and(|(on, _)| on == e.id);
            if ours_there || !seen.contains_key(&f.id) {
                seen.insert(f.id, e.id);
            }
        }
    }
    for e in &mut s.elements {
        let before = e.red_flags.len();
        let me = e.id;
        e.red_flags.retain(|f| seen.get(&f.id) == Some(&me));
        if e.red_flags.len() != before {
            out.push(format!(
                "Element {}: a red flag that is on another element left off",
                quoted(&e.name)
            ));
        }
    }
    // Elements in a category that is gone: ours' category, else theirs', else the base's,
    // else left out (decision 174): an element one side moved into a category the other
    // deleted stays where the other side has it, whichever side is ours.
    let cats: HashSet<CategoryId> = s.categories.iter().map(|c| c.id).collect();
    let lost: Vec<ElementId> = s
        .elements
        .iter()
        .filter(|e| !cats.contains(&e.category))
        .map(|e| e.id)
        .collect();
    for id in lost {
        let other = match m.keep {
            Side::Ours => "theirs'",
            Side::Theirs => "ours'",
        };
        let found = [
            (m.whose(), m.kept()),
            (other, m.other()),
            ("the base's", m.b),
        ]
        .into_iter()
        .find_map(|(whose, x)| {
            x.element(id)
                .filter(|xe| cats.contains(&xe.category))
                .map(|xe| (whose, xe.category))
        });
        match found {
            Some((whose, category)) => {
                out.push(format!(
                    "{} kept {whose} category {}: the merged one is gone",
                    m.element_title(id),
                    m.category_name(category)
                ));
                s.element_mut(id).expect("listed").category = category;
            }
            None => {
                out.push(format!(
                    "{} left out: its category {} is gone",
                    m.element_title(id),
                    m.category_name(s.element(id).expect("listed").category)
                ));
                let _ = s.delete_element(id, true);
            }
        }
    }
    // Two elements with one name in a category: ours' name back for the later one, else
    // for the first, else the later one is merged into the first.
    let mut i = 0;
    while i < s.elements.len() {
        let e = s.elements[i].clone();
        let Some(j) = s.elements[..i]
            .iter()
            .position(|x| x.category == e.category && same_name(&x.name, &e.name))
        else {
            i += 1;
            continue;
        };
        let first = s.elements[j].id;
        // One side renamed an element to the name of one the other side added: the same
        // element, whichever side settles (decision 236).
        let renamed = |k: usize| {
            let x = &s.elements[k];
            m.b.element(x.id)
                .is_some_and(|be| !same_name(&be.name, &x.name))
        };
        let added = |k: usize| m.b.element(s.elements[k].id).is_none();
        let matched = if renamed(j) && added(i) {
            Some((j, i))
        } else if renamed(i) && added(j) {
            Some((i, j))
        } else {
            None
        };
        if let Some((old, new)) = matched {
            let (old_id, new_id) = (s.elements[old].id, s.elements[new].id);
            let (adder, renamer) = if m.o.element(new_id).is_some() {
                ("ours", "theirs")
            } else {
                ("theirs", "ours")
            };
            out.push(format!(
                "{} added by {adder} is the element {renamer} renamed to that name: they are one now",
                m.element_title(new_id)
            ));
            if s.merge_elements(old_id, &[new_id]).is_err() {
                i += 1;
            } else {
                i = j;
            }
            continue;
        }
        let back = |k: usize| {
            let x = &s.elements[k];
            m.kept()
                .element(x.id)
                .filter(|oe| oe.name != x.name)
                .map(|oe| (k, oe.name.clone()))
        };
        match back(i).or_else(|| back(j)) {
            Some((k, name)) => {
                out.push(format!(
                    "{} kept {} name {}: the merged name was taken",
                    m.element_title(s.elements[k].id),
                    m.whose(),
                    quoted(&name)
                ));
                s.elements[k].name = name;
                i = if k == i { i + 1 } else { j };
            }
            None => {
                out.push(format!(
                    "{} merged into the element of the same name",
                    m.element_title(e.id)
                ));
                if s.merge_elements(first, &[e.id]).is_err() {
                    i += 1;
                }
            }
        }
    }
    // Two elements with one board ID in a category: ours' board ID back, else none.
    for i in 0..s.elements.len() {
        let e = s.elements[i].clone();
        let Some(bid) = e.board_id.as_deref() else {
            continue;
        };
        let taken = s.elements[..i]
            .iter()
            .any(|x| x.category == e.category && x.board_id.as_deref() == Some(bid));
        if !taken {
            continue;
        }
        let ours = m
            .kept()
            .element(e.id)
            .filter(|oe| oe.board_id != e.board_id);
        let el = &mut s.elements[i];
        match ours {
            Some(oe) => {
                out.push(format!(
                    "{} kept {} board ID {}: {} was taken",
                    m.element_title(e.id),
                    m.whose(),
                    oe.board_id.as_deref().map_or("none".to_string(), quoted),
                    quoted(bid)
                ));
                el.board_id = oe.board_id.clone();
                el.board_id_locked = oe.board_id_locked;
            }
            None => {
                out.push(format!(
                    "{} lost board ID {}: it was taken",
                    m.element_title(e.id),
                    quoted(bid)
                ));
                el.board_id = None;
                el.board_id_locked = false;
            }
        }
    }
    // References to elements that are gone.
    let els: HashSet<ElementId> = s.elements.iter().map(|e| e.id).collect();
    for e in &mut s.elements {
        let before = e.links.len();
        e.links.retain(|l| els.contains(l));
        if e.links.len() != before {
            out.push(format!(
                "Element {}: links to deleted elements left out",
                quoted(&e.name)
            ));
        }
    }
    for sh in &mut s.sheets {
        let mut seen = HashSet::new();
        let gone: Vec<ElementId> = sh
            .elements
            .iter()
            .filter(|e| !els.contains(e))
            .copied()
            .collect();
        sh.elements.retain(|e| els.contains(e) && seen.insert(*e));
        for e in gone {
            out.push(format!(
                "{}: {} left off, it was deleted",
                upper_first(&sheet_name(sh)),
                m.element_name(e)
            ));
        }
    }
    for b in &mut s.boards {
        for sb in &mut b.sub_boards {
            for ev in &mut sb.calendar.events {
                let mut seen = HashSet::new();
                let before = ev.elements.len();
                ev.elements.retain(|e| els.contains(e) && seen.insert(*e));
                if ev.elements.len() != before {
                    out.push(format!(
                        "Event {}: deleted elements left off",
                        quoted(&ev.name)
                    ));
                }
            }
        }
    }
    // Boards: at least one, each with a sub-board, each holding every sheet once.
    if s.boards.is_empty() {
        out.push(format!(
            "Every board was deleted between the two sides: kept {} boards",
            m.whose()
        ));
        s.boards = m.kept().boards.clone();
    }
    let mut j = 0;
    while j < s.boards.len() {
        if !s.boards[j].sub_boards.is_empty() {
            j += 1;
            continue;
        }
        let id = s.boards[j].id;
        match m.kept().board(id) {
            Some(ob) if !ob.sub_boards.is_empty() => {
                out.push(format!(
                    "Board {}: every sub-board was deleted between the two sides, kept {}",
                    quoted(&s.boards[j].name),
                    m.whose()
                ));
                s.boards[j] = ob.clone();
                j += 1;
            }
            _ => {
                out.push(format!(
                    "Board {} left out: it has no sub-board",
                    quoted(&s.boards[j].name)
                ));
                s.boards.remove(j);
            }
        }
    }
    if s.boards.is_empty() {
        s.boards = m.kept().boards.clone();
    }
    let sheet_ids: Vec<SheetId> = s.sheets.iter().map(|x| x.id).collect();
    let known: HashSet<SheetId> = sheet_ids.iter().copied().collect();
    let mut next = s.next_id;
    for bi in 0..s.boards.len() {
        let bid = s.boards[bi].id;
        let ours_strips: HashSet<StripId> = m
            .kept()
            .board(bid)
            .map(|ob| strip_places(ob).keys().copied().collect())
            .unwrap_or_default();
        let b = &mut s.boards[bi];
        let before = b.boneyard.len();
        b.boneyard
            .retain(|st| !matches!(st.kind, StripKind::Banner(_)));
        if b.boneyard.len() != before {
            out.push(format!(
                "Board {}: banners left out of the Boneyard",
                quoted(&b.name)
            ));
        }
        // Strips of deleted sheets go.
        let keep = |st: &Strip| match st.kind {
            StripKind::Sheet(id) => known.contains(&id),
            _ => true,
        };
        b.boneyard.retain(keep);
        for sb in &mut b.sub_boards {
            sb.strips.retain(keep);
        }
        // A sheet twice: its strips on sub-boards are parts of a scene spanning days
        // (decision 76) and stay; one in the Boneyard as well goes. Twice in the Boneyard,
        // the strip ours has stays.
        let mut count: HashMap<SheetId, Vec<(bool, StripId)>> = HashMap::new();
        for (yard, st) in b
            .sub_boards
            .iter()
            .flat_map(|x| x.strips.iter().map(|s| (false, s)))
            .chain(b.boneyard.iter().map(|s| (true, s)))
        {
            if let StripKind::Sheet(id) = st.kind {
                count.entry(id).or_default().push((yard, st.id));
            }
        }
        let mut drop: HashSet<StripId> = HashSet::new();
        for (sheet, strips) in &count {
            let yard: Vec<StripId> = strips.iter().filter(|x| x.0).map(|x| x.1).collect();
            if strips.len() < 2 || yard.is_empty() {
                continue;
            }
            let kept = if yard.len() < strips.len() {
                drop.extend(yard);
                "kept its place on the schedule, not the Boneyard"
            } else {
                let keep = yard.iter().find(|x| ours_strips.contains(x));
                let which = if keep.is_some() {
                    match m.keep {
                        Side::Ours => "kept ours' strip",
                        Side::Theirs => "kept theirs' strip",
                    }
                } else {
                    "kept the first"
                };
                let keep = keep.unwrap_or(&yard[0]);
                drop.extend(yard.iter().filter(|x| *x != keep));
                which
            };
            out.push(format!(
                "Board {}: {} was on the board twice, {kept}",
                quoted(&b.name),
                m.sheet_text(*sheet)
            ));
        }
        b.boneyard.retain(|st| !drop.contains(&st.id));
        for sb in &mut b.sub_boards {
            sb.strips.retain(|st| !drop.contains(&st.id));
        }
        // A sheet the board lacks goes to the end of its Boneyard.
        for sheet in &sheet_ids {
            if !count.contains_key(sheet) {
                out.push(format!(
                    "Board {}: {} put in the Boneyard, the board had no strip for it",
                    quoted(&b.name),
                    m.sheet_text(*sheet)
                ));
                b.boneyard.push(Strip {
                    id: StripId(next),
                    kind: StripKind::Sheet(*sheet),
                });
                next = next.saturating_add(1);
            }
        }
    }
    s.next_id = next;
    let max = max_id(s);
    if s.next_id <= max {
        s.next_id = max + 1;
    }
}

fn max_id(s: &Schedule) -> u32 {
    kinds(s).keys().copied().max().unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{DayKind, Place, StripColumn, StripField, StripLayout};
    use crate::testgen::Rng;

    // The generators live in `testgen` so the format crate's tests can use them too.
    pub(crate) use crate::testgen::{random_edit, random_schedule};

    fn edited(rng: &mut Rng, base: &Schedule, n: u64) -> Schedule {
        let mut s = base.clone();
        for _ in 0..n {
            random_edit(rng, &mut s);
        }
        s
    }

    fn texts(m: &Merged) -> Vec<String> {
        m.conflicts.iter().map(|c| c.text.clone()).collect()
    }

    /// Main of the diff sample: 1 | 2 | 3 |, ADA, BEN, CARL.
    fn base() -> Schedule {
        crate::diff::tests::sample()
    }

    fn cast(s: &Schedule, name: &str) -> ElementId {
        s.element_named(s.category_named("Cast").unwrap(), name)
            .unwrap()
    }

    #[test]
    fn a_scene_spanning_days_keeps_its_parts_through_a_merge() {
        // Decision 76: a sheet's second strip on a sub-board is a part, not a mistake.
        let b = base();
        let mut o = b.clone();
        let bid = o.boards[0].id;
        let sub = o.boards[0].sub_boards[0].id;
        let sheet = o.sheets[0].id;
        o.add_part(
            bid,
            sheet,
            crate::model::Place {
                sub_board: Some(sub),
                index: 3,
            },
        )
        .unwrap();
        for m in [merge(&b, &o, &b), merge(&b, &b, &o)] {
            assert!(
                m.conflicts.is_empty() && m.fallbacks.is_empty(),
                "{:?}",
                m.fallbacks
            );
            assert_eq!(m.schedule, o);
        }
    }

    #[test]
    fn extensions_keep_both_sides_on_objects_the_base_had() {
        // Decision 227: ours first; an addition on an object only theirs made is dropped,
        // since the merge numbers that object again.
        let b = base();
        let (e0, e1) = (b.elements[0].id.0, b.elements[1].id.0);
        let mut o = b.clone();
        o.extensions.objects.insert(e0, r#"{"a":1}"#.into());
        o.extensions.schedule = Some(r#"{"a":"ours"}"#.into());
        let mut t = b.clone();
        t.extensions.objects.insert(e0, r#"{"a":2}"#.into());
        t.extensions.objects.insert(e1, r#"{"b":1}"#.into());
        t.extensions.schedule = Some(r#"{"a":"theirs"}"#.into());
        t.extensions
            .objects
            .insert(t.next_id + 5, r#"{"new":1}"#.into());
        let m = merge(&b, &o, &t).schedule;
        assert_eq!(m.extensions.of(e0), Some(r#"{"a":1}"#));
        assert_eq!(m.extensions.of(e1), Some(r#"{"b":1}"#));
        assert_eq!(m.extensions.schedule.as_deref(), Some(r#"{"a":"ours"}"#));
        assert_eq!(m.extensions.objects.len(), 2);
    }

    #[test]
    fn a_rename_on_one_side_is_the_element_the_other_side_added() {
        // Decision 236: ours renames ADA to ADDIE; theirs adds ADDIE and tags a scene with
        // it. One element, the renamed one, either way round.
        let b = base();
        let ada = cast(&b, "ADA");
        let mut o = b.clone();
        o.rename_element(ada, "ADDIE").unwrap();
        let mut t = b.clone();
        let addie = t
            .add_element(t.category_named("Cast").unwrap(), "ADDIE")
            .unwrap();
        let sheet = t.sheets[1].id;
        t.tag(sheet, addie).unwrap();
        for (x, y) in [(&o, &t), (&t, &o)] {
            let m = merge(&b, x, y);
            assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
            assert_eq!(m.fallbacks.len(), 1, "{:?}", m.fallbacks);
            assert!(
                m.fallbacks[0].contains("renamed to that name"),
                "{:?}",
                m.fallbacks
            );
            let s = &m.schedule;
            assert_eq!(s.elements.len(), b.elements.len());
            assert_eq!(cast(s, "ADDIE"), ada);
            assert!(s.sheet(sheet).unwrap().elements.contains(&ada));
        }
    }

    #[test]
    fn strip_colors_merge_as_one_value() {
        let b = base();
        let colors = |c: u32| crate::model::StripColors {
            rules: vec![crate::model::StripColorRule {
                int_ext: "EXT".into(),
                time_of_day: String::new(),
                color: crate::model::Rgb(c),
                text: None,
            }],
            ..Default::default()
        };
        let mut o = b.clone();
        o.strip_colors = colors(1);
        let m = merge(&b, &o, &b);
        assert!(m.conflicts.is_empty());
        assert_eq!(m.schedule.strip_colors, colors(1));
        let mut t = b.clone();
        t.strip_colors = colors(2);
        let m = merge(&b, &o, &t);
        assert_eq!(m.conflicts.len(), 1, "{:?}", texts(&m));
        assert_eq!(m.schedule.strip_colors, colors(1), "ours by default");
    }

    #[test]
    fn different_fields_merge_and_the_same_field_conflicts() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        let sh = b.sheets[0].id;
        o.sheet_mut(sh).unwrap().set = "LOG CABIN".into();
        t.sheet_mut(sh).unwrap().synopsis = "Ada arrives.".into();
        let ada = cast(&b, "ADA");
        o.element_mut(ada).unwrap().rules.holds = true;
        t.element_mut(ada).unwrap().rules.drops = true;
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
        assert!(m.fallbacks.is_empty());
        let s = &m.schedule;
        assert_eq!(s.sheet(sh).unwrap().set, "LOG CABIN");
        assert_eq!(s.sheet(sh).unwrap().synopsis, "Ada arrives.");
        let r = s.element(ada).unwrap().rules;
        assert!(r.holds && r.drops);

        t.sheet_mut(sh).unwrap().set = "BARN".into();
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["Scene 1 set: ours \"LOG CABIN\", theirs \"BARN\", was \"CABIN\""]
        );
        assert_eq!(m.conflicts[0].taken, Side::Ours);
        assert_eq!(m.schedule.sheet(sh).unwrap().set, "LOG CABIN");
        // Picked the other way by key.
        let picks = BTreeMap::from([(m.conflicts[0].key.clone(), Side::Theirs)]);
        let m = merge_with(&b, &o, &t, Side::Ours, &picks);
        assert_eq!(m.schedule.sheet(sh).unwrap().set, "BARN");
    }

    #[test]
    fn red_flags_merge_by_id() {
        use crate::model::RedFlagDates;
        let mut b = base();
        let ada = cast(&b, "ADA");
        let unavailable = b.add_red_flag_type("Unavailable").unwrap();
        let day = |s: &str| s.parse::<Date>().unwrap();
        let f = b
            .set_red_flag(
                ada,
                RedFlag {
                    id: RedFlagId(0),
                    kind: unavailable,
                    dates: RedFlagDates::Range {
                        from: day("2027-03-02"),
                        to: day("2027-03-03"),
                    },
                    note: String::new(),
                    hard: false,
                },
            )
            .unwrap();
        let edit = |s: &mut Schedule, change: &dyn Fn(&mut RedFlag)| {
            let mut fl = s.red_flag(f).unwrap().1.clone();
            change(&mut fl);
            s.set_red_flag(ada, fl).unwrap();
        };
        // Different fields of one flag merge; both sides adding a type of one name make one.
        let mut o = b.clone();
        let mut t = b.clone();
        edit(&mut o, &|x| x.note = "Wedding".into());
        edit(&mut t, &|x| x.hard = true);
        let job_o = o.add_red_flag_type("Second job").unwrap();
        t.add_red_flag_type("Second job").unwrap();
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
        assert!(m.fallbacks.is_empty(), "{:?}", m.fallbacks);
        assert_eq!(m.renumbered, 0);
        let got = m.schedule.red_flag(f).unwrap().1;
        assert_eq!((got.note.as_str(), got.hard), ("Wedding", true));
        assert_eq!(m.schedule.red_flag_types.len(), 2);
        assert_eq!(m.schedule.red_flag_types[1].id, job_o);

        // The same field changed both ways is a conflict, settled for ours.
        edit(&mut t, &|x| {
            x.dates = RedFlagDates::NotAfter(day("2027-03-01"))
        });
        edit(&mut o, &|x| {
            x.dates = RedFlagDates::NotBefore(day("2027-03-09"))
        });
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["Element \"ADA\" (Cast) red flag \"Unavailable\" (2027-03-02 to 2027-03-03) dates: ours not before 2027-03-09, theirs not after 2027-03-01, was 2027-03-02 to 2027-03-03"]
        );
        assert_eq!(m.conflicts[0].key, format!("red_flag:{}:dates", f.0));
        assert_eq!(
            m.schedule.red_flag(f).unwrap().1.dates,
            RedFlagDates::NotBefore(day("2027-03-09"))
        );

        // Ours deletes the flag and its type; theirs flags BEN with that type. The type is
        // newly used, so deleting it is a conflict; settled for ours, the type stays anyway
        // because theirs' new flag needs it.
        let mut o = b.clone();
        o.delete_red_flag(f).unwrap();
        o.delete_red_flag_type(unavailable).unwrap();
        let mut t = b.clone();
        let ben = cast(&b, "BEN");
        let nf = t
            .set_red_flag(
                ben,
                RedFlag {
                    id: RedFlagId(0),
                    kind: unavailable,
                    dates: RedFlagDates::NotBefore(day("2027-03-05")),
                    note: String::new(),
                    hard: true,
                },
            )
            .unwrap();
        let m = merge(&b, &o, &t);
        m.schedule.check().unwrap();
        assert_eq!(
            m.conflicts
                .iter()
                .map(|c| c.key.clone())
                .collect::<Vec<_>>(),
            [format!("red_flag_type:{}", unavailable.0)]
        );
        assert!(m.schedule.red_flag(f).is_none());
        assert_eq!(m.schedule.red_flag(nf).unwrap().0, ben);
        assert!(
            m.fallbacks[0].contains("\"Unavailable\" kept"),
            "{:?}",
            m.fallbacks
        );
        let picks = BTreeMap::from([(format!("red_flag_type:{}", unavailable.0), Side::Theirs)]);
        let m = merge_with(&b, &o, &t, Side::Ours, &picks);
        assert!(m.fallbacks.is_empty(), "{:?}", m.fallbacks);
        assert!(m.schedule.red_flag_type(unavailable).is_some());
    }

    #[test]
    fn deleted_against_changed_is_a_conflict() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        let carl = cast(&b, "CARL");
        o.delete_element(carl, false).unwrap();
        t.set_board_id(carl, Some("3")).unwrap();
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["Element \"CARL\" (Cast): ours deleted it, theirs changed it"]
        );
        assert!(m.schedule.element(carl).is_none());
        // Kept: as theirs has it, still tagged on scene 3.
        let m = merge_with(&b, &o, &t, Side::Theirs, &BTreeMap::new());
        assert_eq!(
            m.schedule.element(carl).unwrap().board_id.as_deref(),
            Some("3")
        );
        assert!(m.schedule.sheets[2].elements.contains(&carl));
        // Deleted on one side and untouched on the other just goes.
        let m = merge(&b, &o, &b);
        assert!(m.conflicts.is_empty());
        assert!(m.schedule.element(carl).is_none());
        // A tag on an element the other side deleted: the element is kept by default
        // (ours tagged it), or the tag goes with it.
        let mut o2 = b.clone();
        o2.tag(b.sheets[0].id, carl).unwrap();
        let mut t2 = b.clone();
        t2.delete_element(carl, false).unwrap();
        let m = merge(&b, &o2, &t2);
        assert_eq!(m.conflicts.len(), 1);
        assert!(m.schedule.sheets[0].elements.contains(&carl));
        let m = merge_with(&b, &o2, &t2, Side::Theirs, &BTreeMap::new());
        assert!(m.schedule.element(carl).is_none());
        assert!(
            m.fallbacks.iter().any(|f| f.contains("CARL left off")),
            "{:?}",
            m.fallbacks
        );
        m.schedule.check().unwrap();
    }

    #[test]
    fn merging_sheets_against_an_edit_of_a_merged_away_sheet_is_a_conflict() {
        let b = base();
        let (one, two) = (b.sheets[0].id, b.sheets[1].id);
        let mut o = b.clone();
        o.merge_sheets(one, &[two]).unwrap();
        let mut t = b.clone();
        t.sheet_mut(two).unwrap().synopsis = "Ada swims.".into();
        // The diff lists the merged-away sheet as removed and the survivor as changed.
        let d = crate::diff::diff(&b, &o);
        let lines: Vec<&str> = d.changes.iter().map(|c| c.text.as_str()).collect();
        assert!(
            d.changes
                .iter()
                .any(|c| c.kind == crate::diff::ChangeKind::Removed
                    && c.subject == crate::diff::Subject::Sheet(two)),
            "{lines:?}"
        );
        let m = merge(&b, &o, &t);
        m.schedule.check().unwrap();
        assert_eq!(texts(&m).len(), 1, "{:?}", texts(&m));
        assert!(
            texts(&m)[0].contains("ours deleted it, theirs changed it"),
            "{:?}",
            texts(&m)
        );
        assert_eq!(m.conflicts[0].key, format!("sheet:{}", two.0));
        // Settled for ours: merged, scene 2 gone everywhere.
        assert!(m.schedule.sheet(two).is_none());
        assert_eq!(m.schedule.sheet(one).unwrap().scenes, "1, 2");
        // Settled for theirs: scene 2 comes back with the edit, on every board.
        let picks = BTreeMap::from([(m.conflicts[0].key.clone(), Side::Theirs)]);
        let m = merge_with(&b, &o, &t, Side::Ours, &picks);
        m.schedule.check().unwrap();
        assert_eq!(m.schedule.sheet(two).unwrap().synopsis, "Ada swims.");
        assert_eq!(m.schedule.sheet(one).unwrap().scenes, "1, 2");
        // Theirs untouched: the merge just goes through.
        let m = merge(&b, &o, &b);
        assert!(m.conflicts.is_empty());
        assert_eq!(m.schedule, o);
    }

    #[test]
    fn strip_order_follows_the_side_that_reordered() {
        let b = base();
        let bid = b.boards[0].id;
        let sb = b.boards[0].sub_boards[0].id;
        let main = |s: &Schedule| -> Vec<String> {
            s.boards[0].sub_boards[0]
                .strips
                .iter()
                .map(|st| match &st.kind {
                    StripKind::Sheet(id) => s.sheet(*id).unwrap().scenes.clone(),
                    StripKind::Banner(x) => format!("[{}]", x.text),
                    StripKind::DayBreak => "|".to_string(),
                })
                .collect()
        };
        assert_eq!(main(&b), ["1", "|", "2", "|", "3", "|"]);
        // Ours drags scene 3 to the top; theirs adds a banner after scene 2.
        let mut o = b.clone();
        let s3 = b.boards[0].sub_boards[0].strips[4].id;
        o.move_strips(
            bid,
            &[s3],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let mut t = b.clone();
        t.insert_strip(
            bid,
            StripKind::Banner(Banner {
                text: "Lunch".into(),
                duration: Minutes(60),
                ..Default::default()
            }),
            Place {
                sub_board: Some(sb),
                index: 3,
            },
        )
        .unwrap();
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
        assert_eq!(main(&m.schedule), ["3", "1", "|", "2", "[Lunch]", "|", "|"]);
        // Both reorder differently: a conflict, ours' order.
        let mut t = b.clone();
        let s2 = b.boards[0].sub_boards[0].strips[2].id;
        t.move_strips(
            bid,
            &[s2],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let m = merge(&b, &o, &t);
        assert_eq!(m.conflicts.len(), 1, "{:?}", texts(&m));
        assert!(
            m.conflicts[0]
                .text
                .starts_with("Board \"Board 1\", \"Main\" strip order: ours scene 3"),
            "{}",
            m.conflicts[0].text
        );
        assert_eq!(main(&m.schedule), main(&o));
        // A strip moved to different lists on both sides.
        let mut o = b.clone();
        o.move_strips(
            bid,
            &[s2],
            Place {
                sub_board: None,
                index: 0,
            },
        )
        .unwrap();
        let mut t = b.clone();
        let unit = t.add_sub_board(bid, "Second unit", None).unwrap();
        t.move_strips(
            bid,
            &[s2],
            Place {
                sub_board: Some(unit),
                index: 0,
            },
        )
        .unwrap();
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["Board \"Board 1\": scene 2 place: ours the Boneyard, theirs \"Second unit\", was \"Main\""]
        );
        m.schedule.check().unwrap();
    }

    #[test]
    fn things_added_on_both_sides() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        // The same new prop on both sides, with one name: one element. Sheets added on
        // both sides reuse ids: theirs gets new ones.
        let po = o.add_category("Props");
        let lo = o.add_element(po, "Lantern").unwrap();
        o.add_sheet(Sheet {
            scenes: "4".into(),
            ..Sheet::default()
        });
        let pt = t.add_category("props");
        let lt = t.add_element(pt, "LANTERN").unwrap();
        t.add_sheet(Sheet {
            scenes: "5".into(),
            ..Sheet::default()
        });
        let sh5 = t.sheets.last().unwrap().id;
        t.tag(sh5, lt).unwrap();
        let m = merge(&b, &o, &t);
        let s = &m.schedule;
        s.check().unwrap();
        assert_eq!(s.categories.len(), 2);
        assert_eq!(s.elements.iter().filter(|e| e.category == po).count(), 1);
        assert_eq!(s.sheets.len(), 5);
        assert!(m.renumbered > 0);
        let five = s.sheets.iter().find(|x| x.scenes == "5").unwrap();
        assert_eq!(five.elements, [lo]);
        // The name differs in case only: a conflict on the name.
        assert!(
            texts(&m)
                .iter()
                .any(|x| x.contains("name: ours \"Props\", theirs \"props\"")),
            "{:?}",
            texts(&m)
        );
    }

    #[test]
    fn calendars_merge_date_by_date() {
        let b = base();
        let (bid, sb) = (b.boards[0].id, b.boards[0].sub_boards[0].id);
        let mut o = b.clone();
        let mut t = b.clone();
        let d = |s: &str| s.parse::<Date>().unwrap();
        o.calendar_mut(bid, sb)
            .unwrap()
            .set_day_kind(d("2027-03-03"), DayKind::Holiday);
        t.calendar_mut(bid, sb)
            .unwrap()
            .set_day_kind(d("2027-03-04"), DayKind::Move);
        t.boards[0].sub_boards[0].start = Some(d("2027-03-02"));
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty());
        let c = &m.schedule.boards[0].sub_boards[0].calendar;
        assert_eq!(c.holidays, [d("2027-03-03")]);
        assert_eq!(c.moves, [d("2027-03-04")]);
        assert_eq!(
            m.schedule.boards[0].sub_boards[0].start,
            Some(d("2027-03-02"))
        );
        t.calendar_mut(bid, sb)
            .unwrap()
            .set_day_kind(d("2027-03-03"), DayKind::Off);
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["\"Main\" calendar (board \"Board 1\") 2027-03-03: ours holiday, theirs day off, was as the working week has it"]
        );
    }

    #[test]
    fn strip_layouts_merge_as_one_value() {
        let b = base();
        let id = b.boards[0].id;
        let narrow = |fields: &[StripField]| {
            StripLayout::of(fields.iter().map(|f| StripColumn::of(*f)).collect())
        };
        // Only theirs changed it: theirs is taken.
        let o = b.clone();
        let mut t = b.clone();
        t.set_strip_layout(id, narrow(&[StripField::Scene, StripField::Set]))
            .unwrap();
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty());
        assert_eq!(m.schedule.boards[0].strip_layout, t.boards[0].strip_layout);
        // Both changed it differently: a conflict, ours by default.
        let mut o = b.clone();
        o.set_strip_layout(id, narrow(&[StripField::Synopsis]))
            .unwrap();
        let m = merge(&b, &o, &t);
        assert_eq!(
            texts(&m),
            ["Board \"Board 1\" strip layout: ours Synopsis, theirs Scene, Set, was Scene, I/E, Set, Time of day, Pages, Cast IDs"]
        );
        assert_eq!(m.schedule.boards[0].strip_layout, o.boards[0].strip_layout);
        m.schedule.check().unwrap();
    }

    #[test]
    fn production_reports_codes_and_prep_merge() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        o.production.company = "ZTEST Pictures".into();
        t.production.director = "A. Director".into();
        o.production.episode = "104".into();
        t.production.timing = "52:00".into();
        t.duplicate_report(0).unwrap();
        o.dood_codes.hold = "HD".into();
        let sub = t.boards[0].sub_boards[0].id;
        t.boards[0].sub_boards[0].calendar.prep_start = "2027-01-04".parse().ok();
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
        let s = &m.schedule;
        assert_eq!(s.production.company, "ZTEST Pictures");
        assert_eq!(s.production.director, "A. Director");
        assert_eq!(s.production.episode, "104");
        assert_eq!(s.production.timing, "52:00");
        assert_eq!(s.report_list()[1].name, "One-line schedule copy");
        assert_eq!(s.dood_codes.hold, "HD");
        assert_eq!(
            s.boards[0]
                .sub_boards
                .iter()
                .find(|x| x.id == sub)
                .unwrap()
                .calendar
                .prep_start,
            "2027-01-04".parse().ok()
        );
        // Both changed the report list differently: a conflict, ours by default.
        let mut o2 = b.clone();
        o2.move_report(0, 1).unwrap();
        let m = merge(&b, &o2, &t);
        assert_eq!(m.conflicts.len(), 1);
        assert_eq!(m.conflicts[0].key, "reports");
        assert_eq!(m.schedule.reports, o2.reports);
        // A filter on a category theirs added follows its new id; one ours deleted goes.
        let mut t3 = b.clone();
        let props = t3.add_category("ZTEST Props");
        let mut r = t3.report_list()[0].clone();
        r.options.categories = vec![props];
        t3.set_report(0, r).unwrap();
        let m = merge(&b, &b, &t3);
        let kept = m.schedule.report_list()[0].options.categories.clone();
        assert_eq!(kept.len(), 1);
        assert_eq!(m.schedule.category(kept[0]).unwrap().name, "ZTEST Props");
        assert!(m.schedule.check().is_ok());
    }

    #[test]
    fn names_that_collide_fall_back_to_ours() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        let (ben, carl) = (cast(&b, "BEN"), cast(&b, "CARL"));
        o.rename_element(ben, "BENNY").unwrap();
        t.rename_element(carl, "BENNY").unwrap();
        let m = merge(&b, &o, &t);
        m.schedule.check().unwrap();
        assert_eq!(m.schedule.element(carl).unwrap().name, "CARL");
        assert_eq!(m.fallbacks.len(), 1, "{:?}", m.fallbacks);
        assert!(
            m.fallbacks[0].contains("kept ours' name \"CARL\""),
            "{:?}",
            m.fallbacks
        );
    }

    /// Decision 174: a merge settled for theirs falls back to theirs, so it is the merge
    /// the other way round settled for ours. Fallbacks used to keep ours whatever the
    /// merge settled for.
    #[test]
    fn a_merge_settled_for_theirs_falls_back_to_theirs() {
        let b = base();
        let mut o = b.clone();
        let mut t = b.clone();
        let (ben, carl) = (cast(&b, "BEN"), cast(&b, "CARL"));
        o.rename_element(ben, "BENNY").unwrap();
        t.rename_element(carl, "BENNY").unwrap();
        let m = merge_with(&b, &o, &t, Side::Theirs, &BTreeMap::new());
        m.schedule.check().unwrap();
        assert_eq!(m.schedule.element(carl).unwrap().name, "BENNY");
        assert_eq!(m.schedule.element(ben).unwrap().name, "BEN");
        assert!(
            m.fallbacks[0].contains("kept theirs' name \"BEN\""),
            "{:?}",
            m.fallbacks
        );
        let other_way = merge(&b, &t, &o);
        assert!(other_way.schedule == m.schedule);
    }

    /// Decision 174, found by the symmetry property: ours moves an element into a category
    /// theirs deletes, and the deletion is taken. The element used to be left out, with its
    /// tags, though theirs and the base still had it; it keeps theirs' category now, as it
    /// keeps ours' the other way round.
    #[test]
    fn an_element_moved_into_a_deleted_category_keeps_the_other_sides() {
        let mut b = base();
        let props = b.add_category("Props");
        let carl = cast(&b, "CARL");
        let mut mover = b.clone();
        mover.move_element(carl, props).unwrap();
        let mut deleter = b.clone();
        deleter.delete_category(props).unwrap();
        let tagged = b
            .sheets
            .iter()
            .filter(|s| s.elements.contains(&carl))
            .count();
        assert!(tagged > 0);
        for m in [
            merge_with(&b, &mover, &deleter, Side::Theirs, &BTreeMap::new()),
            merge_with(&b, &deleter, &mover, Side::Ours, &BTreeMap::new()),
        ] {
            m.schedule.check().unwrap();
            assert!(m.schedule.category(props).is_none());
            let e = m.schedule.element(carl).expect("CARL left out");
            assert_eq!(e.category, b.cast().unwrap());
            let still = m
                .schedule
                .sheets
                .iter()
                .filter(|s| s.elements.contains(&carl))
                .count();
            assert_eq!(still, tagged, "CARL's tags lost");
        }
    }

    #[test]
    fn property_merge_laws() {
        let mut rng = crate::testgen::fuzz_rng(4040);
        for case in 0..crate::testgen::rounds(300) {
            let base = random_schedule(&mut rng);
            let n = 1 + rng.below(5);
            let x = edited(&mut rng, &base, n);
            let n = 1 + rng.below(5);
            let y = edited(&mut rng, &base, n);
            for (got, why) in [
                (merge(&base, &x, &base), "merge(base, x, base)"),
                (merge(&base, &base, &x), "merge(base, base, x)"),
                (merge(&base, &x, &x), "merge(base, x, x)"),
            ] {
                assert!(
                    got.conflicts.is_empty(),
                    "case {case} {why}: {:?}",
                    texts(&got)
                );
                assert!(
                    got.fallbacks.is_empty(),
                    "case {case} {why}: {:?}",
                    got.fallbacks
                );
                assert!(
                    got.schedule == x,
                    "case {case} {why}:\n{}",
                    crate::diff::diff(&x, &got.schedule).to_text()
                );
            }
            for side in [Side::Ours, Side::Theirs] {
                let m = merge_with(&base, &x, &y, side, &BTreeMap::new());
                m.schedule
                    .check()
                    .unwrap_or_else(|e| panic!("case {case}: {e}"));
                assert!(m.conflicts.iter().all(|c| c.taken == side));
                assert!(m.conflicts.iter().all(|c| !c.text.contains('\u{2014}')));
                // Keys are stable: the same merge gives the same conflicts.
                let again = merge_with(&base, &x, &y, side, &BTreeMap::new());
                assert_eq!(again.conflicts, m.conflicts);
                assert!(again.schedule == m.schedule);
            }
        }
    }

    /// The two orders of a merge, each settling conflicts for the same side, give the same
    /// schedule, the same conflict keys and as many fallbacks (decision 174).
    ///
    /// Up to what a side can't decide: things both sides put at one place (strips,
    /// events, tags, sheets) go the kept side's first, so only their order may differ
    /// ([`in_order`]); a conflict's `what` and values name things as ours does; and when
    /// both sides add things, theirs' may be renumbered or matched to ours' by name, so
    /// the same things may have other ids. Then only the counts are compared.
    #[test]
    fn property_merge_is_symmetric() {
        let mut rng = crate::testgen::fuzz_rng(5050);
        let mut compared = 0;
        let rounds = crate::testgen::rounds(300);
        let count = |s: &Schedule| {
            let mut n: BTreeMap<String, usize> = BTreeMap::new();
            for k in kinds(s).values() {
                *n.entry(format!("{k:?}")).or_default() += 1;
            }
            n
        };
        for case in 0..rounds {
            let base = random_schedule(&mut rng);
            let n = 1 + rng.below(5);
            let x = edited(&mut rng, &base, n);
            let n = 1 + rng.below(5);
            let y = edited(&mut rng, &base, n);
            let xy = merge_with(&base, &x, &y, Side::Ours, &BTreeMap::new());
            let yx = merge_with(&base, &y, &x, Side::Theirs, &BTreeMap::new());
            let edits = || {
                format!(
                    "case {case}\nx:\n{}\ny:\n{}",
                    crate::diff::diff(&base, &x).to_text(),
                    crate::diff::diff(&base, &y).to_text()
                )
            };
            for m in [&xy, &yx] {
                m.schedule
                    .check()
                    .unwrap_or_else(|e| panic!("{e}\n{}", edits()));
                assert!(crate::diff::diff(&m.schedule, &m.schedule).is_empty());
            }
            assert_eq!(count(&xy.schedule), count(&yx.schedule), "{}", edits());
            assert_eq!(xy.fallbacks.len(), yx.fallbacks.len(), "{}", edits());
            if xy.renumbered + yx.renumbered > 0 || kinds(&xy.schedule) != kinds(&yx.schedule) {
                continue;
            }
            compared += 1;
            // Conflicts are listed in ours' order of things and show values with ours'
            // names, so their keys are compared as sets; the schedules compare the values.
            let keys = |m: &Merged| m.conflicts.iter().map(|c| c.key.clone()).collect();
            let (k1, k2): (BTreeSet<String>, BTreeSet<String>) = (keys(&xy), keys(&yx));
            assert_eq!(k1, k2, "{}", edits());
            let a = in_order(&base, [&x, &y], &xy.schedule);
            let b = in_order(&base, [&x, &y], &yx.schedule);
            assert!(
                a == b,
                "{}\none way against the other:\n{}",
                edits(),
                crate::diff::diff(&a, &b).to_text()
            );
        }
        assert!(compared * 4 >= rounds * 3, "only {compared} cases compared");
    }

    /// `s` with what both sides may put at one place in a fixed order: the lists of
    /// categories, red flag types, elements, sheets, boards and sub-boards, and an
    /// element's red flags and links, and a sheet's elements, by id; events by their
    /// debug text; and in each strip list the strips that are settled (the base and both
    /// sides have them there, and neither side moved them) in their order, then the rest
    /// by id. The laws test holds the order itself where only one side changes.
    fn in_order(base: &Schedule, sides: [&Schedule; 2], s: &Schedule) -> Schedule {
        let ids = |strips: &[Strip]| strips.iter().map(|x| x.id.0).collect::<Vec<u32>>();
        let list = |s: &Schedule, board: BoardId, sub: Option<SubBoardId>| -> Vec<u32> {
            let Some(b) = s.board(board) else {
                return Vec::new();
            };
            match sub {
                None => ids(&b.boneyard),
                Some(sub) => b
                    .sub_boards
                    .iter()
                    .find(|x| x.id == sub)
                    .map_or(Vec::new(), |x| ids(&x.strips)),
            }
        };
        let mut settled: HashSet<u32> = HashSet::new();
        for b in &base.boards {
            let places = b.sub_boards.iter().map(|x| Some(x.id)).chain([None]);
            for sub in places {
                let was = list(base, b.id, sub);
                let mut keep: HashSet<u32> = was.iter().copied().collect();
                for side in sides {
                    let now = list(side, b.id, sub);
                    let moved = crate::diff::moved_in(&was, &now);
                    keep.retain(|id| now.contains(id) && !moved.contains(id));
                }
                settled.extend(keep);
            }
        }
        // A stable sort: the settled strips keep their order.
        let sort = |strips: &mut Vec<Strip>| {
            strips.sort_by_key(|x| if settled.contains(&x.id.0) { 0 } else { x.id.0 });
        };
        let mut s = s.clone();
        s.sheets.sort_by_key(|x| x.id);
        for sheet in &mut s.sheets {
            sheet.elements.sort();
        }
        s.categories.sort_by_key(|x| x.id);
        s.red_flag_types.sort_by_key(|x| x.id);
        s.boards.sort_by_key(|x| x.id);
        s.elements.sort_by_key(|x| x.id);
        for e in &mut s.elements {
            e.red_flags.sort_by_key(|f| f.id);
            e.links.sort();
        }
        for b in &mut s.boards {
            b.sub_boards.sort_by_key(|x| x.id);
            for sb in &mut b.sub_boards {
                sb.calendar.events.sort_by_key(|e| format!("{e:?}"));
                sort(&mut sb.strips);
            }
            sort(&mut b.boneyard);
        }
        s
    }

    // Review findings, each once a failing test.

    fn one_scene() -> Schedule {
        let mut s = Schedule::new("T");
        s.add_sheet(Sheet {
            scenes: "1".into(),
            ..Sheet::default()
        });
        s
    }

    #[test]
    fn a_layout_id_and_their_element_id_do_not_collide() {
        let b = one_scene();
        let mut o = b.clone();
        o.add_layout(
            "Wide",
            crate::layouts::LayoutBody::DayBreaks(crate::model::DayLook::default()),
        )
        .unwrap();
        let mut t = b.clone();
        let cast = t.cast().unwrap();
        t.add_element(cast, "ANNA").unwrap();
        let m = merge(&b, &o, &t);
        assert!(m.schedule.check().is_ok(), "{:?}", m.schedule.check());
        assert_eq!(m.schedule.layouts.layouts.len(), 1, "ours' layout lost");
        assert_eq!(m.schedule.elements.len(), 1, "theirs' element lost");
        assert_eq!(m.renumbered, 1);
    }

    #[test]
    fn a_quantity_follows_its_renumbered_element() {
        let b = one_scene();
        let sheet = b.sheets[0].id;
        let cast = b.cast().unwrap();
        let mut o = b.clone();
        let x = o.add_element(cast, "HORSE").unwrap();
        o.tag(sheet, x).unwrap();
        let mut t = b.clone();
        let y = t.add_element(cast, "COW").unwrap();
        assert_eq!(x, y);
        t.tag(sheet, y).unwrap();
        t.set_quantity(sheet, y, 4, true).unwrap();
        let m = merge(&b, &o, &t);
        let s = &m.schedule;
        let horse = s.element_named(cast, "HORSE").unwrap();
        let cow = s.element_named(cast, "COW").unwrap();
        let h = s.sheet(sheet).unwrap();
        assert_eq!(h.quantity(cow), (4, true), "COW's quantity");
        assert_eq!(h.quantity(horse), (1, true), "HORSE got COW's quantity");
    }

    #[test]
    fn deleting_a_part_survives_a_merge() {
        let mut b = one_scene();
        let board = b.boards[0].id;
        let sub = b.boards[0].sub_boards[0].id;
        let sheet = b.sheets[0].id;
        b.move_strips(
            board,
            &b.sheet_strips(board, sheet),
            Place {
                sub_board: Some(sub),
                index: 0,
            },
        )
        .unwrap();
        let p2 = b
            .add_part(
                board,
                sheet,
                Place {
                    sub_board: Some(sub),
                    index: 1,
                },
            )
            .unwrap();
        let o = b.clone();
        let mut t = b.clone();
        t.delete_strip(board, p2).unwrap();
        assert_eq!(t.sheet_strips(board, sheet).len(), 1);
        let m = merge(&b, &o, &t);
        assert_eq!(
            m.schedule.sheet_strips(board, sheet).len(),
            1,
            "theirs' join of the parts was undone"
        );
        // Both sides deleting the other part still leaves the sheet one strip.
        let mut o = b.clone();
        let first = b.sheet_strips(board, sheet)[0];
        o.delete_strip(board, first).unwrap();
        let m = merge(&b, &o, &t);
        assert_eq!(m.schedule.sheet_strips(board, sheet).len(), 1);
        assert!(m.schedule.check().is_ok());
    }

    #[test]
    fn a_property_value_survives_a_rename_on_the_other_side() {
        let mut b = one_scene();
        let cast = b.cast().unwrap();
        let ann = b.add_element(cast, "ANN").unwrap();
        b.add_property(cast, "Size").unwrap();
        let mut o = b.clone();
        o.rename_property(cast, 0, "Dimensions").unwrap();
        let mut t = b.clone();
        t.set_property_value(ann, "Size", "L").unwrap();
        let m = merge(&b, &o, &t);
        let props = |m: &Merged| -> Vec<(String, String)> {
            m.schedule
                .element(ann)
                .unwrap()
                .properties
                .iter()
                .map(|v| (v.name.clone(), v.value.clone()))
                .collect()
        };
        let want = vec![("Dimensions".to_string(), "L".to_string())];
        assert_eq!(props(&m), want, "theirs' value dropped without a word");
        // And the other way round.
        let m = merge(&b, &t, &o);
        assert_eq!(props(&m), want);
    }

    /// Review: the plan merged as one value, so a call time on one side and a map
    /// location on the other conflicted. Decision 91: list by list, entry by entry.
    #[test]
    fn the_plan_merges_entry_by_entry() {
        use crate::plan::{set_call_time, GeoLocation};
        let mut b = one_scene();
        let board = b.boards[0].id;
        let sub = b.boards[0].sub_boards[0].id;
        let at = |index| Place {
            sub_board: Some(sub),
            index,
        };
        let d1 = b.insert_strip(board, StripKind::DayBreak, at(0)).unwrap();
        let d2 = b.insert_strip(board, StripKind::DayBreak, at(1)).unwrap();
        set_call_time(&mut b, d1, Some(360)).unwrap();
        let pier = |lat| GeoLocation {
            name: "Pier".into(),
            latitude: lat,
            longitude: 0,
            utc_offset: 0,
            daylight_saving: None,
        };
        // Ours: a location and day 1's call time; theirs: day 2's call time and a pin.
        let mut o = b.clone();
        o.plan.set_location(pier(1_000_000)).unwrap();
        set_call_time(&mut o, d1, Some(420)).unwrap();
        let mut t = b.clone();
        set_call_time(&mut t, d2, Some(480)).unwrap();
        let strip = t.sheet_strips(board, t.sheets[0].id)[0];
        crate::plan::set_pinned(&mut t, &[strip], true).unwrap();
        let m = merge(&b, &o, &t);
        assert!(m.conflicts.is_empty(), "{:?}", texts(&m));
        let p = &m.schedule.plan;
        assert_eq!(p.call_time(d1), Some(420));
        assert_eq!(p.call_time(d2), Some(480));
        assert!(p.location("pier").is_some() && p.is_pinned(strip));
        // Both change day 1's call time: one conflict, naming the entry.
        let mut t2 = t.clone();
        set_call_time(&mut t2, d1, Some(300)).unwrap();
        t2.plan.set_location(pier(2_000_000)).unwrap();
        let m = merge(&b, &o, &t2);
        let keys: Vec<&str> = m.conflicts.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "plan:location:pier".to_string(),
                format!("plan:call_time:{}", d1.0)
            ]
        );
        assert!(
            m.conflicts[1].text.contains("was 06:00"),
            "{}",
            m.conflicts[1].text
        );
        assert_eq!(m.schedule.plan.call_time(d1), Some(420), "ours by default");
        assert_eq!(m.schedule.plan.call_time(d2), Some(480));
    }
}
