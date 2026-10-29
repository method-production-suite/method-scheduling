//! The strict reader across the whole file (FORMAT.md, "Defaults"): a schedule with every
//! key set is written, then every key in it is tried as `null`, at the empty value of its
//! type (`false`, `[]`, `{}`, `""`, `0`) and left out. A key written as `null` is always
//! refused, and a file that reads always writes back byte for byte, so no key is taken at
//! its default.

use serde_json::Value;

use method_schedule_core::headers::{HeaderAlign, HeaderDesign, HeaderId, HeaderItem};
use method_schedule_core::layouts::{LayoutBody, LayoutKind};
use method_schedule_core::model::{
    Align, Banner, BannerColor, BannerStyle, CalendarShow, DayLook, Event, EventKind, Minutes, Rgb,
    Seconds, StripColorRule, StripColors, StripColumn, StripField, StripKind, StripLayout,
    StripWidth, MAX_EVENT_DAYS,
};
use method_schedule_core::plan::{set_call_time, set_turnaround, GeoLocation};
use method_schedule_core::setup::{PageBreak, ReportColumn, TextSize};
use method_schedule_core::{Schedule, Weekday};

use crate::{from_str, layouts_from_str, layouts_to_string, to_string, FormatError};

/// The format tests' sample with every optional key of the file given a value.
pub(crate) fn everything() -> Schedule {
    let mut s = crate::tests::sample();
    let b = s.boards[0].id;
    let cast = s.cast().unwrap();
    // Production info, every field.
    for (i, f) in method_schedule_core::setup::Production::FIELDS
        .iter()
        .enumerate()
    {
        s.production.set(f, &format!("Value {i}")).unwrap();
    }
    // Categories: every switch, rules, properties, and the cast under another name.
    s.rename_category(cast, "Players").unwrap();
    let crowd = s.add_category("Background");
    {
        let c = s.category_mut(crowd).unwrap();
        c.daily_quantity = true;
        c.auto_board_id = true;
        c.exclude_from_board = true;
        c.defaults.holds = true;
        c.defaults.drops = true;
        c.defaults.min_drop_gap = 3;
    }
    s.add_property(cast, "Agent").unwrap();
    s.add_property(cast, "Height").unwrap();
    s.set_property_multi_line(cast, 1, true).unwrap();
    // Elements: every field.
    let tove = s.element_named(cast, "TOVE").unwrap();
    s.set_property_value(tove, "height", "1.85 m").unwrap();
    {
        let e = s.element_mut(tove).unwrap();
        e.images = vec!["images/tove.jpg".into()];
        e.notes = "Allergic to cats".into();
        e.board_id_locked = true;
    }
    let walkers = s.add_element(crowd, "Dock workers").unwrap();
    s.element_mut(walkers).unwrap().exclude_from_board = false;
    s.set_links(tove, &[walkers], false).unwrap();
    // Sheets: every field.
    let sheet = s.sheets[0].id;
    s.tag_with_links(sheet, tove).unwrap();
    s.set_quantity(sheet, walkers, 12, false).unwrap();
    {
        let h = &mut s.sheets[0];
        h.number = "12A".into();
        h.sunset = Some(19 * 60 + 2);
        h.screen_time = Some(Seconds(95));
        h.location = "Pier 9".into();
        h.script_pages = "4-5".into();
        h.script_day = "D2".into();
        h.unit = "Second".into();
        h.sequence = "Storm".into();
        h.episode = "104".into();
        h.comments = "Rain towers".into();
        h.images = vec!["images/kitchen.jpg".into()];
    }
    s.set_sheet_field_order(&["scenes".into(), "number".into()])
        .unwrap();
    s.sheet_looks = method_schedule_core::model::SheetLooks::read(
        &["comments".to_string()],
        &[("synopsis".to_string(), "wide".to_string())],
    )
    .and_then(|l| l.with_rows(&[4, 4]))
    .unwrap();
    s.set_calendar_fields(vec![StripField::Scene, StripField::Pages])
        .unwrap();
    s.set_calendar_hidden(&[CalendarShow::Pages]);
    // Red flag types: a color on one of ours and the built-in ones.
    s.red_flag_types[0].color = Some(Rgb(0x3366cc));
    let mut built = Schedule::with_default_categories("ZTEST types");
    let travel = built.red_flag_types.remove(0);
    let mut travel = travel;
    travel.id = method_schedule_core::model::RedFlagTypeId(s.next_id);
    s.next_id += 1;
    travel.name = "Travel (built in)".into();
    s.red_flag_types.push(travel);
    // Strip colors, every field.
    s.set_strip_colors(StripColors {
        rules: vec![StripColorRule {
            int_ext: "EXT".into(),
            time_of_day: "NIGHT".into(),
            color: Rgb(0x1f3524),
            text: Some(Rgb(0xffeeaa)),
        }],
        scene_text: Some(Rgb(0x101010)),
        banner_text: Some(Rgb(0x6a1b9a)),
        break_text: Some(Rgb(0x202020)),
    })
    .unwrap();
    // The board: a strip layout of every column key, a size, a day look and named layouts.
    let layout = StripLayout {
        columns: vec![
            StripColumn {
                bold: true,
                align: Align::Center,
                width: StripWidth::Wide,
                ..StripColumn::of(StripField::Scene)
            },
            StripColumn {
                rule_after: true,
                ..StripColumn::of(StripField::Set)
            },
            StripColumn {
                italic: true,
                rule_under: true,
                ..StripColumn::on_line(StripField::Synopsis, 1, 2)
            },
            StripColumn {
                category: Some(crowd),
                ..StripColumn::of(StripField::ElementCount)
            },
            StripColumn {
                text: "Call 07:00".into(),
                align: Align::Right,
                ..StripColumn::on_line(StripField::Text, 3, 1)
            },
        ],
        size: TextSize::Large,
        hide_red_flags: true,
        hide_conflicts: true,
    };
    // On the board's own layout only, a category's elements by board ID (decision 194).
    let mut own = layout.clone();
    own.columns.push(StripColumn {
        board_ids: true,
        ..StripColumn::of(StripField::CategoryElements)
    });
    s.set_strip_layout(b, own).unwrap();
    s.set_day_look(
        b,
        DayLook {
            shoot_day_strip: true,
            screen_time: true,
            sunrise: true,
            sunset: true,
            quantity: true,
            text: "Lunch at 6".into(),
        },
    )
    .unwrap();
    let lines = s.add_layout("Lines", LayoutBody::Strips(layout)).unwrap();
    let night = s
        .add_layout(
            "Night",
            LayoutBody::DayBreaks(DayLook {
                sunset: true,
                ..DayLook::default()
            }),
        )
        .unwrap();
    let shoot = s
        .add_layout(
            "Shoot",
            LayoutBody::ShootDay(DayLook {
                text: "Shoot".into(),
                ..DayLook::default()
            }),
        )
        .unwrap();
    s.use_layout(b, LayoutKind::Strips, Some(lines)).unwrap();
    s.use_layout(b, LayoutKind::DayBreaks, Some(night)).unwrap();
    s.use_layout(b, LayoutKind::ShootDay, Some(shoot)).unwrap();
    s.use_one_line_layout(Some(lines)).unwrap();
    // A banner with every look.
    let banner = s.boards[0].sub_boards[0]
        .strips
        .iter()
        .find(|x| matches!(x.kind, StripKind::Banner(_)))
        .unwrap()
        .id;
    s.set_banner(
        b,
        banner,
        Banner {
            text: "Company move".into(),
            duration: Minutes(45),
            style: BannerStyle {
                bold: true,
                color: Some(BannerColor::from_text("blue").unwrap()),
                tall: true,
                size: TextSize::Small,
                align: Align::Center,
                font: method_schedule_core::model::BannerFont::Serif,
            },
        },
    )
    .unwrap();
    // The calendar: every list, a labelled and colored event, prep and wrap.
    {
        let cal = &mut s.boards[0].sub_boards[0].calendar;
        cal.weekly_off = vec![Weekday::Sun];
        cal.off_days.push("2027-03-10".parse().unwrap());
        cal.moves.push("2027-03-11".parse().unwrap());
        cal.events.push(Event {
            name: "Read-through".into(),
            kind: EventKind::Other("Table read".into()),
            from: "2027-02-20".parse().unwrap(),
            to: "2027-02-20".parse().unwrap(),
            elements: vec![tove, walkers],
            color: Some(4),
        });
        cal.prep_start = "2027-01-04".parse().ok();
        cal.wrap_end = "2027-04-30".parse().ok();
    }
    // Reports: every option key across the list.
    let mut r = s.report_list()[0].clone();
    r.name = "Cast one-line".into();
    r.options.from = "2027-03-02".parse().ok();
    r.options.to = "2027-03-30".parse().ok();
    r.options.landscape = Some(true);
    r.options.synopsis = false;
    r.options.cast_names = true;
    r.options.page_break = PageBreak::Week;
    r.options.shoot_days = false;
    r.options.day_breaks = false;
    r.options.total_count = true;
    r.options.element_columns = vec![cast];
    r.options.layout = Some(lines);
    r.options.banners = false;
    r.options.categories = vec![cast];
    r.options.elements = vec![tove];
    r.options.colors = true;
    r.options.icons = false;
    r.options.text = TextSize::Small;
    r.options.columns = vec![
        ReportColumn::of(StripField::Scene),
        ReportColumn::of(StripField::Set),
        ReportColumn {
            line: 1,
            span: 2,
            bold: true,
            italic: true,
            align: Align::Center,
            size: Some(TextSize::Large),
            ..ReportColumn::of(StripField::Synopsis)
        },
    ];
    let mut d = HeaderDesign::of(
        HeaderId(0),
        "Top",
        vec![vec![
            HeaderItem::Title,
            HeaderItem::Space,
            HeaderItem::Text("Draft".into()),
        ]],
    );
    d.include = false;
    d.first_page_only = false;
    d.bold = true;
    d.italic = true;
    d.align = HeaderAlign::Right;
    d.size = TextSize::Large;
    r.options.header = s.add_header(d).unwrap();
    s.set_report(0, r).unwrap();
    let mut day = s.report_list()[1].clone();
    day.options.page_break = PageBreak::Day;
    day.options.no_holds = true;
    day.options.weekdays_only = true;
    s.set_report(1, day).unwrap();
    // Day out of days codes, every letter.
    let c = &mut s.dood_codes;
    for (i, code) in [
        &mut c.start,
        &mut c.work,
        &mut c.finish,
        &mut c.hold,
        &mut c.drop,
        &mut c.pickup,
        &mut c.travel,
        &mut c.fitting,
        &mut c.rehearsal,
        &mut c.holiday,
        &mut c.company_move,
    ]
    .into_iter()
    .enumerate()
    {
        *code = format!("Z{i}");
    }
    // The plan, every key.
    let brk = s.boards[0].sub_boards[0]
        .strips
        .iter()
        .find(|x| x.kind == StripKind::DayBreak)
        .unwrap()
        .id;
    set_call_time(&mut s, brk, Some(390)).unwrap();
    set_turnaround(&mut s, cast, Some(720)).unwrap();
    s.plan.long_day = Some(600);
    s.plan
        .set_location(GeoLocation {
            name: "Pier 9".into(),
            latitude: 34_052_200,
            longitude: -118_243_700,
            utc_offset: -480,
            daylight_saving: Some(method_schedule_core::solar::DaylightSaving::UsCanada),
        })
        .unwrap();
    method_schedule_core::drift::lock(&mut s, b, "White", "2027-02-01".parse().ok()).unwrap();
    let last = s.sheets.last().unwrap().id;
    s.plan.omitted.push(last);
    let first = s.boards[0].sub_boards[0].strips[0].id;
    s.plan.pinned.push(first);
    // What was shot (decision 224): every key, a scene finished on the day.
    s.plan.days_shot.push(method_schedule_core::plan::DayShot {
        day: brk,
        date: "2027-03-02".parse().ok(),
        call: Some(420),
        wrap: Some(1500),
    });
    s.plan
        .scenes_shot
        .push(method_schedule_core::plan::SceneShot {
            sheet: s.sheets[0].id,
            day: brk,
            eighths: method_schedule_core::Eighths(4),
            complete: true,
        });
    // A call and wrap of one's own (decision 231).
    s.plan
        .person_calls
        .push(method_schedule_core::plan::PersonCall {
            day: brk,
            element: walkers,
            call: Some(300),
            wrap: Some(1560),
        });
    // A merged script draft (decision 233).
    s.plan.drafts.push(method_schedule_core::plan::Draft {
        color: "Blue".into(),
        date: "2027-02-10".parse().ok(),
        changed: vec![s.sheets[0].id],
        added: vec![last],
        omitted: Vec::new(),
    });
    // A merge conflict kept for review (decision 238).
    s.plan
        .conflicts
        .push(method_schedule_core::plan::ReviewConflict {
            key: format!("sheet:{}:set", s.sheets[0].id.0),
            what: "Scene 1 set".into(),
            base: Some("\"SHED\"".into()),
            ours: "\"LOFT\"".into(),
            theirs: "\"BARN\"".into(),
            taken: method_schedule_core::merge::Side::Theirs,
            date: "2027-02-11".parse().ok(),
        });
    // Additions from another app on every object that keeps them (decision 224).
    let x = &mut s.extensions;
    x.file = Some(r#"{"org.example.sync":{"rev":7}}"#.to_string());
    x.schedule = Some(r#"{"org.example.sync":{"shared":true}}"#.to_string());
    let ids = [
        s.categories[0].id.0,
        s.red_flag_types[0].id.0,
        s.elements[0].id.0,
        s.sheets[0].id.0,
        s.boards[0].id.0,
        s.boards[0].sub_boards[0].id.0,
    ];
    for (i, id) in ids.into_iter().enumerate() {
        s.extensions.objects.insert(
            id,
            format!(r#"{{"org.example.sync":{{"n":{i},"tags":["a","b"]}}}}"#),
        );
    }
    s.check().unwrap();
    s
}

/// Every key name in a JSON value, at any depth.
fn keys(v: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                out.insert(k.clone());
                keys(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
        _ => {}
    }
}

/// The path (keys and indexes) to every key of every object in `v`.
fn paths(v: &Value, at: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                at.push(Step::Key(k.clone()));
                out.push(at.clone());
                // What's inside `extensions` is another app's, kept untouched
                // (decision 224): only the key itself is ours to check.
                if k != "extensions" {
                    paths(x, at, out);
                }
                at.pop();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                at.push(Step::Index(i));
                paths(x, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

#[derive(Clone, Debug)]
enum Step {
    Key(String),
    Index(usize),
}

/// The object holding the last key of `path`, and that key.
fn parent<'a>(v: &'a mut Value, path: &[Step]) -> (&'a mut serde_json::Map<String, Value>, String) {
    let (last, rest) = path.split_last().unwrap();
    let mut at = v;
    for s in rest {
        at = match s {
            Step::Key(k) => &mut at[k.as_str()],
            Step::Index(i) => &mut at[*i],
        };
    }
    let Step::Key(k) = last else { unreachable!() };
    (at.as_object_mut().unwrap(), k.clone())
}

/// The empty value of a JSON value's type: what a key at its default would most often be
/// written as.
fn empty_of(v: &Value) -> Value {
    match v {
        Value::Bool(_) => Value::Bool(false),
        Value::Number(_) => Value::from(0),
        Value::String(_) => Value::String(String::new()),
        Value::Array(_) => Value::Array(Vec::new()),
        Value::Object(_) => Value::Object(Default::default()),
        Value::Null => Value::Null,
    }
}

/// What the writer writes for `s`, as JSON (key order aside, the same bytes).
fn json(s: &Schedule) -> Value {
    serde_json::from_str(&to_string(s)).unwrap()
}

fn pretty(v: &Value) -> String {
    let mut t = serde_json::to_string_pretty(v).unwrap();
    t.push('\n');
    t
}

#[test]
fn every_key_is_refused_as_null_and_never_read_at_its_default() {
    let s = everything();
    let text = to_string(&s);
    assert_eq!(from_str(&text).unwrap(), s);
    assert_eq!(to_string(&from_str(&text).unwrap()), text);
    let file: Value = serde_json::from_str(&text).unwrap();

    // Everything set: every key FORMAT.md documents is in the file (by name).
    let mut seen = Default::default();
    keys(&file, &mut seen);
    let doc = include_str!("../../../docs/FORMAT.md");
    let not_keys = ["template", "sheet", "banner", "day_break"];
    let mut missing = Vec::new();
    for line in doc.lines().filter(|l| l.starts_with("| `")) {
        let name = line.split('`').nth(1).unwrap();
        if !not_keys.contains(&name) && !seen.contains(name) {
            missing.push(name.to_string());
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "keys not set: {missing:?}");

    let mut all = Vec::new();
    paths(&file, &mut Vec::new(), &mut all);
    let (mut optional, mut defaults_refused) = (0, 0);
    for path in &all {
        let name = format!("{path:?}");
        // null: refused, for every key.
        let mut v = file.clone();
        let (obj, k) = parent(&mut v, path);
        obj.insert(k.clone(), Value::Null);
        assert!(from_str(&pretty(&v)).is_err(), "null taken at {name}");
        // The empty value of its type: refused, or read and written back as given.
        let mut v = file.clone();
        let (obj, k) = parent(&mut v, path);
        let empty = empty_of(&obj[&k]);
        if obj[&k] == empty {
            continue;
        }
        obj.insert(k.clone(), empty);
        let given = pretty(&v);
        match from_str(&given) {
            Err(_) => defaults_refused += 1,
            Ok(read) => assert_eq!(json(&read), v, "{name} does not write back"),
        }
        // Left out: an optional key reads, and writes back without it.
        let mut v = file.clone();
        let (obj, k) = parent(&mut v, path);
        obj.remove(&k);
        let given = pretty(&v);
        if let Ok(read) = from_str(&given) {
            optional += 1;
            assert_eq!(json(&read), v, "{name} left out does not write back");
        }
    }
    assert!(optional > 150, "{optional} optional keys tried");
    assert!(
        defaults_refused > 150,
        "{defaults_refused} empty values refused"
    );
}

#[test]
fn null_and_defaults_are_named_in_the_message() {
    let text = to_string(&everything());
    for (from, to, says) in [
        (
            "\"color\": \"teal\"",
            "\"color\": null",
            "is null; leave the key out",
        ),
        (
            "\"prep_start\": \"2027-01-04\"",
            "\"prep_start\": null",
            "prep_start is null",
        ),
        (
            "\"builtin\": \"travel\"",
            "\"builtin\": null",
            "builtin is null",
        ),
        (
            "\"board_id_locked\": true",
            "\"board_id_locked\": false",
            "is the default",
        ),
        (
            "\"notes\": \"Allergic to cats\"",
            "\"notes\": \"\"",
            "is the default",
        ),
    ] {
        assert!(text.contains(from), "{from}");
        let e = from_str(&text.replacen(from, to, 1))
            .unwrap_err()
            .to_string();
        assert!(e.contains(says), "{to}: {e}");
    }
    // An element's `images: []` names its place.
    let bad = text.replacen(
        "\"images\": [\n          \"images/tove.jpg\"\n        ]",
        "\"images\": []",
        1,
    );
    let e = from_str(&bad).unwrap_err().to_string();
    assert!(e.contains("schedule.elements["), "{e}");
    assert!(e.contains("].images [] is the default"), "{e}");
}

#[test]
fn an_event_longer_than_any_edit_makes_is_refused() {
    let mut s = everything();
    let cal = &mut s.boards[0].sub_boards[0].calendar;
    let from: method_schedule_core::Date = "2027-01-01".parse().unwrap();
    cal.events[0].from = from;
    cal.events[0].to = method_schedule_core::Date(from.0 + MAX_EVENT_DAYS - 1);
    let text = to_string(&s);
    from_str(&text).unwrap();
    let cal = &mut s.boards[0].sub_boards[0].calendar;
    cal.events[0].to = method_schedule_core::Date(from.0 + MAX_EVENT_DAYS);
    let e = from_str(&to_string(&s)).unwrap_err();
    assert!(matches!(e, FormatError::BadValue(_)), "{e}");
    assert!(e.to_string().contains("longer than"), "{e}");
}

#[test]
fn a_layout_file_is_as_strict() {
    let s = everything();
    let file = layouts_to_string(&s, &s.layouts.layouts);
    assert_eq!(layouts_from_str(&s, &file).unwrap().len(), 3);
    // An element count's category goes by name (decision 147); in the schedule, by id.
    let crowd = s.category_named("Background").unwrap();
    assert!(file.contains("\"category_name\": \"Background\""), "{file}");
    assert!(!file.contains("\"category\":"), "{file}");
    assert!(to_string(&s).contains(&format!("\"category\": {}", crowd.0)));
    let counted = |x: &LayoutBody| match x {
        LayoutBody::Strips(l) => l.counted().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    assert_eq!(counted(&layouts_from_str(&s, &file).unwrap()[0].1), [crowd]);
    // Into another schedule: its own category of that name, ignoring case; none there,
    // the count counts every element, and the file still reads as written.
    let mut other = Schedule::new("ZTEST other");
    assert!(counted(&layouts_from_str(&other, &file).unwrap()[0].1).is_empty());
    let theirs = other.add_category("BACKGROUND");
    let read = layouts_from_str(&other, &file).unwrap();
    assert_eq!(counted(&read[0].1), [theirs]);
    other.import_layouts(read).unwrap();
    other.check().unwrap();
    // Refused: an id in a layout file, a name in a schedule, a name on another field, a
    // blank name.
    let id_in_file = file.replacen("\"category_name\": \"Background\"", "\"category\": 3", 1);
    assert!(layouts_from_str(&s, &id_in_file).is_err());
    let text = to_string(&s);
    let name_in_schedule = text.replacen(
        &format!("\"category\": {}", crowd.0),
        "\"category_name\": \"Background\"",
        1,
    );
    assert!(from_str(&name_in_schedule).is_err());
    let missing = text.replacen(
        &format!("\"category\": {}", crowd.0),
        "\"category\": 999999",
        1,
    );
    assert!(from_str(&missing).is_err(), "a category the schedule lacks");
    let blank = file.replacen(
        "\"category_name\": \"Background\"",
        "\"category_name\": \" \"",
        1,
    );
    assert!(layouts_from_str(&s, &blank).is_err());
    let on_set = file.replacen(
        "\"field\": \"set\"",
        "\"field\": \"set\",\n          \"category_name\": \"Background\"",
        1,
    );
    assert_ne!(on_set, file);
    assert!(layouts_from_str(&s, &on_set).is_err());
    for (from, to) in [
        ("\"italic\": true", "\"italic\": false"),
        ("\"text\": \"Shoot\"", "\"text\": \"\""),
        ("\"italic\": true", "\"italic\": null"),
        ("\"bold\": true", "\"bold\": true, \"span\": null"),
    ] {
        assert!(file.contains(from), "{from}");
        assert!(
            layouts_from_str(&s, &file.replacen(from, to, 1)).is_err(),
            "{to}"
        );
    }
}

/// Decision 194: a category's elements keeps its category (by id in a schedule, by name
/// in a layout file) and its board ID switch, left out while off; the switch on another
/// field, or written false, is refused.
#[test]
fn a_category_elements_column_round_trips() {
    let mut s = Schedule::new("ZTEST lists");
    let props = s.add_category("Props");
    let b = s.boards[0].id;
    let mut l = StripLayout::default();
    l.columns.push(StripColumn {
        category: Some(props),
        board_ids: true,
        ..StripColumn::of(StripField::CategoryElements)
    });
    s.set_strip_layout(b, l.clone()).unwrap();
    s.add_layout("Lists", LayoutBody::Strips(l)).unwrap();
    s.check().unwrap();
    let text = to_string(&s);
    assert!(text.contains("\"field\": \"category_elements\""), "{text}");
    assert!(text.contains(&format!("\"category\": {}", props.0)));
    assert!(text.contains("\"board_ids\": true"));
    let back = from_str(&text).unwrap();
    assert_eq!(back, s);
    assert_eq!(to_string(&back), text);
    assert!(from_str(&text.replace("\"board_ids\": true", "\"board_ids\": false")).is_err());
    let on_scene = text.replacen(
        "\"field\": \"scene\"",
        "\"field\": \"scene\",\n          \"board_ids\": true",
        1,
    );
    assert_ne!(on_scene, text);
    assert!(from_str(&on_scene).is_err());
    // A layout file names the category; another schedule finds its own of that name.
    let file = layouts_to_string(&s, &s.layouts.layouts);
    assert!(file.contains("\"category_name\": \"Props\""), "{file}");
    let mut other = Schedule::new("ZTEST other");
    let theirs = other.add_category("props");
    let read = layouts_from_str(&other, &file).unwrap();
    let LayoutBody::Strips(x) = &read[0].1 else {
        panic!()
    };
    let last = x.columns.last().unwrap();
    assert_eq!((last.category, last.board_ids), (Some(theirs), true));
}

/// Additions from other apps (decision 224): kept on every object that has them and
/// written back byte for byte; an empty one, one that isn't an object, and an unknown key
/// beside them are refused.
#[test]
fn extensions_are_kept_and_written_back_untouched() {
    let s = everything();
    let text = to_string(&s);
    let back = from_str(&text).unwrap();
    assert_eq!(back.extensions, s.extensions);
    assert_eq!(to_string(&back), text, "byte for byte");
    assert_eq!(back.extensions.objects.len(), 6);
    // A file without any reads and writes as before: nothing is added.
    let plain = crate::tests::sample();
    assert!(!to_string(&plain).contains("extensions"));
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["schedule"]["elements"][0]["extensions"] = serde_json::json!({});
    assert!(from_str(&v.to_string()).is_err(), "empty");
    v["schedule"]["elements"][0]["extensions"] = serde_json::json!([1]);
    assert!(from_str(&v.to_string()).is_err(), "not an object");
    v["schedule"]["elements"][0]["extensions"] = serde_json::json!({"x": 1});
    v["schedule"]["elements"][0]["extension"] = serde_json::json!({"x": 1});
    assert!(from_str(&v.to_string()).is_err(), "any other unknown key");
}
