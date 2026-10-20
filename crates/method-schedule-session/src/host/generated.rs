//! Generated from the desktop's commands (crates/method-schedule-app/src) by
//! scripts/host-dispatch.py: each arm is the desktop command's body, run on the host.
//! Regenerate after a command changes; don't edit by hand.

#![allow(unused_imports, clippy::all)]

use super::{DocState, Error};
use serde_json::Value;

/// The desktop commands the host runs, in source order.
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
    "replace_text",
    "add_layout",
    "save_layout",
    "rename_layout",
    "move_layout",
    "delete_layout",
    "use_layout",
    "use_one_line_layout",
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

mod app_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
            "get_view" => {
                let r: CmdResult = (|| finish(&window, &state.lock(), None))();
                to_reply(r)
            }
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
                let r: CmdResult<DoodGridDto> = (|| {
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
                })();
                to_reply(r)
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
                let r: CmdResult<ExplainDto> = (|| {
                    crate::explain::explain_count(state.lock().schedule(), sub_board, element)
                })();
                to_reply(r)
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
                let r: CmdResult<crate::conflicts::ConflictsDto> =
                    (|| crate::conflicts::conflicts_view(state.lock().schedule(), board))();
                to_reply(r)
            }
            "undo" => {
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.undo();
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            "redo" => {
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.redo();
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::move_strips(s, board, &strips, sub_board, index)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    strip_edit(&window, &state, |s| {
                        board::insert_day_break(s, sub_board, index)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::delete_strips(s, board, &strips)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    strip_edit(&window, &state, |s| {
                        board::insert_banner_styled(
                            s,
                            sub_board,
                            index,
                            &text,
                            &duration,
                            style.as_ref(),
                        )
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::set_banner_styled(s, board, strip, &text, &duration, style.as_ref())
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::sort_strips(s, board, sub_board, &keys)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::set_strip_layout_with(
                            s,
                            board,
                            &columns,
                            size.as_deref().unwrap_or(""),
                            marks.unwrap_or_default(),
                        )
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| strip_edit(&window, &state, |s| board::continue_scene(s, board, strip)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::set_day_look(s, board, &look)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::set_strip_colors(
                            s,
                            &rules,
                            scene_text.as_deref(),
                            banner_text.as_deref(),
                            break_text.as_deref(),
                        )
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.show_board(method_schedule_core::model::BoardId(board))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    // A new board's sub-board starts today, as a new sub-board does (behavior note 27;
                    // decisions 123 and 153).
                    let today = super::super::today();
                    board_edit(&window, &state, |s| board::add_board(s, &name, today))
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| board_edit(&window, &state, |s| board::duplicate_board(s, board, &name)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::rename_board(s, board, &name)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::move_board(s, board, to)))();
                to_reply(r)
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
                let r: CmdResult = (|| edit(&window, &state, |s| board::delete_board(s, board)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    // A new sub-board starts today (behavior note 24; decision 123).
                    let today = super::super::today();
                    let id = s.edit(|x| board::add_sub_board(x, board, &name, today))?;
                    let mut v = finish(&window, &s, None)?;
                    v.sub_board = Some(id);
                    Ok(v)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::rename_sub_board(s, sub_board, &name)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::move_sub_board(s, sub_board, to)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| board::delete_sub_board(s, sub_board)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let n = s.edit(|x| board::auto_day_breaks_by(x, sub_board, &by, &limit))?;
                    let days = if n == 1 {
                        "1 shoot day".to_string()
                    } else {
                        format!("{n} shoot days")
                    };
                    finish(&window, &s, Some(format!("Day breaks set: {days}.")))
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
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
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        board::set_start_date(s, sub_board, date.as_deref())
                    })
                })();
                to_reply(r)
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
                let r: CmdResult<Option<SheetDto>> = (|| {
                    // Previous and Next by board order, sheet number or scene number (decisions.md,
                    // entry 81).
                    let by = sheet_order(order.as_deref())?;
                    view::sheet_view_in(state.lock().schedule(), board, sheet, index, by)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        sheet::set_field(s, sheet, &field, &value)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        sheet::set_pages(s, sheet, &pages, &eighths)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| sheet::tag_element(s, sheet, element)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        sheet::set_quantity(s, sheet, element, &count, daily)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        sheet::create_element(s, sheet, &category, &name)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| sheet::untag_element(s, sheet, element)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    sheet_edit(&window, &state, |s| {
                        sheet::new_sheet(s, board, after).map(Some)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let by = sheet_order(order.as_deref())?;
                    sheet_edit(&window, &state, |s| {
                        sheet::delete_sheet(s, board, sheet, by)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult<sheet::SheetMergeDto> =
                    (|| sheet::merge_preview(state.lock().schedule(), board, &sheets))();
                to_reply(r)
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
                let r: CmdResult = (|| {
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
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
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
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
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
                    let pasted =
                        s.edit(|x| clipboard::paste_strips(x, &clip, board, sub_board, index))?;
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
                        clipboard::Clip::Strips { cut: true, .. }
                            | clipboard::Clip::Carried { cut: true, .. }
                    ) {
                        s.set_clipboard(None);
                    }
                    let mut v = finish(&window, &s, Some(notice))?;
                    v.strips = pasted.strips;
                    Ok(v)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
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
                })();
                to_reply(r)
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
                let r: CmdResult<ElementsDto> = (|| {
                    view::elements_view(
                        state.lock().schedule(),
                        sub_board,
                        category,
                        header.as_deref(),
                    )
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let id = s.edit(|x| elements::add_category(x, &name))?;
                    let mut v = finish(&window, &s, None)?;
                    v.category = Some(id);
                    Ok(v)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::set_category_field(s, category, &field, &value)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::move_category(s, category, to)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| elements::delete_category(s, category)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let n = s.edit(|x| elements::apply_category_defaults(x, category))?;
                    let notice = match n {
                        0 => "Every element already had these rules.".to_string(),
                        1 => "Gave 1 element the category's rules.".to_string(),
                        n => format!("Gave {n} elements the category's rules."),
                    };
                    finish(&window, &s, Some(notice))
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::set_element_field(s, element, &field, &value)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult<ElementDaysDto> =
                    (|| view::element_days(state.lock().schedule(), sub_board, element))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::add_property(s, category, &name)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::rename_property(s, category, at, &name)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::move_property(s, category, at, to)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::delete_property(s, category, at)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::set_property_multi_line(s, category, at, multi_line)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::set_property_value(s, element, &name, &value)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let n = s.edit(|x| elements::set_links(x, element, &links, retroactive))?;
                    let notice = match (retroactive, n) {
                        (false, _) => None,
                        (true, 0) => {
                            Some("Every sheet with it already had its linked elements.".to_string())
                        }
                        (true, 1) => Some("Added its linked elements to 1 sheet.".to_string()),
                        (true, n) => Some(format!("Added its linked elements to {n} sheets.")),
                    };
                    finish(&window, &s, notice)
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| elements::remove_image(s, element, at)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::merge_elements(s, survivor, &others)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::move_element(s, element, category)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::renumber(s, category, &start, order.as_deref())
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        elements::delete_element(s, element, unlink)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult<redflags::RedFlagsDto> =
                    (|| redflags::red_flags_view(state.lock().schedule(), sub_board))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::set_red_flag(s, element, &flag)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| redflags::delete_red_flag(s, flag)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| redflags::add_red_flag_type(s, &name)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::rename_red_flag_type(s, red_flag_type, &name)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::move_red_flag_type(s, red_flag_type, to)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::delete_red_flag_type(s, red_flag_type)
                    })
                })();
                to_reply(r)
            }
            "add_standard_red_flag_types" => {
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::add_standard_types(s).map(|_| ())
                    })
                })();
                to_reply(r)
            }
            "add_red_flags" => {
                #[derive(Deserialize)]
                struct A {
                    flags: redflags::AddFlagsInput,
                }
                let A { flags } = match args::<A>(command, a) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        redflags::add_red_flags(s, &flags).map(|_| ())
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| redflags::delete_red_flags(s, &flags)))();
                to_reply(r)
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
                let r: CmdResult<CalendarDto> = (|| {
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
                                method_schedule_core::model::StripField::from_name(x)
                                    .filter(|f| f.of_sheet())
                            })
                            .collect();
                        if crate::reportsetup::old_calendar_pick(&old) {
                            opts.fields = old;
                        }
                    }
                    view::calendar_view_with(sch, sub_board, year, month, &opts)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        calendar::set_day_kind(s, sub_board, &date, &kind)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        calendar::set_weekly_off(s, sub_board, &days)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        calendar::save_event(s, sub_board, index, &event)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        calendar::set_calendar_hidden(s, &hidden)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        calendar::delete_event(s, sub_board, index)
                    })
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod breakdown_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| {
                        set_design(
                            x,
                            &fields,
                            hidden.as_deref(),
                            widths.as_ref(),
                            rows.as_deref(),
                        )
                    })?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod events_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| move_event(x, sub_board, index, &date))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod find_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult<FoundDto> = (|| find(state.lock().schedule(), &query))();
                to_reply(r)
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
                let r: CmdResult<View> = (|| {
                    let mut s = state.lock();
                    let notice = replace(&mut s, &query, &with, &picks)?;
                    finish(&window, &s, Some(notice))
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod flagcolors_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| set_color(x, red_flag_type, &color))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod layouts_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult = (|| {
                    edit_with(&window, &state, |s| {
                        add_named(s, &name, &kind, &layout, board).map(|_| ())
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit_with(&window, &state, |s| save_named(s, id, &layout)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit_with(&window, &state, |s| rename_named(s, id, &name)))();
                to_reply(r)
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
                let r: CmdResult = (|| edit_with(&window, &state, |s| move_named(s, id, to)))();
                to_reply(r)
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
                let r: CmdResult = (|| edit_with(&window, &state, |s| delete_named(s, id)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit_with(&window, &state, |s| pick_layout(s, board, &kind, id)))();
                to_reply(r)
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
                let r: CmdResult = (|| edit_with(&window, &state, |s| pick_one_line(s, id)))();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod plan_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult<TimelineDto> =
                    (|| timeline_view(state.lock().schedule(), sub_board, strip))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    if strips.len() > 100_000 {
                        return Err("Too many strips at once.".to_string());
                    }
                    let mut s = state.lock();
                    let n = s.edit(|x| push_past(x, sub_board, day, &strips))?;
                    let what = if n == 1 {
                        "1 strip".to_string()
                    } else {
                        format!("{n} strips")
                    };
                    finish(
                        &window,
                        &s,
                        Some(format!(
                            "Moved {what} to the start of day {}. Undo puts them back.",
                            day + 1
                        )),
                    )
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| set_call(x, day, &call))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| set_long(x, &value))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            "locations_data" => {
                let r: CmdResult<Vec<LocationDto>> =
                    (|| Ok(locations_view(state.lock().schedule())))();
                to_reply(r)
            }
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| {
                        place_location(
                            x,
                            &name,
                            &latitude,
                            &longitude,
                            &utc_offset,
                            &daylight_saving,
                        )
                    })?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
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
                let r: CmdResult<RestDto> = (|| rest_view(state.lock().schedule(), board))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| set_minimum(x, category, &value))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
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
                let r: CmdResult<Vec<LockDto>> = (|| {
                    let s = state.lock();
                    Ok(s.schedule()
                        .plan
                        .locks
                        .iter()
                        .filter(|l| l.board.0 == board)
                        .map(|l| LockDto {
                            name: l.name.clone(),
                            made: l.made.map_or(String::new(), |d| d.to_string()),
                            scenes: l.scenes.len(),
                        })
                        .collect())
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| {
                        if name.chars().count() > 200 {
                            return Err("A lock's name is 200 characters at most.".to_string());
                        }
                        lock(x, BoardId(board), &name, super::super::today()).map_err(upper)
                    })?;
                    finish(
                        &window,
                        &s,
                        Some(format!("Locked the schedule as {:?}.", name.trim())),
                    )
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| unlock(x, &name).map_err(upper))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
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
                let r: CmdResult<DriftDto> = (|| drift_view(state.lock().schedule(), &name))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    if strips.len() > 100_000 {
                        return Err("Too many strips at once.".to_string());
                    }
                    let ids: Vec<StripId> = strips.into_iter().map(StripId).collect();
                    let mut s = state.lock();
                    let n = s.edit(|x| {
                        p::set_pinned(x, &ids, pinned)
                            .map_err(|_| "Only scene strips can be pinned.".to_string())
                    })?;
                    let what = if n == 1 {
                        "1 strip".to_string()
                    } else {
                        format!("{n} strips")
                    };
                    let verb = if pinned { "Pinned" } else { "Unpinned" };
                    finish(&window, &s, Some(format!("{verb} {what}.")))
                })();
                to_reply(r)
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
                let r: CmdResult<SuggestionDto> =
                    (|| suggestion_view(state.lock().schedule(), sub_board))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let n = s.edit(|x| apply_order(x, sub_board, &order))?;
                    let what = if n == 1 {
                        "1 scene".to_string()
                    } else {
                        format!("{n} scenes")
                    };
                    finish(
                        &window,
                        &s,
                        Some(format!(
                            "Moved {what} into the suggested order. Undo puts them back."
                        )),
                    )
                })();
                to_reply(r)
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
                let r: CmdResult<serde_json::Value> = (|| {
                    let s = state.lock();
                    let bytes = call_sheet_bytes(s.schedule(), sub_board, day, false)?;
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
                })();
                to_reply(r)
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
                let r: CmdResult<u32> = (|| {
                    let s = state.lock();
                    let (b, sb) = ids(s.schedule(), sub_board)?;
                    Ok(method_schedule_core::days::days(s.schedule(), b, sb)
                        .map_err(upper)?
                        .days
                        .len() as u32)
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod reports_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult<ReportDto> = (|| {
                    build_report(
                        state.lock().schedule(),
                        sub_board,
                        index,
                        week_start.as_deref().unwrap_or("sun"),
                    )
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod reportsetup_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
            "report_list" => {
                let r: CmdResult<ReportsDto> = (|| Ok(reports_dto(state.lock().schedule())))();
                to_reply(r)
            }
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| apply_report(s, index, &name, &options)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let at = s.edit(|x| x.duplicate_report(index).map_err(say))?;
                    let name = s
                        .schedule()
                        .report_list()
                        .get(at)
                        .map_or(String::new(), |r| r.name.clone());
                    finish(&window, &s, Some(format!("Added {name}.")))
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| s.move_report(index, to).map_err(say)))();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| s.delete_report(index).map_err(say)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    let added = design.id == 0;
                    s.edit(|x| apply_header(x, &design).map(|_| ()))?;
                    let note =
                        added.then(|| format!("Added header layout {}.", design.name.trim()));
                    finish(&window, &s, note)
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        s.delete_header(HeaderId(id)).map_err(header_say)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        s.move_header(HeaderId(id), to).map_err(header_say)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| apply_production(s, &field, &value)))();
                to_reply(r)
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
                let r: CmdResult = (|| edit(&window, &state, |s| apply_codes(s, &codes)))();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    edit(&window, &state, |s| {
                        apply_prep_wrap(s, sub_board, &prep_start, &wrap_end)
                    })
                })();
                to_reply(r)
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
                let r: CmdResult =
                    (|| edit(&window, &state, |s| apply_calendar_fields(s, &fields)))();
                to_reply(r)
            }
            _ => return None,
        })
    }
}

mod split_cmds {
    use super::super::{
        args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply,
        CmdResult, DocState, Error, W,
    };
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
                let r: CmdResult<BoardDto> = (|| {
                    let s = state.lock();
                    view::board_ref(s.schedule(), board)?;
                    view::board(s.schedule(), BoardId(board))
                })();
                to_reply(r)
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
                let r: CmdResult = (|| {
                    let mut s = state.lock();
                    s.edit(|x| remove_image(x, sheet, at))?;
                    finish(&window, &s, None)
                })();
                to_reply(r)
            }
            _ => return None,
        })
    }
}
