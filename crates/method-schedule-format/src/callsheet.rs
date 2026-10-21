//! Call sheet data for one shoot day as JSON or CSV (docs/CALLSHEET.md; decisions.md,
//! BYD-02). The data comes from method_schedule_core::callsheet; this only writes it.
//! Every number is an integer: times are minutes after midnight (with `HH:MM` text
//! beside them), lengths are eighths (with page text beside them).

use serde_json::{json, Value};

use method_schedule_core::callsheet::CallSheet;
use method_schedule_core::plan::clock_text;
use method_schedule_core::solar::Sun;

use crate::export::csv::{self as csvw, Delimiter};
use crate::export::{Cell, Sheet, Workbook};

/// The version of the call sheet layout below.
pub const CALLSHEET_VERSION: u32 = 1;

fn time(m: Option<i32>) -> Value {
    m.map_or(Value::Null, |m| json!(m))
}

fn time_text(m: Option<i32>) -> Value {
    m.map_or(Value::Null, |m| json!(clock_text(m)))
}

/// The call sheet as pretty JSON with a trailing newline.
pub fn to_json(c: &CallSheet) -> String {
    let (rise, set, sun_state) = match c.sun {
        Some(Sun::Times { rise, set }) => (Some(rise), Some(set), "times"),
        Some(Sun::AlwaysUp) => (None, None, "always_up"),
        Some(Sun::AlwaysDown) => (None, None, "always_down"),
        None => (None, None, "unknown"),
    };
    let v = json!({
        "format": "method-schedule-callsheet",
        "version": CALLSHEET_VERSION,
        "title": c.title,
        "company": c.company,
        "director": c.director,
        "producer": c.producer,
        "board": c.board,
        "sub_board": c.sub_board,
        "day": c.day,
        "days": c.days,
        "date": c.date.map(|d| d.to_string()),
        "call_minutes": c.call,
        "call": time_text(c.call.map(|m| m as i32)),
        "wrap_minutes": time(c.wrap),
        "wrap": time_text(c.wrap),
        "length_minutes": c.length,
        "eighths": c.pages.0,
        "pages": c.pages.to_string(),
        "sun": {
            "state": sun_state,
            "location": c.sun_location,
            "sunrise_minutes": time(rise),
            "sunrise": time_text(rise),
            "sunset_minutes": time(set),
            "sunset": time_text(set),
        },
        "scenes": c.scenes.iter().map(|x| json!({
            "sheet": x.sheet.0,
            "scenes": x.scenes,
            "int_ext": x.int_ext,
            "set": x.set,
            "time_of_day": x.time_of_day,
            "eighths": x.pages.0,
            "pages": x.pages.to_string(),
            "synopsis": x.synopsis,
            "location": x.location,
            "cast": x.cast,
            "shoot_minutes": x.shoot_minutes,
            "start_minutes": time(x.start),
            "start": time_text(x.start),
            "end_minutes": time(x.end),
            "end": time_text(x.end),
        })).collect::<Vec<_>>(),
        "cast": c.cast.iter().map(|x| json!({
            "element": x.element.0,
            "board_id": x.board_id,
            "name": x.name,
            "scenes": x.scenes,
            "first_minutes": time(x.first),
            "first": time_text(x.first),
            "last_minutes": time(x.last),
            "last": time_text(x.last),
            "call_minutes": time(x.call),
            "call": time_text(x.call),
            "wrap_minutes": time(x.wrap),
            "wrap": time_text(x.wrap),
            "own_call": x.own,
        })).collect::<Vec<_>>(),
        "elements": c.elements.iter().map(|cat| json!({
            "category": cat.category.0,
            "name": cat.name,
            "elements": cat.elements.iter().map(|x| json!({
                "element": x.element.0,
                "board_id": x.board_id,
                "name": x.name,
                "scenes": x.scenes,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "notes": c.notes,
    });
    let mut text = serde_json::to_string_pretty(&v).expect("json always serializes");
    text.push('\n');
    text
}

fn opt(m: Option<i32>) -> Cell {
    m.map_or(Cell::Empty, |m| Cell::text(clock_text(m)))
}

/// The call sheet as tables: Day (field and value), Scenes, Cast, Elements and Notes.
pub fn workbook(c: &CallSheet) -> Workbook {
    let (rise, set) = c.sun_text();
    let row = |k: &str, v: Cell| vec![Cell::text(k), v];
    let day = Sheet {
        name: "Day",
        columns: vec!["Field".into(), "Value".into()],
        rows: vec![
            row("Title", Cell::text(c.title.clone())),
            row("Company", Cell::text(c.company.clone())),
            row("Director", Cell::text(c.director.clone())),
            row("Producer", Cell::text(c.producer.clone())),
            row("Board", Cell::text(c.board.clone())),
            row("Sub-board", Cell::text(c.sub_board.clone())),
            row("Day", Cell::text(format!("{} of {}", c.day, c.days))),
            row("Date", Cell::date(c.date)),
            row("Call", opt(c.call.map(|m| m as i32))),
            row("Wrap", opt(c.wrap)),
            row(
                "Length",
                Cell::text(method_schedule_core::plan::hours_text(c.length)),
            ),
            row("Pages", Cell::text(c.pages.to_string())),
            row(
                "Sun at",
                Cell::text(c.sun_location.clone().unwrap_or_default()),
            ),
            row("Sunrise", Cell::text(rise.unwrap_or_default())),
            row("Sunset", Cell::text(set.unwrap_or_default())),
        ],
    };
    let scenes = Sheet {
        name: "Scenes",
        columns: [
            "Start",
            "End",
            "Scene",
            "I/E",
            "Set",
            "Day/Night",
            "Pages",
            "Cast",
            "Location",
            "Synopsis",
        ]
        .iter()
        .map(|x| x.to_string())
        .collect(),
        rows: c
            .scenes
            .iter()
            .map(|x| {
                vec![
                    opt(x.start),
                    opt(x.end),
                    Cell::text(x.scenes.clone()),
                    Cell::text(x.int_ext.clone()),
                    Cell::text(x.set.clone()),
                    Cell::text(x.time_of_day.clone()),
                    Cell::text(x.pages.to_string()),
                    Cell::text(x.cast.join(", ")),
                    Cell::text(x.location.clone()),
                    Cell::text(x.synopsis.clone()),
                ]
            })
            .collect(),
    };
    let cast = Sheet {
        name: "Cast",
        columns: ["ID", "Name", "Scenes", "First", "Last", "Call", "Wrap"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        rows: c
            .cast
            .iter()
            .map(|x| {
                vec![
                    Cell::text(x.board_id.clone().unwrap_or_default()),
                    Cell::text(x.name.clone()),
                    Cell::text(x.scenes.join(", ")),
                    opt(x.first),
                    opt(x.last),
                    opt(x.call),
                    opt(x.wrap),
                ]
            })
            .collect(),
    };
    let elements = Sheet {
        name: "Elements",
        columns: ["Category", "ID", "Name", "Scenes"]
            .iter()
            .map(|x| x.to_string())
            .collect(),
        rows: c
            .elements
            .iter()
            .flat_map(|cat| {
                cat.elements.iter().map(|x| {
                    vec![
                        Cell::text(cat.name.clone()),
                        Cell::text(x.board_id.clone().unwrap_or_default()),
                        Cell::text(x.name.clone()),
                        Cell::text(x.scenes.join(", ")),
                    ]
                })
            })
            .collect(),
    };
    let notes = Sheet {
        name: "Notes",
        columns: vec!["Note".into()],
        rows: c
            .notes
            .iter()
            .map(|n| vec![Cell::text(n.clone())])
            .collect(),
    };
    Workbook {
        title: c.title.clone(),
        sheets: vec![day, scenes, cast, elements, notes],
    }
}

/// The call sheet as one CSV file: each table of [`workbook`] in turn, each with its
/// header row, a blank line between them. Starts with a byte order mark for Excel.
pub fn to_csv(c: &CallSheet) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, sheet) in workbook(c).sheets.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(b"\r\n");
        }
        out.extend(csvw::write(sheet, Delimiter::Comma, i == 0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::callsheet::call_sheet;
    use method_schedule_core::model::{Banner, Minutes, Place, Sheet as Header, StripKind};
    use method_schedule_core::plan::{set_call_time, GeoLocation};
    use method_schedule_core::Schedule;

    fn one_day() -> CallSheet {
        let mut s = Schedule::new("ZTEST Call");
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        s.set_start(b, sb, "2024-06-20".parse().ok()).unwrap();
        let cast = s.cast().unwrap();
        let ann = s.add_element(cast, "ANN").unwrap();
        s.set_board_id(ann, Some("1")).unwrap();
        let h = s.add_sheet(Header {
            scenes: "12".into(),
            int_ext: "EXT".into(),
            set: "PIER".into(),
            time_of_day: "DAY".into(),
            location: "Pier".into(),
            synopsis: "=She waits, then runs".into(),
            pages: method_schedule_core::Eighths(2),
            shoot_time: Some(Minutes(90)),
            ..Header::default()
        });
        s.tag(h, ann).unwrap();
        let props = s.add_category("Hand props");
        let rope = s.add_element(props, "ROPE").unwrap();
        s.tag(h, rope).unwrap();
        let st = s.boards[0].boneyard[0].id;
        let at = |i| Place {
            sub_board: Some(sb),
            index: i,
        };
        s.move_strips(b, &[st], at(0)).unwrap();
        s.insert_strip(
            b,
            StripKind::Banner(Banner {
                text: "Lunch".into(),
                duration: Minutes(30),
                style: Default::default(),
            }),
            at(1),
        )
        .unwrap();
        let d = s.insert_strip(b, StripKind::DayBreak, at(2)).unwrap();
        set_call_time(&mut s, d, Some(7 * 60)).unwrap();
        s.plan
            .set_location(GeoLocation {
                name: "pier".into(),
                latitude: 34_052_200,
                longitude: -118_243_700,
                utc_offset: -420,
                daylight_saving: None,
            })
            .unwrap();
        call_sheet(&s, b, sb, 1).unwrap()
    }

    #[test]
    fn json_and_csv() {
        let c = one_day();
        let j: Value = serde_json::from_str(&to_json(&c)).unwrap();
        assert_eq!(j["format"], "method-schedule-callsheet");
        assert_eq!(j["date"], "2024-06-20");
        assert_eq!(j["call"], "07:00");
        assert_eq!(j["wrap_minutes"], 420 + 120);
        assert_eq!(j["scenes"][0]["start"], "07:00");
        assert_eq!(j["scenes"][0]["end"], "08:30");
        assert_eq!(j["scenes"][0]["cast"][0], "1");
        assert_eq!(j["cast"][0]["board_id"], "1");
        assert_eq!(j["sun"]["state"], "times");
        assert!(j["sun"]["sunrise"].as_str().unwrap().starts_with("05:4"));
        assert_eq!(j["notes"][0], "08:30 Lunch (0:30)");
        let csv = String::from_utf8(to_csv(&c)).unwrap();
        assert!(csv.starts_with("\u{feff}Field,Value\r\nTitle,ZTEST Call\r\n"));
        assert!(csv.contains(
            "\r\n\r\nStart,End,Scene,I/E,Set,Day/Night,Pages,Cast,Location,Synopsis\r\n"
        ));
        assert!(
            csv.contains("07:00,08:30,12,EXT,PIER,DAY,2/8,1,Pier,\"'=She waits, then runs\"\r\n")
        );
        assert!(csv.contains(
            "\r\n\r\nID,Name,Scenes,First,Last,Call,Wrap\r\n1,ANN,12,07:00,08:30,07:00,09:00\r\n"
        ));
        assert_eq!(j["cast"][0]["call"], "07:00");
        assert_eq!(j["cast"][0]["wrap_minutes"], 540);
        assert_eq!(j["cast"][0]["own_call"], false);
        assert!(csv.contains("\r\n\r\nNote\r\n08:30 Lunch (0:30)\r\n"));
    }

    /// The JSON has exactly the keys CALLSHEET.md's table lists, and inside `sun`,
    /// `scenes`, `cast` and `elements` the keys its notes name, in alphabetical order.
    #[test]
    fn json_keys_are_the_ones_callsheet_md_lists() {
        let doc = include_str!("../../../docs/CALLSHEET.md");
        let table = doc
            .split("| Key | Type | Notes |")
            .nth(1)
            .and_then(|t| t.split("\n\n").next())
            .unwrap();
        let ticks = |cell: &str| -> Vec<String> {
            cell.split('`')
                .skip(1)
                .step_by(2)
                .filter(|w| w.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .map(str::to_string)
                .collect()
        };
        let states = ["times", "always_up", "always_down", "unknown"];
        let mut top = Vec::new();
        let mut inner = std::collections::BTreeMap::new();
        for row in table.lines().skip(2) {
            let cells: Vec<&str> = row.split('|').collect();
            let keys = ticks(cells[1]);
            if let [one] = keys.as_slice() {
                if ["sun", "scenes", "cast", "elements"].contains(&one.as_str()) {
                    // The object's own keys: for elements, those before `(each`.
                    let notes = cells[3].split("(each").next().unwrap();
                    let mut k = ticks(notes);
                    k.retain(|x| !states.contains(&x.as_str()));
                    k.sort();
                    inner.insert(one.clone(), k);
                }
            }
            top.extend(keys);
        }
        top.sort();
        let text = to_json(&one_day());
        let j: Value = serde_json::from_str(&text).unwrap();
        let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
        assert_eq!(keys(&j), top, "top level");
        for part in ["sun", "scenes", "cast", "elements"] {
            let v = if part == "sun" { &j[part] } else { &j[part][0] };
            assert_eq!(keys(v), inner[part], "{part}");
        }
        assert_eq!(
            keys(&j["elements"][0]["elements"][0]),
            ["board_id", "element", "name", "scenes"]
        );
        // Written in alphabetical order, as the page says.
        let mut at = 0;
        for k in keys(&j) {
            let here = text.find(&format!("\n  \"{k}\":")).unwrap();
            assert!(here >= at, "{k} is out of order");
            at = here;
        }
    }
}
