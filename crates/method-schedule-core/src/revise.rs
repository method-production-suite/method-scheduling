//! Script import with production tags, and script revisions merged into a schedule
//! (decisions.md, IMP-04 and BYD-01).
//!
//! **Tags.** [`import_tagged`] is the plain import plus each scene's tagged items as
//! elements: matched by name within their category, the category matched by name (a
//! category called Cast or Cast Members is the built-in cast category, whatever it is
//! called here) and created when the schedule has none.
//!
//! **Revisions.** [`revise`] takes a new draft of the script into a schedule made from an
//! older one, without starting over. Each scene of the draft is matched to a sheet: first
//! by scene number (ignoring case), then, for scenes left over, by heading (I/E, set and
//! time of day, ignoring case) among sheets left over. A matched sheet keeps its id, its
//! tags and its place on every board; the fields the script owns (scene number, I/E, set,
//! time of day, page length) take the draft's values, and its new speaking characters
//! (and tags, with tags on) are added. Nothing is untagged: a character who no longer
//! speaks is reported, since they may still be in the scene. Scenes with no match become
//! new sheets at the end of every Boneyard. Sheets the draft no longer has are marked
//! omitted in the plan, never deleted, and a sheet marked omitted that the draft has
//! again loses its mark. Sheets with neither a scene number nor a set (made by hand, not
//! from a script) are left alone. A sheet holding several scenes (`12, 15`, after Merge
//! Strips) matches each of its numbers and keeps its fields.
//!
//! **Drafts.** A revision that changes anything is recorded in the plan as a draft
//! (decision 233): the next revision color (Blue after the original White, then Pink,
//! Yellow and on), the date given, and the sheets it changed, added and omitted.

use std::collections::{HashMap, HashSet};

use crate::fountain::{ImportedScene, ImportedScript, ImportedTag, IntExt};
use crate::model::{same_name, CategoryId, ElementId, Error, Schedule, Sheet, SheetId};
use crate::{Date, Eighths};

/// Category names a script's tags use for the cast.
const CAST_NAMES: [&str; 3] = ["cast", "cast members", "cast member"];

/// The usual breakdown names a script's tags may use for one of our default categories
/// (DEFAULT_CATEGORIES), as `(their name, ours)`. Our reading (E-I5, IMP-04).
const TAG_NAMES: &[(&str, &str)] = &[
    ("props", "Hand props"),
    ("prop", "Hand props"),
    ("vehicles", "Picture vehicles"),
    ("vehicle", "Picture vehicles"),
    ("extras", "Background"),
    ("background actors", "Background"),
    ("extras/atmosphere", "Background"),
    ("costume", "Wardrobe"),
    ("costumes", "Wardrobe"),
    ("makeup", "Hair and makeup"),
    ("makeup/hair", "Hair and makeup"),
    ("hair/makeup", "Hair and makeup"),
    ("special effects", "Practical effects"),
    ("livestock", "Animals"),
    ("animal wrangler", "Animals"),
    ("sound", "Sound and music"),
    ("music", "Sound and music"),
];

/// The category a tag goes in: the cast category for a cast name; the one with its name;
/// else our default category a usual breakdown name stands for, when the schedule has it
/// ([`TAG_NAMES`]); else a new one with its name.
fn tag_category(s: &mut Schedule, name: &str) -> CategoryId {
    if CAST_NAMES.iter().any(|c| same_name(c, name)) {
        return s.cast_or_add();
    }
    if let Some(id) = s.category_named(name) {
        return id;
    }
    let ours = TAG_NAMES
        .iter()
        .find(|(theirs, _)| same_name(theirs, name))
        .and_then(|(_, ours)| s.category_named(ours));
    ours.unwrap_or_else(|| s.add_category(name))
}

/// Tags `sheet` with `tags`, making elements and categories as needed. Returns the
/// elements newly on the sheet.
pub fn apply_tags(
    s: &mut Schedule,
    sheet: SheetId,
    tags: &[ImportedTag],
) -> Result<Vec<ElementId>, Error> {
    let mut out = Vec::new();
    for t in tags {
        let cat = tag_category(s, &t.category);
        let el = match s.element_named(cat, &t.name) {
            Some(e) => e,
            None => s.add_element(cat, &t.name)?,
        };
        let h = s.sheet(sheet).ok_or(Error::NoSuchSheet(sheet))?;
        if !h.elements.contains(&el) {
            // Its linked elements come too, as when tagging by hand (behavior note 04).
            out.extend(s.tag_with_links(sheet, el)?);
        }
    }
    Ok(out)
}

/// Imports a script as [`Schedule::import_script`] does, then tags each new sheet with
/// its scene's production tags.
pub fn import_tagged(
    s: &mut Schedule,
    script: &ImportedScript,
    cast: CategoryId,
) -> Result<Vec<SheetId>, Error> {
    let sheets = s.import_script(script, cast)?;
    for (id, scene) in sheets.iter().zip(&script.scenes) {
        apply_tags(s, *id, &scene.tags)?;
    }
    Ok(sheets)
}

/// One field a revision changed on a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

/// What a revision did to one matched sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revised {
    pub sheet: SheetId,
    /// Matched by heading rather than scene number.
    pub by_heading: bool,
    pub fields: Vec<FieldChange>,
    /// Elements newly on the sheet (speaking characters, and tags with tags on).
    pub tagged: Vec<ElementId>,
    /// Cast on the sheet who no longer speak in the scene (kept).
    pub silent: Vec<ElementId>,
    /// It was marked omitted and the draft has it again.
    pub restored: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Revision {
    /// Every matched sheet, in draft order; unchanged ones too.
    pub matched: Vec<Revised>,
    /// New sheets for scenes with no match, in draft order.
    pub added: Vec<SheetId>,
    /// Sheets the draft no longer has, newly marked omitted.
    pub omitted: Vec<SheetId>,
    /// The color it was recorded as (decision 233); none when it changed nothing.
    pub draft: Option<String>,
}

impl Revision {
    /// Matched sheets with any change.
    pub fn changed(&self) -> impl Iterator<Item = &Revised> {
        self.matched.iter().filter(|r| {
            !r.fields.is_empty() || !r.tagged.is_empty() || !r.silent.is_empty() || r.restored
        })
    }
}

fn int_ext_text(k: IntExt) -> &'static str {
    match k {
        IntExt::Int => "INT",
        IntExt::Ext => "EXT",
        IntExt::IntExt => "INT/EXT",
        IntExt::ExtInt => "EXT/INT",
        IntExt::Unknown => "",
    }
}

fn norm(t: &str) -> String {
    t.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// The scene numbers a sheet holds: `12`, or each of `12, 15`. A range (`12-14`) is one
/// number as written.
fn numbers(scenes: &str) -> Vec<String> {
    scenes
        .split(',')
        .map(norm)
        .filter(|n| !n.is_empty())
        .collect()
}

fn heading_key(int_ext: &str, set: &str, tod: &str) -> (String, String, String) {
    (norm(int_ext), norm(set), norm(tod))
}

/// Merges a revised draft into the schedule (see the module notes). `cast` takes new
/// speaking characters; `with_tags` adds each scene's production tags too; `date` is
/// when, for the draft it is recorded as.
pub fn revise(
    s: &mut Schedule,
    script: &ImportedScript,
    cast: CategoryId,
    with_tags: bool,
    date: Option<Date>,
) -> Result<Revision, Error> {
    s.category(cast).ok_or(Error::NoSuchCategory(cast))?;
    // Candidates: sheets made from a script, in sheet order.
    let candidates: Vec<SheetId> = s
        .sheets
        .iter()
        .filter(|h| !h.scenes.trim().is_empty() || !h.set.trim().is_empty())
        .map(|h| h.id)
        .collect();
    let mut by_number: HashMap<String, SheetId> = HashMap::new();
    for id in &candidates {
        for n in numbers(&s.sheet(*id).unwrap().scenes) {
            by_number.entry(n).or_insert(*id);
        }
    }
    // Draft scene index to (sheet, by heading).
    let mut matched: Vec<Option<(SheetId, bool)>> = vec![None; script.scenes.len()];
    let mut used: HashSet<SheetId> = HashSet::new();
    for (i, sc) in script.scenes.iter().enumerate() {
        let Some(n) = sc.number.as_deref().map(norm).filter(|n| !n.is_empty()) else {
            continue;
        };
        if let Some(id) = by_number.get(&n) {
            let multi = numbers(&s.sheet(*id).unwrap().scenes).len() > 1;
            if multi || !used.contains(id) {
                matched[i] = Some((*id, false));
                used.insert(*id);
            }
        }
    }
    // Numbers the draft uses: a sheet whose number the draft has elsewhere is not a
    // heading match for another scene.
    let draft_numbers: HashSet<String> = script
        .scenes
        .iter()
        .filter_map(|sc| sc.number.as_deref().map(norm))
        .collect();
    for (i, sc) in script.scenes.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let key = heading_key(int_ext_text(sc.int_ext), &sc.set, &sc.time_of_day);
        let found = candidates.iter().copied().find(|id| {
            let h = s.sheet(*id).unwrap();
            !used.contains(id)
                && heading_key(&h.int_ext, &h.set, &h.time_of_day) == key
                && numbers(&h.scenes)
                    .iter()
                    .all(|n| !draft_numbers.contains(n))
        });
        if let Some(id) = found {
            matched[i] = Some((id, true));
            used.insert(id);
        }
    }

    let mut out = Revision::default();
    let mut seen: HashSet<SheetId> = HashSet::new();
    for (i, sc) in script.scenes.iter().enumerate() {
        let Some((id, by_heading)) = matched[i] else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        out.matched
            .push(update(s, id, sc, cast, with_tags, by_heading)?);
    }
    // New scenes, as their own little script so the import's rules apply.
    let new_scenes: Vec<ImportedScene> = script
        .scenes
        .iter()
        .enumerate()
        .filter(|(i, _)| matched[*i].is_none())
        .map(|(_, sc)| sc.clone())
        .collect();
    if !new_scenes.is_empty() {
        let part = ImportedScript {
            title: None,
            scenes: new_scenes,
        };
        out.added = if with_tags {
            import_tagged(s, &part, cast)?
        } else {
            s.import_script(&part, cast)?
        };
    }
    for id in candidates {
        if !used.contains(&id) && !s.plan.omitted.contains(&id) {
            s.plan.omitted.push(id);
            out.omitted.push(id);
        }
    }
    let changed: Vec<SheetId> = out.changed().map(|r| r.sheet).collect();
    if !(changed.is_empty() && out.added.is_empty() && out.omitted.is_empty()) {
        let color = crate::plan::draft_color(s.plan.drafts.len() + 1);
        s.plan.drafts.push(crate::plan::Draft {
            color: color.clone(),
            date,
            changed,
            added: out.added.clone(),
            omitted: out.omitted.clone(),
        });
        out.draft = Some(color);
    }
    crate::plan::tidy(s);
    Ok(out)
}

fn update(
    s: &mut Schedule,
    id: SheetId,
    sc: &ImportedScene,
    cast: CategoryId,
    with_tags: bool,
    by_heading: bool,
) -> Result<Revised, Error> {
    let mut fields = Vec::new();
    let multi = numbers(&s.sheet(id).ok_or(Error::NoSuchSheet(id))?.scenes).len() > 1;
    if !multi {
        let h = s.sheet_mut(id).unwrap();
        let mut set = |field: &'static str, slot: &mut String, value: &str| {
            if slot.trim() != value.trim() {
                fields.push(FieldChange {
                    field,
                    before: slot.clone(),
                    after: value.to_string(),
                });
                *slot = value.to_string();
            }
        };
        if let Some(n) = sc.number.as_deref() {
            set("scenes", &mut h.scenes, n);
        }
        set("int_ext", &mut h.int_ext, int_ext_text(sc.int_ext));
        set("set", &mut h.set, &sc.set);
        set("time_of_day", &mut h.time_of_day, &sc.time_of_day);
        if h.pages != sc.length && sc.length != Eighths::ZERO {
            fields.push(FieldChange {
                field: "eighths",
                before: h.pages.to_string(),
                after: sc.length.to_string(),
            });
            h.pages = sc.length;
        }
    }
    let mut tagged = Vec::new();
    let mut speaking = Vec::new();
    for name in &sc.characters {
        let el = match s.element_named(cast, name) {
            Some(e) => e,
            None => s.add_element(cast, name)?,
        };
        speaking.push(el);
        if !s.sheet(id).unwrap().elements.contains(&el) {
            tagged.extend(s.tag_with_links(id, el)?);
        }
    }
    if with_tags {
        tagged.extend(apply_tags(s, id, &sc.tags)?);
    }
    let silent = if multi {
        Vec::new()
    } else {
        s.sheet(id)
            .unwrap()
            .elements
            .iter()
            .copied()
            .filter(|e| s.element(*e).is_some_and(|x| x.category == cast) && !speaking.contains(e))
            .filter(|e| {
                // A tagged cast member (from the draft's tags) is not silent either.
                !sc.tags
                    .iter()
                    .any(|t| s.element(*e).is_some_and(|x| same_name(&x.name, &t.name)))
            })
            .collect()
    };
    let restored = s.plan.omitted.contains(&id);
    s.plan.omitted.retain(|x| *x != id);
    Ok(Revised {
        sheet: id,
        by_heading,
        fields,
        tagged,
        silent,
        restored,
    })
}

/// The revision in plain lines, for people: a count line, then each changed, new and
/// omitted scene.
pub fn revision_text(s: &Schedule, r: &Revision) -> String {
    // The scene as a person knows it, never the sheet's id (BYD-01, decision 162).
    let name = |id: SheetId| {
        s.sheet(id).map_or_else(
            || "A scene no longer in the schedule".to_string(),
            Sheet::title,
        )
    };
    let els = |v: &[ElementId]| {
        v.iter()
            .filter_map(|e| s.element(*e))
            .map(|e| e.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let changed = r.changed().count();
    let mut out = format!(
        "{} scenes matched ({} changed), {} new, {} omitted.\n",
        r.matched.len(),
        changed,
        r.added.len(),
        r.omitted.len()
    );
    for m in r.changed() {
        let how = if m.by_heading {
            " (matched by heading)"
        } else {
            ""
        };
        out.push_str(&format!("Changed {}{how}:", name(m.sheet)));
        let mut parts: Vec<String> = m
            .fields
            .iter()
            .map(|f| format!(" {} {:?} -> {:?}", f.field, f.before, f.after))
            .collect();
        if !m.tagged.is_empty() {
            parts.push(format!(" added {}", els(&m.tagged)));
        }
        if !m.silent.is_empty() {
            parts.push(format!(" no longer speaking (kept) {}", els(&m.silent)));
        }
        if m.restored {
            parts.push(" back from omitted".to_string());
        }
        out.push_str(&parts.join(";"));
        out.push('\n');
    }
    for id in &r.added {
        out.push_str(&format!("New {} in the Boneyard\n", name(*id)));
    }
    for id in &r.omitted {
        out.push_str(&format!("Omitted {} (kept, marked omitted)\n", name(*id)));
    }
    if let Some(c) = &r.draft {
        out.push_str(&format!("Recorded as the {c} draft.\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fountain::parse;
    use crate::model::{Place, StripKind};

    /// IMP-04 with an invented tagged script (fixtures/scripts/field-station-tagged.fdx):
    /// the usual breakdown names land in our default categories, and nothing new is made.
    #[test]
    fn a_tagged_script_fills_our_default_categories() {
        let text = include_str!("../../../fixtures/scripts/field-station-tagged.fdx");
        let script = crate::fdx::parse_with_tags(text).unwrap();
        let mut s = Schedule::with_default_categories("Tagged");
        let before = s.categories.len();
        let cast = s.cast_or_add();
        let sheets = import_tagged(&mut s, &script, cast).unwrap();
        assert_eq!(sheets.len(), 3);
        assert_eq!(s.categories.len(), before, "no new categories");
        let in_cat = |cat: &str, name: &str| {
            let c = s.category_named(cat).unwrap();
            s.element_named(c, name)
                .unwrap_or_else(|| panic!("{name} in {cat}"))
        };
        let mara = in_cat("Cast", "MARA");
        let teo = in_cat("Cast", "TEO");
        let radio = in_cat("Hand props", "HAND RADIO");
        let truck = in_cat("Picture vehicles", "PARK TRUCK");
        let drum = in_cat("Set dressing", "FUEL DRUM");
        let on = |i: usize| s.sheet(sheets[i]).unwrap().elements.clone();
        for e in [mara, radio, drum] {
            assert!(on(0).contains(&e));
        }
        for e in [teo, truck] {
            assert!(on(1).contains(&e));
        }
        assert!(on(2).contains(&radio) && on(2).contains(&mara));
        // A name we don't know still makes its own category.
        let mut t = Schedule::with_default_categories("Other");
        assert_eq!(
            tag_category(&mut t, "Greenery"),
            t.category_named("Greenery").unwrap()
        );
    }

    const DRAFT1: &str = "\
INT. KITCHEN - DAY #1#

ANN
Coffee?

EXT. PIER - NIGHT #2#

BO
Cold.

INT. CAR - DAY #3#

ANN
Drive.
";

    /// Scene 1 grows, 2 is renumbered 2A, 3 is cut, 4 is new, and BO speaks in 1.
    const DRAFT2: &str = "\
INT. KITCHEN - MORNING #1#

ANN
Coffee?

BO
Please.

ANN
More words to make this scene longer than it was before, a good deal longer, with many
more lines of action and talk.

EXT. PIER - NIGHT #2A#

BO
Cold.

INT. BOAT - NIGHT #4#

CY
Hello.
";

    fn base() -> Schedule {
        let mut s = Schedule::new("ZTEST Revise");
        let cast = s.cast().unwrap();
        s.import_script(&parse(DRAFT1), cast).unwrap();
        // Put scene 2 on the board and tag a prop on it.
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let st = s.boards[0].boneyard[1].id;
        s.move_strips(
            b,
            &[st],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        let props = s.add_category("Props");
        let net = s.add_element(props, "NET").unwrap();
        let s2 = s.sheets[1].id;
        s.tag(s2, net).unwrap();
        s
    }

    /// Decision 233: each revision that changes anything is the next color, dated.
    #[test]
    fn each_draft_is_recorded_with_its_color_and_date() {
        use crate::plan::draft_color;
        let mut s = base();
        let ids: Vec<SheetId> = s.sheets.iter().map(|h| h.id).collect();
        let cast = s.cast().unwrap();
        let day = "2027-02-10".parse().ok();
        let r = revise(&mut s, &parse(DRAFT2), cast, false, day).unwrap();
        assert_eq!(r.draft.as_deref(), Some("Blue"));
        let d = &s.plan.drafts[0];
        assert_eq!((d.color.as_str(), d.date), ("Blue", day));
        assert_eq!(d.changed, vec![ids[0], ids[1]]);
        assert_eq!(d.added, r.added);
        assert_eq!(d.omitted, vec![ids[2]]);
        assert!(revision_text(&s, &r).ends_with("Recorded as the Blue draft.\n"));
        assert_eq!(s.plan.revised_in(ids[0]).unwrap().color, "Blue");
        assert!(s.plan.revised_in(ids[2]).is_none(), "omitted, not revised");
        s.check().unwrap();
        // The same draft again changes nothing: no draft.
        let again = revise(&mut s, &parse(DRAFT2), cast, false, None).unwrap();
        assert_eq!(again.draft, None);
        assert_eq!(s.plan.drafts.len(), 1);
        // Back to the first draft: Pink, and scene 3 restored counts as changed.
        let r = revise(&mut s, &parse(DRAFT1), cast, false, None).unwrap();
        assert_eq!(r.draft.as_deref(), Some("Pink"));
        assert!(s.plan.drafts[1].changed.contains(&ids[2]));
        assert_eq!(s.plan.revised_in(ids[0]).unwrap().color, "Pink");
        // The colors run on past Cherry.
        assert_eq!(draft_color(8), "Cherry");
        assert_eq!(draft_color(9), "Double White");
        assert_eq!(draft_color(10), "Double Blue");
        assert_eq!(draft_color(19), "Triple Blue");
        assert_eq!(draft_color(28), "Blue 4");
        // Two drafts of one color don't pass check.
        let mut bad = s.clone();
        bad.plan.drafts[1].color = "blue".into();
        assert!(bad.check().is_err());
    }

    #[test]
    fn a_revision_keeps_identity_tags_and_places() {
        let mut s = base();
        let ids: Vec<SheetId> = s.sheets.iter().map(|h| h.id).collect();
        let before_strip = s.boards[0].sub_boards[0].strips[0].clone();
        let cast = s.cast().unwrap();
        let r = revise(&mut s, &parse(DRAFT2), cast, false, None).unwrap();
        assert_eq!(r.matched.len(), 2);
        // Scene 1 by number: time of day and length changed, BO added.
        let m1 = &r.matched[0];
        assert_eq!(m1.sheet, ids[0]);
        assert!(!m1.by_heading);
        assert!(m1
            .fields
            .iter()
            .any(|f| f.field == "time_of_day" && f.after == "MORNING"));
        assert!(m1.fields.iter().any(|f| f.field == "eighths"));
        assert_eq!(s.element(m1.tagged[0]).unwrap().name, "BO");
        // Scene 2 matched by heading, renumbered, its prop and place kept.
        let m2 = &r.matched[1];
        assert_eq!(m2.sheet, ids[1]);
        assert!(m2.by_heading);
        assert_eq!(s.sheet(ids[1]).unwrap().scenes, "2A");
        assert_eq!(s.sheet(ids[1]).unwrap().elements.len(), 2);
        assert_eq!(s.boards[0].sub_boards[0].strips[0], before_strip);
        // Scene 3 omitted, kept; scene 4 new in the Boneyard with a new cast member.
        assert_eq!(r.omitted, vec![ids[2]]);
        assert!(s.sheet(ids[2]).is_some());
        assert!(s.plan.is_omitted(ids[2]));
        assert_eq!(r.added.len(), 1);
        let new = s.sheet(r.added[0]).unwrap();
        assert_eq!(new.scenes, "4");
        assert!(matches!(
            s.boards[0].boneyard.last().unwrap().kind,
            StripKind::Sheet(id) if id == r.added[0]
        ));
        let text = revision_text(&s, &r);
        assert!(
            text.starts_with("2 scenes matched (2 changed), 1 new, 1 omitted.\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "Changed Scene 2A, EXT. PIER (matched by heading): scenes \"2\" -> \"2A\"\n"
            ),
            "{text}"
        );
        assert!(text.contains("Omitted Scene 3, INT. CAR (kept, marked omitted)\n"));
        s.check().unwrap();
        // The first draft again: scene 3 comes back, 4 is omitted, nothing duplicated.
        let n = s.sheets.len();
        let r = revise(&mut s, &parse(DRAFT1), cast, false, None).unwrap();
        assert_eq!(s.sheets.len(), n);
        assert!(r.added.is_empty());
        assert!(r.matched.iter().any(|m| m.sheet == ids[2] && m.restored));
        assert!(!s.plan.is_omitted(ids[2]));
        // Scene 1 keeps BO (never untagged) and says he no longer speaks.
        let m1 = r.matched.iter().find(|m| m.sheet == ids[0]).unwrap();
        assert_eq!(s.element(m1.silent[0]).unwrap().name, "BO");
    }

    #[test]
    fn tagged_import_makes_categories_and_elements() {
        let script = crate::fdx::parse_with_tags(crate::fdx::tests::TAGGED).unwrap();
        let mut s = Schedule::new("ZTEST Tags");
        let cast = s.cast().unwrap();
        let ids = import_tagged(&mut s, &script, cast).unwrap();
        assert_eq!(ids.len(), 2);
        let props = s.category_named("Props").unwrap();
        let dressing = s.category_named("Set Dressing").unwrap();
        let lantern = s.element_named(props, "lantern").unwrap();
        assert!(s.sheet(ids[0]).unwrap().elements.contains(&lantern));
        assert!(s.sheet(ids[1]).unwrap().elements.contains(&lantern));
        assert!(s.element_named(dressing, "rope").is_some());
        // Cast Members went to the cast: MARA once, on both scenes.
        assert!(s.category_named("Cast Members").is_none());
        let mara = s.element_named(cast, "MARA").unwrap();
        assert!(s.sheet(ids[1]).unwrap().elements.contains(&mara));
        assert_eq!(s.elements.iter().filter(|e| e.name == "MARA").count(), 1);
    }

    /// Review: a revision and an import tagged an element without its linked ones.
    #[test]
    fn tags_from_a_script_bring_linked_elements() {
        let mut s = base();
        let cast = s.cast().unwrap();
        let bo = s.element_named(cast, "BO").unwrap();
        let props = s.category_named("Props").unwrap();
        let net = s.element_named(props, "NET").unwrap();
        s.set_links(bo, &[net], false).unwrap();
        let s1 = s.sheets[0].id;
        let r = revise(&mut s, &parse(DRAFT2), cast, false, None).unwrap();
        assert!(s.sheet(s1).unwrap().elements.contains(&net), "BO's NET");
        assert!(r.matched[0].tagged.contains(&net));
        // An import does the same.
        let mut t = Schedule::new("T");
        let cast = t.cast().unwrap();
        let ann = t.add_element(cast, "ANN").unwrap();
        let props = t.add_category("Props");
        let cup = t.add_element(props, "CUP").unwrap();
        t.set_links(ann, &[cup], false).unwrap();
        let ids = t.import_script(&parse(DRAFT1), cast).unwrap();
        assert!(t.sheet(ids[0]).unwrap().elements.contains(&cup));
        // And a tag in the script.
        let tag = ImportedTag {
            category: "Cast".into(),
            name: "ANN".into(),
        };
        let h = t.add_sheet(crate::model::Sheet::default());
        assert_eq!(apply_tags(&mut t, h, &[tag]).unwrap(), [ann, cup]);
    }

    /// Review: omitted marks were never read. Decision 91: an omitted scene stays on the
    /// board but counts nowhere.
    #[test]
    fn an_omitted_scene_counts_nowhere() {
        use crate::model::{RedFlag, RedFlagDates, RedFlagId, Sheet};
        let mut s = Schedule::new("T");
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        let bo = s.add_element(cast, "BO").unwrap();
        let gone = s.add_sheet(Sheet {
            scenes: "1".into(),
            pages: crate::Eighths(16),
            shoot_time: Some(crate::model::Minutes(120)),
            elements: vec![ann],
            ..Sheet::default()
        });
        let kept = s.add_sheet(Sheet {
            scenes: "2".into(),
            pages: crate::Eighths(8),
            shoot_time: Some(crate::model::Minutes(60)),
            elements: vec![bo],
            ..Sheet::default()
        });
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let at = |index| Place {
            sub_board: Some(sb),
            index,
        };
        let ids: Vec<_> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        s.move_strips(b, &ids, at(0)).unwrap();
        s.insert_strip(b, StripKind::DayBreak, at(2)).unwrap();
        s.set_start(b, sb, Some("2027-03-01".parse().unwrap()))
            .unwrap();
        let unavailable = s.add_red_flag_type("Unavailable").unwrap();
        let day1 = "2027-03-01".parse().unwrap();
        s.set_red_flag(
            ann,
            RedFlag {
                id: RedFlagId(0),
                kind: unavailable,
                dates: RedFlagDates::Range {
                    from: day1,
                    to: day1,
                },
                note: String::new(),
                hard: true,
            },
        )
        .unwrap();
        assert_eq!(
            crate::redflags::red_flags(&s, b, sb).unwrap().clashes.len(),
            1
        );
        s.plan.omitted.push(gone);
        s.check().unwrap();
        // Still on the board, in its day.
        let d = crate::days::days(&s, b, sb).unwrap();
        assert_eq!(d.days[0].strips.len(), 2);
        assert!(d.days[0].entries[0].omitted);
        // No pages or time.
        assert_eq!(d.days[0].pages, crate::Eighths(8));
        assert_eq!(d.days[0].time.0, 60);
        // No work: no day out of days row, no red flag clash.
        let rows = crate::dood::dood(&s, b, sb).unwrap();
        assert!(rows.iter().all(|r| r.element != ann));
        assert!(crate::redflags::red_flags(&s, b, sb)
            .unwrap()
            .clashes
            .is_empty());
        // Not on the timeline or call sheet.
        let t = crate::timeline::timelines(&s, b, sb).unwrap();
        assert_eq!(t[0].entries.len(), 1);
        let c = crate::callsheet::call_sheet(&s, b, sb, 1).unwrap();
        assert_eq!(c.scenes.len(), 1);
        assert_eq!(c.scenes[0].sheet, kept);
        // Not counted in the element list; stays put in a suggested order.
        let list = crate::reports::element_list(&s, b, sb).unwrap();
        let line = list.iter().find(|l| l.element == ann).unwrap();
        assert_eq!((line.sheets, line.days), (0, 0));
        let sug = crate::optimize::suggest(&s, b, sb).unwrap();
        assert_eq!(sug.pinned, 1);
        assert_eq!(crate::optimize::score(&s, b, sb).unwrap().cast_days, 1);
    }
}
