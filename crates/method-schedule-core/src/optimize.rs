//! A suggested strip order for one sub-board (decisions.md, BYD-06). Never automatic:
//! [`suggest`] works one out with before and after counts, and [`apply`] puts it on the
//! board when asked (one undo step in the app).
//!
//! The heuristic is meant to be explained in a sentence, not to be optimal:
//!
//! 1. Only the scheduled scene strips move (those above the last day break). Day breaks,
//!    banners, pinned strips ([`crate::plan::Plan::pinned`]) and every part of a scene
//!    spanning days (decision 76: moving them could put two parts on one day) keep their
//!    places, so
//!    every day keeps its number of strips and the shoot days stay the same; the scenes
//!    fill the other places in the new order.
//! 2. Scenes are grouped by location (the sheet's location, else its set), and within a
//!    location by set. Groups come in the order their first scene is found on the board,
//!    except that after each group the next is the one sharing the most cast with it.
//!    Within a set, each next scene is the one sharing the most cast with the scene
//!    before it (ties keep board order).
//! 3. Red flags: when the sub-board has dates, a place on a date some scene's element is
//!    flagged on takes the next scene in the order that has no flag that date, if any.
//!
//! The counts compared: shoot days, cast work days (each cast member's days added up),
//! location changes within a day, switches between day and night scenes within a day,
//! and red flag clashes.
//!
//! **Cost** (decision 234): those counts weighed into one number ([`Score::cost`]), so
//! orders can be compared. Three orders are tried: by location then set (above), by
//! location then day or night then set, and by shared cast alone; the cheapest is
//! suggested, the first on a tie. When none costs less than the order on the board, the
//! board's order is the suggestion and nothing would move.
//!
//! **Weights and search** (decision 246): the weights are the schedule's own
//! ([`crate::plan::OrderWeights`]), and the cost counts each hour a day runs past the long
//! day limit. From the cheapest order so far, pairs of scenes are swapped while a swap
//! lowers the cost, always in the same order, so the same board gives the same suggestion.
//! The swaps are judged by a quick estimate; the result is scored like any other order
//! and kept only when it really costs less.

use std::collections::{BTreeSet, HashSet};

use crate::days::{days, days_with};
use crate::model::{
    BoardId, CategoryId, ElementId, ElementIndex, Error, Schedule, SheetId, SheetIndex, Strip,
    StripId, StripKind, SubBoardId,
};
use crate::plan::OrderWeights;
use crate::redflags::red_flags;
use crate::timeline::timelines;
use crate::Date;

/// What a schedule's order costs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub shoot_days: u32,
    /// Each cast member's shoot days, added up.
    pub cast_days: u32,
    /// Places in a day where the next scene is at another location.
    pub location_changes: u32,
    /// Places in a day where the next scene is day after night or night after day.
    pub day_night_switches: u32,
    pub red_flag_clashes: u32,
    /// Each day's hours past the long day limit, a part of an hour counting whole.
    pub long_day_hours: u32,
}

impl Score {
    /// The counts weighed into one number; lower is better. Shoot days don't count: a
    /// suggestion keeps every day.
    pub fn cost(&self, w: &OrderWeights) -> u64 {
        let part = |n: u32, by: u32| u64::from(n) * u64::from(by);
        part(self.cast_days, w.cast_day)
            + part(self.location_changes, w.location_change)
            + part(self.day_night_switches, w.day_night_switch)
            + part(self.red_flag_clashes, w.red_flag_clash)
            + part(self.long_day_hours, w.long_day_hour)
    }
}

/// The orders tried, by name, in the order tried (decision 234).
pub const RULES: [&str; 3] = [
    "location, then set",
    "location, then day or night, then set",
    "shared cast",
];

/// Where the suggestion came from: one of [`RULES`], or the board's own order.
pub const KEEP: &str = "the order on the board";

/// The search from the cheapest order so far (decision 246).
pub const SWAPS: &str = "swapping scenes from the cheapest order";

/// How many swaps the search tries at most, so a long schedule answers quickly.
const MAX_SWAP_TRIES: usize = 2_000_000;
/// With more movable scenes than this, the search is left out.
const MAX_SEARCHED_SCENES: usize = 2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub board: BoardId,
    pub sub_board: SubBoardId,
    /// The sub-board's strips in the suggested order: every strip, once.
    pub order: Vec<StripId>,
    pub before: Score,
    pub after: Score,
    /// Scene strips that would change place.
    pub moved: usize,
    /// Scene strips that stay where they are: pinned, omitted (decision 91) or one part of
    /// a scene spanning days.
    pub pinned: usize,
    /// Location and set groups.
    pub groups: usize,
    /// The rule the suggestion follows: one of [`RULES`], [`SWAPS`], or [`KEEP`] when none
    /// costs less than the board's order.
    pub rule: &'static str,
    /// Each rule tried with the cost of its order.
    pub tried: Vec<(&'static str, u64)>,
    /// The weights the costs were worked out with.
    pub weights: OrderWeights,
}

fn norm(t: &str) -> String {
    t.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// The sheets and elements by id, looked up once for a whole suggestion.
struct Ix<'a> {
    sheets: SheetIndex<'a>,
    elements: ElementIndex<'a>,
    cast: Option<CategoryId>,
}

impl<'a> Ix<'a> {
    fn new(s: &'a Schedule) -> Ix<'a> {
        Ix {
            sheets: s.sheet_index(),
            elements: s.element_index(),
            cast: s.cast(),
        }
    }

    /// The location a scene is at: its location, else its set.
    fn place_key(&self, sheet: SheetId) -> String {
        self.sheets.get(&sheet).map_or(String::new(), |h| {
            if h.location.trim().is_empty() {
                norm(&h.set)
            } else {
                norm(&h.location)
            }
        })
    }

    /// Whether the scene is shot in the dark: night, dusk, evening or dawn.
    fn dark(&self, sheet: SheetId) -> bool {
        self.sheets.get(&sheet).is_some_and(|h| {
            let t = norm(&h.time_of_day);
            ["NIGHT", "DUSK", "EVENING", "DAWN"]
                .iter()
                .any(|w| t.contains(w))
        })
    }

    fn set(&self, sheet: SheetId) -> String {
        self.sheets
            .get(&sheet)
            .map_or(String::new(), |h| norm(&h.set))
    }

    fn cast_of(&self, sheet: SheetId) -> BTreeSet<ElementId> {
        self.sheets.get(&sheet).map_or(BTreeSet::new(), |h| {
            h.elements
                .iter()
                .copied()
                .filter(|e| {
                    self.elements
                        .get(e)
                        .is_some_and(|x| Some(x.category) == self.cast)
                })
                .collect()
        })
    }

    /// Whether an element of the scene has a red flag on `d`.
    fn flagged(&self, sheet: SheetId, d: Date) -> bool {
        self.sheets.get(&sheet).is_some_and(|h| {
            h.elements.iter().any(|e| {
                self.elements
                    .get(e)
                    .is_some_and(|x| x.red_flags.iter().any(|f| f.dates.covers(d)))
            })
        })
    }
}

/// How the sub-board's current order scores. An omitted scene counts nowhere (decision
/// 91).
pub fn score(s: &Schedule, board: BoardId, sub: SubBoardId) -> Result<Score, Error> {
    let ix = Ix::new(s);
    let all = days_with(s, &ix.sheets, board, sub)?;
    let mut out = Score {
        shoot_days: all.days.len() as u32,
        ..Score::default()
    };
    for d in &all.days {
        let mut cast: BTreeSet<ElementId> = BTreeSet::new();
        let mut last: Option<(String, bool)> = None;
        for sheet in d.counted_sheets() {
            cast.extend(ix.cast_of(sheet));
            let here = (ix.place_key(sheet), ix.dark(sheet));
            if let Some((place, dark)) = &last {
                out.location_changes += u32::from(*place != here.0);
                out.day_night_switches += u32::from(*dark != here.1);
            }
            last = Some(here);
        }
        out.cast_days += cast.len() as u32;
    }
    out.red_flag_clashes = red_flags(s, board, sub)?.clashes.len() as u32;
    let limit = s.plan.long_day();
    out.long_day_hours = timelines(s, board, sub)?
        .iter()
        .map(|t| t.length.saturating_sub(limit).div_ceil(60))
        .sum();
    Ok(out)
}

/// A scene as the search sees it.
struct Facts {
    place: usize,
    dark: bool,
    cast: Vec<u64>,
}

/// A counted entry of a day: a scene that stays, or a place the search fills.
#[derive(Clone, Copy)]
enum Item {
    Fixed(usize),
    Slot(usize),
}

/// One sub-board's days, costed quickly from what a swap can change. An estimate: a real
/// [`score`] has the last word.
struct Estimate {
    weights: OrderWeights,
    limit: u32,
    /// The movable scenes, then the scenes that stay.
    facts: Vec<Facts>,
    minutes: Vec<u32>,
    days: Vec<Vec<Item>>,
    /// Banners and scenes that stay, in minutes.
    base: Vec<u32>,
    slot_day: Vec<usize>,
    /// Per day, per movable scene: how many of its elements are flagged that date.
    flags: Vec<Vec<u32>>,
    words: usize,
}

impl Estimate {
    fn day_cost(&self, day: usize, fill: &[usize], seen: &mut [u64]) -> u64 {
        seen.fill(0);
        let w = &self.weights;
        let mut cost = 0u64;
        let mut minutes = self.base[day];
        let mut last: Option<&Facts> = None;
        for item in &self.days[day] {
            let i = match item {
                Item::Fixed(i) => *i,
                Item::Slot(k) => {
                    let i = fill[*k];
                    minutes += self.minutes[i];
                    if let Some(row) = self.flags.get(day) {
                        cost += u64::from(row[i]) * u64::from(w.red_flag_clash);
                    }
                    i
                }
            };
            let f = &self.facts[i];
            for (a, b) in seen.iter_mut().zip(&f.cast) {
                *a |= *b;
            }
            if let Some(p) = last {
                if p.place != f.place {
                    cost += u64::from(w.location_change);
                }
                if p.dark != f.dark {
                    cost += u64::from(w.day_night_switch);
                }
            }
            last = Some(f);
        }
        let cast: u32 = seen.iter().map(|x| x.count_ones()).sum();
        cost += u64::from(cast) * u64::from(w.cast_day);
        cost + u64::from(minutes.saturating_sub(self.limit).div_ceil(60))
            * u64::from(w.long_day_hour)
    }

    /// Swaps pairs of places while that lowers the estimate. Goes through the pairs in
    /// order, so the same start always gives the same result.
    fn improve(&self, fill: &mut [usize]) {
        let mut seen = vec![0u64; self.words];
        let mut cost: Vec<u64> = (0..self.days.len())
            .map(|d| self.day_cost(d, fill, &mut seen))
            .collect();
        let mut tries = 0;
        loop {
            let mut better = false;
            for a in 0..fill.len() {
                for b in a + 1..fill.len() {
                    if tries >= MAX_SWAP_TRIES {
                        return;
                    }
                    tries += 1;
                    let (da, db) = (self.slot_day[a], self.slot_day[b]);
                    let old = cost[da] + if da == db { 0 } else { cost[db] };
                    fill.swap(a, b);
                    let new_a = self.day_cost(da, fill, &mut seen);
                    let new_b = if da == db {
                        0
                    } else {
                        self.day_cost(db, fill, &mut seen)
                    };
                    if new_a + new_b < old {
                        cost[da] = new_a;
                        if da != db {
                            cost[db] = new_b;
                        }
                        better = true;
                    } else {
                        fill.swap(a, b);
                    }
                }
            }
            if !better {
                return;
            }
        }
    }
}

/// The next item by most shared cast with `prev` (ties: the earliest).
fn closest(prev: &BTreeSet<ElementId>, left: &[(usize, BTreeSet<ElementId>)]) -> usize {
    let mut best = 0;
    let mut best_n = None;
    for (i, (_, c)) in left.iter().enumerate() {
        let n = prev.intersection(c).count();
        if best_n.is_none_or(|b| n > b) {
            best = i;
            best_n = Some(n);
        }
    }
    best
}

/// Chains items (an index and its cast) by most shared cast, starting with the first.
fn chain(mut left: Vec<(usize, BTreeSet<ElementId>)>) -> Vec<usize> {
    let mut out = Vec::new();
    if left.is_empty() {
        return out;
    }
    let (i, mut prev) = left.remove(0);
    out.push(i);
    while !left.is_empty() {
        let k = closest(&prev, &left);
        let (i, c) = left.remove(k);
        out.push(i);
        prev = c;
    }
    out
}

/// A location's sets, each with its scenes (indexes into the movable scenes).
type Sets = Vec<(String, Vec<usize>)>;

/// Works out a suggested order for the sub-board (see the module notes). Nothing changes.
pub fn suggest(s: &Schedule, board: BoardId, sub: SubBoardId) -> Result<Suggestion, Error> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let all = days(s, board, sub)?;
    let scheduled: HashSet<StripId> = all
        .days
        .iter()
        .flat_map(|d| d.strips.iter().copied())
        .collect();
    // Movable scenes, in board order, and the places they fill.
    let mut slots = Vec::new();
    let mut scenes: Vec<(StripId, SheetId)> = Vec::new();
    let mut pinned = 0;
    let split = s.split_sheets(board);
    let ix = Ix::new(s);
    for (i, st) in sb.strips.iter().enumerate() {
        if let StripKind::Sheet(sheet) = st.kind {
            if !scheduled.contains(&st.id) {
                continue;
            }
            // An omitted scene stays where it is too: it isn't shot (decision 91).
            if s.plan.is_pinned(st.id) || split.contains(&sheet) || s.plan.is_omitted(sheet) {
                pinned += 1;
                continue;
            }
            slots.push(i);
            scenes.push((st.id, sheet));
        }
    }
    let cast: Vec<BTreeSet<ElementId>> = scenes.iter().map(|(_, h)| ix.cast_of(*h)).collect();
    // Groups: location, then (with `by_dark`) day or night, then set, in first-seen order.
    let grouped = |by_dark: bool| -> Vec<(String, Sets)> {
        let mut locs: Vec<(String, Sets)> = Vec::new();
        for (i, (_, sheet)) in scenes.iter().enumerate() {
            let mut loc = ix.place_key(*sheet);
            if by_dark {
                loc.push_str(if ix.dark(*sheet) { "\u{0}N" } else { "\u{0}D" });
            }
            let set = ix.set(*sheet);
            let li = match locs.iter().position(|(l, _)| *l == loc) {
                Some(x) => x,
                None => {
                    locs.push((loc, Vec::new()));
                    locs.len() - 1
                }
            };
            let sets = &mut locs[li].1;
            match sets.iter_mut().find(|(x, _)| *x == set) {
                Some((_, v)) => v.push(i),
                None => sets.push((set, vec![i])),
            }
        }
        locs
    };
    let union = |idx: &[usize]| -> BTreeSet<ElementId> {
        idx.iter().flat_map(|i| cast[*i].iter().copied()).collect()
    };
    // Blocks chained by cast, sets within chained by cast, scenes within too.
    let chained = |locs: &[(String, Sets)]| -> Vec<usize> {
        let blocks: Vec<(usize, BTreeSet<ElementId>)> = locs
            .iter()
            .enumerate()
            .map(|(i, (_, sets))| {
                let all: Vec<usize> = sets.iter().flat_map(|(_, v)| v.iter().copied()).collect();
                (i, union(&all))
            })
            .collect();
        let mut order: Vec<usize> = Vec::new();
        for li in chain(blocks) {
            let sets = &locs[li].1;
            let set_items: Vec<(usize, BTreeSet<ElementId>)> = sets
                .iter()
                .enumerate()
                .map(|(i, (_, v))| (i, union(v)))
                .collect();
            for si in chain(set_items) {
                let items: Vec<(usize, BTreeSet<ElementId>)> =
                    sets[si].1.iter().map(|i| (*i, cast[*i].clone())).collect();
                order.extend(chain(items));
            }
        }
        order
    };
    let by_place = grouped(false);
    let groups = by_place.iter().map(|(_, v)| v.len()).sum();
    let candidates = [
        chained(&by_place),
        chained(&grouped(true)),
        chain((0..scenes.len()).map(|i| (i, cast[i].clone())).collect()),
    ];
    // Dates of the places, for red flags.
    let mut slot_date: Vec<Option<Date>> = vec![None; sb.strips.len()];
    if let Some(start) = sb.start {
        for d in &all.days {
            let date = sb.calendar.date_of_day(start, d.number);
            for e in &d.entries {
                slot_date[e.at] = date;
            }
        }
    }
    let flagged = |i: usize, d: Date| ix.flagged(scenes[i].1, d);
    let weights = s.plan.order_weights();
    // An order of the movable scenes into the places, red-flagged dates avoided: which
    // scene goes in each place.
    let fill = |order: &[usize]| -> Vec<usize> {
        let mut left = order.to_vec();
        slots
            .iter()
            .map(|&slot| {
                let pick = match slot_date[slot] {
                    Some(d) => left.iter().position(|&i| !flagged(i, d)).unwrap_or(0),
                    None => 0,
                };
                left.remove(pick)
            })
            .collect()
    };
    // The sub-board's strips with those scenes in the places, and how many moved.
    let strips_of = |filled: &[usize]| -> (Vec<StripId>, usize) {
        let mut new_strips: Vec<StripId> = sb.strips.iter().map(|x| x.id).collect();
        let mut moved = 0;
        for (k, &slot) in slots.iter().enumerate() {
            new_strips[slot] = scenes[filled[k]].0;
            moved += usize::from(filled[k] != k);
        }
        (new_strips, moved)
    };
    let scored = |strips: &[StripId]| -> Result<Score, Error> {
        let mut copy = s.clone();
        reorder(&mut copy, board, sub, strips)?;
        score(&copy, board, sub)
    };
    let before = score(s, board, sub)?;
    let mut best: Option<(&'static str, Vec<usize>, Score)> = None;
    let mut tried = Vec::new();
    for (rule, order) in RULES.iter().zip(&candidates) {
        let filled = fill(order);
        let after = scored(&strips_of(&filled).0)?;
        tried.push((*rule, after.cost(&weights)));
        if best
            .as_ref()
            .is_none_or(|b| after.cost(&weights) < b.2.cost(&weights))
        {
            best = Some((rule, filled, after));
        }
    }
    // Keep the board's own order unless a rule beats it.
    let (mut rule, mut filled, mut after) = match best {
        Some(b) if b.2.cost(&weights) < before.cost(&weights) => b,
        _ => (KEEP, (0..scenes.len()).collect(), before),
    };
    if scenes.len() > 1 && scenes.len() <= MAX_SEARCHED_SCENES {
        let estimate = estimate(s, &ix, board, sub, &all, &slots, &scenes, &slot_date)?;
        let mut swapped = filled.clone();
        estimate.improve(&mut swapped);
        if swapped != filled {
            let found = scored(&strips_of(&swapped).0)?;
            tried.push((SWAPS, found.cost(&weights)));
            if found.cost(&weights) < after.cost(&weights) {
                (rule, filled, after) = (SWAPS, swapped, found);
            }
        }
    }
    let (order, moved) = strips_of(&filled);
    Ok(Suggestion {
        board,
        sub_board: sub,
        order,
        before,
        after,
        moved,
        pinned,
        groups,
        rule,
        tried,
        weights,
    })
}

/// The facts the search needs, from the board as it stands.
#[allow(clippy::too_many_arguments)]
fn estimate(
    s: &Schedule,
    ix: &Ix<'_>,
    board: BoardId,
    sub: SubBoardId,
    all: &crate::days::Days,
    slots: &[usize],
    scenes: &[(StripId, SheetId)],
    slot_date: &[Option<Date>],
) -> Result<Estimate, Error> {
    let mut places: Vec<String> = Vec::new();
    let mut people: Vec<ElementId> = Vec::new();
    for sheet in s.sheets.iter().map(|h| h.id) {
        for e in ix.cast_of(sheet) {
            if !people.contains(&e) {
                people.push(e);
            }
        }
    }
    let words = people.len() / 64 + 1;
    let minutes_of = |sheet: SheetId| {
        ix.sheets
            .get(&sheet)
            .and_then(|h| h.shoot_time)
            .map_or(0, |m| m.0)
    };
    let mut facts_of = |sheet: SheetId| {
        let key = ix.place_key(sheet);
        let place = places.iter().position(|p| *p == key).unwrap_or_else(|| {
            places.push(key);
            places.len() - 1
        });
        let mut cast = vec![0u64; words];
        for e in ix.cast_of(sheet) {
            if let Some(n) = people.iter().position(|p| *p == e) {
                cast[n / 64] |= 1 << (n % 64);
            }
        }
        Facts {
            place,
            dark: ix.dark(sheet),
            cast,
        }
    };
    let mut facts: Vec<Facts> = scenes.iter().map(|(_, h)| facts_of(*h)).collect();
    let minutes: Vec<u32> = scenes.iter().map(|(_, h)| minutes_of(*h)).collect();
    let lengths = timelines(s, board, sub)?;
    let mut days = Vec::new();
    let mut base = Vec::new();
    let mut slot_day = vec![0; slots.len()];
    let mut flags = Vec::new();
    for (n, d) in all.days.iter().enumerate() {
        let mut items = Vec::new();
        let mut length = lengths.get(n).map_or(0, |t| t.length);
        let mut date = None;
        for e in &d.entries {
            date = date.or(slot_date[e.at]);
            if let Ok(k) = slots.binary_search(&e.at) {
                items.push(Item::Slot(k));
                slot_day[k] = n;
                length = length.saturating_sub(minutes[k]);
            } else if let Some(sheet) = e.counted_sheet() {
                facts.push(facts_of(sheet));
                items.push(Item::Fixed(facts.len() - 1));
            }
        }
        days.push(items);
        base.push(length);
        flags.push(
            scenes
                .iter()
                .map(|(_, h)| date.map_or(0, |d| flag_count(ix, *h, d)))
                .collect(),
        );
    }
    Ok(Estimate {
        weights: s.plan.order_weights(),
        limit: s.plan.long_day(),
        facts,
        minutes,
        days,
        base,
        slot_day,
        flags,
        words,
    })
}

/// How many of a scene's elements have a red flag on a date.
fn flag_count(ix: &Ix<'_>, sheet: SheetId, d: Date) -> u32 {
    ix.sheets.get(&sheet).map_or(0, |h| {
        h.elements
            .iter()
            .filter(|e| {
                ix.elements
                    .get(e)
                    .is_some_and(|x| x.red_flags.iter().any(|f| f.dates.covers(d)))
            })
            .count() as u32
    })
}

fn reorder(
    s: &mut Schedule,
    board: BoardId,
    sub: SubBoardId,
    order: &[StripId],
) -> Result<(), Error> {
    let b = s
        .boards
        .iter_mut()
        .find(|x| x.id == board)
        .ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter_mut()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let have: BTreeSet<StripId> = sb.strips.iter().map(|x| x.id).collect();
    let want: BTreeSet<StripId> = order.iter().copied().collect();
    if have != want || order.len() != sb.strips.len() {
        return Err(Error::Broken(
            "the sub-board changed since the order was worked out; suggest it again".to_string(),
        ));
    }
    let mut strips: std::collections::HashMap<StripId, Strip> = std::mem::take(&mut sb.strips)
        .into_iter()
        .map(|x| (x.id, x))
        .collect();
    sb.strips = order
        .iter()
        .map(|id| strips.remove(id).expect("checked above"))
        .collect();
    Ok(())
}

/// Puts a suggestion on the board. Refused when the sub-board's strips changed since.
pub fn apply(s: &mut Schedule, sug: &Suggestion) -> Result<(), Error> {
    reorder(s, sug.board, sug.sub_board, &sug.order)
}

/// The suggestion's counts as plain lines.
pub fn suggestion_text(sug: &Suggestion) -> String {
    let row = |name: &str, a: u32, b: u32| format!("  {name:<20} {a:>5} -> {b}\n");
    let mut out = format!(
        "{} scenes would move ({} stay put: pinned, omitted or spanning days), in {} location and set groups.\n",
        sug.moved, sug.pinned, sug.groups
    );
    out.push_str(&row(
        "Shoot days",
        sug.before.shoot_days,
        sug.after.shoot_days,
    ));
    out.push_str(&row(
        "Cast work days",
        sug.before.cast_days,
        sug.after.cast_days,
    ));
    out.push_str(&row(
        "Location changes",
        sug.before.location_changes,
        sug.after.location_changes,
    ));
    out.push_str(&row(
        "Day/night switches",
        sug.before.day_night_switches,
        sug.after.day_night_switches,
    ));
    out.push_str(&row(
        "Red flag clashes",
        sug.before.red_flag_clashes,
        sug.after.red_flag_clashes,
    ));
    out.push_str(&row(
        "Hours past long day",
        sug.before.long_day_hours,
        sug.after.long_day_hours,
    ));
    let (was, now) = (sug.before.cost(&sug.weights), sug.after.cost(&sug.weights));
    out.push_str(&format!("  {:<20} {was:>5} -> {now}\n", "Cost"));
    let tried: Vec<String> = sug.tried.iter().map(|(r, c)| format!("{r} {c}")).collect();
    out.push_str(&format!(
        "Suggested: {} (costs tried: {}).\n",
        sug.rule,
        tried.join(", ")
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Minutes;
    use crate::model::{Place, RedFlag, RedFlagDates, RedFlagId, Sheet};

    /// Six scenes alternating between two locations, a day break after each pair.
    fn mixed() -> (Schedule, BoardId, SubBoardId) {
        let mut s = Schedule::new("ZTEST Order");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        s.set_start(b, sb, "2027-03-01".parse().ok()).unwrap();
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        let bo = s.add_element(cast, "BO").unwrap();
        for (i, (loc, who)) in [
            ("Pier", ann),
            ("House", bo),
            ("Pier", ann),
            ("House", bo),
            ("Pier", ann),
            ("House", bo),
        ]
        .iter()
        .enumerate()
        {
            let id = s.add_sheet(Sheet {
                scenes: (i + 1).to_string(),
                set: format!("{loc} SET"),
                location: loc.to_string(),
                ..Sheet::default()
            });
            s.tag(id, *who).unwrap();
        }
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
        for at in [2, 5, 8] {
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sb),
                    index: at,
                },
            )
            .unwrap();
        }
        (s, b, sb)
    }

    fn scene_order(s: &Schedule) -> Vec<String> {
        s.boards[0].sub_boards[0]
            .strips
            .iter()
            .map(|x| match x.kind {
                StripKind::Sheet(h) => s.sheet(h).unwrap().scenes.clone(),
                _ => "|".to_string(),
            })
            .collect()
    }

    #[test]
    fn groups_by_location_and_keeps_days() {
        let (mut s, b, sb) = mixed();
        let sug = suggest(&s, b, sb).unwrap();
        assert_eq!(sug.before.shoot_days, 3);
        assert_eq!(sug.after.shoot_days, 3);
        assert_eq!(sug.before.cast_days, 6);
        assert_eq!(sug.after.cast_days, 4);
        assert_eq!(sug.before.location_changes, 3);
        assert_eq!(sug.after.location_changes, 1);
        assert_eq!(sug.groups, 2);
        // Nothing changed yet.
        assert_eq!(
            scene_order(&s),
            ["1", "2", "|", "3", "4", "|", "5", "6", "|"]
        );
        apply(&mut s, &sug).unwrap();
        assert_eq!(
            scene_order(&s),
            ["1", "3", "|", "5", "2", "|", "4", "6", "|"]
        );
        s.check().unwrap();
        assert!(suggestion_text(&sug).contains("Cast work days           6 -> 4"));
        // A changed sub-board refuses an old suggestion.
        let first = s.boards[0].sub_boards[0].strips[0].id;
        s.delete_strip(b, s.boards[0].sub_boards[0].strips[2].id)
            .unwrap();
        assert!(apply(&mut s, &sug).is_err());
        let _ = first;
    }

    #[test]
    fn pins_and_red_flags_are_respected() {
        let (mut s, b, sb) = mixed();
        // Pin scene 2 in place.
        let second = s.boards[0].sub_boards[0].strips[1].id;
        crate::plan::set_pinned(&mut s, &[second], true).unwrap();
        // ANN can't work on 2027-03-01 (day 1).
        let t = s.add_red_flag_type("Unavailable").unwrap();
        let ann = s.elements[0].id;
        s.set_red_flag(
            ann,
            RedFlag {
                id: RedFlagId(0),
                kind: t,
                dates: RedFlagDates::Range {
                    from: "2027-03-01".parse().unwrap(),
                    to: "2027-03-01".parse().unwrap(),
                },
                note: String::new(),
                hard: true,
            },
        )
        .unwrap();
        let sug = suggest(&s, b, sb).unwrap();
        assert_eq!(sug.pinned, 1);
        assert_eq!(sug.before.red_flag_clashes, 1);
        assert_eq!(sug.after.red_flag_clashes, 0);
        apply(&mut s, &sug).unwrap();
        let order = scene_order(&s);
        assert_eq!(order[1], "2");
        assert_ne!(order[0], "1");
        assert_ne!(order[0], "3");
        assert_ne!(order[0], "5");
    }

    /// Decision 234: day and night scenes at one place, alternating on the board.
    fn day_and_night(tods: &[&str]) -> (Schedule, BoardId, SubBoardId) {
        let mut s = Schedule::new("ZTEST Night");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        for (i, tod) in tods.iter().enumerate() {
            let id = s.add_sheet(Sheet {
                scenes: (i + 1).to_string(),
                set: "PIER".into(),
                location: "Pier".into(),
                time_of_day: (*tod).into(),
                ..Sheet::default()
            });
            s.tag(id, ann).unwrap();
        }
        let at = |index| Place {
            sub_board: Some(sb),
            index,
        };
        let strips: Vec<StripId> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        s.move_strips(b, &strips, at(0)).unwrap();
        s.insert_strip(b, StripKind::DayBreak, at(2)).unwrap();
        s.insert_strip(b, StripKind::DayBreak, at(5)).unwrap();
        (s, b, sb)
    }

    #[test]
    fn the_cheapest_order_is_suggested() {
        let (mut s, b, sb) = day_and_night(&["DAY", "NIGHT", "DAY", "NIGHT"]);
        let sug = suggest(&s, b, sb).unwrap();
        assert_eq!(sug.before.day_night_switches, 2);
        assert_eq!(sug.after.day_night_switches, 0);
        assert_eq!(sug.rule, RULES[1]);
        let w = OrderWeights::default();
        assert_eq!(
            sug.before.cost(&w) - sug.after.cost(&w),
            2 * u64::from(w.day_night_switch)
        );
        assert_eq!(sug.tried.len(), 3);
        assert!(sug
            .tried
            .iter()
            .any(|(r, c)| *r == RULES[0] && *c == sug.before.cost(&w)));
        let text = suggestion_text(&sug);
        assert!(
            text.contains("Suggested: location, then day or night, then set"),
            "{text}"
        );
        apply(&mut s, &sug).unwrap();
        assert_eq!(scene_order(&s), ["1", "3", "|", "2", "4", "|"]);
        // Sorted already: nothing costs less, so the board's order stays.
        let again = suggest(&s, b, sb).unwrap();
        assert_eq!((again.rule, again.moved), (KEEP, 0));
        assert_eq!(again.after, again.before);
        assert_eq!(
            again.order,
            s.boards[0].sub_boards[0]
                .strips
                .iter()
                .map(|x| x.id)
                .collect::<Vec<_>>()
        );
    }

    /// Review: the suggestion once put both parts of a scene spanning days on one day.
    #[test]
    fn weights_change_what_is_cheapest_and_are_the_schedules_own() {
        let (mut s, b, sb) = mixed();
        let plain = suggest(&s, b, sb).unwrap();
        assert_eq!(plain.weights, OrderWeights::default());
        let cost = plain.after.cost(&plain.weights);
        // Nothing costs anything: no order beats the board, and nothing would move.
        s.plan
            .set_order_weights(Some(OrderWeights::from_values([0, 0, 0, 0, 0])))
            .unwrap();
        let free = suggest(&s, b, sb).unwrap();
        assert_eq!((free.rule, free.moved), (KEEP, 0));
        assert_eq!(free.after.cost(&free.weights), 0);
        // Limits, and the defaults kept as none.
        assert!(s
            .plan
            .set_order_weights(Some(OrderWeights::from_values([1001, 0, 0, 0, 0])))
            .is_err());
        s.plan
            .set_order_weights(Some(OrderWeights::default()))
            .unwrap();
        assert_eq!(s.plan.order_weights, None);
        assert_eq!(suggest(&s, b, sb).unwrap().after.cost(&plain.weights), cost);
    }

    #[test]
    fn a_long_day_costs_and_the_search_evens_it_out() {
        // Four scenes on two days, both long ones on day 1: each order the rules try keeps
        // them together, since all are at one place with one cast.
        let mut s = Schedule::new("ZTEST Long");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        for (i, hours) in [7, 7, 1, 1].iter().enumerate() {
            s.add_sheet(Sheet {
                scenes: (i + 1).to_string(),
                set: "PIER".to_string(),
                shoot_time: Some(Minutes(hours * 60)),
                ..Sheet::default()
            });
        }
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
        for at in [2, 5] {
            s.insert_strip(
                b,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sb),
                    index: at,
                },
            )
            .unwrap();
        }
        // 14 hours against the 12 hour limit: two hours past.
        let before = score(&s, b, sb).unwrap();
        assert_eq!(before.long_day_hours, 2);
        let sug = suggest(&s, b, sb).unwrap();
        assert_eq!(sug.rule, SWAPS, "{:?}", sug.tried);
        assert_eq!(sug.after.long_day_hours, 0);
        assert!(sug.tried.iter().any(|(r, _)| *r == SWAPS));
        // The same board gives the same answer, so Apply finds what the preview showed.
        assert_eq!(suggest(&s, b, sb).unwrap().order, sug.order);
        apply(&mut s, &sug).unwrap();
        assert_eq!(score(&s, b, sb).unwrap().long_day_hours, 0);
        assert!(s.check().is_ok());
    }

    #[test]
    fn a_scene_spanning_days_keeps_its_parts_apart() {
        let mut s = Schedule::new("T");
        let board = s.boards[0].id;
        let sub = s.boards[0].sub_boards[0].id;
        let mk = |s: &mut Schedule, n: &str, loc: &str| {
            s.add_sheet(Sheet {
                scenes: n.into(),
                location: loc.into(),
                ..Sheet::default()
            })
        };
        let a = mk(&mut s, "1", "Pier");
        mk(&mut s, "2", "House");
        mk(&mut s, "3", "House");
        let at = |index| Place {
            sub_board: Some(sub),
            index,
        };
        let ids: Vec<StripId> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        s.move_strips(board, &ids, at(0)).unwrap();
        // Day 1: A, B; day 2: A (part 2), C.
        s.insert_strip(board, StripKind::DayBreak, at(2)).unwrap();
        s.add_part(board, a, at(3)).unwrap();
        s.insert_strip(board, StripKind::DayBreak, at(5)).unwrap();
        let sug = suggest(&s, board, sub).unwrap();
        assert_eq!(sug.pinned, 2, "both parts stay put");
        apply(&mut s, &sug).unwrap();
        let d = days(&s, board, sub).unwrap();
        let day_of = |st: StripId| d.days.iter().position(|x| x.strips.contains(&st)).unwrap();
        let parts = s.sheet_strips(board, a);
        assert_ne!(day_of(parts[0]), day_of(parts[1]));
    }
}
