//! The `app` commands shared by the desktop app and the browser host (each desktop
//! command in method-schedule-app's `app.rs` is a one-line call to its function here).

#![allow(unused_imports)]

use super::{
    board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, today, CmdResult,
    DocLock, Shell,
};
use crate::board;
use crate::calendar::{self, EventInput};
use crate::clipboard;
use crate::elements;
use crate::explain::ExplainDto;
use crate::redflags;
use crate::session::{self, Session};
use crate::sheet;
use crate::view::{self, CalendarDto, DoodGridDto, ElementDaysDto, ElementsDto, SheetDto, View};
use method_schedule_core::model::{BoardId, SheetId};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// The `get_view` command.
pub fn get_view<W: Shell + ?Sized, D: DocLock + ?Sized>(window: &W, state: &D) -> CmdResult {
    finish(&window, &state.lock(), None)
}

/// The day out of days grid for a sub-board, for `category` (Cast when it is gone or not
/// given), or for header field `header` (`reports::HeaderField`: set, location, unit,
/// episode, I/E, time of day, script day, sequence).
pub fn dood_grid<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    category: Option<u32>,
    header: Option<String>,
    sort: Option<Vec<String>>,
    no_holds: Option<bool>,
) -> CmdResult<DoodGridDto> {
    // The sort and the without-holds view are the session's (decision 59).
    let opts = view::GridOpts {
        no_holds: no_holds.unwrap_or(false),
        sort: crate::reports::parse_sort(sort.as_deref().unwrap_or(&[]))?,
        ..view::GridOpts::default()
    };
    view::dood_grid_for(
        state.lock().schedule(),
        sub_board,
        category,
        header.as_deref(),
        &opts,
    )
}

/// Why an element's day out of days counts are what they are, on the grid's sub-board
/// (src/explain.rs).
pub fn explain_count<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    element: u32,
) -> CmdResult<ExplainDto> {
    crate::explain::explain_count(state.lock().schedule(), sub_board, element)
}

/// The Conflicts tab for `board` (src/conflicts.rs).
pub fn conflicts_view<D: DocLock + ?Sized>(
    state: &D,
    board: u32,
) -> CmdResult<crate::conflicts::ConflictsDto> {
    crate::conflicts::conflicts_view(state.lock().schedule(), board)
}

/// The `undo` command.
pub fn undo<W: Shell + ?Sized, D: DocLock + ?Sized>(window: &W, state: &D) -> CmdResult {
    let mut s = state.lock();
    s.undo();
    finish(&window, &s, None)
}

/// The `redo` command.
pub fn redo<W: Shell + ?Sized, D: DocLock + ?Sized>(window: &W, state: &D) -> CmdResult {
    let mut s = state.lock();
    s.redo();
    finish(&window, &s, None)
}

/// Moves strips of `board` (a drag, or Option-Up and Down) as one undo step.
pub fn move_strips<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    strips: Vec<u32>,
    sub_board: Option<u32>,
    index: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::move_strips(s, board, &strips, sub_board, index)
    })
}

/// The `insert_day_break` command.
pub fn insert_day_break<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    index: usize,
) -> CmdResult {
    strip_edit(&window, &state, |s| {
        board::insert_day_break(s, sub_board, index)
    })
}

/// Deletes day breaks and banners (the page asks first about banners).
pub fn delete_strips<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    strips: Vec<u32>,
) -> CmdResult {
    edit(&window, &state, |s| board::delete_strips(s, board, &strips))
}

/// Inserts a banner with its text and duration (`h:mm`).
pub fn insert_banner<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    index: usize,
    text: String,
    duration: String,
    style: Option<board::BannerStyleInput>,
) -> CmdResult {
    strip_edit(&window, &state, |s| {
        board::insert_banner_styled(s, sub_board, index, &text, &duration, style.as_ref())
    })
}

/// Changes a banner's text and duration.
pub fn set_banner<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    strip: u32,
    text: String,
    duration: String,
    style: Option<board::BannerStyleInput>,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::set_banner_styled(s, board, strip, &text, &duration, style.as_ref())
    })
}

/// Sorts a sub-board (or the Boneyard, `sub_board` null) by up to four header fields.
pub fn sort_strips<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    sub_board: Option<u32>,
    keys: Vec<board::SortKeyInput>,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::sort_strips(s, board, sub_board, &keys)
    })
}

/// Sets `board`'s strip layout (Board > Strip Layout) as one undo step.
pub fn set_strip_layout<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    columns: Vec<board::LayoutColumnInput>,
    size: Option<String>,
    marks: Option<board::MarksInput>,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::set_strip_layout_with(
            s,
            board,
            &columns,
            size.as_deref().unwrap_or(""),
            marks.unwrap_or_default(),
        )
    })
}

/// Continue Scene on Next Day: a part of the scene below the next day break, as one undo
/// step; the view names the new part, so the board selects it.
pub fn continue_scene<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    strip: u32,
) -> CmdResult {
    strip_edit(&window, &state, |s| board::continue_scene(s, board, strip))
}

/// Sets what `board`'s day breaks show (Board > Day Breaks) as one undo step.
pub fn set_day_look<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    look: board::DayLookInput,
) -> CmdResult {
    edit(&window, &state, |s| board::set_day_look(s, board, &look))
}

/// Sets the schedule's strip colors (Board > Strip Colors) as one undo step.
pub fn set_strip_colors<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    rules: Vec<board::ColorRuleInput>,
    scene_text: Option<String>,
    banner_text: Option<String>,
    break_text: Option<String>,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::set_strip_colors(
            s,
            &rules,
            scene_text.as_deref(),
            banner_text.as_deref(),
            break_text.as_deref(),
        )
    })
}

/// Shows another board. Not an edit: nothing to undo, nothing unsaved.
pub fn show_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
) -> CmdResult {
    let mut s = state.lock();
    s.show_board(method_schedule_core::model::BoardId(board))?;
    finish(&window, &s, None)
}

/// The `add_board` command.
pub fn add_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
) -> CmdResult {
    // A new board's sub-board starts today, as a new sub-board does (behavior note 27;
    // decisions 123 and 153).
    let today = today();
    board_edit(&window, &state, |s| board::add_board(s, &name, today))
}

/// The `duplicate_board` command.
pub fn duplicate_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    name: String,
) -> CmdResult {
    board_edit(&window, &state, |s| board::duplicate_board(s, board, &name))
}

/// The `rename_board` command.
pub fn rename_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| board::rename_board(s, board, &name))
}

/// Moves a board to place `to` among the boards.
pub fn move_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| board::move_board(s, board, to))
}

/// Deletes a board (the page has asked first); the first board shows when it was shown.
pub fn delete_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
) -> CmdResult {
    edit(&window, &state, |s| board::delete_board(s, board))
}

/// Adds a sub-board to `board`; the view names it, so the page shows it.
pub fn add_sub_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    name: String,
) -> CmdResult {
    let mut s = state.lock();
    // A new sub-board starts today (behavior note 24; decision 123).
    let today = today();
    let id = s.edit(|x| board::add_sub_board(x, board, &name, today))?;
    let mut v = finish(&window, &s, None)?;
    v.sub_board = Some(id);
    Ok(v)
}

/// The `rename_sub_board` command.
pub fn rename_sub_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::rename_sub_board(s, sub_board, &name)
    })
}

/// Moves a sub-board to place `to` on its board.
pub fn move_sub_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| board::move_sub_board(s, sub_board, to))
}

/// Deletes a sub-board (the page has asked first): its scenes go to the Boneyard.
pub fn delete_sub_board<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
) -> CmdResult {
    edit(&window, &state, |s| board::delete_sub_board(s, sub_board))
}

/// Automatic day breaks on a sub-board, `by` `pages`, `time` or `screen_time`, every
/// `limit` (as typed). The status line says how many shoot days that made.
pub fn auto_day_breaks<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    by: String,
    limit: String,
) -> CmdResult {
    let mut s = state.lock();
    let n = s.edit(|x| board::auto_day_breaks_by(x, sub_board, &by, &limit))?;
    let days = if n == 1 {
        "1 shoot day".to_string()
    } else {
        format!("{n} shoot days")
    };
    finish(&window, &s, Some(format!("Day breaks set: {days}.")))
}

/// Remove All Day Breaks on a sub-board, as one undo step.
pub fn remove_day_breaks<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
) -> CmdResult {
    let mut s = state.lock();
    let n = s.edit(|x| board::remove_day_breaks(x, sub_board))?;
    let what = if n == 1 {
        "1 day break".to_string()
    } else {
        format!("{n} day breaks")
    };
    finish(
        &window,
        &s,
        Some(format!("Removed {what}. Undo puts them back.")),
    )
}

/// The `set_start_date` command.
pub fn set_start_date<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    date: Option<String>,
) -> CmdResult {
    edit(&window, &state, |s| {
        board::set_start_date(s, sub_board, date.as_deref())
    })
}

/// The Breakdown tab's sheet: `sheet` while it exists, else the one now at `index` in
/// `board`'s order. `None` when the schedule has no sheets.
pub fn get_sheet<D: DocLock + ?Sized>(
    state: &D,
    board: u32,
    sheet: Option<u32>,
    index: Option<usize>,
    order: Option<String>,
) -> CmdResult<Option<SheetDto>> {
    // Previous and Next by board order, sheet number or scene number (decisions.md,
    // entry 81).
    let by = sheet_order(order.as_deref())?;
    view::sheet_view_in(state.lock().schedule(), board, sheet, index, by)
}

/// Sets one header field (src/sheet.rs reads times and text strictly).
pub fn set_sheet_field<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    field: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        sheet::set_field(s, sheet, &field, &value)
    })
}

/// Sets the page length from whole pages and eighths.
pub fn set_sheet_pages<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    pages: String,
    eighths: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        sheet::set_pages(s, sheet, &pages, &eighths)
    })
}

/// Puts an existing element on a sheet.
pub fn tag_element<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    element: u32,
) -> CmdResult {
    edit(&window, &state, |s| sheet::tag_element(s, sheet, element))
}

/// Sets how many of an element a sheet needs and whether it counts daily.
pub fn set_quantity<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    element: u32,
    count: String,
    daily: bool,
) -> CmdResult {
    edit(&window, &state, |s| {
        sheet::set_quantity(s, sheet, element, &count, daily)
    })
}

/// Puts the element `name` of `category` on a sheet, adding it (and the category) when
/// new.
pub fn create_element<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    category: String,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        sheet::create_element(s, sheet, &category, &name)
    })
}

/// Takes an element off a sheet.
pub fn untag_element<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
    element: u32,
) -> CmdResult {
    edit(&window, &state, |s| sheet::untag_element(s, sheet, element))
}

/// Adds an empty sheet after sheet `after` on its sub-board of `board`, or at the end of
/// the Boneyard. The view names the new sheet.
pub fn new_sheet<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    after: Option<u32>,
) -> CmdResult {
    sheet_edit(&window, &state, |s| {
        sheet::new_sheet(s, board, after).map(Some)
    })
}

/// Deletes a sheet (the page has asked first). The view names the sheet to show next in
/// `board`'s order as the tab steps (`order`, as for `get_sheet`).
pub fn delete_sheet<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    sheet: u32,
    order: Option<String>,
) -> CmdResult {
    let by = sheet_order(order.as_deref())?;
    sheet_edit(&window, &state, |s| {
        sheet::delete_sheet(s, board, sheet, by)
    })
}

/// What Board > Merge Strips would do with `sheets` on `board`: the engine's report on a
/// copy, nothing changed.
pub fn merge_sheets_preview<D: DocLock + ?Sized>(
    state: &D,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult<sheet::SheetMergeDto> {
    sheet::merge_preview(state.lock().schedule(), board, &sheets)
}

/// Merges `sheets`, in the order they were selected, into the first of them as one undo
/// step. The view selects
/// the surviving strip and names its sheet.
pub fn merge_sheets<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult {
    let mut s = state.lock();
    let survivor = s.edit(|x| sheet::merge_sheets(x, board, &sheets))?;
    let shown = BoardId(board);
    let title = s
        .schedule()
        .sheet(SheetId(survivor))
        .map(sheet::sheet_title)
        .unwrap_or_default();
    let n = sheets.len();
    let mut v = finish(
        &window,
        &s,
        Some(format!("Merged {n} strips into one sheet: {title}.")),
    )?;
    v.sheet = Some(survivor);
    v.strip = sheet::strip_of(s.schedule(), shown, survivor);
    Ok(v)
}

/// Duplicates `sheets` as one undo step (Board > Duplicate Strip, Breakdown > Duplicate
/// Sheet). The view selects the copies' strips on `board` and names the last copy.
pub fn duplicate_sheets<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    sheets: Vec<u32>,
) -> CmdResult {
    let mut s = state.lock();
    let copies = s.edit(|x| sheet::duplicate_sheets(x, board, &sheets))?;
    let notice = match copies.len() {
        1 => "Duplicated the scene. The copy is selected.".to_string(),
        n => format!("Duplicated {n} scenes. The copies are selected."),
    };
    let mut v = finish(&window, &s, Some(notice))?;
    v.strips = copies
        .iter()
        .filter_map(|c| sheet::strip_of(s.schedule(), BoardId(board), *c))
        .collect();
    v.sheet = copies.last().copied();
    Ok(v)
}

/// Pastes the clipboard's strips on `board` at `index` of `sub_board` (null: the
/// Boneyard), as one undo step. The view selects what was pasted.
pub fn paste_strips<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    board: u32,
    sub_board: Option<u32>,
    index: usize,
) -> CmdResult {
    let mut s = state.lock();
    let clip = s
        .clipboard()
        .cloned()
        .ok_or("Nothing has been cut or copied yet.")?;
    let from = match &clip {
        clipboard::Clip::Strips { board, .. } => s
            .schedule()
            .board(*board)
            .map(|b| b.name.clone())
            .unwrap_or_default(),
        clipboard::Clip::Carried { from, .. } => from.clone(),
        clipboard::Clip::Elements { .. } => String::new(),
    };
    let carried = matches!(clip, clipboard::Clip::Carried { .. });
    let pasted = s.edit(|x| clipboard::paste_strips(x, &clip, board, sub_board, index))?;
    let n = clipboard::count_text(pasted.strips.len(), "strip", "strips");
    let notice = if pasted.moved {
        format!("Moved {n}.")
    } else if carried && pasted.from_other_board {
        format!(
            "Pasted copies of {n} from {from}. A cut moves strips only within its own schedule, so the originals stay there."
        )
    } else if carried {
        format!("Pasted {n} from {from}.")
    } else if pasted.from_other_board {
        format!(
            "Pasted copies of {n}. A cut moves strips only on its own board, so the originals stay on {from}."
        )
    } else {
        format!("Pasted {n}.")
    };
    if matches!(
        clip,
        clipboard::Clip::Strips { cut: true, .. } | clipboard::Clip::Carried { cut: true, .. }
    ) {
        s.set_clipboard(None);
    }
    let mut v = finish(&window, &s, Some(notice))?;
    v.strips = pasted.strips;
    Ok(v)
}

/// Pastes the clipboard's elements on sheet `sheet` as one undo step.
pub fn paste_elements<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sheet: u32,
) -> CmdResult {
    let mut s = state.lock();
    let clip = s
        .clipboard()
        .cloned()
        .ok_or("Nothing has been cut or copied yet.")?;
    let pasted = s.edit(|x| clipboard::paste_elements(x, &clip, sheet))?;
    let notice = clipboard::elements_notice(s.schedule(), &clip, sheet, &pasted);
    if matches!(clip, clipboard::Clip::Elements { cut: true, .. })
        && pasted.moved + pasted.added > 0
    {
        s.set_clipboard(None);
    }
    finish(&window, &s, Some(notice))
}

/// The Elements tab: every category, and `category`'s elements (the first category's
/// when it is gone or not given), or header field `header`'s values, with work days and
/// dates from `sub_board`.
pub fn elements_view<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    category: Option<u32>,
    header: Option<String>,
) -> CmdResult<ElementsDto> {
    view::elements_view(
        state.lock().schedule(),
        sub_board,
        category,
        header.as_deref(),
    )
}

/// Adds a category. The view names it, so the tab shows it.
pub fn add_category<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
) -> CmdResult {
    let mut s = state.lock();
    let id = s.edit(|x| elements::add_category(x, &name))?;
    let mut v = finish(&window, &s, None)?;
    v.category = Some(id);
    Ok(v)
}

/// Sets a category's name, automatic board IDs or default hold and drop rules.
pub fn set_category_field<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    field: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::set_category_field(s, category, &field, &value)
    })
}

/// Moves a user category to place `to` among the user categories.
pub fn move_category<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::move_category(s, category, to)
    })
}

/// Deletes an empty user category.
pub fn delete_category<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
) -> CmdResult {
    edit(&window, &state, |s| elements::delete_category(s, category))
}

/// Gives every element of a category its default hold and drop rules (the page asks
/// first), as one undo step.
pub fn apply_category_defaults<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
) -> CmdResult {
    let mut s = state.lock();
    let n = s.edit(|x| elements::apply_category_defaults(x, category))?;
    let notice = match n {
        0 => "Every element already had these rules.".to_string(),
        1 => "Gave 1 element the category's rules.".to_string(),
        n => format!("Gave {n} elements the category's rules."),
    };
    finish(&window, &s, Some(notice))
}

/// Sets an element's name, board ID, lock, or hold and drop rules.
pub fn set_element_field<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    field: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::set_element_field(s, element, &field, &value)
    })
}

/// One element's days and dates on `sub_board`: every date of its day out of days row.
pub fn element_days<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    element: u32,
) -> CmdResult<ElementDaysDto> {
    view::element_days(state.lock().schedule(), sub_board, element)
}

/// Adds a custom property to a category.
pub fn add_property<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::add_property(s, category, &name)
    })
}

/// Renames property `at` of a category.
pub fn rename_property<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    at: usize,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::rename_property(s, category, at, &name)
    })
}

/// Moves property `at` of a category to place `to`.
pub fn move_property<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    at: usize,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::move_property(s, category, at, to)
    })
}

/// Deletes property `at` of a category and its values (the page asks first).
pub fn delete_property<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    at: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::delete_property(s, category, at)
    })
}

/// Whether property `at` of a category takes several lines (decision 119).
pub fn set_property_multi_line<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    at: usize,
    multi_line: bool,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::set_property_multi_line(s, category, at, multi_line)
    })
}

/// Sets an element's value of one of its category's properties.
pub fn set_property_value<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    name: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::set_property_value(s, element, &name, &value)
    })
}

/// Sets the elements `element` links to; with `retroactive`, sheets that already hold it
/// get them too.
pub fn set_links<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    links: Vec<u32>,
    retroactive: bool,
) -> CmdResult {
    let mut s = state.lock();
    let n = s.edit(|x| elements::set_links(x, element, &links, retroactive))?;
    let notice = match (retroactive, n) {
        (false, _) => None,
        (true, 0) => Some("Every sheet with it already had its linked elements.".to_string()),
        (true, 1) => Some("Added its linked elements to 1 sheet.".to_string()),
        (true, n) => Some(format!("Added its linked elements to {n} sheets.")),
    };
    finish(&window, &s, notice)
}

/// Takes image `at` off an element; the file stays where it is.
pub fn remove_element_image<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    at: usize,
) -> CmdResult {
    edit(&window, &state, |s| elements::remove_image(s, element, at))
}

/// Merges `others` into `survivor` (the page asks which survives).
pub fn merge_elements<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    survivor: u32,
    others: Vec<u32>,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::merge_elements(s, survivor, &others)
    })
}

/// Moves an element to another category.
pub fn move_element<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    category: u32,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::move_element(s, element, category)
    })
}

/// Renumbers a category's unlocked elements from `start`, in the tab's order: `order`
/// when its table is sorted by a column.
pub fn renumber_elements<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    category: u32,
    start: String,
    order: Option<Vec<u32>>,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::renumber(s, category, &start, order.as_deref())
    })
}

/// Deletes an element (the page has asked first; `unlink` after a second question when
/// it is linked).
pub fn delete_element<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    unlink: bool,
) -> CmdResult {
    edit(&window, &state, |s| {
        elements::delete_element(s, element, unlink)
    })
}

/// The Red flags tab: every clash on `sub_board`, in date order.
pub fn red_flags_view<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
) -> CmdResult<redflags::RedFlagsDto> {
    redflags::red_flags_view(state.lock().schedule(), sub_board)
}

/// Adds a red flag to `element` (`flag.id` null) or changes one of its flags.
pub fn set_red_flag<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    element: u32,
    flag: redflags::RedFlagInput,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::set_red_flag(s, element, &flag)
    })
}

/// Deletes a red flag (the page has asked first).
pub fn delete_red_flag<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    flag: u32,
) -> CmdResult {
    edit(&window, &state, |s| redflags::delete_red_flag(s, flag))
}

/// Adds a red flag type at the end of the list.
pub fn add_red_flag_type<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| redflags::add_red_flag_type(s, &name))
}

/// Renames a red flag type.
pub fn rename_red_flag_type<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    red_flag_type: u32,
    name: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::rename_red_flag_type(s, red_flag_type, &name)
    })
}

/// Moves a red flag type to place `to` in the list.
pub fn move_red_flag_type<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    red_flag_type: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::move_red_flag_type(s, red_flag_type, to)
    })
}

/// Deletes a red flag type no flag uses.
pub fn delete_red_flag_type<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    red_flag_type: u32,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::delete_red_flag_type(s, red_flag_type)
    })
}

/// Red Flag Types > Add the Standard Types: the built-in types an older schedule lacks.
pub fn add_standard_red_flag_types<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::add_standard_types(s).map(|_| ())
    })
}

/// The Red flags tab's Add: flags for several elements and dates, one undo step.
pub fn add_red_flags<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    flags: redflags::AddFlagsInput,
) -> CmdResult {
    edit(&window, &state, |s| {
        redflags::add_red_flags(s, &flags).map(|_| ())
    })
}

/// The Red flags tab's Delete for the checked rows (the page has asked first).
pub fn delete_red_flags<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    flags: Vec<u32>,
) -> CmdResult {
    edit(&window, &state, |s| redflags::delete_red_flags(s, &flags))
}

/// One month of a sub-board's calendar (`year` and `month` both, or neither for the start
/// date's month).
pub fn calendar_view<D: DocLock + ?Sized>(
    state: &D,
    sub_board: u32,
    year: Option<i32>,
    month: Option<u32>,
    week_start: Option<String>,
    fields: Option<Vec<String>>,
    span: Option<bool>,
) -> CmdResult<CalendarDto> {
    // The user's week start (decision 59); the strip fields are the schedule's
    // (decision 102), or, while it has none, the user's older per-user pick when it was
    // ever changed from that preference's default.
    let doc = state.lock();
    let sch = doc.schedule();
    let mut opts = view::CalOpts {
        monday: week_start.as_deref() == Some("mon"),
        fields: sch.calendar_strip_fields(),
        span: span.unwrap_or(false),
    };
    if let (true, Some(f)) = (sch.calendar_fields.is_empty(), fields) {
        let old: Vec<_> = f
            .iter()
            .take(13)
            .filter_map(|x| {
                method_schedule_core::model::StripField::from_name(x).filter(|f| f.of_sheet())
            })
            .collect();
        if crate::reportsetup::old_calendar_pick(&old) {
            opts.fields = old;
        }
    }
    view::calendar_view_with(sch, sub_board, year, month, &opts)
}

/// Makes a date of a sub-board's calendar working, a day off, a holiday or a company
/// travel day.
pub fn set_day_kind<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    date: String,
    kind: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        calendar::set_day_kind(s, sub_board, &date, &kind)
    })
}

/// Sets a sub-board's weekly days off (`mon` to `sun`).
pub fn set_weekly_off<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    days: Vec<String>,
) -> CmdResult {
    edit(&window, &state, |s| {
        calendar::set_weekly_off(s, sub_board, &days)
    })
}

/// Adds an event to a sub-board's calendar (`index` null) or replaces event `index`.
pub fn save_event<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    index: Option<usize>,
    event: EventInput,
) -> CmdResult {
    edit(&window, &state, |s| {
        calendar::save_event(s, sub_board, index, &event)
    })
}

/// The Calendar's View menu: what it hides, by key (decision 111), as one undo step.
pub fn set_calendar_hidden<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    hidden: Vec<String>,
) -> CmdResult {
    edit(&window, &state, |s| {
        calendar::set_calendar_hidden(s, &hidden)
    })
}

/// Deletes event `index` of a sub-board's calendar (the page has asked first).
pub fn delete_event<W: Shell + ?Sized, D: DocLock + ?Sized>(
    window: &W,
    state: &D,
    sub_board: u32,
    index: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        calendar::delete_event(s, sub_board, index)
    })
}
