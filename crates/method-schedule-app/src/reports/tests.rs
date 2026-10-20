use super::*;
use crate::board;
use crate::session::{read_script, Session};
use method_schedule_core::headers::HeaderId;
use method_schedule_core::model::{StripColumn, StripField, StripLayout};
use method_schedule_core::reports;
use method_schedule_core::setup::{
    PageBreak, ReportColumn, ReportKind, ReportOptions, ReportSetup, TextSize,
};
use std::path::Path;

/// Harbor Light on the first sub-board, three eighths a day, starting 2027-03-01, with
/// board IDs for TOVE and KIT.
fn harbor() -> Session {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scripts/harbor-light.fountain");
    let mut s = Session::new();
    s.import_script(&read_script(&path).unwrap()).unwrap();
    s.edit(|x| {
        let b = x.boards[0].id;
        let sb = x.boards[0].sub_boards[0].id;
        let ids: Vec<_> = x.boards[0].boneyard.iter().map(|st| st.id).collect();
        x.move_strips(
            b,
            &ids,
            method_schedule_core::model::Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .map_err(|e| e.to_string())?;
        board::auto_day_breaks(x, sb.0, "3/8")?;
        board::set_start_date(x, sb.0, Some("2027-03-01"))?;
        let cast = x.category_named("Cast").unwrap();
        let tove = x.element_named(cast, "TOVE").unwrap();
        let kit = x.element_named(cast, "KIT").unwrap();
        x.set_board_id(tove, Some("1")).map_err(|e| e.to_string())?;
        x.set_board_id(kit, Some("2")).map_err(|e| e.to_string())?;
        Ok(())
    })
    .unwrap();
    s
}

fn sub(s: &Session) -> u32 {
    crate::view::first_sub(s.schedule())
}

/// The report of `kind` with the default options.
fn of(s: &Session, kind: ReportKind) -> ReportDto {
    let at = s
        .schedule()
        .report_list()
        .iter()
        .position(|r| r.kind == kind)
        .unwrap();
    build_report(s.schedule(), sub(s), at, "sun").unwrap()
}

/// The report of `kind` with options changed by `f`.
fn with(s: &Session, kind: ReportKind, f: impl FnOnce(&mut ReportOptions)) -> ReportDto {
    let mut setup = ReportSetup::of(kind);
    f(&mut setup.options);
    build(s.schedule(), sub(s), 0, &setup, "sun").unwrap()
}

/// Decisions 99 and 100: the header's items filled in, pages per week, the shoot day
/// lines and day breaks, the total line, category columns and a named layout.
#[test]
fn headers_page_breaks_totals_and_category_columns() {
    let mut s = harbor();
    s.edit(|x| {
        x.production.director = "A. Director".into();
        x.production.custom_1 = "Second unit".into();
        Ok(())
    })
    .unwrap();
    let r = with(&s, ReportKind::OneLine, |_| {});
    // The Full layout: title; company, director, producer; report name and the facts.
    assert_eq!(r.head.rows.len(), 3);
    let row1: Vec<&str> = r.head.rows[1].iter().map(|i| i.text.as_str()).collect();
    assert_eq!(row1, ["", "Director A. Director", ""]);
    assert_eq!(r.head.rows[2][0].text, "One-line schedule");
    assert!(!r.head.every_page);
    assert!(r.total.is_none());
    assert!(r.days.iter().all(|d| !d.new_page));
    assert!(r.shoot_days && r.day_breaks);
    let cast = s.schedule().cast().unwrap();
    let w = with(&s, ReportKind::OneLine, |o| {
        o.page_break = PageBreak::Day;
        o.total_count = true;
        o.shoot_days = false;
        o.element_columns = vec![cast];
    });
    assert!(!w.shoot_days);
    assert!(!w.days[0].new_page);
    assert!(w.days.iter().skip(1).all(|d| d.new_page));
    let total = w.total.unwrap();
    assert!(
        total.starts_with("Total: 5 scenes on 3 shoot days"),
        "{total}"
    );
    let last = w.layout.last().unwrap();
    assert_eq!((last.field, last.label.as_str()), ("elements", "Cast"));
    assert_eq!(w.days[0].lines[0].cells.last().unwrap(), "1, 2");
    // Days of one week stay on one page.
    let wk = with(&s, ReportKind::OneLine, |o| o.page_break = PageBreak::Week);
    assert!(wk.days.iter().all(|d| !d.new_page));
    assert_eq!(
        week_of("2027-03-06".parse().unwrap(), false),
        week_of("2027-02-28".parse().unwrap(), false)
    );
    assert_ne!(
        week_of("2027-03-07".parse().unwrap(), false),
        week_of("2027-03-06".parse().unwrap(), false)
    );
    assert_ne!(
        week_of("2027-03-08".parse().unwrap(), true),
        week_of("2027-03-07".parse().unwrap(), true)
    );
    // A named strip layout picked by a report other than the one-line.
    let lay = s
        .edit(|x| {
            x.add_layout(
                "Two fields",
                method_schedule_core::layouts::LayoutBody::Strips(StripLayout::of(vec![
                    StripColumn::of(StripField::Set),
                    StripColumn::of(StripField::Pages),
                ])),
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
        .map(|_| s.schedule().layouts.layouts[0].id)
        .unwrap();
    let b = with(&s, ReportKind::BreakdownDay, |o| o.layout = Some(lay));
    let fields: Vec<&str> = b.layout.iter().map(|c| c.field).collect();
    assert_eq!(fields, ["set", "pages"]);
}

#[test]
fn every_report_has_data_and_a_title() {
    let s = harbor();
    for (i, r) in s.schedule().report_list().iter().enumerate() {
        let d = build_report(s.schedule(), sub(&s), i, "sun").unwrap();
        assert_eq!(d.report, r.kind.name());
        assert_eq!(d.name, r.name);
        assert_eq!(d.title, "Harbor Light");
        assert_eq!(d.sub_board, "Main");
        assert_eq!(d.start.as_deref(), Some("2027-03-01"));
        let quiet = matches!(r.kind, ReportKind::Conflicts | ReportKind::RedFlags);
        assert!(quiet || d.empty.is_none(), "{}: {:?}", r.name, d.empty);
        assert!(!d.name.contains('!') && !d.name.contains('\u{2014}'));
    }
    assert!(build_report(s.schedule(), sub(&s), 99, "sun").is_err());
}

#[test]
fn one_line_and_shooting_schedule_follow_the_engine() {
    let s = harbor();
    let one = of(&s, ReportKind::OneLine);
    let engine = reports::one_line(
        s.schedule(),
        s.schedule().boards[0].id,
        s.schedule().boards[0].sub_boards[0].id,
        s.schedule().category_named("Cast"),
    )
    .unwrap();
    assert_eq!(one.days.len(), engine.len());
    let d1 = &one.days[0];
    assert_eq!(d1.number, 1);
    assert_eq!(d1.date.as_deref(), Some("2027-03-01"));
    assert_eq!(d1.weekday, Some("Mon"));
    assert_eq!(d1.lines[0].kind, "scene");
    assert_eq!(d1.lines[0].set, "NET LOFT");
    assert_eq!(d1.lines[0].cast, ["1", "2"]);
    assert_eq!(d1.lines[0].tone, "int-night");
    assert_eq!(d1.scenes, d1.lines.len());
    assert!(!d1.pages.is_empty() && d1.time.contains(':'));
    // The board's layout: scene, I/E, set, time of day, pages, cast.
    assert_eq!(one.layout.len(), 6);
    assert_eq!(d1.lines[0].cells[5], "1, 2");

    let shoot = of(&s, ReportKind::Shooting);
    assert_eq!(shoot.days.len(), one.days.len());
    let first = &shoot.days[0].lines[0];
    assert_eq!(first.groups[0].category, "Cast");
    assert_eq!(first.groups[0].elements, ["1 TOVE", "2 KIT"]);
}

#[test]
fn options_reach_the_lines() {
    let s = harbor();
    // A date range: day 2 only.
    let r = with(&s, ReportKind::OneLine, |o| {
        o.from = "2027-03-02".parse().ok();
        o.to = "2027-03-02".parse().ok();
    });
    assert_eq!(r.days.len(), 1);
    assert_eq!(r.days[0].number, 2);
    assert_eq!(r.range.as_deref(), Some("2027-03-02 to 2027-03-02"));
    let none = with(&s, ReportKind::OneLine, |o| {
        o.from = "2030-01-01".parse().ok()
    });
    assert!(none.empty.unwrap().contains("date range"));
    // Synopsis off, cast by name, chosen columns.
    let r = with(&s, ReportKind::OneLine, |o| {
        o.synopsis = false;
        o.cast_names = true;
        o.columns = vec![
            ReportColumn::of(StripField::Scene),
            ReportColumn::of(StripField::Cast),
        ];
        o.header = HeaderId::TITLE;
    });
    assert_eq!(r.layout.len(), 2);
    assert_eq!(r.days[0].lines[0].cells, ["1", "TOVE, KIT"]);
    // A cast member left off the board shows by neither name nor ID (decision 128).
    let mut off = harbor();
    off.edit(|x| {
        let cast = x.category_named("Cast").unwrap();
        let tove = x.element_named(cast, "TOVE").unwrap();
        x.element_mut(tove).unwrap().exclude_from_board = true;
        Ok(())
    })
    .unwrap();
    for names in [true, false] {
        let r = with(&off, ReportKind::OneLine, |o| {
            o.cast_names = names;
            o.columns = vec![ReportColumn::of(StripField::Cast)];
        });
        assert_eq!(r.days[0].lines[0].cells, [if names { "KIT" } else { "2" }]);
    }
    // The report designer (decision 90): a merged cell on a later line, bold, with its
    // own text size; the page gets its line, span and size.
    let d = with(&s, ReportKind::OneLine, |o| {
        o.columns = vec![
            ReportColumn::of(StripField::Scene),
            ReportColumn::of(StripField::Cast),
            ReportColumn {
                line: 1,
                span: 2,
                bold: true,
                size: Some(TextSize::Small),
                ..ReportColumn::of(StripField::Synopsis)
            },
        ];
    });
    let syn = &d.layout[2];
    assert_eq!((syn.field, syn.line, syn.span), ("synopsis", 2, 2));
    assert!(syn.bold);
    assert_eq!(syn.size, "small");
    assert_eq!(d.layout[0].size, "");
    assert!(r
        .days
        .iter()
        .flat_map(|d| &d.lines)
        .all(|l| l.synopsis.is_empty()));
    assert_eq!(r.head.rows.len(), 1);
    assert_eq!(r.head.rows[0][0].kind, "title");
    assert_eq!(r.head.rows[0][0].text, s.schedule().title);
    // The category filter.
    let props = {
        let mut x = harbor();
        x.edit(|y| {
            let p = y.add_category("Props");
            let n = y
                .add_element(p, "Netting needle")
                .map_err(|e| e.to_string())?;
            let h = y.sheets[0].id;
            y.tag(h, n).map_err(|e| e.to_string())
        })
        .unwrap();
        x
    };
    let cast = props.schedule().cast().unwrap();
    let r = with(&props, ReportKind::Shooting, |o| o.categories = vec![cast]);
    assert!(r.days[0].lines[0]
        .groups
        .iter()
        .all(|g| g.category == "Cast"));
    assert!(r.filtered.iter().any(|c| c == "Props"));
    assert!(!r.filtered.iter().any(|c| c == "Cast"));
}

#[test]
fn breakdowns_detail_vertical_and_calendar() {
    let s = harbor();
    let b = of(&s, ReportKind::Breakdown);
    assert_eq!(b.blocks.len(), s.schedule().sheets.len());
    assert_eq!(b.blocks[0].day, Some(1));
    assert_eq!(b.blocks[0].line.cells[2], "NET LOFT");

    let d = of(&s, ReportKind::BreakdownDay);
    // Each day lists its scenes with their fields (RPT-06, decision 161): the default
    // columns, as the breakdown by strip's, and a cell for each.
    let fields: Vec<_> = d.layout.iter().map(|c| c.field).collect();
    assert_eq!(fields, ["scene", "int_ext", "set", "time_of_day", "pages"]);
    assert!(d.days.len() > 1);
    let mut listed = 0;
    for day in &d.days {
        assert!(!day.lines.is_empty(), "day {} lists no scenes", day.number);
        assert_eq!(day.scenes, day.lines.len());
        for l in &day.lines {
            assert_eq!(l.cells.len(), fields.len());
            assert_eq!(l.cells[0], l.scenes);
            assert_eq!(l.cells[2], l.set);
            assert!(!l.set.is_empty());
        }
        listed += day.lines.len();
    }
    assert_eq!(listed, s.schedule().sheets.len());
    assert_eq!(d.days[0].lines[0].cells[2], "NET LOFT");
    assert_eq!(d.days[0].groups[0].category, "Cast");
    assert!(d.days[0].groups[0]
        .elements
        .contains(&"2 KIT (2)".to_string()));

    let e = of(&s, ReportKind::ElementDetail);
    let kit = e.details.iter().find(|x| x.name == "2 KIT").unwrap();
    assert_eq!(kit.work_dates.len(), 3);
    assert_eq!(kit.scenes.len(), 4);

    let v = of(&s, ReportKind::Vertical);
    let v = v.vertical.unwrap();
    assert_eq!(v.cast[0], ("1".to_string(), "TOVE".to_string()));
    assert!(v.columns[0].first && v.columns[0].rows == [0, 1]);
    // Left off the board, TOVE keeps its row, ID and name (RPT-20, decision 142).
    let mut off = harbor();
    off.edit(|x| {
        let cast = x.category_named("Cast").unwrap();
        let tove = x.element_named(cast, "TOVE").unwrap();
        x.element_mut(tove).unwrap().exclude_from_board = true;
        Ok(())
    })
    .unwrap();
    let w = of(&off, ReportKind::Vertical).vertical.unwrap();
    assert_eq!(w.cast, v.cast);
    assert_eq!(w.columns[0].rows, [0, 1]);
    // Behavior note 23: each day's last scene carries the day's totals for its dark day
    // break column; no production info filled in, none on the left.
    for (i, c) in v.columns.iter().enumerate() {
        let last = v.columns.get(i + 1).is_none_or(|n| n.day != c.day);
        assert_eq!(c.end.is_some(), last, "column {i}");
    }
    assert!(v.production.is_empty());
    let mut with = harbor();
    with.edit(|x| {
        x.production.producer = "ZTEST Producer".into();
        x.production.script_date = "2027-01-15".into();
        Ok(())
    })
    .unwrap();
    let v = of(&with, ReportKind::Vertical).vertical.unwrap();
    assert_eq!(
        v.production,
        [
            ("Producer", "ZTEST Producer".to_string()),
            ("Script date", "2027-01-15".to_string())
        ]
    );

    let c = of(&s, ReportKind::Calendar);
    assert_eq!(c.week[0], "Sun");
    assert_eq!(c.months.len(), 1);
    assert_eq!(c.months[0].title, "March 2027");
    let mar1 = c.months[0]
        .weeks
        .iter()
        .flatten()
        .find(|x| x.date == "2027-03-01")
        .unwrap();
    assert_eq!(mar1.shoot_day, Some(1));
    assert_eq!(mar1.scenes.len(), 2);
    let mon = build(
        s.schedule(),
        sub(&s),
        0,
        &{
            let mut r = ReportSetup::of(ReportKind::Calendar);
            r.options.weekdays_only = true;
            r
        },
        "mon",
    )
    .unwrap();
    assert_eq!(mon.week, ["Mon", "Tue", "Wed", "Thu", "Fri"]);
    assert!(mon.months[0].weeks.iter().all(|w| w.len() == 5));

    let m = of(&s, ReportKind::MultiUnit);
    assert_eq!(m.units[0].units[0].name, "Main");
    assert!(m.undated.is_empty());
}

#[test]
fn dood_elements_totals_and_the_flag_reports() {
    let s = harbor();
    let d = of(&s, ReportKind::Dood);
    assert_eq!(d.dood.len(), 1);
    assert_eq!(d.dood[0].category, "Cast");
    let grid = &d.dood[0].grid;
    assert!(grid.categories.is_empty());
    let kit = grid.rows.iter().find(|r| r.name == "KIT").unwrap();
    assert_eq!(kit.total, kit.work + kit.hold);
    assert_eq!(kit.cells.len(), grid.dates.len());
    // A date range narrows the grid; the element filter its rows.
    let tove = s
        .schedule()
        .element_named(s.schedule().cast().unwrap(), "TOVE")
        .unwrap();
    let r = with(&s, ReportKind::Dood, |o| {
        o.from = "2027-03-02".parse().ok();
        o.elements = vec![tove];
    });
    assert!(r.dood.is_empty() || r.dood[0].grid.rows.iter().all(|x| x.name == "TOVE"));

    let e = of(&s, ReportKind::Elements);
    assert_eq!(e.elements[0].name, "TOVE");
    let kit = e.elements.iter().find(|x| x.name == "KIT").unwrap();
    assert_eq!(kit.start.as_deref(), Some("2027-03-01"));

    let t = of(&s, ReportKind::Totals);
    let totals = t.totals.unwrap();
    assert_eq!(totals.categories, ["Cast"]);
    // Every other category (a new schedule's defaults among them) is unused.
    assert_eq!(totals.left_out, s.schedule().categories.len() - 1);
    assert_eq!(totals.days.len(), 3);

    let c = of(&s, ReportKind::Conflicts);
    assert!(c.conflicts.is_some());
    assert!(c.empty.unwrap().contains("no conflicts"));
    let f = of(&s, ReportKind::RedFlags);
    assert!(f.red_flags.is_some());
    assert!(f.empty.unwrap().contains("no red flags"));
}

#[test]
fn an_empty_schedule_says_why() {
    let s = Session::new();
    for (i, _) in s.schedule().report_list().iter().enumerate() {
        let r = build_report(s.schedule(), sub(&s), i, "sun").unwrap();
        let why = r
            .empty
            .unwrap_or_else(|| panic!("{} is empty but says nothing", r.name));
        assert!(why.ends_with('.') && !why.contains('!'), "{why}");
    }
}

#[test]
fn unused_categories_leave_the_totals() {
    let mut s = harbor();
    s.edit(|x| {
        x.add_category("Props");
        Ok(())
    })
    .unwrap();
    let t = of(&s, ReportKind::Totals).totals.unwrap();
    assert_eq!(t.categories, ["Cast"]);
    assert_eq!(t.left_out, s.schedule().categories.len() - 1);
    // A category filter leaves the others out of the count too.
    let cast = s.schedule().cast().unwrap();
    let f = with(&s, ReportKind::Totals, |o| o.categories = vec![cast]);
    assert_eq!(f.totals.unwrap().left_out, 0);
}

#[test]
fn sort_keys_read_strictly() {
    assert_eq!(
        parse_sort(&["work:desc".into(), "name".into()])
            .unwrap()
            .len(),
        2
    );
    assert!(parse_sort(&["scene".into()]).is_err());
    assert!(parse_sort(&["work:down".into()]).is_err());
    assert!(parse_sort(&vec!["name".to_string(); 5]).is_err());
}

/// The DOOD tab's headings sort by the engine's own keys, each the key it names: Total
/// is no key, so its heading does not sort (it once sorted by work).
#[test]
fn the_dood_headings_sort_by_engine_keys() {
    use method_schedule_core::dood::SortKey;
    const JS: &str = include_str!("../../ui/js/dood.js");
    let line = JS
        .lines()
        .find(|l| l.starts_with("const SORT_BY = {"))
        .unwrap();
    let body = line
        .trim_start_matches("const SORT_BY = {")
        .trim_end_matches("};");
    let pairs: Vec<(&str, &str)> = body
        .split(',')
        .map(|p| {
            let (k, v) = p.split_once(':').unwrap();
            (k.trim(), v.trim().trim_matches('\''))
        })
        .collect();
    let want = [
        ("ID", SortKey::BoardId),
        ("Element", SortKey::Name),
        ("Work", SortKey::Work),
        ("Hold", SortKey::Hold),
        ("Start", SortKey::Start),
        ("Finish", SortKey::Finish),
    ];
    assert_eq!(pairs.len(), want.len(), "{line}");
    for (heading, key) in want {
        let got = pairs.iter().find(|(k, _)| *k == heading).unwrap().1;
        assert_eq!(SortKey::from_name(got), Some(key), "{heading}");
    }
}

#[test]
fn pdf_names() {
    assert_eq!(
        pdf_file_name("Harbor Light - One-line"),
        "Harbor Light - One-line.pdf"
    );
    assert_eq!(pdf_file_name("A/B: C?"), "A-B- C-.pdf");
    assert_eq!(pdf_file_name("  "), "Report.pdf");
    assert_eq!(pdf_file_name("..hidden"), "hidden.pdf");
    assert_eq!(pdf_file_name("line\nbreak"), "line break.pdf");
    assert_eq!(pdf_file_name(&"x".repeat(300)).len(), 124);
}

#[test]
fn pdf_properties_name_the_report_and_the_company() {
    let mut s = harbor().schedule().clone();
    let none = pdf_info(&s, "Harbor Light - One-line", None);
    assert_eq!(none.title, "Harbor Light - One-line");
    assert_eq!(none.subject, "");
    assert_eq!(none.author, "");
    assert_eq!(none.creator, "Method Schedule");
    s.production.company = "ZTEST Pictures\n".into();
    let i = pdf_info(&s, "T", Some("One-line\u{7}; Board 1, Main"));
    assert_eq!(i.author, "ZTEST Pictures");
    assert_eq!(i.subject, "One-line ; Board 1, Main");
}

#[test]
fn the_calendar_month_has_strips_drop_places_and_the_users_week() {
    let s = harbor();
    let opts = crate::view::CalOpts {
        monday: true,
        fields: vec![StripField::Scene, StripField::Cast],
        span: false,
    };
    let c =
        crate::view::calendar_view_with(s.schedule(), sub(&s), Some(2027), Some(3), &opts).unwrap();
    assert_eq!(c.week[0], "Mon");
    assert_eq!(c.weeks[0][0].weekday, "Mon");
    assert_eq!(c.weeks[0][0].date, "2027-03-01");
    let mar1 = &c.weeks[0][0];
    assert_eq!(mar1.strips.len(), 2);
    assert_eq!(mar1.strips[0].cells, ["1", "1, 2"]);
    assert!(!mar1.pages.is_empty());
    // A strip dropped on day 2 goes before day 2's day break.
    let mar2 = &c.weeks[0][1];
    let at = mar2.drop_at.unwrap();
    let sb = &s.schedule().boards[0].sub_boards[0];
    assert!(matches!(
        sb.strips[at].kind,
        method_schedule_core::model::StripKind::DayBreak
    ));
    assert!(c.weeks[0][5].drop_at.is_none());
    assert_eq!(c.board, s.schedule().boards[0].id.0);
    let sun = crate::view::calendar_view(s.schedule(), sub(&s), Some(2027), Some(3)).unwrap();
    assert_eq!(sun.week[0], "Sun");
}

#[test]
fn banners_keep_their_look_in_the_one_line_and_shooting_schedule() {
    let mut s = harbor();
    let sb = sub(&s);
    s.edit(|x| {
        let style = board::BannerStyleInput {
            bold: true,
            color: Some("red".into()),
            tall: false,
            size: "large".into(),
            align: "center".into(),
            font: "mono".into(),
        };
        board::insert_banner_styled(x, sb, 0, "Company move", "0:30", Some(&style)).map(|_| ())
    })
    .unwrap();
    for kind in [ReportKind::OneLine, ReportKind::Shooting] {
        let d = of(&s, kind);
        let b = d.days[0].lines.iter().find(|l| l.kind == "banner").unwrap();
        assert_eq!(b.text, "Company move");
        assert!(b.bold && !b.tall);
        assert_eq!(
            (b.banner_color.as_str(), b.banner_size, b.banner_align),
            ("red", "large", "center")
        );
        // Its font family on paper too (decision 187).
        assert_eq!(b.banner_font, "mono");
    }
}

/// Decision 91: an omitted scene stays listed in the reports and on the board, struck
/// (the page reads `omitted`), and counts no work in the day out of days.
#[test]
fn an_omitted_scene_is_listed_struck() {
    let mut s = harbor();
    let first = s.schedule().sheets[0].id;
    s.edit(|x| {
        x.plan.omitted.push(first);
        Ok(())
    })
    .unwrap();
    let one = of(&s, ReportKind::OneLine);
    let line = &one.days[0].lines[0];
    assert_eq!(line.scenes, "1");
    assert!(line.omitted);
    assert!(one
        .days
        .iter()
        .flat_map(|d| &d.lines)
        .skip(1)
        .all(|l| !l.omitted));
    let json = serde_json::to_value(&one).unwrap();
    assert_eq!(json["days"][0]["lines"][0]["omitted"], true);
    assert!(json["days"][0]["lines"][1].get("omitted").is_none());
    let shooting = of(&s, ReportKind::Shooting);
    assert!(shooting.days[0].lines[0].omitted);
    let blocks = of(&s, ReportKind::Breakdown);
    assert!(blocks.blocks[0].line.omitted);
    // The board draws it struck too.
    let v = crate::view::view(&s, None).unwrap();
    let strips: Vec<_> = v.board.sub_boards[0]
        .days
        .iter()
        .flat_map(|d| &d.strips)
        .collect();
    let st = strips.iter().find(|x| x.sheet == Some(first.0)).unwrap();
    assert!(st.omitted);
    assert_eq!(strips.iter().filter(|x| x.omitted).count(), 1);
}

/// The calendar report prints at most two years and says so when the range is longer.
#[test]
fn a_calendar_report_past_two_years_says_what_prints() {
    let s = harbor();
    let r = with(&s, ReportKind::Calendar, |o| {
        o.from = "2027-03-01".parse().ok();
        o.to = "2029-12-31".parse().ok();
    });
    assert_eq!(r.months.len(), 24);
    assert!(r.range.unwrap().contains("first 24 months"));
    let r = with(&s, ReportKind::Calendar, |o| {
        o.from = "2027-03-01".parse().ok();
        o.to = "2027-04-30".parse().ok();
    });
    assert_eq!(r.months.len(), 2);
    assert_eq!(r.range.as_deref(), Some("2027-03-01 to 2027-04-30"));
}

/// A day out of days report whose date range misses every row says the range is why.
#[test]
fn a_dood_report_out_of_range_says_so() {
    let s = harbor();
    let r = with(&s, ReportKind::Dood, |o| o.from = "2030-01-01".parse().ok());
    assert!(r.dood.is_empty());
    assert!(r.empty.unwrap().contains("date range"));
    // In range: the cast's grid, clipped to the range from the week's first day, the
    // summary whole.
    let r = with(&s, ReportKind::Dood, |o| {
        o.from = "2027-03-02".parse().ok();
        o.to = "2027-03-02".parse().ok();
    });
    let g = &r.dood[0].grid;
    let dates: Vec<&str> = g.dates.iter().map(|d| d.date.as_str()).collect();
    assert_eq!(dates, ["2027-02-28", "2027-03-01", "2027-03-02"]);
    let kit = g.rows.iter().find(|x| x.name == "KIT").unwrap();
    assert_eq!(kit.work, 3);
    assert_eq!(kit.cells.len(), 3);
    assert_eq!(kit.cells[0], "", "before the shoot");
}

/// DOD-19 (behavior note 27, decision 141): the DOOD report pages by whole weeks from the
/// week's first day, so a range from Monday March 1 starts on Sunday February 28, or on
/// the Monday when weeks start on Monday. The tab's grid starts where the work does.
#[test]
fn the_dood_report_starts_on_the_weeks_first_day() {
    let s = harbor();
    let mut setup = ReportSetup::of(ReportKind::Dood);
    setup.options.from = "2027-03-01".parse().ok();
    let first = |week: &str| {
        let r = build(s.schedule(), sub(&s), 0, &setup, week).unwrap();
        let g = &r.dood[0].grid;
        let kit = g.rows.iter().find(|x| x.name == "KIT").unwrap();
        assert_eq!(kit.cells.len(), g.dates.len());
        let (a, z) = kit.span.unwrap();
        assert_eq!(g.dates[a].date, kit.start.clone().unwrap());
        assert!(z < g.dates.len());
        (g.dates[0].date.clone(), g.dates[0].weekday)
    };
    assert_eq!(first("sun"), ("2027-02-28".to_string(), "Sun"));
    assert_eq!(first("mon"), ("2027-03-01".to_string(), "Mon"));
    let tab = crate::view::dood_grid(s.schedule(), sub(&s), None).unwrap();
    assert_eq!(tab.dates[0].date, "2027-03-01");
}
