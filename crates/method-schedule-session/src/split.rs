//! The split screen's data (decisions.md, entry 80) and images on scenes (entry 77):
//! another board drawn beside the board shown, and a sheet's image paths with their
//! thumbnails, as an element's are (src/elements.rs, decision 62). Paths come from a Rust
//! Open dialog, never from the page; files are read as an element's images are
//! (`elements::read_image`).

use std::path::Path;

use method_schedule_core::model::SheetId;
use method_schedule_core::Schedule;

use crate::board::EditResult;
use crate::elements::{image_type, MAX_IMAGES};

fn sheet_id(s: &Schedule, sheet: u32) -> EditResult<SheetId> {
    s.sheet(SheetId(sheet))
        .map(|h| h.id)
        .ok_or_else(|| "That sheet is no longer in the schedule.".to_string())
}

/// Adds an image path (from a Rust dialog) to a sheet.
pub fn add_image(s: &mut Schedule, sheet: u32, stored: String) -> EditResult {
    let id = sheet_id(s, sheet)?;
    if image_type(Path::new(&stored)).is_none() {
        return Err("Choose a PNG, JPEG, GIF, WebP, TIFF, HEIC or BMP image.".into());
    }
    let h = s.sheet_mut(id).expect("checked above");
    if h.images.contains(&stored) {
        return Err("That image is already on this sheet.".into());
    }
    if h.images.len() >= MAX_IMAGES {
        return Err(format!("A sheet holds at most {MAX_IMAGES} images."));
    }
    h.images.push(stored);
    Ok(())
}

/// Takes image `at` off a sheet. The file itself is never touched.
pub fn remove_image(s: &mut Schedule, sheet: u32, at: usize) -> EditResult {
    let id = sheet_id(s, sheet)?;
    let h = s.sheet_mut(id).expect("checked above");
    if at >= h.images.len() {
        return Err("That image is no longer on this sheet.".into());
    }
    h.images.remove(at);
    Ok(())
}

/// The stored path of image `at` of a sheet (the desktop reads the file with `elements::read_image`, with
/// no lock held).
pub fn image_of(s: &Schedule, sheet: u32, at: usize) -> EditResult<String> {
    let id = sheet_id(s, sheet)?;
    s.sheet(id)
        .and_then(|h| h.images.get(at))
        .cloned()
        .ok_or_else(|| "That image is no longer on this sheet.".to_string())
}
