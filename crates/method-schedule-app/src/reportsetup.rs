//! The report list and the settings the schedule keeps for reports and the day out of
//! days (decision 59): each report's name, order and options, the production info, the
//! code letters and a sub-board's prep start and wrap end. Plus the user's own display
//! preferences (week start, date format, what a calendar strip shows), which live in the
//! app's `settings.json`, not in the schedule.
//!
//! Every schedule change is one undo step through `Session::edit`; options travel from
//! the page as plain values and are read strictly here (typed dates through
//! `calendar::typed_date`, names of fields, header layouts and sizes checked by name).
use crate::app::{finish, AppState, CmdResult};
use crate::board::EditResult;
use crate::docs::Doc;
use method_schedule_core::headers::HeaderId;
use method_schedule_core::model::Schedule;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow};

pub use method_schedule_session::reportsetup::*;

fn edit<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    f: impl FnOnce(&mut Schedule) -> EditResult,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(f)?;
    finish(window, &s, None)
}

/// The report list, the production info and what the filters can pick.
#[tauri::command]
pub fn report_list(state: Doc) -> CmdResult<ReportsDto> {
    Ok(reports_dto(state.lock().schedule()))
}

#[tauri::command]
pub fn save_report<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    index: usize,
    name: String,
    options: OptionsDto,
) -> CmdResult {
    edit(&window, &state, |s| apply_report(s, index, &name, &options))
}

/// Adds a copy right after report `index`; the view's notice names it.
#[tauri::command]
pub fn duplicate_report<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    index: usize,
) -> CmdResult {
    let mut s = state.lock();
    let at = s.edit(|x| x.duplicate_report(index).map_err(say))?;
    let name = s
        .schedule()
        .report_list()
        .get(at)
        .map_or(String::new(), |r| r.name.clone());
    finish(&window, &s, Some(format!("Added {name}.")))
}

#[tauri::command]
pub fn move_report<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    index: usize,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| s.move_report(index, to).map_err(say))
}

#[tauri::command]
pub fn delete_report<R: Runtime>(window: WebviewWindow<R>, state: Doc, index: usize) -> CmdResult {
    edit(&window, &state, |s| s.delete_report(index).map_err(say))
}

/// Saves a header layout (id 0: a new one); the notice names it.
#[tauri::command]
pub fn save_header_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    design: HeaderDto,
) -> CmdResult {
    let mut s = state.lock();
    let added = design.id == 0;
    s.edit(|x| apply_header(x, &design).map(|_| ()))?;
    let note = added.then(|| format!("Added header layout {}.", design.name.trim()));
    finish(&window, &s, note)
}

#[tauri::command]
pub fn delete_header_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: u32,
) -> CmdResult {
    edit(&window, &state, |s| {
        s.delete_header(HeaderId(id)).map_err(header_say)
    })
}

#[tauri::command]
pub fn move_header_layout<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    id: u32,
    to: usize,
) -> CmdResult {
    edit(&window, &state, |s| {
        s.move_header(HeaderId(id), to).map_err(header_say)
    })
}

/// `field`: `title` or a production field's key (`company`, `art_director`...).
#[tauri::command]
pub fn set_production<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    field: String,
    value: String,
) -> CmdResult {
    edit(&window, &state, |s| apply_production(s, &field, &value))
}

/// `codes`: the letters to change, by key (`hold`, `company_move`...).
#[tauri::command]
pub fn set_dood_codes<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    codes: std::collections::BTreeMap<String, String>,
) -> CmdResult {
    edit(&window, &state, |s| apply_codes(s, &codes))
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_prep_wrap<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sub_board: u32,
    prep_start: String,
    wrap_end: String,
) -> CmdResult {
    edit(&window, &state, |s| {
        apply_prep_wrap(s, sub_board, &prep_start, &wrap_end)
    })
}

/// Calendar > Strip Fields: what a strip shows on a date, kept in the schedule.
#[tauri::command]
pub fn set_calendar_fields<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    fields: Vec<String>,
) -> CmdResult {
    edit(&window, &state, |s| apply_calendar_fields(s, &fields))
}

#[tauri::command]
pub fn preferences<R: Runtime>(app: AppHandle<R>) -> Prefs {
    app.state::<crate::safety_app::Safety>().prefs()
}

/// The event every window hears when the preferences change; the payload is the new
/// preferences. Each window redraws with them (they are the user's, not one schedule's).
pub const PREFS_CHANGED: &str = "prefs-changed";

#[tauri::command]
pub fn set_preferences<R: Runtime>(app: AppHandle<R>, prefs: Prefs) -> Result<Prefs, String> {
    let p = prefs.checked()?;
    app.state::<crate::safety_app::Safety>().set_prefs(&p);
    let _ = app.emit(PREFS_CHANGED, &p);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use method_schedule_core::model::StripField;
    use method_schedule_core::setup::{ReportColumn, TextSize};

    fn opts(s: &Schedule, i: usize) -> OptionsDto {
        OptionsDto::of(&s.report_list()[i].options)
    }

    /// The harness's fake (harness/fake-tauri.js) refuses with these words too: each pair
    /// is a piece of Rust's message that the fake's source holds as written.
    #[test]
    fn the_harness_fake_refuses_in_the_same_words() {
        let fake = include_str!("../harness/fake-tauri.js");
        let mut s = Schedule::new("ZTEST Reports");
        let sub = s.boards[0].sub_boards[0].id.0;
        let code =
            |v: &str| std::collections::BTreeMap::from([("hold".to_string(), v.to_string())]);
        let o = opts(&s, 0);
        let first = s.report_list()[0].name.clone();
        let cases = [
            (
                apply_codes(&mut s, &code("")),
                " code must be 1 to 3 characters.",
            ),
            (
                apply_codes(&mut s, &code("a!")),
                " may hold only letters, digits and /.",
            ),
            (apply_codes(&mut s, &code("S")), " is used twice."),
            (
                apply_prep_wrap(&mut s, sub, "", "soon"),
                " is not a date. Type it as 3/1/2027 or 2027-03-01.",
            ),
            (apply_prep_wrap(&mut s, sub, "soon", ""), "The prep start"),
            (apply_prep_wrap(&mut s, sub, "", "soon"), "The wrap end"),
            (
                apply_report(&mut s, 99, "X", &o),
                "That report is no longer in the list.",
            ),
            (
                apply_report(&mut s, 0, &"x".repeat(201), &o),
                "A report name is 200 characters at most.",
            ),
            (apply_report(&mut s, 1, &first, &o), "A report named "),
            (
                apply_report(&mut s, 1, &first, &o),
                " is already in the list.",
            ),
            (
                s.delete_report(99).map_err(say),
                "That report is not in the list.",
            ),
            (
                s.move_report(99, 0).map_err(say),
                "That report is not in the list.",
            ),
        ];
        for (said, piece) in cases {
            let said = said.unwrap_err();
            assert!(said.contains(piece), "Rust said {said:?}, not {piece:?}");
            assert!(fake.contains(piece), "the fake never says {piece:?}");
        }
    }

    #[test]
    fn options_read_strictly_and_save_as_one_report() {
        let mut s = Schedule::new("ZTEST Reports");
        let mut o = opts(&s, 0);
        o.from = "3/1/2027".into();
        o.to = "2027-03-05".into();
        o.synopsis = false;
        o.columns = vec![
            ColumnDto::Field("scene".into()),
            ColumnDto::Field("set".into()),
        ];
        o.header = 2;
        apply_report(&mut s, 0, "  Week one ", &o).unwrap();
        let r = &s.report_list()[0];
        assert_eq!(r.name, "Week one");
        assert_eq!(r.options.from, "2027-03-01".parse().ok());
        assert_eq!(
            r.options.columns,
            [
                ReportColumn::of(StripField::Scene),
                ReportColumn::of(StripField::Set)
            ]
        );
        assert_eq!(r.options.header, HeaderId::COMPACT);
        // Round trip through the page's shape.
        assert_eq!(opts(&s, 0).read().unwrap(), r.options);

        let mut bad = o.clone();
        bad.from = "3/1/27".into();
        assert!(apply_report(&mut s, 0, "x", &bad)
            .unwrap_err()
            .contains("not a date"));
        bad = o.clone();
        bad.to = "2027-02-01".into();
        assert!(apply_report(&mut s, 0, "x", &bad)
            .unwrap_err()
            .contains("before it starts"));
        bad = o.clone();
        bad.header = 99;
        assert!(apply_report(&mut s, 0, "x", &bad)
            .unwrap_err()
            .contains("no longer in the list"));
        bad = o.clone();
        bad.page_break = "month".into();
        assert!(apply_report(&mut s, 0, "x", &bad).is_err());
        bad = o.clone();
        bad.columns = vec![
            ColumnDto::Field("scene".into()),
            ColumnDto::Field("scene".into()),
        ];
        assert!(apply_report(&mut s, 0, "x", &bad).is_err());
        // The report designer: a merged cell under two columns, bold and large; a later
        // line wider than the first is refused.
        let mut cells = o.clone();
        cells.columns.push(ColumnDto::Cell(CellDto {
            field: "synopsis".into(),
            line: 2,
            span: 2,
            bold: true,
            size: "large".into(),
            ..CellDto::default()
        }));
        apply_report(&mut s, 0, "Week one", &cells).unwrap();
        let syn = s.report_list()[0].options.columns[2];
        assert_eq!((syn.line, syn.span, syn.bold), (1, 2, true));
        assert_eq!(syn.size, Some(TextSize::Large));
        assert_eq!(opts(&s, 0).read().unwrap(), s.report_list()[0].options);
        if let ColumnDto::Cell(c) = &mut cells.columns[2] {
            c.span = 3;
        }
        let err = apply_report(&mut s, 0, "Week one", &cells).unwrap_err();
        assert!(err.contains("covers 3 columns"), "{err}");
        assert!(apply_report(&mut s, 0, " ", &o).is_err());
        let err = apply_report(&mut s, 0, "Shooting schedule", &o).unwrap_err();
        assert!(err.contains("already in the list"), "{err}");
        assert!(apply_report(&mut s, 99, "x", &o).is_err());
        let last = s
            .report_list()
            .iter()
            .position(|r| r.kind == method_schedule_core::setup::ReportKind::Calendar)
            .unwrap();
        let err = s.delete_report(last).map_err(say).unwrap_err();
        assert!(err.contains("last of its kind"), "{err}");
        assert!(!err.contains('!'));
    }

    #[test]
    fn header_layouts_save_read_strictly_and_reports_pick_them() {
        let mut s = Schedule::new("ZTEST Reports");
        let dto = reports_dto(&s);
        assert_eq!(dto.headers.len(), 3);
        assert!(dto.header_items.iter().any(|(k, _)| *k == "set_dresser"));
        let mut h = dto.headers[0].clone();
        h.id = 0;
        h.name = " Wide top ".into();
        h.align = "center".into();
        h.rows = vec![vec![
            HeaderItemDto::Name("title".into()),
            HeaderItemDto::Name("space".into()),
            HeaderItemDto::Text(HeaderTextDto {
                text: " Draft ".into(),
            }),
        ]];
        let id = apply_header(&mut s, &h).unwrap();
        assert_eq!(id, 4);
        let back = &reports_dto(&s).headers[3];
        assert_eq!(back.name, "Wide top");
        assert_eq!(
            back.rows[0][2],
            HeaderItemDto::Text(HeaderTextDto {
                text: "Draft".into()
            })
        );
        assert!(apply_header(&mut s, &h).unwrap_err().contains("already"));
        let mut bad = h.clone();
        bad.name = "Other".into();
        bad.align = "justify".into();
        assert!(apply_header(&mut s, &bad).is_err());
        bad.align = "left".into();
        bad.rows = vec![vec![HeaderItemDto::Name("writer".into())]];
        assert!(apply_header(&mut s, &bad).is_err());
        let mut o = opts(&s, 0);
        o.header = id;
        apply_report(&mut s, 0, "One-line schedule", &o).unwrap();
        assert_eq!(s.report_list()[0].options.header, HeaderId(id));
        s.delete_header(HeaderId(id)).unwrap();
        // The report keeps its pick and prints with the first layout.
        assert_eq!(s.header_of(HeaderId(id)).name, "Full");
    }

    #[test]
    fn calendar_fields_live_in_the_schedule() {
        let mut s = Schedule::new("ZTEST Calendar");
        assert_eq!(
            s.calendar_strip_fields(),
            method_schedule_core::model::DEFAULT_CALENDAR_FIELDS
        );
        apply_calendar_fields(&mut s, &["set".into(), "scene".into()]).unwrap();
        assert_eq!(s.calendar_fields, [StripField::Set, StripField::Scene]);
        assert!(apply_calendar_fields(&mut s, &[]).is_err());
        assert!(apply_calendar_fields(&mut s, &["spacer".into()]).is_err());
        assert!(apply_calendar_fields(&mut s, &["set".into(), "set".into()]).is_err());
        // The older per-user pick counts only when the user changed it.
        assert!(!old_calendar_pick(&[
            StripField::Scene,
            StripField::Set,
            StripField::Pages
        ]));
        assert!(old_calendar_pick(&[StripField::Scene, StripField::Cast]));
        assert!(!old_calendar_pick(&[]));
    }

    #[test]
    fn production_codes_and_prep() {
        let mut s = Schedule::new("ZTEST Reports");
        apply_production(&mut s, "company", " ZTEST Pictures ").unwrap();
        assert_eq!(s.production.company, "ZTEST Pictures");
        apply_production(&mut s, "title", "ZTEST Two").unwrap();
        assert_eq!(s.title, "ZTEST Two");
        assert!(apply_production(&mut s, "title", " ").is_err());
        assert!(apply_production(&mut s, "writer", "x").is_err());
        apply_production(&mut s, "art_director", " A. Director ").unwrap();
        assert_eq!(s.production.art_director, "A. Director");
        let dto = reports_dto(&s);
        assert_eq!(dto.production.len(), 16);
        assert_eq!(
            dto.production[5],
            ("art_director", "Art director", "A. Director".to_string())
        );

        let mut c = std::collections::BTreeMap::new();
        c.insert("hold".to_string(), "W".to_string());
        let err = apply_codes(&mut s, &c).unwrap_err();
        assert!(err.ends_with("used twice."), "{err}");
        c.insert("hold".to_string(), "HD".to_string());
        apply_codes(&mut s, &c).unwrap();
        assert_eq!(s.dood_codes.hold, "HD");
        c.insert("nope".to_string(), "X".to_string());
        assert!(apply_codes(&mut s, &c).is_err());

        let sub = crate::view::first_sub(&s);
        apply_prep_wrap(&mut s, sub, "1/4/2027", "").unwrap();
        assert_eq!(
            s.boards[0].sub_boards[0].calendar.prep_start,
            "2027-01-04".parse().ok()
        );
        assert!(apply_prep_wrap(&mut s, sub, "2027-02-01", "2027-01-01").is_err());
        assert!(apply_prep_wrap(&mut s, sub, "soon", "").is_err());
        assert!(s.check().is_ok());
    }

    #[test]
    fn preferences_are_checked() {
        let p = Prefs::default();
        assert!(p.checked().is_ok());
        let mut q = p.clone();
        q.week_start = "tue".into();
        assert!(q.checked().is_err());
        q = p.clone();
        q.date_format = "eu".into();
        assert!(q.checked().is_err());
        q = p.clone();
        q.calendar_fields = vec![];
        assert!(q.checked().is_err());
        q.calendar_fields = vec!["scene".into(), "scene".into()];
        assert!(q.checked().is_err());
        // Fountain lines per page (decision 195): 56 by default, 40 to 70, refused in
        // the harness fake's words too.
        assert_eq!(p.fountain_lines, 56);
        let fake = include_str!("../harness/fake-tauri.js");
        for (n, ok) in [(40, true), (70, true), (55, true), (39, false), (71, false)] {
            q = p.clone();
            q.fountain_lines = n;
            match q.checked() {
                Ok(_) => assert!(ok, "{n}"),
                Err(e) => {
                    assert!(!ok, "{n}");
                    assert!(fake.contains(&e), "{e}");
                }
            }
        }
    }

    /// Every report setup edit is one undo step, and redo brings it back exactly:
    /// options, copies, order, deletes, header layouts, production info, code letters.
    #[test]
    fn each_report_edit_undoes_and_redoes_as_one_step() {
        use crate::session::Session;
        let mut s = Session::new();
        let mut o = opts(s.schedule(), 0);
        o.synopsis = false;
        type Step = Box<dyn Fn(&mut Schedule) -> EditResult>;
        let steps: Vec<Step> = vec![
            Box::new(move |x| apply_report(x, 0, "Week one", &o)),
            Box::new(|x| x.duplicate_report(0).map(|_| ()).map_err(say)),
            Box::new(|x| x.move_report(1, 3).map_err(say)),
            Box::new(|x| x.delete_report(3).map_err(say)),
            Box::new(|x| {
                let mut h = HeaderDto::of(&x.header_list()[0]);
                h.id = 0;
                h.name = "Mine".into();
                apply_header(x, &h).map(|_| ())
            }),
            Box::new(|x| apply_production(x, "director", "A. Director")),
            Box::new(|x| {
                apply_codes(
                    x,
                    &std::collections::BTreeMap::from([("hold".to_string(), "HD".to_string())]),
                )
            }),
        ];
        for step in &steps {
            let before = s.schedule().clone();
            s.edit(|x| step(x)).unwrap();
            let after = s.schedule().clone();
            assert_ne!(before, after);
            assert!(s.undo());
            assert_eq!(*s.schedule(), before);
            assert!(s.redo());
            assert_eq!(*s.schedule(), after);
        }
    }
}
