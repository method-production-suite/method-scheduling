//! Tools that write. Every one but `new_schedule` takes the `revision` of the read it is
//! based on, makes its change through `method-schedule-core` on a copy, and goes through
//! [`files::edit`]: the copy must pass `Schedule::check` before it is written, atomically.

use method_schedule_core::days::{
    auto_day_breaks as auto_breaks, days as shoot_days, remove_day_breaks as remove_breaks, Limit,
};
use method_schedule_core::find::{find as find_places, label, replace as replace_text, Refusal};
use method_schedule_core::model::{
    Banner, BannerColor, DayKind, ElementId, Event, EventKind, Minutes, Place, RedFlag,
    RedFlagDates, RedFlagId, RedFlagTypeId, Seconds, SheetId, SortField, SortKey, StripId,
    StripKind, SubBoardId, MAX_SORT_KEYS,
};
use method_schedule_core::reports::{set_header_value, HeaderField};
use method_schedule_core::setup::TextSize;
use method_schedule_core::{fdx, fountain, Eighths, Schedule, Weekday};
use serde_json::{json, Map, Value};

use super::super::files::{self, edit, Edited};
use super::super::limits;
use super::super::paths::SCRIPT_EXTS;
use super::super::schema::{array, boolean, integer, name, nullable, object, one_of, path, text};
use super::read::day_kind_name;
use super::{
    board_prop, date_prop, id_prop, location_json, obj, path_prop, pick, pick_board, quoted,
    red_flag_json, revision_prop, search_props, search_query, sub_board_prop, Args, Ctx, Tool,
    ToolError, ToolOut, ToolResult,
};

/// The sheet header fields `set_sheet_fields` takes: text fields, then numbers.
const TEXT_FIELDS: &[&str] = &[
    "number",
    "scenes",
    "int_ext",
    "set",
    "time_of_day",
    "synopsis",
    "location",
    "script_pages",
    "script_day",
    "unit",
    "sequence",
    "episode",
    "comments",
];

pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "new_schedule",
            title: "New schedule",
            description: "Creates an empty schedule (one board with one sub-board, Main, and \
                          a Cast category) in a new .mtsc file. Never replaces a file that \
                          exists. Returns its revision.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        (
                            "path",
                            path("The new file (.mtsc): relative to the first root, or absolute inside a root."),
                        ),
                        ("title", name("The schedule's title. Default \"Untitled\".")),
                    ],
                    &["path"],
                )
            },
            run: new_schedule,
        },
        Tool {
            name: "import_script",
            title: "Import a script",
            description: "Adds a script's scenes to a schedule, one sheet per scene at the end \
                          of every board's Boneyard, with the speaking characters as \
                          elements of a category (the cast category, whatever it is called, \
                          by default; matched by name, new ones created), as the app's Import Script does. The script is a \
                          Fountain (.fountain), .fdx or .mscenes file inside the roots. \
                          Returns the new sheet ids in script order.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        (
                            "script",
                            path("The script file (.fountain or .fdx), inside a root."),
                        ),
                        (
                            "category",
                            name("The category characters go in. Default: the cast category."),
                        ),
                        (
                            "tags",
                            boolean(
                                "Also bring in the script's production tags as elements \
                                 (.fdx only; decisions.md, IMP-04). Default false.",
                            ),
                        ),
                        super::lines_per_page_prop(),
                    ],
                    &["path", "revision", "script"],
                )
            },
            run: import_script,
        },
        Tool {
            name: "set_sheet_fields",
            title: "Set sheet fields",
            description: "Changes a breakdown sheet's header fields: only those given. Text \
                          fields take a string (empty clears it); sunrise and sunset are \
                          minutes after midnight (0 to 1439), eighths the page length (11 \
                          is 1 3/8 pages), shoot_minutes and screen_seconds whole numbers; \
                          null clears an optional number. A one-value field (set, \
                          location, int_ext, time_of_day, unit, sequence, episode, \
                          script_day) takes another sheet's spelling when it matches \
                          ignoring case.",
            mutating: true,
            destructive: false,
            schema: || {
                let mut fields: Vec<(&str, Value)> = TEXT_FIELDS
                    .iter()
                    .map(|f| (*f, text("Text; empty clears it.")))
                    .collect();
                fields.push((
                    "sunrise",
                    nullable(integer("Minutes after midnight.", 0, 1439)),
                ));
                fields.push((
                    "sunset",
                    nullable(integer("Minutes after midnight.", 0, 1439)),
                ));
                fields.push((
                    "eighths",
                    integer("Page length in eighths of a page.", 0, 8 * 100_000),
                ));
                fields.push((
                    "shoot_minutes",
                    nullable(integer("Estimated shoot time in minutes.", 0, 1_000_000)),
                ));
                fields.push((
                    "screen_seconds",
                    nullable(integer("Screen time in seconds.", 0, 1_000_000)),
                ));
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("sheet", id_prop("The sheet id.")),
                        ("fields", object(fields, &[])),
                    ],
                    &["path", "revision", "sheet", "fields"],
                )
            },
            run: set_sheet_fields,
        },
        Tool {
            name: "tag_element",
            title: "Tag an element",
            description: "Puts an element on a breakdown sheet (once; an element already on \
                          it changes nothing), with the elements it links to. Returns \
                          added: the elements newly on the sheet.",
            mutating: true,
            destructive: false,
            schema: tag_schema,
            run: tag_element,
        },
        Tool {
            name: "untag_element",
            title: "Untag an element",
            description: "Takes an element off a breakdown sheet. The element stays in the \
                          schedule.",
            mutating: true,
            destructive: true,
            schema: tag_schema,
            run: untag_element,
        },
        Tool {
            name: "add_element",
            title: "Add an element",
            description: "Adds an element to a category by name (the category is created \
                          when the schedule doesn't have it), with the category's default \
                          rules and, when the category numbers automatically, the next \
                          board ID. Optionally gives it a board ID and puts it on a sheet. \
                          Names are unique in a category, ignoring case.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("category", name("The category, by name (Cast, Props...).")),
                        ("name", name("The element's name.")),
                        (
                            "board_id",
                            name("A board ID (cast number), unique in the category."),
                        ),
                        ("sheet", id_prop("A sheet to put it on.")),
                    ],
                    &["path", "revision", "category", "name"],
                )
            },
            run: add_element,
        },
        Tool {
            name: "set_element_rules",
            title: "Set element rules",
            description: "Changes an element's day out of days switches: holds, drops (acts \
                          only with holds on) and the minimum gap in calendar days that can \
                          become a drop. Only the ones given change.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("element", id_prop("The element id.")),
                        ("holds", boolean("Hold the element between work days.")),
                        ("drops", boolean("Let the longest gap become a drop.")),
                        (
                            "min_drop_gap",
                            integer(
                                "Shortest gap, in calendar days, that may be a drop.",
                                1,
                                3650,
                            ),
                        ),
                    ],
                    &["path", "revision", "element"],
                )
            },
            run: set_element_rules,
        },
        Tool {
            name: "set_element_details",
            title: "Set element details",
            description: "Changes an element's notes, its values of its category's custom \
                          properties (an empty value clears one), the elements it links \
                          to (adding it to a sheet adds them; retroactive also adds them to \
                          every sheet that already holds it), and whether it is left off the \
                          board on its own. Only what is given changes.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("element", id_prop("The element id.")),
                        ("notes", text("The notes; empty clears them.")),
                        (
                            "properties",
                            array(
                                "Property values to set, each a property of the element's \
                                 category by name.",
                                object(
                                    vec![
                                        ("name", name("The property's name.")),
                                        ("value", text("Its value; empty clears it.")),
                                    ],
                                    &["name", "value"],
                                ),
                                100,
                            ),
                        ),
                        (
                            "links",
                            array(
                                "Every element it links to (replaces the list).",
                                id_prop("An element id."),
                                limits::MAX_EVENT_ELEMENTS,
                            ),
                        ),
                        (
                            "retroactive",
                            boolean("Also put the linked elements on sheets that hold it now."),
                        ),
                        (
                            "exclude_from_board",
                            boolean(
                                "Leave this element off the board (no ID on strips, never a \
                                 conflict); its own setting decides.",
                            ),
                        ),
                    ],
                    &["path", "revision", "element"],
                )
            },
            run: set_element_details,
        },
        Tool {
            name: "set_category",
            title: "Set category settings",
            description: "Changes a category's settings: its name (unique, ignoring case; \
                          the cast category too), its position among the user categories \
                          (from 0; the cast category stays first), automatic board IDs, left off the \
                          board (a default for its new elements, which then show on no strip \
                          and never conflict; existing elements keep their own), use in \
                          daily quantity, and its custom element properties (the whole \
                          list, in order; values of a property left out are dropped) and \
                          which of them take several lines. Only what is given changes.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("category", id_prop("The category id.")),
                        ("name", name("A new name.")),
                        (
                            "position",
                            integer(
                                "Its place among the user categories, from 0; past the end is last.",
                                0,
                                10_000,
                            ),
                        ),
                        ("auto_board_id", boolean("Number new elements automatically.")),
                        (
                            "exclude_from_board",
                            boolean("New elements in it start left off the board."),
                        ),
                        (
                            "daily_quantity",
                            boolean("Quantities add up in the daily category totals."),
                        ),
                        (
                            "properties",
                            array(
                                "The custom property names, in order, unique ignoring case.",
                                name("A property name."),
                                100,
                            ),
                        ),
                        (
                            "multi_line",
                            array(
                                "The properties whose values take several lines, by name \
                                 (ignoring case); the others take one line.",
                                name("A property name."),
                                100,
                            ),
                        ),
                    ],
                    &["path", "revision", "category"],
                )
            },
            run: set_category,
        },
        Tool {
            name: "set_quantity",
            title: "Set a quantity",
            description: "How many of an element a breakdown sheet needs (it must be on the \
                          sheet), and whether that number counts in the day's total for its \
                          category (daily, true by default; false for a group already \
                          counted on another sheet that day).",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("sheet", id_prop("The sheet id.")),
                        ("element", id_prop("The element id.")),
                        ("count", integer("How many, at least 1.", 1, 99_999)),
                        ("daily", boolean("Counts in the daily total. Default true.")),
                    ],
                    &["path", "revision", "sheet", "element", "count"],
                )
            },
            run: set_quantity,
        },
        Tool {
            name: "set_board_id",
            title: "Set a board ID",
            description: "Sets or clears (null) an element's board ID, unique in its \
                          category, and optionally its lock (a locked ID survives \
                          renumbering).",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("element", id_prop("The element id.")),
                        (
                            "board_id",
                            nullable(name("The board ID, or null to clear it.")),
                        ),
                        ("locked", boolean("Lock the board ID.")),
                    ],
                    &["path", "revision", "element", "board_id"],
                )
            },
            run: set_board_id,
        },
        Tool {
            name: "move_strips",
            title: "Move strips",
            description: "Moves strips (sheets, banners, day breaks) of one board, in their \
                          current board order, to a place in a sub-board, or to the Boneyard \
                          (to_sub_board null; sheets and parked day breaks, never banners). index counts positions in \
                          the destination before the move, from 0; past the end means last.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        (
                            "strips",
                            json!({
                                "type": "array",
                                "description": "Strip ids (from days).",
                                "items": id_prop("A strip id."),
                                "minItems": 1,
                                "maxItems": limits::MAX_BATCH,
                            }),
                        ),
                        (
                            "to_sub_board",
                            nullable(id_prop(
                                "The sub-board to move them to; null for the Boneyard.",
                            )),
                        ),
                        ("index", integer("Where, from 0.", 0, i64::from(u32::MAX))),
                    ],
                    &["path", "revision", "strips", "to_sub_board", "index"],
                )
            },
            run: move_strips,
        },
        Tool {
            name: "insert_day_break",
            title: "Insert a day break",
            description: "Inserts a day break into a sub-board at index (from 0; past the \
                          end means last). Every day break closes a shoot day. Returns its \
                          strip id.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        ("index", integer("Where, from 0.", 0, i64::from(u32::MAX))),
                    ],
                    &["path", "revision", "index"],
                )
            },
            run: insert_day_break,
        },
        Tool {
            name: "delete_day_break",
            title: "Delete a day break",
            description: "Deletes one day break strip, joining the two days around it. Only \
                          day breaks: a sheet's strip or a banner is refused.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        ("strip", id_prop("The day break's strip id.")),
                    ],
                    &["path", "revision", "strip"],
                )
            },
            run: delete_day_break,
        },
        Tool {
            name: "insert_banner",
            title: "Insert a banner",
            description: "Inserts a banner into a sub-board at index (from 0; past the end \
                          means last), with its text, minutes (counted in the day's time) \
                          and style: bold, color (one of the palette, null for the board's), \
                          tall (two lines high), size and align. Returns its strip id.",
            mutating: true,
            destructive: false,
            schema: || {
                let mut props = vec![
                    path_prop(),
                    revision_prop(),
                    board_prop(),
                    sub_board_prop(),
                    ("index", integer("Where, from 0.", 0, i64::from(u32::MAX))),
                    ("text", banner_text()),
                    ("minutes", banner_minutes()),
                ];
                props.extend(banner_style_props());
                object(props, &["path", "revision", "index", "text"])
            },
            run: insert_banner,
        },
        Tool {
            name: "set_banner",
            title: "Change a banner",
            description: "Changes a banner's text, minutes or style (bold, color, tall, \
                          size, align); what is left out is kept, and color null sets the \
                          board's color. Anything but a banner is refused.",
            mutating: true,
            destructive: false,
            schema: || {
                let mut props = vec![
                    path_prop(),
                    revision_prop(),
                    board_prop(),
                    ("strip", id_prop("The banner's strip id.")),
                    ("text", banner_text()),
                    ("minutes", banner_minutes()),
                ];
                props.extend(banner_style_props());
                object(props, &["path", "revision", "strip"])
            },
            run: set_banner,
        },
        Tool {
            name: "auto_day_breaks",
            title: "Automatic day breaks",
            description: "Replaces every day break on a sub-board with new ones: strips are \
                          added to a day in board order and the day closes as soon as its \
                          total reaches or passes the limit (eighths of a page, minutes of \
                          shoot time, or seconds of screen time). Returns the shoot days.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "by",
                            one_of("What limits a day.", &["eighths", "minutes", "seconds"]),
                        ),
                        (
                            "limit",
                            integer(
                                "The limit per day (40 eighths is 5 pages).",
                                1,
                                limits::MAX_LIMIT,
                            ),
                        ),
                    ],
                    &["path", "revision", "by", "limit"],
                )
            },
            run: auto_day_breaks,
        },
        Tool {
            name: "remove_day_breaks",
            title: "Remove all day breaks",
            description: "Removes every day break on a sub-board, leaving its strips in order \
                          and unscheduled. Returns how many were removed.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                    ],
                    &["path", "revision"],
                )
            },
            run: remove_day_breaks,
        },
        Tool {
            name: "sort_strips",
            title: "Sort strips",
            description: "Sorts a sub-board's strips by up to four header fields, the first \
                          deciding first: banners go to the top in their order, day breaks \
                          are removed (break the days again after), scenes follow; empty \
                          values come last, ties keep their order, scene numbers sort \
                          naturally. Returns how many scene strips were sorted.",
            mutating: true,
            destructive: true,
            schema: || {
                let fields: Vec<&str> = SortField::ALL.iter().map(|f| f.name()).collect();
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "keys",
                            json!({
                                "type": "array",
                                "description": "The fields to sort by, each once.",
                                "items": object(
                                    vec![
                                        ("field", one_of("A header field.", &fields)),
                                        (
                                            "descending",
                                            boolean("Largest first. Default false."),
                                        ),
                                    ],
                                    &["field"],
                                ),
                                "minItems": 1,
                                "maxItems": MAX_SORT_KEYS,
                            }),
                        ),
                    ],
                    &["path", "revision", "keys"],
                )
            },
            run: sort_strips,
        },
        Tool {
            name: "set_start_date",
            title: "Set the start date",
            description: "Sets a sub-board's first possible shoot day (YYYY-MM-DD), or clears \
                          it with null. Dates, the day out of days and the budget exchange \
                          need it.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        ("date", nullable(date_prop("The start date, or null."))),
                    ],
                    &["path", "revision", "date"],
                )
            },
            run: set_start_date,
        },
        Tool {
            name: "set_day_kind",
            title: "Set a day's kind",
            description: "Makes a date on a sub-board's calendar a working day, a day off, a \
                          holiday or a company travel day (move), dropping whatever it was. \
                          On a weekly day off, work marks it back to working.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        ("date", date_prop("The date.")),
                        (
                            "kind",
                            one_of(
                                "What the date becomes.",
                                &["work", "off", "holiday", "move"],
                            ),
                        ),
                    ],
                    &["path", "revision", "date", "kind"],
                )
            },
            run: set_day_kind,
        },
        Tool {
            name: "add_event",
            title: "Add an event",
            description: "Adds an event to a sub-board's calendar: travel, fitting and \
                          rehearsal count in the day out of days; unavailable, camera_test, \
                          the post-production kinds (post_production, visual_effects, \
                          dubbing and the rest) and other (with a label saying what it is) \
                          show on the calendar only. From and to are inclusive and cover at \
                          most 366 days; color is one of ten. Returns its index in the \
                          calendar's list.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "kind",
                            one_of("The kind of event.", &EventKind::keys()),
                        ),
                        (
                            "color",
                            one_of(
                                "One of the ten event colors (none: the kind's usual look).",
                                &method_schedule_core::model::EVENT_COLORS
                                    .iter()
                                    .map(|(k, _, _)| *k)
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                        ("label", name("For other: what kind of event it is.")),
                        ("name", name("A name for the event.")),
                        ("from", date_prop("The first day.")),
                        ("to", date_prop("The last day (inclusive).")),
                        (
                            "elements",
                            array(
                                "Element ids taking part.",
                                id_prop("An element id."),
                                limits::MAX_EVENT_ELEMENTS,
                            ),
                        ),
                    ],
                    &["path", "revision", "kind", "from", "to"],
                )
            },
            run: add_event,
        },
        Tool {
            name: "delete_event",
            title: "Delete an event",
            description: "Deletes event index (from 0, as the calendar lists them) from a \
                          sub-board's calendar; the events after it move up one place.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "index",
                            integer("The event's place in the calendar, from 0.", 0, i64::from(u32::MAX)),
                        ),
                    ],
                    &["path", "revision", "index"],
                )
            },
            run: delete_event,
        },
        Tool {
            name: "set_weekly_off",
            title: "Set the weekly days off",
            description: "Sets the weekdays a sub-board never shoots on (Saturday and Sunday \
                          by default); at least one weekday stays working. Dates already \
                          marked keep their marks; later shoot days move to the next working \
                          dates.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        board_prop(),
                        sub_board_prop(),
                        (
                            "days",
                            array(
                                "The weekly days off, each once; empty for none.",
                                one_of("A weekday.", WEEKDAYS),
                                7,
                            ),
                        ),
                    ],
                    &["path", "revision", "days"],
                )
            },
            run: set_weekly_off,
        },
        Tool {
            name: "replace",
            title: "Replace",
            description: "Replaces the text a find matches by new text in every result (all) \
                          or in the results numbered as find numbers them (picks), with the \
                          same search arguments as find. Refused, with nothing written, when \
                          a rename would give two elements of one category (or two \
                          categories) the same name ignoring case, would leave a name empty, \
                          or would put a line break in a one-line field. The cast category \
                          can be renamed. Returns each place changed, before and after.",
            mutating: true,
            destructive: false,
            schema: || {
                let mut props = vec![path_prop(), revision_prop()];
                props.extend(search_props());
                props.push(("with", text("The replacement; empty removes the text.")));
                props.push(("all", boolean("Replace in every result.")));
                props.push((
                    "picks",
                    array(
                        "Result numbers from find (from 1), instead of all.",
                        integer("A result number.", 1, limits::MAX_PAGE as i64),
                        limits::MAX_BATCH,
                    ),
                ));
                object(props, &["path", "revision", "text", "with"])
            },
            run: replace,
        },
        Tool {
            name: "set_red_flag",
            title: "Set a red flag",
            description: "Adds a red flag to an element, or replaces one (red_flag, its id; \
                          it stays on its element): dates the element can't (hard) or would \
                          rather not (soft) work. Give the dates as from and to (inclusive, \
                          one day when equal), or not_before (every date before it is ruled \
                          out), or not_after (every date after it). The type is an id from \
                          red_flags, or type_name, which adds the type when the schedule \
                          doesn't have it (names ignore case). A warning only: red_flags \
                          lists the clashes. Returns the red flag.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("element", id_prop("The element id.")),
                        (
                            "red_flag",
                            id_prop("A red flag id, to replace that flag; leave out to add."),
                        ),
                        ("type", id_prop("A red flag type id.")),
                        (
                            "type_name",
                            name("A red flag type by name, instead of type; added if new."),
                        ),
                        ("from", date_prop("The first ruled-out day, with to.")),
                        (
                            "to",
                            date_prop("The last ruled-out day (inclusive), with from."),
                        ),
                        (
                            "not_before",
                            date_prop("Rules out every date before this one."),
                        ),
                        (
                            "not_after",
                            date_prop("Rules out every date after this one."),
                        ),
                        ("note", text("A note: why, or what to do.")),
                        (
                            "hard",
                            boolean("A hard limit (can't work) rather than soft. Default false."),
                        ),
                    ],
                    &["path", "revision", "element"],
                )
            },
            run: set_red_flag,
        },
        Tool {
            name: "delete_red_flag",
            title: "Delete a red flag",
            description: "Deletes a red flag by its id (from red_flags). Its type stays.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("red_flag", id_prop("The red flag id.")),
                    ],
                    &["path", "revision", "red_flag"],
                )
            },
            run: delete_red_flag,
        },
        Tool {
            name: "set_red_flag_type_color",
            title: "Color a red flag type",
            description: "Sets the color a red flag type's marks are drawn in, as #rrggbb, \
                          or null for none (the usual red). Returns the type.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("type", id_prop("The red flag type id (from red_flags).")),
                        ("color", nullable(name("#rrggbb, or null for none."))),
                    ],
                    &["path", "revision", "type", "color"],
                )
            },
            run: set_red_flag_type_color,
        },
        Tool {
            name: "add_red_flags",
            title: "Add red flags to several elements and dates",
            description: "Adds one red flag of one type for each element and each run of \
                          dates (behavior note 23): `dates` lists dates picked one by one \
                          (following dates become one range), or give from and to, or \
                          not_before, or not_after. All or nothing. Returns the new flags.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        (
                            "elements",
                            array(
                                "The element ids to flag.",
                                id_prop("An element id."),
                                limits::MAX_BATCH,
                            ),
                        ),
                        ("type", id_prop("A red flag type id.")),
                        (
                            "type_name",
                            name("A red flag type by name, instead of type; added if new."),
                        ),
                        (
                            "dates",
                            array(
                                "Dates picked one by one; runs of following dates join.",
                                date_prop("A ruled-out date."),
                                limits::MAX_BATCH,
                            ),
                        ),
                        ("from", date_prop("The first ruled-out day, with to.")),
                        (
                            "to",
                            date_prop("The last ruled-out day (inclusive), with from."),
                        ),
                        (
                            "not_before",
                            date_prop("Rules out every date before this one."),
                        ),
                        (
                            "not_after",
                            date_prop("Rules out every date after this one."),
                        ),
                        ("note", text("A note: why, or what to do.")),
                        (
                            "hard",
                            boolean("A hard limit (can't work) rather than soft. Default false."),
                        ),
                    ],
                    &["path", "revision", "elements"],
                )
            },
            run: add_red_flags,
        },
        Tool {
            name: "add_standard_red_flag_types",
            title: "Add the standard red flag types",
            description: "Adds the standard red flag types the schedule lacks, one per event \
                          kind but other (decision 113): travel, fitting, rehearsal, \
                          unavailable, camera test and the post-production kinds. A type of \
                          the schedule's own with the same name becomes the standard one. \
                          Standard types can be renamed and recolored, never deleted. \
                          Returns how many were added or marked.",
            mutating: true,
            destructive: false,
            schema: || object(vec![path_prop(), revision_prop()], &["path", "revision"]),
            run: add_standard_red_flag_types,
        },
        Tool {
            name: "merge_sheets",
            title: "Merge sheets",
            description: "Merges breakdown sheets into one (merge strips). The survivor keeps \
                          its id and its place on every board; the others are deleted and \
                          their strips leave every board (banners and day breaks stay). \
                          Scenes, synopses and comments are joined and elements unioned in \
                          the order given (the survivor first, then others as listed); page \
                          lengths, shoot times and screen times summed. Every other field \
                          keeps the survivor's value (or, when it is empty, the first in that \
                          order); the others' values \
                          are returned as dropped. Refused: the survivor among the others, \
                          a sheet twice, an unknown sheet.",
            mutating: true,
            destructive: true,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("survivor", id_prop("The sheet that stays.")),
                        (
                            "others",
                            json!({
                                "type": "array",
                                "description": "The sheet ids merged into the survivor and deleted.",
                                "items": id_prop("A sheet id."),
                                "minItems": 1,
                                "maxItems": limits::MAX_BATCH,
                            }),
                        ),
                    ],
                    &["path", "revision", "survivor", "others"],
                )
            },
            run: merge_sheets,
        },
        Tool {
            name: "duplicate_sheet",
            title: "Duplicate a sheet",
            description: "Copies a breakdown sheet, every header field and element, to a new \
                          sheet (to split a scene in two). On each board where the original \
                          is on a sub-board the copy goes right after it; where it is in the \
                          Boneyard, at the Boneyard's end. Returns the new sheet id and where \
                          its strip is on each board.",
            mutating: true,
            destructive: false,
            schema: || {
                object(
                    vec![
                        path_prop(),
                        revision_prop(),
                        ("sheet", id_prop("The sheet to copy.")),
                    ],
                    &["path", "revision", "sheet"],
                )
            },
            run: duplicate_sheet,
        },
    ]
}

fn tag_schema() -> Value {
    object(
        vec![
            path_prop(),
            revision_prop(),
            ("sheet", id_prop("The sheet id.")),
            ("element", id_prop("The element id.")),
        ],
        &["path", "revision", "sheet", "element"],
    )
}

/// The result every change returns: the path, whether anything changed, the new
/// revision, with `extra` merged in.
pub(super) fn done<T>(e: &Edited<T>, what: &str, mut extra: Map<String, Value>) -> ToolOut {
    extra.insert("path".into(), json!(e.shown));
    extra.insert("changed".into(), json!(e.changed));
    extra.insert("revision".into(), json!(e.revision));
    let summary = if e.changed {
        format!("{what} in {}. Revision {}.", e.shown, e.revision)
    } else {
        format!("{what}: nothing changed in {}; nothing written.", e.shown)
    };
    ToolOut {
        summary,
        data: Value::Object(extra),
    }
}

// ---- the tools

fn new_schedule(ctx: &Ctx, a: &Args) -> ToolResult {
    let title = a.str("title").unwrap_or("Untitled");
    let s = Schedule::with_default_categories(title);
    let (shown, revision) = files::create(ctx, a, &s)?;
    Ok(ToolOut {
        summary: format!("Created {shown} ({}). Revision {revision}.", quoted(title)),
        data: json!({
            "path": shown,
            "changed": true,
            "revision": revision,
            "board": s.boards[0].id.0,
            "sub_board": s.boards[0].sub_boards[0].id.0,
        }),
    })
}

fn import_script(ctx: &Ctx, a: &Args) -> ToolResult {
    let typed = a.req("script")?;
    let real = ctx.roots.existing(typed, SCRIPT_EXTS)?;
    let script_shown = ctx.roots.show(&real);
    let bytes = files::read_file(&real, &script_shown)?;
    let text = files::text(&bytes, &script_shown)?;
    let is_fdx = real
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fdx"));
    let tags = a.opt_bool("tags").unwrap_or(false);
    if tags && !is_fdx {
        return Err(ToolError::invalid(
            "production tags come only from .fdx files",
        ));
    }
    let script = if is_fdx && tags {
        fdx::parse_with_tags(&text)
            .map_err(|e| ToolError::invalid(format!("{script_shown}: {e}")))?
    } else if is_fdx {
        fdx::parse(&text).map_err(|e| ToolError::invalid(format!("{script_shown}: {e}")))?
    } else {
        fountain::parse_with(&text, super::lines_per_page(a))
    };
    let category = a.str("category").map(str::to_string);
    let e = edit(ctx, a, |s| {
        let cast = match &category {
            Some(c) => s.add_category(c),
            None => s.cast_or_add(),
        };
        if tags {
            Ok(method_schedule_core::revise::import_tagged(
                s, &script, cast,
            )?)
        } else {
            Ok(s.import_script(&script, cast)?)
        }
    })?;
    let total: Eighths = script.scenes.iter().map(|x| x.length).sum();
    Ok(done(
        &e,
        &format!(
            "Added {} sheet(s), {total} pages, from {script_shown}",
            e.out.len()
        ),
        obj(vec![
            (
                "sheets",
                json!(e.out.iter().map(|x| x.0).collect::<Vec<_>>()),
            ),
            ("eighths", json!(total.0)),
        ]),
    ))
}

fn set_sheet_fields(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = SheetId(a.req_u32("sheet")?);
    let fields = a
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(|| ToolError::invalid("`fields` is required"))?;
    if fields.is_empty() {
        return Err(ToolError::invalid("give at least one field to change"));
    }
    let f = Args(fields);
    let e = edit(ctx, a, |s| {
        if s.sheet(id).is_none() {
            return Err(ToolError::not_found(format!("no sheet {}", id.0)));
        }
        for name in TEXT_FIELDS {
            if let Some(v) = f.str(name) {
                // A one-value field takes the spelling another sheet already has when it
                // matches ignoring case (decisions.md, entry 155).
                if let Some(field) = HeaderField::from_key(name) {
                    set_header_value(s, id, field, v);
                    continue;
                }
                let h = s
                    .sheet_mut(id)
                    .ok_or_else(|| ToolError::not_found(format!("no sheet {}", id.0)))?;
                let slot = match *name {
                    "number" => &mut h.number,
                    "scenes" => &mut h.scenes,
                    "int_ext" => &mut h.int_ext,
                    "set" => &mut h.set,
                    "time_of_day" => &mut h.time_of_day,
                    "synopsis" => &mut h.synopsis,
                    "location" => &mut h.location,
                    "script_pages" => &mut h.script_pages,
                    "script_day" => &mut h.script_day,
                    "unit" => &mut h.unit,
                    "sequence" => &mut h.sequence,
                    "episode" => &mut h.episode,
                    _ => &mut h.comments,
                };
                *slot = v.to_string();
            }
        }
        let h = s
            .sheet_mut(id)
            .ok_or_else(|| ToolError::not_found(format!("no sheet {}", id.0)))?;
        if f.has("sunrise") {
            h.sunrise = f.u32("sunrise");
        }
        if f.has("sunset") {
            h.sunset = f.u32("sunset");
        }
        if let Some(n) = f.u32("eighths") {
            h.pages = Eighths(n);
        }
        if f.has("shoot_minutes") {
            h.shoot_time = f.u32("shoot_minutes").map(Minutes);
        }
        if f.has("screen_seconds") {
            h.screen_time = f.u32("screen_seconds").map(Seconds);
        }
        Ok(())
    })?;
    let names: Vec<&str> = fields.keys().map(String::as_str).collect();
    Ok(done(
        &e,
        &format!("Set {} on sheet {}", names.join(", "), id.0),
        obj(vec![("sheet", json!(id.0))]),
    ))
}

fn tag_element(ctx: &Ctx, a: &Args) -> ToolResult {
    let sheet = SheetId(a.req_u32("sheet")?);
    let element = ElementId(a.req_u32("element")?);
    let e = edit(ctx, a, |s| Ok(s.tag_with_links(sheet, element)?))?;
    Ok(done(
        &e,
        &format!("Tagged element {} on sheet {}", element.0, sheet.0),
        obj(vec![
            ("sheet", json!(sheet.0)),
            ("element", json!(element.0)),
            (
                "added",
                json!(e.out.iter().map(|x| x.0).collect::<Vec<_>>()),
            ),
        ]),
    ))
}

fn untag_element(ctx: &Ctx, a: &Args) -> ToolResult {
    let sheet = SheetId(a.req_u32("sheet")?);
    let element = ElementId(a.req_u32("element")?);
    let e = edit(ctx, a, |s| Ok(s.untag(sheet, element)?))?;
    Ok(done(
        &e,
        &format!("Took element {} off sheet {}", element.0, sheet.0),
        obj(vec![
            ("sheet", json!(sheet.0)),
            ("element", json!(element.0)),
        ]),
    ))
}

fn add_element(ctx: &Ctx, a: &Args) -> ToolResult {
    let category = a.req("category")?.to_string();
    let el_name = a.req("name")?.to_string();
    let board_id = a.str("board_id").map(str::to_string);
    let sheet = a.u32("sheet").map(SheetId);
    let e = edit(ctx, a, |s| {
        // The category name is not checked here: an unusable one (blank) is caught by
        // `Schedule::check` before anything is written.
        let created = s.category_named(&category).is_none();
        let cat = s.add_category(&category);
        let id = s.add_element(cat, &el_name)?;
        if let Some(b) = &board_id {
            s.set_board_id(id, Some(b))?;
        }
        if let Some(sh) = sheet {
            s.tag_with_links(sh, id)?;
        }
        Ok((id, cat, created))
    })?;
    let (id, cat, created) = e.out;
    let board_id = e.schedule.element(id).and_then(|x| x.board_id.clone());
    Ok(done(
        &e,
        &format!("Added element {} {}", id.0, quoted(&el_name)),
        obj(vec![
            ("element", json!(id.0)),
            ("category", json!(cat.0)),
            ("category_created", json!(created)),
            ("board_id", json!(board_id)),
        ]),
    ))
}

fn set_element_rules(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = ElementId(a.req_u32("element")?);
    let (holds, drops, gap) = (
        a.opt_bool("holds"),
        a.opt_bool("drops"),
        a.u32("min_drop_gap"),
    );
    let e = edit(ctx, a, |s| {
        let el = s
            .element_mut(id)
            .ok_or_else(|| ToolError::not_found(format!("no element {}", id.0)))?;
        if let Some(h) = holds {
            el.rules.holds = h;
        }
        if let Some(d) = drops {
            el.rules.drops = d;
        }
        if let Some(g) = gap {
            el.rules.min_drop_gap = g;
        }
        Ok(el.rules)
    })?;
    let r = e.out;
    Ok(done(
        &e,
        &format!("Set the rules of element {}", id.0),
        obj(vec![
            ("element", json!(id.0)),
            (
                "rules",
                json!({"holds": r.holds, "drops": r.drops, "min_drop_gap": r.min_drop_gap}),
            ),
        ]),
    ))
}

fn set_element_details(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = ElementId(a.req_u32("element")?);
    let notes = a.str("notes").map(str::to_string);
    let props: Vec<(String, String)> = a
        .list("properties")
        .iter()
        .filter_map(|p| {
            Some((
                p.get("name")?.as_str()?.to_string(),
                p.get("value")?.as_str()?.to_string(),
            ))
        })
        .collect();
    let links = a.has("links").then(|| {
        a.u32s("links")
            .into_iter()
            .map(ElementId)
            .collect::<Vec<_>>()
    });
    let retroactive = a.opt_bool("retroactive").unwrap_or(false);
    let excluded = a.opt_bool("exclude_from_board");
    let e = edit(ctx, a, |s| {
        s.element(id)
            .ok_or_else(|| ToolError::not_found(format!("no element {}", id.0)))?;
        if let Some(n) = &notes {
            s.element_mut(id).expect("checked above").notes = n.trim_end().to_string();
        }
        for (n, v) in &props {
            s.set_property_value(id, n, v)?;
        }
        if let Some(x) = excluded {
            s.element_mut(id).expect("checked above").exclude_from_board = x;
        }
        let mut sheets = 0;
        if let Some(l) = &links {
            sheets = s.set_links(id, l, retroactive)?;
        }
        Ok(sheets)
    })?;
    let el = e.schedule.element(id).expect("edited");
    Ok(done(
        &e,
        &format!("Set the details of element {}", id.0),
        obj(vec![
            ("element", json!(id.0)),
            ("notes", json!(el.notes)),
            (
                "properties",
                json!(el
                    .properties
                    .iter()
                    .map(|p| json!({"name": p.name, "value": p.value}))
                    .collect::<Vec<_>>()),
            ),
            (
                "links",
                json!(el.links.iter().map(|x| x.0).collect::<Vec<_>>()),
            ),
            ("exclude_from_board", json!(el.exclude_from_board)),
            ("excluded", json!(e.schedule.element_excluded(el))),
            ("sheets_gained", json!(e.out)),
        ]),
    ))
}

fn set_category(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = method_schedule_core::model::CategoryId(a.req_u32("category")?);
    let (auto, excluded, daily) = (
        a.opt_bool("auto_board_id"),
        a.opt_bool("exclude_from_board"),
        a.opt_bool("daily_quantity"),
    );
    let props: Option<Vec<String>> = a.has("properties").then(|| {
        a.list("properties")
            .iter()
            .filter_map(|v| v.as_str().map(|x| x.trim().to_string()))
            .collect()
    });
    let multi: Option<Vec<String>> = a.has("multi_line").then(|| {
        a.list("multi_line")
            .iter()
            .filter_map(|v| v.as_str().map(|x| x.trim().to_string()))
            .collect()
    });
    let rename = a.str("name").map(str::to_string);
    let position = a.u32("position");
    let e = edit(ctx, a, |s| {
        s.category(id)
            .ok_or_else(|| ToolError::not_found(format!("no category {}", id.0)))?;
        if let Some(n) = &rename {
            s.rename_category(id, n)?;
        }
        if let Some(to) = position {
            s.move_category(id, to as usize)?;
        }
        let c = s.category_mut(id).expect("checked above");
        if let Some(x) = auto {
            c.auto_board_id = x;
        }
        if let Some(x) = excluded {
            c.exclude_from_board = x;
        }
        if let Some(x) = daily {
            c.daily_quantity = x;
        }
        if let Some(list) = &props {
            // Kept by name (ignoring case): the old values of a property kept stay.
            let old = c.properties.clone();
            let keep: Vec<String> = old
                .iter()
                .filter(|o| list.iter().any(|n| n.eq_ignore_ascii_case(o)))
                .cloned()
                .collect();
            for (i, o) in old.iter().enumerate().rev() {
                if !keep.contains(o) {
                    s.delete_property(id, i)?;
                }
            }
            for n in list {
                if !keep.iter().any(|k| k.eq_ignore_ascii_case(n)) {
                    s.add_property(id, n)?;
                }
            }
            for (to, n) in list.iter().enumerate() {
                let now = &s.category(id).expect("checked above").properties;
                let at = now
                    .iter()
                    .position(|p| p.eq_ignore_ascii_case(n))
                    .expect("added above");
                if at != to {
                    s.move_property(id, at, to)?;
                }
                if s.category(id).expect("checked above").properties[to] != *n {
                    s.rename_property(id, to, n)?;
                }
            }
        }
        if let Some(list) = &multi {
            let names = s.category(id).expect("checked above").properties.clone();
            if let Some(n) = list
                .iter()
                .find(|n| !names.iter().any(|p| p.eq_ignore_ascii_case(n)))
            {
                return Err(method_schedule_core::model::Error::Broken(format!(
                    "the category has no property {n:?}"
                ))
                .into());
            }
            for (at, p) in names.iter().enumerate() {
                let on = list.iter().any(|n| n.eq_ignore_ascii_case(p));
                s.set_property_multi_line(id, at, on)?;
            }
        }
        Ok(())
    })?;
    let c = e.schedule.category(id).expect("edited");
    Ok(done(
        &e,
        &format!("Set the settings of category {}", quoted(&c.name)),
        obj(vec![
            ("category", json!(id.0)),
            ("name", json!(c.name)),
            ("auto_board_id", json!(c.auto_board_id)),
            ("exclude_from_board", json!(c.exclude_from_board)),
            ("daily_quantity", json!(c.daily_quantity)),
            ("properties", json!(c.properties)),
            ("multi_line", json!(c.multi_line)),
        ]),
    ))
}

fn set_quantity(ctx: &Ctx, a: &Args) -> ToolResult {
    let sheet = SheetId(a.req_u32("sheet")?);
    let element = ElementId(a.req_u32("element")?);
    let count = a.req_u32("count")?;
    let daily = a.opt_bool("daily").unwrap_or(true);
    let e = edit(
        ctx,
        a,
        |s| Ok(s.set_quantity(sheet, element, count, daily)?),
    )?;
    Ok(done(
        &e,
        &format!("Set {count} of element {} on sheet {}", element.0, sheet.0),
        obj(vec![
            ("sheet", json!(sheet.0)),
            ("element", json!(element.0)),
            ("count", json!(count)),
            ("daily", json!(daily)),
        ]),
    ))
}

fn set_board_id(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = ElementId(a.req_u32("element")?);
    let board_id = a.str("board_id").map(str::to_string);
    let locked = a.opt_bool("locked");
    let e = edit(ctx, a, |s| {
        s.set_board_id(id, board_id.as_deref())?;
        if let Some(l) = locked {
            s.element_mut(id)
                .expect("checked by set_board_id")
                .board_id_locked = l;
        }
        Ok(())
    })?;
    Ok(done(
        &e,
        &format!("Set the board ID of element {}", id.0),
        obj(vec![
            ("element", json!(id.0)),
            ("board_id", json!(board_id)),
        ]),
    ))
}

fn move_strips(ctx: &Ctx, a: &Args) -> ToolResult {
    let strips: Vec<StripId> = a.u32s("strips").into_iter().map(StripId).collect();
    let to = a.u32("to_sub_board").map(SubBoardId);
    let index = a.u64("index").unwrap_or(0) as usize;
    let e = edit(ctx, a, |s| {
        let board = pick_board(s, a)?;
        s.move_strips(
            board,
            &strips,
            Place {
                sub_board: to,
                index,
            },
        )?;
        Ok(board)
    })?;
    Ok(done(
        &e,
        &format!(
            "Moved {} strip(s) to {}",
            strips.len(),
            to.map_or("the Boneyard".to_string(), |x| format!("sub-board {}", x.0))
        ),
        obj(vec![
            ("board", json!(e.out.0)),
            ("moved", json!(strips.len())),
        ]),
    ))
}

fn insert_day_break(ctx: &Ctx, a: &Args) -> ToolResult {
    let index = a.u64("index").unwrap_or(0) as usize;
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        Ok(s.insert_strip(
            b,
            StripKind::DayBreak,
            Place {
                sub_board: Some(sb),
                index,
            },
        )?)
    })?;
    Ok(done(
        &e,
        &format!("Inserted day break {}", e.out.0),
        obj(vec![("strip", json!(e.out.0))]),
    ))
}

fn banner_text() -> Value {
    json!({"type": "string", "description": "The banner's text.", "minLength": 1,
        "maxLength": crate::banners::MAX_TEXT})
}

fn banner_minutes() -> Value {
    integer(
        "How long it lasts, in minutes (counts toward the day's time).",
        0,
        i64::from(crate::banners::MAX_MINUTES),
    )
}

/// A banner's style arguments (decisions.md, entries 65 and 87).
fn banner_style_props() -> Vec<(&'static str, Value)> {
    let names = BannerColor::NAMED
        .map(|c| c.name().unwrap_or_default())
        .join(", ");
    vec![
        ("bold", boolean("Bold text.")),
        (
            "color",
            nullable(text(&format!(
                "A palette name ({names}) or any color as lowercase #rrggbb; null for the board's."
            ))),
        ),
        ("tall", boolean("Two lines high instead of one.")),
        (
            "size",
            one_of("Text size.", &TextSize::ALL.map(TextSize::name)),
        ),
        (
            "align",
            one_of("Where the text sits.", &crate::banners::ALIGNS),
        ),
        (
            "font",
            one_of(
                "The font family: usual (the board's) or one every system has.",
                &method_schedule_core::model::BannerFont::ALL
                    .map(method_schedule_core::model::BannerFont::name),
            ),
        ),
    ]
}

/// The style arguments given, as a change.
fn banner_style_change<'a>(a: &'a Args) -> crate::banners::StyleChange<'a> {
    crate::banners::StyleChange {
        bold: a.opt_bool("bold"),
        color: a.has("color").then(|| a.str("color")),
        tall: a.opt_bool("tall"),
        size: a.str("size"),
        align: a.str("align"),
        font: a.str("font"),
    }
}

fn banner_json(id: StripId, b: &Banner) -> Map<String, Value> {
    obj(vec![
        ("strip", json!(id.0)),
        ("text", json!(b.text)),
        ("minutes", json!(b.duration.0)),
        ("style", crate::banners::style_json(&b.style)),
    ])
}

fn insert_banner(ctx: &Ctx, a: &Args) -> ToolResult {
    let index = a.u64("index").unwrap_or(0) as usize;
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let mut banner = Banner {
            text: crate::banners::text(a.req("text")?).map_err(ToolError::invalid)?,
            duration: crate::banners::minutes(a.u32("minutes").unwrap_or(0))
                .map_err(ToolError::invalid)?,
            style: Default::default(),
        };
        banner_style_change(a)
            .apply(&mut banner.style)
            .map_err(ToolError::invalid)?;
        let id = s.insert_strip(
            b,
            StripKind::Banner(banner.clone()),
            Place {
                sub_board: Some(sb),
                index,
            },
        )?;
        Ok((id, banner))
    })?;
    let (id, b) = &e.out;
    Ok(done(
        &e,
        &format!("Inserted banner {}", id.0),
        banner_json(*id, b),
    ))
}

fn set_banner(ctx: &Ctx, a: &Args) -> ToolResult {
    let strip = StripId(a.req_u32("strip")?);
    let e = edit(ctx, a, |s| {
        let board = pick_board(s, a)?;
        let b = s.board(board).expect("picked");
        let kind = b
            .sub_boards
            .iter()
            .flat_map(|x| &x.strips)
            .chain(&b.boneyard)
            .find(|st| st.id == strip)
            .map(|st| &st.kind)
            .ok_or_else(|| {
                ToolError::not_found(format!("board {} has no strip {}", board.0, strip.0))
            })?;
        let StripKind::Banner(old) = kind else {
            return Err(ToolError::refused(format!(
                "strip {} is not a banner; nothing written",
                strip.0
            )));
        };
        let mut banner = old.clone();
        if let Some(t) = a.str("text") {
            banner.text = crate::banners::text(t).map_err(ToolError::invalid)?;
        }
        if let Some(m) = a.u32("minutes") {
            banner.duration = crate::banners::minutes(m).map_err(ToolError::invalid)?;
        }
        banner_style_change(a)
            .apply(&mut banner.style)
            .map_err(ToolError::invalid)?;
        s.set_banner(board, strip, banner.clone())?;
        Ok(banner)
    })?;
    Ok(done(
        &e,
        &format!("Changed banner {}", strip.0),
        banner_json(strip, &e.out),
    ))
}

fn delete_day_break(ctx: &Ctx, a: &Args) -> ToolResult {
    let strip = StripId(a.req_u32("strip")?);
    let e = edit(ctx, a, |s| {
        let board = pick_board(s, a)?;
        let b = s.board(board).expect("picked");
        let kind = b
            .sub_boards
            .iter()
            .flat_map(|x| &x.strips)
            .chain(&b.boneyard)
            .find(|st| st.id == strip)
            .map(|st| &st.kind)
            .ok_or_else(|| {
                ToolError::not_found(format!("board {} has no strip {}", board.0, strip.0))
            })?;
        if *kind != StripKind::DayBreak {
            return Err(ToolError::refused(format!(
                "strip {} is not a day break; nothing written",
                strip.0
            )));
        }
        Ok(s.delete_strip(board, strip)?)
    })?;
    Ok(done(
        &e,
        &format!("Deleted day break {}", strip.0),
        obj(vec![("strip", json!(strip.0))]),
    ))
}

fn auto_day_breaks(ctx: &Ctx, a: &Args) -> ToolResult {
    let n = a.req_u32("limit")?;
    let limit = match a.req("by")? {
        "eighths" => Limit::Pages(Eighths(n)),
        "minutes" => Limit::Time(Minutes(n)),
        _ => Limit::ScreenTime(Seconds(n)),
    };
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        auto_breaks(s, b, sb, limit)?;
        Ok(shoot_days(s, b, sb)?.days.len())
    })?;
    Ok(done(
        &e,
        &format!("Day breaks set: {} shoot day(s)", e.out),
        obj(vec![("shoot_days", json!(e.out))]),
    ))
}

fn remove_day_breaks(ctx: &Ctx, a: &Args) -> ToolResult {
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        Ok(remove_breaks(s, b, sb)?)
    })?;
    Ok(done(
        &e,
        &format!("Removed {} day break(s)", e.out),
        obj(vec![("removed", json!(e.out))]),
    ))
}

fn sort_strips(ctx: &Ctx, a: &Args) -> ToolResult {
    let list = a.list("keys");
    if list.is_empty() || list.len() > MAX_SORT_KEYS {
        return Err(ToolError::invalid(format!(
            "keys: give 1 to {MAX_SORT_KEYS} fields"
        )));
    }
    let mut keys: Vec<SortKey> = Vec::with_capacity(list.len());
    for k in list {
        let name = k.get("field").and_then(Value::as_str).unwrap_or("");
        let field = SortField::from_name(name)
            .ok_or_else(|| ToolError::invalid(format!("keys: unknown field {}", quoted(name))))?;
        if keys.iter().any(|x| x.field == field) {
            return Err(ToolError::invalid(format!(
                "keys: {} is given twice",
                quoted(name)
            )));
        }
        let descending = k
            .get("descending")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        keys.push(SortKey { field, descending });
    }
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        s.sort_strips(b, Some(sb), &keys)?;
        let board = s.board(b).expect("picked");
        let sub = board
            .sub_boards
            .iter()
            .find(|x| x.id == sb)
            .expect("picked");
        Ok(sub
            .strips
            .iter()
            .filter(|st| matches!(st.kind, StripKind::Sheet(_)))
            .count())
    })?;
    Ok(done(
        &e,
        &format!("Sorted {} scene strip(s)", e.out),
        obj(vec![("sorted", json!(e.out))]),
    ))
}

/// The sub-board `pick` chooses, to change.
fn sub_board_mut<'a>(
    s: &'a mut Schedule,
    a: &Args,
) -> Result<&'a mut method_schedule_core::model::SubBoard, ToolError> {
    let (b, sb) = pick(s, a)?;
    Ok(s.boards
        .iter_mut()
        .find(|x| x.id == b)
        .and_then(|x| x.sub_boards.iter_mut().find(|y| y.id == sb))
        .expect("picked"))
}

fn set_start_date(ctx: &Ctx, a: &Args) -> ToolResult {
    let date = a.date("date")?;
    let e = edit(ctx, a, |s| {
        sub_board_mut(s, a)?.start = date;
        Ok(())
    })?;
    Ok(done(
        &e,
        &match date {
            Some(d) => format!("Start date set to {d}"),
            None => "Start date cleared".to_string(),
        },
        obj(vec![("date", super::date_json(date))]),
    ))
}

fn set_day_kind(ctx: &Ctx, a: &Args) -> ToolResult {
    let date = a.req_date("date")?;
    let kind = match a.req("kind")? {
        "work" => DayKind::Work,
        "off" => DayKind::Off,
        "holiday" => DayKind::Holiday,
        _ => DayKind::Move,
    };
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let cal = s.calendar_mut(b, sb)?;
        cal.set_day_kind(date, kind);
        Ok(cal.day_kind(date))
    })?;
    Ok(done(
        &e,
        &format!("{date} is now {}", day_kind_name(e.out)),
        obj(vec![
            ("date", json!(date.to_string())),
            ("kind", json!(day_kind_name(e.out))),
        ]),
    ))
}

fn add_event(ctx: &Ctx, a: &Args) -> ToolResult {
    let from = a.req_date("from")?;
    let to = a.req_date("to")?;
    if to.0 - from.0 >= limits::MAX_EVENT_DAYS {
        return Err(ToolError::invalid(format!(
            "an event runs at most {} days; {from} to {to} is longer",
            limits::MAX_EVENT_DAYS
        )));
    }
    let kind = EventKind::from_key(a.req("kind")?, a.str("label").unwrap_or_default())
        .ok_or_else(|| ToolError::invalid("not an event kind"))?;
    let event = Event {
        name: a.str("name").unwrap_or_default().to_string(),
        kind,
        from,
        to,
        elements: a.u32s("elements").into_iter().map(ElementId).collect(),
        color: match a.str("color") {
            None => None,
            Some(k) => Some(
                method_schedule_core::model::event_color(k)
                    .ok_or_else(|| ToolError::invalid(format!("{k:?} is not an event color")))?,
            ),
        },
    };
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        Ok(s.set_event(b, sb, None, event)?)
    })?;
    Ok(done(
        &e,
        &format!("Added event {} ({from} to {to})", e.out),
        obj(vec![("index", json!(e.out))]),
    ))
}

fn delete_event(ctx: &Ctx, a: &Args) -> ToolResult {
    let index = a.req_u32("index")? as usize;
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let title = s
            .calendar_mut(b, sb)?
            .events
            .get(index)
            .map(Event::title)
            .ok_or_else(|| ToolError::not_found(format!("no event {index} on that calendar")))?;
        s.delete_event(b, sb, index)?;
        Ok(title)
    })?;
    Ok(done(
        &e,
        &format!("Deleted event {index} ({})", e.out),
        obj(vec![("index", json!(index)), ("deleted", json!(e.out))]),
    ))
}

/// The weekdays as `set_weekly_off` takes them, Monday first.
const WEEKDAYS: &[&str] = &["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn set_weekly_off(ctx: &Ctx, a: &Args) -> ToolResult {
    let mut off = Vec::new();
    for v in a.list("days") {
        let key = v.as_str().unwrap_or_default();
        let at = WEEKDAYS
            .iter()
            .position(|w| *w == key)
            .ok_or_else(|| ToolError::invalid(format!("{key:?} is not a weekday")))?;
        off.push(Weekday::ALL[at]);
    }
    let e = edit(ctx, a, |s| {
        let (b, sb) = pick(s, a)?;
        let cal = s.calendar_mut(b, sb)?;
        cal.set_weekly_off(&off)?;
        Ok(cal
            .weekly_off
            .iter()
            .map(|w| WEEKDAYS[Weekday::ALL.iter().position(|x| x == w).unwrap_or(0)])
            .collect::<Vec<_>>())
    })?;
    Ok(done(
        &e,
        &if e.out.is_empty() {
            "No weekly days off".to_string()
        } else {
            format!("Weekly days off: {}", e.out.join(", "))
        },
        obj(vec![("days", json!(e.out))]),
    ))
}

fn replace(ctx: &Ctx, a: &Args) -> ToolResult {
    let with = a.str("with").unwrap_or_default().to_string();
    let all = a.opt_bool("all").unwrap_or(false);
    let picks = a.u32s("picks");
    if all != picks.is_empty() {
        return Err(ToolError::invalid(
            "give all: true, or picks (result numbers from find), not both",
        ));
    }
    let e = edit(ctx, a, |s| {
        let q = search_query(s, a)?;
        let hits = find_places(s, &q)?;
        let chosen = if all {
            hits.iter().map(|h| h.location).collect::<Vec<_>>()
        } else {
            picks
                .iter()
                .map(|&n| {
                    hits.get(n as usize - 1).map(|h| h.location).ok_or_else(|| {
                        ToolError::invalid(format!(
                            "picks: no result {n} (find gives {})",
                            hits.len()
                        ))
                    })
                })
                .collect::<Result<_, _>>()?
        };
        if chosen.is_empty() {
            return Ok(Vec::new());
        }
        let r = replace_text(s, &q, &with, &chosen).map_err(|e| match e {
            Refusal::Scope(e) => ToolError::from(e),
            other => ToolError::refused(format!("{other}; nothing written")),
        })?;
        let changes: Vec<Value> = r
            .changes
            .iter()
            .map(|c| {
                let mut v = location_json(&c.location);
                v["place"] = json!(label(&r.schedule, &c.location));
                v["before"] = json!(c.before);
                v["after"] = json!(c.after);
                v
            })
            .collect();
        *s = r.schedule;
        Ok(changes)
    })?;
    let n = e.out.len();
    Ok(done(
        &e,
        &format!("Replaced in {n} place(s)"),
        obj(vec![("changes", Value::Array(e.out.clone()))]),
    ))
}

fn set_red_flag(ctx: &Ctx, a: &Args) -> ToolResult {
    let element = ElementId(a.req_u32("element")?);
    let dates = match (
        a.date("from")?,
        a.date("to")?,
        a.date("not_before")?,
        a.date("not_after")?,
    ) {
        (Some(from), Some(to), None, None) => RedFlagDates::Range { from, to },
        (None, None, Some(d), None) => RedFlagDates::NotBefore(d),
        (None, None, None, Some(d)) => RedFlagDates::NotAfter(d),
        _ => {
            return Err(ToolError::invalid(
                "give the dates as from and to, or not_before, or not_after (one of them)",
            ))
        }
    };
    let type_id = a.u32("type").map(RedFlagTypeId);
    let type_name = a.str("type_name").map(str::to_string);
    if type_id.is_some() == type_name.is_some() {
        return Err(ToolError::invalid(
            "give type (an id) or type_name, one of them",
        ));
    }
    let id = a.u32("red_flag").map_or(RedFlagId(0), RedFlagId);
    let note = a.str("note").unwrap_or_default().to_string();
    let hard = a.opt_bool("hard").unwrap_or(false);
    let e = edit(ctx, a, |s| {
        let kind = match (type_id, &type_name) {
            (Some(t), _) => t,
            (None, Some(n)) => s.add_red_flag_type(n)?,
            (None, None) => unreachable!("checked above"),
        };
        let id = s.set_red_flag(
            element,
            RedFlag {
                id,
                kind,
                dates,
                note,
                hard,
            },
        )?;
        let (on, f) = s.red_flag(id).expect("just set");
        Ok((id, red_flag_json(s, on, f)))
    })?;
    let (id, flag) = e.out.clone();
    Ok(done(
        &e,
        &format!("Set red flag {} on element {}", id.0, element.0),
        obj(vec![("red_flag", flag)]),
    ))
}

fn delete_red_flag(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = RedFlagId(a.req_u32("red_flag")?);
    let e = edit(ctx, a, |s| Ok(s.delete_red_flag(id)?))?;
    Ok(done(
        &e,
        &format!("Deleted red flag {}", id.0),
        obj(vec![("red_flag", json!(id.0))]),
    ))
}

fn add_red_flags(ctx: &Ctx, a: &Args) -> ToolResult {
    let elements: Vec<ElementId> = a.u32s("elements").into_iter().map(ElementId).collect();
    let picked = a
        .list("dates")
        .iter()
        .map(|v| {
            let t = v.as_str().unwrap_or_default();
            t.parse()
                .map_err(|e| ToolError::invalid(format!("dates: {t:?}: {e}")))
        })
        .collect::<Result<Vec<method_schedule_core::Date>, _>>()?;
    let dates =
        match (
            picked.is_empty(),
            a.date("from")?,
            a.date("to")?,
            a.date("not_before")?,
            a.date("not_after")?,
        ) {
            (false, None, None, None, None) => method_schedule_core::redflags::runs(&picked),
            (true, Some(from), Some(to), None, None) => vec![RedFlagDates::Range { from, to }],
            (true, None, None, Some(d), None) => vec![RedFlagDates::NotBefore(d)],
            (true, None, None, None, Some(d)) => vec![RedFlagDates::NotAfter(d)],
            _ => return Err(ToolError::invalid(
                "give the dates as dates, or from and to, or not_before, or not_after (one way)",
            )),
        };
    let type_id = a.u32("type").map(RedFlagTypeId);
    let type_name = a.str("type_name").map(str::to_string);
    if type_id.is_some() == type_name.is_some() {
        return Err(ToolError::invalid(
            "give type (an id) or type_name, one of them",
        ));
    }
    let note = a.str("note").unwrap_or_default().to_string();
    let hard = a.opt_bool("hard").unwrap_or(false);
    let e = edit(ctx, a, |s| {
        let kind = match (type_id, &type_name) {
            (Some(t), _) => t,
            (None, Some(n)) => s.add_red_flag_type(n)?,
            (None, None) => unreachable!("checked above"),
        };
        let ids = s.add_red_flags(&elements, kind, &dates, &note, hard)?;
        Ok(ids
            .iter()
            .map(|id| {
                let (on, f) = s.red_flag(*id).expect("just added");
                red_flag_json(s, on, f)
            })
            .collect::<Vec<_>>())
    })?;
    let flags = e.out.clone();
    Ok(done(
        &e,
        &format!("Added {} red flags", flags.len()),
        obj(vec![("red_flags", Value::Array(flags))]),
    ))
}

fn add_standard_red_flag_types(ctx: &Ctx, a: &Args) -> ToolResult {
    let e = edit(ctx, a, |s| Ok(s.standard_red_flag_types()))?;
    let n = e.out;
    Ok(done(
        &e,
        &format!("Added or marked {n} standard red flag types"),
        obj(vec![("added", json!(n))]),
    ))
}

fn set_red_flag_type_color(ctx: &Ctx, a: &Args) -> ToolResult {
    let id = RedFlagTypeId(a.req_u32("type")?);
    let color = match a.str("color") {
        None => None,
        Some(c) => Some(
            method_schedule_core::model::Rgb::parse(c.trim())
                .ok_or_else(|| ToolError::invalid(format!("color: {c:?} is not #rrggbb")))?,
        ),
    };
    let e = edit(ctx, a, |s| {
        let t = s
            .red_flag_types
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| ToolError::not_found(format!("no red flag type {}", id.0)))?;
        t.color = color;
        Ok(())
    })?;
    Ok(done(
        &e,
        &format!("Set the color of red flag type {}", id.0),
        obj(vec![
            ("type", json!(id.0)),
            ("color", json!(color.map(|c| c.hex()))),
        ]),
    ))
}

fn merge_sheets(ctx: &Ctx, a: &Args) -> ToolResult {
    let survivor = SheetId(a.req_u32("survivor")?);
    let others: Vec<SheetId> = a.u32s("others").into_iter().map(SheetId).collect();
    if others.is_empty() {
        return Err(ToolError::invalid("give at least one sheet in `others`"));
    }
    let e = edit(ctx, a, |s| Ok(s.merge_sheets(survivor, &others)?))?;
    let r = &e.out;
    let s = &e.schedule;
    let h = s.sheet(survivor);
    let added: Vec<Value> = r
        .elements_added
        .iter()
        .filter_map(|id| s.element(*id))
        .map(|el| {
            json!({
                "id": el.id.0,
                "name": el.name,
                "category": s.category(el.category).map_or(String::new(), |c| c.name.clone()),
            })
        })
        .collect();
    Ok(done(
        &e,
        &format!(
            "Merged {} sheet(s) into sheet {} (scenes {}); {} value(s) dropped",
            r.merged.len(),
            survivor.0,
            quoted(h.map_or("", |h| h.scenes.as_str())),
            r.dropped.len()
        ),
        obj(vec![
            ("survivor", json!(survivor.0)),
            (
                "merged",
                json!(r.merged.iter().map(|x| x.0).collect::<Vec<_>>()),
            ),
            ("scenes", json!(h.map_or("", |h| h.scenes.as_str()))),
            ("eighths", json!(h.map_or(0, |h| h.pages.0))),
            (
                "pages",
                json!(h.map_or(String::new(), |h| h.pages.to_string())),
            ),
            (
                "combined",
                json!(r
                    .combined
                    .iter()
                    .map(|c| json!({"field": c.field, "before": c.before, "after": c.after}))
                    .collect::<Vec<_>>()),
            ),
            ("elements_added", json!(added)),
            (
                "dropped",
                json!(r
                    .dropped
                    .iter()
                    .map(|d| json!({"sheet": d.sheet.0, "field": d.field, "value": d.value}))
                    .collect::<Vec<_>>()),
            ),
        ]),
    ))
}

fn duplicate_sheet(ctx: &Ctx, a: &Args) -> ToolResult {
    let sheet = SheetId(a.req_u32("sheet")?);
    let e = edit(ctx, a, |s| Ok(s.duplicate_sheet(sheet)?))?;
    let copy = e.out;
    let s = &e.schedule;
    let places: Vec<Value> = s
        .boards
        .iter()
        .filter_map(|b| {
            s.sheet_place(b.id, copy).map(|p| {
                json!({
                    "board": b.id.0,
                    "sub_board": p.sub_board.map(|x| x.0),
                    "index": p.index,
                })
            })
        })
        .collect();
    Ok(done(
        &e,
        &format!("Duplicated sheet {} as sheet {}", sheet.0, copy.0),
        obj(vec![
            ("original", json!(sheet.0)),
            ("sheet", json!(copy.0)),
            ("places", json!(places)),
        ]),
    ))
}
