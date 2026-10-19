//! Tests of the board and the view that read reports and exports, which stay on the
//! desktop: kept here when the board and the view moved to `method-schedule-session`.

mod board {
    use crate::board::*;
    use crate::session::Session;
    use method_schedule_core::model::*;
    use method_schedule_core::Eighths;

    /// A schedule with scenes 1 to 4 on Main and 5 in the Boneyard.
    fn sample() -> (Session, u32, u32) {
        let mut s = Session::new();
        s.edit(|x| {
            for n in 1..=5 {
                x.add_sheet(Sheet {
                    scenes: n.to_string(),
                    pages: Eighths(8),
                    ..Sheet::default()
                });
            }
            let b = x.boards[0].id;
            let sb = x.boards[0].sub_boards[0].id;
            let ids: Vec<StripId> = x.boards[0].boneyard[..4].iter().map(|st| st.id).collect();
            x.move_strips(
                b,
                &ids,
                Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .map_err(say)
        })
        .unwrap();
        let b = s.schedule().boards[0].id.0;
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        (s, b, sb)
    }

    fn col(field: &str, width: &str) -> LayoutColumnInput {
        LayoutColumnInput {
            field: field.to_string(),
            width: width.to_string(),
            line: 1,
            span: 1,
            align: String::new(),
            bold: false,
            italic: false,
            text: String::new(),
            category: None,
            board_ids: false,
            rule_after: false,
            rule_under: false,
        }
    }

    #[test]
    fn a_category_elements_field_lists_one_category() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        let props = s
            .edit(|x| {
                let props = x.add_category("Props");
                let rope = x.add_element(props, "ROPE").map_err(|e| e.to_string())?;
                let lamp = x.add_element(props, "LAMP").map_err(|e| e.to_string())?;
                x.set_board_id(lamp, Some("P1"))
                    .map_err(|e| e.to_string())?;
                let first = x.sheets[0].id;
                x.sheet_mut(first).unwrap().elements = vec![rope, lamp];
                Ok(props)
            })
            .unwrap();
        let list = |c: Option<u32>, ids: bool| LayoutColumnInput {
            category: c,
            board_ids: ids,
            ..col("category_elements", "wide")
        };
        let cols = [col("scene", "narrow"), list(Some(props.0), false)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.layout[1].label, "Props");
        let cells =
            |v: &crate::view::View, i: usize| v.board.sub_boards[0].days[0].strips[i].cells.clone();
        assert_eq!(cells(&v, 0), ["1", "LAMP, ROPE"]);
        assert_eq!(cells(&v, 1), ["2", ""]);
        let cols = [col("scene", "narrow"), list(Some(props.0), true)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert!(v.board.layout[1].board_ids);
        assert_eq!(cells(&v, 0), ["1", "P1, ROPE"]);
        // The one-line report follows the board's layout, so it lists them too.
        let r = crate::reports::build_report(s.schedule(), sb, 0, "sun").unwrap();
        assert_eq!(r.layout[1].label, "Props");
        assert_eq!(r.days[0].lines[0].cells, ["1", "P1, ROPE"]);
        // No category: an empty cell. The switch on another field is refused.
        let cols = [col("scene", "narrow"), list(None, true)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        assert_eq!(cells(&crate::view::view(&s, None).unwrap(), 0), ["1", ""]);
        let bad = [LayoutColumnInput {
            board_ids: true,
            ..col("scene", "narrow")
        }];
        let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
        assert!(e.ends_with('.'), "{e}");
    }

    #[test]
    fn the_strip_layout_is_one_step_and_the_board_and_one_line_follow_it() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        s.edit(|x| {
            let id = x.sheets[0].id;
            x.sheet_mut(id).unwrap().synopsis = "Tove mends the net.".into();
            Ok(())
        })
        .unwrap();
        let before = s.schedule().clone();
        let v = crate::view::view(&s, None).unwrap();
        let fields: Vec<&str> = v.board.layout.iter().map(|c| c.field).collect();
        assert_eq!(
            fields,
            ["scene", "int_ext", "set", "time_of_day", "pages", "cast"]
        );
        let cols = [
            col("synopsis", "widest"),
            col("scene", "narrow"),
            col("pages", "medium"),
        ];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let labels: Vec<&str> = v.board.layout.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Synopsis", "Scene", "Pages"]);
        assert_eq!(v.board.layout[2].width, "medium");
        let first = &v.board.sub_boards[0].days[0].strips[0];
        assert_eq!(first.cells, ["Tove mends the net.", "1", "1"]);
        assert!(
            v.board.sub_boards[0].days[0].end.cells.is_empty(),
            "day breaks have none"
        );
        let r = crate::reports::build_report(s.schedule(), sb, 0, "sun").unwrap();
        assert_eq!(r.layout.len(), 3);
        assert_eq!(r.days[0].lines[0].cells, ["Tove mends the net.", "1", "1"]);
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");
        // Refused, nothing changed.
        for bad in [
            vec![],
            vec![col("scene", "narrow"), col("scene", "wide")],
            vec![col("colour", "narrow")],
            vec![col("scene", "huge")],
        ] {
            let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
            assert!(e.ends_with('.') && !e.contains('!'), "{e}");
        }
        assert!(s.edit(|x| set_strip_layout(x, 999_999, &cols, "")).is_err());
        assert_eq!(s.schedule(), &before);
    }

    /// BRD-18 (behavior note 27, decision 147): the element count takes one category from
    /// the page and the strips show `Name (n)`; the one-line report counts every element.
    #[test]
    fn an_element_count_takes_a_category_from_the_page() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        let (cast, props) = s
            .edit(|x| {
                let cast = x.cast_or_add();
                let props = x.add_category("Props");
                let net = x.add_element(props, "Net").map_err(|e| e.to_string())?;
                let tove = x.add_element(cast, "Tove").map_err(|e| e.to_string())?;
                let first = x.sheets[0].id;
                x.sheet_mut(first).unwrap().elements = vec![net, tove];
                Ok((cast, props))
            })
            .unwrap();
        let count = |c: Option<u32>| LayoutColumnInput {
            category: c,
            ..col("element_count", "narrow")
        };
        let cols = [col("scene", "narrow"), count(Some(props.0))];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.layout[1].category, Some(props.0));
        let cells =
            |v: &crate::view::View, i: usize| v.board.sub_boards[0].days[0].strips[i].cells.clone();
        assert_eq!(cells(&v, 0), ["1", "Props (1)"]);
        assert_eq!(cells(&v, 1), ["2", "Props (0)"]);
        // Another pick replaces it; none counts every element, quiet at none.
        let cols = [col("scene", "narrow"), count(Some(cast.0))];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        assert_eq!(
            cells(&crate::view::view(&s, None).unwrap(), 0),
            ["1", "Cast (1)"]
        );
        let cols = [col("scene", "narrow"), count(None)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(cells(&v, 0), ["1", "2"]);
        assert_eq!(cells(&v, 1), ["2", ""]);
        // The report keeps its own count of every element.
        let cols = [col("scene", "narrow"), count(Some(props.0))];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let r = crate::reports::build_report(s.schedule(), sb, 0, "sun").unwrap();
        assert_eq!(r.layout[1].category, None);
        assert_eq!(r.days[0].lines[0].cells, ["1", "2"]);
        // Refused: a category the schedule lacks, or one on another field.
        let before = s.schedule().clone();
        for bad in [
            vec![col("scene", "narrow"), count(Some(999_999))],
            vec![LayoutColumnInput {
                category: Some(props.0),
                ..col("scene", "narrow")
            }],
        ] {
            let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
            assert!(e.ends_with('.') && !e.contains('!'), "{e}");
        }
        assert_eq!(s.schedule(), &before);
    }
}

mod view {
    use crate::board;
    use crate::session::{read_script, Session};
    use crate::view::*;
    use std::path::Path;

    /// A session holding Harbor Light, every scene still in the Boneyard.
    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        let mut s = Session::new();
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s
    }

    #[test]
    fn the_view_lists_boards_and_draws_the_one_shown_with_banners() {
        let mut s = harbor();
        let b = s.schedule().boards[0].id.0;
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        let ids: Vec<u32> = s.schedule().boards[0]
            .boneyard
            .iter()
            .map(|x| x.id.0)
            .collect();
        s.edit(|x| board::move_strips(x, b, &ids[..2], Some(sb), 0))
            .unwrap();
        s.edit(|x| board::insert_banner(x, sb, 1, "Lunch", "0:45"))
            .unwrap();
        s.edit(|x| board::insert_day_break(x, sb, 3)).unwrap();
        let v = view(&s, None).unwrap();
        let day = &v.board.sub_boards[0].days[0];
        assert_eq!(day.strips[1].kind, "banner");
        assert_eq!(
            (day.strips[1].text.as_str(), day.strips[1].duration.as_str()),
            ("Lunch", "0:45")
        );
        assert_eq!(day.scenes, 2);
        let copy = s
            .edit(|x| board::duplicate_board(x, b, "Rain plan"))
            .unwrap();
        s.edit(|x| board::rename_board(x, b, "Dry")).unwrap();
        let names: Vec<_> = view(&s, None)
            .unwrap()
            .boards
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(names, ["Dry", "Rain plan"]);
        s.show_board(method_schedule_core::model::BoardId(copy))
            .unwrap();
        let v = view(&s, None).unwrap();
        assert_eq!(v.board.id, copy);
        assert_eq!(v.board.name, "Rain plan");
        // The copy's sub-board has its own id; the other tabs take it.
        let csb = v.board.sub_boards[0].id;
        assert_ne!(csb, sb);
        assert!(dood_grid(s.schedule(), csb, None).is_ok());
        assert!(calendar_view(s.schedule(), csb, None, None).is_ok());
        assert!(elements_view(s.schedule(), csb, None, None).is_ok());
        assert!(dood_grid(s.schedule(), 9999, None).is_err());
        let report = crate::reports::build_report(s.schedule(), csb, 0, "sun").unwrap();
        assert_eq!(
            (report.board.as_str(), report.sub_board.as_str()),
            ("Rain plan", "Main")
        );
        assert!(crate::export::workbook(s.schedule(), csb, crate::export::Report::OneLine).is_ok());
        assert!(crate::explain::explain_count(s.schedule(), csb, 1).is_err());
        // A sheet's place is on the board it is asked about.
        let sheet = v.board.sub_boards[0].days[0].strips[0].sheet.unwrap();
        let shown = sheet_view(s.schedule(), copy, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert!(shown.place.starts_with("Main, day 1"), "{}", shown.place);
        s.edit(|x| {
            board::move_strips(
                x,
                copy,
                &[v.board.sub_boards[0].days[0].strips[0].id],
                None,
                0,
            )
        })
        .unwrap();
        let moved = sheet_view(s.schedule(), copy, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert_eq!(moved.place, "Boneyard");
        let still = sheet_view(s.schedule(), b, Some(sheet), None)
            .unwrap()
            .unwrap();
        assert!(still.place.starts_with("Main, day 1"), "{}", still.place);
    }
}
