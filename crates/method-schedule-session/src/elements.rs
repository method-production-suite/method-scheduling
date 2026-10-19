//! The Elements tab's edits, as plain functions on a [`Schedule`]: categories (add,
//! rename, reorder, delete, settings, custom properties, apply defaults to existing
//! elements) and elements (rename, board ID and its lock, hold and drop rules, notes,
//! property values, links, images, merge, move, renumber, delete).
//! Each validates what the page sent and turns the engine's refusals into one plain
//! sentence; `Session::edit` makes it one undo step. Integers only (decisions.md, entry 3).

use std::path::{Component, Path, PathBuf, Prefix};

use method_schedule_core::model::{CategoryId, ElementId, Error};
use method_schedule_core::reports::element_list;
use method_schedule_core::Schedule;

use crate::board::{first_board, EditResult};
use crate::sheet::{MAX_NAME, MAX_TEXT};

pub use method_schedule_core::model::MAX_BOARD_ID;
/// The longest minimum drop gap, in days.
pub const MAX_DROP_GAP: u32 = 999;
/// The highest number a renumbering may start from.
pub const MAX_START: u32 = 99_999;

/// The fields [`set_category_field`] takes.
pub const CATEGORY_FIELDS: &[&str] = &[
    "name",
    "auto_board_id",
    "exclude_from_board",
    "daily_quantity",
    "holds",
    "drops",
    "min_drop_gap",
];
/// The fields [`set_element_field`] takes.
pub const ELEMENT_FIELDS: &[&str] = &[
    "name",
    "board_id",
    "locked",
    "exclude_from_board",
    "holds",
    "drops",
    "min_drop_gap",
    "notes",
];
/// The longest property value, in characters.
pub const MAX_VALUE: usize = 1_000;
/// The most images one element holds.
pub const MAX_IMAGES: usize = 20;
/// The largest image file shown as a thumbnail, in bytes.
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// The image files an element takes, by extension, with their media types.
pub const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("heic", "image/heic"),
    ("bmp", "image/bmp"),
];

/// The engine's refusals, as the page says them.
pub fn say(e: Error) -> String {
    match e {
        Error::NoSuchCategory(_) => "That category is no longer in the schedule.".into(),
        Error::NoSuchElement(_) => "That element is no longer in the schedule.".into(),
        Error::DuplicateName(n) => format!("{n} is already in this category."),
        Error::DuplicateBoardId(b) => format!("Board ID {b} is already used in this category."),
        Error::DuplicateCategory(n) => format!("There is already a category called {n}."),
        Error::EmptyName => "Type a name.".into(),
        Error::BuiltIn(n) => format!("{n} is built in, so it can't be moved or deleted."),
        Error::NotEmpty(n) => format!("{n} still has elements. Move or delete them first."),
        Error::Linked(n) => format!("{n} is linked to other elements. Unlink it to delete it."),
        other => {
            let t = other.to_string();
            let mut c = t.chars();
            match c.next() {
                Some(f) => format!("{}{}.", f.to_uppercase(), c.as_str()),
                None => "That change was refused.".into(),
            }
        }
    }
}

fn name_text(what: &str, value: &str) -> EditResult<String> {
    let t = value.trim();
    if t.is_empty() {
        return Err(format!("Type a name for the {what}."));
    }
    if t.chars().count() > MAX_NAME {
        return Err(format!("A name is at most {MAX_NAME} characters."));
    }
    Ok(t.to_string())
}

fn switch(value: &str) -> EditResult<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("That switch takes true or false.".into()),
    }
}

/// A minimum drop gap typed as whole days, 1 to [`MAX_DROP_GAP`].
pub fn parse_drop_gap(value: &str) -> EditResult<u32> {
    let t = value.trim();
    let n = (!t.is_empty() && t.len() <= 3 && t.bytes().all(|b| b.is_ascii_digit()))
        .then(|| t.parse::<u32>().ok())
        .flatten()
        .filter(|n| (1..=MAX_DROP_GAP).contains(n));
    n.ok_or_else(|| {
        format!("The minimum drop gap is a whole number of days from 1 to {MAX_DROP_GAP}.")
    })
}

/// A board ID as typed: letters and digits only, at most [`MAX_BOARD_ID`] of them, case
/// kept. Empty clears it.
pub fn parse_board_id(value: &str) -> EditResult<Option<String>> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if !method_schedule_core::model::board_id_ok(t) {
        return Err(format!(
            "A board ID is letters and digits, at most {MAX_BOARD_ID}, like 12 or 12A."
        ));
    }
    Ok(Some(t.to_string()))
}

fn category_id(s: &Schedule, id: u32) -> EditResult<CategoryId> {
    s.category(CategoryId(id))
        .map(|c| c.id)
        .ok_or_else(|| say(Error::NoSuchCategory(CategoryId(id))))
}

fn element_id(s: &Schedule, id: u32) -> EditResult<ElementId> {
    s.element(ElementId(id))
        .map(|e| e.id)
        .ok_or_else(|| say(Error::NoSuchElement(ElementId(id))))
}

// ---------------------------------------------------------------------------
// Categories

/// Adds a category at the end. Returns its id.
pub fn add_category(s: &mut Schedule, name: &str) -> EditResult<u32> {
    let name = name_text("category", name)?;
    if s.category_named(&name).is_some() {
        return Err(say(Error::DuplicateCategory(name)));
    }
    Ok(s.add_category(&name).0)
}

/// Sets one of a category's settings: its name, automatic board IDs, or the default hold
/// and drop rules new elements get. Switches are `true` or `false`.
pub fn set_category_field(s: &mut Schedule, category: u32, field: &str, value: &str) -> EditResult {
    let id = category_id(s, category)?;
    if field == "name" {
        let name = name_text("category", value)?;
        return s.rename_category(id, &name).map_err(say);
    }
    let c = s.category_mut(id).expect("checked above");
    match field {
        "auto_board_id" => c.auto_board_id = switch(value)?,
        "exclude_from_board" => c.exclude_from_board = switch(value)?,
        "daily_quantity" => c.daily_quantity = switch(value)?,
        "holds" => c.defaults.holds = switch(value)?,
        "drops" => c.defaults.drops = switch(value)?,
        "min_drop_gap" => c.defaults.min_drop_gap = parse_drop_gap(value)?,
        _ => return Err("That is not a category setting.".into()),
    }
    Ok(())
}

/// Moves a user category to place `to` among the user categories (built-ins stay first).
pub fn move_category(s: &mut Schedule, category: u32, to: usize) -> EditResult {
    let id = category_id(s, category)?;
    s.move_category(id, to).map_err(say)
}

/// Deletes a user category with no elements.
pub fn delete_category(s: &mut Schedule, category: u32) -> EditResult {
    let id = category_id(s, category)?;
    s.delete_category(id).map_err(say)
}

/// Gives every element of a category the category's default hold and drop rules and its
/// off the board setting (decision 120). Returns how many changed.
pub fn apply_category_defaults(s: &mut Schedule, category: u32) -> EditResult<usize> {
    let id = category_id(s, category)?;
    s.apply_defaults(id).map_err(say)
}

// ---------------------------------------------------------------------------
// Elements

/// Sets one of an element's fields: its name (unique in the category, ignoring case), its
/// board ID (unique in the category; empty clears it), the board ID's lock, and its hold
/// and drop rules. Switches are `true` or `false`.
pub fn set_element_field(s: &mut Schedule, element: u32, field: &str, value: &str) -> EditResult {
    let id = element_id(s, element)?;
    match field {
        "name" => {
            let name = name_text("element", value)?;
            s.rename_element(id, &name).map_err(say)
        }
        "board_id" => {
            let b = parse_board_id(value)?;
            s.set_board_id(id, b.as_deref()).map_err(say)
        }
        _ => {
            let e = s.element_mut(id).expect("checked above");
            match field {
                "locked" => e.board_id_locked = switch(value)?,
                "exclude_from_board" => e.exclude_from_board = switch(value)?,
                "holds" => e.rules.holds = switch(value)?,
                "drops" => e.rules.drops = switch(value)?,
                "min_drop_gap" => e.rules.min_drop_gap = parse_drop_gap(value)?,
                "notes" => {
                    if value.chars().count() > MAX_TEXT {
                        return Err(format!("Notes are at most {MAX_TEXT} characters."));
                    }
                    e.notes = value.trim_end().to_string();
                }
                _ => return Err("That is not an element field.".into()),
            }
            Ok(())
        }
    }
}

/// Merges `others` into `survivor`: sheets and events that listed any of them list the
/// survivor once.
pub fn merge_elements(s: &mut Schedule, survivor: u32, others: &[u32]) -> EditResult {
    let keep = element_id(s, survivor)?;
    let mut gone = Vec::new();
    for &o in others {
        let id = element_id(s, o)?;
        if id != keep && !gone.contains(&id) {
            gone.push(id);
        }
    }
    if gone.is_empty() {
        return Err("Choose at least two elements to merge.".into());
    }
    let cat = s.element(keep).map(|e| e.category);
    if gone
        .iter()
        .any(|g| s.element(*g).map(|e| e.category) != cat)
    {
        return Err("Only elements of one category can be merged.".into());
    }
    s.merge_elements(keep, &gone).map_err(say)
}

/// Moves an element to another category.
pub fn move_element(s: &mut Schedule, element: u32, category: u32) -> EditResult {
    let id = element_id(s, element)?;
    let to = category_id(s, category)?;
    s.move_element(id, to).map_err(say)
}

/// Renumbers a category's unlocked elements from `start` (typed as a whole number), in
/// the order the Elements tab shows them: `order` when the table is sorted by a column
/// (every element of the category once), else its usual order (board ID, then name).
pub fn renumber(s: &mut Schedule, category: u32, start: &str, order: Option<&[u32]>) -> EditResult {
    let id = category_id(s, category)?;
    let t = start.trim();
    let n = (!t.is_empty() && t.len() <= 5 && t.bytes().all(|b| b.is_ascii_digit()))
        .then(|| t.parse::<u32>().ok())
        .flatten()
        .filter(|n| (1..=MAX_START).contains(n))
        .ok_or_else(|| format!("Start from a whole number from 1 to {MAX_START}."))?;
    let order = match order {
        Some(o) => o.iter().map(|&e| ElementId(e)).collect(),
        None => list_order(s, id)?,
    };
    s.renumber(id, n, &order).map_err(|e| match e {
        Error::Broken(_) => "The list changed while it was open. Try again.".to_string(),
        other => say(other),
    })
}

/// A category's elements in the order the Elements tab lists them. The order is board ID,
/// then name, whichever sub-board the counts come from, so the first one serves.
pub fn list_order(s: &Schedule, category: CategoryId) -> EditResult<Vec<ElementId>> {
    let board = first_board(s)?;
    let sub = s.boards[0].sub_boards[0].id;
    Ok(element_list(s, board, sub)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|l| l.category == category)
        .map(|l| l.element)
        .collect())
}

// ---------------------------------------------------------------------------
// Custom properties (inventory section 4)

fn property_at(s: &Schedule, category: CategoryId, at: usize) -> EditResult<()> {
    let n = s.category(category).map_or(0, |c| c.properties.len());
    if at < n {
        Ok(())
    } else {
        Err("That property is no longer in the category.".into())
    }
}

fn say_property(e: Error) -> String {
    match e {
        Error::DuplicateName(n) => format!("{n} is already a property of this category."),
        Error::EmptyName => "Type a name for the property.".into(),
        other => say(other),
    }
}

/// Adds a custom property to a category, at the end.
pub fn add_property(s: &mut Schedule, category: u32, name: &str) -> EditResult {
    let id = category_id(s, category)?;
    s.add_property(id, name).map_err(say_property)
}

/// Renames property `at` of a category; values follow.
pub fn rename_property(s: &mut Schedule, category: u32, at: usize, name: &str) -> EditResult {
    let id = category_id(s, category)?;
    property_at(s, id, at)?;
    s.rename_property(id, at, name).map_err(say_property)
}

/// Moves property `at` of a category to place `to`.
pub fn move_property(s: &mut Schedule, category: u32, at: usize, to: usize) -> EditResult {
    let id = category_id(s, category)?;
    property_at(s, id, at)?;
    property_at(s, id, to)?;
    s.move_property(id, at, to).map_err(say_property)
}

/// Deletes property `at` of a category and every value of it.
pub fn delete_property(s: &mut Schedule, category: u32, at: usize) -> EditResult {
    let id = category_id(s, category)?;
    property_at(s, id, at)?;
    s.delete_property(id, at).map_err(say_property)
}

/// Whether property `at` of a category takes several lines (decision 119).
pub fn set_property_multi_line(s: &mut Schedule, category: u32, at: usize, on: bool) -> EditResult {
    let id = category_id(s, category)?;
    property_at(s, id, at)?;
    s.set_property_multi_line(id, at, on).map_err(say_property)
}

/// Sets an element's value of a property of its category; empty clears it. One line,
/// or several for a property of several lines (decision 119).
pub fn set_property_value(s: &mut Schedule, element: u32, name: &str, value: &str) -> EditResult {
    let id = element_id(s, element)?;
    let multi = s.element(id).is_some_and(|e| {
        s.category(e.category).is_some_and(|c| {
            c.multi_line
                .iter()
                .any(|m| m.eq_ignore_ascii_case(name.trim()))
        })
    });
    let bad = |c: char| c.is_control() && !(multi && c == '\n');
    if value.chars().count() > MAX_VALUE || value.chars().any(bad) {
        return Err(if multi {
            format!("A property value is at most {MAX_VALUE} characters.")
        } else {
            format!("A property value is one line of at most {MAX_VALUE} characters.")
        });
    }
    s.set_property_value(id, name, value).map_err(say_property)
}

// ---------------------------------------------------------------------------
// Linked elements (behavior note 04; engine check E-L2)

/// Sets the elements `element` links to; with `retroactive`, every sheet already holding
/// it gets them too. Returns how many sheets gained an element.
pub fn set_links(
    s: &mut Schedule,
    element: u32,
    links: &[u32],
    retroactive: bool,
) -> EditResult<usize> {
    let id = element_id(s, element)?;
    let mut list = Vec::new();
    for &l in links {
        list.push(element_id(s, l)?);
    }
    if list.contains(&id) {
        return Err("An element can't link to itself.".into());
    }
    s.set_links(id, &list, retroactive).map_err(say)
}

// ---------------------------------------------------------------------------
// Images (inventory section 4): paths only, the files stay outside the schedule

/// The media type of an image path, by its extension; `None` for any other file.
pub fn image_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(e, _)| *e == ext).map(|(_, t)| *t)
}

/// How a picked image is stored (docs/FORMAT.md, Images): relative to the schedule
/// file's folder, with `/` between folders, when it is inside that folder; else absolute.
pub fn stored_image_path(doc: Option<&Path>, picked: &Path) -> String {
    if let Some(dir) = doc.and_then(Path::parent) {
        if let Ok(rel) = picked.strip_prefix(dir) {
            let parts: Vec<String> = rel
                .components()
                .filter_map(|c| match c {
                    Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect();
            if !parts.is_empty() && parts.len() == rel.components().count() {
                return parts.join("/");
            }
        }
    }
    picked.to_string_lossy().into_owned()
}

/// Where a stored image path points: as it is when absolute, else from the schedule
/// file's folder. `None` for a relative path in a schedule never saved, and for a
/// network share or a device (`\\server\share`, `\\?\UNC\`, `\\.\`): a schedule from
/// elsewhere may name any path, and reading one of those reaches out over the network.
pub fn resolve_image_path(doc: Option<&Path>, stored: &str) -> Option<PathBuf> {
    let p = Path::new(stored);
    if let Some(Component::Prefix(prefix)) = p.components().next() {
        if matches!(
            prefix.kind(),
            Prefix::UNC(..) | Prefix::VerbatimUNC(..) | Prefix::DeviceNS(..)
        ) {
            return None;
        }
    }
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    doc.and_then(Path::parent).map(|d| d.join(p))
}

/// Adds an image path (from a Rust dialog, never from the page) to an element.
pub fn add_image(s: &mut Schedule, element: u32, stored: String) -> EditResult {
    let id = element_id(s, element)?;
    if image_type(Path::new(&stored)).is_none() {
        return Err("Choose a PNG, JPEG, GIF, WebP, TIFF, HEIC or BMP image.".into());
    }
    let e = s.element_mut(id).expect("checked above");
    if e.images.contains(&stored) {
        return Err("That image is already on this element.".into());
    }
    if e.images.len() >= MAX_IMAGES {
        return Err(format!("An element holds at most {MAX_IMAGES} images."));
    }
    e.images.push(stored);
    Ok(())
}

/// Takes image `at` off an element. The file itself is never touched.
pub fn remove_image(s: &mut Schedule, element: u32, at: usize) -> EditResult {
    let id = element_id(s, element)?;
    let e = s.element_mut(id).expect("checked above");
    if at >= e.images.len() {
        return Err("That image is no longer on this element.".into());
    }
    e.images.remove(at);
    Ok(())
}

/// The stored path of image `at` of an element (read the file with [`read_image`], with
/// no lock held).
pub fn image_of(s: &Schedule, element: u32, at: usize) -> EditResult<String> {
    let id = element_id(s, element)?;
    s.element(id)
        .and_then(|e| e.images.get(at))
        .cloned()
        .ok_or_else(|| "That image is no longer on this element.".to_string())
}

/// The media type of an image by its first bytes (PNG, JPEG, GIF, WebP, TIFF, BMP or
/// HEIC); `None` for anything else, whatever its name says.
pub fn sniff_image(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        Some("image/tiff")
    } else if head.starts_with(b"BM") && head.len() >= 14 {
        Some("image/bmp")
    } else if head.len() >= 12
        && &head[4..8] == b"ftyp"
        && matches!(&head[8..12], b"heic" | b"heix" | b"mif1" | b"msf1")
    {
        Some("image/heic")
    } else {
        None
    }
}

/// `bytes` as a `data:` URL for a thumbnail, or `None` when they are more than
/// [`MAX_IMAGE_BYTES`] or not one of the image types [`sniff_image`] knows. The desktop
/// reads the file (`method-schedule-app`'s `elements::read_image`); a browser host hands
/// in the bytes it keeps.
pub fn image_data_url(bytes: &[u8]) -> Option<String> {
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return None;
    }
    let kind = sniff_image(bytes)?;
    Some(format!("data:{kind};base64,{}", base64(bytes)))
}

/// Standard base64 with padding, for `data:` URLs.
pub fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Deletes an element: it leaves every sheet and event. A linked element is refused
/// unless `unlink` (the page asks first).
pub fn delete_element(s: &mut Schedule, element: u32, unlink: bool) -> EditResult {
    let id = element_id(s, element)?;
    s.delete_element(id, unlink).map_err(say)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{read_script, Session};
    use std::path::Path;

    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        // Cast only, so the category order below is short.
        let mut s = Session::with_schedule(Schedule::new("ZTEST"), None);
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s
    }

    fn el(s: &Session, name: &str) -> u32 {
        s.schedule()
            .elements
            .iter()
            .find(|e| e.name == name)
            .unwrap()
            .id
            .0
    }

    fn cast(s: &Session) -> u32 {
        s.schedule().category_named("Cast").unwrap().0
    }

    #[test]
    fn readers_are_strict() {
        assert_eq!(parse_drop_gap("10"), Ok(10));
        assert_eq!(parse_drop_gap(" 1 "), Ok(1));
        for bad in ["0", "", "1000", "1.5", "-2", "ten", "7 days"] {
            assert!(parse_drop_gap(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_board_id(" 12A "), Ok(Some("12A".into())));
        assert_eq!(parse_board_id(""), Ok(None));
        for bad in ["1 2", "1-2", "é1", "1234567890123"] {
            assert!(parse_board_id(bad).is_err(), "{bad}");
        }
        for e in [
            parse_drop_gap("x").unwrap_err(),
            parse_board_id("?").unwrap_err(),
            say(Error::Linked("ADA".into())),
            say(Error::NotEmpty("Props".into())),
            say(Error::Broken("something odd".into())),
        ] {
            assert!(!e.contains('!') && !e.contains('\u{2014}'), "{e}");
            assert!(e.ends_with('.'), "{e}");
        }
    }

    #[test]
    fn categories_through_the_session() {
        let mut s = harbor();
        let props = s.edit(|x| add_category(x, " Props ")).unwrap();
        assert!(s.edit(|x| add_category(x, "props")).is_err());
        assert!(s.edit(|x| add_category(x, "  ")).is_err());
        let fx = s.edit(|x| add_category(x, "Effects")).unwrap();
        s.edit(|x| set_category_field(x, props, "name", "Hand props"))
            .unwrap();
        s.edit(|x| set_category_field(x, props, "auto_board_id", "true"))
            .unwrap();
        s.edit(|x| set_category_field(x, props, "holds", "true"))
            .unwrap();
        s.edit(|x| set_category_field(x, props, "min_drop_gap", "10"))
            .unwrap();
        let c = s.schedule().category(CategoryId(props)).unwrap().clone();
        assert_eq!(c.name, "Hand props");
        assert!(c.auto_board_id && c.defaults.holds && !c.defaults.drops);
        assert_eq!(c.defaults.min_drop_gap, 10);
        let before = s.schedule().clone();
        for (f, v) in [
            ("holds", "yes"),
            ("min_drop_gap", "0"),
            ("nope", "x"),
            ("name", ""),
        ] {
            assert!(
                s.edit(|x| set_category_field(x, props, f, v)).is_err(),
                "{f}"
            );
        }
        assert_eq!(s.schedule(), &before);
        // The cast category can be renamed (behavior note 17, A10), one undo step.
        let c0 = cast(&s);
        s.edit(|x| set_category_field(x, c0, "name", "People"))
            .unwrap();
        assert_eq!(s.schedule().cast(), Some(CategoryId(c0)));
        assert_eq!(s.schedule().categories[0].name, "People");
        s.undo();
        assert_eq!(s.schedule(), &before);
        // Reordering and deleting, each one undo step.
        s.edit(|x| move_category(x, fx, 0)).unwrap();
        let names: Vec<&str> = s
            .schedule()
            .categories
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, ["Cast", "Effects", "Hand props"]);
        s.edit(|x| delete_category(x, fx)).unwrap();
        s.undo();
        assert!(s.schedule().category(CategoryId(fx)).is_some());
        let err = s.edit(|x| delete_category(x, c0)).unwrap_err();
        assert!(err.contains("built in"), "{err}");
    }

    #[test]
    fn applying_defaults_is_one_step() {
        let mut s = harbor();
        let c = cast(&s);
        s.edit(|x| set_category_field(x, c, "holds", "true"))
            .unwrap();
        assert!(s.schedule().elements.iter().all(|e| !e.rules.holds));
        let n = s.edit(|x| apply_category_defaults(x, c)).unwrap();
        assert_eq!(n, s.schedule().elements.len());
        assert!(s.schedule().elements.iter().all(|e| e.rules.holds));
        s.undo();
        assert!(s.schedule().elements.iter().all(|e| !e.rules.holds));
    }

    #[test]
    fn element_fields_through_the_session() {
        let mut s = harbor();
        let kit = el(&s, "KIT");
        let tove = el(&s, "TOVE");
        s.edit(|x| set_element_field(x, kit, "name", " Kit "))
            .unwrap();
        s.edit(|x| set_element_field(x, kit, "board_id", "2"))
            .unwrap();
        s.edit(|x| set_element_field(x, kit, "locked", "true"))
            .unwrap();
        s.edit(|x| set_element_field(x, kit, "exclude_from_board", "true"))
            .unwrap();
        assert!(s.schedule().excluded(ElementId(kit)));
        s.edit(|x| set_element_field(x, kit, "holds", "true"))
            .unwrap();
        s.edit(|x| set_element_field(x, kit, "drops", "true"))
            .unwrap();
        s.edit(|x| set_element_field(x, kit, "min_drop_gap", "7"))
            .unwrap();
        let e = s.schedule().element(ElementId(kit)).unwrap().clone();
        assert_eq!(e.name, "Kit");
        assert_eq!(e.board_id.as_deref(), Some("2"));
        assert!(e.board_id_locked && e.rules.holds && e.rules.drops);
        assert_eq!(e.rules.min_drop_gap, 7);
        let before = s.schedule().clone();
        let err = s
            .edit(|x| set_element_field(x, tove, "board_id", "2"))
            .unwrap_err();
        assert_eq!(err, "Board ID 2 is already used in this category.");
        let err = s
            .edit(|x| set_element_field(x, tove, "name", "KIT"))
            .unwrap_err();
        assert_eq!(err, "KIT is already in this category.");
        assert!(s
            .edit(|x| set_element_field(x, 999_999, "name", "X"))
            .is_err());
        assert!(s.edit(|x| set_element_field(x, kit, "nope", "1")).is_err());
        assert_eq!(s.schedule(), &before);
        // Clearing the board ID.
        s.edit(|x| set_element_field(x, kit, "board_id", ""))
            .unwrap();
        assert_eq!(s.schedule().element(ElementId(kit)).unwrap().board_id, None);
    }

    #[test]
    fn merge_move_renumber_and_delete() {
        let mut s = harbor();
        let c = cast(&s);
        let (kit, tove, lund) = (el(&s, "KIT"), el(&s, "TOVE"), el(&s, "HARBORMASTER LUND"));
        let sheets_with = |s: &Session, id: u32| {
            s.schedule()
                .sheets
                .iter()
                .filter(|h| h.elements.contains(&ElementId(id)))
                .count()
        };
        let kit_or_tove = s
            .schedule()
            .sheets
            .iter()
            .filter(|h| {
                h.elements.contains(&ElementId(kit)) || h.elements.contains(&ElementId(tove))
            })
            .count();
        assert!(s.edit(|x| merge_elements(x, kit, &[kit])).is_err());
        s.edit(|x| merge_elements(x, kit, &[tove])).unwrap();
        assert_eq!(sheets_with(&s, kit), kit_or_tove);
        assert!(s.schedule().element(ElementId(tove)).is_none());
        // Renumber in list order: no IDs yet, so by name: KIT 1, LUND 2... from 5.
        s.edit(|x| renumber(x, c, "5", None)).unwrap();
        let id = |s: &Session, e: u32| s.schedule().element(ElementId(e)).unwrap().board_id.clone();
        assert_eq!(id(&s, lund).as_deref(), Some("5"));
        assert_eq!(id(&s, kit).as_deref(), Some("6"));
        for bad in ["0", "", "1.5", "100000"] {
            assert!(s.edit(|x| renumber(x, c, bad, None)).is_err(), "{bad}");
        }
        // The table sorted by a column renumbers in the order it shows.
        s.edit(|x| renumber(x, c, "1", Some(&[kit, lund]))).unwrap();
        assert_eq!(id(&s, kit).as_deref(), Some("1"));
        assert_eq!(id(&s, lund).as_deref(), Some("2"));
        // An order that misses one, or names one twice, changes nothing.
        let before = s.schedule().clone();
        for bad in [&[kit][..], &[kit, kit], &[kit, lund, 999_999]] {
            let e = s.edit(|x| renumber(x, c, "7", Some(bad))).unwrap_err();
            assert!(e.contains("changed"), "{e}");
        }
        assert_eq!(s.schedule(), &before);
        // Move to another category, then delete.
        let extras = s.edit(|x| add_category(x, "Extras")).unwrap();
        s.edit(|x| move_element(x, lund, extras)).unwrap();
        assert_eq!(
            s.schedule().element(ElementId(lund)).unwrap().category,
            CategoryId(extras)
        );
        s.edit(|x| delete_element(x, lund, false)).unwrap();
        assert_eq!(sheets_with(&s, lund), 0);
        // A linked element asks to be unlinked first.
        s.edit(|x| {
            x.element_mut(ElementId(kit)).unwrap().links = x
                .elements
                .iter()
                .filter(|e| e.id.0 != kit)
                .map(|e| e.id)
                .take(1)
                .collect();
            Ok(())
        })
        .unwrap();
        if s.schedule().is_linked(ElementId(kit)) {
            let err = s.edit(|x| delete_element(x, kit, false)).unwrap_err();
            assert!(err.contains("Unlink"), "{err}");
            s.edit(|x| delete_element(x, kit, true)).unwrap();
        }
    }

    #[test]
    fn notes_properties_links_and_settings_through_the_session() {
        let mut s = harbor();
        let c = cast(&s);
        let (kit, tove) = (el(&s, "KIT"), el(&s, "TOVE"));
        s.edit(|x| set_element_field(x, kit, "notes", "Left-handed.\nNo boats.\n"))
            .unwrap();
        assert_eq!(
            s.schedule().element(ElementId(kit)).unwrap().notes,
            "Left-handed.\nNo boats."
        );
        for f in ["exclude_from_board", "daily_quantity"] {
            s.edit(|x| set_category_field(x, c, f, "true")).unwrap();
        }
        let cat = s.schedule().category(CategoryId(c)).unwrap().clone();
        assert!(cat.exclude_from_board && cat.daily_quantity);
        s.edit(|x| add_property(x, c, "Agent")).unwrap();
        s.edit(|x| add_property(x, c, "Height")).unwrap();
        let err = s.edit(|x| add_property(x, c, "agent")).unwrap_err();
        assert_eq!(err, "agent is already a property of this category.");
        s.edit(|x| set_property_value(x, kit, "Height", "1.80 m"))
            .unwrap();
        assert!(s
            .edit(|x| set_property_value(x, kit, "Height", "a\nb"))
            .is_err());
        // A property of several lines takes a line break (decision 119).
        s.edit(|x| set_property_multi_line(x, c, 1, true)).unwrap();
        s.edit(|x| set_property_value(x, kit, "Height", "a\nb"))
            .unwrap();
        assert!(s.edit(|x| set_property_multi_line(x, c, 9, true)).is_err());
        s.edit(|x| set_property_value(x, kit, "Height", "1.80 m"))
            .unwrap();
        s.edit(|x| rename_property(x, c, 1, "Height (m)")).unwrap();
        s.edit(|x| move_property(x, c, 1, 0)).unwrap();
        assert!(s.edit(|x| move_property(x, c, 5, 0)).is_err());
        let e = s.schedule().element(ElementId(kit)).unwrap().clone();
        assert_eq!(e.properties[0].name, "Height (m)");
        s.edit(|x| delete_property(x, c, 0)).unwrap();
        assert!(s
            .schedule()
            .element(ElementId(kit))
            .unwrap()
            .properties
            .is_empty());
        // Links: retroactive adds to the sheets that hold KIT.
        assert!(s.edit(|x| set_links(x, kit, &[kit], false)).is_err());
        let n = s.edit(|x| set_links(x, kit, &[tove], true)).unwrap();
        let with_kit = s
            .schedule()
            .sheets
            .iter()
            .filter(|h| h.elements.contains(&ElementId(kit)))
            .count();
        assert!(n <= with_kit);
        assert!(s
            .schedule()
            .sheets
            .iter()
            .filter(|h| h.elements.contains(&ElementId(kit)))
            .all(|h| h.elements.contains(&ElementId(tove))));
        // Tagging KIT on a sheet brings TOVE with it.
        let sheet = s
            .schedule()
            .sheets
            .iter()
            .find(|h| !h.elements.contains(&ElementId(kit)))
            .map(|h| h.id.0);
        if let Some(sheet) = sheet {
            s.edit(|x| {
                x.untag(method_schedule_core::model::SheetId(sheet), ElementId(tove))
                    .map_err(say)
            })
            .unwrap();
            s.edit(|x| crate::sheet::tag_element(x, sheet, kit))
                .unwrap();
            let h = s
                .schedule()
                .sheet(method_schedule_core::model::SheetId(sheet))
                .unwrap()
                .clone();
            assert!(h.elements.contains(&ElementId(tove)));
            // A quantity on it, strictly read.
            s.edit(|x| crate::sheet::set_quantity(x, sheet, kit, "12", false))
                .unwrap();
            for bad in ["0", "x", "1.5", "100000"] {
                assert!(s
                    .edit(|x| crate::sheet::set_quantity(x, sheet, kit, bad, true))
                    .is_err());
            }
            let h = s
                .schedule()
                .sheet(method_schedule_core::model::SheetId(sheet))
                .unwrap()
                .clone();
            assert_eq!(h.quantity(ElementId(kit)), (12, false));
        }
    }
}
