//! The browser host's dispatch: each desktop command the host runs, its arguments read
//! as the page sends them (snake_case for `rename_all = "snake_case"` commands, camelCase
//! otherwise), then the same function the desktop command calls
//! (`crate::commands`). A new desktop command that needs only the schedule goes here
//! too; a test keeps the two lists in step.

use serde_json::Value;

use super::{DocState, Error};

/// The desktop commands the host runs, in the desktop's source order.
pub const COMMANDS: &[&str] = &[
    "get_view",
    "dood_grid",
    "explain_count",
    "conflicts_view",
    "undo",
    "redo",
    "move_strips",
    "insert_day_break",
    "delete_strips",
    "insert_banner",
    "set_banner",
    "sort_strips",
    "set_strip_layout",
    "continue_scene",
    "set_day_look",
    "set_strip_colors",
    "show_board",
    "add_board",
    "duplicate_board",
    "rename_board",
    "move_board",
    "delete_board",
    "add_sub_board",
    "rename_sub_board",
    "move_sub_board",
    "delete_sub_board",
    "auto_day_breaks",
    "remove_day_breaks",
    "set_start_date",
    "get_sheet",
    "set_sheet_field",
    "set_sheet_pages",
    "tag_element",
    "set_quantity",
    "create_element",
    "untag_element",
    "new_sheet",
    "delete_sheet",
    "merge_sheets_preview",
    "merge_sheets",
    "duplicate_sheets",
    "paste_strips",
    "paste_elements",
    "elements_view",
    "add_category",
    "set_category_field",
    "move_category",
    "delete_category",
    "apply_category_defaults",
    "set_element_field",
    "element_days",
    "add_property",
    "rename_property",
    "move_property",
    "delete_property",
    "set_property_multi_line",
    "set_property_value",
    "set_links",
    "remove_element_image",
    "merge_elements",
    "move_element",
    "renumber_elements",
    "delete_element",
    "red_flags_view",
    "set_red_flag",
    "delete_red_flag",
    "add_red_flag_type",
    "rename_red_flag_type",
    "move_red_flag_type",
    "delete_red_flag_type",
    "add_standard_red_flag_types",
    "add_red_flags",
    "delete_red_flags",
    "calendar_view",
    "set_day_kind",
    "set_weekly_off",
    "save_event",
    "set_calendar_hidden",
    "delete_event",
    "set_sheet_field_order",
    "move_event_to",
    "find_text",
    "replace_text",
    "set_red_flag_type_color",
    "add_layout",
    "save_layout",
    "rename_layout",
    "move_layout",
    "delete_layout",
    "use_layout",
    "use_one_line_layout",
    "timeline_data",
    "push_past_wrap",
    "set_call_time",
    "set_person_call",
    "set_shot",
    "set_long_day",
    "locations_data",
    "set_location",
    "rest_data",
    "set_turnaround",
    "locks_data",
    "lock_schedule",
    "delete_lock",
    "drift_data",
    "pin_strips",
    "suggest_order",
    "apply_suggested_order",
    "call_sheet_data",
    "shoot_day_count",
    "report_data",
    "report_list",
    "save_report",
    "duplicate_report",
    "move_report",
    "delete_report",
    "save_header_layout",
    "delete_header_layout",
    "move_header_layout",
    "set_production",
    "set_dood_codes",
    "set_prep_wrap",
    "set_calendar_fields",
    "board_view",
    "remove_sheet_image",
];

/// The commands a read-only host still runs: they change no schedule.
pub const READS: &[&str] = &[
    "get_view",
    "dood_grid",
    "explain_count",
    "conflicts_view",
    "show_board",
    "get_sheet",
    "merge_sheets_preview",
    "elements_view",
    "element_days",
    "red_flags_view",
    "calendar_view",
    "find_text",
    "timeline_data",
    "locations_data",
    "rest_data",
    "locks_data",
    "drift_data",
    "suggest_order",
    "call_sheet_data",
    "shoot_day_count",
    "report_data",
    "report_list",
    "board_view",
];

/// Runs a desktop command; `None` when it isn't one of [`COMMANDS`].
pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
    if let Some(r) = app_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = breakdown_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = events_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = find_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = flagcolors_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = layouts_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = plan_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = reports_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = reportsetup_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    if let Some(r) = split_cmds::dispatch(command, a, state) {
        return Some(r);
    }
    None
}

#[allow(unused_imports)]
mod app_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::board;
    use crate::calendar::{self, EventInput};
    use crate::clipboard;
    use crate::elements;
    use crate::explain::ExplainDto;
    use crate::redflags;
    use crate::session::{self, Session};
    use crate::sheet;
    use crate::view::{
        self, CalendarDto, DoodGridDto, ElementDaysDto, ElementsDto, SheetDto, View,
    };
    use method_schedule_core::model::{BoardId, SheetId};
    use serde::Deserialize;
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard};

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "get_view" => to_reply(crate::commands::app::get_view(&window, state)),
            "dood_grid" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    category: Option<u32>,
                    #[serde(default)]
                    header: Option<String>,
                    #[serde(default)]
                    sort: Option<Vec<String>>,
                    #[serde(default)]
                    no_holds: Option<bool>,
                }
                let A {
                    sub_board,
                    category,
                    header,
                    sort,
                    no_holds,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::dood_grid(
                    state, sub_board, category, header, sort, no_holds,
                ))
            }
            "explain_count" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    element: u32,
                }
                let A { sub_board, element } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::explain_count(
                    state, sub_board, element,
                ))
            }
            "conflicts_view" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::conflicts_view(state, board))
            }
            "undo" => to_reply(crate::commands::app::undo(&window, state)),
            "redo" => to_reply(crate::commands::app::redo(&window, state)),
            "move_strips" => {
                #[derive(Deserialize)]
                struct A {
                    board: u32,
                    strips: Vec<u32>,
                    #[serde(default)]
                    sub_board: Option<u32>,
                    index: usize,
                }
                let A {
                    board,
                    strips,
                    sub_board,
                    index,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_strips(
                    &window, state, board, strips, sub_board, index,
                ))
            }
            "insert_day_break" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    index: usize,
                }
                let A { sub_board, index } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::insert_day_break(
                    &window, state, sub_board, index,
                ))
            }
            "delete_strips" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    strips: Vec<u32>,
                }
                let A { board, strips } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_strips(
                    &window, state, board, strips,
                ))
            }
            "insert_banner" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    index: usize,
                    text: String,
                    duration: String,
                    #[serde(default)]
                    style: Option<board::BannerStyleInput>,
                }
                let A {
                    sub_board,
                    index,
                    text,
                    duration,
                    style,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::insert_banner(
                    &window, state, sub_board, index, text, duration, style,
                ))
            }
            "set_banner" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    strip: u32,
                    text: String,
                    duration: String,
                    #[serde(default)]
                    style: Option<board::BannerStyleInput>,
                }
                let A {
                    board,
                    strip,
                    text,
                    duration,
                    style,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_banner(
                    &window, state, board, strip, text, duration, style,
                ))
            }
            "sort_strips" => {
                #[derive(Deserialize)]
                struct A {
                    board: u32,
                    #[serde(default)]
                    sub_board: Option<u32>,
                    keys: Vec<board::SortKeyInput>,
                }
                let A {
                    board,
                    sub_board,
                    keys,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::sort_strips(
                    &window, state, board, sub_board, keys,
                ))
            }
            "set_strip_layout" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    columns: Vec<board::LayoutColumnInput>,
                    #[serde(default)]
                    size: Option<String>,
                    #[serde(default)]
                    marks: Option<board::MarksInput>,
                }
                let A {
                    board,
                    columns,
                    size,
                    marks,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_strip_layout(
                    &window, state, board, columns, size, marks,
                ))
            }
            "continue_scene" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    strip: u32,
                }
                let A { board, strip } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::continue_scene(
                    &window, state, board, strip,
                ))
            }
            "set_day_look" => {
                #[derive(Deserialize)]
                struct A {
                    board: u32,
                    look: board::DayLookInput,
                }
                let A { board, look } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_day_look(
                    &window, state, board, look,
                ))
            }
            "set_strip_colors" => {
                #[derive(Deserialize)]
                struct A {
                    rules: Vec<board::ColorRuleInput>,
                    #[serde(default)]
                    scene_text: Option<String>,
                    #[serde(default)]
                    banner_text: Option<String>,
                    #[serde(default)]
                    break_text: Option<String>,
                }
                let A {
                    rules,
                    scene_text,
                    banner_text,
                    break_text,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_strip_colors(
                    &window,
                    state,
                    rules,
                    scene_text,
                    banner_text,
                    break_text,
                ))
            }
            "show_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::show_board(&window, state, board))
            }
            "add_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    name: String,
                }
                let A { name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_board(&window, state, name))
            }
            "duplicate_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    name: String,
                }
                let A { board, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::duplicate_board(
                    &window, state, board, name,
                ))
            }
            "rename_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    name: String,
                }
                let A { board, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::rename_board(
                    &window, state, board, name,
                ))
            }
            "move_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    to: usize,
                }
                let A { board, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_board(&window, state, board, to))
            }
            "delete_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_board(&window, state, board))
            }
            "add_sub_board" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    name: String,
                }
                let A { board, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_sub_board(
                    &window, state, board, name,
                ))
            }
            "rename_sub_board" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    name: String,
                }
                let A { sub_board, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::rename_sub_board(
                    &window, state, sub_board, name,
                ))
            }
            "move_sub_board" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    to: usize,
                }
                let A { sub_board, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_sub_board(
                    &window, state, sub_board, to,
                ))
            }
            "delete_sub_board" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                }
                let A { sub_board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_sub_board(
                    &window, state, sub_board,
                ))
            }
            "auto_day_breaks" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    by: String,
                    limit: String,
                }
                let A {
                    sub_board,
                    by,
                    limit,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::auto_day_breaks(
                    &window, state, sub_board, by, limit,
                ))
            }
            "remove_day_breaks" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                }
                let A { sub_board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::remove_day_breaks(
                    &window, state, sub_board,
                ))
            }
            "set_start_date" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    date: Option<String>,
                }
                let A { sub_board, date } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_start_date(
                    &window, state, sub_board, date,
                ))
            }
            "get_sheet" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    #[serde(default)]
                    sheet: Option<u32>,
                    #[serde(default)]
                    index: Option<usize>,
                    #[serde(default)]
                    order: Option<String>,
                }
                let A {
                    board,
                    sheet,
                    index,
                    order,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::get_sheet(
                    state, board, sheet, index, order,
                ))
            }
            "set_sheet_field" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    field: String,
                    value: String,
                }
                let A {
                    sheet,
                    field,
                    value,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_sheet_field(
                    &window, state, sheet, field, value,
                ))
            }
            "set_sheet_pages" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    pages: String,
                    eighths: String,
                }
                let A {
                    sheet,
                    pages,
                    eighths,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_sheet_pages(
                    &window, state, sheet, pages, eighths,
                ))
            }
            "tag_element" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    element: u32,
                }
                let A { sheet, element } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::tag_element(
                    &window, state, sheet, element,
                ))
            }
            "set_quantity" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    element: u32,
                    count: String,
                    daily: bool,
                }
                let A {
                    sheet,
                    element,
                    count,
                    daily,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_quantity(
                    &window, state, sheet, element, count, daily,
                ))
            }
            "create_element" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    category: String,
                    name: String,
                }
                let A {
                    sheet,
                    category,
                    name,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::create_element(
                    &window, state, sheet, category, name,
                ))
            }
            "untag_element" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    element: u32,
                }
                let A { sheet, element } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::untag_element(
                    &window, state, sheet, element,
                ))
            }
            "new_sheet" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    #[serde(default)]
                    after: Option<u32>,
                }
                let A { board, after } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::new_sheet(
                    &window, state, board, after,
                ))
            }
            "delete_sheet" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    sheet: u32,
                    #[serde(default)]
                    order: Option<String>,
                }
                let A {
                    board,
                    sheet,
                    order,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_sheet(
                    &window, state, board, sheet, order,
                ))
            }
            "merge_sheets_preview" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    sheets: Vec<u32>,
                }
                let A { board, sheets } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::merge_sheets_preview(
                    state, board, sheets,
                ))
            }
            "merge_sheets" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    sheets: Vec<u32>,
                }
                let A { board, sheets } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::merge_sheets(
                    &window, state, board, sheets,
                ))
            }
            "duplicate_sheets" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    sheets: Vec<u32>,
                }
                let A { board, sheets } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::duplicate_sheets(
                    &window, state, board, sheets,
                ))
            }
            "paste_strips" => {
                #[derive(Deserialize)]
                struct A {
                    board: u32,
                    #[serde(default)]
                    sub_board: Option<u32>,
                    index: usize,
                }
                let A {
                    board,
                    sub_board,
                    index,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::paste_strips(
                    &window, state, board, sub_board, index,
                ))
            }
            "paste_elements" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                }
                let A { sheet } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::paste_elements(&window, state, sheet))
            }
            "elements_view" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    category: Option<u32>,
                    #[serde(default)]
                    header: Option<String>,
                }
                let A {
                    sub_board,
                    category,
                    header,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::elements_view(
                    state, sub_board, category, header,
                ))
            }
            "add_category" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    name: String,
                }
                let A { name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_category(&window, state, name))
            }
            "set_category_field" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    field: String,
                    value: String,
                }
                let A {
                    category,
                    field,
                    value,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_category_field(
                    &window, state, category, field, value,
                ))
            }
            "move_category" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    to: usize,
                }
                let A { category, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_category(
                    &window, state, category, to,
                ))
            }
            "delete_category" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                }
                let A { category } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_category(
                    &window, state, category,
                ))
            }
            "apply_category_defaults" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                }
                let A { category } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::apply_category_defaults(
                    &window, state, category,
                ))
            }
            "set_element_field" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    field: String,
                    value: String,
                }
                let A {
                    element,
                    field,
                    value,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_element_field(
                    &window, state, element, field, value,
                ))
            }
            "element_days" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    element: u32,
                }
                let A { sub_board, element } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::element_days(
                    state, sub_board, element,
                ))
            }
            "add_property" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    name: String,
                }
                let A { category, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_property(
                    &window, state, category, name,
                ))
            }
            "rename_property" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    at: usize,
                    name: String,
                }
                let A { category, at, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::rename_property(
                    &window, state, category, at, name,
                ))
            }
            "move_property" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    at: usize,
                    to: usize,
                }
                let A { category, at, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_property(
                    &window, state, category, at, to,
                ))
            }
            "delete_property" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    at: usize,
                }
                let A { category, at } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_property(
                    &window, state, category, at,
                ))
            }
            "set_property_multi_line" => {
                #[derive(Deserialize)]
                struct A {
                    category: u32,
                    at: usize,
                    multi_line: bool,
                }
                let A {
                    category,
                    at,
                    multi_line,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_property_multi_line(
                    &window, state, category, at, multi_line,
                ))
            }
            "set_property_value" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    name: String,
                    value: String,
                }
                let A {
                    element,
                    name,
                    value,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_property_value(
                    &window, state, element, name, value,
                ))
            }
            "set_links" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    links: Vec<u32>,
                    retroactive: bool,
                }
                let A {
                    element,
                    links,
                    retroactive,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_links(
                    &window,
                    state,
                    element,
                    links,
                    retroactive,
                ))
            }
            "remove_element_image" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    at: usize,
                }
                let A { element, at } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::remove_element_image(
                    &window, state, element, at,
                ))
            }
            "merge_elements" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    survivor: u32,
                    others: Vec<u32>,
                }
                let A { survivor, others } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::merge_elements(
                    &window, state, survivor, others,
                ))
            }
            "move_element" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    category: u32,
                }
                let A { element, category } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_element(
                    &window, state, element, category,
                ))
            }
            "renumber_elements" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    start: String,
                    #[serde(default)]
                    order: Option<Vec<u32>>,
                }
                let A {
                    category,
                    start,
                    order,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::renumber_elements(
                    &window, state, category, start, order,
                ))
            }
            "delete_element" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    element: u32,
                    unlink: bool,
                }
                let A { element, unlink } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_element(
                    &window, state, element, unlink,
                ))
            }
            "red_flags_view" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                }
                let A { sub_board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::red_flags_view(state, sub_board))
            }
            "set_red_flag" => {
                #[derive(Deserialize)]
                struct A {
                    element: u32,
                    flag: redflags::RedFlagInput,
                }
                let A { element, flag } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_red_flag(
                    &window, state, element, flag,
                ))
            }
            "delete_red_flag" => {
                #[derive(Deserialize)]
                struct A {
                    flag: u32,
                }
                let A { flag } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_red_flag(&window, state, flag))
            }
            "add_red_flag_type" => {
                #[derive(Deserialize)]
                struct A {
                    name: String,
                }
                let A { name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_red_flag_type(
                    &window, state, name,
                ))
            }
            "rename_red_flag_type" => {
                #[derive(Deserialize)]
                struct A {
                    red_flag_type: u32,
                    name: String,
                }
                let A {
                    red_flag_type,
                    name,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::rename_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                    name,
                ))
            }
            "move_red_flag_type" => {
                #[derive(Deserialize)]
                struct A {
                    red_flag_type: u32,
                    to: usize,
                }
                let A { red_flag_type, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::move_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                    to,
                ))
            }
            "delete_red_flag_type" => {
                #[derive(Deserialize)]
                struct A {
                    red_flag_type: u32,
                }
                let A { red_flag_type } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                ))
            }
            "add_standard_red_flag_types" => to_reply(
                crate::commands::app::add_standard_red_flag_types(&window, state),
            ),
            "add_red_flags" => {
                #[derive(Deserialize)]
                struct A {
                    flags: redflags::AddFlagsInput,
                }
                let A { flags } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::add_red_flags(&window, state, flags))
            }
            "delete_red_flags" => {
                #[derive(Deserialize)]
                struct A {
                    flags: Vec<u32>,
                }
                let A { flags } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_red_flags(
                    &window, state, flags,
                ))
            }
            "calendar_view" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    year: Option<i32>,
                    #[serde(default)]
                    month: Option<u32>,
                    #[serde(default)]
                    week_start: Option<String>,
                    #[serde(default)]
                    fields: Option<Vec<String>>,
                    #[serde(default)]
                    span: Option<bool>,
                }
                let A {
                    sub_board,
                    year,
                    month,
                    week_start,
                    fields,
                    span,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::calendar_view(
                    state, sub_board, year, month, week_start, fields, span,
                ))
            }
            "set_day_kind" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    date: String,
                    kind: String,
                }
                let A {
                    sub_board,
                    date,
                    kind,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_day_kind(
                    &window, state, sub_board, date, kind,
                ))
            }
            "set_weekly_off" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    days: Vec<String>,
                }
                let A { sub_board, days } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_weekly_off(
                    &window, state, sub_board, days,
                ))
            }
            "save_event" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    index: Option<usize>,
                    event: EventInput,
                }
                let A {
                    sub_board,
                    index,
                    event,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::save_event(
                    &window, state, sub_board, index, event,
                ))
            }
            "set_calendar_hidden" => {
                #[derive(Deserialize)]
                struct A {
                    hidden: Vec<String>,
                }
                let A { hidden } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::set_calendar_hidden(
                    &window, state, hidden,
                ))
            }
            "delete_event" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    index: usize,
                }
                let A { sub_board, index } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::app::delete_event(
                    &window, state, sub_board, index,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod breakdown_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::breakdown::*;
    use serde::Deserialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "set_sheet_field_order" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    fields: Vec<String>,
                    #[serde(default)]
                    hidden: Option<Vec<String>>,
                    #[serde(default)]
                    widths: Option<std::collections::BTreeMap<String, String>>,
                    #[serde(default)]
                    rows: Option<Vec<u8>>,
                }
                let A {
                    fields,
                    hidden,
                    widths,
                    rows,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::breakdown::set_sheet_field_order(
                    &window, state, fields, hidden, widths, rows,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod events_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::events::*;
    use serde::Deserialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "move_event_to" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    index: usize,
                    date: String,
                }
                let A {
                    sub_board,
                    index,
                    date,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::events::move_event_to(
                    &window, state, sub_board, index, date,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod find_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::find::*;
    use crate::view::View;
    use serde::Deserialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "find_text" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    query: QueryArg,
                }
                let A { query } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::find::find_text(state, query))
            }
            "replace_text" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    query: QueryArg,
                    with: String,
                    picks: Vec<String>,
                }
                let A { query, with, picks } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::find::replace_text(
                    &window, state, query, with, picks,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod flagcolors_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::flagcolors::*;
    use serde::Deserialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "set_red_flag_type_color" => {
                #[derive(Deserialize)]
                struct A {
                    red_flag_type: u32,
                    color: String,
                }
                let A {
                    red_flag_type,
                    color,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::flagcolors::set_red_flag_type_color(
                    &window,
                    state,
                    red_flag_type,
                    color,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod layouts_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::board::EditResult;
    use crate::layouts::*;
    use method_schedule_core::Schedule;
    use serde::Deserialize;
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "add_layout" => {
                #[derive(Deserialize)]
                struct A {
                    name: String,
                    kind: String,
                    layout: LayoutInput,
                    #[serde(default)]
                    board: Option<u32>,
                }
                let A {
                    name,
                    kind,
                    layout,
                    board,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::add_layout(
                    &window, state, name, kind, layout, board,
                ))
            }
            "save_layout" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    layout: LayoutInput,
                }
                let A { id, layout } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::save_layout(
                    &window, state, id, layout,
                ))
            }
            "rename_layout" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    name: String,
                }
                let A { id, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::rename_layout(
                    &window, state, id, name,
                ))
            }
            "move_layout" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    to: usize,
                }
                let A { id, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::move_layout(
                    &window, state, id, to,
                ))
            }
            "delete_layout" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                }
                let A { id } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::delete_layout(&window, state, id))
            }
            "use_layout" => {
                #[derive(Deserialize)]
                struct A {
                    board: u32,
                    kind: String,
                    #[serde(default)]
                    id: Option<u32>,
                }
                let A { board, kind, id } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::use_layout(
                    &window, state, board, kind, id,
                ))
            }
            "use_one_line_layout" => {
                #[derive(Deserialize)]
                struct A {
                    #[serde(default)]
                    id: Option<u32>,
                }
                let A { id } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::layouts::use_one_line_layout(
                    &window, state, id,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod plan_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::plan::*;
    use crate::session;
    use crate::view::View;
    use method_schedule_core::drift::{lock, unlock};
    use method_schedule_core::fdx;
    use method_schedule_core::fountain::ImportedScript;
    use method_schedule_core::model::{BoardId, StripId};
    use method_schedule_core::plan::{self as p};
    use method_schedule_core::revise::{import_tagged, revise, revision_text};
    use serde::Deserialize;
    use serde_json::Value;
    use std::sync::Mutex;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "timeline_data" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    #[serde(default)]
                    strip: Option<u32>,
                }
                let A { sub_board, strip } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::timeline_data(
                    state, sub_board, strip,
                ))
            }
            "push_past_wrap" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    day: u32,
                    strips: Vec<u32>,
                }
                let A {
                    sub_board,
                    day,
                    strips,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::push_past_wrap(
                    &window, state, sub_board, day, strips,
                ))
            }
            "set_call_time" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    day: u32,
                    call: String,
                }
                let A { day, call } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_call_time(
                    &window, state, day, call,
                ))
            }
            "set_person_call" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    day: u32,
                    element: u32,
                    call: String,
                    wrap: String,
                }
                let A {
                    day,
                    element,
                    call,
                    wrap,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_person_call(
                    &window, state, day, element, call, wrap,
                ))
            }
            "set_shot" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    day: u32,
                    eighths: String,
                    complete: bool,
                }
                let A {
                    sheet,
                    day,
                    eighths,
                    complete,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_shot(
                    &window, state, sheet, day, eighths, complete,
                ))
            }
            "set_long_day" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    value: String,
                }
                let A { value } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_long_day(&window, state, value))
            }
            "locations_data" => to_reply(crate::commands::plan::locations_data(state)),
            "set_location" => {
                #[derive(Deserialize)]
                struct A {
                    name: String,
                    latitude: String,
                    longitude: String,
                    utc_offset: String,
                    daylight_saving: String,
                }
                let A {
                    name,
                    latitude,
                    longitude,
                    utc_offset,
                    daylight_saving,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_location(
                    &window,
                    state,
                    name,
                    latitude,
                    longitude,
                    utc_offset,
                    daylight_saving,
                ))
            }
            "rest_data" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::rest_data(state, board))
            }
            "set_turnaround" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    category: u32,
                    value: String,
                }
                let A { category, value } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::set_turnaround(
                    &window, state, category, value,
                ))
            }
            "locks_data" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::locks_data(state, board))
            }
            "lock_schedule" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                    name: String,
                }
                let A { board, name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::lock_schedule(
                    &window, state, board, name,
                ))
            }
            "delete_lock" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    name: String,
                }
                let A { name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::delete_lock(&window, state, name))
            }
            "drift_data" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    name: String,
                }
                let A { name } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::drift_data(state, name))
            }
            "pin_strips" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    strips: Vec<u32>,
                    pinned: bool,
                }
                let A { strips, pinned } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::pin_strips(
                    &window, state, strips, pinned,
                ))
            }
            "suggest_order" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                }
                let A { sub_board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::suggest_order(state, sub_board))
            }
            "apply_suggested_order" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    order: Vec<u32>,
                }
                let A { sub_board, order } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::apply_suggested_order(
                    &window, state, sub_board, order,
                ))
            }
            "call_sheet_data" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    day: u32,
                }
                let A { sub_board, day } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::call_sheet_data(
                    state, sub_board, day,
                ))
            }
            "shoot_day_count" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                }
                let A { sub_board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::plan::shoot_day_count(state, sub_board))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod reports_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::reports::*;
    use method_schedule_core::model::Schedule;
    use method_schedule_format::pdf_info::PdfInfo;
    use serde::Deserialize;
    use serde::Serialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "report_data" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    index: usize,
                    #[serde(default)]
                    week_start: Option<String>,
                }
                let A {
                    sub_board,
                    index,
                    week_start,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reports::report_data(
                    state, sub_board, index, week_start,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod reportsetup_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::board::EditResult;
    use crate::reportsetup::*;
    use method_schedule_core::headers::HeaderId;
    use method_schedule_core::model::Schedule;
    use serde::Deserialize;
    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "report_list" => to_reply(crate::commands::reportsetup::report_list(state)),
            "save_report" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    index: usize,
                    name: String,
                    options: OptionsDto,
                }
                let A {
                    index,
                    name,
                    options,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::save_report(
                    &window, state, index, name, options,
                ))
            }
            "duplicate_report" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    index: usize,
                }
                let A { index } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::duplicate_report(
                    &window, state, index,
                ))
            }
            "move_report" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    index: usize,
                    to: usize,
                }
                let A { index, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::move_report(
                    &window, state, index, to,
                ))
            }
            "delete_report" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    index: usize,
                }
                let A { index } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::delete_report(
                    &window, state, index,
                ))
            }
            "save_header_layout" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    design: HeaderDto,
                }
                let A { design } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::save_header_layout(
                    &window, state, design,
                ))
            }
            "delete_header_layout" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    id: u32,
                }
                let A { id } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::delete_header_layout(
                    &window, state, id,
                ))
            }
            "move_header_layout" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    id: u32,
                    to: usize,
                }
                let A { id, to } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::move_header_layout(
                    &window, state, id, to,
                ))
            }
            "set_production" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    field: String,
                    value: String,
                }
                let A { field, value } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::set_production(
                    &window, state, field, value,
                ))
            }
            "set_dood_codes" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    codes: std::collections::BTreeMap<String, String>,
                }
                let A { codes } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::set_dood_codes(
                    &window, state, codes,
                ))
            }
            "set_prep_wrap" => {
                #[derive(Deserialize)]
                struct A {
                    sub_board: u32,
                    prep_start: String,
                    wrap_end: String,
                }
                let A {
                    sub_board,
                    prep_start,
                    wrap_end,
                } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::set_prep_wrap(
                    &window, state, sub_board, prep_start, wrap_end,
                ))
            }
            "set_calendar_fields" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    fields: Vec<String>,
                }
                let A { fields } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::reportsetup::set_calendar_fields(
                    &window, state, fields,
                ))
            }
            _ => return None,
        })
    }
}

#[allow(unused_imports)]
mod split_cmds {
    use super::super::{args, to_reply, DocState, Error, W};
    use crate::split::*;
    use crate::view::{self, BoardDto, View};
    use method_schedule_core::model::BoardId;
    use serde::Deserialize;
    use serde_json::Value;
    use std::path::PathBuf;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "board_view" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    board: u32,
                }
                let A { board } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::split::board_view(state, board))
            }
            "remove_sheet_image" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct A {
                    sheet: u32,
                    at: usize,
                }
                let A { sheet, at } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                to_reply(crate::commands::split::remove_sheet_image(
                    &window, state, sheet, at,
                ))
            }
            _ => return None,
        })
    }
}
