//! Invented schedules: a large synthetic one for performance work, seeded so the same size
//! always gives the same schedule ([`sample`]), and the small hand-made production the app
//! ships as Help > Open Sample ([`demo`], decisions.md entry 198), and Field Station
//! ([`field_station`], entry 215), the suite's shared invented production that the README
//! and site pictures show and Method Budget budgets.

use crate::days::{auto_day_breaks, Limit};
use crate::model::{
    Banner, BannerColor, BannerStyle, DoodRules, Event, EventKind, Minutes, Place, RedFlag,
    RedFlagDates, RedFlagId, Rgb, Sheet, StripColorRule, StripColors, StripKind,
};
use crate::plan::GeoLocation;
use crate::solar::DaylightSaving;
use crate::testgen::Rng;
use crate::{Date, Eighths, Schedule};

/// How long [`reference_work`] takes on the owner's Mac (an Apple silicon MacBook Pro),
/// release build, the median of 15 runs: 410 to 475 µs measured in the budget runs, the
/// upper end kept so the scaling errs strict (600 to 695 µs while many other builds ran).
/// The yardstick the budgets are calibrated by (decisions.md, entry 191).
pub const REFERENCE_WORK_MICROS: u64 = 450;

/// A fixed piece of work much like what the budgets time, but none of the app's own code:
/// short strings made, sorted, hashed and copied, and integer arithmetic. The same on
/// every run, so its time tells how fast this machine is against the owner's Mac.
/// Returns a checksum so the compiler keeps the work.
pub fn reference_work() -> u64 {
    const N: u64 = 4000;
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut words: Vec<String> = Vec::with_capacity(N as usize);
    for i in 0..N {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        words.push(format!("{:x}-{i}", x % 100_000));
    }
    words.sort_unstable();
    let mut at = std::collections::HashMap::with_capacity(words.len());
    for (i, w) in words.iter().enumerate() {
        at.insert(w.as_str(), i as u64);
    }
    let copy = words.clone();
    copy.iter().fold(0u64, |sum, w| {
        sum.wrapping_mul(31)
            .wrapping_add(at[w.as_str()] ^ w.len() as u64)
    })
}

/// What every budget is multiplied by on this machine, in percent: the time of
/// [`reference_work`] here against [`REFERENCE_WORK_MICROS`], never below 100 (a faster
/// machine keeps the budgets as they are) and at most 400 (a machine more than four times
/// slower fails rather than hides a slip). Prints what it found (decision 191).
pub fn budget_scale_percent() -> u32 {
    let mut times: Vec<std::time::Duration> = (0..16)
        .map(|_| {
            let t = std::time::Instant::now();
            std::hint::black_box(reference_work());
            t.elapsed()
        })
        .skip(1)
        .collect();
    times.sort();
    let took = times[times.len() / 2];
    let ratio = took.as_micros() as u64 * 100 / REFERENCE_WORK_MICROS;
    let scale = ratio.clamp(100, 400) as u32;
    println!(
        "budget calibration: reference work {took:.2?} here, {REFERENCE_WORK_MICROS}µs on the \
         owner's Mac: {}.{:02}x, budgets scaled by {}.{:02}x",
        ratio / 100,
        ratio % 100,
        scale / 100,
        scale % 100
    );
    scale
}

/// A schedule with `sheets` scenes on its first sub-board, broken into days of about five
/// pages, starting Monday 2027-03-01. Cast of 60 with holds on and a 10-day drop gap,
/// plus props and vehicles, a few holidays and travel days.
pub fn sample(sheets: usize) -> Schedule {
    let mut s = Schedule::new("Sample: The Long Haul");
    let mut rng = Rng::new(0x05a4_d91e);
    let cast = s.cast().expect("new schedules have a cast category");
    s.category_mut(cast).expect("cast").auto_board_id = true;
    let props = s.add_category("Props");
    let vehicles = s.add_category("Vehicles");
    let mut people = Vec::new();
    for i in 0..60 {
        let id = s
            .add_element(cast, &format!("PLAYER {i:02}"))
            .expect("unique");
        s.element_mut(id).expect("just added").rules = DoodRules {
            holds: true,
            drops: true,
            min_drop_gap: 10,
        };
        people.push(id);
    }
    let things: Vec<_> = (0..120)
        .map(|i| {
            s.add_element(props, &format!("Prop {i:03}"))
                .expect("unique")
        })
        .collect();
    let cars: Vec<_> = (0..20)
        .map(|i| {
            s.add_element(vehicles, &format!("Car {i:02}"))
                .expect("unique")
        })
        .collect();
    let tods = ["DAY", "NIGHT", "DUSK", "DAWN"];
    for n in 1..=sheets {
        let mut elements = Vec::new();
        // Leads appear often, day players rarely.
        for (i, p) in people.iter().enumerate() {
            let odds = if i < 6 {
                3
            } else if i < 20 {
                12
            } else {
                60
            };
            if rng.below(odds) == 0 {
                elements.push(*p);
            }
        }
        for _ in 0..rng.below(4) {
            let t = things[rng.below(things.len() as u64) as usize];
            if !elements.contains(&t) {
                elements.push(t);
            }
        }
        if rng.below(5) == 0 {
            elements.push(cars[rng.below(cars.len() as u64) as usize]);
        }
        s.add_sheet(Sheet {
            scenes: n.to_string(),
            int_ext: if rng.below(3) == 0 { "EXT" } else { "INT" }.to_string(),
            set: format!("SET {:03}", rng.below(150)),
            time_of_day: tods[rng.below(4) as usize].to_string(),
            pages: Eighths(1 + rng.below(16) as u32),
            shoot_time: Some(Minutes(30 + 15 * rng.below(8) as u32)),
            elements,
            ..Sheet::default()
        });
    }
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
    .expect("moves onto Main");
    auto_day_breaks(&mut s, b, sb, Limit::Pages(Eighths(40))).expect("breaks");
    let start = Date::from_ymd(2027, 3, 1).expect("valid");
    let sub = &mut s.boards[0].sub_boards[0];
    sub.start = Some(start);
    for d in [24, 59, 120] {
        sub.calendar.holidays.push(start.add_days(d));
    }
    sub.calendar.moves.push(start.add_days(45));
    s
}

/// One line of [`demo`]'s boards, in shooting order.
enum Row {
    /// Scene, I/E, set, time of day, location, eighths, cast (by place in [`DEMO_CAST`]),
    /// other elements (by name, in [`DEMO_THINGS`]) and the synopsis.
    Scene(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u32,
        &'static [usize],
        &'static [&'static str],
        &'static str,
    ),
    DayBreak,
    /// Text, minutes, color, two lines high.
    Banner(&'static str, u32, BannerColor, bool),
}

/// The sample's cast, in board ID order. Invented people.
const DEMO_CAST: [&str; 8] = [
    "MAREN COLE",
    "OWEN PRICE",
    "IDA FENN",
    "THEO MARSH",
    "RUTH ALDER",
    "JONAH VALE",
    "DR. LISS",
    "FERRY CAPTAIN",
];

/// The sample's other elements: category, name.
const DEMO_THINGS: [(&str, &str); 9] = [
    ("Hand props", "LANTERN"),
    ("Hand props", "RADIO"),
    ("Hand props", "MAP"),
    ("Hand props", "KEYS"),
    ("Picture vehicles", "PICKUP TRUCK"),
    ("Picture vehicles", "FERRY"),
    ("Animals", "BISCUIT"),
    ("Background", "DINER REGULARS"),
    ("Background", "DOCK WORKERS"),
];

const MARSH: &str = "Marsh House";
const HARBOR: &str = "Harbor";
const STAGE: &str = "Stage 2";

/// The main unit, from the first shoot day: eleven days, a holiday among them.
#[rustfmt::skip]
const DEMO_MAIN: &[Row] = {
    use Row::{Banner as Bn, DayBreak as Br, Scene as Sc};
    &[
        Bn("Block one: Marsh House", 0, BannerColor::Yellow, true),
        Sc("1", "EXT", "MARSH HOUSE - PORCH", "DAWN", MARSH, 6, &[0], &["LANTERN", "BISCUIT"],
            "Maren watches the tide come in over the marsh."),
        Sc("2", "INT", "MARSH HOUSE - KITCHEN", "DAY", MARSH, 12, &[0, 2], &["RADIO"],
            "Ida brings the mail and news that the road will close."),
        Sc("13", "EXT", "MARSH HOUSE - PORCH", "DAY", MARSH, 5, &[1, 2], &["BISCUIT"],
            "Owen and Ida argue about the tide tables."),
        Sc("14", "EXT", "MARSH HOUSE - PORCH", "DUSK", MARSH, 4, &[0], &["LANTERN"],
            "Maren lights the lantern for the first time in years."),
        Br,
        Sc("5", "INT", "MARSH HOUSE - KITCHEN", "NIGHT", MARSH, 9, &[0, 1], &["RADIO", "LANTERN"],
            "Owen turns up out of the storm, soaked."),
        Sc("6", "INT", "MARSH HOUSE - KITCHEN", "NIGHT", MARSH, 14, &[0, 1], &["MAP"],
            "Owen spreads the old map on the table."),
        Br,
        Sc("12", "INT", "MARSH HOUSE - KITCHEN", "DAY", MARSH, 10, &[0, 1, 2], &["MAP"],
            "Ida knows the handwriting on the map."),
        Sc("27", "INT", "MARSH HOUSE - BEDROOM", "NIGHT", MARSH, 8, &[0], &["KEYS"],
            "Maren finds her father's keys under a floorboard."),
        Sc("28", "INT", "MARSH HOUSE - BEDROOM", "DAY", MARSH, 6, &[0, 4], &[],
            "Ruth helps Maren pack."),
        Br,
        Sc("40", "EXT", "MARSH HOUSE - PORCH", "DAY", MARSH, 7, &[0, 1, 2, 4], &["BISCUIT"],
            "Everyone says goodbye on the porch."),
        Bn("Company move to the harbor", 45, BannerColor::Teal, false),
        Sc("3", "EXT", "HARBOR PIER", "DAY", HARBOR, 8, &[0, 3], &["PICKUP TRUCK", "DOCK WORKERS"],
            "Theo unloads crates and won't meet Maren's eye."),
        Sc("4", "EXT", "HARBOR PIER", "DAY", HARBOR, 5, &[3, 7], &["DOCK WORKERS"],
            "The ferry captain warns Theo off."),
        Br,
        Sc("9", "EXT", "FERRY DECK", "DAY", HARBOR, 11, &[0, 7], &["FERRY"],
            "Maren crosses to the island."),
        Sc("10", "EXT", "FERRY DECK", "DUSK", HARBOR, 6, &[0, 1, 7], &["FERRY", "MAP"],
            "Owen finds Maren on the deck."),
        Br,
        Sc("17", "INT", "HARBOR DINER", "DAY", HARBOR, 13, &[0, 3, 5], &["DINER REGULARS"],
            "Jonah offers Maren a job she doesn't want."),
        Sc("18", "INT", "HARBOR DINER", "NIGHT", HARBOR, 9, &[3, 5], &["DINER REGULARS"],
            "Theo and Jonah settle an old debt."),
        Br,
        Sc("21", "EXT", "BOATYARD", "DAY", HARBOR, 10, &[1, 3], &["PICKUP TRUCK"],
            "Owen asks Theo about the night of the wreck."),
        Sc("22", "EXT", "BOATYARD", "NIGHT", HARBOR, 7, &[1, 3, 5], &["LANTERN"],
            "Jonah catches them in the boatyard."),
        Br,
        Sc("30", "EXT", "HARBOR PIER", "NIGHT", HARBOR, 12, &[0, 1, 3, 5], &["LANTERN", "KEYS"],
            "The keys open the harbor master's shed."),
        Sc("31", "INT", "HARBOR MASTER'S SHED", "NIGHT", HARBOR, 9, &[0, 1], &["KEYS", "RADIO"],
            "The radio log says who called the ferry."),
        Br,
        Sc("7", "INT", "CLINIC", "DAY", STAGE, 8, &[0, 6], &[],
            "Dr. Liss tells Maren what her father knew."),
        Sc("8", "INT", "CLINIC", "DAY", STAGE, 4, &[6], &[],
            "Dr. Liss makes a phone call."),
        Sc("24", "INT", "CLINIC", "NIGHT", STAGE, 10, &[2, 6], &["RADIO"],
            "Ida confronts Dr. Liss."),
        Br,
        Sc("33", "INT", "OWEN'S TRUCK", "NIGHT", STAGE, 6, &[0, 1], &["PICKUP TRUCK"],
            "Driving the marsh road in the dark."),
        Sc("34", "INT", "OWEN'S TRUCK", "DAY", STAGE, 5, &[0, 1], &["PICKUP TRUCK"],
            "Owen admits why he came back."),
        Sc("36", "INT", "JONAH'S OFFICE", "DAY", STAGE, 11, &[3, 5], &[],
            "Jonah's account books don't add up."),
        Br,
        Sc("37", "INT", "JONAH'S OFFICE", "NIGHT", STAGE, 9, &[0, 5], &["MAP"],
            "Maren lays the map on Jonah's desk."),
        Sc("38", "INT", "RUTH'S HOUSE", "DAY", STAGE, 8, &[0, 4], &[],
            "Ruth tells the story her sister never would."),
        Sc("39", "INT", "RUTH'S HOUSE", "NIGHT", STAGE, 6, &[0, 2, 4], &[],
            "Ida and Ruth make peace."),
        Br,
    ]
};

/// The second unit: two days of road, marsh and water.
#[rustfmt::skip]
const DEMO_SECOND: &[Row] = {
    use Row::{DayBreak as Br, Scene as Sc};
    &[
        Sc("11", "EXT", "SALTMARSH ROAD", "DAY", MARSH, 3, &[], &["PICKUP TRUCK"],
            "The truck on the causeway at high tide."),
        Sc("16", "EXT", "SALTMARSH ROAD", "DUSK", MARSH, 3, &[], &["PICKUP TRUCK"],
            "Headlights along the road."),
        Sc("32", "EXT", "SALTMARSH ROAD", "NIGHT", MARSH, 4, &[], &["PICKUP TRUCK"],
            "The road under water."),
        Br,
        Sc("15", "EXT", "MARSH", "DAWN", MARSH, 2, &[], &["BISCUIT"],
            "Biscuit runs the tide line."),
        Sc("35", "EXT", "MARSH", "DAY", MARSH, 2, &[], &[],
            "Wind through the grass."),
        Sc("25", "EXT", "OPEN WATER", "DAY", HARBOR, 4, &[], &["FERRY"],
            "The ferry from above."),
        Br,
    ]
};

/// Scenes left in the Boneyard.
#[rustfmt::skip]
const DEMO_BONEYARD: &[Row] = &[
    Row::Scene("19", "INT", "HARBOR DINER", "DAY", HARBOR, 3, &[5], &["DINER REGULARS"],
        "Jonah alone after closing."),
    Row::Scene("23", "EXT", "BOATYARD", "DAWN", HARBOR, 4, &[3], &[],
        "Theo watches the wreck towed in."),
];

/// The sample production the app ships (Help > Open Sample) and `method-schedule sample
/// --demo` writes: an invented feature, *Saltmarsh Road*, small enough to read in a
/// minute and showing a little of everything. Eight cast with holds and drops, props,
/// vehicles, an animal and background; two sub-boards (Main from Monday 2027-04-05 for
/// eleven shoot days, Second Unit for two) and scenes left in the Boneyard; banners, a
/// holiday, a fitting, a rehearsal and travel days; a hard red flag that clashes; two
/// locations placed on the map, so sunrise and sunset show; strip colors and production
/// information. Everything in it is made up (decisions.md, entry 198).
pub fn demo() -> Schedule {
    let mut s = Schedule::with_default_categories("Saltmarsh Road (sample)");
    s.production.company = "Tidewater Pictures".into();
    s.production.director = "Wren Halloway".into();
    s.production.producer = "Ezra Quill".into();
    s.production.script_date = "2027-02-12".into();
    let cast = s.cast().expect("new schedules have a cast category");
    s.category_mut(cast).expect("cast").auto_board_id = true;
    let mut people = Vec::new();
    for name in DEMO_CAST {
        let id = s.add_element(cast, name).expect("unique");
        s.element_mut(id).expect("just added").rules = DoodRules {
            holds: true,
            drops: true,
            min_drop_gap: 7,
        };
        people.push(id);
    }
    let mut things = Vec::new();
    for (category, name) in DEMO_THINGS {
        let c = s.category_named(category).expect("a default category");
        things.push((name, s.add_element(c, name).expect("unique")));
    }
    let b = s.boards[0].id;
    let main = s.boards[0].sub_boards[0].id;
    let start = Date::from_ymd(2027, 4, 5).expect("valid");
    s.rename_board(b, "Shooting Order").expect("a fine name");
    let second = s
        .add_sub_board(b, "Second Unit", Some(start.add_days(7)))
        .expect("a new name");
    s.set_start(b, main, Some(start)).expect("main");
    for (sub, rows) in [
        (Some(main), DEMO_MAIN),
        (Some(second), DEMO_SECOND),
        (None, DEMO_BONEYARD),
    ] {
        for row in rows {
            let at = |s: &Schedule| Place {
                sub_board: sub,
                index: match sub {
                    Some(sb) => s.boards[0]
                        .sub_boards
                        .iter()
                        .find(|x| x.id == sb)
                        .map_or(0, |x| x.strips.len()),
                    None => s.boards[0].boneyard.len(),
                },
            };
            let kind = match row {
                Row::Scene(scene, ie, set, tod, location, eighths, who, what, synopsis) => {
                    let mut elements: Vec<_> = who.iter().map(|&i| people[i]).collect();
                    for name in *what {
                        let (_, id) = things.iter().find(|(n, _)| n == name).expect("listed");
                        elements.push(*id);
                    }
                    let id = s.add_sheet(Sheet {
                        number: scene.to_string(),
                        scenes: scene.to_string(),
                        int_ext: ie.to_string(),
                        set: set.to_string(),
                        time_of_day: tod.to_string(),
                        location: location.to_string(),
                        pages: Eighths(*eighths),
                        shoot_time: Some(Minutes(45 * eighths.div_ceil(4))),
                        synopsis: synopsis.to_string(),
                        elements,
                        ..Sheet::default()
                    });
                    for (name, count) in [("DINER REGULARS", 12), ("DOCK WORKERS", 6)] {
                        let (_, el) = things.iter().find(|(n, _)| *n == name).expect("listed");
                        if what.contains(&name) {
                            s.set_quantity(id, *el, count, true).expect("on the sheet");
                        }
                    }
                    if sub.is_none() {
                        continue; // added sheets start in the Boneyard
                    }
                    let strip = s.boards[0]
                        .boneyard
                        .iter()
                        .find(|x| x.kind == StripKind::Sheet(id))
                        .expect("a new sheet's strip")
                        .id;
                    let to = at(&s);
                    s.move_strips(b, &[strip], to).expect("moves");
                    continue;
                }
                Row::DayBreak => StripKind::DayBreak,
                Row::Banner(text, minutes, color, tall) => StripKind::Banner(Banner {
                    text: text.to_string(),
                    duration: Minutes(*minutes),
                    style: BannerStyle {
                        bold: true,
                        color: Some(*color),
                        tall: *tall,
                        ..BannerStyle::default()
                    },
                }),
            };
            let to = at(&s);
            s.insert_strip(b, kind, to).expect("inserts");
        }
    }
    // A holiday on the Friday of the second week, on both units' calendars.
    let holiday = start.add_days(11);
    for sub in &mut s.boards[0].sub_boards {
        sub.calendar.holidays.push(holiday);
    }
    let person = |i: usize| vec![people[i]];
    let events = [
        (
            EventKind::Fitting,
            "Wardrobe fitting",
            -3,
            -3,
            vec![people[0], people[1]],
            Some(4),
        ),
        (EventKind::Travel, "Travel in", 2, 2, person(7), None),
        (EventKind::Travel, "Travel home", 5, 5, person(7), None),
        (
            EventKind::Rehearsal,
            "Diner rehearsal",
            4,
            4,
            vec![people[3], people[5]],
            Some(1),
        ),
    ];
    for (kind, name, from, to, elements, color) in events {
        let event = Event {
            name: name.into(),
            kind,
            from: start.add_days(from),
            to: start.add_days(to),
            elements,
            color,
        };
        s.set_event(b, main, None, event).expect("a valid event");
    }
    let unavailable = s
        .red_flag_type_named("Unavailable")
        .expect("a built-in type");
    s.set_red_flag(
        people[6],
        RedFlag {
            id: RedFlagId(0),
            kind: unavailable,
            dates: RedFlagDates::Range {
                from: start.add_days(10),
                to: holiday,
            },
            note: "Teaching that week".into(),
            hard: true,
        },
    )
    .expect("a valid flag");
    for (name, latitude, longitude) in [
        (MARSH, 43_851_000, -69_947_000),
        (HARBOR, 43_657_000, -70_248_000),
    ] {
        s.plan
            .set_location(GeoLocation {
                name: name.into(),
                latitude,
                longitude,
                utc_offset: -5 * 60,
                daylight_saving: Some(DaylightSaving::UsCanada),
            })
            .expect("a valid place");
    }
    let rule = |ie: &str, tod: &str, hex: &str| StripColorRule {
        int_ext: ie.into(),
        time_of_day: tod.into(),
        color: Rgb::parse(hex).expect("a color"),
        text: None,
    };
    s.set_strip_colors(StripColors {
        rules: vec![
            rule("", "DAWN", "#f6d6e4"),
            rule("", "DUSK", "#f4c7a1"),
            rule("INT", "DAY", "#ffffff"),
            rule("EXT", "DAY", "#fff2a8"),
            rule("INT", "NIGHT", "#b7cff7"),
            rule("EXT", "NIGHT", "#a6d8b0"),
        ],
        ..StripColors::default()
    })
    .expect("valid colors");
    s
}

/// The Field Station script (fixtures/scripts/field-station.fountain): the suite's shared
/// invented feature, this repository's own copy with a neutral title page (decision 215).
pub const FIELD_STATION_SCRIPT: &str =
    include_str!("../../../fixtures/scripts/field-station.fountain");

/// Field Station's breakdown beyond the cast: (category, element, the scene numbers it is
/// in), one or more in every category. Read off the script; invented, like everything in
/// it.
const FIELD_STATION_THINGS: &[(&str, &str, &[&str])] = &[
    (
        "Background",
        "BASE STAFF",
        &["13", "36", "52", "58", "66", "73"],
    ),
    ("Background", "RELIEF TEAM", &["80", "83"]),
    ("Background", "SECOND BOAT CREW", &["79"]),
    ("Background", "NEW TOP KEEPER", &["83", "90"]),
    ("Stand-ins", "STAND-IN FOR MARA", &["4", "15", "67"]),
    ("Stunts", "MAST CLIMB", &["4", "15", "67"]),
    ("Stunts", "QUAD RIDE", &["8", "18", "24"]),
    (
        "Hand props",
        "PAPER MAP",
        &[
            "1", "3", "4", "6", "7", "8", "10", "11", "13", "19", "25", "31", "36", "38", "61",
            "71", "84", "87",
        ],
    ),
    (
        "Hand props",
        "FIELD PACKS",
        &[
            "1", "2", "18", "19", "28", "30", "32", "34", "38", "41", "48", "55", "60", "62", "80",
            "83", "87",
        ],
    ),
    (
        "Hand props",
        "HAND RADIO",
        &[
            "3", "7", "9", "11", "13", "16", "31", "35", "37", "48", "63", "71", "74", "78", "89",
        ],
    ),
    (
        "Hand props",
        "TORCH",
        &["6", "19", "26", "43", "49", "63", "67", "69", "70", "71"],
    ),
    (
        "Hand props",
        "HEADLAMPS",
        &["8", "9", "24", "27", "28", "32", "49"],
    ),
    (
        "Hand props",
        "NOTEBOOK",
        &["9", "17", "18", "43", "51", "54", "80", "81"],
    ),
    (
        "Hand props",
        "PENCIL",
        &[
            "7", "17", "25", "26", "37", "44", "51", "52", "54", "57", "66", "68", "81", "85",
        ],
    ),
    (
        "Hand props",
        "KEY RING",
        &[
            "9", "16", "17", "19", "22", "24", "27", "29", "31", "34", "36", "43", "48", "63",
            "64", "68", "78", "89",
        ],
    ),
    (
        "Hand props",
        "CHALK",
        &[
            "13", "18", "24", "28", "32", "33", "34", "47", "48", "52", "60", "75", "88",
        ],
    ),
    (
        "Hand props",
        "TEA MUGS",
        &[
            "9", "12", "13", "17", "34", "41", "43", "44", "53", "59", "65", "72",
        ],
    ),
    ("Hand props", "FLARE GUN", &["48", "53", "61"]),
    ("Hand props", "FLAT STONE", &["6", "32", "34", "41", "80"]),
    (
        "Set dressing",
        "STATION RADIO SET",
        &[
            "2", "3", "5", "7", "9", "10", "11", "13", "16", "19", "31", "36", "37", "41", "52",
            "63", "71", "72", "81", "84", "86", "90",
        ],
    ),
    (
        "Set dressing",
        "GENERATOR",
        &[
            "2", "8", "10", "24", "27", "40", "41", "43", "51", "63", "64", "70", "72", "82", "83",
            "90",
        ],
    ),
    (
        "Set dressing",
        "FUEL DRUM",
        &["2", "8", "12", "18", "35", "42", "60", "64", "67", "70"],
    ),
    (
        "Set dressing",
        "TARP",
        &[
            "2", "3", "5", "8", "10", "11", "31", "41", "63", "65", "72", "82", "83", "90",
        ],
    ),
    (
        "Set dressing",
        "VALLEY MAP WITH PINS",
        &[
            "3", "5", "10", "14", "16", "17", "23", "26", "31", "36", "84",
        ],
    ),
    ("Set dressing", "CAMP STOVE", &["5", "32", "34", "50", "65"]),
    (
        "Set dressing",
        "MARKER CAIRN",
        &["6", "50", "55", "62", "82", "90"],
    ),
    (
        "Set dressing",
        "WALL CLOCK",
        &[
            "9", "17", "27", "29", "40", "43", "44", "50", "57", "68", "76", "78", "80", "81", "89",
        ],
    ),
    (
        "Set dressing",
        "WEATHER BOARD",
        &["13", "22", "36", "52", "73"],
    ),
    (
        "Set dressing",
        "NETS",
        &["33", "34", "45", "46", "52", "53", "56", "76"],
    ),
    (
        "Wardrobe",
        "WEATHER COATS",
        &[
            "2", "8", "17", "29", "34", "37", "52", "54", "58", "65", "73", "83",
        ],
    ),
    ("Wardrobe", "TEO'S JUMPERS", &["2", "45", "86"]),
    (
        "Hair and makeup",
        "ANSEL'S BEARD AND SPLINT",
        &["34", "45", "50", "55", "59", "77"],
    ),
    (
        "Picture vehicles",
        "QUAD BIKE",
        &[
            "8", "18", "21", "22", "24", "35", "37", "39", "40", "42", "75",
        ],
    ),
    (
        "Picture vehicles",
        "PARK TRUCK",
        &[
            "24", "35", "38", "39", "47", "48", "53", "54", "61", "68", "72", "74", "75",
        ],
    ),
    (
        "Picture vehicles",
        "SEARCH PLANE",
        &["36", "44", "48", "50", "51", "53", "54", "57", "78", "79"],
    ),
    (
        "Picture vehicles",
        "ROWBOAT",
        &["33", "46", "49", "53", "76", "79"],
    ),
    ("Animals", "RAVEN", &["20"]),
    (
        "Practical effects",
        "WIND",
        &[
            "1", "2", "4", "6", "8", "9", "15", "19", "21", "27", "32", "60", "62", "64", "67",
            "83", "87", "90",
        ],
    ),
    (
        "Practical effects",
        "RAIN",
        &["2", "14", "18", "61", "64", "66"],
    ),
    ("Practical effects", "MIST", &["53", "54"]),
    ("Practical effects", "FLARE", &["53", "61"]),
    ("Visual effects", "THE FAR LIGHT", &["4", "49", "88"]),
    (
        "Camera",
        "CAR MOUNT",
        &["21", "24", "35", "38", "39", "47", "48", "61", "74", "75"],
    ),
    (
        "Grip and electric",
        "NIGHT EXTERIOR LIGHTING",
        &["6", "8", "9", "27", "28", "30", "64", "67", "70", "90"],
    ),
    (
        "Sound and music",
        "RADIO PLAYBACK",
        &[
            "3", "7", "16", "36", "37", "44", "51", "52", "57", "58", "59", "68",
        ],
    ),
    (
        "Special equipment",
        "CAMERA BOAT",
        &["32", "46", "49", "53", "76", "79"],
    ),
    ("Extra crew", "MARINE COORDINATOR", &["46", "49", "79"]),
    ("Safety", "RIGGER", &["4", "15", "67"]),
    ("Safety", "SAFETY DIVER", &["46", "49", "79"]),
    (
        "Other",
        "PARK ROAD PERMIT",
        &[
            "1", "20", "21", "24", "28", "30", "35", "38", "39", "47", "48", "61", "74", "75",
            "87", "88",
        ],
    ),
];

/// Background counts on their scenes: (element, scene, how many).
const FIELD_STATION_COUNTS: &[(&str, &str, u32)] = &[
    ("BASE STAFF", "13", 3),
    ("BASE STAFF", "36", 4),
    ("BASE STAFF", "52", 3),
    ("BASE STAFF", "58", 2),
    ("BASE STAFF", "66", 2),
    ("BASE STAFF", "73", 4),
    ("RELIEF TEAM", "80", 4),
    ("RELIEF TEAM", "83", 2),
    ("SECOND BOAT CREW", "79", 2),
];

/// The one cast member who never speaks, and the scenes he is seen in.
const FIELD_STATION_SILENT: (&str, &[&str]) = ("ANSEL", &["34", "45", "50", "55", "59", "77"]);

/// The second unit's scenes: the drive on the fire road, shot on day 14 beside the main
/// unit (decision 223).
const FIELD_STATION_SECOND_UNIT: &[&str] = &["21", "39", "74"];

/// Where each set is shot: the three invented locations, placed on the map so the board
/// and the call sheet show sunrise and sunset.
const FIELD_STATION_RIDGE: &str = "Hollins Ridge fire road";
const FIELD_STATION_STATION: &str = "Cold Spring field station";
const FIELD_STATION_LAKE: &str = "Kettle Lake boathouse";

/// The shooting order, agreed with Method Budget's sample (decision 223): 18 shoot days
/// from Monday 3 May 2027, 12 at the station, a company move to the lake for 3, the ridge
/// for 3 with the second unit's drive on day 14, then 2 nights back at the station. Each
/// day's scenes, with banners (text, minutes) where they fall.
const FIELD_STATION_DAYS: &[&[FieldRow]] = &[
    &[
        FieldRow::Scene("13"),
        FieldRow::Scene("22"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("36"),
        FieldRow::Scene("44"),
    ],
    &[
        FieldRow::Scene("58"),
        FieldRow::Scene("73"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("86"),
        FieldRow::Scene("52"),
    ],
    &[
        FieldRow::Scene("2"),
        FieldRow::Scene("3"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("12"),
        FieldRow::Scene("14"),
    ],
    &[
        FieldRow::Scene("16"),
        FieldRow::Scene("19"),
        FieldRow::Scene("37"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("40"),
        FieldRow::Scene("41"),
    ],
    &[
        FieldRow::Scene("83"),
        FieldRow::Scene("84"),
        FieldRow::Scene("85"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("11"),
        FieldRow::Scene("72"),
        FieldRow::Scene("23"),
    ],
    &[
        FieldRow::Scene("63"),
        FieldRow::Scene("17"),
        FieldRow::Scene("18"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("42"),
        FieldRow::Scene("43"),
    ],
    &[
        FieldRow::Scene("59"),
        FieldRow::Scene("60"),
        FieldRow::Scene("80"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("81"),
        FieldRow::Scene("15"),
        FieldRow::Scene("4"),
    ],
    &[
        FieldRow::Scene("82"),
        FieldRow::Scene("25"),
        FieldRow::Scene("62"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("29"),
        FieldRow::Scene("66"),
    ],
    &[
        FieldRow::Scene("89"),
        FieldRow::Scene("5"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("7"),
        FieldRow::Scene("8"),
    ],
    &[
        FieldRow::Scene("10"),
        FieldRow::Scene("26"),
        FieldRow::Scene("27"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("31"),
        FieldRow::Scene("64"),
    ],
    &[
        FieldRow::Banner("Company move to Kettle Lake", 45),
        FieldRow::Scene("34"),
        FieldRow::Scene("45"),
        FieldRow::Scene("55"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("56"),
        FieldRow::Scene("77"),
    ],
    &[
        FieldRow::Scene("78"),
        FieldRow::Scene("33"),
        FieldRow::Scene("46"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("54"),
        FieldRow::Scene("57"),
        FieldRow::Scene("76"),
    ],
    &[
        FieldRow::Scene("32"),
        FieldRow::Scene("53"),
        FieldRow::Scene("49"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("79"),
        FieldRow::Scene("50"),
    ],
    &[
        FieldRow::Banner("Company move to Hollins Ridge", 45),
        FieldRow::Scene("35"),
        FieldRow::Scene("47"),
        FieldRow::Scene("75"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("24"),
        FieldRow::Scene("48"),
        FieldRow::Scene("88"),
        FieldRow::Banner("Second unit: the drive on the fire road", 0),
        FieldRow::Scene("21"),
        FieldRow::Scene("39"),
        FieldRow::Scene("74"),
    ],
    &[
        FieldRow::Scene("1"),
        FieldRow::Scene("20"),
        FieldRow::Scene("38"),
    ],
    &[
        FieldRow::Scene("61"),
        FieldRow::Scene("87"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("28"),
        FieldRow::Scene("30"),
    ],
    &[
        FieldRow::Banner("Company move to the station", 45),
        FieldRow::Scene("65"),
        FieldRow::Scene("68"),
        FieldRow::Scene("69"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("70"),
        FieldRow::Scene("71"),
    ],
    &[
        FieldRow::Scene("90"),
        FieldRow::Scene("9"),
        FieldRow::Scene("51"),
        FieldRow::Banner("Lunch", 60),
        FieldRow::Scene("67"),
        FieldRow::Scene("6"),
    ],
];

enum FieldRow {
    Scene(&'static str),
    Banner(&'static str, u32),
}

/// Field Station, broken down and scheduled, to show every part of the app (decision
/// 223): the script imported (90 scenes, page lengths, cast), an element in every
/// category with background counts, a silent cast member, three locations on the map, a
/// second unit, 18 shoot days from Monday 3 May 2027 with day breaks, banners and call
/// times, prep and wrap on the calendar, and red flags. The same schedule every time; `fixtures/field-station.mtsc` is it, written by
/// `method-schedule sample --field-station`, and a test holds the two together.
pub fn field_station() -> Schedule {
    let mut s = Schedule::with_default_categories("Field Station");
    s.production.company = "Ridgeline Pictures".into();
    s.production.director = "Ines Varga".into();
    s.production.producer = "Tom Achebe".into();
    s.production.script_date = "2027-03-01".into();
    let cast = s.cast_or_add();
    s.category_mut(cast).expect("cast").auto_board_id = true;
    let script = crate::fountain::parse(FIELD_STATION_SCRIPT);
    let sheets = s.import_script(&script, cast).expect("the script imports");
    for &id in &sheets {
        let sheet = s.sheet_mut(id).expect("just imported");
        sheet.location = if sheet.set.contains("RIDGE ROAD") || sheet.set.contains("FIRE ROAD") {
            FIELD_STATION_RIDGE
        } else if sheet.set.contains("BOATHOUSE") || sheet.set.contains("KETTLE LAKE") {
            FIELD_STATION_LAKE
        } else {
            FIELD_STATION_STATION
        }
        .to_string();
        sheet.unit = if FIELD_STATION_SECOND_UNIT.contains(&sheet.scenes.as_str()) {
            "Second Unit"
        } else {
            "Main Unit"
        }
        .to_string();
        sheet.shoot_time = Some(Minutes(30 * sheet.pages.0.div_ceil(4).max(1)));
    }
    for cat in s.categories.clone() {
        if cat.id == cast {
            for e in s
                .elements
                .clone()
                .into_iter()
                .filter(|e| e.category == cast)
            {
                s.element_mut(e.id).expect("cast").rules = DoodRules {
                    holds: true,
                    drops: false,
                    min_drop_gap: 0,
                };
            }
        }
    }
    let sheet_of = |s: &Schedule, scene: &str| {
        sheets
            .iter()
            .copied()
            .find(|&id| s.sheet(id).is_some_and(|x| x.scenes == scene))
            .expect("a scene of the script")
    };
    let (silent, seen) = FIELD_STATION_SILENT;
    let ansel = s.add_element(cast, silent).expect("unique");
    for scene in seen {
        let sheet = sheet_of(&s, scene);
        s.sheet_mut(sheet).expect("a sheet").elements.push(ansel);
    }
    for (category, name, scenes) in FIELD_STATION_THINGS {
        let c = s.category_named(category).expect("a default category");
        let id = s.add_element(c, name).expect("unique");
        for scene in *scenes {
            let sheet = sheet_of(&s, scene);
            s.sheet_mut(sheet).expect("a sheet").elements.push(id);
        }
    }
    for (name, scene, count) in FIELD_STATION_COUNTS {
        let element = s
            .elements
            .iter()
            .find(|e| e.name == *name)
            .expect("a background element")
            .id;
        let sheet = sheet_of(&s, scene);
        s.set_quantity(sheet, element, *count, true)
            .expect("on the sheet");
    }
    let b = s.boards[0].id;
    let main = s.boards[0].sub_boards[0].id;
    s.rename_board(b, "Shooting Order").expect("a fine name");
    s.set_start(b, main, Some(Date::from_ymd(2027, 5, 3).expect("valid")))
        .expect("main");
    let end = |s: &Schedule| Place {
        sub_board: Some(main),
        index: s.boards[0].sub_boards[0].strips.len(),
    };
    for day in FIELD_STATION_DAYS {
        for row in *day {
            match row {
                FieldRow::Scene(scene) => {
                    let sheet = sheet_of(&s, scene);
                    let strip = s.boards[0]
                        .boneyard
                        .iter()
                        .find(|x| x.kind == StripKind::Sheet(sheet))
                        .expect("an imported scene's strip")
                        .id;
                    let to = end(&s);
                    s.move_strips(b, &[strip], to).expect("moves");
                }
                FieldRow::Banner(text, minutes) => {
                    let to = end(&s);
                    let banner = StripKind::Banner(Banner {
                        text: (*text).to_string(),
                        duration: Minutes(*minutes),
                        style: BannerStyle {
                            bold: true,
                            ..BannerStyle::default()
                        },
                    });
                    s.insert_strip(b, banner, to).expect("inserts");
                }
            }
        }
        let to = end(&s);
        s.insert_strip(b, StripKind::DayBreak, to).expect("inserts");
    }
    // Calls: 7:00 on day work, 16:00 on the night days (every scene a night).
    let ends: Vec<_> = s.boards[0].sub_boards[0]
        .strips
        .iter()
        .filter(|x| x.kind == StripKind::DayBreak)
        .map(|x| x.id)
        .collect();
    for (day, end) in FIELD_STATION_DAYS.iter().zip(ends) {
        let nights = day.iter().all(|row| match row {
            FieldRow::Scene(n) => {
                let sheet = sheet_of(&s, n);
                let tod = &s.sheet(sheet).expect("a sheet").time_of_day;
                tod == "NIGHT" || tod == "LATER" || tod == "EVENING"
            }
            FieldRow::Banner(..) => true,
        });
        let call = if nights { 16 * 60 } else { 7 * 60 };
        crate::plan::set_call_time(&mut s, end, Some(call)).expect("a day break");
    }
    // Prep and wrap for the budget's exchange, as Method Budget's sample pays them: four
    // weeks of prep from Monday 5 April, two weeks of wrap to Wednesday 9 June.
    let calendar = s.calendar_mut(b, main).expect("main");
    calendar.prep_start = Some(Date::from_ymd(2027, 4, 5).expect("valid"));
    calendar.wrap_end = Some(Date::from_ymd(2027, 6, 9).expect("valid"));
    // Red flags: a fitting and travel in prep, and the ranger would rather not work the
    // ridge's last day (soft, so the board marks it without refusing anything).
    s.standard_red_flag_types();
    let kind = |s: &Schedule, k: EventKind| {
        s.red_flag_types
            .iter()
            .find(|t| t.builtin.as_ref() == Some(&k))
            .expect("a standard type")
            .id
    };
    let person = |s: &Schedule, name: &str| {
        s.elements
            .iter()
            .find(|e| e.category == cast && e.name == name)
            .expect("in the cast")
            .id
    };
    let day = |y, m, d| Date::from_ymd(y, m, d).expect("valid");
    for (who, k, date, note, hard) in [
        (
            "MARA",
            EventKind::Fitting,
            day(2027, 4, 28),
            "Weather gear fitting",
            true,
        ),
        (
            "DELACROIX",
            EventKind::Travel,
            day(2027, 5, 2),
            "Flies in",
            true,
        ),
        (
            "RANGER",
            EventKind::Unavailable,
            day(2027, 5, 24),
            "Would rather not",
            false,
        ),
    ] {
        let (element, kind) = (person(&s, who), kind(&s, k));
        s.add_red_flags(
            &[element],
            kind,
            &[RedFlagDates::Range {
                from: date,
                to: date,
            }],
            note,
            hard,
        )
        .expect("a red flag");
    }
    for (name, latitude, longitude) in [
        (FIELD_STATION_RIDGE, 46_812_000, -121_532_000),
        (FIELD_STATION_STATION, 46_790_000, -121_498_000),
        (FIELD_STATION_LAKE, 46_771_000, -121_545_000),
    ] {
        s.plan
            .set_location(GeoLocation {
                name: name.into(),
                latitude,
                longitude,
                utc_offset: -8 * 60,
                daylight_saving: Some(DaylightSaving::UsCanada),
            })
            .expect("a valid place");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dood::dood;
    use std::time::{Duration, Instant};

    #[test]
    fn field_station_is_valid_stable_and_scheduled() {
        let a = field_station();
        a.check().unwrap();
        assert_eq!(a, field_station());
        assert_eq!(a.title, "Field Station");
        assert_eq!(a.sheets.len(), 90);
        let board = &a.boards[0];
        assert!(board.boneyard.is_empty(), "every scene is scheduled");
        let main = &board.sub_boards[0];
        let breaks = main
            .strips
            .iter()
            .filter(|x| x.kind == StripKind::DayBreak)
            .count();
        assert_eq!(breaks, 18);
        let cast = a.cast().unwrap();
        let mut names: Vec<_> = a
            .elements
            .iter()
            .filter(|e| e.category == cast)
            .map(|e| e.name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "ANSEL",
                "BOATHOUSE KEEPER",
                "DELACROIX",
                "DISPATCHER",
                "IVERSEN",
                "MARA",
                "PILOT",
                "RANGER",
                "TEO"
            ]
        );
        assert_eq!(a.plan.locations.len(), 3);
        // Every category has something in it, to show each one off.
        for c in &a.categories {
            assert!(
                a.elements.iter().any(|e| e.category == c.id),
                "{} is empty",
                c.name
            );
        }
        let units: std::collections::BTreeSet<_> =
            a.sheets.iter().map(|x| x.unit.as_str()).collect();
        assert_eq!(
            units.into_iter().collect::<Vec<_>>(),
            ["Main Unit", "Second Unit"]
        );
        // The neutral title page: this copy never carries another project's credit line.
        assert!(!FIELD_STATION_SCRIPT.to_lowercase().contains("screepub"));
        assert!(FIELD_STATION_SCRIPT.starts_with("Title: FIELD STATION\n"));
    }

    #[test]
    fn the_demo_is_valid_stable_and_shows_a_little_of_everything() {
        let a = demo();
        a.check().unwrap();
        assert_eq!(a, demo());
        assert_eq!(a.sheets.len(), 37);
        let board = &a.boards[0];
        assert_eq!(board.sub_boards.len(), 2);
        assert_eq!(board.boneyard.len(), 2);
        let days = |i: usize| {
            board.sub_boards[i]
                .strips
                .iter()
                .filter(|x| x.kind == StripKind::DayBreak)
                .count()
        };
        assert_eq!((days(0), days(1)), (11, 2));
        let banners = board.sub_boards[0]
            .strips
            .iter()
            .filter(|x| matches!(x.kind, StripKind::Banner(_)))
            .count();
        assert_eq!(banners, 2);
        let main = &board.sub_boards[0];
        assert_eq!(main.calendar.events.len(), 4);
        assert_eq!(main.calendar.holidays.len(), 1);
        assert_eq!(a.plan.locations.len(), 2);
        assert!(!a.strip_colors.is_default());
        assert_eq!(
            a.elements.iter().map(|e| e.red_flags.len()).sum::<usize>(),
            1
        );
        // The red flag falls on a day its element works, so the Red flags tab has a clash.
        let flags = crate::redflags::red_flags(&a, board.id, main.id).unwrap();
        assert_eq!(flags.clashes.len(), 1, "{flags:?}");
        assert_eq!(flags.clashes[0].day, Some(9));
    }

    #[test]
    fn sample_is_valid_and_stable() {
        let a = sample(300);
        a.check().unwrap();
        assert_eq!(a, sample(300));
        assert_eq!(a.sheets.len(), 300);
    }

    /// The kickoff's target: a 1,500-strip board reorders and recomputes the day out of
    /// days without a visible lag. Measured in release builds with
    /// `cargo test --release -p method-schedule-core perf -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn perf_reorder_and_dood_at_1500_strips() {
        let mut s = sample(1500);
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let strips: Vec<_> = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .map(|x| x.id)
            .collect();
        let t = Instant::now();
        let rounds = 50;
        for i in 0..rounds {
            let pick = strips[(i * 37) % strips.len()];
            s.move_strips(
                b,
                &[pick],
                Place {
                    sub_board: Some(sb),
                    index: (i * 101) % strips.len(),
                },
            )
            .unwrap();
            let rows = dood(&s, b, sb).unwrap();
            assert!(!rows.is_empty());
        }
        let each = t.elapsed() / rounds as u32;
        let t = Instant::now();
        s.check().unwrap();
        let check = t.elapsed();
        println!("1500 strips: move + full DOOD {each:?} each; check {check:?}");
    }

    /// The engine's budgets at 1,500 strips (decisions.md, entry 176): a move plus the
    /// whole day out of days, the day out of days alone and `Schedule::check`, each the
    /// median of 15 runs. Release builds only; CI runs them with
    /// `cargo test --release --workspace --locked -- --ignored budget`.
    #[test]
    #[ignore]
    fn budget_move_dood_and_check_at_1500_strips() {
        if cfg!(debug_assertions) {
            println!("budgets skipped: run them with cargo test --release");
            return;
        }
        let median = |f: &mut dyn FnMut()| {
            f();
            let mut times: Vec<_> = (0..15)
                .map(|_| {
                    let t = Instant::now();
                    f();
                    t.elapsed()
                })
                .collect();
            times.sort();
            times[7]
        };
        // Scaled for a slower machine than the owner's Mac (decision 191).
        let scale = budget_scale_percent();
        let within = |label: &str, took: Duration, budget_ms: u64| {
            let budget = Duration::from_millis(budget_ms);
            let allowed = budget * scale / 100;
            let ok = took <= allowed;
            println!(
                "budget {label:<36} {took:>10.2?} of {allowed:>8.2?} ({budget:?} x{}.{:02}){}",
                scale / 100,
                scale % 100,
                if ok { "" } else { "  OVER" }
            );
            ok
        };
        let mut s = sample(1500);
        let b = s.boards[0].id;
        let sb = s.boards[0].sub_boards[0].id;
        let strips: Vec<_> = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .map(|x| x.id)
            .collect();
        let mut i = 0;
        let moved = median(&mut || {
            i += 1;
            s.move_strips(
                b,
                &[strips[(i * 37) % strips.len()]],
                Place {
                    sub_board: Some(sb),
                    index: (i * 101) % strips.len(),
                },
            )
            .unwrap();
            assert!(!dood(&s, b, sb).unwrap().is_empty());
        });
        let alone = median(&mut || assert!(!dood(&s, b, sb).unwrap().is_empty()));
        let check = median(&mut || s.check().unwrap());
        let ok = [
            within("move + day out of days", moved, 8),
            within("day out of days", alone, 8),
            within("check", check, 3),
        ];
        assert!(ok.iter().all(|x| *x), "over budget at 1,500 strips");
    }
}
