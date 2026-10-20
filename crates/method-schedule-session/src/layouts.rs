//! The layout library in the app (decisions.md, entry 78): named layouts of scene strips,
//! day breaks and shoot day strips, picked per board and by the one-line report, saved
//! and loaded as layout files. The edits are plain functions on a [`Schedule`] (one undo
//! step each through `Session::edit`); the two file commands show their dialogs from
//! Rust, so the page never names a path.

use serde::Deserialize;

use method_schedule_core::layouts::{LayoutBody, LayoutId, LayoutKind};
use method_schedule_core::model::{DayLook, Error};
use method_schedule_core::Schedule;

use crate::board::{DayLookInput, EditResult, LayoutColumnInput};

/// A layout's body as the page sends it: a strip layout's columns, or a day break or
/// shoot day layout's switches and text.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutInput {
    #[serde(default)]
    pub columns: Option<Vec<LayoutColumnInput>>,
    /// A strip layout's text size step (decision 87).
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub look: Option<DayLookInput>,
    /// A strip layout's mark switches (decision 136).
    #[serde(default)]
    pub marks: Option<crate::board::MarksInput>,
}

fn say(e: Error) -> String {
    match e {
        Error::EmptyName => "Type a name for the layout.".into(),
        Error::DuplicateName(n) => format!("There is already a layout named {n:?}."),
        Error::NoSuchBoard(_) => "That board is no longer in the schedule.".into(),
        Error::Broken(why) => {
            let mut s = why;
            if let Some(c) = s.get(..1) {
                s = c.to_uppercase() + &s[1..];
            }
            if !s.ends_with('.') {
                s.push('.');
            }
            s
        }
        other => crate::elements::say(other),
    }
}

fn kind(name: &str) -> EditResult<LayoutKind> {
    LayoutKind::from_name(name).ok_or_else(|| format!("{name:?} is not a layout type."))
}

fn layout_id(s: &Schedule, id: u32) -> EditResult<LayoutId> {
    s.layouts
        .get(LayoutId(id))
        .map(|l| l.id)
        .ok_or_else(|| "That layout is no longer in the schedule.".to_string())
}

/// A named layout's day look: a board's, without the shoot day strip switch (a board's
/// own setting; picking a shoot day layout turns it on).
fn look(input: &DayLookInput) -> EditResult<DayLook> {
    Ok(DayLook {
        shoot_day_strip: false,
        ..crate::board::day_look(input)?
    })
}

/// A body of type `kind` from what the page sent.
fn body(kind: LayoutKind, input: &LayoutInput) -> EditResult<LayoutBody> {
    match kind {
        LayoutKind::Strips => {
            let cols = input
                .columns
                .as_deref()
                .ok_or("Choose the fields the strips show.")?;
            Ok(LayoutBody::Strips(crate::board::strip_layout_with(
                cols,
                input.size.as_deref().unwrap_or(""),
                input.marks.unwrap_or_default(),
            )?))
        }
        LayoutKind::DayBreaks => Ok(LayoutBody::DayBreaks(look(
            input.look.as_ref().ok_or("Choose what day breaks show.")?,
        )?)),
        LayoutKind::ShootDay => Ok(LayoutBody::ShootDay(look(
            input
                .look
                .as_ref()
                .ok_or("Choose what the shoot day strip shows.")?,
        )?)),
    }
}

/// Adds a named layout of type `kind`; with `board`, that board picks it. Returns its id.
pub fn add_named(
    s: &mut Schedule,
    name: &str,
    kind_name: &str,
    input: &LayoutInput,
    board: Option<u32>,
) -> EditResult<u32> {
    let k = kind(kind_name)?;
    let b = body(k, input)?;
    let id = s.add_layout(name, b).map_err(say)?;
    if let Some(board) = board {
        let board = crate::board::board_id(s, board)?;
        s.use_layout(board, k, Some(id)).map_err(say)?;
    }
    Ok(id.0)
}

/// Changes what a named layout lays out (every board that picked it follows).
pub fn save_named(s: &mut Schedule, layout: u32, input: &LayoutInput) -> EditResult {
    let id = layout_id(s, layout)?;
    let k = s.layouts.get(id).expect("layout_id checked it").body.kind();
    let b = body(k, input)?;
    s.set_layout_body(id, b).map_err(say)
}

pub fn rename_named(s: &mut Schedule, layout: u32, name: &str) -> EditResult {
    let id = layout_id(s, layout)?;
    s.rename_layout(id, name).map_err(say)
}

pub fn move_named(s: &mut Schedule, layout: u32, to: usize) -> EditResult {
    let id = layout_id(s, layout)?;
    s.move_layout(id, to).map_err(say)
}

/// Deletes a named layout; boards that picked it go back to their own.
pub fn delete_named(s: &mut Schedule, layout: u32) -> EditResult {
    let id = layout_id(s, layout)?;
    s.delete_layout(id).map_err(say)
}

/// Board `board` picks layout `layout` of type `kind` (none: its own).
pub fn pick_layout(
    s: &mut Schedule,
    board: u32,
    kind_name: &str,
    layout: Option<u32>,
) -> EditResult {
    let board = crate::board::board_id(s, board)?;
    let k = kind(kind_name)?;
    let id = layout.map(|l| layout_id(s, l)).transpose()?;
    s.use_layout(board, k, id).map_err(say)
}

/// The one-line report picks strip layout `layout` (none: the board's).
pub fn pick_one_line(s: &mut Schedule, layout: Option<u32>) -> EditResult {
    let id = layout.map(|l| layout_id(s, l)).transpose()?;
    s.use_one_line_layout(id).map_err(say)
}

/// A layout file's extension.
pub const LAYOUTS_EXTENSION: &str = "mtlayouts";

/// The text of a layout file holding `layouts` (all when empty), and how many.
pub fn layouts_file(s: &Schedule, layouts: &[u32]) -> EditResult<(String, usize)> {
    let picked: Vec<_> = if layouts.is_empty() {
        s.layouts.layouts.clone()
    } else {
        let mut out = Vec::new();
        for &id in layouts {
            out.push(
                s.layouts
                    .get(LayoutId(id))
                    .cloned()
                    .ok_or("That layout is no longer in the schedule.")?,
            );
        }
        out
    };
    if picked.is_empty() {
        return Err(
            "There are no layouts to save. Save a strip layout as a named layout first."
                .to_string(),
        );
    }
    let n = picked.len();
    Ok((method_schedule_format::layouts_to_string(s, &picked), n))
}

/// Adds the layouts of a layout file's text. Returns how many.
pub fn load_layouts(s: &mut Schedule, text: &str) -> EditResult<usize> {
    let read = method_schedule_format::layouts_from_str(s, text)
        .map_err(|e| format!("That isn't a layout file Method Schedule can read: {e}."))?;
    if read.is_empty() {
        return Err("That layout file holds no layouts.".to_string());
    }
    s.import_layouts(read).map(|ids| ids.len()).map_err(say)
}

// ---------------------------------------------------------------------------
// Commands
