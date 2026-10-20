//! More reports as tables (decisions.md, entry 59): the day out of days grid, the
//! element detail, the breakdown and the whole board. Same rules as [`super::tables`]:
//! integers only, page lengths as eighths with the text beside them in [`PAGES`], dates
//! as real dates, the codes in the schedule's own letters.

use method_schedule_core::days::days;
use method_schedule_core::dood::{dood, summary};
use method_schedule_core::model::{
    BoardId, CategoryId, Error, Sheet as Header, StripKind, SubBoardId,
};
use method_schedule_core::report_data::{self, element_text};
use method_schedule_core::reports::cast_ids;
use method_schedule_core::{Date, Eighths, Schedule};

use super::{Cell, ExportError, Formula, Sheet, Workbook, OMITTED, PAGES};

fn int(n: u32) -> Cell {
    Cell::Int(u64::from(n))
}

fn pages(p: Eighths) -> [Cell; 2] {
    [int(p.0), Cell::Text(p.to_string())]
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn header_cells(h: &Header) -> Vec<Cell> {
    let [e, p] = pages(h.pages);
    vec![
        Cell::text(h.scenes.as_str()),
        Cell::text(h.int_ext.as_str()),
        Cell::text(h.set.as_str()),
        Cell::text(h.time_of_day.as_str()),
        e,
        p,
    ]
}

const HEADER: [&str; 6] = ["scenes", "int_ext", "set", "time_of_day", "eighths", PAGES];

/// The day out of days grid of a sub-board: a row per element (every category in
/// category order, or only `category`), a column per date from the first to the last date
/// any row covers, each cell the day's code, then the summary.
pub fn dood_grid(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
    category: Option<CategoryId>,
) -> Result<Workbook, ExportError> {
    dood_grid_of(s, &dood(s, board, sub)?, category)
}

/// [`dood_grid`] from rows already worked out (without holds, sorted: the command
/// line's `dood --grid` options); within a category the rows keep their order. The dates
/// run over the rows shown, so with `category` they are that category's, as in the
/// app's grid.
pub fn dood_grid_of(
    s: &Schedule,
    rows: &[method_schedule_core::dood::Row],
    category: Option<CategoryId>,
) -> Result<Workbook, ExportError> {
    let shown = |r: &&method_schedule_core::dood::Row| {
        category.is_none()
            || s.element(r.element)
                .is_some_and(|e| Some(e.category) == category)
    };
    let first = rows
        .iter()
        .filter(shown)
        .filter_map(|r| r.cells.first())
        .map(|c| c.0)
        .min();
    let last = rows
        .iter()
        .filter(shown)
        .filter_map(|r| r.cells.last())
        .map(|c| c.0)
        .max();
    let mut dates: Vec<Date> = Vec::new();
    if let (Some(mut d), Some(z)) = (first, last) {
        while d <= z {
            dates.push(d);
            d = d.add_days(1);
        }
    }
    let mut columns = names(&["category", "board_id", "element"]);
    columns.extend(dates.iter().map(|d| d.to_string()));
    columns.extend(names(&["work", "hold", "start", "finish", "total"]));
    let sums = summary(rows);
    let mut out = Vec::new();
    for cat in &s.categories {
        if category.is_some_and(|c| c != cat.id) {
            continue;
        }
        for (r, sum) in rows.iter().zip(&sums) {
            let e = s
                .element(r.element)
                .ok_or(Error::NoSuchElement(r.element))?;
            if e.category != cat.id {
                continue;
            }
            let mut line = vec![
                Cell::text(cat.name.as_str()),
                Cell::text(e.board_id.as_deref().unwrap_or("")),
                Cell::text(e.name.as_str()),
            ];
            // A row's cells run date by date with no gaps, so each sits at its offset.
            let mut codes = vec![String::new(); dates.len()];
            if let Some(&d0) = dates.first() {
                for (d, c) in &r.cells {
                    if let Some(code) = usize::try_from(d.0 - d0.0)
                        .ok()
                        .and_then(|i| codes.get_mut(i))
                    {
                        *code = c.code_with(&s.dood_codes);
                    }
                }
            }
            line.extend(codes.into_iter().map(Cell::text));
            // The total is work plus hold, as a working formula.
            let work = 3 + dates.len();
            line.extend([
                int(sum.work),
                int(sum.hold),
                Cell::date(sum.start),
                Cell::date(sum.finish),
                Formula::add(&[work, work + 1], out.len(), u64::from(sum.total)),
            ]);
            out.push(line);
        }
    }
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Day Out of Days Grid",
            columns,
            rows: out,
        }],
    })
}

/// The element detail of a sub-board: a row per element and scene it is in, in category
/// order, then board ID and name, each scene with its day and date.
pub fn element_detail(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
) -> Result<Workbook, ExportError> {
    let mut out = Vec::new();
    for d in report_data::element_detail(s, board, sub, |_, _| true)? {
        let e = s
            .element(d.element)
            .ok_or(Error::NoSuchElement(d.element))?;
        for (h, slot) in &d.scenes {
            let h = s.sheet(*h).ok_or(Error::NoSuchSheet(*h))?;
            let mut line = vec![
                Cell::text(s.category(d.category).map_or("", |c| c.name.as_str())),
                Cell::text(e.board_id.as_deref().unwrap_or("")),
                Cell::text(e.name.as_str()),
                slot.day.map_or(Cell::Empty, int),
                Cell::date(slot.date),
            ];
            line.extend(header_cells(h));
            out.push(line);
        }
    }
    let mut columns = names(&["category", "board_id", "element", "day", "date"]);
    columns.extend(names(&HEADER));
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Element Detail",
            columns,
            rows: out,
        }],
    })
}

/// The breakdown of a sub-board: a row per sheet in board order with its day, header
/// fields and a column per category listing its elements (`ID NAME`, separated by `; `).
pub fn breakdown(s: &Schedule, board: BoardId, sub: SubBoardId) -> Result<Workbook, ExportError> {
    let mut out = Vec::new();
    for b in report_data::breakdown_by_strip(s, board, sub)? {
        let h = s.sheet(b.sheet).ok_or(Error::NoSuchSheet(b.sheet))?;
        let mut line = vec![b.slot.day.map_or(Cell::Empty, int), Cell::date(b.slot.date)];
        line.extend(header_cells(h));
        line.push(Cell::text(h.synopsis.as_str()));
        line.push(Cell::text(h.location.as_str()));
        for cat in &s.categories {
            let els = b
                .elements
                .iter()
                .find(|(c, _)| *c == cat.id)
                .map(|(_, els)| {
                    els.iter()
                        .map(|e| element_text(s, *e))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            line.push(Cell::text(els));
        }
        out.push(line);
    }
    let mut columns = names(&["day", "date"]);
    columns.extend(names(&HEADER));
    columns.extend(names(&["synopsis", "location"]));
    columns.extend(s.categories.iter().map(|c| c.name.clone()));
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Breakdown",
            columns,
            rows: out,
        }],
    })
}

/// The breakdown by shoot day of a sub-board, the base of a call sheet: a row per day and
/// element the day's scenes list, days in order, each day's elements in category order
/// then board ID and name. `scenes` is how many of the day's scenes list it; `quantity` is
/// the day's quantity for a category that uses daily quantities (decision 63's rule,
/// empty for any other category, which counts once a day), and `partial` is `yes` when a
/// sheet's quantity was left out of it. A day with no elements has no rows.
pub fn breakdown_day(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
) -> Result<Workbook, ExportError> {
    let mut out = Vec::new();
    for d in report_data::breakdown_by_day(s, board, sub)? {
        for (c, els) in &d.elements {
            let cat = s.category(*c).map_or("", |x| x.name.as_str());
            for (e, n) in els {
                let x = s.element(*e).ok_or(Error::NoSuchElement(*e))?;
                let needed = d.needed.iter().find(|(y, _)| y == e).map(|(_, q)| *q);
                out.push(vec![
                    int(d.number),
                    Cell::date(d.date),
                    Cell::text(cat),
                    Cell::text(x.board_id.as_deref().unwrap_or("")),
                    Cell::text(x.name.as_str()),
                    int(*n),
                    needed.map_or(Cell::Empty, int),
                    Cell::text(if d.partial.contains(e) { "yes" } else { "" }),
                ]);
            }
        }
    }
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Breakdown by Day",
            columns: names(&[
                "day", "date", "category", "board_id", "element", "scenes", "quantity", "partial",
            ]),
            rows: out,
        }],
    })
}

/// The daily category totals of a sub-board: a row per shoot day with how many of each
/// category it uses (decision 63's counting, the same as `method-schedule totals`), a
/// column per category in schedule order, then `partial` naming the categories whose
/// count left out a quantity a sheet marked not to count daily. A last `total` row adds
/// each category's column up, as a working formula.
pub fn daily_totals(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
) -> Result<Workbook, ExportError> {
    let totals = method_schedule_core::reports::daily_category_totals(s, board, sub)?;
    let mut out = Vec::new();
    for t in &totals {
        let mut line = vec![int(t.number), Cell::date(t.date)];
        for c in &s.categories {
            let n = t
                .counts
                .iter()
                .find(|(x, _)| *x == c.id)
                .map_or(0, |(_, n)| *n);
            line.push(int(n));
        }
        let partial: Vec<&str> = s
            .categories
            .iter()
            .filter(|c| t.partial.contains(&c.id))
            .map(|c| c.name.as_str())
            .collect();
        line.push(Cell::text(partial.join("; ")));
        out.push(line);
    }
    let days = out.len();
    let mut last = vec![Cell::text("total"), Cell::Empty];
    for (i, c) in s.categories.iter().enumerate() {
        let sum: u64 = totals
            .iter()
            .map(|t| {
                t.counts
                    .iter()
                    .find(|(x, _)| *x == c.id)
                    .map_or(0, |(_, n)| u64::from(*n))
            })
            .sum();
        last.push(Formula::sum(2 + i, 0..days, sum));
    }
    last.push(Cell::Empty);
    out.push(last);
    let mut columns = names(&["day", "date"]);
    columns.extend(s.categories.iter().map(|c| c.name.clone()));
    columns.push("partial".to_string());
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Daily Totals",
            columns,
            rows: out,
        }],
    })
}

/// A whole board: every strip of every sub-board in board order (day breaks too), then
/// the Boneyard, each with where it is, its day and date, and its fields.
pub fn board(s: &Schedule, board: BoardId) -> Result<Workbook, ExportError> {
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let cast = s.cast();
    let mut out = Vec::new();
    let strip_row = |list: &str, day: Option<u32>, date: Option<Date>, kind: &StripKind| {
        let mut line = vec![
            Cell::text(list),
            day.map_or(Cell::Empty, int),
            Cell::date(date),
        ];
        match kind {
            StripKind::Sheet(id) => {
                let h = s.sheet(*id).ok_or(Error::NoSuchSheet(*id))?;
                line.push(Cell::text(if s.plan.is_omitted(*id) {
                    OMITTED
                } else {
                    "scene"
                }));
                line.extend(header_cells(h));
                line.push(h.shoot_time.map_or(Cell::Empty, |m| int(m.0)));
                line.push(Cell::text(cast_ids(s, h, cast).join(" ")));
                line.push(Cell::text(h.synopsis.as_str()));
            }
            StripKind::Banner(bn) => {
                line.push(Cell::text("banner"));
                line.extend(std::iter::repeat_n(Cell::Empty, HEADER.len()));
                line.push(int(bn.duration.0));
                line.push(Cell::Empty);
                line.push(Cell::text(bn.text.as_str()));
            }
            StripKind::DayBreak => {
                line.push(Cell::text("day_break"));
                line.extend(std::iter::repeat_n(Cell::Empty, HEADER.len() + 3));
            }
        }
        Ok::<_, Error>(line)
    };
    for sb in &b.sub_boards {
        let ds = days(s, b.id, sb.id)?;
        let mut day_of = std::collections::HashMap::new();
        for d in &ds.days {
            for st in d.strips.iter().chain(std::iter::once(&d.break_id)) {
                day_of.insert(*st, d.number);
            }
        }
        for st in &sb.strips {
            let day = day_of.get(&st.id).copied();
            let date = day.and_then(|n| sb.start.and_then(|x| sb.calendar.date_of_day(x, n)));
            out.push(strip_row(&sb.name, day, date, &st.kind)?);
        }
    }
    for st in &b.boneyard {
        out.push(strip_row("Boneyard", None, None, &st.kind)?);
    }
    let mut columns = names(&["list", "day", "date", "kind"]);
    columns.extend(names(&HEADER));
    columns.extend(names(&["minutes", "cast", "text"]));
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Board",
            columns,
            rows: out,
        }],
    })
}

/// Every red flag in the schedule (behavior note 23; the Red flags tab's table): its
/// first and last ruled-out dates (empty for an open start or end), category, element,
/// type, hard, the dates in words and the note, by first date (open starts first), then
/// the element's place in the schedule, then the flag's.
pub fn red_flags(s: &Schedule) -> Workbook {
    use method_schedule_core::model::RedFlagDates;
    let mut rows: Vec<(Option<Date>, usize, usize, Vec<Cell>)> = Vec::new();
    for (place, e) in s.elements.iter().enumerate() {
        let category = s.category(e.category).map_or("", |c| c.name.as_str());
        for (k, f) in e.red_flags.iter().enumerate() {
            let (start, end) = match f.dates {
                RedFlagDates::Range { from, to } => (Some(from), Some(to)),
                RedFlagDates::NotBefore(d) => (None, Some(d.add_days(-1))),
                RedFlagDates::NotAfter(d) => (Some(d.add_days(1)), None),
            };
            let kind = s.red_flag_type(f.kind).map_or("", |t| t.name.as_str());
            rows.push((
                start,
                place,
                k,
                vec![
                    Cell::date(start),
                    Cell::date(end),
                    Cell::text(category),
                    Cell::text(e.board_id.clone().unwrap_or_default()),
                    Cell::text(e.name.as_str()),
                    Cell::text(kind),
                    Cell::text(if f.hard { "hard" } else { "soft" }),
                    Cell::text(f.dates.text()),
                    Cell::text(f.note.as_str()),
                ],
            ));
        }
    }
    rows.sort_by_key(|r| (r.0, r.1, r.2));
    Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Red flags",
            columns: names(&[
                "start", "end", "category", "board_id", "element", "type", "hard", "dates", "note",
            ]),
            rows: rows.into_iter().map(|r| r.3).collect(),
        }],
    }
}

/// What was shot against what was planned (M5, from `plan.days_shot` and
/// `plan.scenes_shot`): a row per shoot day of the sub-board (its planned and shot dates,
/// pages planned, shot that day and still to shoot of its scenes, scenes finished that day),
/// then a sheet with a row per scheduled scene (what was shot of it on every day together,
/// what is left, the day that finished it). A CSV export holds the first sheet.
pub fn shot_against_planned(
    s: &Schedule,
    board: BoardId,
    sub: SubBoardId,
) -> Result<Workbook, ExportError> {
    use method_schedule_core::plan::eighths_left;
    let b = s.board(board).ok_or(Error::NoSuchBoard(board))?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or(Error::NoSuchSubBoard(sub))?;
    let ds = days(s, board, sub)?;
    let number_of: std::collections::HashMap<_, u32> =
        ds.days.iter().map(|d| (d.break_id, d.number)).collect();
    let mut day_rows = Vec::new();
    let mut scene_rows = Vec::new();
    let mut listed = std::collections::HashSet::new();
    for d in &ds.days {
        let planned = sb.start.and_then(|x| sb.calendar.date_of_day(x, d.number));
        let shot_date = s.plan.day_shot(d.break_id).and_then(|x| x.date);
        let on_day = s.plan.scenes_shot.iter().filter(|x| x.day == d.break_id);
        let shot = Eighths(on_day.clone().map(|x| x.eighths.0).sum());
        let finished = on_day.filter(|x| x.complete).count() as u32;
        let mut seen = std::collections::HashSet::new();
        let mut left = 0;
        for id in d.counted_sheets() {
            let Some(h) = s.sheet(id) else { continue };
            if seen.insert(id) {
                left += eighths_left(h.pages, s.plan.shot_so_far(id)).0;
            }
            if listed.insert(id) {
                let so_far = s.plan.shot_so_far(id).unwrap_or_default();
                let done_on = s
                    .plan
                    .shot_of(id)
                    .find(|x| x.complete)
                    .and_then(|x| number_of.get(&x.day).copied());
                let mut line = vec![Cell::text(h.scenes.as_str()), int(d.number)];
                line.extend(pages(h.pages));
                line.extend(pages(so_far.eighths));
                line.extend(pages(eighths_left(h.pages, s.plan.shot_so_far(id))));
                line.push(done_on.map_or(Cell::Empty, int));
                scene_rows.push(line);
            }
        }
        let mut line = vec![int(d.number), Cell::date(planned), Cell::date(shot_date)];
        line.extend(pages(d.pages));
        line.extend(pages(shot));
        line.extend(pages(Eighths(left)));
        line.push(int(finished));
        day_rows.push(line);
    }
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![
            Sheet {
                name: "Shot by day",
                columns: names(&[
                    "day",
                    "planned_date",
                    "shot_date",
                    "planned_eighths",
                    "planned_pages",
                    "shot_eighths",
                    "shot_pages",
                    "left_eighths",
                    "left_pages",
                    "scenes_finished",
                ]),
                rows: day_rows,
            },
            Sheet {
                name: "Shot by scene",
                columns: names(&[
                    "scene",
                    "planned_day",
                    "eighths",
                    PAGES,
                    "shot_eighths",
                    "shot_pages",
                    "left_eighths",
                    "left_pages",
                    "finished_day",
                ]),
                rows: scene_rows,
            },
        ],
    })
}

/// Each actor's drift against a schedule lock (D74, BYD-05): a row per actor whose work
/// days changed since lock `name`, with the days before and after and the days added,
/// dropped and moved, in words.
pub fn drift_table(s: &Schedule, name: &str) -> Result<Workbook, ExportError> {
    let d = method_schedule_core::drift::drift(s, name)?;
    let days = |v: &[method_schedule_core::drift::WorkDay]| {
        v.iter().map(|w| w.text(s)).collect::<Vec<_>>().join(", ")
    };
    let rows = d
        .changed()
        .map(|a| {
            let moved: Vec<String> = a
                .moved
                .iter()
                .map(|m| {
                    let scene = s.sheet(m.sheet).map_or("", |h| h.scenes.as_str());
                    format!("{scene}: {} to {}", m.from.text(s), m.to.text(s))
                })
                .collect();
            vec![
                Cell::text(a.board_id.as_deref().unwrap_or("")),
                Cell::text(a.name.as_str()),
                Cell::Int(a.before as u64),
                Cell::Int(a.after as u64),
                Cell::text(days(&a.added)),
                Cell::text(days(&a.dropped)),
                Cell::text(moved.join("; ")),
            ]
        })
        .collect();
    Ok(Workbook {
        title: s.title.clone(),
        sheets: vec![Sheet {
            name: "Drift",
            columns: names(&[
                "board_id",
                "actor",
                "days_before",
                "days_after",
                "added",
                "dropped",
                "moved",
            ]),
            rows,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::xlsx;
    use method_schedule_core::days::{auto_day_breaks, Limit};
    use method_schedule_core::fountain;
    use method_schedule_core::model::{Banner, Minutes, Place};

    fn harbor() -> (Schedule, BoardId, SubBoardId, CategoryId) {
        let mut s = Schedule::new("ZTEST Harbor Light");
        let script = fountain::parse(include_str!(
            "../../../../fixtures/scripts/harbor-light.fountain"
        ));
        let cast = s.category_named("Cast").unwrap();
        s.import_script(&script, cast).unwrap();
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let ids: Vec<_> = s.boards[0].boneyard.iter().map(|x| x.id).collect();
        // All but the last scene on the sub-board; the last stays in the Boneyard.
        s.move_strips(
            b,
            &ids[..ids.len() - 1],
            Place {
                sub_board: Some(sb),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            StripKind::Banner(Banner {
                text: "Move".into(),
                duration: Minutes(30),
                ..Default::default()
            }),
            Place {
                sub_board: Some(sb),
                index: 2,
            },
        )
        .unwrap();
        auto_day_breaks(&mut s, b, sb, Limit::Pages(Eighths(3))).unwrap();
        let tove = s.element_named(cast, "TOVE").unwrap();
        s.set_board_id(tove, Some("1")).unwrap();
        let props = s.add_category("Props");
        let needle = s.add_element(props, "Netting needle").unwrap();
        let first = s.sheets[0].id;
        s.tag(first, needle).unwrap();
        s.boards[0].sub_boards[0].start = "2027-03-01".parse().ok();
        (s, b, sb, cast)
    }

    fn written(wb: &Workbook) -> Vec<Vec<Cell>> {
        let bytes = xlsx::write(wb).unwrap();
        let back = xlsx::read_cells(&bytes).unwrap();
        let header: Vec<Cell> = wb.sheets[0].columns.iter().map(Cell::text).collect();
        assert_eq!(back[0].rows[0], header);
        assert_eq!(&back[0].rows[1..], &wb.sheets[0].rows[..]);
        back[0].rows[1..].to_vec()
    }

    #[test]
    fn the_grid_has_a_column_per_date_and_the_schedule_letters() {
        let (mut s, b, sb, cast) = harbor();
        let wb = dood_grid(&s, b, sb, Some(cast)).unwrap();
        let cols = &wb.sheets[0].columns;
        assert_eq!(cols[3], "2027-03-01");
        assert_eq!(cols[cols.len() - 5], "work");
        let rows = written(&wb);
        let tove = rows.iter().find(|r| r[2] == Cell::text("TOVE")).unwrap();
        assert_eq!(tove[0], Cell::text("Cast"));
        assert_eq!(tove[3], Cell::text("SWF"));
        s.dood_codes.start = "B".into();
        let wb = dood_grid(&s, b, sb, None).unwrap();
        let tove = wb.sheets[0]
            .rows
            .iter()
            .find(|r| r[2] == Cell::text("TOVE"))
            .unwrap();
        assert_eq!(tove[3], Cell::text("BWF"));
        // Every category: Props is there too.
        assert!(wb.sheets[0]
            .rows
            .iter()
            .any(|r| r[0] == Cell::text("Props")));
        // One category's dates are its own rows': a prop's fitting the week before
        // widens the whole grid, not the cast's.
        let needle = s
            .element_named(s.category_named("Props").unwrap(), "Netting needle")
            .unwrap();
        let d: Date = "2027-02-25".parse().unwrap();
        s.boards[0].sub_boards[0]
            .calendar
            .events
            .push(method_schedule_core::model::Event {
                name: String::new(),
                kind: method_schedule_core::model::EventKind::Fitting,
                from: d,
                to: d,
                elements: vec![needle],
                color: None,
            });
        assert_eq!(
            dood_grid(&s, b, sb, None).unwrap().sheets[0].columns[3],
            "2027-02-25"
        );
        let cast_only = dood_grid(&s, b, sb, Some(cast)).unwrap();
        assert_eq!(cast_only.sheets[0].columns[3], "2027-03-01");
        assert_eq!(
            written(&cast_only)[0].len(),
            cast_only.sheets[0].columns.len()
        );
    }

    #[test]
    fn breakdown_by_day_lists_each_element_once_a_day() {
        let (mut s, b, sb, cast) = harbor();
        let first = s.sheets[0].id;
        let kit = s.element_named(cast, "KIT").unwrap();
        s.category_mut(cast).unwrap().daily_quantity = true;
        s.set_quantity(first, kit, 3, true).unwrap();
        let wb = breakdown_day(&s, b, sb).unwrap();
        assert_eq!(wb.sheets[0].columns[5..], ["scenes", "quantity", "partial"]);
        let rows = written(&wb);
        // Day 1: TOVE (ID 1) first, in one of its scenes.
        assert_eq!(
            rows[0][..6],
            [
                Cell::Int(1),
                Cell::Date("2027-03-01".parse().unwrap()),
                Cell::text("Cast"),
                Cell::text("1"),
                Cell::text("TOVE"),
                Cell::Int(1)
            ]
        );
        let kit1 = rows
            .iter()
            .find(|r| r[0] == Cell::Int(1) && r[4] == Cell::text("KIT"))
            .unwrap();
        // In both of the day's scenes: three on the first sheet, one on the other.
        assert_eq!(kit1[5], Cell::Int(2));
        assert_eq!(kit1[6], Cell::Int(4));
        assert_eq!(kit1[7], Cell::Empty);
        let needle = rows
            .iter()
            .find(|r| r[4] == Cell::text("Netting needle"))
            .unwrap();
        assert_eq!(needle[2], Cell::text("Props"));
        assert_eq!(needle[6], Cell::Empty);
        // Partial when a sheet's quantity doesn't count daily.
        s.set_quantity(first, kit, 3, false).unwrap();
        let rows = written(&breakdown_day(&s, b, sb).unwrap());
        let kit1 = rows
            .iter()
            .find(|r| r[0] == Cell::Int(1) && r[4] == Cell::text("KIT"))
            .unwrap();
        assert_eq!(kit1[7], Cell::text("yes"));
    }

    #[test]
    fn daily_totals_add_up_by_formula() {
        let (s, b, sb, _) = harbor();
        let wb = daily_totals(&s, b, sb).unwrap();
        let cols = &wb.sheets[0].columns;
        assert_eq!(cols[..3], ["day", "date", "Cast"]);
        assert_eq!(cols[cols.len() - 1], "partial");
        let rows = written(&wb);
        let last = rows.last().unwrap();
        assert_eq!(last[0], Cell::text("total"));
        let days = rows.len() - 1;
        let Cell::Formula(f) = &last[2] else {
            panic!("{:?}", last[2]);
        };
        assert_eq!(f.expr(), format!("SUM(C2:C{})", days + 1));
        let cast: u64 = rows[..days]
            .iter()
            .map(|r| match r[2] {
                Cell::Int(n) => n,
                _ => 0,
            })
            .sum();
        assert_eq!(f.value(), cast);
        assert!(cast > 0);
        // The grid's total is work plus hold too.
        let grid = dood_grid(&s, b, sb, None).unwrap();
        let w = grid.sheets[0].columns.len() - 5;
        let Cell::Formula(f) = &grid.sheets[0].rows[0][w + 4] else {
            panic!("not a formula");
        };
        assert_eq!(
            f.expr(),
            format!("{}2+{}2", xlsx::column_name(w), xlsx::column_name(w + 1))
        );
    }

    #[test]
    fn element_detail_breakdown_and_board() {
        let (s, b, sb, _) = harbor();
        let rows = written(&element_detail(&s, b, sb).unwrap());
        let kit: Vec<_> = rows.iter().filter(|r| r[2] == Cell::text("KIT")).collect();
        assert!(kit.len() >= 3);
        assert_eq!(kit[0][3], Cell::Int(1));

        let wb = breakdown(&s, b, sb).unwrap();
        let cols = &wb.sheets[0].columns;
        assert_eq!(&cols[cols.len() - 2..], ["Cast", "Props"]);
        let rows = written(&wb);
        assert_eq!(rows[0][rows[0].len() - 1], Cell::text("Netting needle"));
        assert!(matches!(&rows[0][rows[0].len() - 2], Cell::Text(t) if t.contains("1 TOVE")));

        let rows = written(&board(&s, b).unwrap());
        assert!(rows.iter().any(|r| r[3] == Cell::text("day_break")));
        assert!(rows.iter().any(|r| r[3] == Cell::text("banner")));
        let last = rows.last().unwrap();
        assert_eq!(last[0], Cell::text("Boneyard"));
        assert_eq!(last[1], Cell::Empty);
        // Every row is as wide as the header.
        assert!(rows.iter().all(|r| r.len() == 4 + 6 + 3));
    }

    /// Behavior note 23: the red flag table, open ends empty, by first date.
    #[test]
    fn drift_lists_the_actors_whose_days_changed() {
        let mut s = method_schedule_core::sample::field_station();
        let b = s.boards[0].id;
        method_schedule_core::drift::lock(&mut s, b, "Locked", None).unwrap();
        assert!(drift_table(&s, "Locked").unwrap().sheets[0].rows.is_empty());
        // The first day's first scene moved to the end of the sub-board's last day.
        let sb = s.boards[0].sub_boards[0].id;
        let d = days(&s, b, sb).unwrap();
        let first = d.days[0].strips[0];
        let last = s.boards[0].sub_boards[0].strips.len() - 1;
        s.move_strips(
            b,
            &[first],
            Place {
                sub_board: Some(sb),
                index: last,
            },
        )
        .unwrap();
        let wb = drift_table(&s, "Locked").unwrap();
        assert!(!wb.sheets[0].rows.is_empty());
        assert_eq!(wb.sheets[0].columns[1], "actor");
        assert!(drift_table(&s, "Nope").is_err());
        xlsx::write(&wb).unwrap();
    }

    #[test]
    fn shot_against_planned_counts_by_day_and_by_scene() {
        use method_schedule_core::plan::record_shot;
        let s0 = method_schedule_core::sample::field_station();
        let (b, sb) = (s0.boards[0].id, s0.boards[0].sub_boards[0].id);
        let mut s = s0.clone();
        let d = days(&s, b, sb).unwrap();
        let day1 = &d.days[0];
        let sheets: Vec<_> = day1.counted_sheets().collect();
        let first = s.sheet(sheets[0]).unwrap().pages;
        record_shot(&mut s, sheets[0], day1.break_id, first, true).unwrap();
        record_shot(&mut s, sheets[1], day1.break_id, Eighths(1), false).unwrap();
        let wb = shot_against_planned(&s, b, sb).unwrap();
        assert_eq!(wb.sheets[0].rows.len(), d.days.len());
        let row = &wb.sheets[0].rows[0];
        let col = |name: &str| wb.sheets[0].columns.iter().position(|c| c == name).unwrap();
        let n = |c: &Cell| match c {
            Cell::Int(n) => *n as u32,
            _ => panic!("{c:?}"),
        };
        assert_eq!(n(&row[col("shot_eighths")]), first.0 + 1);
        assert_eq!(n(&row[col("scenes_finished")]), 1);
        assert_eq!(
            n(&row[col("left_eighths")]),
            day1.pages.0 - first.0 - 1,
            "the day's pages less what was shot"
        );
        // Day two: nothing shot, all of it left.
        let row2 = &wb.sheets[0].rows[1];
        assert_eq!(n(&row2[col("shot_eighths")]), 0);
        assert_eq!(n(&row2[col("left_eighths")]), d.days[1].pages.0);
        // By scene: the finished one names its day, the part-shot one doesn't.
        let scenes = &wb.sheets[1];
        let fin = scenes
            .columns
            .iter()
            .position(|c| c == "finished_day")
            .unwrap();
        assert_eq!(scenes.rows[0][fin], Cell::Int(1));
        assert_eq!(scenes.rows[1][fin], Cell::Empty);
        // Nothing reported: every day has its pages left, and the workbook still writes.
        let plain = shot_against_planned(&s0, b, sb).unwrap();
        assert_eq!(
            n(&plain.sheets[0].rows[0][col("left_eighths")]),
            day1.pages.0
        );
        xlsx::write(&wb).unwrap();
    }

    #[test]
    fn red_flags_table_lists_every_flag() {
        use method_schedule_core::model::{RedFlag, RedFlagDates, RedFlagId};
        let (mut s, _, _, cast) = harbor();
        let kind = s.add_red_flag_type("Second job").unwrap();
        let e = s.elements.iter().find(|e| e.category == cast).unwrap().id;
        let d = |x: &str| -> Date { x.parse().unwrap() };
        for dates in [
            RedFlagDates::Range {
                from: d("2027-03-04"),
                to: d("2027-03-05"),
            },
            RedFlagDates::NotBefore(d("2027-03-02")),
        ] {
            s.set_red_flag(
                e,
                RedFlag {
                    id: RedFlagId(0),
                    kind,
                    dates,
                    note: "Day job".into(),
                    hard: true,
                },
            )
            .unwrap();
        }
        let wb = red_flags(&s);
        let t = wb.sheet("Red flags").unwrap();
        assert_eq!(t.columns[0], "start");
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][0], Cell::Empty);
        assert_eq!(t.rows[0][1], Cell::Date(d("2027-03-01")));
        assert_eq!(t.rows[1][0], Cell::Date(d("2027-03-04")));
        assert_eq!(t.rows[1][5], Cell::text("Second job"));
        assert_eq!(t.rows[1][8], Cell::text("Day job"));
        xlsx::write(&wb).unwrap();
    }
}
