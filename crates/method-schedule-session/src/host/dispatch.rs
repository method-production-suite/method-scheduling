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
    "set_day_crew_call",
    "settle_conflicts",
    "set_shot",
    "set_long_day",
    "locations_data",
    "set_location",
    "set_location_guide",
    "set_order_weights",
    "rename_draft",
    "remove_location_map",
    "rest_data",
    "set_turnaround",
    "crew_data",
    "set_crew_call",
    "drafts_data",
    "conflicts_data",
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
    "drafts_data",
    "conflicts_data",
    "crew_data",
    "locks_data",
    "drift_data",
    "suggest_order",
    "call_sheet_data",
    "shoot_day_count",
    "report_data",
    "report_list",
    "board_view",
];

/// One command's arm: its arguments read as the page sends them (`snake` keys, or `camel`
/// for the commands whose desktop side takes camelCase), refused in words when they don't
/// fit, then `$call`'s result as the reply. Each field is bound by name for `$call`.
macro_rules! call {
    ($command:expr, $a:expr, snake { $($(#[$fa:meta])* $f:ident : $t:ty),* $(,)? } => $call:block) => {{
        #[derive(serde::Deserialize)]
        struct A {
            $($(#[$fa])* $f: $t,)*
        }
        match $crate::host::args::<A>($command, $a) {
            Ok(A { $($f),* }) => $crate::host::to_reply($call),
            Err(e) => return Some(Err(e)),
        }
    }};
    ($command:expr, $a:expr, camel { $($(#[$fa:meta])* $f:ident : $t:ty),* $(,)? } => $call:block) => {{
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            $($(#[$fa])* $f: $t,)*
        }
        match $crate::host::args::<A>($command, $a) {
            Ok(A { $($f),* }) => $crate::host::to_reply($call),
            Err(e) => return Some(Err(e)),
        }
    }};
}

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
    use super::super::{to_reply, DocState, Error, W};
    use crate::board;
    use crate::calendar::EventInput;
    use crate::redflags;

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "get_view" => to_reply(crate::commands::app::get_view(&window, state)),
            "dood_grid" => call!(command, a, snake {
                sub_board: u32,
                #[serde(default)]
                category: Option<u32>,
                #[serde(default)]
                header: Option<String>,
                #[serde(default)]
                sort: Option<Vec<String>>,
                #[serde(default)]
                no_holds: Option<bool>,
            } => {
                crate::commands::app::dood_grid(
                    state, sub_board, category, header, sort, no_holds,
                )
            }),
            "explain_count" => call!(command, a, snake {
                sub_board: u32,
                element: u32,
            } => {
                crate::commands::app::explain_count(
                    state, sub_board, element,
                )
            }),
            "conflicts_view" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::app::conflicts_view(state, board)
            }),
            "undo" => to_reply(crate::commands::app::undo(&window, state)),
            "redo" => to_reply(crate::commands::app::redo(&window, state)),
            "move_strips" => call!(command, a, snake {
                board: u32,
                strips: Vec<u32>,
                #[serde(default)]
                sub_board: Option<u32>,
                index: usize,
            } => {
                crate::commands::app::move_strips(
                    &window, state, board, strips, sub_board, index,
                )
            }),
            "insert_day_break" => call!(command, a, snake {
                sub_board: u32,
                index: usize,
            } => {
                crate::commands::app::insert_day_break(
                    &window, state, sub_board, index,
                )
            }),
            "delete_strips" => call!(command, a, camel {
                board: u32,
                strips: Vec<u32>,
            } => {
                crate::commands::app::delete_strips(
                    &window, state, board, strips,
                )
            }),
            "insert_banner" => call!(command, a, snake {
                sub_board: u32,
                index: usize,
                text: String,
                duration: String,
                #[serde(default)]
                style: Option<board::BannerStyleInput>,
            } => {
                crate::commands::app::insert_banner(
                    &window, state, sub_board, index, text, duration, style,
                )
            }),
            "set_banner" => call!(command, a, camel {
                board: u32,
                strip: u32,
                text: String,
                duration: String,
                #[serde(default)]
                style: Option<board::BannerStyleInput>,
            } => {
                crate::commands::app::set_banner(
                    &window, state, board, strip, text, duration, style,
                )
            }),
            "sort_strips" => call!(command, a, snake {
                board: u32,
                #[serde(default)]
                sub_board: Option<u32>,
                keys: Vec<board::SortKeyInput>,
            } => {
                crate::commands::app::sort_strips(
                    &window, state, board, sub_board, keys,
                )
            }),
            "set_strip_layout" => call!(command, a, camel {
                board: u32,
                columns: Vec<board::LayoutColumnInput>,
                #[serde(default)]
                size: Option<String>,
                #[serde(default)]
                marks: Option<board::MarksInput>,
            } => {
                crate::commands::app::set_strip_layout(
                    &window, state, board, columns, size, marks,
                )
            }),
            "continue_scene" => call!(command, a, camel {
                board: u32,
                strip: u32,
            } => {
                crate::commands::app::continue_scene(
                    &window, state, board, strip,
                )
            }),
            "set_day_look" => call!(command, a, snake {
                board: u32,
                look: board::DayLookInput,
            } => {
                crate::commands::app::set_day_look(
                    &window, state, board, look,
                )
            }),
            "set_strip_colors" => call!(command, a, snake {
                rules: Vec<board::ColorRuleInput>,
                #[serde(default)]
                scene_text: Option<String>,
                #[serde(default)]
                banner_text: Option<String>,
                #[serde(default)]
                break_text: Option<String>,
            } => {
                crate::commands::app::set_strip_colors(
                    &window,
                    state,
                    rules,
                    scene_text,
                    banner_text,
                    break_text,
                )
            }),
            "show_board" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::app::show_board(&window, state, board)
            }),
            "add_board" => call!(command, a, camel {
                name: String,
            } => {
                crate::commands::app::add_board(&window, state, name)
            }),
            "duplicate_board" => call!(command, a, camel {
                board: u32,
                name: String,
            } => {
                crate::commands::app::duplicate_board(
                    &window, state, board, name,
                )
            }),
            "rename_board" => call!(command, a, camel {
                board: u32,
                name: String,
            } => {
                crate::commands::app::rename_board(
                    &window, state, board, name,
                )
            }),
            "move_board" => call!(command, a, camel {
                board: u32,
                to: usize,
            } => {
                crate::commands::app::move_board(&window, state, board, to)
            }),
            "delete_board" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::app::delete_board(&window, state, board)
            }),
            "add_sub_board" => call!(command, a, camel {
                board: u32,
                name: String,
            } => {
                crate::commands::app::add_sub_board(
                    &window, state, board, name,
                )
            }),
            "rename_sub_board" => call!(command, a, snake {
                sub_board: u32,
                name: String,
            } => {
                crate::commands::app::rename_sub_board(
                    &window, state, sub_board, name,
                )
            }),
            "move_sub_board" => call!(command, a, snake {
                sub_board: u32,
                to: usize,
            } => {
                crate::commands::app::move_sub_board(
                    &window, state, sub_board, to,
                )
            }),
            "delete_sub_board" => call!(command, a, snake {
                sub_board: u32,
            } => {
                crate::commands::app::delete_sub_board(
                    &window, state, sub_board,
                )
            }),
            "auto_day_breaks" => call!(command, a, snake {
                sub_board: u32,
                by: String,
                limit: String,
            } => {
                crate::commands::app::auto_day_breaks(
                    &window, state, sub_board, by, limit,
                )
            }),
            "remove_day_breaks" => call!(command, a, snake {
                sub_board: u32,
            } => {
                crate::commands::app::remove_day_breaks(
                    &window, state, sub_board,
                )
            }),
            "set_start_date" => call!(command, a, snake {
                sub_board: u32,
                #[serde(default)]
                date: Option<String>,
            } => {
                crate::commands::app::set_start_date(
                    &window, state, sub_board, date,
                )
            }),
            "get_sheet" => call!(command, a, camel {
                board: u32,
                #[serde(default)]
                sheet: Option<u32>,
                #[serde(default)]
                index: Option<usize>,
                #[serde(default)]
                order: Option<String>,
            } => {
                crate::commands::app::get_sheet(
                    state, board, sheet, index, order,
                )
            }),
            "set_sheet_field" => call!(command, a, camel {
                sheet: u32,
                field: String,
                value: String,
            } => {
                crate::commands::app::set_sheet_field(
                    &window, state, sheet, field, value,
                )
            }),
            "set_sheet_pages" => call!(command, a, camel {
                sheet: u32,
                pages: String,
                eighths: String,
            } => {
                crate::commands::app::set_sheet_pages(
                    &window, state, sheet, pages, eighths,
                )
            }),
            "tag_element" => call!(command, a, camel {
                sheet: u32,
                element: u32,
            } => {
                crate::commands::app::tag_element(
                    &window, state, sheet, element,
                )
            }),
            "set_quantity" => call!(command, a, camel {
                sheet: u32,
                element: u32,
                count: String,
                daily: bool,
            } => {
                crate::commands::app::set_quantity(
                    &window, state, sheet, element, count, daily,
                )
            }),
            "create_element" => call!(command, a, camel {
                sheet: u32,
                category: String,
                name: String,
            } => {
                crate::commands::app::create_element(
                    &window, state, sheet, category, name,
                )
            }),
            "untag_element" => call!(command, a, camel {
                sheet: u32,
                element: u32,
            } => {
                crate::commands::app::untag_element(
                    &window, state, sheet, element,
                )
            }),
            "new_sheet" => call!(command, a, camel {
                board: u32,
                #[serde(default)]
                after: Option<u32>,
            } => {
                crate::commands::app::new_sheet(
                    &window, state, board, after,
                )
            }),
            "delete_sheet" => call!(command, a, camel {
                board: u32,
                sheet: u32,
                #[serde(default)]
                order: Option<String>,
            } => {
                crate::commands::app::delete_sheet(
                    &window, state, board, sheet, order,
                )
            }),
            "merge_sheets_preview" => call!(command, a, camel {
                board: u32,
                sheets: Vec<u32>,
            } => {
                crate::commands::app::merge_sheets_preview(
                    state, board, sheets,
                )
            }),
            "merge_sheets" => call!(command, a, camel {
                board: u32,
                sheets: Vec<u32>,
            } => {
                crate::commands::app::merge_sheets(
                    &window, state, board, sheets,
                )
            }),
            "duplicate_sheets" => call!(command, a, camel {
                board: u32,
                sheets: Vec<u32>,
            } => {
                crate::commands::app::duplicate_sheets(
                    &window, state, board, sheets,
                )
            }),
            "paste_strips" => call!(command, a, snake {
                board: u32,
                #[serde(default)]
                sub_board: Option<u32>,
                index: usize,
            } => {
                crate::commands::app::paste_strips(
                    &window, state, board, sub_board, index,
                )
            }),
            "paste_elements" => call!(command, a, camel {
                sheet: u32,
            } => {
                crate::commands::app::paste_elements(&window, state, sheet)
            }),
            "elements_view" => call!(command, a, snake {
                sub_board: u32,
                #[serde(default)]
                category: Option<u32>,
                #[serde(default)]
                header: Option<String>,
            } => {
                crate::commands::app::elements_view(
                    state, sub_board, category, header,
                )
            }),
            "add_category" => call!(command, a, camel {
                name: String,
            } => {
                crate::commands::app::add_category(&window, state, name)
            }),
            "set_category_field" => call!(command, a, camel {
                category: u32,
                field: String,
                value: String,
            } => {
                crate::commands::app::set_category_field(
                    &window, state, category, field, value,
                )
            }),
            "move_category" => call!(command, a, camel {
                category: u32,
                to: usize,
            } => {
                crate::commands::app::move_category(
                    &window, state, category, to,
                )
            }),
            "delete_category" => call!(command, a, camel {
                category: u32,
            } => {
                crate::commands::app::delete_category(
                    &window, state, category,
                )
            }),
            "apply_category_defaults" => call!(command, a, camel {
                category: u32,
            } => {
                crate::commands::app::apply_category_defaults(
                    &window, state, category,
                )
            }),
            "set_element_field" => call!(command, a, camel {
                element: u32,
                field: String,
                value: String,
            } => {
                crate::commands::app::set_element_field(
                    &window, state, element, field, value,
                )
            }),
            "element_days" => call!(command, a, snake {
                sub_board: u32,
                element: u32,
            } => {
                crate::commands::app::element_days(
                    state, sub_board, element,
                )
            }),
            "add_property" => call!(command, a, camel {
                category: u32,
                name: String,
            } => {
                crate::commands::app::add_property(
                    &window, state, category, name,
                )
            }),
            "rename_property" => call!(command, a, camel {
                category: u32,
                at: usize,
                name: String,
            } => {
                crate::commands::app::rename_property(
                    &window, state, category, at, name,
                )
            }),
            "move_property" => call!(command, a, camel {
                category: u32,
                at: usize,
                to: usize,
            } => {
                crate::commands::app::move_property(
                    &window, state, category, at, to,
                )
            }),
            "delete_property" => call!(command, a, camel {
                category: u32,
                at: usize,
            } => {
                crate::commands::app::delete_property(
                    &window, state, category, at,
                )
            }),
            "set_property_multi_line" => call!(command, a, snake {
                category: u32,
                at: usize,
                multi_line: bool,
            } => {
                crate::commands::app::set_property_multi_line(
                    &window, state, category, at, multi_line,
                )
            }),
            "set_property_value" => call!(command, a, camel {
                element: u32,
                name: String,
                value: String,
            } => {
                crate::commands::app::set_property_value(
                    &window, state, element, name, value,
                )
            }),
            "set_links" => call!(command, a, camel {
                element: u32,
                links: Vec<u32>,
                retroactive: bool,
            } => {
                crate::commands::app::set_links(
                    &window,
                    state,
                    element,
                    links,
                    retroactive,
                )
            }),
            "remove_element_image" => call!(command, a, camel {
                element: u32,
                at: usize,
            } => {
                crate::commands::app::remove_element_image(
                    &window, state, element, at,
                )
            }),
            "merge_elements" => call!(command, a, camel {
                survivor: u32,
                others: Vec<u32>,
            } => {
                crate::commands::app::merge_elements(
                    &window, state, survivor, others,
                )
            }),
            "move_element" => call!(command, a, camel {
                element: u32,
                category: u32,
            } => {
                crate::commands::app::move_element(
                    &window, state, element, category,
                )
            }),
            "renumber_elements" => call!(command, a, camel {
                category: u32,
                start: String,
                #[serde(default)]
                order: Option<Vec<u32>>,
            } => {
                crate::commands::app::renumber_elements(
                    &window, state, category, start, order,
                )
            }),
            "delete_element" => call!(command, a, camel {
                element: u32,
                unlink: bool,
            } => {
                crate::commands::app::delete_element(
                    &window, state, element, unlink,
                )
            }),
            "red_flags_view" => call!(command, a, snake {
                sub_board: u32,
            } => {
                crate::commands::app::red_flags_view(state, sub_board)
            }),
            "set_red_flag" => call!(command, a, snake {
                element: u32,
                flag: redflags::RedFlagInput,
            } => {
                crate::commands::app::set_red_flag(
                    &window, state, element, flag,
                )
            }),
            "delete_red_flag" => call!(command, a, snake {
                flag: u32,
            } => {
                crate::commands::app::delete_red_flag(&window, state, flag)
            }),
            "add_red_flag_type" => call!(command, a, snake {
                name: String,
            } => {
                crate::commands::app::add_red_flag_type(
                    &window, state, name,
                )
            }),
            "rename_red_flag_type" => call!(command, a, snake {
                red_flag_type: u32,
                name: String,
            } => {
                crate::commands::app::rename_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                    name,
                )
            }),
            "move_red_flag_type" => call!(command, a, snake {
                red_flag_type: u32,
                to: usize,
            } => {
                crate::commands::app::move_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                    to,
                )
            }),
            "delete_red_flag_type" => call!(command, a, snake {
                red_flag_type: u32,
            } => {
                crate::commands::app::delete_red_flag_type(
                    &window,
                    state,
                    red_flag_type,
                )
            }),
            "add_standard_red_flag_types" => to_reply(
                crate::commands::app::add_standard_red_flag_types(&window, state),
            ),
            "add_red_flags" => call!(command, a, snake {
                flags: redflags::AddFlagsInput,
            } => {
                crate::commands::app::add_red_flags(&window, state, flags)
            }),
            "delete_red_flags" => call!(command, a, snake {
                flags: Vec<u32>,
            } => {
                crate::commands::app::delete_red_flags(
                    &window, state, flags,
                )
            }),
            "calendar_view" => call!(command, a, snake {
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
            } => {
                crate::commands::app::calendar_view(
                    state, sub_board, year, month, week_start, fields, span,
                )
            }),
            "set_day_kind" => call!(command, a, snake {
                sub_board: u32,
                date: String,
                kind: String,
            } => {
                crate::commands::app::set_day_kind(
                    &window, state, sub_board, date, kind,
                )
            }),
            "set_weekly_off" => call!(command, a, snake {
                sub_board: u32,
                days: Vec<String>,
            } => {
                crate::commands::app::set_weekly_off(
                    &window, state, sub_board, days,
                )
            }),
            "save_event" => call!(command, a, snake {
                sub_board: u32,
                #[serde(default)]
                index: Option<usize>,
                event: EventInput,
            } => {
                crate::commands::app::save_event(
                    &window, state, sub_board, index, event,
                )
            }),
            "set_calendar_hidden" => call!(command, a, snake {
                hidden: Vec<String>,
            } => {
                crate::commands::app::set_calendar_hidden(
                    &window, state, hidden,
                )
            }),
            "delete_event" => call!(command, a, snake {
                sub_board: u32,
                index: usize,
            } => {
                crate::commands::app::delete_event(
                    &window, state, sub_board, index,
                )
            }),
            _ => return None,
        })
    }
}

mod breakdown_cmds {
    use super::super::{DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "set_sheet_field_order" => call!(command, a, camel {
                fields: Vec<String>,
                #[serde(default)]
                hidden: Option<Vec<String>>,
                #[serde(default)]
                widths: Option<std::collections::BTreeMap<String, String>>,
                #[serde(default)]
                rows: Option<Vec<u8>>,
            } => {
                crate::commands::breakdown::set_sheet_field_order(
                    &window, state, fields, hidden, widths, rows,
                )
            }),
            _ => return None,
        })
    }
}

mod events_cmds {
    use super::super::{DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "move_event_to" => call!(command, a, snake {
                sub_board: u32,
                index: usize,
                date: String,
            } => {
                crate::commands::events::move_event_to(
                    &window, state, sub_board, index, date,
                )
            }),
            _ => return None,
        })
    }
}

mod find_cmds {
    use super::super::{DocState, Error, W};
    use crate::find::*;

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "find_text" => call!(command, a, camel {
                query: QueryArg,
            } => {
                crate::commands::find::find_text(state, query)
            }),
            "replace_text" => call!(command, a, camel {
                query: QueryArg,
                with: String,
                picks: Vec<String>,
            } => {
                crate::commands::find::replace_text(
                    &window, state, query, with, picks,
                )
            }),
            _ => return None,
        })
    }
}

mod flagcolors_cmds {
    use super::super::{DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "set_red_flag_type_color" => call!(command, a, snake {
                red_flag_type: u32,
                color: String,
            } => {
                crate::commands::flagcolors::set_red_flag_type_color(
                    &window,
                    state,
                    red_flag_type,
                    color,
                )
            }),
            _ => return None,
        })
    }
}

mod layouts_cmds {
    use super::super::{DocState, Error, W};
    use crate::layouts::*;

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "add_layout" => call!(command, a, snake {
                name: String,
                kind: String,
                layout: LayoutInput,
                #[serde(default)]
                board: Option<u32>,
            } => {
                crate::commands::layouts::add_layout(
                    &window, state, name, kind, layout, board,
                )
            }),
            "save_layout" => call!(command, a, snake {
                id: u32,
                layout: LayoutInput,
            } => {
                crate::commands::layouts::save_layout(
                    &window, state, id, layout,
                )
            }),
            "rename_layout" => call!(command, a, snake {
                id: u32,
                name: String,
            } => {
                crate::commands::layouts::rename_layout(
                    &window, state, id, name,
                )
            }),
            "move_layout" => call!(command, a, snake {
                id: u32,
                to: usize,
            } => {
                crate::commands::layouts::move_layout(
                    &window, state, id, to,
                )
            }),
            "delete_layout" => call!(command, a, snake {
                id: u32,
            } => {
                crate::commands::layouts::delete_layout(&window, state, id)
            }),
            "use_layout" => call!(command, a, snake {
                board: u32,
                kind: String,
                #[serde(default)]
                id: Option<u32>,
            } => {
                crate::commands::layouts::use_layout(
                    &window, state, board, kind, id,
                )
            }),
            "use_one_line_layout" => call!(command, a, snake {
                #[serde(default)]
                id: Option<u32>,
            } => {
                crate::commands::layouts::use_one_line_layout(
                    &window, state, id,
                )
            }),
            _ => return None,
        })
    }
}

mod plan_cmds {
    use super::super::{to_reply, DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "timeline_data" => call!(command, a, snake {
                sub_board: u32,
                #[serde(default)]
                strip: Option<u32>,
            } => {
                crate::commands::plan::timeline_data(
                    state, sub_board, strip,
                )
            }),
            "push_past_wrap" => call!(command, a, snake {
                sub_board: u32,
                day: u32,
                strips: Vec<u32>,
            } => {
                crate::commands::plan::push_past_wrap(
                    &window, state, sub_board, day, strips,
                )
            }),
            "set_call_time" => call!(command, a, camel {
                day: u32,
                call: String,
            } => {
                crate::commands::plan::set_call_time(
                    &window, state, day, call,
                )
            }),
            "set_day_crew_call" => call!(command, a, camel {
                day: u32,
                department: String,
                value: String,
            } => {
                crate::commands::plan::set_day_crew_call(
                    &window, state, day, department, value,
                )
            }),
            "set_person_call" => call!(command, a, camel {
                day: u32,
                element: u32,
                call: String,
                wrap: String,
            } => {
                crate::commands::plan::set_person_call(
                    &window, state, day, element, call, wrap,
                )
            }),
            "set_shot" => call!(command, a, camel {
                sheet: u32,
                day: u32,
                eighths: String,
                complete: bool,
            } => {
                crate::commands::plan::set_shot(
                    &window, state, sheet, day, eighths, complete,
                )
            }),
            "set_long_day" => call!(command, a, camel {
                value: String,
            } => {
                crate::commands::plan::set_long_day(&window, state, value)
            }),
            "locations_data" => to_reply(crate::commands::plan::locations_data(state)),
            "drafts_data" => to_reply(crate::commands::plan::drafts_data(state)),
            "conflicts_data" => to_reply(crate::commands::plan::conflicts_data(state)),
            "settle_conflicts" => call!(command, a, snake {
                #[serde(default)]
                keys: Option<Vec<String>>,
            } => {
                crate::commands::plan::settle_conflicts(
                    &window, state, keys,
                )
            }),
            "set_location" => call!(command, a, snake {
                name: String,
                latitude: String,
                longitude: String,
                utc_offset: String,
                daylight_saving: String,
                // Left out keeps the location's periods (decision 244).
                #[serde(default)]
                offsets: Option<String>,
            } => {
                crate::commands::plan::set_location(
                    &window,
                    state,
                    name,
                    latitude,
                    longitude,
                    utc_offset,
                    daylight_saving,
                    offsets,
                )
            }),
            "rename_draft" => call!(command, a, snake {
                color: String,
                name: String,
            } => {
                crate::commands::plan::rename_draft(
                    &window, state, color, name,
                )
            }),
            "set_order_weights" => call!(command, a, snake {
                values: Vec<u32>,
            } => {
                crate::commands::plan::set_order_weights(
                    &window, state, values,
                )
            }),
            "set_location_guide" => call!(command, a, snake {
                name: String,
                address: String,
                directions: String,
            } => {
                crate::commands::plan::set_location_guide(
                    &window, state, name, address, directions,
                )
            }),
            "remove_location_map" => call!(command, a, snake {
                name: String,
            } => {
                crate::commands::plan::remove_location_map(
                    &window, state, name,
                )
            }),
            "rest_data" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::plan::rest_data(state, board)
            }),
            "set_turnaround" => call!(command, a, camel {
                category: u32,
                value: String,
            } => {
                crate::commands::plan::set_turnaround(
                    &window, state, category, value,
                )
            }),
            "crew_data" => to_reply(crate::commands::plan::crew_data(state)),
            "set_crew_call" => call!(command, a, camel {
                department: String,
                value: String,
            } => {
                crate::commands::plan::set_crew_call(
                    &window, state, department, value,
                )
            }),
            "locks_data" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::plan::locks_data(state, board)
            }),
            "lock_schedule" => call!(command, a, camel {
                board: u32,
                name: String,
            } => {
                crate::commands::plan::lock_schedule(
                    &window, state, board, name,
                )
            }),
            "delete_lock" => call!(command, a, camel {
                name: String,
            } => {
                crate::commands::plan::delete_lock(&window, state, name)
            }),
            "drift_data" => call!(command, a, camel {
                name: String,
            } => {
                crate::commands::plan::drift_data(state, name)
            }),
            "pin_strips" => call!(command, a, camel {
                strips: Vec<u32>,
                pinned: bool,
            } => {
                crate::commands::plan::pin_strips(
                    &window, state, strips, pinned,
                )
            }),
            "suggest_order" => call!(command, a, snake {
                sub_board: u32,
            } => {
                crate::commands::plan::suggest_order(state, sub_board)
            }),
            "apply_suggested_order" => call!(command, a, snake {
                sub_board: u32,
                order: Vec<u32>,
            } => {
                crate::commands::plan::apply_suggested_order(
                    &window, state, sub_board, order,
                )
            }),
            "call_sheet_data" => call!(command, a, snake {
                sub_board: u32,
                day: u32,
            } => {
                crate::commands::plan::call_sheet_data(
                    state, sub_board, day,
                )
            }),
            "shoot_day_count" => call!(command, a, snake {
                sub_board: u32,
            } => {
                crate::commands::plan::shoot_day_count(state, sub_board)
            }),
            _ => return None,
        })
    }
}

mod reports_cmds {
    use super::super::{DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "report_data" => call!(command, a, snake {
                sub_board: u32,
                index: usize,
                #[serde(default)]
                week_start: Option<String>,
            } => {
                crate::commands::reports::report_data(
                    state, sub_board, index, week_start,
                )
            }),
            _ => return None,
        })
    }
}

mod reportsetup_cmds {
    use super::super::{to_reply, DocState, Error, W};
    use crate::reportsetup::*;

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "report_list" => to_reply(crate::commands::reportsetup::report_list(state)),
            "save_report" => call!(command, a, camel {
                index: usize,
                name: String,
                options: OptionsDto,
            } => {
                crate::commands::reportsetup::save_report(
                    &window, state, index, name, options,
                )
            }),
            "duplicate_report" => call!(command, a, camel {
                index: usize,
            } => {
                crate::commands::reportsetup::duplicate_report(
                    &window, state, index,
                )
            }),
            "move_report" => call!(command, a, camel {
                index: usize,
                to: usize,
            } => {
                crate::commands::reportsetup::move_report(
                    &window, state, index, to,
                )
            }),
            "delete_report" => call!(command, a, camel {
                index: usize,
            } => {
                crate::commands::reportsetup::delete_report(
                    &window, state, index,
                )
            }),
            "save_header_layout" => call!(command, a, camel {
                design: HeaderDto,
            } => {
                crate::commands::reportsetup::save_header_layout(
                    &window, state, design,
                )
            }),
            "delete_header_layout" => call!(command, a, camel {
                id: u32,
            } => {
                crate::commands::reportsetup::delete_header_layout(
                    &window, state, id,
                )
            }),
            "move_header_layout" => call!(command, a, camel {
                id: u32,
                to: usize,
            } => {
                crate::commands::reportsetup::move_header_layout(
                    &window, state, id, to,
                )
            }),
            "set_production" => call!(command, a, camel {
                field: String,
                value: String,
            } => {
                crate::commands::reportsetup::set_production(
                    &window, state, field, value,
                )
            }),
            "set_dood_codes" => call!(command, a, camel {
                codes: std::collections::BTreeMap<String, String>,
            } => {
                crate::commands::reportsetup::set_dood_codes(
                    &window, state, codes,
                )
            }),
            "set_prep_wrap" => call!(command, a, snake {
                sub_board: u32,
                prep_start: String,
                wrap_end: String,
            } => {
                crate::commands::reportsetup::set_prep_wrap(
                    &window, state, sub_board, prep_start, wrap_end,
                )
            }),
            "set_calendar_fields" => call!(command, a, camel {
                fields: Vec<String>,
            } => {
                crate::commands::reportsetup::set_calendar_fields(
                    &window, state, fields,
                )
            }),
            _ => return None,
        })
    }
}

mod split_cmds {
    use super::super::{DocState, Error, W};

    use serde_json::Value;

    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {
        let window = W;
        let _ = &window;
        Some(match command {
            "board_view" => call!(command, a, camel {
                board: u32,
            } => {
                crate::commands::split::board_view(state, board)
            }),
            "remove_sheet_image" => call!(command, a, camel {
                sheet: u32,
                at: usize,
            } => {
                crate::commands::split::remove_sheet_image(
                    &window, state, sheet, at,
                )
            }),
            _ => return None,
        })
    }
}
