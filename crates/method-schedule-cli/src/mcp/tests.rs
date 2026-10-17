//! Server tests: a scripted JSON-RPC session over in-memory streams, the handshake and
//! version negotiation of both protocol eras, tool listing and schemas, read-only mode,
//! the safety checks on paths and revisions, and every tool on a small schedule.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

use method_schedule_core::model::{Place, StripKind};
use method_schedule_core::{fountain, Date, Schedule};
use serde_json::{json, Value};

use super::protocol::{
    parse_line, read_line, run, spawn_reader, Incoming, Scripted, Server, LEGACY, LIST_TTL_MS,
    MODERN,
};
use super::{files, limits, schema, Config};

fn script_text() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/scripts/harbor-light.fountain"
    ))
    .unwrap()
}

/// The Harbor Light script as a schedule: every strip on Main, starting 2027-03-01, no
/// day breaks yet.
fn harbor() -> Schedule {
    let mut s = Schedule::new("ZTEST Harbor Light");
    let cast = s.add_category("Cast");
    s.import_script(&fountain::parse(&script_text()), cast)
        .unwrap();
    let b = s.boards[0].id;
    let sb = s.boards[0].sub_boards[0].id;
    let ids: Vec<_> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
    s.move_strips(
        b,
        &ids,
        Place {
            sub_board: Some(sb),
            index: 0,
        },
    )
    .unwrap();
    s.boards[0].sub_boards[0].start = Some("2027-03-01".parse::<Date>().unwrap());
    s
}

/// A fresh folder holding `film.mtsc` (the Harbor Light schedule) and the script as
/// `harbor.fountain`, inside a `root` folder, with an `outside` folder beside it.
fn temp_root(name: &str) -> (PathBuf, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("method-schedule-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("root");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::write(
        root.join("film.mtsc"),
        method_schedule_format::to_string(&harbor()),
    )
    .unwrap();
    std::fs::write(root.join("harbor.fountain"), script_text()).unwrap();
    (base, root)
}

/// A server on a fresh root, already through the handshake.
pub struct Fixture {
    base: PathBuf,
    pub root: PathBuf,
    pub server: Server,
    next_id: u64,
}

impl Fixture {
    pub fn new(name: &str, read_only: bool) -> Fixture {
        let (base, root) = temp_root(name);
        let server = Server::new(&Config {
            roots: vec![root.clone()],
            read_only,
        })
        .unwrap();
        let mut f = Fixture {
            base,
            root,
            server,
            next_id: 100,
        };
        let r = f.raw(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}));
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        f
    }

    /// Sends one message; the response.
    pub fn raw(&mut self, msg: Value) -> Value {
        self.server.handle(msg).unwrap_or(Value::Null)
    }

    /// A request; its whole response.
    pub fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        self.raw(json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}))
    }

    /// Calls a tool: its structured result, or its error kind and text.
    pub fn tool(&mut self, name: &str, args: Value) -> Result<Value, (String, String)> {
        let r = self.rpc("tools/call", json!({"name": name, "arguments": args}));
        let result = &r["result"];
        assert!(result.is_object(), "{name}: not a tool result: {r}");
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if result["isError"] == json!(true) {
            let err = &result["structuredContent"]["error"];
            assert_eq!(err["message"], json!(text));
            Err((err["kind"].as_str().unwrap().to_string(), text))
        } else {
            // The JSON is in the content too, for clients without structured results.
            let mirrored: Value =
                serde_json::from_str(result["content"][1]["text"].as_str().unwrap()).unwrap();
            assert_eq!(mirrored, result["structuredContent"], "{name}");
            Ok(result["structuredContent"].clone())
        }
    }

    /// Calls a tool that must work.
    pub fn ok(&mut self, name: &str, args: Value) -> Value {
        self.tool(name, args)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
    }

    /// The current revision of a file, from read_schedule.
    pub fn rev(&mut self, path: &str) -> String {
        self.ok("read_schedule", json!({"path": path}))["revision"]
            .as_str()
            .unwrap()
            .to_string()
    }

    pub fn bytes(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.root.join(name)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn error_code(r: &Value) -> i64 {
    r["error"]["code"]
        .as_i64()
        .unwrap_or_else(|| panic!("not an error: {r}"))
}

fn tool_call(id: u64, name: &str, args: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})
        .to_string()
}

/// The scripted session the task asks for, as a client would send it: one line per
/// message into stdin, one line per answer out of stdout, both in memory.
#[test]
fn a_scripted_session_over_in_memory_streams() {
    let (base, root) = temp_root("session");
    let film = root.join("film.mtsc");
    let before = std::fs::read(&film).unwrap();
    let rev0 = files::revision(&before);
    std::fs::write(base.join("outside/other.mtsc"), &before).unwrap();

    let lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "script", "version": "1"}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        tool_call(3, "read_schedule", json!({"path": "film.mtsc"})),
        // A write with the revision just read.
        tool_call(4, "auto_day_breaks", json!({"path": "film.mtsc", "revision": rev0, "by": "eighths", "limit": 2})),
        // The same revision again is stale now: refused, nothing written.
        tool_call(5, "set_start_date", json!({"path": "film.mtsc", "revision": rev0, "date": "2030-01-07"})),
        // Outside the root, by a relative path and by an absolute one.
        tool_call(6, "read_schedule", json!({"path": "../outside/other.mtsc"})),
        tool_call(7, "check", json!({"path": base.join("outside/other.mtsc").to_str().unwrap()})),
        "this is not json".to_string(),
        json!({"jsonrpc": "2.0", "id": 8, "method": "ping"}).to_string(),
    ];
    let input = std::io::Cursor::new((lines.join("\n") + "\n").into_bytes());
    let server = Server::new(&Config {
        roots: vec![root.clone()],
        read_only: false,
    })
    .unwrap();
    let mut out = Vec::new();
    run(server, spawn_reader(input), &mut out).unwrap();
    let replies: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let ids: Vec<Value> = replies.iter().map(|r| r["id"].clone()).collect();
    assert_eq!(
        ids,
        [1, 2, 3, 4, 5, 6, 7]
            .iter()
            .map(|i| json!(i))
            .chain([Value::Null, json!(8)])
            .collect::<Vec<_>>(),
        "the notification gets no answer"
    );

    // initialize and tools/list.
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        "method-schedule"
    );
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"read_schedule") && names.contains(&"auto_day_breaks"));

    // The read.
    let read = &replies[2]["result"]["structuredContent"];
    assert_eq!(read["revision"], json!(rev0));
    assert_eq!(read["title"], "ZTEST Harbor Light");
    assert_eq!(read["counts"]["sheets"], 5);

    // The write with the right revision went through.
    let wrote = &replies[3]["result"]["structuredContent"];
    assert_eq!(replies[3]["result"]["isError"], false, "{}", replies[3]);
    assert_eq!(wrote["changed"], true);
    assert_eq!(wrote["shoot_days"], 3);
    let after = std::fs::read(&film).unwrap();
    assert_ne!(after, before);
    assert_eq!(wrote["revision"], json!(files::revision(&after)));

    // The stale write was refused and left the file as the first write made it.
    assert_eq!(replies[4]["result"]["isError"], true);
    assert_eq!(
        replies[4]["result"]["structuredContent"]["error"]["kind"],
        "conflict"
    );
    assert_eq!(std::fs::read(&film).unwrap(), after);

    // Both paths outside the root were refused.
    for r in &replies[5..7] {
        assert_eq!(r["result"]["isError"], true, "{r}");
        let err = &r["result"]["structuredContent"]["error"];
        assert_eq!(err["kind"], "path_refused", "{r}");
        assert!(err["message"].as_str().unwrap().contains("outside"));
    }
    assert_eq!(replies[7]["error"]["code"], -32700);
    assert_eq!(replies[8]["result"], json!({}));
    // No temporary files were left in the root.
    let mut left: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["film.mtsc", "harbor.fountain"]);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn read_only_hides_every_tool_that_writes() {
    let mut f = Fixture::new("readonly", true);
    let r = f.rpc("tools/list", json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    let names: BTreeSet<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "list_schedules",
            "read_schedule",
            "get_sheet",
            "list_elements",
            "days",
            "dood",
            "one_line",
            "budget_exchange",
            "check",
            "diff",
            "explain",
            "find",
            "conflicts",
            "red_flags",
            "report",
            "report_list",
            "timeline",
            "call_sheet",
            "rest_report",
            "drift",
            "suggest_order",
            "locations",
        ])
    );
    for t in tools {
        assert_eq!(t["annotations"]["readOnlyHint"], true, "{}", t["name"]);
    }
    let before = f.bytes("film.mtsc");
    let rev = f.rev("film.mtsc");
    let r = f.rpc(
        "tools/call",
        json!({"name": "set_start_date", "arguments": {"path": "film.mtsc", "revision": rev, "date": null}}),
    );
    assert_eq!(error_code(&r), -32602);
    assert!(r["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read-only"));
    let r = f.rpc(
        "tools/call",
        json!({"name": "new_schedule", "arguments": {"path": "new.mtsc"}}),
    );
    assert_eq!(error_code(&r), -32602);
    assert_eq!(f.bytes("film.mtsc"), before);
    assert!(!f.root.join("new.mtsc").exists());
}

#[test]
fn a_write_that_fails_check_leaves_the_file_unchanged() {
    let mut f = Fixture::new("failcheck", false);
    let before = f.bytes("film.mtsc");
    let rev = f.rev("film.mtsc");
    // A blank category name passes the schema and the engine makes the category, but
    // Schedule::check refuses a category with no name, so nothing is written.
    let (kind, msg) = f
        .tool(
            "add_element",
            json!({"path": "film.mtsc", "revision": rev, "category": "   ", "name": "Lantern"}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");
    assert!(msg.contains("wouldn't hang together"), "{msg}");
    assert!(msg.contains("category has no name"), "{msg}");
    assert_eq!(f.bytes("film.mtsc"), before);
    // The engine's own refusals write nothing either: a board ID taken in the category,
    // a strip that isn't there.
    let el = f.ok("list_elements", json!({"path": "film.mtsc"}))["elements"][0]["id"].clone();
    let set = f.ok(
        "set_board_id",
        json!({"path": "film.mtsc", "revision": rev, "element": el, "board_id": "1"}),
    );
    let before = f.bytes("film.mtsc");
    let (kind, msg) = f
        .tool(
            "add_element",
            json!({"path": "film.mtsc", "revision": set["revision"], "category": "Cast", "name": "Other", "board_id": "1"}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused", "{msg}");
    // A board ID is letters and digits, as in the app.
    for bad in ["1 A", "1-A", "1234567890123"] {
        let (kind, msg) = f
            .tool(
                "set_board_id",
                json!({"path": "film.mtsc", "revision": set["revision"], "element": el, "board_id": bad}),
            )
            .unwrap_err();
        assert_eq!(kind, "refused", "{bad:?}: {msg}");
        assert!(msg.contains("letters and digits"), "{msg}");
    }
    let (kind, _) = f
        .tool(
            "delete_day_break",
            json!({"path": "film.mtsc", "revision": set["revision"], "strip": 999_999}),
        )
        .unwrap_err();
    assert_eq!(kind, "not_found");
    assert_eq!(f.bytes("film.mtsc"), before);
}

#[test]
fn every_tool_works_on_a_small_schedule() {
    let mut f = Fixture::new("tools", false);

    // Listing and new files.
    let list = f.ok("list_schedules", json!({}));
    assert_eq!(list["files"].as_array().unwrap().len(), 1);
    assert_eq!(list["files"][0]["path"], "film.mtsc");
    let made = f.ok(
        "new_schedule",
        json!({"path": "new.mtsc", "title": "ZTEST New"}),
    );
    assert_eq!(
        made["revision"],
        json!(files::revision(&f.bytes("new.mtsc")))
    );
    let (kind, _) = f
        .tool("new_schedule", json!({"path": "new.mtsc"}))
        .unwrap_err();
    assert_eq!(kind, "refused", "never replaces a file");
    let (kind, _) = f
        .tool("new_schedule", json!({"path": "new.json"}))
        .unwrap_err();
    assert_eq!(kind, "path_refused", "new files are .mtsc only");
    std::fs::write(f.root.join("other.json"), "{\"name\": \"not a schedule\"}").unwrap();
    assert_eq!(
        f.ok("list_schedules", json!({}))["files"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "a .json that isn't a schedule is not listed"
    );

    // Importing a script into the new schedule.
    let rev = made["revision"].as_str().unwrap().to_string();
    let imp = f.ok(
        "import_script",
        json!({"path": "new.mtsc", "revision": rev, "script": "harbor.fountain"}),
    );
    assert_eq!(imp["sheets"].as_array().unwrap().len(), 5);
    let read = f.ok("read_schedule", json!({"path": "new.mtsc", "limit": 2}));
    assert_eq!(read["counts"]["sheets"], 5);
    assert_eq!(read["sheets"].as_array().unwrap().len(), 2);
    assert_eq!(read["boards"][0]["boneyard"], 5);
    let (kind, _) = f
        .tool(
            "import_script",
            json!({"path": "new.mtsc", "revision": read["revision"], "script": "film.mtsc"}),
        )
        .unwrap_err();
    assert_eq!(kind, "path_refused", "scripts are .fountain or .fdx");
    // Fountain lines per page (decision 195): 56 by default, 40 to 70 when given.
    assert_eq!(imp["eighths"], 7);
    let (kind, _) = f
        .tool(
            "import_script",
            json!({"path": "new.mtsc", "revision": read["revision"], "script": "harbor.fountain", "lines_per_page": 39}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    let short = f.ok(
        "import_script",
        json!({"path": "new.mtsc", "revision": read["revision"], "script": "harbor.fountain", "lines_per_page": 40}),
    );
    assert_eq!(short["eighths"], 10, "shorter pages, more eighths");

    // Sheets and elements on film.mtsc, from a copy kept for diff.
    let p = "film.mtsc";
    std::fs::copy(f.root.join(p), f.root.join("before.mtsc")).unwrap();
    let sheet = f.ok("read_schedule", json!({"path": p}))["sheets"][0]["id"].clone();
    let rev = f.rev(p);
    let set = f.ok(
        "set_sheet_fields",
        json!({"path": p, "revision": rev, "sheet": sheet, "fields": {"synopsis": "Kit mends a net.", "shoot_minutes": 90, "sunrise": 400}}),
    );
    assert_eq!(set["changed"], true);
    let got = f.ok("get_sheet", json!({"path": p, "sheet": sheet}));
    assert_eq!(got["synopsis"], "Kit mends a net.");
    assert_eq!(got["shoot_minutes"], 90);
    assert_eq!(got["places"][0]["index"], 0);
    let same = f.ok(
        "set_sheet_fields",
        json!({"path": p, "revision": set["revision"], "sheet": sheet, "fields": {"synopsis": "Kit mends a net."}}),
    );
    assert_eq!(same["changed"], false, "nothing to write");
    assert_eq!(same["revision"], set["revision"]);
    let e = f
        .tool(
            "set_sheet_fields",
            json!({"path": p, "revision": set["revision"], "sheet": sheet, "fields": {"sunrise": 1440}}),
        )
        .unwrap_err();
    assert_eq!(e.0, "invalid_input", "the schema bounds minutes");

    let added = f.ok(
        "add_element",
        json!({"path": p, "revision": set["revision"], "category": "Props", "name": "Lantern", "sheet": sheet}),
    );
    assert_eq!(added["category_created"], true);
    let lantern = added["element"].clone();
    let els = f.ok("list_elements", json!({"path": p, "category": "Props"}));
    assert_eq!(els["elements"][0]["name"], "Lantern");
    assert_eq!(els["elements"][0]["sheets"], 1);
    let un = f.ok(
        "untag_element",
        json!({"path": p, "revision": added["revision"], "sheet": sheet, "element": lantern}),
    );
    let tag = f.ok(
        "tag_element",
        json!({"path": p, "revision": un["revision"], "sheet": sheet, "element": lantern}),
    );
    let kit = f.ok("list_elements", json!({"path": p, "category": "Cast"}))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "KIT")
        .unwrap()["id"]
        .clone();
    let rules = f.ok(
        "set_element_rules",
        json!({"path": p, "revision": tag["revision"], "element": kit, "holds": true}),
    );
    assert_eq!(rules["rules"]["holds"], true);
    let bid = f.ok(
        "set_board_id",
        json!({"path": p, "revision": rules["revision"], "element": kit, "board_id": "1", "locked": true}),
    );

    // Days, breaks and strips.
    let auto = f.ok(
        "auto_day_breaks",
        json!({"path": p, "revision": bid["revision"], "by": "eighths", "limit": 2}),
    );
    assert_eq!(auto["shoot_days"], 3);
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(days["days"][0]["date"], "2027-03-01");
    assert_eq!(days["days"].as_array().unwrap().len(), 3);
    let first_break = days["days"][0]["day_break"].clone();
    let del = f.ok(
        "delete_day_break",
        json!({"path": p, "revision": auto["revision"], "strip": first_break}),
    );
    assert_eq!(
        f.ok("days", json!({"path": p}))["days"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let ins = f.ok(
        "insert_day_break",
        json!({"path": p, "revision": del["revision"], "index": 1}),
    );
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(days["days"].as_array().unwrap().len(), 3);
    assert_eq!(days["days"][0]["day_break"], ins["strip"]);
    let sheet_strip = days["days"][0]["strips"][0]["strip"].clone();
    let (kind, _) = f
        .tool(
            "delete_day_break",
            json!({"path": p, "revision": ins["revision"], "strip": sheet_strip}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused", "only day breaks");
    let moved = f.ok(
        "move_strips",
        json!({"path": p, "revision": ins["revision"], "strips": [sheet_strip], "to_sub_board": null, "index": 0}),
    );
    assert_eq!(
        f.ok("days", json!({"path": p}))["boneyard"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // A day break parks in the Boneyard (decisions.md, entry 57): one shoot day fewer.
    let parked = f.ok(
        "move_strips",
        json!({"path": p, "revision": moved["revision"], "strips": [ins["strip"]], "to_sub_board": null, "index": 0}),
    );
    let days_now = f.ok("days", json!({"path": p}));
    assert_eq!(days_now["boneyard"][0]["kind"], "day_break");
    assert_eq!(days_now["days"].as_array().unwrap().len(), 2);
    let sub = days["sub_board"].clone();
    let back = f.ok(
        "move_strips",
        json!({"path": p, "revision": parked["revision"], "strips": [sheet_strip], "to_sub_board": sub, "index": 0}),
    );

    // The calendar.
    let start = f.ok(
        "set_start_date",
        json!({"path": p, "revision": back["revision"], "date": "2027-03-02"}),
    );
    let (kind, _) = f
        .tool(
            "set_start_date",
            json!({"path": p, "revision": start["revision"], "date": "2027-02-30"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    let hol = f.ok(
        "set_day_kind",
        json!({"path": p, "revision": start["revision"], "date": "2027-03-03", "kind": "holiday"}),
    );
    assert_eq!(hol["kind"], "holiday");
    let ev = f.ok(
        "add_event",
        json!({"path": p, "revision": hol["revision"], "kind": "fitting", "from": "2027-02-26", "to": "2027-02-26", "elements": [kit]}),
    );
    assert_eq!(ev["index"], 0);
    let (kind, _) = f
        .tool(
            "add_event",
            json!({"path": p, "revision": ev["revision"], "kind": "travel", "from": "2027-01-01", "to": "2029-01-01"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input", "at most 366 days");
    // A leap year is the longest event; one day more is refused, as the app refuses it.
    let (kind, _) = f
        .tool(
            "add_event",
            json!({"path": p, "revision": ev["revision"], "kind": "travel", "from": "2028-01-01", "to": "2029-01-01"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input", "367 days");
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(
        days["days"][1]["date"], "2027-03-04",
        "the holiday takes no day"
    );
    assert_eq!(days["calendar"]["holidays"], json!(["2027-03-03"]));
    assert_eq!(days["calendar"]["events"][0]["kind"], "fitting");
    // A second event, deleted again by its place; a stale place is refused.
    let second = f.ok(
        "add_event",
        json!({"path": p, "revision": ev["revision"], "kind": "camera_test", "from": "2027-03-08", "to": "2027-03-08"}),
    );
    assert_eq!(second["index"], 1);
    let gone = f.ok(
        "delete_event",
        json!({"path": p, "revision": second["revision"], "index": 1}),
    );
    assert_eq!(
        gone["deleted"], "Camera test",
        "a nameless event by its kind"
    );
    let (kind, _) = f
        .tool(
            "delete_event",
            json!({"path": p, "revision": gone["revision"], "index": 1}),
        )
        .unwrap_err();
    assert_eq!(kind, "not_found");
    // The working week: Saturdays shot, then the default again; all seven off is refused.
    let six = f.ok(
        "set_weekly_off",
        json!({"path": p, "revision": gone["revision"], "days": ["sun"]}),
    );
    assert_eq!(six["days"], json!(["sun"]));
    let (kind, _) = f
        .tool(
            "set_weekly_off",
            json!({"path": p, "revision": six["revision"], "days": ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]}),
        )
        .unwrap_err();
    assert_ne!(kind, "", "a week with no working day is refused");
    let back = f.ok(
        "set_weekly_off",
        json!({"path": p, "revision": six["revision"], "days": ["sun", "sat"]}),
    );
    assert_eq!(back["days"], json!(["sat", "sun"]));
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(days["calendar"]["events"].as_array().unwrap().len(), 1);

    // Reports.
    let dood = f.ok("dood", json!({"path": p, "category": "Cast"}));
    let kit_row = dood["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "KIT")
        .unwrap();
    assert_eq!(
        kit_row["cells"][0],
        json!({"date": "2027-02-26", "code": "FT"})
    );
    assert_eq!(kit_row["board_id"], "1");
    let kit_sum = dood["summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "KIT")
        .unwrap();
    assert_eq!(
        kit_sum["total"].as_u64().unwrap(),
        kit_sum["work"].as_u64().unwrap() + kit_sum["hold"].as_u64().unwrap()
    );
    let one = f.ok("one_line", json!({"path": p}));
    assert_eq!(one["days"][0]["date"], "2027-03-02");
    assert_eq!(one["days"][0]["lines"][0]["kind"], "scene");
    // The one-line export's element columns: each shown category with its elements.
    let groups = &one["days"][0]["lines"][0]["elements"];
    assert_eq!(groups[0]["category"], "Cast");
    assert_eq!(groups[0]["elements"][0]["quantity"], 1);
    assert!(groups[0]["elements"][0]["name"].is_string());
    assert!(one.get("table").is_none());
    assert!(one["days"][0]["lines"][0].get("omitted").is_none());
    // An omitted scene is listed and marked (decision 91).
    {
        let file = f.root.join(p);
        let text = std::fs::read_to_string(&file).unwrap();
        let mut sch = method_schedule_format::from_str(&text).unwrap();
        let first = sch.sheets[0].id;
        sch.plan.omitted.push(first);
        std::fs::write(&file, method_schedule_format::to_string(&sch)).unwrap();
        let marked = f.ok("one_line", json!({"path": p}));
        let lines: Vec<&Value> = marked["days"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|d| d["lines"].as_array().unwrap())
            .collect();
        assert_eq!(lines.iter().filter(|l| l["omitted"] == true).count(), 1);
        std::fs::write(&file, text).unwrap();
    }
    // With `columns`, the export's first sheet: the strip layout, or the fixed columns.
    let lay = f.ok("one_line", json!({"path": p, "columns": "layout"}));
    assert_eq!(lay["table"]["columns"][0], "Day breaks and banners");
    assert_eq!(lay["table"]["columns"][1], "Scene");
    let fixed = f.ok("one_line", json!({"path": p, "columns": "fixed"}));
    assert_eq!(fixed["table"]["columns"][0], "day");
    assert_eq!(fixed["table"]["rows"][0][2], "scene");
    let ex = f.ok(
        "budget_exchange",
        json!({"path": p, "generated": "2026-10-11"}),
    );
    assert_eq!(ex["exchange"]["format"], "method-schedule-days");
    assert_eq!(ex["exchange"]["generated"], "2026-10-11");
    // Two: the third day break is parked in the Boneyard.
    assert_eq!(ex["exchange"]["shoot_days"]["total"], 2);
    let chk = f.ok("check", json!({"path": p}));
    assert_eq!(chk["ok"], true);
    assert_eq!(chk["canonical"], true);
    assert_eq!(chk["revision"], ev["revision"]);

    // What changed since the copy, and why KIT's counts are what they are.
    let d = f.ok("diff", json!({"path": "before.mtsc", "other": p}));
    assert_eq!(d["other_revision"], ev["revision"]);
    let changes = d["changes"].as_array().unwrap();
    let synopsis = changes
        .iter()
        .find(|c| c["subject"]["kind"] == "sheet" && c["subject"]["sheet"] == sheet)
        .unwrap();
    assert_eq!(synopsis["kind"], "changed");
    assert!(synopsis["text"]
        .as_str()
        .unwrap()
        .contains("synopsis \"\" -> \"Kit mends a net.\""));
    assert!(changes.iter().any(|c| c["subject"]["kind"] == "calendar"));
    assert!(changes
        .iter()
        .all(|c| !c["text"].as_str().unwrap().contains('\u{2014}')));
    let same = f.ok("diff", json!({"path": p, "other": p}));
    assert_eq!(same["changes"], json!([]));
    let x = f.ok("explain", json!({"path": p, "name": "kit"}));
    assert_eq!(x["name"], "KIT");
    assert_eq!(x["work"], kit_sum["work"]);
    assert_eq!(x["hold"], kit_sum["hold"]);
    assert_eq!(x["total"], kit_sum["total"]);
    assert_eq!(x["days"][0]["code"], "FT");
    assert_eq!(x["days"][0]["reason"], "event");
    assert_eq!(x["event_days"], 1);
    let by_id = f.ok("explain", json!({"path": p, "element": x["element"]}));
    assert_eq!(by_id["days"], x["days"]);
    let (kind, _) = f.tool("explain", json!({"path": p})).unwrap_err();
    assert_eq!(kind, "invalid_input");
    let (kind, _) = f
        .tool("explain", json!({"path": p, "name": "NOBODY"}))
        .unwrap_err();
    assert_eq!(kind, "not_found");

    // The file is what the last write said, and reads back with the engine's reader.
    let s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    assert_eq!(s.boards[0].sub_boards[0].calendar.events.len(), 1);
    assert!(s.boards[0].sub_boards[0]
        .strips
        .iter()
        .any(|x| x.kind == StripKind::DayBreak));

    // A broken file is reported by check, and refused by every other tool.
    std::fs::write(
        f.root.join("broken.mtsc"),
        "{\"format\": \"method-schedule\", \"version\": 0}",
    )
    .unwrap();
    let chk = f.ok("check", json!({"path": "broken.mtsc"}));
    assert_eq!(chk["ok"], false);
    assert!(chk["problem"].is_string());
    assert_eq!(
        f.tool("read_schedule", json!({"path": "broken.mtsc"}))
            .unwrap_err()
            .0,
        "invalid_input"
    );
}

#[test]
fn find_replace_and_conflicts() {
    let mut f = Fixture::new("findreplace", false);
    let p = "film.mtsc";
    let found = f.ok("find", json!({"path": p, "text": "loft"}));
    assert_eq!(found["count"], 2);
    assert_eq!(found["truncated"], false);
    let first = &found["results"][0];
    assert_eq!(first["number"], 1);
    assert_eq!(first["kind"], "sheet");
    assert_eq!(first["field"], "set");
    assert_eq!(first["text"], "NET LOFT");
    assert_eq!(first["context"], "Scene 1 set: NET LOFT");
    assert!(first["sheet"].is_u64());
    let el = f.ok(
        "find",
        json!({"path": p, "text": "KIT", "field": "element", "match_case": true, "whole_word": true}),
    );
    assert_eq!(el["count"], 1);
    assert_eq!(el["results"][0]["kind"], "element");
    assert_eq!(
        f.ok("find", json!({"path": p, "text": "loft", "limit": 1}))["truncated"],
        true
    );
    let (kind, _) = f
        .tool(
            "find",
            json!({"path": p, "text": "x", "sub_board": 999_999}),
        )
        .unwrap_err();
    assert_eq!(kind, "not_found");
    let (kind, _) = f
        .tool("find", json!({"path": p, "text": "x", "field": "nope"}))
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    // An advanced search: only the sheets that meet the conditions.
    let adv = f.ok(
        "find",
        json!({"path": p, "text": "loft", "conditions": [{"field": "scenes", "op": "is", "value": "1"}]}),
    );
    assert_eq!(adv["count"], 1);
    assert_eq!(adv["results"][0]["context"], "Scene 1 set: NET LOFT");
    let either = f.ok(
        "find",
        json!({"path": p, "text": "loft", "any": true, "conditions": [
            {"field": "scenes", "op": "is", "value": "1"},
            {"field": "pages", "op": "greater", "value": "0"}
        ]}),
    );
    assert_eq!(either["count"], 2);
    let (kind, _) = f
        .tool(
            "find",
            json!({"path": p, "text": "x", "conditions": [{"field": "pages", "op": "greater", "value": "lots"}]}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");

    // Replace the second result only, with the revision of the read.
    let rev = found["revision"].clone();
    let r = f.ok(
        "replace",
        json!({"path": p, "revision": rev, "text": "LOFT", "with": "ATTIC", "picks": [2]}),
    );
    assert_eq!(r["changed"], true);
    assert_eq!(r["changes"][0]["before"], "NET LOFT");
    assert_eq!(r["changes"][0]["after"], "NET ATTIC");
    assert_eq!(r["changes"][0]["place"], "Scene 4 set");
    // The old revision is stale now.
    let (kind, _) = f
        .tool(
            "replace",
            json!({"path": p, "revision": rev, "text": "LOFT", "with": "X", "all": true}),
        )
        .unwrap_err();
    assert_eq!(kind, "conflict");
    // A collision in Cast is refused and writes nothing.
    let rev = r["revision"].clone();
    let before = f.bytes(p);
    let (kind, msg) = f
        .tool(
            "replace",
            json!({"path": p, "revision": rev, "text": "TOVE", "with": "kit", "field": "element", "all": true}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");
    assert!(msg.contains("would be in Cast twice"), "{msg}");
    assert_eq!(f.bytes(p), before);
    // all or picks, one of them; picks within the results.
    for args in [
        json!({"path": p, "revision": rev, "text": "LOFT", "with": "X"}),
        json!({"path": p, "revision": rev, "text": "LOFT", "with": "X", "all": true, "picks": [1]}),
        json!({"path": p, "revision": rev, "text": "LOFT", "with": "X", "picks": [5]}),
    ] {
        assert_eq!(f.tool("replace", args).unwrap_err().0, "invalid_input");
    }
    // The same text changes nothing and writes nothing.
    let same = f.ok(
        "replace",
        json!({"path": p, "revision": rev, "text": "LOFT", "with": "LOFT", "match_case": true, "all": true}),
    );
    assert_eq!(same["changed"], false);
    assert_eq!(f.bytes(p), before);

    // Conflicts: none on one sub-board; then a second sub-board on the same dates.
    let c = f.ok("conflicts", json!({"path": p}));
    assert_eq!(c["conflicts"], json!([]));
    let mut s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let b = s.boards[0].id;
    let main = s.boards[0].sub_boards[0].id;
    let unit = s
        .add_sub_board(b, "Unit B", "2027-03-01".parse().ok())
        .unwrap();
    let last = s.boards[0].sub_boards[0].strips.last().unwrap().id;
    s.move_strips(
        b,
        &[last],
        Place {
            sub_board: Some(unit),
            index: 0,
        },
    )
    .unwrap();
    for sub in [main, unit] {
        s.insert_strip(
            b,
            StripKind::DayBreak,
            Place {
                sub_board: Some(sub),
                index: 99,
            },
        )
        .unwrap();
    }
    s.set_event(
        b,
        unit,
        None,
        method_schedule_core::model::Event {
            name: "Fitting".into(),
            kind: method_schedule_core::model::EventKind::Fitting,
            from: "2027-03-01".parse().unwrap(),
            to: "2027-03-01".parse().unwrap(),
            elements: vec![s.elements[0].id],
            color: None,
        },
    )
    .unwrap();
    let kit = s.element_named(s.cast().unwrap(), "KIT").unwrap();
    s.set_event(
        b,
        unit,
        None,
        method_schedule_core::model::Event {
            name: "Rehearse".into(),
            kind: method_schedule_core::model::EventKind::Rehearsal,
            from: "2027-03-01".parse().unwrap(),
            to: "2027-03-01".parse().unwrap(),
            elements: vec![kit],
            color: None,
        },
    )
    .unwrap();
    let _ = s.add_sub_board(b, "Undated", None).unwrap();
    std::fs::write(f.root.join(p), method_schedule_format::to_string(&s)).unwrap();
    let c = f.ok("conflicts", json!({"path": p, "board": b.0}));
    let list = c["conflicts"].as_array().unwrap();
    assert_eq!(list.len(), 1, "{c}");
    assert_eq!(list[0]["date"], "2027-03-01");
    assert_eq!(list[0]["element"]["name"], "KIT");
    assert_eq!(list[0]["bookings"][0]["sub_board_name"], "Main");
    assert_eq!(list[0]["bookings"][1]["sub_board_name"], "Unit B");
    assert_eq!(list[0]["bookings"][1]["sheets"][0]["scenes"], "4");
    assert_eq!(c["not_compared"][0]["name"], "Undated");
    // The rehearsal falls on a day KIT works on Unit B: a conflict too.
    let on_work = c["event_on_work"].as_array().unwrap();
    assert_eq!(on_work.len(), 1, "{c}");
    assert_eq!(on_work[0]["element"]["name"], "KIT");
    assert_eq!(on_work[0]["sub_board_name"], "Unit B");
    assert_eq!(on_work[0]["day"], 1);
    assert_eq!(on_work[0]["event"]["name"], "Rehearse");
    assert_eq!(c["count"], 2);
    assert!(c["event_clashes_note"]
        .as_str()
        .unwrap()
        .contains("Informational"));
    let (kind, _) = f
        .tool("conflicts", json!({"path": p, "board": 999_999}))
        .unwrap_err();
    assert_eq!(kind, "not_found");
}

#[test]
fn reports_and_report_settings() {
    let mut f = Fixture::new("reportsx", false);
    let p = "film.mtsc";
    let mut s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
    method_schedule_core::days::auto_day_breaks(
        &mut s,
        b,
        sb,
        method_schedule_core::days::Limit::Pages(method_schedule_core::Eighths(2)),
    )
    .unwrap();
    std::fs::write(f.root.join(p), method_schedule_format::to_string(&s)).unwrap();

    let g = f.ok(
        "report",
        json!({"path": p, "kind": "dood_grid", "sort": ["work:desc", "name"]}),
    );
    assert_eq!(g["report"][0]["name"], "KIT");
    assert!(f
        .tool(
            "report",
            json!({"path": p, "kind": "dood_grid", "sort": ["scene"]})
        )
        .is_err());
    let bd = f.ok("report", json!({"path": p, "kind": "breakdown"}));
    assert_eq!(bd["report"][0]["slot"]["day"], 1);
    let day = f.ok("report", json!({"path": p, "kind": "breakdown_day"}));
    assert_eq!(day["report"][0]["day"], 1);
    let det = f.ok("report", json!({"path": p, "kind": "element_detail"}));
    assert!(!det["report"].as_array().unwrap().is_empty());
    let v = f.ok("report", json!({"path": p, "kind": "vertical"}));
    assert!(!v["report"]["cast"].as_array().unwrap().is_empty());
    assert!(v["report"]["days"][0]["eighths"].as_u64().unwrap() > 0);
    assert!(v["report"]["days"][0]["minutes"].is_u64());
    let m = f.ok("report", json!({"path": p, "kind": "multi_unit"}));
    assert_eq!(m["report"]["dates"][0]["date"], "2027-03-01");
    let c = f.ok("report", json!({"path": p, "kind": "calendar"}));
    assert_eq!(c["report"][0]["shoot_day"], 1);

    let l = f.ok("report_list", json!({"path": p}));
    assert_eq!(l["reports"][0]["kind"], "one_line");
    assert_eq!(l["dood_codes"]["hold"], "H");
    assert_eq!(l["reports"][0]["header"], "Full");
    assert_eq!(l["reports"][0]["page_break"], "none");
    assert_eq!(l["reports"][0]["shoot_days"], true);
    assert_eq!(l["header_layouts"][1]["name"], "Compact");
    assert_eq!(l["header_layouts"][0]["rows"][0][0], "title");
    let rev = l["revision"].clone();
    let set = f.ok(
        "set_production",
        json!({"path": p, "revision": rev, "company": "ZTEST Pictures", "set_dresser": " S. Dresser "}),
    );
    assert_eq!(set["company"], "ZTEST Pictures");
    assert_eq!(set["set_dresser"], "S. Dresser");
    assert_eq!(set["custom_4"], "");
    let l2 = f.ok("report_list", json!({"path": p}));
    assert_eq!(l2["production"]["set_dresser"], "S. Dresser");
    let bad = f.tool(
        "set_dood_codes",
        json!({"path": p, "revision": set["revision"], "hold": "W"}),
    );
    assert_eq!(bad.unwrap_err().0, "refused");
    let codes = f.ok(
        "set_dood_codes",
        json!({"path": p, "revision": set["revision"], "hold": "HD"}),
    );
    assert_eq!(
        codes["codes"],
        "S W F H D P TR FT R / M".replace(" H ", " HD ")
    );
    let pw = f.ok(
        "set_prep_wrap",
        json!({"path": p, "revision": codes["revision"], "prep_start": "2027-01-04"}),
    );
    assert_eq!(pw["prep_start"], "2027-01-04");
    assert_eq!(pw["wrap_end"], Value::Null);
    let bx = f.ok("budget_exchange", json!({"path": p}));
    assert!(bx.to_string().contains("2027-01-04"), "{bx}");
}

/// Behavior note 23 and decision 113: flags for several elements and dates at once, and
/// the standard types for an older file.
#[test]
fn red_flags_several_at_once_and_standard_types() {
    let mut f = Fixture::new("redflags-many", false);
    let p = "film.mtsc";
    let s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let cast = s.category_named("Cast").unwrap();
    let ids: Vec<u32> = s
        .elements
        .iter()
        .filter(|e| e.category == cast)
        .take(2)
        .map(|e| e.id.0)
        .collect();
    let r = f.ok("red_flags", json!({"path": p}));
    let std = f.ok(
        "add_standard_red_flag_types",
        json!({"path": p, "revision": r["revision"]}),
    );
    assert_eq!(std["added"], 16);
    let r = f.ok("red_flags", json!({"path": p}));
    assert_eq!(r["types"][0]["name"], "Travel");
    assert_eq!(r["types"][0]["builtin"], "travel");
    let added = f.ok(
        "add_red_flags",
        json!({"path": p, "revision": r["revision"], "elements": ids, "type_name": "Unavailable",
               "dates": ["2027-03-03", "2027-03-02", "2027-03-09"], "note": "Exam", "hard": true}),
    );
    let flags = added["red_flags"].as_array().unwrap();
    assert_eq!(flags.len(), 4);
    assert_eq!(flags[0]["from"], "2027-03-02");
    assert_eq!(flags[0]["to"], "2027-03-03");
    assert_eq!(flags[0]["type"], r["types"][3]["id"]);
    let before = f.bytes(p);
    let (kind, _) = f
        .tool(
            "add_red_flags",
            json!({"path": p, "revision": added["revision"], "elements": ids, "type_name": "Unavailable",
                   "dates": ["2027-03-02"], "not_after": "2027-03-02"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    assert_eq!(f.bytes(p), before);
}

#[test]
fn red_flags_set_list_and_delete() {
    let mut f = Fixture::new("redflags", false);
    let p = "film.mtsc";
    // Day breaks every two eighths, as the command line's tests have them: day 2 on
    // 2027-03-02 has scenes 2 and 3, day 3 on 2027-03-03 scene 4; KIT is in all three.
    let mut s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
    method_schedule_core::days::auto_day_breaks(
        &mut s,
        b,
        sb,
        method_schedule_core::days::Limit::Pages(method_schedule_core::Eighths(2)),
    )
    .unwrap();
    std::fs::write(f.root.join(p), method_schedule_format::to_string(&s)).unwrap();
    let kit = s
        .element_named(s.category_named("Cast").unwrap(), "KIT")
        .unwrap()
        .0;

    let r = f.ok("red_flags", json!({"path": p}));
    assert_eq!(r["types"], json!([]));
    assert_eq!(r["red_flags"], json!([]));
    assert_eq!(r["clashes"], json!([]));
    assert_eq!(r["undated"], false);

    let set = f.ok(
        "set_red_flag",
        json!({"path": p, "revision": r["revision"], "element": kit, "type_name": "Unavailable",
               "from": "2027-03-02", "to": "2027-03-02", "note": "Exam", "hard": true}),
    );
    assert_eq!(set["changed"], true);
    let flag = set["red_flag"].clone();
    assert_eq!(flag["type_name"], "Unavailable");
    assert_eq!(flag["from"], "2027-03-02");
    assert_eq!(flag["not_before"], Value::Null);
    assert_eq!(flag["dates"], "on 2027-03-02");
    let r = f.ok(
        "red_flags",
        json!({"path": p, "board": b.0, "sub_board": sb.0}),
    );
    assert_eq!(r["types"][0]["name"], "Unavailable");
    assert_eq!(r["red_flags"][0], flag);
    let clashes = r["clashes"].as_array().unwrap();
    assert_eq!(clashes.len(), 1, "{r}");
    assert_eq!(clashes[0]["date"], "2027-03-02");
    assert_eq!(clashes[0]["element_name"], "KIT");
    assert_eq!(clashes[0]["day"], 2);
    assert_eq!(clashes[0]["hard"], true);
    assert_eq!(clashes[0]["sheets"][0]["scenes"], "2");
    assert_eq!(clashes[0]["red_flag"], flag["id"]);

    // Replace it: gone after Mar 1, so days 2 and 3 clash.
    let rev = set["revision"].clone();
    let again = f.ok(
        "set_red_flag",
        json!({"path": p, "revision": rev, "element": kit, "red_flag": flag["id"],
               "type": flag["type"], "not_after": "2027-03-01"}),
    );
    assert_eq!(again["red_flag"]["id"], flag["id"]);
    assert_eq!(again["red_flag"]["hard"], false);
    let r = f.ok("red_flags", json!({"path": p}));
    let dates: Vec<&str> = r["clashes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["date"].as_str().unwrap())
        .collect();
    assert_eq!(dates, ["2027-03-02", "2027-03-03"]);

    // Refusals write nothing.
    let rev = again["revision"].clone();
    let before = f.bytes(p);
    for (args, want) in [
        (
            json!({"path": p, "revision": rev, "element": kit, "type": flag["type"], "from": "2027-03-02", "not_before": "2027-03-02"}),
            "invalid_input",
        ),
        (
            json!({"path": p, "revision": rev, "element": kit, "type": flag["type"], "from": "2027-03-02"}),
            "invalid_input",
        ),
        (
            json!({"path": p, "revision": rev, "element": kit, "not_after": "2027-03-02"}),
            "invalid_input",
        ),
        (
            json!({"path": p, "revision": rev, "element": kit, "type": 999_999, "not_after": "2027-03-02"}),
            "not_found",
        ),
        (
            json!({"path": p, "revision": rev, "element": kit, "red_flag": 999_999, "type": flag["type"], "not_after": "2027-03-02"}),
            "not_found",
        ),
        (
            json!({"path": p, "revision": rev, "element": kit, "type": flag["type"], "from": "2027-03-05", "to": "2027-03-04"}),
            "refused",
        ),
        (
            json!({"path": p, "revision": set["revision"], "element": kit, "type": flag["type"], "not_after": "2027-03-02"}),
            "conflict",
        ),
    ] {
        let (kind, msg) = f.tool("set_red_flag", args).unwrap_err();
        assert_eq!(kind, want, "{msg}");
    }
    assert_eq!(f.bytes(p), before);

    let del = f.ok(
        "delete_red_flag",
        json!({"path": p, "revision": rev, "red_flag": flag["id"]}),
    );
    assert_eq!(del["changed"], true);
    let r = f.ok("red_flags", json!({"path": p}));
    assert_eq!(r["red_flags"], json!([]));
    assert_eq!(r["types"][0]["name"], "Unavailable", "the type stays");
    // A color for the type (decision 84), and back to none.
    let col = f.ok(
        "set_red_flag_type_color",
        json!({"path": p, "revision": r["revision"], "type": r["types"][0]["id"], "color": "#3366CC"}),
    );
    assert_eq!(col["color"], "#3366cc");
    let r2 = f.ok("red_flags", json!({"path": p}));
    assert_eq!(r2["types"][0]["color"], "#3366cc");
    let (kind, _) = f
        .tool(
            "set_red_flag_type_color",
            json!({"path": p, "revision": r2["revision"], "type": r["types"][0]["id"], "color": "blue"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    let none = f.ok(
        "set_red_flag_type_color",
        json!({"path": p, "revision": r2["revision"], "type": r["types"][0]["id"], "color": null}),
    );
    assert_eq!(none["color"], Value::Null);
    let (kind, _) = f
        .tool(
            "delete_red_flag",
            json!({"path": p, "revision": del["revision"], "red_flag": flag["id"]}),
        )
        .unwrap_err();
    assert_eq!(kind, "not_found");
    let (kind, _) = f
        .tool("red_flags", json!({"path": p, "sub_board": 999_999}))
        .unwrap_err();
    assert_eq!(kind, "not_found");
}

#[test]
fn a_sheet_field_takes_the_existing_spelling() {
    // Behavior note 25 (E-L12): `room 2` typed where a sheet has `ROOM 2` is `ROOM 2`.
    let mut f = Fixture::new("headerspelling", false);
    let p = "film.mtsc";
    let s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let (one, two) = (s.sheets[0].id.0, s.sheets[1].id.0);
    let typed = s.sheets[0].set.to_lowercase();
    assert_ne!(typed, s.sheets[0].set, "the fixture's set is in capitals");
    let rev = f.rev(p);
    f.ok(
        "set_sheet_fields",
        json!({"path": p, "revision": rev, "sheet": two, "fields": {"set": typed, "synopsis": "lower stays"}}),
    );
    let one_set = f.ok("get_sheet", json!({"path": p, "sheet": one}))["set"].clone();
    let got = f.ok("get_sheet", json!({"path": p, "sheet": two}));
    assert_eq!(got["set"], one_set);
    assert_eq!(got["synopsis"], "lower stays");
}

#[test]
fn merge_and_duplicate_sheets() {
    let mut f = Fixture::new("mergesheets", false);
    let p = "film.mtsc";
    let s = method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap();
    let ids: Vec<u32> = s.sheets.iter().map(|h| h.id.0).collect();
    let (one, two, three) = (ids[0], ids[1], ids[2]);
    let rev = f.rev(p);

    // Refusals write nothing.
    let before = f.bytes(p);
    for (args, want) in [
        (
            json!({"path": p, "revision": rev, "survivor": one, "others": [one]}),
            "refused",
        ),
        (
            json!({"path": p, "revision": rev, "survivor": one, "others": [two, two]}),
            "refused",
        ),
        (
            json!({"path": p, "revision": rev, "survivor": one, "others": [999_999]}),
            "not_found",
        ),
        (
            json!({"path": p, "revision": rev, "survivor": 999_999, "others": [two]}),
            "not_found",
        ),
        (
            json!({"path": p, "revision": rev, "survivor": one, "others": []}),
            "invalid_input",
        ),
        (
            json!({"path": p, "revision": "0000000000000000", "survivor": one, "others": [two]}),
            "conflict",
        ),
    ] {
        let (kind, msg) = f.tool("merge_sheets", args).unwrap_err();
        assert_eq!(kind, want, "{msg}");
    }
    assert_eq!(f.bytes(p), before);

    // Scene 2 survives scenes 3 and 1, joined in the order given; its own set stays, the
    // others' are dropped.
    let m = f.ok(
        "merge_sheets",
        json!({"path": p, "revision": rev, "survivor": two, "others": [three, one]}),
    );
    assert_eq!(m["changed"], true);
    assert_eq!(m["survivor"], two);
    assert_eq!(m["merged"], json!([three, one]));
    assert_eq!(m["scenes"], "2, 3, 1");
    assert_eq!(m["eighths"], 5);
    assert_eq!(m["pages"], "5/8");
    assert_eq!(
        m["combined"][0],
        json!({"field": "scenes", "before": "2", "after": "2, 3, 1"})
    );
    let dropped_sets: Vec<&str> = m["dropped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["field"] == "set")
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert_eq!(dropped_sets, ["HARBOR WALL", "NET LOFT"]);
    let added: Vec<&str> = m["elements_added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(added, ["HARBORMASTER LUND", "TOVE"]);
    let (kind, _) = f
        .tool("get_sheet", json!({"path": p, "sheet": one}))
        .unwrap_err();
    assert_eq!(kind, "not_found");
    let read = f.ok("read_schedule", json!({"path": p}));
    assert_eq!(read["counts"]["sheets"], 3);
    assert_eq!(f.ok("check", json!({"path": p}))["ok"], true);

    // Duplicate the survivor: the copy lands right after it on Main.
    let d = f.ok(
        "duplicate_sheet",
        json!({"path": p, "revision": m["revision"], "sheet": two}),
    );
    assert_eq!(d["changed"], true);
    assert_eq!(d["original"], two);
    let copy = d["sheet"].as_u64().unwrap();
    assert_eq!(d["places"][0]["sub_board"], s.boards[0].sub_boards[0].id.0);
    assert_eq!(d["places"][0]["index"], 1);
    let got = f.ok("get_sheet", json!({"path": p, "sheet": copy}));
    assert_eq!(got["scenes"], "2, 3, 1");
    assert_eq!(got["places"][0]["index"], 1);
    let (kind, _) = f
        .tool(
            "duplicate_sheet",
            json!({"path": p, "revision": d["revision"], "sheet": 999_999}),
        )
        .unwrap_err();
    assert_eq!(kind, "not_found");
    let (kind, _) = f
        .tool(
            "duplicate_sheet",
            json!({"path": p, "revision": m["revision"], "sheet": two}),
        )
        .unwrap_err();
    assert_eq!(kind, "conflict", "a stale revision");
}

#[test]
fn paths_are_confined_to_the_roots() {
    let mut f = Fixture::new("paths", false);
    let outside = f.base.join("outside/o.mtsc");
    std::fs::write(&outside, f.bytes("film.mtsc")).unwrap();
    std::fs::create_dir_all(f.root.join(".git")).unwrap();
    std::fs::write(f.root.join(".git/h.mtsc"), f.bytes("film.mtsc")).unwrap();
    for p in [
        "../outside/o.mtsc",
        outside.to_str().unwrap(),
        ".git/h.mtsc",
        "harbor.fountain",
    ] {
        let (kind, _) = f.tool("read_schedule", json!({"path": p})).unwrap_err();
        assert_eq!(kind, "path_refused", "{p}");
    }
    let (kind, _) = f
        .tool("new_schedule", json!({"path": "../outside/new.mtsc"}))
        .unwrap_err();
    assert_eq!(kind, "path_refused");
    assert!(!f.base.join("outside/new.mtsc").exists());
    let (kind, _) = f
        .tool("list_schedules", json!({"folder": "../outside"}))
        .unwrap_err();
    assert_eq!(kind, "path_refused");
    let rev = f.rev("film.mtsc");
    let (kind, _) = f
        .tool(
            "import_script",
            json!({"path": "film.mtsc", "revision": rev, "script": "../outside/x.fountain"}),
        )
        .unwrap_err();
    assert!(kind == "path_refused" || kind == "not_found", "{kind}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(&outside, f.root.join("link.mtsc")).unwrap();
        let (kind, _) = f
            .tool("read_schedule", json!({"path": "link.mtsc"}))
            .unwrap_err();
        assert_eq!(kind, "path_refused", "a link that leads out");
        symlink(f.root.join("film.mtsc"), f.root.join("inner.mtsc")).unwrap();
        let rev = f.rev("inner.mtsc");
        let before = f.bytes("film.mtsc");
        let (kind, msg) = f
            .tool(
                "set_start_date",
                json!({"path": "inner.mtsc", "revision": rev, "date": null}),
            )
            .unwrap_err();
        assert_eq!(kind, "path_refused", "nothing is written through a link");
        assert!(msg.contains("symbolic link"));
        assert_eq!(f.bytes("film.mtsc"), before);
        let listed = f.ok("list_schedules", json!({}));
        assert_eq!(
            listed["files"].as_array().unwrap().len(),
            1,
            "links are skipped"
        );
    }
}

#[test]
fn oversized_files_are_refused() {
    let mut f = Fixture::new("big", false);
    let big = std::fs::File::create(f.root.join("big.mtsc")).unwrap();
    big.set_len(limits::MAX_FILE_BYTES + 1).unwrap();
    let (kind, msg) = f
        .tool("read_schedule", json!({"path": "big.mtsc"}))
        .unwrap_err();
    assert_eq!(kind, "refused");
    assert!(msg.contains("larger than"), "{msg}");
}

#[test]
fn legacy_handshake_negotiates_the_version() {
    let mut f = Fixture::new("handshake", false);
    for v in LEGACY {
        let r = f.rpc("initialize", json!({"protocolVersion": v, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
        assert_eq!(r["result"]["protocolVersion"], json!(v));
        assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], false);
        assert!(r["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("never instructions"));
    }
    for v in ["1999-01-01", MODERN] {
        let r = f.rpc(
            "initialize",
            json!({"protocolVersion": v, "capabilities": {}}),
        );
        assert_eq!(r["result"]["protocolVersion"], json!(LEGACY[0]));
    }
    let r = f.rpc("initialize", json!({"capabilities": {}}));
    assert_eq!(error_code(&r), -32602);
    // Older revisions get text-only results.
    f.rpc(
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}}),
    );
    let r = f.rpc(
        "tools/call",
        json!({"name": "check", "arguments": {"path": "film.mtsc"}}),
    );
    assert!(r["result"].get("structuredContent").is_none());
    assert_eq!(r["result"]["content"].as_array().unwrap().len(), 2);
}

#[test]
fn requests_before_initialize_and_modern_requests() {
    let (base, root) = temp_root("modern");
    let mut s = Server::new(&Config {
        roots: vec![root],
        read_only: false,
    })
    .unwrap();
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .unwrap();
    assert_eq!(error_code(&r), -32602);
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}))
        .unwrap();
    assert_eq!(r["result"], json!({}));
    let meta = |v: &str| json!({"io.modelcontextprotocol/protocolVersion": v, "io.modelcontextprotocol/clientCapabilities": {}});
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 3, "method": "server/discover", "params": {"_meta": meta(MODERN)}}))
        .unwrap();
    assert_eq!(r["result"]["resultType"], "complete");
    assert_eq!(
        r["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "method-schedule"
    );
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"_meta": meta(MODERN), "name": "check", "arguments": {"path": "film.mtsc"}}}))
        .unwrap();
    assert_eq!(r["result"]["structuredContent"]["ok"], true);
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {"_meta": meta("1900-01-01")}}))
        .unwrap();
    assert_eq!(error_code(&r), -32022);
    let _ = std::fs::remove_dir_all(&base);
}

/// Revision 2026-07-28's cacheable results (server/discover and tools/list) carry
/// `ttlMs` and `cacheScope`; other results don't, and the earlier revisions never see
/// them. Claude Code 2.1.280 speaks this revision and asked for the list again and again
/// while the hints were missing.
#[test]
fn modern_lists_carry_caching_hints_and_legacy_lists_do_not() {
    let (base, root) = temp_root("cache-hints");
    let mut s = Server::new(&Config {
        roots: vec![root],
        read_only: false,
    })
    .unwrap();
    let meta = json!({"io.modelcontextprotocol/protocolVersion": MODERN, "io.modelcontextprotocol/clientCapabilities": {}});
    for (id, method) in [(1, "server/discover"), (2, "tools/list")] {
        let r = s
            .handle(
                json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {"_meta": meta}}),
            )
            .unwrap();
        let result = &r["result"];
        assert_eq!(result["resultType"], "complete", "{method}");
        assert_eq!(result["ttlMs"], json!(LIST_TTL_MS), "{method}");
        assert!(
            result["ttlMs"].is_u64(),
            "{method}: a whole number, never negative"
        );
        assert_eq!(result["cacheScope"], "public", "{method}");
    }
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"_meta": meta, "name": "check", "arguments": {"path": "film.mtsc"}}}))
        .unwrap();
    assert!(
        r["result"].get("ttlMs").is_none(),
        "a tool call is not cacheable"
    );
    assert!(r["result"].get("cacheScope").is_none());
    let _ = std::fs::remove_dir_all(&base);

    let (base, root) = temp_root("cache-hints-legacy");
    let mut s = Server::new(&Config {
        roots: vec![root],
        read_only: false,
    })
    .unwrap();
    s.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}))
        .unwrap();
    let r = s
        .handle(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))
        .unwrap();
    assert!(r["result"]["tools"].is_array());
    assert!(
        r["result"].get("ttlMs").is_none(),
        "2025-06-18 has no caching hints"
    );
    assert!(r["result"].get("cacheScope").is_none());
    let _ = std::fs::remove_dir_all(&base);
}

/// The tools MCP.md lists are exactly the tools the server answers: its group table, its
/// counts line, and a `**`name`**` paragraph for each one.
#[test]
fn mcp_md_lists_every_tool() {
    let doc = include_str!("../../../../docs/MCP.md");
    let names = |text: &str| -> BTreeSet<String> {
        text.split('`')
            .skip(1)
            .step_by(2)
            .filter(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
            .map(str::to_string)
            .collect()
    };
    let table = doc
        .split("| Group | Tools |")
        .nth(1)
        .and_then(|t| t.split("\n\n").next())
        .expect("the group table");
    let listed = names(table);
    let mut f = Fixture::new("mcpmd", false);
    let served: BTreeSet<String> = f.rpc("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(listed, served, "MCP.md's group table");
    let mut ro = Fixture::new("mcpmdro", true);
    let read_only = ro.rpc("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .len();
    let counts = format!(
        "{} tools; `--read-only` keeps the {read_only} that read.",
        served.len()
    );
    assert!(doc.contains(&counts), "MCP.md should say: {counts}");
    for name in &served {
        assert!(
            doc.contains(&format!("**`{name}`**")),
            "MCP.md has no paragraph for {name}"
        );
    }
}

#[test]
fn tools_list_schemas_are_valid() {
    let mut f = Fixture::new("schemas", false);
    let r = f.rpc("tools/list", json!({}));
    let tools = r["result"]["tools"].as_array().unwrap().clone();
    assert_eq!(tools.len(), 74);
    for t in &tools {
        let name = t["name"].as_str().unwrap();
        schema::check(&t["inputSchema"], name).unwrap_or_else(|e| panic!("{e}"));
        assert!(t["description"].as_str().is_some_and(|d| d.len() > 20));
        assert_eq!(t["annotations"]["openWorldHint"], false);
        assert!(!t.to_string().contains('\u{2014}'), "{name}: em dash");
    }
    let again = f.rpc("tools/list", json!({}));
    assert_eq!(again["result"]["tools"], json!(tools));
    // Arguments that don't fit the schema are a tool error the model can read.
    let (kind, msg) = f
        .tool("get_sheet", json!({"path": "film.mtsc"}))
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    assert!(msg.contains("`sheet` is required"), "{msg}");
    let (_, msg) = f
        .tool("check", json!({"path": "film.mtsc", "bogus": 1}))
        .unwrap_err();
    assert!(msg.contains("unknown property `bogus`"), "{msg}");
    let (_, msg) = f
        .tool(
            "set_start_date",
            json!({"path": "film.mtsc", "revision": "short", "date": null}),
        )
        .unwrap_err();
    assert!(msg.contains("revision"), "{msg}");
}

#[test]
fn bad_messages_get_json_rpc_errors() {
    let mut f = Fixture::new("badmsg", false);
    let cases = [
        (json!(42), -32600, Value::Null),
        (json!({"id": 1, "method": "ping"}), -32600, json!(1)),
        (
            json!({"jsonrpc": "2.0", "id": 1.5, "method": "ping"}),
            -32600,
            Value::Null,
        ),
        (
            json!({"jsonrpc": "2.0", "id": "x", "method": "nope/never"}),
            -32601,
            json!("x"),
        ),
        (
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "no_such_tool"}}),
            -32602,
            json!(3),
        ),
        (json!([]), -32600, Value::Null),
    ];
    for (msg, code, id) in cases {
        let r = f.raw(msg.clone());
        assert_eq!(error_code(&r), code, "{msg}: {r}");
        assert_eq!(r["id"], id, "{msg}");
    }
}

#[test]
fn lines_are_framed_and_bounded() {
    let mut input: &[u8] = b"{\"a\":1}\r\n\nnot json\n";
    let mut buf = Vec::new();
    assert_eq!(read_line(&mut input, 100, &mut buf).unwrap(), Some(true));
    assert_eq!(
        parse_line(&buf, true),
        Some(Incoming::Message(json!({"a": 1})))
    );
    assert_eq!(read_line(&mut input, 100, &mut buf).unwrap(), Some(true));
    assert_eq!(parse_line(&buf, true), None);
    assert_eq!(read_line(&mut input, 100, &mut buf).unwrap(), Some(true));
    assert!(matches!(parse_line(&buf, true), Some(Incoming::Invalid(_))));
    assert_eq!(read_line(&mut input, 100, &mut buf).unwrap(), None);
    let long = format!("{}\n{{\"b\":2}}\n", "x".repeat(50));
    let mut input = std::io::BufReader::with_capacity(8, long.as_bytes());
    assert_eq!(read_line(&mut input, 20, &mut buf).unwrap(), Some(false));
    assert!(
        matches!(parse_line(&buf, false), Some(Incoming::Invalid(e)) if e.contains(&limits::MAX_MESSAGE_BYTES.to_string()))
    );
    assert_eq!(read_line(&mut input, 20, &mut buf).unwrap(), Some(true));
}

#[test]
fn cancelled_requests_never_run() {
    let (base, root) = temp_root("cancel");
    let before = std::fs::read(root.join("film.mtsc")).unwrap();
    let rev = files::revision(&before);
    let server = Server::new(&Config {
        roots: vec![root.clone()],
        read_only: false,
    })
    .unwrap();
    let msg = Incoming::Message;
    let mut out = Vec::new();
    run(
        server,
        Scripted(VecDeque::from(vec![
            msg(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}}})),
            msg(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "set_start_date", "arguments": {"path": "film.mtsc", "revision": rev, "date": null}}})),
            msg(json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 5}})),
            msg(json!({"jsonrpc": "2.0", "id": 6, "method": "ping"})),
        ])),
        &mut out,
    )
    .unwrap();
    let ids: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["id"].clone())
        .collect();
    assert_eq!(ids, [json!(1), json!(6)]);
    assert_eq!(std::fs::read(root.join("film.mtsc")).unwrap(), before);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn element_details_categories_quantities_links_and_headers() {
    let mut f = Fixture::new("details", false);
    let p = "film.mtsc";
    let rev = f.rev(p);
    let auto = f.ok(
        "auto_day_breaks",
        json!({"path": p, "revision": rev, "by": "eighths", "limit": 2}),
    );
    let els = f.ok("list_elements", json!({"path": p, "category": "Cast"}));
    let cast = els["categories"][0]["id"].clone();
    let id = |n: &str| {
        els["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == n)
            .unwrap()["id"]
            .clone()
    };
    let (kit, tove) = (id("KIT"), id("TOVE"));
    let cat = f.ok(
        "set_category",
        json!({"path": p, "revision": auto["revision"], "category": cast, "properties": ["Agent", "Height"], "exclude_from_board": true}),
    );
    assert_eq!(cat["properties"], json!(["Agent", "Height"]));
    assert_eq!(cat["exclude_from_board"], true);
    let det = f.ok(
        "set_element_details",
        json!({"path": p, "revision": cat["revision"], "element": kit, "notes": "Left-handed.", "properties": [{"name": "height", "value": "1.80 m"}], "links": [tove], "retroactive": true}),
    );
    assert_eq!(
        det["properties"],
        json!([{"name": "Height", "value": "1.80 m"}])
    );
    assert_eq!(det["links"], json!([tove]));
    assert_eq!(det["exclude_from_board"], false);
    assert_eq!(
        det["excluded"], false,
        "the category's setting is a default for new elements (decision 120)"
    );
    let det = f.ok(
        "set_element_details",
        json!({"path": p, "revision": det["revision"], "element": kit, "exclude_from_board": true}),
    );
    assert_eq!(det["exclude_from_board"], true);
    assert_eq!(det["notes"], "Left-handed.", "the rest is kept");
    let (kind, _) = f
        .tool(
            "set_element_details",
            json!({"path": p, "revision": det["revision"], "element": kit, "properties": [{"name": "Weight", "value": "x"}]}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");
    let listed = f.ok("list_elements", json!({"path": p, "category": "Cast"}));
    let k = listed["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == kit)
        .unwrap()
        .clone();
    assert_eq!(k["notes"], "Left-handed.");
    assert_eq!(k["exclude_from_board"], true);
    assert_eq!(k["excluded"], true);
    assert_eq!(
        listed["categories"][0]["properties"],
        json!(["Agent", "Height"])
    );
    // Renaming by case and dropping a property keeps the other's values.
    let cat = f.ok(
        "set_category",
        json!({"path": p, "revision": det["revision"], "category": cast, "properties": ["HEIGHT"]}),
    );
    assert_eq!(cat["properties"], json!(["HEIGHT"]));
    // A quantity on a sheet the element is on.
    let read = f.ok("read_schedule", json!({"path": p}));
    let first = read["sheets"][0]["id"].clone();
    let got = f.ok("get_sheet", json!({"path": p, "sheet": first}));
    let on = got["elements"][0].clone();
    let q = f.ok(
        "set_quantity",
        json!({"path": p, "revision": cat["revision"], "sheet": first, "element": on, "count": 3, "daily": false}),
    );
    assert_eq!(q["count"], 3);
    let got = f.ok("get_sheet", json!({"path": p, "sheet": first}));
    assert_eq!(
        got["quantities"],
        json!([{"element": on, "count": 3, "daily": false}])
    );
    // Unavailable and camera test are kinds of their own.
    f.ok(
        "add_event",
        json!({"path": p, "revision": q["revision"], "kind": "camera_test", "from": "2027-03-01", "to": "2027-03-01", "elements": [kit]}),
    );
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(days["calendar"]["events"][0]["kind"], "camera_test");
    // A header field as a category.
    let sets = f.ok("dood", json!({"path": p, "header": "set"}));
    let names: Vec<&str> = sets["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"NET LOFT"), "{names:?}");
}

/// Templates (`.mtst`, decisions 77 and 85) are read, never changed: as `.mtst`, and as a
/// `.json` file marked as a template.
#[test]
fn templates_are_read_only() {
    let mut f = Fixture::new("templates", false);
    let text = method_schedule_format::to_template_string(&harbor());
    std::fs::write(f.root.join("series.mtst"), &text).unwrap();
    std::fs::write(f.root.join("series.json"), &text).unwrap();
    for path in ["series.mtst", "series.json"] {
        let read = f.ok("read_schedule", json!({"path": path}));
        assert_eq!(read["template"], json!(true), "{path}");
        assert_eq!(read["canonical"], json!(true), "{path}");
        assert_eq!(read["counts"]["sheets"], json!(0));
        let rev = read["revision"].as_str().unwrap().to_string();
        let (kind, why) = f
            .tool(
                "set_start_date",
                json!({"path": path, "revision": rev, "date": "2030-01-07"}),
            )
            .unwrap_err();
        assert_eq!(kind, "refused", "{path}");
        assert!(why.contains("is a template"), "{why}");
        assert_eq!(f.bytes(path), text.as_bytes(), "{path}");
        assert_eq!(
            f.ok("check", json!({"path": path}))["canonical"],
            json!(true)
        );
    }
    let film = f.ok("read_schedule", json!({"path": "film.mtsc"}));
    assert_eq!(film["template"], json!(false));
    let listed = f.ok("list_schedules", json!({}));
    let names: Vec<&str> = listed["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["path"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"series.mtst"), "{names:?}");
}

#[test]
fn banners_insert_list_and_restyle() {
    let mut f = Fixture::new("banners", false);
    let p = "film.mtsc";
    let rev = f.rev(p);
    let ins = f.ok(
        "insert_banner",
        json!({"path": p, "revision": rev, "index": 0, "text": "Company move", "minutes": 45,
            "bold": true, "color": "teal", "tall": true, "size": "large", "align": "center",
            "font": "serif"}),
    );
    assert_eq!(
        ins["style"],
        json!({"bold": true, "color": "teal", "tall": true, "size": "large", "align": "center",
            "font": "serif"})
    );
    // Listed with its style where banners are listed.
    let days = f.ok("days", json!({"path": p}));
    let listed = &days["unscheduled"][0];
    assert_eq!(listed["kind"], "banner");
    assert_eq!(listed["strip"], ins["strip"]);
    assert_eq!(listed["style"]["color"], "teal");
    // What is left out is kept; null color is the board's; left is the usual way.
    let set = f.ok(
        "set_banner",
        json!({"path": p, "revision": ins["revision"], "strip": ins["strip"], "color": null,
            "align": "left", "bold": false}),
    );
    assert_eq!(
        set["style"],
        json!({"bold": false, "color": null, "tall": true, "size": "large", "align": "left",
            "font": "serif"})
    );
    assert_eq!(set["text"], "Company move");
    assert_eq!(set["minutes"], 45);
    // Unknown names are refused by the schema; a sheet's strip is not a banner.
    let (kind, _) = f
        .tool(
            "set_banner",
            json!({"path": p, "revision": set["revision"], "strip": ins["strip"], "color": "mauve"}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");
    let sheet_strip = f.ok("days", json!({"path": p}))["unscheduled"][1]["strip"].clone();
    let (kind, _) = f
        .tool(
            "set_banner",
            json!({"path": p, "revision": set["revision"], "strip": sheet_strip, "bold": true}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");
}

#[test]
fn strips_sort_by_up_to_four_fields() {
    let mut f = Fixture::new("sort", false);
    let p = "film.mtsc";
    let rev = f.rev(p);
    let brk = f.ok(
        "insert_day_break",
        json!({"path": p, "revision": rev, "index": 2}),
    );
    let ban = f.ok(
        "insert_banner",
        json!({"path": p, "revision": brk["revision"], "index": 4, "text": "Lunch"}),
    );
    let sorted = f.ok(
        "sort_strips",
        json!({"path": p, "revision": ban["revision"],
            "keys": [{"field": "scene", "descending": true}]}),
    );
    assert_eq!(sorted["sorted"], 5);
    // The banner first, the day break gone, scenes in natural order backwards.
    let days = f.ok("days", json!({"path": p}));
    assert_eq!(days["days"], json!([]));
    let order: Vec<String> = days["unscheduled"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| match x["scenes"].as_str() {
            Some(n) => n.to_string(),
            None => format!("[{}]", x["text"].as_str().unwrap_or("")),
        })
        .collect();
    assert_eq!(order, ["[Lunch]", "4", "3A", "3", "2", "1"]);
    // Five keys, a key twice, an unknown field and none are refused; nothing is written.
    let before = f.bytes(p);
    for keys in [
        json!([{"field": "set"}, {"field": "unit"}, {"field": "episode"}, {"field": "scene"},
            {"field": "pages"}]),
        json!([{"field": "set"}, {"field": "set", "descending": true}]),
        json!([{"field": "cast"}]),
        json!([]),
    ] {
        let (kind, _) = f
            .tool(
                "sort_strips",
                json!({"path": p, "revision": sorted["revision"], "keys": keys}),
            )
            .unwrap_err();
        assert_eq!(kind, "invalid_input", "{keys}");
    }
    assert_eq!(f.bytes(p), before);
}

#[test]
fn sheets_elements_and_categories_are_added_merged_moved_and_deleted() {
    let mut f = Fixture::new("elements", false);
    let p = "film.mtsc";
    let read = |f: &Fixture| {
        method_schedule_format::from_str(&String::from_utf8(f.bytes(p)).unwrap()).unwrap()
    };
    let s0 = read(&f);
    let cast = s0.cast().unwrap();
    let el = |s: &Schedule, n: &str| s.element_named(cast, n).unwrap().0;
    let (kit, tove, lund) = (
        el(&s0, "KIT"),
        el(&s0, "TOVE"),
        el(&s0, "HARBORMASTER LUND"),
    );
    let first = s0.sheets_in_board_order(s0.boards[0].id)[0];
    let rev = f.rev(p);

    // New Sheet after a sheet on Main goes right after it; without one, to the Boneyard.
    let made = f.ok(
        "new_sheet",
        json!({"path": p, "revision": rev, "after": first.0}),
    );
    assert_eq!(made["index"], 1);
    assert!(made["sub_board"].is_u64());
    let parked = f.ok(
        "new_sheet",
        json!({"path": p, "revision": made["revision"]}),
    );
    assert_eq!(parked["sub_board"], Value::Null);
    let gone = f.ok(
        "delete_sheet",
        json!({"path": p, "revision": parked["revision"], "sheet": parked["sheet"]}),
    );
    assert_eq!(read(&f).sheets.len(), s0.sheets.len() + 1);

    // Rename, then a name taken in the category ignoring case is refused.
    let named = f.ok(
        "rename_element",
        json!({"path": p, "revision": gone["revision"], "element": lund, "name": "Lund"}),
    );
    assert_eq!(named["name"], "Lund");
    let (kind, _) = f
        .tool(
            "rename_element",
            json!({"path": p, "revision": named["revision"], "element": lund, "name": "kit"}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");

    // Categories: add, rename and move through set_category, then a merge across them is
    // refused (E-L6) and a move needs a free name.
    let props = f.ok(
        "add_category",
        json!({"path": p, "revision": named["revision"], "name": "Props"}),
    );
    let (kind, _) = f
        .tool(
            "add_category",
            json!({"path": p, "revision": props["revision"], "name": "props"}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused");
    let renamed = f.ok(
        "set_category",
        json!({"path": p, "revision": props["revision"], "category": props["category"], "name": "Hand props", "position": 0}),
    );
    assert_eq!(renamed["name"], "Hand props");
    let s = read(&f);
    let users: Vec<&str> = s
        .categories
        .iter()
        .filter(|c| !c.cast)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(users.first(), Some(&"Hand props"), "{users:?}");
    let moved = f.ok(
        "move_element",
        json!({"path": p, "revision": renamed["revision"], "element": lund, "category": props["category"]}),
    );
    assert_eq!(moved["category"], props["category"]);
    let (kind, msg) = f
        .tool(
            "merge_elements",
            json!({"path": p, "revision": moved["revision"], "survivor": kit, "others": [lund]}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused", "{msg}");
    assert!(msg.contains("one category"), "{msg}");
    let (kind, _) = f
        .tool(
            "delete_category",
            json!({"path": p, "revision": moved["revision"], "category": props["category"]}),
        )
        .unwrap_err();
    assert_eq!(kind, "refused", "it still has Lund");

    // Merge within the cast: the survivor gets every sheet of both.
    let both = s0
        .sheets
        .iter()
        .filter(|h| h.elements.iter().any(|e| e.0 == kit || e.0 == tove))
        .count();
    let merged = f.ok(
        "merge_elements",
        json!({"path": p, "revision": moved["revision"], "survivor": kit, "others": [tove]}),
    );
    assert_eq!(merged["sheets"], both);
    assert_eq!(merged["merged"], json!([tove]));
    let (kind, _) = f
        .tool(
            "merge_elements",
            json!({"path": p, "revision": merged["revision"], "survivor": kit, "others": [kit]}),
        )
        .unwrap_err();
    assert_eq!(kind, "invalid_input");

    // Renumber from 5 with a locked 5: KIT skips to 6.
    let back = f.ok(
        "move_element",
        json!({"path": p, "revision": merged["revision"], "element": lund, "category": cast.0}),
    );
    let locked = f.ok(
        "set_board_id",
        json!({"path": p, "revision": back["revision"], "element": lund, "board_id": "5", "locked": true}),
    );
    let num = f.ok(
        "renumber_elements",
        json!({"path": p, "revision": locked["revision"], "category": cast.0, "start": 5}),
    );
    let s2 = read(&f);
    let id = |e: u32| {
        s2.element(method_schedule_core::model::ElementId(e))
            .unwrap()
            .board_id
            .clone()
    };
    assert_eq!(id(lund).as_deref(), Some("5"));
    assert_eq!(id(kit).as_deref(), Some("6"));
    assert_eq!(num["elements"].as_array().unwrap().len(), 2);
    assert!(s2
        .element(method_schedule_core::model::ElementId(tove))
        .is_none());

    // Delete: an empty category goes; an element leaves every sheet.
    let del = f.ok(
        "delete_category",
        json!({"path": p, "revision": num["revision"], "category": props["category"]}),
    );
    let del = f.ok(
        "delete_element",
        json!({"path": p, "revision": del["revision"], "element": kit}),
    );
    let s3 = read(&f);
    assert!(s3.category_named("Hand props").is_none());
    assert!(s3
        .sheets
        .iter()
        .all(|h| h.elements.iter().all(|e| e.0 != kit)));
    assert_eq!(del["changed"], true);
}
