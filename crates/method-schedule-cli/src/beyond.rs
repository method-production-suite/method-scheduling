//! The features beyond the reference app (decisions.md, IMP-04 and BYD-01 to BYD-09): script
//! revisions, call sheet data, the running timeline and call times, locations and
//! sunrise and sunset, turnarounds and rest checks, schedule locks and drift, and the
//! strip order suggestion.

use method_schedule_core::callsheet::call_sheet;
use method_schedule_core::drift::{drift, drift_text, lock, unlock};
use method_schedule_core::model::{BoardId, StripId, SubBoardId};
use method_schedule_core::optimize::{apply, suggest, suggestion_text};
use method_schedule_core::plan::{
    self, clock_text, degrees_text, hours_text, offset_text, parse_clock, parse_daylight_saving,
    parse_degrees, parse_hours, parse_offset, GeoLocation,
};
use method_schedule_core::rest::rest_report;
use method_schedule_core::revise::{revise, revision_text};
use method_schedule_core::solar::Sun;
use method_schedule_core::timeline::{
    past_wrap, past_wrap_in, push_past_wrap, timelines, EntryKind,
};
use method_schedule_core::{fdx, fountain, Schedule};
use method_schedule_format::callsheet as cs;

use super::{first, flag, id_flag, load, positional, read, save, today, write_bytes};

pub const HELP: &str = "
beyond the reference app (decisions.md, IMP-04 and BYD-01 to BYD-09):
  import SCRIPT.fdx --into FILE --tags
                                  also bring in the script's production tags as elements
  revise FILE SCRIPT [--tags] [--dry-run]
                                  merge a revised draft: scenes matched by number, then
                                  heading, keep their tags and places; new ones go to the
                                  Boneyard; missing ones are marked omitted; lists changes
  timeline FILE [DAY] [--sub-board ID]
                                  each shoot day from its call time: strip times, wrap,
                                  length, sunrise and sunset, warnings, and what runs
                                  past the wrap (the call plus the long day limit)
  timeline FILE DAY --push [--sub-board ID]
                                  move what runs past day DAY's wrap, whole strips in
                                  order, to the start of the next shoot day
  call FILE DAY (TIME | --clear) [--sub-board ID]
                                  set or clear a shoot day's call time (6:30, 18:00)
  long-day FILE (HOURS | --default)
                                  the day length that counts as long (12 or 10:30)
  locations FILE                  locations on the map, and each one's sheets
  location FILE NAME LAT LON [UTC_OFFSET] [--dst RULE] | location FILE NAME --remove
                                  place a sheet location on the map (34.0522 -118.2437
                                  -7) for sunrise and sunset; RULE is a daylight saving
                                  rule (us_canada, eu, australia, new_zealand or none;
                                  left out, kept), and with
                                  one UTC_OFFSET is the standard (winter) offset
  sun FILE [--sub-board ID]       sunrise and sunset for each dated shoot day
  callsheet FILE DAY [--csv] [--out OUT] [--sub-board ID]
                                  a shoot day's call sheet data as JSON (or CSV)
                                  (docs/CALLSHEET.md)
  turnaround FILE CATEGORY (HOURS | --clear)
                                  the shortest rest between wrap and next call for a
                                  category's elements
  rest FILE [--board NAME]        rests shorter than their turnaround (warnings)
  lock FILE NAME                  save where every scene of the board is now
  unlock FILE NAME                delete a lock
  drift FILE NAME                 each actor's work days added, dropped and moved since
                                  the lock
  pin FILE STRIP... [--unpin]     keep strips in place when suggesting an order
  suggest FILE [--apply] [--sub-board ID]
                                  a strip order grouped by location, set and cast, with
                                  before and after counts; --apply saves it
";

/// Runs one of this module's commands, or `None` when `cmd` is not one of them.
pub fn run(cmd: &str, rest: &[String]) -> Option<Result<String, String>> {
    Some(match cmd {
        "revise" => revise_cmd(rest),
        "timeline" => timeline_cmd(rest),
        "call" => call_cmd(rest),
        "long-day" => long_day_cmd(rest),
        "locations" => locations_cmd(rest),
        "location" => location_cmd(rest),
        "sun" => sun_cmd(rest),
        "callsheet" => callsheet_cmd(rest),
        "turnaround" => turnaround_cmd(rest),
        "rest" => rest_cmd(rest),
        "lock" => lock_cmd(rest),
        "unlock" => unlock_cmd(rest),
        "drift" => drift_cmd(rest),
        "pin" => pin_cmd(rest),
        "suggest" => suggest_cmd(rest),
        _ => return None,
    })
}

/// The board (`--board`, the first by default) and the sub-board `--sub-board` names
/// (its first by default).
fn pick(s: &Schedule, args: &[String]) -> Result<(BoardId, SubBoardId), String> {
    let (b, first_sub) = first(s)?;
    match id_flag(args, "--sub-board")? {
        None => Ok((b, first_sub)),
        Some(id) => {
            let board = s.board(b).ok_or("the schedule has no board")?;
            if board.sub_boards.iter().any(|x| x.id == SubBoardId(id)) {
                Ok((b, SubBoardId(id)))
            } else {
                Err(format!("the board {:?} has no sub-board {id}", board.name))
            }
        }
    }
}

const VALUE_FLAGS: &[&str] = &["--sub-board", "--board", "--out", "--lines-per-page"];

fn args_of<'a>(args: &'a [String], usage: &str, n: usize) -> Result<Vec<&'a str>, String> {
    let p = positional(args, VALUE_FLAGS);
    if p.len() < n {
        return Err(format!("usage: method-schedule {usage}"));
    }
    Ok(p)
}

fn day_number(v: &str) -> Result<u32, String> {
    v.parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("{v:?} is not a shoot day number"))
}

/// Lines a Fountain page from `--lines-per-page N` (40 to 70), else the default 56
/// (decision 195).
pub fn lines_per_page(args: &[String]) -> Result<u32, String> {
    use fountain::{LINES_PER_PAGE, MAX_LINES_PER_PAGE, MIN_LINES_PER_PAGE};
    let Some(i) = args.iter().position(|a| a == "--lines-per-page") else {
        return Ok(LINES_PER_PAGE);
    };
    let v = args.get(i + 1).ok_or("--lines-per-page needs a number")?;
    v.parse::<u32>()
        .ok()
        .filter(|n| (MIN_LINES_PER_PAGE..=MAX_LINES_PER_PAGE).contains(n))
        .ok_or_else(|| {
            format!("--lines-per-page is {MIN_LINES_PER_PAGE} to {MAX_LINES_PER_PAGE}, not {v:?}")
        })
}

/// Reads a script, with production tags when asked for (`.fdx` only); a Fountain one at
/// `lines` a page.
pub fn read_script(path: &str, tags: bool, lines: u32) -> Result<fountain::ImportedScript, String> {
    let text = read(path)?;
    if path.to_ascii_lowercase().ends_with(".mscenes") {
        if tags {
            return Err(format!("{path}: production tags come only from .fdx files"));
        }
        method_schedule_format::mscenes::parse(&text).map_err(|e| format!("{path}: {e}"))
    } else if path.to_ascii_lowercase().ends_with(".fdx") {
        let r = if tags {
            fdx::parse_with_tags(&text)
        } else {
            fdx::parse(&text)
        };
        r.map_err(|e| format!("{path}: {e}"))
    } else if tags {
        Err(format!("{path}: production tags come only from .fdx files"))
    } else {
        Ok(fountain::parse_with(&text, lines))
    }
}

fn revise_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(
        args,
        "revise FILE SCRIPT [--tags] [--dry-run] [--lines-per-page N]",
        2,
    )?;
    let (path, script_path) = (p[0], p[1]);
    let tags = args.iter().any(|a| a == "--tags");
    let script = read_script(script_path, tags, lines_per_page(args)?)?;
    let mut s = load(path)?;
    let cast = s.cast_or_add();
    let r = revise(&mut s, &script, cast, tags).map_err(|e| e.to_string())?;
    let text = revision_text(&s, &r);
    if args.iter().any(|a| a == "--dry-run") {
        return Ok(format!("{text}(dry run: nothing saved)\n"));
    }
    s.check().map_err(|e| e.to_string())?;
    save(path, &s)?;
    Ok(text)
}

fn timeline_cmd(args: &[String]) -> Result<String, String> {
    let usage = "timeline FILE [DAY] [--push] [--sub-board ID]";
    let p = args_of(args, usage, 1)?;
    let mut s = load(p[0])?;
    let only = p.get(1).map(|v| day_number(v)).transpose()?;
    let (b, sb) = pick(&s, args)?;
    if args.iter().any(|a| a == "--push") {
        let n = only.ok_or(format!("usage: method-schedule {usage}"))?;
        let past = past_wrap(&s, b, sb, n).map_err(|e| e.to_string())?;
        push_past_wrap(&mut s, b, sb, n, &past.strips).map_err(|e| e.to_string())?;
        save(p[0], &s)?;
        return Ok(format!(
            "{}\nMoved {}; the day breaks stayed.\n",
            past.text(),
            super::count(past.strips.len(), "strip")
        ));
    }
    let mut out = String::new();
    let all = timelines(&s, b, sb).map_err(|e| e.to_string())?;
    for t in &all {
        if only.is_some_and(|n| n != t.number) {
            continue;
        }
        let date = t.date.map_or(String::new(), |d| format!(" {d}"));
        let call = t.call.map_or("no call time".to_string(), |c| {
            format!("call {}", clock_text(c as i32))
        });
        out.push_str(&format!(
            "Day {}{date}: {call}, wrap {}, {} long",
            t.number,
            t.time_text(t.wrap),
            hours_text(t.length)
        ));
        if let Some(sun) = &t.sun {
            if let Sun::Times { rise, set } = sun.sun {
                out.push_str(&format!(
                    ", sunrise {} sunset {} at {}",
                    clock_text(rise),
                    clock_text(set),
                    sun.location
                ));
            }
        }
        out.push('\n');
        for e in &t.entries {
            let what = match &e.kind {
                EntryKind::Scene(id) => s.sheet(*id).map_or(String::new(), |h| {
                    format!(
                        "{} {} {} {}",
                        method_schedule_core::timeline::scene_name(h),
                        h.int_ext,
                        h.set,
                        h.time_of_day
                    )
                }),
                EntryKind::Banner(text) => format!("-- {text}"),
            };
            let untimed = if e.untimed { " (no shoot time)" } else { "" };
            out.push_str(&format!(
                "  {:>8} {:>8}  {what}{untimed}\n",
                t.time_text(e.start),
                t.time_text(e.end)
            ));
        }
        for w in &t.warnings {
            out.push_str(&format!("  ! {w}\n"));
        }
        let past = past_wrap_in(&s, b, sb, &all, t.number).map_err(|e| e.to_string())?;
        if !past.strips.is_empty() {
            out.push_str(&format!("  > {}\n", past.text()));
        }
    }
    if out.is_empty() {
        out.push_str("No shoot days.\n");
    }
    Ok(out)
}

/// The day break closing shoot day `n`.
fn day_break(s: &Schedule, b: BoardId, sb: SubBoardId, n: u32) -> Result<StripId, String> {
    let all = method_schedule_core::days::days(s, b, sb).map_err(|e| e.to_string())?;
    all.days
        .iter()
        .find(|d| d.number == n)
        .map(|d| d.break_id)
        .ok_or_else(|| {
            let have = super::count(all.days.len(), "shoot day");
            format!("the sub-board has {have}; no day {n}")
        })
}

fn call_cmd(args: &[String]) -> Result<String, String> {
    let usage = "call FILE DAY (TIME | --clear) [--sub-board ID]";
    let p = args_of(args, usage, 2)?;
    let mut s = load(p[0])?;
    let (b, sb) = pick(&s, args)?;
    let n = day_number(p[1])?;
    let day = day_break(&s, b, sb, n)?;
    let time = if args.iter().any(|a| a == "--clear") {
        None
    } else {
        let v = p.get(2).ok_or(format!("usage: method-schedule {usage}"))?;
        Some(parse_clock(v)?)
    };
    plan::set_call_time(&mut s, day, time).map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    Ok(match time {
        Some(t) => format!("day {n} calls at {}\n", clock_text(t as i32)),
        None => format!("day {n} has no call time\n"),
    })
}

fn long_day_cmd(args: &[String]) -> Result<String, String> {
    let usage = "long-day FILE (HOURS | --default)";
    let p = args_of(args, usage, 1)?;
    let mut s = load(p[0])?;
    let m = if args.iter().any(|a| a == "--default") {
        None
    } else {
        Some(parse_hours(
            p.get(1).ok_or(format!("usage: method-schedule {usage}"))?,
        )?)
    };
    plan::set_long_day(&mut s, m).map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    Ok(format!(
        "a day longer than {} is long\n",
        hours_text(s.plan.long_day())
    ))
}

fn locations_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "locations FILE", 1)?;
    let s = load(p[0])?;
    let mut names: Vec<String> = Vec::new();
    for h in &s.sheets {
        let n = h.location.trim();
        if !n.is_empty() && !names.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            names.push(n.to_string());
        }
    }
    for l in &s.plan.locations {
        if !names.iter().any(|x| x.eq_ignore_ascii_case(&l.name)) {
            names.push(l.name.clone());
        }
    }
    let mut out = String::new();
    for n in names {
        let sheets = s
            .sheets
            .iter()
            .filter(|h| h.location.trim().eq_ignore_ascii_case(&n))
            .count();
        let at = s
            .plan
            .location(&n)
            .map_or("not on the map".to_string(), |l| {
                let dst = l
                    .daylight_saving
                    .map_or(String::new(), |r| format!(" ({} summer time)", r.label()));
                format!(
                    "{} {} UTC{}{dst}",
                    degrees_text(l.latitude),
                    degrees_text(l.longitude),
                    offset_text(l.utc_offset)
                )
            });
        out.push_str(&format!("{n:<28} {sheets:>4} sheets  {at}\n"));
    }
    if out.is_empty() {
        out.push_str("No sheet has a location.\n");
    }
    Ok(out)
}

fn location_cmd(args: &[String]) -> Result<String, String> {
    let usage =
        "location FILE NAME LAT LON [UTC_OFFSET] [--dst RULE] | location FILE NAME --remove";
    // Negative numbers are values here, not flags.
    let dst_at = args.iter().position(|a| a == "--dst");
    let p: Vec<&str> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            *a != "--remove" && Some(*i) != dst_at && Some(*i) != dst_at.map(|d| d + 1)
        })
        .map(|(_, a)| a.as_str())
        .collect();
    if p.len() < 2 {
        return Err(format!("usage: method-schedule {usage}"));
    }
    let mut s = load(p[0])?;
    let name = p[1];
    if args.iter().any(|a| a == "--remove") {
        if !s.plan.remove_location(name) {
            return Err(format!("{name:?} is not on the map"));
        }
        save(p[0], &s)?;
        return Ok(format!("{name} is off the map\n"));
    }
    if p.len() < 4 {
        return Err(format!("usage: method-schedule {usage}"));
    }
    let lat = parse_degrees(p[2], 90)?;
    let lon = parse_degrees(p[3], 180)?;
    // Without an offset, the sun's own: an hour for every 15 degrees.
    let offset = match p.get(4) {
        Some(v) => parse_offset(v)?,
        None => ((lon as i64 * 60 / 15 / 1_000_000) as i32 / 60) * 60,
    };
    // Without --dst, the rule the location had stays (decision 193).
    let daylight_saving = match dst_at {
        Some(i) => parse_daylight_saving(
            args.get(i + 1)
                .ok_or(format!("usage: method-schedule {usage}"))?,
        )?,
        None => s.plan.location(name).and_then(|l| l.daylight_saving),
    };
    s.plan
        .set_location(GeoLocation {
            name: name.to_string(),
            latitude: lat,
            longitude: lon,
            utc_offset: offset,
            daylight_saving,
        })
        .map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    let dst = daylight_saving.map_or(String::new(), |r| format!(", {} summer time", r.label()));
    Ok(format!(
        "{name} at {} {}, UTC{}{dst}\n",
        degrees_text(lat),
        degrees_text(lon),
        offset_text(offset)
    ))
}

fn sun_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "sun FILE [--sub-board ID]", 1)?;
    let s = load(p[0])?;
    let (b, sb) = pick(&s, args)?;
    let mut out = String::new();
    for t in timelines(&s, b, sb).map_err(|e| e.to_string())? {
        let Some(date) = t.date else { continue };
        let line = match &t.sun {
            None => "no located scene".to_string(),
            Some(x) => match x.sun {
                Sun::Times { rise, set } => format!(
                    "sunrise {} sunset {} at {}",
                    clock_text(rise),
                    clock_text(set),
                    x.location
                ),
                Sun::AlwaysUp => format!("the sun stays up at {}", x.location),
                Sun::AlwaysDown => format!("the sun does not rise at {}", x.location),
            },
        };
        out.push_str(&format!("Day {:>3} {date}  {line}\n", t.number));
    }
    if out.is_empty() {
        out.push_str("No dated shoot days: set a start date.\n");
    }
    Ok(out)
}

fn callsheet_cmd(args: &[String]) -> Result<String, String> {
    let usage = "callsheet FILE DAY [--csv] [--out OUT] [--sub-board ID]";
    let p = args_of(args, usage, 2)?;
    let s = load(p[0])?;
    let (b, sb) = pick(&s, args)?;
    let c = call_sheet(&s, b, sb, day_number(p[1])?).map_err(|e| e.to_string())?;
    let bytes = if args.iter().any(|a| a == "--csv") {
        cs::to_csv(&c)
    } else {
        cs::to_json(&c).into_bytes()
    };
    match flag(args, "--out") {
        Some(out) => {
            write_bytes(out, &bytes)?;
            Ok(format!("wrote {out}\n"))
        }
        None => String::from_utf8(bytes).map_err(|e| e.to_string()),
    }
}

fn turnaround_cmd(args: &[String]) -> Result<String, String> {
    let usage = "turnaround FILE CATEGORY (HOURS | --clear)";
    let p = args_of(args, usage, 2)?;
    let mut s = load(p[0])?;
    let cat = s
        .category_named(p[1])
        .ok_or_else(|| format!("no category {:?}", p[1]))?;
    let m = if args.iter().any(|a| a == "--clear") {
        None
    } else {
        Some(parse_hours(
            p.get(2).ok_or(format!("usage: method-schedule {usage}"))?,
        )?)
    };
    plan::set_turnaround(&mut s, cat, m).map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    Ok(match m {
        Some(m) => format!("{} rests {} at least\n", p[1], hours_text(m)),
        None => format!("{} has no turnaround minimum\n", p[1]),
    })
}

fn rest_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "rest FILE [--board NAME]", 1)?;
    let s = load(p[0])?;
    let b = first(&s)?.0;
    if s.plan.turnarounds.is_empty() {
        return Ok("No category has a turnaround minimum.\n".to_string());
    }
    let r = rest_report(&s, b).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for x in &r.short {
        out.push_str(&format!("{}\n", x.text));
    }
    if r.short.is_empty() {
        out.push_str("Every rest meets its turnaround.\n");
    }
    if r.unchecked > 0 {
        out.push_str(&format!(
            "{} work days were not checked: they need a date and a call time.\n",
            r.unchecked
        ));
    }
    Ok(out)
}

fn lock_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "lock FILE NAME", 2)?;
    let mut s = load(p[0])?;
    let (b, _) = first(&s)?;
    lock(&mut s, b, p[1], today()).map_err(|e| e.to_string())?;
    let n = s.plan.lock(p[1]).map_or(0, |l| l.scenes.len());
    save(p[0], &s)?;
    Ok(format!("locked {n} scheduled scenes as {:?}\n", p[1]))
}

fn unlock_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "unlock FILE NAME", 2)?;
    let mut s = load(p[0])?;
    unlock(&mut s, p[1]).map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    Ok(format!("deleted the lock {:?}\n", p[1]))
}

fn drift_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "drift FILE NAME", 2)?;
    let s = load(p[0])?;
    let d = drift(&s, p[1]).map_err(|e| e.to_string())?;
    Ok(drift_text(&s, &d))
}

fn pin_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "pin FILE STRIP... [--unpin]", 2)?;
    let mut s = load(p[0])?;
    let ids = p[1..]
        .iter()
        .map(|v| {
            v.parse::<u32>()
                .map(StripId)
                .map_err(|_| format!("{v:?}: not a strip id"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let on = !args.iter().any(|a| a == "--unpin");
    let n = plan::set_pinned(&mut s, &ids, on).map_err(|e| e.to_string())?;
    save(p[0], &s)?;
    Ok(format!(
        "{} {n} strip(s)\n",
        if on { "pinned" } else { "unpinned" }
    ))
}

fn suggest_cmd(args: &[String]) -> Result<String, String> {
    let p = args_of(args, "suggest FILE [--apply] [--sub-board ID]", 1)?;
    let mut s = load(p[0])?;
    let (b, sb) = pick(&s, args)?;
    let sug = suggest(&s, b, sb).map_err(|e| e.to_string())?;
    let mut out = suggestion_text(&sug);
    if args.iter().any(|a| a == "--apply") {
        apply(&mut s, &sug).map_err(|e| e.to_string())?;
        save(p[0], &s)?;
        out.push_str("Saved the new order.\n");
    } else {
        out.push_str("Nothing changed; --apply saves this order.\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::run as run_cli;

    fn run(args: &[&str]) -> Result<String, String> {
        run_cli(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn the_beyond_commands_work_end_to_end() {
        let dir = std::env::temp_dir().join(format!("ms-beyond-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.mtsc");
        let f = file.to_str().unwrap();
        let _ = std::fs::remove_file(&file);
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/scripts/harbor-light.fountain"
        );
        run(&["import", script, "--into", f]).unwrap();
        run(&["schedule", f]).unwrap();
        run(&["start", f, "2027-03-01"]).unwrap();
        run(&["breaks", f, "--pages", "5"]).unwrap();
        assert!(run(&["call", f, "1", "7:00"]).unwrap().contains("07:00"));
        let t = run(&["timeline", f, "1"]).unwrap();
        assert!(t.starts_with("Day 1 2027-03-01: call 07:00"), "{t}");
        // Seven hours a scene: day 1 runs past its 19:00 wrap, and --push moves the rest
        // of it to day 2 (decision 192).
        let mut s = super::super::load(f).unwrap();
        for h in &mut s.sheets {
            h.shoot_time = Some(method_schedule_core::model::Minutes(420));
        }
        super::super::save(f, &s).unwrap();
        let t = run(&["timeline", f, "1"]).unwrap();
        assert!(
            t.contains("  > ") && t.contains("past 19:00 on day 1"),
            "{t}"
        );
        // Day 1 is the last day, so nothing moves until a day break follows it.
        assert!(run(&["timeline", f, "1", "--push"])
            .unwrap_err()
            .contains("the last shoot day"));
        let sub = Some(s.boards[0].sub_boards[0].id);
        let at = method_schedule_core::model::Place {
            sub_board: sub,
            index: usize::MAX,
        };
        let b = s.boards[0].id;
        s.insert_strip(b, method_schedule_core::model::StripKind::DayBreak, at)
            .unwrap();
        super::super::save(f, &s).unwrap();
        let m = run(&["timeline", f, "1", "--push"]).unwrap();
        assert!(m.contains("They would move to the start of day 2."), "{m}");
        assert!(m.contains("; the day breaks stayed."), "{m}");
        assert!(!run(&["timeline", f, "1"]).unwrap().contains("  > "));
        assert!(run(&["timeline", f, "1", "--push"]).is_err());
        assert!(run(&["timeline", f, "--push"]).is_err());
        assert!(run(&["location", f, "Harbor", "41.5", "-70.6", "-5"])
            .unwrap()
            .contains("UTC-05:00"));
        assert!(run(&["locations", f]).unwrap().contains("Harbor"));
        // A daylight saving rule (decision 193): kept when --dst is left out.
        assert!(run(&[
            "location",
            f,
            "Harbor",
            "41.5",
            "-70.6",
            "-5",
            "--dst",
            "us_canada"
        ])
        .unwrap()
        .ends_with("UTC-05:00, US and Canada summer time\n"));
        assert!(run(&["location", f, "Harbor", "41.5", "-70.6", "-5"])
            .unwrap()
            .contains("US and Canada summer time"));
        assert!(run(&["locations", f])
            .unwrap()
            .contains("UTC-05:00 (US and Canada summer time)"));
        assert!(run(&["location", f, "Harbor", "41.5", "-70.6", "--dst", "mars"]).is_err());
        assert!(
            !run(&["location", f, "Harbor", "41.5", "-70.6", "-5", "--dst", "none"])
                .unwrap()
                .contains("summer")
        );
        assert!(run(&["sun", f]).unwrap().starts_with("Day   1 2027-03-01"));
        let j = run(&["callsheet", f, "1"]).unwrap();
        assert!(j.contains("\"format\": \"method-schedule-callsheet\""));
        assert!(run(&["callsheet", f, "1", "--csv"])
            .unwrap()
            .starts_with("\u{feff}Field,Value"));
        assert!(run(&["turnaround", f, "Cast", "12"])
            .unwrap()
            .contains("12:00"));
        run(&["rest", f]).unwrap();
        assert!(run(&["lock", f, "White"]).unwrap().starts_with("locked"));
        assert!(run(&["drift", f, "White"])
            .unwrap()
            .contains("No actor's days changed"));
        let sug = run(&["suggest", f]).unwrap();
        assert!(sug.contains("Cast work days"), "{sug}");
        run(&["suggest", f, "--apply"]).unwrap();
        run(&["drift", f, "White"]).unwrap();
        let r = run(&["revise", f, script, "--dry-run"]).unwrap();
        assert!(r.contains("0 new, 0 omitted"), "{r}");
        assert!(run(&["revise", f, script, "--tags"]).is_err());
        run(&["unlock", f, "White"]).unwrap();
        assert!(run(&["long-day", f, "10:30"]).unwrap().contains("10:30"));
        run(&["check", f]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
