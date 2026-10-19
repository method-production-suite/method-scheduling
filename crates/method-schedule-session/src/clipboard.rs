//! Cut, copy and paste: the app's own clipboard, held in the session (decisions.md, entry
//! 38). Nothing here touches the system clipboard: the page puts plain text there itself,
//! from the DOM's copy and cut events, which need no permission.
//!
//! **Strips.** Copy takes a snapshot of the selected strips in board order: each scene's
//! whole sheet, each banner's text and duration, each day break. Cut takes the same
//! snapshot and marks the strips; nothing leaves the board until the paste. Paste puts
//! them below the selection:
//!
//! - strips cut on the board pasted on: they **move** there (`Schedule::move_strips`), so
//!   no sheet is copied and every board still holds every sheet once;
//! - otherwise (a copy, or a cut pasted on another board): **copies** go there. A scene
//!   becomes a new sheet with the snapshot's fields and the elements still in the
//!   schedule, its strip at the paste on this board and at the end of every other
//!   board's Boneyard, as a new sheet's is. Banners and day breaks are new strips.
//!
//! A cut is used up by its paste; a copy can be pasted again. Banners never go in the
//! Boneyard, so a paste there that holds one is refused; day breaks park there
//! (decisions.md, entry 57).
//!
//! **Strips carried to another window** (decisions.md, entry 183). Each window has its
//! own schedule, so the ids in a clip mean nothing in another one. When strips are cut
//! or copied, every other window's clipboard gets them as [`Clip::Carried`]: each scene's
//! sheet with its elements by category and name. A paste there always makes copies, as a
//! cut pasted on another board does; the originals stay in their own schedule. Each
//! element is matched by category and name ignoring case, or added; header values take
//! the spelling the target already has (entry 155). One paste is one undo step there.
//!
//! **Elements.** Copy remembers the chosen elements of one sheet; cut marks them. Paste on
//! a sheet tags them there (each once); a cut also takes them off the sheet they were cut
//! from, which is a move, and is used up.

use serde::Serialize;

use method_schedule_core::model::{
    Banner, BoardId, Category, CategoryId, ElementId, Place, Quantity, Sheet, SheetId, StripId,
    StripKind, SubBoardId,
};
use method_schedule_core::reports::{set_header_value, HeaderField};
use method_schedule_core::Schedule;

use crate::board::{board_id, EditResult};
use crate::sheet::sheet_title;

/// What the clipboard holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clip {
    Strips {
        /// The board they were copied or cut on.
        board: BoardId,
        cut: bool,
        items: Vec<ClipStrip>,
    },
    Elements {
        /// The sheet they were copied or cut from.
        sheet: SheetId,
        cut: bool,
        elements: Vec<ElementId>,
    },
    /// Strips cut or copied in another window's schedule.
    Carried {
        /// That schedule's name, for the notice after the paste.
        from: String,
        cut: bool,
        items: Vec<Carried>,
    },
}

/// One strip carried from another schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Carried {
    /// A scene: its sheet as it was (its element ids and quantities are the other
    /// schedule's and are not used) and its elements by name.
    Sheet {
        sheet: Box<Sheet>,
        elements: Vec<CarriedElement>,
    },
    Banner(Banner),
    DayBreak,
}

/// An element on a carried sheet, as the other schedule had it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarriedElement {
    /// Its category there, whole: a category the target lacks is added with its settings.
    pub category: Category,
    pub name: String,
    /// Its quantity on the sheet, when it had one.
    pub count: Option<(u32, bool)>,
}

/// One strip on the clipboard: the strip it was, and a snapshot of what it held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipStrip {
    pub strip: StripId,
    pub what: ClipWhat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipWhat {
    Sheet(Box<Sheet>),
    Banner(Banner),
    DayBreak,
}

/// The clipboard as the page needs it: what Paste would paste, and which strips or
/// elements to mark as cut.
#[derive(Clone, Debug, Serialize)]
pub struct ClipboardDto {
    /// `strips` or `elements`.
    pub kind: &'static str,
    pub count: usize,
    pub cut: bool,
    /// Strips: the board they came from.
    pub board: Option<u32>,
    /// Strips cut: their ids, for the cut marks on that board.
    pub strips: Vec<u32>,
    /// Elements: the sheet they came from.
    pub sheet: Option<u32>,
    /// Elements cut: their ids, for the cut marks on that sheet.
    pub elements: Vec<u32>,
    /// What the strips hold: how many scenes, banners and day breaks.
    pub scenes: usize,
}

pub fn dto(clip: &Clip) -> ClipboardDto {
    match clip {
        Clip::Strips { board, cut, items } => ClipboardDto {
            kind: "strips",
            count: items.len(),
            cut: *cut,
            board: Some(board.0),
            strips: if *cut {
                items.iter().map(|i| i.strip.0).collect()
            } else {
                Vec::new()
            },
            sheet: None,
            elements: Vec::new(),
            scenes: items
                .iter()
                .filter(|i| matches!(i.what, ClipWhat::Sheet(_)))
                .count(),
        },
        Clip::Elements {
            sheet,
            cut,
            elements,
        } => ClipboardDto {
            kind: "elements",
            count: elements.len(),
            cut: *cut,
            board: None,
            strips: Vec::new(),
            sheet: Some(sheet.0),
            elements: if *cut {
                elements.iter().map(|e| e.0).collect()
            } else {
                Vec::new()
            },
            scenes: 0,
        },
        Clip::Carried { cut, items, .. } => ClipboardDto {
            kind: "strips",
            count: items.len(),
            cut: *cut,
            board: None,
            strips: Vec::new(),
            sheet: None,
            elements: Vec::new(),
            scenes: items
                .iter()
                .filter(|i| matches!(i, Carried::Sheet { .. }))
                .count(),
        },
    }
}

/// `1 strip`, `3 strips`; `1 element`, `2 elements`.
pub fn count_text(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

// ---------------------------------------------------------------------------
// Strips

/// The strips `strips` of `board` as a clip, in board order (each sub-board's strips in
/// turn, then the Boneyard). Each must be on the board, once.
pub fn copy_strips(s: &Schedule, board: u32, strips: &[u32], cut: bool) -> EditResult<Clip> {
    let board = board_id(s, board)?;
    if strips.is_empty() {
        return Err("Select the strips to copy first.".to_string());
    }
    let b = s.board(board).expect("board_id checked it");
    let wanted: Vec<StripId> = strips.iter().map(|&i| StripId(i)).collect();
    for (i, id) in wanted.iter().enumerate() {
        if wanted[..i].contains(id) {
            return Err("A strip was named twice.".to_string());
        }
    }
    let mut items = Vec::with_capacity(wanted.len());
    for st in b
        .sub_boards
        .iter()
        .flat_map(|x| &x.strips)
        .chain(&b.boneyard)
    {
        if !wanted.contains(&st.id) {
            continue;
        }
        let what = match &st.kind {
            StripKind::Sheet(id) => ClipWhat::Sheet(Box::new(
                s.sheet(*id)
                    .ok_or("That sheet is no longer in the schedule.")?
                    .clone(),
            )),
            StripKind::Banner(bn) => ClipWhat::Banner(bn.clone()),
            StripKind::DayBreak => ClipWhat::DayBreak,
        };
        items.push(ClipStrip { strip: st.id, what });
    }
    if items.len() != wanted.len() {
        return Err("That strip is no longer on the board.".to_string());
    }
    Ok(Clip::Strips { board, cut, items })
}

/// What a paste of strips did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pasted {
    /// The strips now at the paste, in order: the moved ones, or the new copies.
    pub strips: Vec<u32>,
    /// True when cut strips moved; false when copies were made.
    pub moved: bool,
    /// Strips cut on another board: copies were made and the originals stay there.
    pub from_other_board: bool,
}

/// Pastes the clipboard's strips on `board` at `index` of sub-board `sub` (its Boneyard
/// when `None`), the place below the selection the page works out. See the module notes
/// for when strips move and when copies are made.
pub fn paste_strips(
    s: &mut Schedule,
    clip: &Clip,
    board: u32,
    sub: Option<u32>,
    index: usize,
) -> EditResult<Pasted> {
    if let Clip::Carried { cut, items, .. } = clip {
        return paste_carried(s, items, *cut, board, sub, index);
    }
    let Clip::Strips {
        board: from,
        cut,
        items,
    } = clip
    else {
        return Err("The clipboard holds elements. Paste them on a breakdown sheet.".to_string());
    };
    let (board, sub_board) = paste_target(s, board, sub)?;
    let at = Place { sub_board, index };
    if *cut && *from == board {
        // A move: the cut strips still on this board.
        let b = s.board(board).expect("board_id checked it");
        let present: Vec<(StripId, bool)> = b
            .sub_boards
            .iter()
            .flat_map(|x| &x.strips)
            .chain(&b.boneyard)
            .filter(|st| items.iter().any(|i| i.strip == st.id))
            .map(|st| (st.id, !matches!(st.kind, StripKind::Banner(_))))
            .collect();
        if present.is_empty() {
            return Err("The cut strips are no longer on this board.".to_string());
        }
        if sub_board.is_none() && present.iter().any(|(_, scene)| !scene) {
            return Err(no_boneyard());
        }
        let ids: Vec<StripId> = present.iter().map(|(id, _)| *id).collect();
        s.move_strips(board, &ids, at)
            .map_err(crate::elements::say)?;
        return Ok(Pasted {
            strips: ids.iter().map(|id| id.0).collect(),
            moved: true,
            from_other_board: false,
        });
    }
    if sub_board.is_none() && items.iter().any(|i| matches!(i.what, ClipWhat::Banner(_))) {
        return Err(no_boneyard());
    }
    let mut out = Vec::with_capacity(items.len());
    for (k, item) in items.iter().enumerate() {
        let place = Place {
            sub_board,
            index: index.saturating_add(k),
        };
        let id = match &item.what {
            ClipWhat::Sheet(h) => {
                let mut copy = (**h).clone();
                copy.elements.retain(|e| s.element(*e).is_some());
                let on = copy.elements.clone();
                copy.quantities.retain(|q| on.contains(&q.element));
                let sheet = s
                    .add_sheet_at(copy, board, place)
                    .map_err(crate::elements::say)?;
                crate::sheet::strip_of(s, board, sheet.0).ok_or("The pasted scene has no strip.")?
            }
            ClipWhat::Banner(bn) => {
                s.insert_strip(board, StripKind::Banner(bn.clone()), place)
                    .map_err(crate::elements::say)?
                    .0
            }
            ClipWhat::DayBreak => {
                s.insert_strip(board, StripKind::DayBreak, place)
                    .map_err(crate::elements::say)?
                    .0
            }
        };
        out.push(id);
    }
    Ok(Pasted {
        strips: out,
        moved: false,
        from_other_board: *cut,
    })
}

fn no_boneyard() -> String {
    "Banners don't go in the Boneyard. Select a strip on a sub-board to paste below it.".to_string()
}

/// The board and sub-board (`None`: the Boneyard) a paste goes to, checked.
fn paste_target(
    s: &Schedule,
    board: u32,
    sub: Option<u32>,
) -> EditResult<(BoardId, Option<SubBoardId>)> {
    let board = board_id(s, board)?;
    let sub_board = match sub {
        None => None,
        Some(id) => {
            let (b, sb) = crate::board::sub_board(s, id)?;
            if b != board {
                return Err("That sub-board is on another board.".to_string());
            }
            Some(sb)
        }
    };
    Ok((board, sub_board))
}

/// The strips of `clip` as another window's schedule takes them: each scene's sheet with
/// its elements by category and name. `None` for elements, which stay in their window.
pub fn carry(s: &Schedule, clip: &Clip, from: &str) -> Option<Clip> {
    let Clip::Strips { cut, items, .. } = clip else {
        return None;
    };
    let items = items
        .iter()
        .map(|i| match &i.what {
            ClipWhat::Sheet(h) => Carried::Sheet {
                elements: h
                    .elements
                    .iter()
                    .filter_map(|e| s.element(*e))
                    .filter_map(|e| {
                        let q = h.quantities.iter().find(|q| q.element == e.id);
                        Some(CarriedElement {
                            category: s.category(e.category)?.clone(),
                            name: e.name.clone(),
                            count: q.map(|q| (q.count, q.daily)),
                        })
                    })
                    .collect(),
                sheet: h.clone(),
            },
            ClipWhat::Banner(b) => Carried::Banner(b.clone()),
            ClipWhat::DayBreak => Carried::DayBreak,
        })
        .collect();
    Some(Clip::Carried {
        from: from.to_string(),
        cut: *cut,
        items,
    })
}

/// The target's category for a carried one: the one of the same name ignoring case,
/// else for the cast category the target's own cast category, else a new one with the
/// carried settings (never a second cast category).
fn category_for(s: &mut Schedule, c: &Category) -> CategoryId {
    if let Some(id) = s.category_named(&c.name) {
        return id;
    }
    if c.cast {
        if let Some(id) = s.cast() {
            return id;
        }
    }
    let id = s.add_category(&c.name);
    if let Some(n) = s.category_mut(id) {
        n.auto_board_id = c.auto_board_id;
        n.exclude_from_board = c.exclude_from_board;
        n.defaults = c.defaults;
        n.daily_quantity = c.daily_quantity;
        n.properties = c.properties.clone();
        n.multi_line = c.multi_line.clone();
    }
    id
}

/// Pastes strips carried from another window's schedule: copies, as the module notes say.
fn paste_carried(
    s: &mut Schedule,
    items: &[Carried],
    cut: bool,
    board: u32,
    sub: Option<u32>,
    index: usize,
) -> EditResult<Pasted> {
    let (board, sub_board) = paste_target(s, board, sub)?;
    if sub_board.is_none() && items.iter().any(|i| matches!(i, Carried::Banner(_))) {
        return Err(no_boneyard());
    }
    let mut out = Vec::with_capacity(items.len());
    for (k, item) in items.iter().enumerate() {
        let place = Place {
            sub_board,
            index: index.saturating_add(k),
        };
        let id = match item {
            Carried::Sheet { sheet, elements } => {
                let mut copy = (**sheet).clone();
                copy.elements.clear();
                copy.quantities.clear();
                for e in elements {
                    let cat = category_for(s, &e.category);
                    let id = match s.element_named(cat, &e.name) {
                        Some(id) => id,
                        None => s.add_element(cat, &e.name).map_err(crate::elements::say)?,
                    };
                    if copy.elements.contains(&id) {
                        continue;
                    }
                    copy.elements.push(id);
                    if let Some((count, daily)) = e.count {
                        copy.quantities.push(Quantity {
                            element: id,
                            count,
                            daily,
                        });
                    }
                }
                let new = s
                    .add_sheet_at(copy, board, place)
                    .map_err(crate::elements::say)?;
                for f in HeaderField::ALL {
                    let typed = s
                        .sheet(new)
                        .map(|h| f.value(h).to_string())
                        .unwrap_or_default();
                    set_header_value(s, new, f, &typed);
                }
                crate::sheet::strip_of(s, board, new.0).ok_or("The pasted scene has no strip.")?
            }
            Carried::Banner(bn) => {
                s.insert_strip(board, StripKind::Banner(bn.clone()), place)
                    .map_err(crate::elements::say)?
                    .0
            }
            Carried::DayBreak => {
                s.insert_strip(board, StripKind::DayBreak, place)
                    .map_err(crate::elements::say)?
                    .0
            }
        };
        out.push(id);
    }
    Ok(Pasted {
        strips: out,
        moved: false,
        from_other_board: cut,
    })
}

// ---------------------------------------------------------------------------
// Elements

/// The elements `elements` of sheet `sheet` as a clip, in the sheet's order. Each must be
/// on the sheet, once.
pub fn copy_elements(s: &Schedule, sheet: u32, elements: &[u32], cut: bool) -> EditResult<Clip> {
    let h = s
        .sheet(SheetId(sheet))
        .ok_or("That sheet is no longer in the schedule.")?;
    if elements.is_empty() {
        return Err("Select the elements to copy first.".to_string());
    }
    let wanted: Vec<ElementId> = elements.iter().map(|&e| ElementId(e)).collect();
    for (i, id) in wanted.iter().enumerate() {
        if wanted[..i].contains(id) {
            return Err("An element was named twice.".to_string());
        }
        if !h.elements.contains(id) {
            return Err("That element is no longer on this sheet.".to_string());
        }
    }
    let ordered: Vec<ElementId> = h
        .elements
        .iter()
        .copied()
        .filter(|e| wanted.contains(e))
        .collect();
    Ok(Clip::Elements {
        sheet: h.id,
        cut,
        elements: ordered,
    })
}

/// What a paste of elements did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastedElements {
    /// Elements newly on the sheet.
    pub added: usize,
    /// Elements taken off the sheet they were cut from.
    pub moved: usize,
}

/// Pastes the clipboard's elements on sheet `sheet`: each tagged there once; a cut also
/// takes them off the sheet they were cut from. Elements deleted since are skipped.
pub fn paste_elements(s: &mut Schedule, clip: &Clip, sheet: u32) -> EditResult<PastedElements> {
    let Clip::Elements {
        sheet: from,
        cut,
        elements,
    } = clip
    else {
        return Err("The clipboard holds strips. Paste them on the Board.".to_string());
    };
    let to = s
        .sheet(SheetId(sheet))
        .ok_or("That sheet is no longer in the schedule.")?
        .id;
    let live: Vec<ElementId> = elements
        .iter()
        .copied()
        .filter(|e| s.element(*e).is_some())
        .collect();
    if live.is_empty() {
        return Err("The copied elements are no longer in the schedule.".to_string());
    }
    if *cut && *from == to {
        return Ok(PastedElements { added: 0, moved: 0 });
    }
    let mut added = 0;
    for e in &live {
        let on = s.sheet(to).is_some_and(|h| h.elements.contains(e));
        if !on {
            s.tag_with_links(to, *e).map_err(crate::elements::say)?;
            added += 1;
        }
    }
    let mut moved = 0;
    if *cut {
        if let Some(h) = s.sheet_mut(*from) {
            let before = h.elements.len();
            h.elements.retain(|e| !live.contains(e));
            h.quantities.retain(|q| !live.contains(&q.element));
            moved = before - h.elements.len();
        }
    }
    Ok(PastedElements { added, moved })
}

/// The sentence after an element paste.
pub fn elements_notice(s: &Schedule, clip: &Clip, sheet: u32, p: &PastedElements) -> String {
    let to = s.sheet(SheetId(sheet)).map(sheet_title).unwrap_or_default();
    let Clip::Elements {
        sheet: from, cut, ..
    } = clip
    else {
        return String::new();
    };
    if *cut && from.0 == sheet {
        return "Those elements are already on this sheet. Paste them on another sheet to move them."
            .to_string();
    }
    if *cut {
        let from = s.sheet(*from).map(sheet_title);
        return match from {
            Some(f) => format!(
                "Moved {} from {} to {}.",
                count_text(p.moved.max(p.added), "element", "elements"),
                lower_first(&f),
                lower_first(&to)
            ),
            None => format!(
                "Pasted {} onto {}.",
                count_text(p.added, "element", "elements"),
                lower_first(&to)
            ),
        };
    }
    if p.added == 0 {
        return "Those elements are already on this sheet.".to_string();
    }
    format!(
        "Pasted {} onto {}.",
        count_text(p.added, "element", "elements"),
        lower_first(&to)
    )
}

fn lower_first(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::{Minutes, SubBoardId};
    use method_schedule_core::Eighths;
    use std::collections::HashMap;

    /// Main holds scenes 1 to 4 with a banner after 1 and a day break after 2; scene 5 is
    /// in the Boneyard; a second board, Alt, holds every scene in its Boneyard. Scenes 1
    /// and 2 have ADA on them.
    fn sample() -> (Session, u32, u32, u32) {
        let mut s = Session::new();
        s.edit(|x| {
            let cast = x.category_named("Cast").unwrap();
            let ada = x.add_element(cast, "ADA").unwrap();
            for n in 1..=5 {
                let id = x.add_sheet(Sheet {
                    scenes: n.to_string(),
                    set: format!("SET {n}"),
                    pages: Eighths(n),
                    ..Sheet::default()
                });
                if n <= 2 {
                    x.tag(id, ada).unwrap();
                }
            }
            let b = x.boards[0].id;
            let sb = x.boards[0].sub_boards[0].id;
            let ids: Vec<StripId> = x.boards[0].boneyard[..4].iter().map(|st| st.id).collect();
            let at = |index| Place {
                sub_board: Some(sb),
                index,
            };
            x.move_strips(b, &ids, at(0)).unwrap();
            x.insert_strip(
                b,
                StripKind::Banner(Banner {
                    text: "Lunch".into(),
                    duration: Minutes(30),
                    ..Default::default()
                }),
                at(1),
            )
            .unwrap();
            x.insert_strip(b, StripKind::DayBreak, at(3)).unwrap();
            x.add_board("Alt", None);
            Ok(())
        })
        .unwrap();
        let x = s.schedule();
        let (b, sb, alt) = (
            x.boards[0].id.0,
            x.boards[0].sub_boards[0].id.0,
            x.boards[1].id.0,
        );
        (s, b, sb, alt)
    }

    /// A list as text: scene numbers, `[banner]`, `|` for a day break.
    fn list(s: &Schedule, board: u32, sub: Option<u32>) -> Vec<String> {
        let b = s.board(BoardId(board)).unwrap();
        let l = match sub {
            None => &b.boneyard,
            Some(id) => &b.sub_boards.iter().find(|x| x.id.0 == id).unwrap().strips,
        };
        l.iter()
            .map(|st| match &st.kind {
                StripKind::Sheet(id) => s.sheet(*id).unwrap().scenes.clone(),
                StripKind::Banner(bn) => format!("[{}]", bn.text),
                StripKind::DayBreak => "|".to_string(),
            })
            .collect()
    }

    fn strip_ids(s: &Schedule, board: u32, sub: Option<u32>) -> Vec<u32> {
        let b = s.board(BoardId(board)).unwrap();
        let l = match sub {
            None => &b.boneyard,
            Some(id) => &b.sub_boards.iter().find(|x| x.id.0 == id).unwrap().strips,
        };
        l.iter().map(|st| st.id.0).collect()
    }

    /// Every board holds every sheet exactly once (what `check` asks for, said plainly).
    fn every_sheet_once(s: &Schedule) {
        s.check().unwrap();
        for b in &s.boards {
            let mut seen: HashMap<SheetId, usize> = HashMap::new();
            for st in b
                .sub_boards
                .iter()
                .flat_map(|x| &x.strips)
                .chain(&b.boneyard)
            {
                if let StripKind::Sheet(id) = st.kind {
                    *seen.entry(id).or_default() += 1;
                }
            }
            assert_eq!(seen.len(), s.sheets.len());
            assert!(seen.values().all(|n| *n == 1));
        }
    }

    /// Strips copied in one window's schedule paste in another's (decisions.md, entry
    /// 183): the scene's elements matched by category and name ignoring case or added,
    /// its quantity kept, header values in the target's spelling, a banner and a day
    /// break with it, all in one undo step there.
    #[test]
    fn strips_carried_to_another_schedule_paste_as_copies() {
        let (src, b, sb, _) = sample();
        let ids = strip_ids(src.schedule(), b, Some(sb));
        // Scene 1 (ADA), the banner and scene 2, with a quantity and a prop on scene 1.
        let mut src = src;
        src.edit(|x| {
            let props = x.add_category("Props");
            let lamp = x.add_element(props, "Lamp").unwrap();
            let one = x.sheets[0].id;
            x.tag(one, lamp).unwrap();
            x.sheet_mut(one).unwrap().quantities.push(Quantity {
                element: lamp,
                count: 3,
                daily: true,
            });
            x.sheet_mut(one).unwrap().int_ext = "int".into();
            Ok(())
        })
        .unwrap();
        let clip = copy_strips(src.schedule(), b, &ids[..3], true).unwrap();
        let carried = carry(src.schedule(), &clip, "ZTEST One").unwrap();
        assert!(carry(
            src.schedule(),
            &Clip::Elements {
                sheet: SheetId(1),
                cut: false,
                elements: vec![]
            },
            "x"
        )
        .is_none());
        let d = dto(&carried);
        assert_eq!((d.kind, d.count, d.scenes, d.board), ("strips", 3, 2, None));
        assert!(d.strips.is_empty());

        // The target: a cast category named in another case with Ada, a set spelled
        // `set 1`, I/E `INT`, and one scene on its board.
        let mut dst = Session::new();
        dst.edit(|x| {
            let cast = x.cast().unwrap();
            x.category_mut(cast).unwrap().name = "CAST".into();
            let ada = x.add_element(cast, "Ada").unwrap();
            let id = x.add_sheet(Sheet {
                scenes: "9".into(),
                set: "set 1".into(),
                int_ext: "INT".into(),
                ..Sheet::default()
            });
            x.tag(id, ada).unwrap();
            let b = x.boards[0].id;
            let sb = x.boards[0].sub_boards[0].id;
            let st = x.boards[0].boneyard[0].id;
            x.move_strips(
                b,
                &[st],
                Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        let before = dst.schedule().clone();
        let (db, dsb) = (before.boards[0].id.0, before.boards[0].sub_boards[0].id.0);
        let p = dst
            .edit(|x| paste_strips(x, &carried, db, Some(dsb), 1))
            .unwrap();
        assert!(!p.moved && p.from_other_board);
        let x = dst.schedule();
        every_sheet_once(x);
        assert_eq!(list(x, db, Some(dsb)), ["9", "1", "[Lunch]", "2"]);
        assert_eq!(p.strips, strip_ids(x, db, Some(dsb))[1..].to_vec());
        let ada = x.element_named(x.cast().unwrap(), "ADA").unwrap();
        let cast_count = x
            .elements
            .iter()
            .filter(|e| e.category == x.cast().unwrap())
            .count();
        assert_eq!(cast_count, 1, "ADA matched Ada");
        let props = x.category_named("props").expect("Props added");
        let lamp = x.element_named(props, "Lamp").expect("Lamp added");
        let one = x.sheets.iter().find(|h| h.scenes == "1").unwrap();
        assert_eq!(one.elements, [ada, lamp]);
        assert_eq!(
            one.quantities,
            [Quantity {
                element: lamp,
                count: 3,
                daily: true
            }]
        );
        assert_eq!((one.set.as_str(), one.int_ext.as_str()), ("set 1", "INT"));
        let two = x.sheets.iter().find(|h| h.scenes == "2").unwrap();
        assert_eq!(two.set, "SET 2");
        // One undo takes it all away; the source is untouched.
        assert!(dst.undo());
        assert_eq!(dst.schedule(), &before);
        assert_eq!(src.schedule().sheets.len(), 5);
        // Banners never go in the Boneyard there either.
        let e = dst
            .edit(|x| paste_strips(x, &carried, db, None, 0))
            .unwrap_err();
        assert!(e.contains("Boneyard"), "{e}");
    }

    #[test]
    fn copied_strips_paste_as_copies_below_the_selection() {
        let (mut s, b, sb, alt) = sample();
        assert_eq!(
            list(s.schedule(), b, Some(sb)),
            ["1", "[Lunch]", "2", "|", "3", "4"]
        );
        let ids = strip_ids(s.schedule(), b, Some(sb));
        // Scene 1, the banner and the day break, given out of order.
        let clip = copy_strips(s.schedule(), b, &[ids[3], ids[0], ids[1]], false).unwrap();
        let before = s.schedule().clone();
        // Below scene 4, at the end of Main.
        let p = s.edit(|x| paste_strips(x, &clip, b, Some(sb), 6)).unwrap();
        assert!(!p.moved && !p.from_other_board);
        let x = s.schedule();
        every_sheet_once(x);
        assert_eq!(
            list(x, b, Some(sb)),
            ["1", "[Lunch]", "2", "|", "3", "4", "1", "[Lunch]", "|"]
        );
        assert_eq!(p.strips, strip_ids(x, b, Some(sb))[6..].to_vec());
        // The copy of scene 1 is a new sheet with its fields and elements, at the end of
        // Alt's Boneyard.
        assert_eq!(x.sheets.len(), 6);
        let copy = x.sheets.last().unwrap();
        let one = &x.sheets[0];
        assert_ne!(copy.id, one.id);
        assert_eq!((&copy.set, &copy.elements), (&one.set, &one.elements));
        assert_eq!(list(x, alt, None).last().unwrap(), "1");
        // A copy pastes again.
        s.edit(|x| paste_strips(x, &clip, b, Some(sb), 0)).unwrap();
        every_sheet_once(s.schedule());
        assert_eq!(s.schedule().sheets.len(), 7);
        assert!(s.undo() && s.undo());
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn cut_strips_move_on_their_board_and_copy_elsewhere() {
        let (mut s, b, sb, alt) = sample();
        let ids = strip_ids(s.schedule(), b, Some(sb));
        // Cut scene 1 and the banner; paste below scene 3.
        let clip = copy_strips(s.schedule(), b, &[ids[0], ids[1]], true).unwrap();
        let p = s.edit(|x| paste_strips(x, &clip, b, Some(sb), 5)).unwrap();
        assert!(p.moved);
        assert_eq!(p.strips, [ids[0], ids[1]], "the same strips");
        let x = s.schedule();
        every_sheet_once(x);
        assert_eq!(x.sheets.len(), 5, "no sheet copied");
        assert_eq!(list(x, b, Some(sb)), ["2", "|", "3", "1", "[Lunch]", "4"]);

        // Cut on Main, pasted on Alt's Boneyard: scenes only, as copies; Main keeps its own.
        let ids = strip_ids(s.schedule(), b, Some(sb));
        let clip = copy_strips(s.schedule(), b, &[ids[0], ids[2]], true).unwrap();
        let p = s.edit(|x| paste_strips(x, &clip, alt, None, 0)).unwrap();
        assert!(!p.moved && p.from_other_board);
        let x = s.schedule();
        every_sheet_once(x);
        assert_eq!(x.sheets.len(), 7);
        assert_eq!(list(x, alt, None)[..2], ["2", "3"]);
        assert_eq!(list(x, b, Some(sb)), ["2", "|", "3", "1", "[Lunch]", "4"]);
        // The copies sit at the end of Main's Boneyard.
        assert_eq!(list(x, b, None), ["5", "2", "3"]);
    }

    #[test]
    fn banners_never_paste_into_the_boneyard_but_day_breaks_park_there() {
        let (mut s, b, sb, alt) = sample();
        let ids = strip_ids(s.schedule(), b, Some(sb));
        let before = s.schedule().clone();
        for cut in [false, true] {
            let clip = copy_strips(s.schedule(), b, &[ids[0], ids[1]], cut).unwrap();
            for board in [b, alt] {
                let err = s
                    .edit(|x| paste_strips(x, &clip, board, None, 0))
                    .unwrap_err();
                assert!(err.contains("Boneyard"), "{err}");
            }
        }
        assert_eq!(s.schedule(), &before);
        // The day break after scene 2, cut and pasted in the Boneyard: parked (decision 57).
        let clip = copy_strips(s.schedule(), b, &[ids[3]], true).unwrap();
        s.edit(|x| paste_strips(x, &clip, b, None, 0)).unwrap();
        assert_eq!(s.schedule().boards[0].boneyard[0].kind, StripKind::DayBreak);
        assert!(s.undo());
        // Refusals: no strips, a strip twice, one not on the board, a sub-board elsewhere.
        assert!(copy_strips(s.schedule(), b, &[], false).is_err());
        assert!(copy_strips(s.schedule(), b, &[ids[0], ids[0]], false).is_err());
        assert!(copy_strips(s.schedule(), alt, &[ids[0]], false).is_err());
        let clip = copy_strips(s.schedule(), b, &[ids[0]], false).unwrap();
        assert!(s
            .edit(|x| paste_strips(x, &clip, alt, Some(sb), 0))
            .is_err());
        let el = copy_elements(
            s.schedule(),
            s.schedule().sheets[0].id.0,
            &[s.schedule().elements[0].id.0],
            false,
        )
        .unwrap();
        assert!(s.edit(|x| paste_strips(x, &el, b, Some(sb), 0)).is_err());
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn elements_copy_and_cut_between_sheets() {
        let (mut s, _, _, _) = sample();
        let x = s.schedule();
        let (one, two, three) = (x.sheets[0].id.0, x.sheets[1].id.0, x.sheets[2].id.0);
        let ada = x.elements[0].id.0;
        let cast = x.category_named("Cast").unwrap();
        s.edit(|x| {
            let ben = x.add_element(cast, "BEN").unwrap();
            x.tag(SheetId(one), ben).map_err(|e| e.to_string())
        })
        .unwrap();
        let ben = s.schedule().elements[1].id.0;
        // Copy ADA and BEN from sheet 1 (named out of order) onto sheet 3.
        let clip = copy_elements(s.schedule(), one, &[ben, ada], false).unwrap();
        let Clip::Elements { elements, .. } = &clip else {
            panic!()
        };
        assert_eq!(
            elements,
            &[ElementId(ada), ElementId(ben)],
            "the sheet's order"
        );
        let p = s.edit(|x| paste_elements(x, &clip, three)).unwrap();
        assert_eq!(p, PastedElements { added: 2, moved: 0 });
        let tags = |s: &Session, id: u32| s.schedule().sheet(SheetId(id)).unwrap().elements.clone();
        assert_eq!(tags(&s, three), [ElementId(ada), ElementId(ben)]);
        assert_eq!(tags(&s, one).len(), 2, "a copy leaves them");
        // Onto sheet 2, which has ADA already: only BEN is added.
        let p = s.edit(|x| paste_elements(x, &clip, two)).unwrap();
        assert_eq!(p.added, 1);

        // Cut BEN from sheet 1 and paste on sheet 5: a move.
        let five = s.schedule().sheets[4].id.0;
        let clip = copy_elements(s.schedule(), one, &[ben], true).unwrap();
        let before = s.schedule().clone();
        let p = s.edit(|x| paste_elements(x, &clip, five)).unwrap();
        assert_eq!(p, PastedElements { added: 1, moved: 1 });
        assert_eq!(tags(&s, one), [ElementId(ada)]);
        assert_eq!(tags(&s, five), [ElementId(ben)]);
        assert_eq!(
            elements_notice(s.schedule(), &clip, five, &p),
            "Moved 1 element from scene 1 to scene 5."
        );
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");
        // Pasting a cut on its own sheet changes nothing.
        let p = s.edit(|x| paste_elements(x, &clip, one)).unwrap();
        assert_eq!(p, PastedElements { added: 0, moved: 0 });
        assert_eq!(s.schedule(), &before);
        // An element deleted since the copy is skipped; all of them gone is refused.
        s.edit(|x| crate::elements::delete_element(x, ben, false))
            .unwrap();
        assert!(s.edit(|x| paste_elements(x, &clip, five)).is_err());
        // Refusals when copying: not on the sheet, twice, none.
        assert!(copy_elements(s.schedule(), five, &[ada], false).is_err());
        assert!(copy_elements(s.schedule(), one, &[ada, ada], false).is_err());
        assert!(copy_elements(s.schedule(), one, &[], false).is_err());
    }

    /// Random selections of strips, copied or cut, pasted at random places on random
    /// boards: the schedule always passes `check`, every board holds every sheet once, a
    /// move makes no sheet and a copy makes one per scene, and nothing else changes.
    #[test]
    fn property_paste_keeps_every_board_whole() {
        let mut seed: u64 = 0x5eed_c11b;
        let mut next = |n: usize| -> usize {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..300 {
            let (mut s, _, _, _) = sample();
            // A second sub-board on Main, with a few strips.
            s.edit(|x| {
                let b = x.boards[0].id;
                let sub = crate::board::add_sub_board(x, b.0, "Unit B", None)?;
                let id = x.boards[0].boneyard[0].id;
                x.move_strips(
                    b,
                    &[id],
                    Place {
                        sub_board: Some(SubBoardId(sub)),
                        index: 0,
                    },
                )
                .map_err(|e| e.to_string())
            })
            .unwrap();
            for _ in 0..4 {
                let x = s.schedule();
                let board = &x.boards[next(x.boards.len())];
                let all: Vec<u32> = board
                    .sub_boards
                    .iter()
                    .flat_map(|y| &y.strips)
                    .chain(&board.boneyard)
                    .map(|st| st.id.0)
                    .collect();
                let mut picked: Vec<u32> = Vec::new();
                for _ in 0..1 + next(3) {
                    let id = all[next(all.len())];
                    if !picked.contains(&id) {
                        picked.push(id);
                    }
                }
                let cut = next(2) == 0;
                let clip = copy_strips(x, board.id.0, &picked, cut).unwrap();
                let target = &x.boards[next(x.boards.len())];
                let lists: Vec<(Option<u32>, usize)> = target
                    .sub_boards
                    .iter()
                    .map(|y| (Some(y.id.0), y.strips.len()))
                    .chain(std::iter::once((None, target.boneyard.len())))
                    .collect();
                let (sub, len) = lists[next(lists.len())];
                let index = next(len + 1);
                let (sheets, tb) = (x.sheets.len(), target.id.0);
                let scenes = match &clip {
                    Clip::Strips { items, .. } => items
                        .iter()
                        .filter(|i| matches!(i.what, ClipWhat::Sheet(_)))
                        .count(),
                    Clip::Elements { .. } | Clip::Carried { .. } => 0,
                };
                let before = s.schedule().clone();
                match s.edit(|y| paste_strips(y, &clip, tb, sub, index)) {
                    Ok(p) => {
                        let y = s.schedule();
                        every_sheet_once(y);
                        if p.moved {
                            assert_eq!(y.sheets.len(), sheets);
                            assert_eq!(&y.sheets, &before.sheets, "a move changes no sheet");
                        } else {
                            assert_eq!(y.sheets.len(), sheets + scenes);
                            assert_eq!(p.strips.len(), picked.len());
                        }
                        // Other boards change only by copies at their Boneyard's end.
                        for (ob, nb) in before.boards.iter().zip(&y.boards) {
                            if ob.id.0 == tb {
                                continue;
                            }
                            assert_eq!(ob.sub_boards, nb.sub_boards);
                            assert_eq!(nb.boneyard[..ob.boneyard.len()], ob.boneyard[..]);
                        }
                    }
                    Err(e) => {
                        assert!(e.contains("Boneyard"), "{e}");
                        assert!(sub.is_none());
                        assert_eq!(s.schedule(), &before);
                    }
                }
            }
        }
    }
}
