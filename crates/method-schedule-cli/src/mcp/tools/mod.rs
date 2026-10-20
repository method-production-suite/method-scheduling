//! The tools: each one a name, a description, an input schema, whether it writes, and a
//! function from checked arguments to a result.
//!
//! Results are a short summary for people and a JSON object for programs
//! (`structuredContent`). Every number is an integer: page lengths in eighths (keys
//! `eighths`, with the same length as text under `pages`), times in minutes or seconds,
//! ids as numbers. Dates are `YYYY-MM-DD`. Text from a schedule (titles, headings,
//! synopses, names, notes) is returned as data, never as instructions.

use method_schedule_core::find::{CondField, Condition, FieldFilter, Location, Op, Query, Scope};
use method_schedule_core::model::{BoardId, Error as CoreError, SubBoardId};
use method_schedule_core::{Date, Schedule};
use serde_json::{json, Map, Value};

use super::paths::Roots;
use super::schema;

pub mod beyond;
pub mod elements;
pub mod read;
pub mod reports;
pub mod write;

/// What a tool returns when it works.
pub struct ToolOut {
    /// One or two sentences for a person.
    pub summary: String,
    /// The result for a program.
    pub data: Value,
}

/// Why a tool didn't do what it was asked. Returned to the client as a tool result with
/// `isError: true`, so a model can read it and try again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolError {
    /// `invalid_input`, `not_found`, `refused`, `conflict`, `path_refused` or `io`.
    pub kind: &'static str,
    pub message: String,
}

impl ToolError {
    pub fn invalid(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "invalid_input",
            message: m.into(),
        }
    }
    pub fn not_found(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "not_found",
            message: m.into(),
        }
    }
    pub fn refused(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "refused",
            message: m.into(),
        }
    }
    pub fn conflict(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "conflict",
            message: m.into(),
        }
    }
    pub fn path(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "path_refused",
            message: m.into(),
        }
    }
    pub fn io(m: impl Into<String>) -> ToolError {
        ToolError {
            kind: "io",
            message: m.into(),
        }
    }
}

/// The engine's refusals: a missing id is `not_found`, anything else `refused`. Nothing
/// is written either way.
impl From<CoreError> for ToolError {
    fn from(e: CoreError) -> ToolError {
        match e {
            CoreError::NoSuchCategory(_)
            | CoreError::NoSuchElement(_)
            | CoreError::NoSuchSheet(_)
            | CoreError::NoSuchBoard(_)
            | CoreError::NoSuchSubBoard(_)
            | CoreError::NoSuchStrip(_)
            | CoreError::NoSuchRedFlagType(_)
            | CoreError::NoSuchRedFlag(_) => ToolError::not_found(format!("{e}; nothing written")),
            _ => ToolError::refused(format!("{e}; nothing written")),
        }
    }
}

pub type ToolResult = Result<ToolOut, ToolError>;

/// What every tool works with.
pub struct Ctx {
    pub roots: Roots,
}

/// A tool.
pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// Writes a file. Hidden (and refused) when the server is read-only.
    pub mutating: bool,
    /// Removes something from a schedule (a hint for clients). No tool removes a file.
    pub destructive: bool,
    pub schema: fn() -> Value,
    pub run: fn(&Ctx, &Args) -> ToolResult,
}

impl Tool {
    /// The tool as `tools/list` describes it.
    pub fn describe(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": (self.schema)(),
            "annotations": {
                "title": self.title,
                "readOnlyHint": !self.mutating,
                "destructiveHint": self.mutating && self.destructive,
                "idempotentHint": !self.mutating,
                "openWorldHint": false,
            },
        })
    }
}

/// The `lines_per_page` argument of the tools that read a script: lines on a Fountain
/// page, 40 to 70, default 56 (decision 195).
pub fn lines_per_page_prop() -> (&'static str, Value) {
    use method_schedule_core::fountain::{MAX_LINES_PER_PAGE, MIN_LINES_PER_PAGE};
    (
        "lines_per_page",
        schema::integer(
            "Lines on a page when a Fountain script is paginated (7 lines an eighth at \
             56). Default 56. An .fdx or .mscenes file keeps its own lengths.",
            i64::from(MIN_LINES_PER_PAGE),
            i64::from(MAX_LINES_PER_PAGE),
        ),
    )
}

/// The lines a Fountain page from `lines_per_page`, else the default.
pub fn lines_per_page(a: &Args) -> u32 {
    a.u32("lines_per_page")
        .unwrap_or(method_schedule_core::fountain::LINES_PER_PAGE)
}

/// Every tool, in the order `tools/list` gives them.
pub fn all() -> Vec<Tool> {
    let mut v = read::tools();
    v.extend(reports::tools());
    v.extend(beyond::tools());
    v.extend(write::tools());
    v.extend(elements::tools());
    v
}

/// A tool call's arguments, already checked against the tool's schema.
pub struct Args<'a>(pub &'a Map<String, Value>);

impl Args<'_> {
    pub fn get(&self, k: &str) -> Option<&Value> {
        self.0.get(k).filter(|v| !v.is_null())
    }
    /// Whether the key is there, `null` included (null clears a value).
    pub fn has(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        self.get(k).and_then(Value::as_str)
    }
    pub fn req(&self, k: &str) -> Result<&str, ToolError> {
        self.str(k)
            .ok_or_else(|| ToolError::invalid(format!("`{k}` is required")))
    }
    pub fn opt_bool(&self, k: &str) -> Option<bool> {
        self.get(k).and_then(Value::as_bool)
    }
    /// A whole number the schema keeps within `u32`.
    pub fn u32(&self, k: &str) -> Option<u32> {
        self.get(k)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    }
    pub fn req_u32(&self, k: &str) -> Result<u32, ToolError> {
        self.u32(k)
            .ok_or_else(|| ToolError::invalid(format!("`{k}` is required")))
    }
    pub fn u64(&self, k: &str) -> Option<u64> {
        self.get(k).and_then(Value::as_u64)
    }
    pub fn list(&self, k: &str) -> &[Value] {
        self.get(k)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn u32s(&self, k: &str) -> Vec<u32> {
        self.list(k)
            .iter()
            .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect()
    }
    /// A `YYYY-MM-DD` date argument, if given.
    pub fn date(&self, k: &str) -> Result<Option<Date>, ToolError> {
        self.str(k)
            .map(|t| {
                t.parse::<Date>()
                    .map_err(|e| ToolError::invalid(format!("{k}: {t:?}: {e}")))
            })
            .transpose()
    }
    pub fn req_date(&self, k: &str) -> Result<Date, ToolError> {
        self.date(k)?
            .ok_or_else(|| ToolError::invalid(format!("`{k}` is required")))
    }
}

/// The board and sub-board named by the `board` and `sub_board` arguments: the first
/// board by default, and its first sub-board.
pub fn pick(s: &Schedule, a: &Args) -> Result<(BoardId, SubBoardId), ToolError> {
    let b = match a.u32("board") {
        Some(id) => s
            .board(BoardId(id))
            .ok_or_else(|| ToolError::not_found(format!("no board {id}")))?,
        None => s
            .boards
            .first()
            .ok_or_else(|| ToolError::not_found("the schedule has no board"))?,
    };
    let sb = match a.u32("sub_board") {
        Some(id) => b
            .sub_boards
            .iter()
            .find(|x| x.id == SubBoardId(id))
            .ok_or_else(|| {
                ToolError::not_found(format!("board {} has no sub-board {id}", b.id.0))
            })?,
        None => b
            .sub_boards
            .first()
            .ok_or_else(|| ToolError::not_found("the board has no sub-board"))?,
    };
    Ok((b.id, sb.id))
}

/// The board named by the `board` argument, the first by default.
pub fn pick_board(s: &Schedule, a: &Args) -> Result<BoardId, ToolError> {
    match a.u32("board") {
        Some(id) => s
            .board(BoardId(id))
            .map(|b| b.id)
            .ok_or_else(|| ToolError::not_found(format!("no board {id}"))),
        None => s
            .boards
            .first()
            .map(|b| b.id)
            .ok_or_else(|| ToolError::not_found("the schedule has no board")),
    }
}

/// A date as JSON: its text, or null.
pub fn date_json(d: Option<Date>) -> Value {
    d.map_or(Value::Null, |d| json!(d.to_string()))
}

/// Schedule text quoted for a summary line, cut short: it is data, so it is shown, never
/// followed.
pub fn quoted(s: &str) -> String {
    let one_line: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cut: String = one_line.chars().take(60).collect();
    if cut.chars().count() < one_line.chars().count() {
        format!("\"{cut}...\"")
    } else {
        format!("\"{cut}\"")
    }
}

/// A JSON object from key and value pairs.
pub fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// The schema property every schedule tool takes.
pub fn path_prop() -> (&'static str, Value) {
    (
        "path",
        schema::path(
            "The schedule file (.mtsc, or a schedule saved as .json): relative to the \
             server's first root folder, or absolute inside a root.",
        ),
    )
}

/// The schema property every writing tool takes.
pub fn revision_prop() -> (&'static str, Value) {
    (
        "revision",
        json!({
            "type": "string",
            "description": "The file's revision from the read this change is based on. The \
                            change is refused when the file has changed since.",
            "minLength": 16,
            "maxLength": 16,
        }),
    )
}

/// An id property.
pub fn id_prop(description: &str) -> Value {
    schema::integer(description, 1, i64::from(u32::MAX))
}

/// The optional `board` property.
pub fn board_prop() -> (&'static str, Value) {
    (
        "board",
        id_prop("A board id; the first board when left out."),
    )
}

/// The optional `sub_board` property.
pub fn sub_board_prop() -> (&'static str, Value) {
    (
        "sub_board",
        id_prop("A sub-board id of that board; its first sub-board when left out."),
    )
}

/// A date property.
pub fn date_prop(description: &str) -> Value {
    json!({"type": "string", "description": format!("{description} YYYY-MM-DD."), "minLength": 10, "maxLength": 10})
}

/// The search properties `find` and `replace` share: the text, the options, the field
/// filter and the scope.
pub fn search_props() -> Vec<(&'static str, Value)> {
    vec![
        ("text", schema::name("The text to look for.")),
        (
            "field",
            schema::one_of(
                "Look only here: sheet (every header field), one header field, element \
                 (names), category (names), banner (text) or event (names). Default: all \
                 of them.",
                &FieldFilter::keys(),
            ),
        ),
        (
            "match_case",
            schema::boolean("Capitals must match. Default false."),
        ),
        (
            "whole_word",
            schema::boolean(
                "The match must not run on into letters, digits or _ on either side. \
                 Default false.",
            ),
        ),
        (
            "board",
            id_prop("With sub_board: the board holding it (found when left out)."),
        ),
        (
            "sub_board",
            id_prop(
                "Search one sub-board only: its sheets and banners, its events, and the \
                 elements (and their categories) on them. Default: the whole schedule.",
            ),
        ),
        (
            "conditions",
            schema::array(
                "An advanced search: conditions a sheet must meet to be searched. With \
                 any, only sheet header fields are searched.",
                schema::object(
                    vec![
                        (
                            "field",
                            schema::one_of(
                                "A header field, or pages, shoot_time or screen_time.",
                                &CondField::keys(),
                            ),
                        ),
                        (
                            "op",
                            schema::one_of(
                                "How to compare. greater and less compare numbers, and \
                                 text naturally (scene 12 after 9).",
                                &Op::KEYS,
                            ),
                        ),
                        (
                            "value",
                            schema::text(
                                "What to compare with: pages as 2 or 1 3/8, shoot time \
                                 as h:mm, screen time as m:ss. Not needed for empty and \
                                 not_empty.",
                            ),
                        ),
                    ],
                    &["field", "op"],
                ),
                20,
            ),
        ),
        (
            "any",
            schema::boolean("Join the conditions with OR rather than AND. Default false."),
        ),
    ]
}

/// The conditions an advanced search gives, checked.
pub fn search_conditions(a: &Args) -> Result<Vec<Condition>, ToolError> {
    a.list("conditions")
        .iter()
        .map(|c| {
            let get = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or("");
            let field = CondField::parse(get("field")).ok_or_else(|| {
                ToolError::invalid(format!(
                    "conditions: unknown field {}",
                    quoted(get("field"))
                ))
            })?;
            let op = Op::parse(get("op")).ok_or_else(|| {
                ToolError::invalid(format!("conditions: unknown op {}", quoted(get("op"))))
            })?;
            Condition::new(field, op, get("value")).map_err(ToolError::invalid)
        })
        .collect()
}

/// The search the `search_props` arguments describe.
pub fn search_query(s: &Schedule, a: &Args) -> Result<Query, ToolError> {
    let field = match a.str("field") {
        Some(f) => Some(
            FieldFilter::parse(f)
                .ok_or_else(|| ToolError::invalid(format!("field: unknown {}", quoted(f))))?,
        ),
        None => None,
    };
    let scope = match a.u32("sub_board") {
        None => Scope::Schedule,
        Some(id) => {
            let holds = |b: &&method_schedule_core::model::Board| {
                b.sub_boards.iter().any(|x| x.id == SubBoardId(id))
            };
            let b = match a.u32("board") {
                Some(bid) => s
                    .board(BoardId(bid))
                    .ok_or_else(|| ToolError::not_found(format!("no board {bid}")))?,
                None => s
                    .boards
                    .iter()
                    .find(holds)
                    .ok_or_else(|| ToolError::not_found(format!("no sub-board {id}")))?,
            };
            if !holds(&b) {
                return Err(ToolError::not_found(format!(
                    "board {} has no sub-board {id}",
                    b.id.0
                )));
            }
            Scope::SubBoard {
                board: b.id,
                sub_board: SubBoardId(id),
            }
        }
    };
    Ok(Query {
        text: a.req("text")?.to_string(),
        match_case: a.opt_bool("match_case").unwrap_or(false),
        whole_word: a.opt_bool("whole_word").unwrap_or(false),
        field,
        scope,
        conditions: search_conditions(a)?,
        any: a.opt_bool("any").unwrap_or(false),
    })
}

/// A red flag as JSON: its id, element, type, dates (`from` and `to`, or `not_before`, or
/// `not_after`, the others null), note and whether it is hard.
pub fn red_flag_json(
    s: &Schedule,
    element: method_schedule_core::model::ElementId,
    f: &method_schedule_core::model::RedFlag,
) -> Value {
    use method_schedule_core::model::RedFlagDates;
    let (from, to, not_before, not_after) = match f.dates {
        RedFlagDates::Range { from, to } => (Some(from), Some(to), None, None),
        RedFlagDates::NotBefore(d) => (None, None, Some(d), None),
        RedFlagDates::NotAfter(d) => (None, None, None, Some(d)),
    };
    let el = s.element(element);
    json!({
        "id": f.id.0,
        "element": element.0,
        "element_name": el.map_or(String::new(), |e| e.name.clone()),
        "category": el.and_then(|e| s.category(e.category)).map_or(String::new(), |c| c.name.clone()),
        "type": f.kind.0,
        "type_name": s.red_flag_type(f.kind).map_or(String::new(), |t| t.name.clone()),
        "from": date_json(from),
        "to": date_json(to),
        "not_before": date_json(not_before),
        "not_after": date_json(not_after),
        "dates": f.dates.text(),
        "note": f.note,
        "hard": f.hard,
    })
}

/// A found place as JSON: its kind, its field, and the ids that name it.
pub fn location_json(l: &Location) -> Value {
    let mut v = json!({"kind": l.kind(), "field": l.field()});
    match *l {
        Location::Sheet { sheet, .. } => v["sheet"] = json!(sheet.0),
        Location::Element(id) => v["element"] = json!(id.0),
        Location::Category(id) => v["category"] = json!(id.0),
        Location::Banner {
            board,
            sub_board,
            strip,
        } => {
            v["board"] = json!(board.0);
            v["sub_board"] = json!(sub_board.0);
            v["strip"] = json!(strip.0);
        }
        Location::Event {
            board,
            sub_board,
            index,
        } => {
            v["board"] = json!(board.0);
            v["sub_board"] = json!(sub_board.0);
            v["event"] = json!(index);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_is_in_the_subset_and_names_are_unique() {
        let mut names = std::collections::BTreeSet::new();
        for t in all() {
            schema::check(&(t.schema)(), t.name).unwrap();
            assert!(names.insert(t.name), "{} twice", t.name);
            assert!(!t.description.contains('\u{2014}'), "{}: em dash", t.name);
            let s = (t.schema)();
            if t.mutating && t.name != "new_schedule" {
                let req = s["required"].as_array().unwrap();
                assert!(
                    req.contains(&json!("revision")),
                    "{} needs revision",
                    t.name
                );
            }
        }
    }

    #[test]
    fn quotes_are_cut_and_flattened() {
        assert_eq!(quoted("a\nb"), "\"a b\"");
        assert!(quoted(&"x".repeat(100)).ends_with("...\""));
    }
}
