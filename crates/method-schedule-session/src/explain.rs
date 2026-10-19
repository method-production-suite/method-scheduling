//! "Explain this count" on the Day out of days tab (docs/MERGE.md, "Explain this count";
//! decisions.md, entries 25 and 26). The engine works it out (`method_schedule_core::explain`) for
//! one element on the sub-board the grid shows; this turns
//! it into what the page's panel draws (ui/js/dood.js). Every string is the engine's
//! sentence, shown with `textContent`.

use serde::Serialize;

use method_schedule_core::explain::explain;
use method_schedule_core::model::ElementId;
use method_schedule_core::Schedule;

/// The explain panel.
#[derive(Clone, Debug, Serialize)]
pub struct ExplainDto {
    pub element: u32,
    pub name: String,
    pub category: String,
    pub sub_board: String,
    pub work: u32,
    pub hold: u32,
    pub total: u32,
    /// The totals, what work is made of, the rules and the drop, one sentence each.
    pub summary: Vec<String>,
    pub gaps: Vec<GapDto>,
    pub days: Vec<DayDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GapDto {
    /// The work or event days either side, `YYYY-MM-DD`.
    pub after: String,
    pub before: String,
    pub days: u32,
    /// `dropped`, `too_short`, `another_gap_dropped`, `drops_off`, `holds_off`.
    pub outcome: &'static str,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DayDto {
    pub date: String,
    /// The grid's code, empty where it shows nothing.
    pub code: String,
    /// `work`, `hold` or `nowhere`.
    pub counted: &'static str,
    pub text: String,
}

/// Why `element`'s day out of days on sub-board `sub` is what it is.
pub fn explain_count(s: &Schedule, sub: u32, element: u32) -> Result<ExplainDto, String> {
    let (b, sb) = crate::view::sub_ref(s, sub)?;
    let el = s
        .element(ElementId(element))
        .ok_or("That element is no longer in the schedule.")?;
    if sb.start.is_none() {
        return Err(format!(
            "Set a start date for {} to see the day out of days.",
            sb.name
        ));
    }
    let x = explain(s, b.id, sb.id, el.id).map_err(|e| e.to_string())?;
    Ok(ExplainDto {
        element,
        name: el.name.clone(),
        category: s
            .category(el.category)
            .map(|c| c.name.clone())
            .unwrap_or_default(),
        sub_board: sb.name.clone(),
        work: x.work,
        hold: x.hold,
        total: x.total,
        summary: x.summary,
        gaps: x
            .gaps
            .iter()
            .map(|g| GapDto {
                after: g.gap.after.to_string(),
                before: g.gap.before.to_string(),
                days: g.gap.days,
                outcome: g.outcome.key(),
                text: g.text.clone(),
            })
            .collect(),
        days: x
            .days
            .iter()
            .map(|d| DayDto {
                date: d.date.to_string(),
                code: d.code.clone(),
                counted: d.counted.key(),
                text: d.text.clone(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::dood_grid;

    #[test]
    fn the_panel_matches_the_grid_row() {
        let s = method_schedule_core::sample::sample(60);
        let grid = dood_grid(&s, crate::view::first_sub(&s), None).unwrap();
        assert!(!grid.rows.is_empty());
        for row in &grid.rows {
            let x = explain_count(&s, crate::view::first_sub(&s), row.element).unwrap();
            assert_eq!((x.work, x.hold, x.total), (row.work, row.hold, row.total));
            assert_eq!(x.name, row.name);
            assert_eq!(x.category, "Cast");
            assert!(x.summary[0].contains(&format!("total {}", row.total)));
            // Every date of the explanation is a column of the grid, with the same code.
            for d in &x.days {
                let i = grid
                    .dates
                    .iter()
                    .position(|g| g.date == d.date)
                    .unwrap_or_else(|| panic!("{} is not a column", d.date));
                assert_eq!(row.cells[i], d.code, "{}", d.date);
            }
        }
        // Someone with holds and drops on and a long gap: the gaps say so.
        let gaps: usize = grid
            .rows
            .iter()
            .map(|r| {
                explain_count(&s, crate::view::first_sub(&s), r.element)
                    .unwrap()
                    .gaps
                    .len()
            })
            .sum();
        assert!(gaps > 0, "the sample has gaps");
    }

    #[test]
    fn refusals_are_plain_sentences() {
        let s = method_schedule_core::sample::sample(10);
        let err = explain_count(&s, crate::view::first_sub(&s), 999_999).unwrap_err();
        assert_eq!(err, "That element is no longer in the schedule.");
        let mut t = s.clone();
        t.boards[0].sub_boards[0].start = None;
        let any = s.elements[0].id.0;
        assert!(explain_count(&t, crate::view::first_sub(&t), any)
            .unwrap_err()
            .starts_with("Set a start date for "));
    }
}
