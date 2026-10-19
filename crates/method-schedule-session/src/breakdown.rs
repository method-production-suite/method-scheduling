//! The Breakdown tab's header field order (decisions.md, entry 81) and the breakdown
//! designer's hidden fields, widths (entry 101) and row lengths (entry 186): kept in the schedule
//! (`Schedule::sheet_fields`, `Schedule::sheet_looks`), one undo step.

use method_schedule_core::model::{SheetLooks, SHEET_FIELDS};
use method_schedule_core::Schedule;

use crate::board::EditResult;

/// Sets the header field order from the keys the page sends (each one of
/// `SHEET_FIELDS`, once); fields left out follow in the default order.
pub fn set_field_order(s: &mut Schedule, keys: &[String]) -> EditResult {
    if keys.len() > SHEET_FIELDS.len() {
        return Err("That is more fields than a sheet has.".into());
    }
    s.set_sheet_field_order(keys)
        .map_err(|_| "Each field can be named once, and only the sheet's own fields.".into())
}

/// The order, the hidden fields, the widths and the row lengths together, one undo step.
/// `hidden`, `widths` and `rows` left out keep what the schedule has; `rows` empty is
/// the usual six, five and the rest.
pub fn set_design(
    s: &mut Schedule,
    keys: &[String],
    hidden: Option<&[String]>,
    widths: Option<&std::collections::BTreeMap<String, String>>,
    rows: Option<&[u8]>,
) -> EditResult {
    set_field_order(s, keys)?;
    if hidden.is_none() && widths.is_none() && rows.is_none() {
        return Ok(());
    }
    if rows.is_some_and(|r| r.len() > SHEET_FIELDS.len()) {
        return Err("That is more rows than a sheet has fields.".into());
    }
    let rows: Vec<u8> = rows.map_or_else(|| s.sheet_looks.rows.clone(), <[u8]>::to_vec);
    if hidden.is_some_and(|h| h.len() > SHEET_FIELDS.len())
        || widths.is_some_and(|w| w.len() > SHEET_FIELDS.len())
    {
        return Err("That is more fields than a sheet has.".into());
    }
    let hidden: Vec<String> = match hidden {
        Some(h) => h.to_vec(),
        None => s.sheet_looks.hidden.iter().map(|k| k.to_string()).collect(),
    };
    let widths: Vec<(String, String)> = match widths {
        Some(w) => w.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        None => s
            .sheet_looks
            .widths
            .iter()
            .map(|(k, w)| (k.to_string(), w.name().to_string()))
            .collect(),
    };
    s.sheet_looks = SheetLooks::read(&hidden, &widths)
        .and_then(|l| l.with_rows(&rows))
        .map_err(|e| {
            format!("{}.", {
                let t = e.to_string();
                let mut c = t.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or(t)
            })
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    #[test]
    fn field_order_is_one_step_and_checked() {
        let mut s = Session::new();
        let keys = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        s.edit(|x| set_field_order(x, &keys(&["synopsis", "scenes"])))
            .unwrap();
        assert_eq!(
            &s.schedule().sheet_field_order()[..3],
            ["synopsis", "scenes", "number"]
        );
        assert!(s.can_undo());
        for bad in [vec!["scenes", "scenes"], vec!["budget"]] {
            assert!(s.edit(|x| set_field_order(x, &keys(&bad))).is_err());
        }
        let all: Vec<&str> = SHEET_FIELDS.to_vec();
        s.edit(|x| set_field_order(x, &keys(&all))).unwrap();
        assert!(
            s.schedule().sheet_fields.is_empty(),
            "the default stores nothing"
        );
    }

    #[test]
    fn hidden_fields_and_widths_save_with_the_order() {
        let mut s = Session::new();
        let keys = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let mut w = std::collections::BTreeMap::new();
        w.insert("set".to_string(), "narrow".to_string());
        s.edit(|x| set_design(x, &keys(&["set"]), Some(&keys(&["unit"])), Some(&w), None))
            .unwrap();
        assert_eq!(s.schedule().sheet_looks.hidden, ["unit"]);
        assert_eq!(s.schedule().sheet_field_order()[0], "set");
        // Left out, they stay; an order alone keeps them.
        s.edit(|x| set_design(x, &[], None, None, None)).unwrap();
        assert_eq!(s.schedule().sheet_looks.hidden, ["unit"]);
        w.insert("unit".to_string(), "huge".to_string());
        let err = s
            .edit(|x| set_design(x, &[], None, Some(&w), None))
            .unwrap_err();
        assert!(err.ends_with('.'), "{err}");
        // Rows of the user's own lengths (decision 186): kept with the rest, one step.
        s.edit(|x| set_design(x, &[], None, None, Some(&[4, 4, 4])))
            .unwrap();
        assert_eq!(s.schedule().sheet_looks.rows, [4, 4, 4]);
        assert_eq!(s.schedule().sheet_looks.hidden, ["unit"]);
        for bad in [&[4, 0][..], &[10, 10]] {
            let err = s
                .edit(|x| set_design(x, &[], None, None, Some(bad)))
                .unwrap_err();
            assert!(err.starts_with('A') || err.starts_with('T'), "{err}");
            assert!(err.ends_with('.'), "{err}");
        }
        assert_eq!(s.schedule().sheet_looks.rows, [4, 4, 4]);
        assert!(s.undo());
        assert!(s.schedule().sheet_looks.rows.is_empty());
        let v = crate::view::sheet_view_in(
            s.schedule(),
            s.schedule().boards[0].id.0,
            None,
            None,
            method_schedule_core::model::SheetOrder::Board,
        );
        assert!(v.is_ok());
    }

    #[test]
    fn the_sheet_steps_by_the_order_asked_for() {
        use method_schedule_core::model::{Sheet, SheetOrder};
        let mut s = Session::new();
        let mut ids = Vec::new();
        s.edit(|x| {
            for (number, scenes) in [("3", "20"), ("1", "10"), ("", "2")] {
                ids.push(
                    x.add_sheet(Sheet {
                        number: number.into(),
                        scenes: scenes.into(),
                        shoot_time: Some(method_schedule_core::model::Minutes(90)),
                        ..Sheet::default()
                    })
                    .0,
                );
            }
            Ok(())
        })
        .unwrap();
        let b = s.schedule().boards[0].id.0;
        let by = |order, sheet| {
            crate::view::sheet_view_in(s.schedule(), b, Some(sheet), None, order)
                .unwrap()
                .unwrap()
        };
        let d = by(SheetOrder::Number, ids[0]);
        assert_eq!((d.index, d.prev, d.next), (1, Some(ids[1]), Some(ids[2])));
        assert_eq!(
            (d.first, d.last, d.order),
            (Some(ids[1]), Some(ids[2]), "number")
        );
        assert_eq!(d.number, "3");
        assert_eq!(d.shoot_time_decimal, "1.50");
        assert_eq!(d.field_order[0], "number");
        let d = by(SheetOrder::Scene, ids[0]);
        assert_eq!((d.index, d.prev, d.next), (2, Some(ids[1]), None));
        let d = by(SheetOrder::Board, ids[0]);
        assert_eq!((d.index, d.prev, d.order), (0, None, "board"));
    }
}
