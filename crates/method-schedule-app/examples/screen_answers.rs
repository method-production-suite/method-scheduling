//! The engine's real answers for the README and site pictures (scripts/capture-screens.mjs,
//! decision 215). Opens Field Station (fixtures/field-station.mtsc) as the app would and
//! writes, as JSON, exactly what the page gets for each call a picture makes: the board
//! view, the day out of days grid, and the view after importing the Field Station script
//! into a new schedule. The capture page answers those calls from this file and refuses
//! every other call about the schedule, so a picture can only show what the engine said.
//!
//!   cargo run -q -p method-schedule-app --example screen_answers -- OUT.json

use std::path::Path;

use method_schedule_app::session::{import_notice, Session, SessionFiles};
use method_schedule_app::view::{dood_grid_with, view, GridOpts};
use method_schedule_core::sample::FIELD_STATION_SCRIPT;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: screen_answers OUT.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("fixtures/field-station.mtsc");
    let session = Session::open(&file).expect("fixtures/field-station.mtsc opens");
    let board = view(&session, None).expect("the board view");
    let sub = session.schedule().boards[0].sub_boards[0].id.0;
    let dood = dood_grid_with(session.schedule(), sub, None, &GridOpts::default())
        .expect("the day out of days");

    // Import Script into a new schedule, as File > New then File > Import Script does.
    let mut fresh = Session::default();
    let script = method_schedule_core::fountain::parse(FIELD_STATION_SCRIPT);
    let added = fresh.import_script(&script).expect("the script imports");
    let imported = view(&fresh, Some(import_notice(added))).expect("the imported view");

    let answers = serde_json::json!({
        "board": board,
        "dood_grid": dood,
        "imported": imported,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&answers).expect("JSON"))
        .expect("write the answers");
    eprintln!("wrote {out}");
}
