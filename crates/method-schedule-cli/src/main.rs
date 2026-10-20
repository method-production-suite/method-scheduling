//! `method-schedule`: the scheduling command line.

mod banners;
mod beyond;
mod mcp;
mod more;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use method_schedule_core::conflicts::{conflicts, Booking};
use method_schedule_core::days::{auto_day_breaks, days, remove_day_breaks, Limit};
use method_schedule_core::diff::diff;
use method_schedule_core::dood::{dood, summary};
use method_schedule_core::explain::explain;
use method_schedule_core::find::{
    self as finder, CondField, Condition, FieldFilter, Op, Query, Scope,
};
use method_schedule_core::merge::{merge_with, Merged, Side};
use method_schedule_core::model::{
    BoardId, ElementId, Minutes, Place, Seconds, SheetId, SubBoardId,
};
use method_schedule_core::redflags::red_flags;
use method_schedule_core::reports::{
    daily_category_totals, element_list, one_line, shooting_schedule, OneLine, ShootingLine,
};
use method_schedule_core::{fountain, Date, Eighths, Schedule};
use method_schedule_format::export::{self, csv, xlsx, ExportError, Workbook, PAGES};

const HELP: &str = "\
usage: method-schedule <command> [args]

schedules:
  new FILE [--title TEXT] [--from TEMPLATE]
                                  create an empty schedule, or one from a template
  template FILE OUT.mtst          write FILE's setup (categories, elements, types,
                                  boards, layouts) as a template, without sheets,
                                  strips, events or red flags
  sample FILE [--sheets N]        create a large invented schedule (default 1500 sheets)
  sample --demo FILE              write the small invented production the app opens
                                  with Help > Open Sample
  sample --field-station FILE     write Field Station, the suite's shared invented
                                  production, broken down and scheduled
  show FILE                       sheets, boards and shoot days at a glance
  check FILE                      confirm the file reads and hangs together
  fmt FILE                        rewrite the file in canonical form
  import SCRIPT                   list a script's scenes (Fountain, or .fdx)
  import SCRIPT --into FILE       add a script's scenes to a schedule (created if missing)
  import ... --lines-per-page N   lines on a Fountain page, 40 to 70 (default 56); also
                                  for revise
  schedule FILE                   move every Boneyard strip to the end of the first sub-board
  start FILE DATE                 set the first sub-board's start date (YYYY-MM-DD)
  breaks FILE --pages LEN | --minutes N | --seconds N | --remove
                                  replace (or remove) day breaks on the first sub-board
  sort FILE FIELD[:desc],...      sort the first sub-board's strips by up to four
                                  fields (scene, int_ext, set, time_of_day, location,
                                  pages, script_day, unit, sequence, episode): banners
                                  go to the top and day breaks are removed
  days FILE                       list shoot days with dates and totals
  banners FILE                    every banner of the board with its strip id,
                                  length and style
  banner FILE --add TEXT [--at N] | STRIP [--text TEXT]
         [--minutes N] [--bold | --no-bold] [--color NAME|#rrggbb|none] [--height 1|2]
         [--size small|normal|large] [--align left|center|right]
         [--font usual|serif|sans|mono]
                                  insert a banner on the first sub-board (at the end
                                  unless --at), or change one, keeping what is not
                                  given; colors yellow, orange, red, pink, purple,
                                  blue, teal, green, gray
  layouts FILE [--export OUT | --import IN]
                                  list the named layouts, save them all to a layout
                                  file, or add a layout file's layouts
  dood FILE [--csv]               day out of days summary
  dood FILE --grid [--no-holds] [--sort KEY[:desc],...] [--csv | --xlsx OUT]
                                  the day out of days grid in the schedule's code
                                  letters; sort keys board_id, name, start (or
                                  first_work), finish (last_work), work (worked),
                                  hold (held)
  oneline FILE [--csv | --xlsx OUT] [--columns layout|fixed]
                                  one-line schedule; the export follows the report's
                                  strip layout (fixed: the stable day to text columns),
                                  then a column per category the report shows
  elements FILE [--csv]           element list with sheets, pages, days and dates
  shooting FILE                   shooting schedule: each scene with its elements
  totals FILE [--csv | --xlsx OUT]
                                  elements per category per shoot day (the
                                  workbook's total row is a formula)
  breakdown FILE [--by-day] [--csv | --xlsx OUT]
                                  each scene with its elements by category, or each
                                  shoot day's elements merged by category
  detail FILE [--csv | --xlsx OUT]
                                  each element's scenes with their day and date
  board FILE (--csv | --xlsx OUT) every strip of the board, then the Boneyard
  calendar FILE [--from DATE] [--to DATE]
                                  each date: what it is, shoot day, scenes, pages,
                                  length and events
  vertical FILE                   the cast ID grid: a row per cast member, a column
                                  per scene
  multiunit FILE [--element NAME] every sub-board's shoot days merged by date
  reports FILE                    the report list with each report's options
  headers FILE                    the report header layouts: id, looks and rows
  production FILE [--company T] [--director T] [--producer T] [--episode T] ...
                                  show or set the production info report headers use
                                  (every field: --unit-production-manager,
                                  --assistant-director, --art-director, --set-dresser,
                                  --script-date, --production-number, --preparer,
                                  --timing, --episode, --custom-1 to --custom-4)
  codes FILE [KEY=LETTERS ...] [default]
                                  show or set the day out of days code letters
  prep FILE [--start DATE|none] [--end DATE|none]
                                  show or set the prep start and wrap end the budget
                                  exchange carries
  budget FILE [--out OUT]         budget exchange file: shoot days and element day counts
                                  (docs/EXCHANGE.md); prints it unless --out is given
  dood|oneline|elements FILE --xlsx OUT.xlsx
                                  write the report as an Excel workbook

versions:
  diff OLD NEW                    what changed between two schedules, in plain words
  merge BASE OURS THEIRS [--out FILE] [--theirs]
                                  three-way merge of two schedules made from BASE; prints
                                  the merged file unless --out is given; conflicts are
                                  settled for ours (or --theirs) and listed, exit 1
  merge-driver BASE OURS THEIRS [PATH] [--theirs]
                                  the same merge as a git merge driver: writes the result
                                  over OURS; exit 1 on conflicts (docs/MERGE.md)
  git-setup [--global] [--theirs] sets git up to merge schedules with merge-driver: in this
                                  repository (its .gitattributes and config), or for every
                                  repository with --global
  explain FILE ELEMENT [--category CAT]
                                  why an element's day out of days counts are what they
                                  are, date by date

find and conflicts:
  find FILE TEXT [--field F] [--case] [--word] [--sub-board ID]
                                  numbered places TEXT is found, in board order; F is
                                  sheet (every header field), a header field (scenes,
                                  int_ext, set, time_of_day, synopsis, location,
                                  script_pages, script_day, unit, sequence, episode,
                                  comments, number), element, category, banner or event.
                                  --where FIELD:OP:VALUE keeps only the sheets that
                                  meet it (OP: contains, not_contains, is, is_not,
                                  starts_with, empty, not_empty, greater, less; FIELD
                                  a header field, pages, shoot_time or screen_time),
                                  repeatable, joined by AND (OR with --any); TEXT may
                                  then be empty to list the sheets
  replace FILE TEXT NEW [--field F] [--case] [--word] [--sub-board ID] [--where W]...
          [--any] (--all | --pick N,N)
                                  replace TEXT by NEW in every result, or in the results
                                  numbered as find numbers them; saves, then lists changes
  conflicts FILE [--board NAME]   elements on one date in two sub-boards of a board,
                                  events on work days, and event clashes (informational)
  redflags FILE [--board NAME] [--sub-board ID]
                                  red flag clashes: elements working, or at a counted
                                  event, on dates their red flags rule out (warnings)

sheets:
  merge-sheets FILE SURVIVOR OTHER...
                                  merge breakdown sheets (ids) into SURVIVOR: scenes
                                  joined in the order given (SURVIVOR first), lengths and
                                  times summed, elements unioned, the others deleted;
                                  saves, then lists what was combined and what was dropped
                                  (decisions.md, entries 35 and 46)
  duplicate-sheet FILE SHEET      copy a sheet (to split a scene), right after it on every
                                  sub-board holding it, else at the end of the Boneyard

agents:
  mcp --root DIR [--root DIR...] [--read-only]
                                  serve schedules in DIR to a coding agent over MCP on
                                  stdin and stdout (docs/MCP.md)

other:
  eighths LENGTH...               add page lengths (2, 3/8, 1 3/8) and print the total
  help                            show this help
  --version                       show the version

Commands that work on one board take --board NAME (or the board's id) and use the
first board without it; they work on its first sub-board unless --sub-board is given.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        args.first().map(String::as_str),
        Some("merge" | "merge-driver")
    ) {
        // Conflicts are their own exit code: the merged file is written either way.
        return match merge_cmd(&args) {
            Ok(m) => {
                print!("{}", m.stdout);
                eprint!("{}", m.stderr);
                if m.conflicts {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(msg) => {
                eprintln!("method-schedule: {msg}");
                ExitCode::FAILURE
            }
        };
    }
    match run(&args) {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("method-schedule: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// `method-schedule --version`: the suite's form, "method-<noun> X (Method <Noun>)"
/// (decision 211).
fn version_line() -> String {
    format!(
        "method-schedule {} (Method Schedule)\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn run(args: &[String]) -> Result<String, String> {
    let rest = args.get(1..).unwrap_or(&[]);
    let cmd = args.first().map(String::as_str);
    let rest = &take_board(cmd.unwrap_or(""), rest)?;
    match cmd {
        None | Some("help" | "-h" | "--help") => Ok(format!("{HELP}{}", beyond::HELP)),
        Some("--version" | "-V") => Ok(version_line()),
        Some("eighths") => eighths(rest),
        Some("new") => new(rest),
        Some("template") => template_cmd(rest),
        Some("sample") => sample_cmd(rest),
        Some("show") => show(&load(one(rest, "show FILE")?)?),
        Some("check") => {
            let path = one(rest, "check FILE")?;
            load(path)?;
            Ok(format!("{path}: ok\n"))
        }
        Some("fmt") => {
            let path = one(rest, "fmt FILE")?;
            let s = load(path)?;
            save(path, &s)?;
            Ok(String::new())
        }
        Some("import") => import(rest),
        Some("start") => start(rest),
        Some("schedule") => schedule_all(one(rest, "schedule FILE")?),
        Some("breaks") => breaks(rest),
        Some("sort") => sort_cmd(rest),
        Some("layouts") => layouts_cmd(rest),
        Some("banners") => banners::banners_cmd(rest),
        Some("banner") => banners::banner_cmd(rest),
        Some("days") => list_days(&load(one(rest, "days FILE")?)?),
        Some("dood") if rest.iter().any(|a| a == "--grid") => more::dood_grid_cmd(rest),
        Some("dood") => dood_cmd(rest),
        Some("breakdown") => more::breakdown_cmd(rest),
        Some("detail") => more::detail_cmd(rest),
        Some("board") => more::board_cmd(rest),
        Some("calendar") => more::calendar_cmd(rest),
        Some("vertical") => more::vertical_cmd(rest),
        Some("multiunit") => more::multi_unit_cmd(rest),
        Some("reports") => more::reports_cmd(rest),
        Some("production") => more::production_cmd(rest),
        Some("headers") => more::headers_cmd(rest),
        Some("codes") => more::codes_cmd(rest),
        Some("prep") => more::prep_cmd(rest),
        Some("oneline") => oneline_cmd(rest),
        Some("elements") => elements_cmd(rest),
        Some("shooting") => shooting_cmd(one(rest, "shooting FILE")?),
        Some("budget") => budget_cmd(rest),
        Some("totals") => totals_cmd(rest),
        Some("mcp") => mcp_cmd(rest),
        Some("diff") => diff_cmd(rest),
        Some("git-setup") => git_setup_cmd(rest),
        Some("explain") => explain_cmd(rest),
        Some("find") => find_cmd(rest),
        Some("replace") => replace_cmd(rest),
        Some("conflicts") => conflicts_cmd(rest),
        Some("redflags") => redflags_cmd(rest),
        Some("merge-sheets") => merge_sheets_cmd(rest),
        Some("duplicate-sheet") => duplicate_sheet_cmd(rest),
        Some(other) => beyond::run(other, rest).unwrap_or_else(|| {
            Err(format!(
                "unknown command {other:?}, try `method-schedule help`"
            ))
        }),
    }
}

fn one<'a>(args: &'a [String], usage: &str) -> Result<&'a str, String> {
    match args {
        [x] => Ok(x),
        _ => Err(format!("usage: method-schedule {usage}")),
    }
}

/// The value after `flag`, if present.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// The arguments that are not flags, skipping the values of `with_value` flags.
fn positional<'a>(args: &'a [String], with_value: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if with_value.contains(&a.as_str()) {
            it.next();
        } else if !a.starts_with("--") {
            out.push(a.as_str());
        }
    }
    out
}

/// Files larger than this are refused, so a stray huge file can't stall a command.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

fn read(path: &str) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{path}: larger than 64 MB"));
    }
    fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn load(path: &str) -> Result<Schedule, String> {
    method_schedule_format::from_str(&read(path)?).map_err(|e| format!("{path}: {e}"))
}

/// Writes `s` over `path`. A template stays a template (decision 77): the change is
/// refused when it gave the template sheets, strips, events or red flags.
fn save(path: &str, s: &Schedule) -> Result<(), String> {
    let template = Path::new(path).is_file()
        && read(path).is_ok_and(|t| method_schedule_format::is_template(&t));
    let text = if template {
        method_schedule_format::to_template_string_as_is(s).map_err(|e| {
            format!("{path}: {e}; start a schedule from it with new FILE --from {path}")
        })?
    } else {
        method_schedule_format::to_string(s)
    };
    write_bytes(path, text.as_bytes())
}

/// Writes atomically: a temporary file beside the target, flushed, then renamed over it.
fn write_bytes(path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = Path::new(path);
    let dir = target
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = target
        .file_name()
        .ok_or(format!("{path}: not a file name"))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, target)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("{path}: {e}")
    })
}

/// The commands that work on one board. `--board NAME` (or the board's id) picks it;
/// without it they use the first board.
const BOARD_COMMANDS: &[&str] = &[
    "schedule",
    "start",
    "breaks",
    "sort",
    "days",
    "banners",
    "banner",
    "dood",
    "oneline",
    "elements",
    "shooting",
    "totals",
    "breakdown",
    "detail",
    "board",
    "calendar",
    "vertical",
    "multiunit",
    "prep",
    "budget",
    "explain",
    "conflicts",
    "redflags",
    "timeline",
    "call",
    "sun",
    "callsheet",
    "rest",
    "lock",
    "suggest",
];

thread_local! {
    /// The `--board` value of the command being run, if it gave one.
    static BOARD: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Takes `--board VALUE` out of a board command's arguments and keeps it for [`first`].
fn take_board(cmd: &str, args: &[String]) -> Result<Vec<String>, String> {
    let mut value = None;
    let mut out = Vec::with_capacity(args.len());
    if BOARD_COMMANDS.contains(&cmd) {
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a == "--board" {
                let v = it
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or("--board needs a board's name or id")?;
                if value.replace(v.clone()).is_some() {
                    return Err("--board is given twice".to_string());
                }
            } else {
                out.push(a.clone());
            }
        }
    } else {
        out.extend_from_slice(args);
    }
    BOARD.with(|b| *b.borrow_mut() = value);
    Ok(out)
}

/// Whether the command was given `--board`.
fn board_given() -> bool {
    BOARD.with(|b| b.borrow().is_some())
}

/// The board `--board` names, by name (any case) or id; the first board without it.
fn chosen_board(s: &Schedule) -> Result<&method_schedule_core::model::Board, String> {
    let Some(v) = BOARD.with(|b| b.borrow().clone()) else {
        return s
            .boards
            .first()
            .ok_or("the schedule has no board".to_string());
    };
    if let Some(b) = s.boards.iter().find(|b| b.name == v) {
        return Ok(b);
    }
    let by_case: Vec<_> = s
        .boards
        .iter()
        .filter(|b| b.name.eq_ignore_ascii_case(v.trim()))
        .collect();
    if by_case.len() == 1 {
        return Ok(by_case[0]);
    }
    if let Some(b) = v.parse::<u32>().ok().and_then(|id| s.board(BoardId(id))) {
        return Ok(b);
    }
    let names: Vec<String> = s.boards.iter().map(|b| format!("{:?}", b.name)).collect();
    Err(if by_case.len() > 1 {
        format!("--board {v:?}: more than one board has that name; give its id")
    } else {
        format!(
            "--board {v:?}: no such board; the boards are {}",
            names.join(", ")
        )
    })
}

/// The board the command works on and its first sub-board.
fn first(s: &Schedule) -> Result<(BoardId, SubBoardId), String> {
    let b = chosen_board(s)?;
    let sb = b.sub_boards.first().ok_or("the board has no sub-board")?;
    Ok((b.id, sb.id))
}

/// The board the command works on, to change.
fn chosen_board_mut(s: &mut Schedule) -> Result<&mut method_schedule_core::model::Board, String> {
    let id = chosen_board(s)?.id;
    Ok(s.boards
        .iter_mut()
        .find(|b| b.id == id)
        .expect("the board was just found"))
}

fn eighths(lengths: &[String]) -> Result<String, String> {
    let mut total = Eighths::ZERO;
    for s in lengths {
        let e: Eighths = s.parse().map_err(|err| format!("{s:?}: {err}"))?;
        total = total.checked_add(e).ok_or("total is too large")?;
    }
    Ok(format!("{total}\n"))
}

fn new(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule new FILE [--title TEXT]")?;
    if Path::new(path).exists() {
        return Err(format!("{path}: already exists"));
    }
    let title = flag(args, "--title").unwrap_or("Untitled");
    // --from TEMPLATE: the template's setup, no sheets, strips, events or red flags
    // (decision 85).
    let s = match flag(args, "--from") {
        Some(t) => {
            let mut s = load(t)?.to_template();
            s.title = title.to_string();
            s
        }
        None => Schedule::with_default_categories(title),
    };
    save(path, &s)?;
    Ok(format!("created {path}\n"))
}

/// `template FILE OUT`: writes FILE's setup as a template (decision 85).
fn template_cmd(args: &[String]) -> Result<String, String> {
    let [from, out] = positional(args, &[])[..] else {
        return Err("usage: method-schedule template FILE OUT.mtst".to_string());
    };
    // As the app and MCP: a template is always a `.mtst`, so it never passes for a
    // schedule (decision 85).
    if !out.to_ascii_lowercase().ends_with(".mtst") {
        return Err(format!("{out}: a template's name ends in .mtst"));
    }
    if Path::new(out).exists() {
        return Err(format!("{out}: already exists"));
    }
    let s = load(from)?;
    write_bytes(
        out,
        method_schedule_format::to_template_string(&s).as_bytes(),
    )?;
    Ok(format!("wrote the template {out}\n"))
}

fn sample_cmd(args: &[String]) -> Result<String, String> {
    const USAGE: &str = "usage: method-schedule sample FILE [--sheets N], or sample --demo FILE, or sample --field-station FILE";
    if args.first().is_some_and(|a| a == "--field-station") {
        // The suite's shared production, broken down: fixtures/field-station.mtsc byte for
        // byte (decision 215).
        let path = args.get(1).ok_or(USAGE)?;
        if Path::new(path).exists() {
            return Err(format!("{path}: already exists"));
        }
        save(path, &method_schedule_core::sample::field_station())?;
        return Ok(format!("created {path}, Field Station\n"));
    }
    if args.first().is_some_and(|a| a == "--demo") {
        // The app's Help > Open Sample, byte for byte (decisions.md, entry 198).
        let path = args.get(1).ok_or(USAGE)?;
        if Path::new(path).exists() {
            return Err(format!("{path}: already exists"));
        }
        save(path, &method_schedule_core::sample::demo())?;
        return Ok(format!("created {path}, the sample production\n"));
    }
    let path = args.first().ok_or(USAGE)?;
    if Path::new(path).exists() {
        return Err(format!("{path}: already exists"));
    }
    let n = match flag(args, "--sheets") {
        Some(v) => v
            .parse::<usize>()
            .map_err(|_| format!("{v:?} is not a whole number"))?,
        None => 1500,
    };
    if !(1..=20_000).contains(&n) {
        return Err("--sheets must be 1 to 20000".to_string());
    }
    save(path, &method_schedule_core::sample::sample(n))?;
    Ok(format!("created {path} with {}\n", count(n, "sheet")))
}

fn import(args: &[String]) -> Result<String, String> {
    let script_path = args
        .first()
        .ok_or("usage: method-schedule import SCRIPT [--into FILE] [--lines-per-page N]")?;
    let tags = args.iter().any(|a| a == "--tags");
    let script = beyond::read_script(script_path, tags, beyond::lines_per_page(args)?)?;
    let Some(into) = flag(args, "--into") else {
        return Ok(list_script(&script));
    };
    let mut s = if Path::new(into).exists() {
        load(into)?
    } else {
        Schedule::with_default_categories(script.title.as_deref().unwrap_or("Untitled"))
    };
    let cast = s.cast_or_add();
    let added = if tags {
        method_schedule_core::revise::import_tagged(&mut s, &script, cast)
    } else {
        s.import_script(&script, cast)
    }
    .map_err(|e| e.to_string())?;
    save(into, &s)?;
    Ok(format!(
        "added {} to {into}; they wait in the Boneyard\n",
        count(added.len(), "sheet")
    ))
}

fn list_script(script: &fountain::ImportedScript) -> String {
    let mut out = String::new();
    if let Some(title) = &script.title {
        out.push_str(&format!("{title}\n\n"));
    }
    let mut total = Eighths::ZERO;
    for s in &script.scenes {
        total += s.length;
        out.push_str(&format!(
            "{:>5}  p{:<3} {:>7}  {}\n",
            s.number.as_deref().unwrap_or("-"),
            s.page,
            s.length.to_string(),
            s.heading
        ));
        if !s.characters.is_empty() {
            out.push_str(&format!("{:20}{}\n", "", s.characters.join(", ")));
        }
    }
    out.push_str(&format!(
        "\n{}, {total} pages\n",
        count(script.scenes.len(), "scene")
    ));
    out
}

fn schedule_all(path: &str) -> Result<String, String> {
    let mut s = load(path)?;
    let (b, sb) = first(&s)?;
    let board = chosen_board(&s)?;
    let ids: Vec<_> = board.boneyard.iter().map(|x| x.id).collect();
    let end = board.sub_boards[0].strips.len();
    s.move_strips(
        b,
        &ids,
        Place {
            sub_board: Some(sb),
            index: end,
        },
    )
    .map_err(|e| e.to_string())?;
    save(path, &s)?;
    Ok(format!(
        "moved {} onto {}\n",
        count(ids.len(), "strip"),
        chosen_board(&s)?.sub_boards[0].name
    ))
}

fn start(args: &[String]) -> Result<String, String> {
    let [path, date] = args else {
        return Err("usage: method-schedule start FILE YYYY-MM-DD".to_string());
    };
    let d: Date = date.parse().map_err(|e| format!("{date:?}: {e}"))?;
    let mut s = load(path)?;
    first(&s)?;
    chosen_board_mut(&mut s)?.sub_boards[0].start = Some(d);
    save(path, &s)?;
    Ok(String::new())
}

/// The named layouts (decisions.md, entry 78): listed, saved to a layout file, or loaded
/// from one.
fn layouts_cmd(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule layouts FILE [--export OUT | --import IN]")?;
    let mut s = load(path)?;
    if let Some(out) = flag(args, "--export") {
        if s.layouts.layouts.is_empty() {
            return Err("the schedule has no named layouts".to_string());
        }
        if Path::new(out).exists() {
            return Err(format!("{out}: already exists"));
        }
        write_bytes(
            out,
            method_schedule_format::layouts_to_string(&s, &s.layouts.layouts).as_bytes(),
        )?;
        return Ok(format!("{} layouts saved\n", s.layouts.layouts.len()));
    }
    if let Some(input) = flag(args, "--import") {
        let read = method_schedule_format::layouts_from_str(&s, &read(input)?)
            .map_err(|e| format!("{input}: {e}"))?;
        let n = s.import_layouts(read).map_err(|e| e.to_string())?.len();
        s.check().map_err(|e| e.to_string())?;
        save(path, &s)?;
        return Ok(format!("{n} layouts added\n"));
    }
    let mut out = String::new();
    for l in &s.layouts.layouts {
        let users: Vec<&str> = s.boards_using(l.id).map(|b| b.name.as_str()).collect();
        out.push_str(&format!(
            "{}\t{}\t{}{}\n",
            l.name,
            l.body.kind().label(),
            l.body.text(&s),
            if users.is_empty() {
                String::new()
            } else {
                format!("\tboards: {}", users.join(", "))
            }
        ));
    }
    if out.is_empty() {
        out.push_str("no named layouts\n");
    }
    Ok(out)
}

fn breaks(args: &[String]) -> Result<String, String> {
    let path = args.first().ok_or(
        "usage: method-schedule breaks FILE --pages LEN | --minutes N | --seconds N | --remove",
    )?;
    if args.iter().any(|a| a == "--remove") {
        let mut s = load(path)?;
        let (b, sb) = first(&s)?;
        let n = remove_day_breaks(&mut s, b, sb).map_err(|e| e.to_string())?;
        if n > 0 {
            save(path, &s)?;
        }
        return Ok(format!("{} removed\n", count(n as usize, "day break")));
    }
    let number = |v: &str| {
        v.parse::<u32>()
            .map_err(|_| format!("{v:?} is not a whole number"))
    };
    let limit = if let Some(v) = flag(args, "--pages") {
        Limit::Pages(v.parse().map_err(|e| format!("{v:?}: {e}"))?)
    } else if let Some(v) = flag(args, "--minutes") {
        Limit::Time(Minutes(number(v)?))
    } else if let Some(v) = flag(args, "--seconds") {
        Limit::ScreenTime(Seconds(number(v)?))
    } else {
        return Err("give --pages, --minutes, --seconds or --remove".to_string());
    };
    // As the app and MCP: a limit of nothing would close a day after every strip.
    let zero = match limit {
        Limit::Pages(e) => e == Eighths::ZERO,
        Limit::Time(m) => m.0 == 0,
        Limit::ScreenTime(t) => t.0 == 0,
    };
    if zero {
        return Err("the limit per day must be more than 0".to_string());
    }
    let mut s = load(path)?;
    let (b, sb) = first(&s)?;
    auto_day_breaks(&mut s, b, sb, limit).map_err(|e| e.to_string())?;
    save(path, &s)?;
    let n = days(&s, b, sb).map_err(|e| e.to_string())?.days.len();
    Ok(format!("{}\n", count(n, "shoot day")))
}

/// `1 shoot day`, `3 shoot days`.
pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// `sort FILE FIELD[:desc],...`: the first sub-board sorted as Board > Sort Strips sorts
/// it (decision 122), then saved.
fn sort_cmd(args: &[String]) -> Result<String, String> {
    use method_schedule_core::model::{SortField, SortKey, StripKind, MAX_SORT_KEYS};
    let usage = "usage: method-schedule sort FILE FIELD[:desc],...";
    let (path, spec) = match args {
        [path, spec] => (path, spec),
        _ => return Err(usage.to_string()),
    };
    let mut keys: Vec<SortKey> = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, descending) = match part.split_once(':') {
            None => (part, false),
            Some((n, "desc")) => (n, true),
            Some((n, "asc")) => (n, false),
            Some((_, d)) => return Err(format!("{d:?} is not asc or desc")),
        };
        let field =
            SortField::from_name(name).ok_or_else(|| format!("{name:?} is not a sort field"))?;
        if keys.iter().any(|k| k.field == field) {
            return Err(format!("{name:?} is given twice"));
        }
        keys.push(SortKey { field, descending });
    }
    if keys.is_empty() || keys.len() > MAX_SORT_KEYS {
        return Err(format!("give 1 to {MAX_SORT_KEYS} fields; {usage}"));
    }
    let mut s = load(path)?;
    let (b, sb) = first(&s)?;
    s.sort_strips(b, Some(sb), &keys)
        .map_err(|e| e.to_string())?;
    save(path, &s)?;
    let n = chosen_board(&s)?.sub_boards[0]
        .strips
        .iter()
        .filter(|st| matches!(st.kind, StripKind::Sheet(_)))
        .count();
    Ok(format!(
        "sorted {}; day breaks removed, break the days again\n",
        count(n, "scene strip")
    ))
}

fn show(s: &Schedule) -> Result<String, String> {
    let mut out = format!("{}\n", s.title);
    let total: Eighths = s.sheets.iter().map(|h| h.pages).sum();
    out.push_str(&format!(
        "{}, {total} pages, {}\n",
        count(s.sheets.len(), "sheet"),
        count(s.elements.len(), "element")
    ));
    for b in &s.boards {
        out.push_str(&format!(
            "\nboard {}: {} in the Boneyard\n",
            b.name,
            b.boneyard.len()
        ));
        for sb in &b.sub_boards {
            let d = days(s, b.id, sb.id).map_err(|e| e.to_string())?;
            let start = sb
                .start
                .map_or("no start date".to_string(), |d| format!("starts {d}"));
            out.push_str(&format!(
                "  {}: {}, {} unscheduled, {start}\n",
                sb.name,
                count(d.days.len(), "shoot day"),
                count(d.unscheduled.len(), "strip")
            ));
        }
    }
    Ok(out)
}

/// A banner's style after its line, `  [bold, red]`, or nothing when plain.
fn style_note(st: &method_schedule_core::model::BannerStyle) -> String {
    if st.is_default() {
        String::new()
    } else {
        format!("  [{}]", st.text())
    }
}

fn hm(m: Minutes) -> String {
    format!("{}:{:02}", m.0 / 60, m.0 % 60)
}

fn list_days(s: &Schedule) -> Result<String, String> {
    let (b, sb) = first(s)?;
    let sub = &chosen_board(s)?.sub_boards[0];
    let d = days(s, b, sb).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for day in &d.days {
        let date = match sub.start {
            Some(st) => sub
                .calendar
                .date_of_day(st, day.number)
                .map_or("-".to_string(), |x| x.to_string()),
            None => "-".to_string(),
        };
        out.push_str(&format!(
            "day {:>3}  {date:>10}  {:>8} pages  {:>6}  {}\n",
            day.number,
            day.pages.to_string(),
            hm(day.time),
            count(day.strips.len(), "strip")
        ));
    }
    if !d.unscheduled.is_empty() {
        let n = d.unscheduled.len();
        out.push_str(&format!(
            "{} after the last day break {} not scheduled\n",
            count(n, "strip"),
            if n == 1 { "is" } else { "are" }
        ));
    }
    if out.is_empty() {
        out.push_str("no shoot days\n");
    }
    Ok(out)
}

/// For `--xlsx OUT`, writes the report's workbook to OUT; for `--csv`, returns its table
/// as CSV. `None` when neither was asked for, so the caller prints its text table.
fn export(
    args: &[String],
    build: impl FnOnce() -> Result<Workbook, ExportError>,
) -> Result<Option<String>, String> {
    if let Some(out) = flag(args, "--xlsx") {
        let wb = build().map_err(|e| e.to_string())?;
        let bytes = xlsx::write(&wb).map_err(|e| e.to_string())?;
        write_bytes(out, &bytes)?;
        return Ok(Some(format!("wrote {out}\n")));
    }
    if args.iter().any(|a| a == "--xlsx") {
        return Err("--xlsx needs a file name".to_string());
    }
    if args.iter().any(|a| a == "--csv") {
        let wb = build().map_err(|e| e.to_string())?;
        let sheet = wb.sheets.first().ok_or("nothing to export")?;
        let bytes = csv::write(&sheet.without(PAGES), csv::Delimiter::Comma, false);
        return String::from_utf8(bytes)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    Ok(None)
}

fn dood_cmd(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule dood FILE [--csv | --xlsx OUT]")?;
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    if let Some(out) = export(args, || export::dood_summary(&s, b, sb))? {
        return Ok(out);
    }
    let mut rows = dood(&s, b, sb).map_err(|e| e.to_string())?;
    // A category at a time, each under its name, as the elements list (DOD-01).
    method_schedule_core::dood::group_by_category(&s, &mut rows);
    let mut out = format!(
        "{:<6} {:<24} {:>5} {:>5} {:>11} {:>11} {:>6}\n",
        "ID", "ELEMENT", "WORK", "HOLD", "START", "FINISH", "TOTAL"
    );
    let date = |d: Option<Date>| d.map_or(String::new(), |d| d.to_string());
    let mut last_cat = None;
    for r in summary(&rows) {
        let e = s.element(r.element).ok_or("missing element")?;
        if last_cat != Some(e.category) {
            let name = s.category(e.category).map_or("", |c| c.name.as_str());
            out.push_str(&format!("{name}\n"));
            last_cat = Some(e.category);
        }
        let id = e.board_id.as_deref().unwrap_or("");
        out.push_str(&format!(
            "{id:<6} {:<24} {:>5} {:>5} {:>11} {:>11} {:>6}\n",
            e.name,
            r.work,
            r.hold,
            date(r.start),
            date(r.finish),
            r.total
        ));
    }
    Ok(out)
}

fn oneline_cmd(args: &[String]) -> Result<String, String> {
    let path = args.first().ok_or(
        "usage: method-schedule oneline FILE [--csv | --xlsx OUT] [--columns layout|fixed]",
    )?;
    let columns = match flag(args, "--columns") {
        None if args.iter().any(|a| a == "--columns") => {
            return Err("--columns needs layout or fixed".to_string())
        }
        None => export::OneLineColumns::default(),
        Some(c) => export::OneLineColumns::from_name(c)
            .ok_or_else(|| format!("--columns is layout or fixed, not {c:?}"))?,
    };
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    let cast = s.cast();
    if let Some(out) = export(args, || export::one_line(&s, b, sb, cast, columns))? {
        return Ok(out);
    }
    let days = one_line(&s, b, sb, cast).map_err(|e| e.to_string())?;
    let date = |d: Option<Date>| d.map_or(String::new(), |d| d.to_string());
    let mut out = String::new();
    for day in &days {
        out.push_str(&format!("Day {}  {}\n", day.number, date(day.date)));
        for line in &day.lines {
            match line {
                OneLine::Scene {
                    sheet,
                    scenes,
                    int_ext,
                    set,
                    time_of_day,
                    pages,
                    cast,
                    ..
                } => {
                    out.push_str(&format!(
                        "  {scenes:<6} {int_ext:<8} {:<28} {time_of_day:<10} {:>7}  {}{}\n",
                        set,
                        pages.to_string(),
                        cast.join(", "),
                        omitted_note(&s, *sheet)
                    ));
                }
                OneLine::Banner {
                    text,
                    duration,
                    style,
                } => {
                    out.push_str(&format!(
                        "  -- {text} ({}){}\n",
                        hm(*duration),
                        style_note(style)
                    ));
                }
            }
        }
        out.push_str(&format!(
            "  end of day {}: {} pages, {}\n\n",
            day.number,
            day.pages,
            hm(day.time)
        ));
    }
    Ok(out)
}

/// `  (omitted)` after a scene a script revision left out: listed, counting nowhere
/// (decision 91), as the app's reports strike it.
fn omitted_note(s: &Schedule, sheet: method_schedule_core::model::SheetId) -> &'static str {
    if s.plan.is_omitted(sheet) {
        "  (omitted)"
    } else {
        ""
    }
}

fn shooting_cmd(path: &str) -> Result<String, String> {
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    let date = |d: Option<Date>| d.map_or(String::new(), |d| d.to_string());
    let mut out = String::new();
    for day in shooting_schedule(&s, b, sb).map_err(|e| e.to_string())? {
        out.push_str(&format!("Day {}  {}\n", day.number, date(day.date)));
        for line in &day.lines {
            match line {
                ShootingLine::Scene(sc) => {
                    out.push_str(&format!(
                        "  Scene {}  {} {} {}  {} pages{}\n",
                        sc.scenes,
                        sc.int_ext,
                        sc.set,
                        sc.time_of_day,
                        sc.pages,
                        omitted_note(&s, sc.sheet)
                    ));
                    if !sc.synopsis.is_empty() {
                        out.push_str(&format!("    {}\n", sc.synopsis));
                    }
                    for (cat, names) in &sc.elements {
                        let cat = s.category(*cat).map_or("", |c| c.name.as_str());
                        out.push_str(&format!("    {cat}: {}\n", names.join(", ")));
                    }
                }
                ShootingLine::Banner {
                    text,
                    duration,
                    style,
                } => out.push_str(&format!(
                    "  -- {text} ({}){}\n",
                    hm(*duration),
                    style_note(style)
                )),
            }
        }
        out.push_str(&format!(
            "  end of day {}: {} pages, {}\n\n",
            day.number,
            day.pages,
            hm(day.time)
        ));
    }
    Ok(out)
}

/// Today's local date, for stamping exchange files.
fn today() -> Option<Date> {
    method_schedule_format::exchange::local_today()
}

fn budget_cmd(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule budget FILE [--out OUT]")?;
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    let text = method_schedule_format::exchange::to_string(&s, b, sb, today())
        .map_err(|e| e.to_string())?;
    match flag(args, "--out") {
        Some(out) => {
            write_bytes(out, text.as_bytes())?;
            Ok(format!("wrote {out}\n"))
        }
        None if args.iter().any(|a| a == "--out") => Err("--out needs a file name".to_string()),
        None => Ok(text),
    }
}

fn totals_cmd(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule totals FILE [--csv | --xlsx OUT]")?;
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    if let Some(out) = export(args, || export::daily_totals(&s, b, sb))? {
        return Ok(out);
    }
    let totals = daily_category_totals(&s, b, sb).map_err(|e| e.to_string())?;
    let mut out = format!("{:<5} {:<11}", "DAY", "DATE");
    for c in &s.categories {
        out.push_str(&format!(" {:>10}", c.name));
    }
    out.push('\n');
    for t in &totals {
        out.push_str(&format!(
            "{:<5} {:<11}",
            t.number,
            t.date.map_or(String::new(), |d| d.to_string())
        ));
        for (c, n) in &t.counts {
            // A partial total (a quantity left out of the day's count) gets an asterisk.
            let n = if t.partial.contains(c) {
                format!("{n}*")
            } else {
                n.to_string()
            };
            out.push_str(&format!(" {n:>10}"));
        }
        out.push('\n');
    }
    if totals.iter().any(|t| !t.partial.is_empty()) {
        out.push_str("* partial: a quantity a sheet marks not to count daily is left out\n");
    }
    Ok(out)
}

/// Parses `mcp`'s flags: at least one `--root`, and `--read-only`.
fn mcp_config(args: &[String]) -> Result<mcp::Config, String> {
    let usage = "usage: method-schedule mcp --root DIR [--root DIR...] [--read-only]";
    let mut roots = Vec::new();
    let mut read_only = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => roots.push(std::path::PathBuf::from(it.next().ok_or(usage)?)),
            "--read-only" => read_only = true,
            other => return Err(format!("unknown option {other:?}; {usage}")),
        }
    }
    if roots.is_empty() {
        return Err(format!("give at least one --root; {usage}"));
    }
    Ok(mcp::Config { roots, read_only })
}

fn mcp_cmd(args: &[String]) -> Result<String, String> {
    mcp::serve_stdio(mcp_config(args)?)?;
    Ok(String::new())
}

/// The line in a `.gitattributes` file that sends schedules to the driver.
const GIT_ATTRIBUTE: &str = "*.mtsc merge=method-schedule";

/// The driver line git runs: base, ours (replaced by the result), theirs and the path.
fn driver_line(theirs: bool) -> String {
    format!(
        "method-schedule merge-driver %O %A %B %P{}",
        if theirs { " --theirs" } else { "" }
    )
}

/// `text` with the driver's attribute line added, or `None` when it is there already.
fn with_attribute(text: &str) -> Option<String> {
    if text.lines().any(|l| l.trim() == GIT_ATTRIBUTE) {
        return None;
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(GIT_ATTRIBUTE);
    out.push('\n');
    Some(out)
}

/// `method-schedule git-setup [--global] [--theirs]`: the driver in git's config, and the
/// attribute line in this repository's `.gitattributes` (with `--global`, in git's own
/// attributes file, `core.attributesfile` or its default).
fn git_setup_cmd(args: &[String]) -> Result<String, String> {
    let mut global = false;
    let mut theirs = false;
    for a in args {
        match a.as_str() {
            "--global" => global = true,
            "--theirs" => theirs = true,
            other => return Err(format!("git-setup doesn't take {other}")),
        }
    }
    let git = |a: &[&str]| -> Result<String, String> {
        let out = std::process::Command::new("git")
            .args(a)
            .output()
            .map_err(|_| "git isn't installed or isn't on the PATH".to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let scope: &[&str] = if global { &["--global"] } else { &[] };
    let attributes = if global {
        match git(&["config", "--global", "--get", "core.attributesfile"]) {
            Ok(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => {
                let base = std::env::var_os("XDG_CONFIG_HOME")
                    .map(std::path::PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config"))
                    })
                    .ok_or("can't find your home folder for git's attributes file")?;
                base.join("git").join("attributes")
            }
        }
    } else {
        let top = git(&["rev-parse", "--show-toplevel"]).map_err(|_| {
            "this folder isn't in a git repository; run it there, or use --global".to_string()
        })?;
        std::path::Path::new(&top).join(".gitattributes")
    };
    let set = |key: &str, value: &str| {
        let mut a = vec!["config"];
        a.extend_from_slice(scope);
        a.extend([key, value]);
        git(&a).map(|_| ())
    };
    set("merge.method-schedule.name", "Method Schedule merge")?;
    set("merge.method-schedule.driver", &driver_line(theirs))?;
    let old = std::fs::read_to_string(&attributes).unwrap_or_default();
    let added = match with_attribute(&old) {
        Some(text) => {
            if let Some(dir) = attributes.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            std::fs::write(&attributes, text)
                .map_err(|e| format!("{}: {e}", attributes.display()))?;
            true
        }
        None => false,
    };
    Ok(format!(
        "git merges schedules with Method Schedule {}: the driver is in git's {} config, and {} {}.\n\
         When a merge leaves conflicts, the file is still a valid schedule with each settled for {}; check them, then git add it.\n",
        if global { "in every repository" } else { "in this repository" },
        if global { "global" } else { "repository" },
        attributes.display(),
        if added { "now sends .mtsc files to it" } else { "already sent .mtsc files to it" },
        if theirs { "theirs" } else { "ours" },
    ))
}

fn diff_cmd(args: &[String]) -> Result<String, String> {
    let [a, b] = args else {
        return Err("usage: method-schedule diff OLD NEW".to_string());
    };
    Ok(diff(&load(a)?, &load(b)?).to_text())
}

/// What `method-schedule merge` and `method-schedule merge-driver` print, and whether conflicts were
/// settled (exit 1).
struct MergeOut {
    stdout: String,
    stderr: String,
    conflicts: bool,
}

fn merge_cmd(args: &[String]) -> Result<MergeOut, String> {
    let driver = args.first().is_some_and(|a| a == "merge-driver");
    let rest = args.get(1..).unwrap_or(&[]);
    let usage = if driver {
        "usage: method-schedule merge-driver BASE OURS THEIRS [PATH] [--theirs]"
    } else {
        "usage: method-schedule merge BASE OURS THEIRS [--out FILE] [--theirs]"
    };
    for a in rest.iter().filter(|a| a.starts_with("--")) {
        if !matches!(a.as_str(), "--theirs" | "--out") || (driver && a == "--out") {
            return Err(format!("unknown option {a:?}; {usage}"));
        }
    }
    let out = flag(rest, "--out");
    if out.is_none() && rest.iter().any(|a| a == "--out") {
        return Err("--out needs a file name".to_string());
    }
    let files = positional(rest, &["--out"]);
    let wanted = if driver { 3..=4 } else { 3..=3 };
    if !wanted.contains(&files.len()) {
        return Err(usage.to_string());
    }
    let side = if rest.iter().any(|a| a == "--theirs") {
        Side::Theirs
    } else {
        Side::Ours
    };
    // git names the file by its path in the repository; the inputs are temporary files.
    let shown = files.get(3).copied().unwrap_or(files[1]);
    if driver && fs::metadata(files[0]).is_ok_and(|m| m.len() == 0) {
        return Err(format!(
            "{shown}: no common base (both branches added it); merge it by hand"
        ));
    }
    let named = |e: String| if driver { format!("{shown}: {e}") } else { e };
    let base = load(files[0]).map_err(named)?;
    let ours = load(files[1]).map_err(named)?;
    let theirs = load(files[2]).map_err(named)?;
    let m = merge_with(&base, &ours, &theirs, side, &BTreeMap::new());
    let report = merge_report(&m, side);
    let conflicts = !m.conflicts.is_empty();
    if driver {
        save(files[1], &m.schedule)?;
        let stderr = if conflicts || !m.fallbacks.is_empty() {
            format!("method-schedule merge-driver: {shown}\n{report}")
        } else {
            String::new()
        };
        return Ok(MergeOut {
            stdout: String::new(),
            stderr,
            conflicts,
        });
    }
    match out {
        Some(path) => {
            // Never over one of the schedules being merged: that is merge-driver's job.
            let same = |a: &str, b: &str| match (fs::canonicalize(a), fs::canonicalize(b)) {
                (Ok(x), Ok(y)) => x == y,
                _ => a == b,
            };
            if files.iter().any(|f| same(f, path)) {
                return Err(format!(
                    "{path}: --out can't be one of the schedules being merged"
                ));
            }
            save(path, &m.schedule)?;
            Ok(MergeOut {
                stdout: format!("wrote {path}\n{report}"),
                stderr: String::new(),
                conflicts,
            })
        }
        None => Ok(MergeOut {
            stdout: method_schedule_format::to_string(&m.schedule),
            stderr: report,
            conflicts,
        }),
    }
}

fn merge_report(m: &Merged, side: Side) -> String {
    let mut out = String::new();
    if m.renumbered > 0 {
        out.push_str(&format!(
            "{} added on both sides with the same ids: theirs got new ids\n",
            if m.renumbered == 1 {
                "1 thing".to_string()
            } else {
                format!("{} things", m.renumbered)
            }
        ));
    }
    if !m.fallbacks.is_empty() {
        out.push_str("to keep the schedule whole:\n");
        for f in &m.fallbacks {
            out.push_str(&format!("  {f}\n"));
        }
    }
    let n = m.conflicts.len();
    if n == 0 {
        out.push_str("no conflicts\n");
    } else {
        out.push_str(&format!(
            "{n} conflict{}, settled for {}:\n",
            if n == 1 { "" } else { "s" },
            side.key()
        ));
        for c in &m.conflicts {
            out.push_str(&format!("  {} (took {})\n", c.text, c.taken.key()));
        }
    }
    out
}

fn explain_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule explain FILE ELEMENT [--category CAT]";
    let [path, name] = positional(args, &["--category"])[..] else {
        return Err(usage.to_string());
    };
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    let found: Vec<ElementId> = match flag(args, "--category") {
        Some(c) => {
            let cat = s.category_named(c).ok_or(format!("no category {c:?}"))?;
            s.element_named(cat, name).into_iter().collect()
        }
        None if args.iter().any(|a| a == "--category") => {
            return Err("--category needs a name".to_string())
        }
        None => s
            .categories
            .iter()
            .filter_map(|c| s.element_named(c.id, name))
            .collect(),
    };
    match found[..] {
        [] => Err(format!("no element {name:?}")),
        [id] => Ok(explain(&s, b, sb, id).map_err(|e| e.to_string())?.to_text()),
        _ => {
            let cats: Vec<String> = found
                .iter()
                .filter_map(|e| s.element(*e))
                .filter_map(|e| s.category(e.category))
                .map(|c| c.name.clone())
                .collect();
            Err(format!(
                "{name:?} is in more than one category ({}); pick one with --category",
                cats.join(", ")
            ))
        }
    }
}

/// The search `find` and `replace` are given: `--field`, `--case`, `--word`,
/// `--sub-board`, `--where` and `--any` around `TEXT`. Unknown options are refused.
/// A `--where` value may begin with `--`; it is read as the option's value.
fn query(s: &Schedule, args: &[String], text: &str, extra: &[&str]) -> Result<Query, String> {
    for a in args.iter().filter(|a| a.starts_with("--")) {
        let known = [
            "--field",
            "--case",
            "--word",
            "--sub-board",
            "--where",
            "--any",
        ];
        if !known.contains(&a.as_str()) && !extra.contains(&a.as_str()) {
            return Err(format!("unknown option {a:?}"));
        }
    }
    let field =
        match flag(args, "--field") {
            Some(f) => Some(FieldFilter::parse(f).ok_or_else(|| {
                format!("--field {f:?}: one of {}", FieldFilter::keys().join(", "))
            })?),
            None if args.iter().any(|a| a == "--field") => {
                return Err("--field needs a name".to_string())
            }
            None => None,
        };
    let scope = match flag(args, "--sub-board") {
        Some(v) => {
            let id: u32 = v
                .parse()
                .map_err(|_| format!("--sub-board {v:?}: not an id"))?;
            let b = s
                .boards
                .iter()
                .find(|b| b.sub_boards.iter().any(|x| x.id.0 == id))
                .ok_or(format!("no sub-board {id}"))?;
            Scope::SubBoard {
                board: b.id,
                sub_board: SubBoardId(id),
            }
        }
        None if args.iter().any(|a| a == "--sub-board") => {
            return Err("--sub-board needs an id".to_string())
        }
        None => Scope::Schedule,
    };
    // --where FIELD:OP:VALUE, as often as needed (an advanced search, decision 77).
    let mut conditions = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a != "--where" {
            continue;
        }
        let w = it.next().ok_or("--where needs FIELD:OP:VALUE")?;
        let mut parts = w.splitn(3, ':');
        let (f, op, value) = (
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
        );
        let field = CondField::parse(f).ok_or_else(|| {
            format!(
                "--where {w:?}: the field is one of {}",
                CondField::keys().join(", ")
            )
        })?;
        let op = Op::parse(op)
            .ok_or_else(|| format!("--where {w:?}: the op is one of {}", Op::KEYS.join(", ")))?;
        conditions
            .push(Condition::new(field, op, value).map_err(|e| format!("--where {w:?}: {e}"))?);
    }
    if text.is_empty() && conditions.is_empty() {
        return Err("give the text to find".to_string());
    }
    Ok(Query {
        text: text.to_string(),
        match_case: args.iter().any(|a| a == "--case"),
        whole_word: args.iter().any(|a| a == "--word"),
        field,
        scope,
        conditions,
        any: args.iter().any(|a| a == "--any"),
    })
}

fn find_cmd(args: &[String]) -> Result<String, String> {
    let usage =
        "usage: method-schedule find FILE TEXT [--field F] [--case] [--word] [--sub-board ID] \
         [--where FIELD:OP:VALUE]... [--any]";
    let [path, text] = positional(args, &["--field", "--sub-board", "--where"])[..] else {
        return Err(usage.to_string());
    };
    let s = load(path)?;
    let q = query(&s, args, text, &[])?;
    let hits = finder::find(&s, &q).map_err(|e| e.to_string())?;
    let mut out = format!(
        "{} result{} for {text:?}\n",
        hits.len(),
        if hits.len() == 1 { "" } else { "s" }
    );
    for (i, h) in hits.iter().enumerate() {
        out.push_str(&format!("{:>4}  {}\n", i + 1, h.context));
    }
    Ok(out)
}

/// `--pick 1,3,4`: result numbers from 1, each within `count`.
fn picks(v: &str, count: usize) -> Result<Vec<usize>, String> {
    v.split(',')
        .map(|p| {
            let n: usize = p
                .trim()
                .parse()
                .map_err(|_| format!("--pick {p:?}: not a result number"))?;
            if n == 0 || n > count {
                return Err(format!(
                    "--pick {n}: there {} {count} result{}",
                    if count == 1 { "is" } else { "are" },
                    if count == 1 { "" } else { "s" }
                ));
            }
            Ok(n - 1)
        })
        .collect()
}

fn replace_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule replace FILE TEXT NEW [--field F] [--case] [--word] \
                 [--sub-board ID] [--where FIELD:OP:VALUE]... [--any] (--all | --pick N,N)";
    let [path, text, with] = positional(args, &["--field", "--sub-board", "--pick", "--where"])[..]
    else {
        return Err(usage.to_string());
    };
    let s = load(path)?;
    let q = query(&s, args, text, &["--all", "--pick"])?;
    let hits = finder::find(&s, &q).map_err(|e| e.to_string())?;
    let all = args.iter().any(|a| a == "--all");
    let chosen = match (all, flag(args, "--pick")) {
        (true, None) if !args.iter().any(|a| a == "--pick") => {
            hits.iter().map(|h| h.location).collect::<Vec<_>>()
        }
        (false, Some(v)) => picks(v, hits.len())?
            .into_iter()
            .map(|i| hits[i].location)
            .collect(),
        (false, None) if args.iter().any(|a| a == "--pick") => {
            return Err("--pick needs result numbers, like --pick 1,3".to_string())
        }
        _ => return Err(format!("give --all or --pick N,N; {usage}")),
    };
    if chosen.is_empty() {
        return Ok(format!("no results for {text:?}; {path} not written\n"));
    }
    let r = finder::replace(&s, &q, with, &chosen).map_err(|e| e.to_string())?;
    if r.changes.is_empty() {
        return Ok(format!(
            "nothing changed (the text is the same); {path} not written\n"
        ));
    }
    save(path, &r.schedule)?;
    let n = r.changes.len();
    let mut out = format!(
        "replaced in {n} place{} in {path}\n",
        if n == 1 { "" } else { "s" }
    );
    for c in &r.changes {
        out.push_str(&format!(
            "  {}: {:?} -> {:?}\n",
            finder::label(&r.schedule, &c.location),
            c.before,
            c.after
        ));
    }
    Ok(out)
}

fn conflicts_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule conflicts FILE [--board NAME]";
    if let Some(a) = args.iter().find(|a| a.starts_with("--")) {
        return Err(format!("unknown option {a:?}; {usage}"));
    }
    let [path] = positional(args, &[])[..] else {
        return Err(usage.to_string());
    };
    let s = load(path)?;
    let board = first(&s)?.0;
    let c = conflicts(&s, board).map_err(|e| e.to_string())?;
    let b = s.board(board).ok_or("no such board")?;
    let sub_name = |id: SubBoardId| {
        b.sub_boards
            .iter()
            .find(|x| x.id == id)
            .map_or(String::new(), |x| x.name.clone())
    };
    let element = |id: ElementId| {
        s.element(id).map_or(String::new(), |e| {
            let cat = s.category(e.category).map_or("", |c| c.name.as_str());
            match &e.board_id {
                Some(n) => format!("{} ({cat} {n})", e.name),
                None => format!("{} ({cat})", e.name),
            }
        })
    };
    let booking = |bk: &Booking| {
        let scenes: Vec<String> = bk
            .sheets
            .iter()
            .filter_map(|h| s.sheet(*h))
            .map(|h| {
                if h.scenes.is_empty() {
                    format!("sheet {}", h.id.0)
                } else {
                    format!("scene {}", h.scenes)
                }
            })
            .collect();
        format!(
            "{} day {} ({})",
            sub_name(bk.sub_board),
            bk.day,
            scenes.join(", ")
        )
    };
    let n = c.count();
    let mut out = format!(
        "board {:?}: {n} conflict{}\n",
        b.name,
        if n == 1 { "" } else { "s" }
    );
    for x in &c.conflicts {
        let list: Vec<String> = x.bookings.iter().map(booking).collect();
        out.push_str(&format!(
            "  {}  {}: {}\n",
            x.date,
            element(x.element),
            list.join("; ")
        ));
    }
    if !c.event_on_work.is_empty() {
        out.push_str("events on work days (conflicts):\n");
        for x in &c.event_on_work {
            let ev = b
                .sub_boards
                .iter()
                .find(|sb| sb.id == x.sub_board)
                .and_then(|sb| sb.calendar.events.get(x.event));
            let kind = match ev.map(|e| &e.kind) {
                Some(method_schedule_core::model::EventKind::Travel) => "travel",
                Some(method_schedule_core::model::EventKind::Fitting) => "fitting",
                Some(method_schedule_core::model::EventKind::Rehearsal) => "rehearsal",
                _ => "event",
            };
            let what = if x.name.is_empty() {
                kind.to_string()
            } else {
                format!("{kind} {:?}", x.name)
            };
            out.push_str(&format!(
                "  {}  {}: {what} on {} day {}, a day it works there\n",
                x.date,
                element(x.element),
                sub_name(x.sub_board),
                x.day
            ));
        }
    }
    if !c.event_clashes.is_empty() {
        out.push_str("event clashes across sub-boards (informational, not conflicts):\n");
        for x in &c.event_clashes {
            let mut list: Vec<String> = x
                .events
                .iter()
                .map(|e| {
                    let ev = b
                        .sub_boards
                        .iter()
                        .find(|sb| sb.id == e.sub_board)
                        .and_then(|sb| sb.calendar.events.get(e.index));
                    let title = ev.map_or_else(|| e.name.clone(), |ev| ev.title());
                    format!("event {title:?} on {}", sub_name(e.sub_board))
                })
                .collect();
            list.extend(
                x.bookings
                    .iter()
                    .map(|bk| format!("work on {}", booking(bk))),
            );
            out.push_str(&format!(
                "  {}  {}: {}\n",
                x.date,
                element(x.element),
                list.join("; ")
            ));
        }
    }
    if !c.undated.is_empty() {
        let names: Vec<String> = c.undated.iter().map(|id| sub_name(*id)).collect();
        out.push_str(&format!(
            "not compared (no start date): {}\n",
            names.join(", ")
        ));
    }
    Ok(out)
}

/// A sheet id typed on the command line.
fn sheet_arg(v: &str) -> Result<SheetId, String> {
    v.parse()
        .map(SheetId)
        .map_err(|_| format!("{v:?}: not a sheet id"))
}

/// A sheet as people read it: `sheet 4 (scene 12)`.
fn sheet_label(s: &Schedule, id: SheetId) -> String {
    match s.sheet(id).map(|h| h.scenes.trim()) {
        Some(sc) if sc.contains(',') => format!("sheet {} (scenes {sc})", id.0),
        Some(sc) if !sc.is_empty() => format!("sheet {} (scene {sc})", id.0),
        _ => format!("sheet {}", id.0),
    }
}

fn merge_sheets_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule merge-sheets FILE SURVIVOR OTHER...";
    if let Some(a) = args.iter().find(|a| a.starts_with("--")) {
        return Err(format!("unknown option {a:?}; {usage}"));
    }
    let [path, survivor, others @ ..] = args else {
        return Err(usage.to_string());
    };
    if others.is_empty() {
        return Err(format!("give at least one sheet to merge in; {usage}"));
    }
    let survivor = sheet_arg(survivor)?;
    let others = others
        .iter()
        .map(|o| sheet_arg(o))
        .collect::<Result<Vec<_>, _>>()?;
    let before = load(path)?;
    let mut s = before.clone();
    let r = s
        .merge_sheets(survivor, &others)
        .map_err(|e| format!("{e}; {path} not written"))?;
    s.check().map_err(|e| format!("{e}; {path} not written"))?;
    save(path, &s)?;
    let names: Vec<String> = r
        .merged
        .iter()
        .map(|id| sheet_label(&before, *id))
        .collect();
    let mut out = format!(
        "merged {} into {} in {path}\n",
        names.join(", "),
        sheet_label(&s, survivor)
    );
    for c in &r.combined {
        out.push_str(&format!("  {}: {:?} -> {:?}\n", c.field, c.before, c.after));
    }
    if !r.elements_added.is_empty() {
        let els: Vec<String> = r
            .elements_added
            .iter()
            .filter_map(|e| s.element(*e))
            .map(|e| {
                let cat = s.category(e.category).map_or("", |c| c.name.as_str());
                format!("{} ({cat})", e.name)
            })
            .collect();
        out.push_str(&format!("  elements added: {}\n", els.join(", ")));
    }
    if r.dropped.is_empty() {
        out.push_str("  nothing dropped\n");
    }
    for d in &r.dropped {
        out.push_str(&format!(
            "  dropped {} of {}: {:?}\n",
            d.field,
            sheet_label(&before, d.sheet),
            d.value
        ));
    }
    Ok(out)
}

fn duplicate_sheet_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule duplicate-sheet FILE SHEET";
    let [path, sheet] = args else {
        return Err(usage.to_string());
    };
    if sheet.starts_with("--") {
        return Err(format!("unknown option {sheet:?}; {usage}"));
    }
    let sheet = sheet_arg(sheet)?;
    let mut s = load(path)?;
    let copy = s
        .duplicate_sheet(sheet)
        .map_err(|e| format!("{e}; {path} not written"))?;
    s.check().map_err(|e| format!("{e}; {path} not written"))?;
    save(path, &s)?;
    let mut out = format!(
        "duplicated {} as sheet {} in {path}\n",
        sheet_label(&s, sheet),
        copy.0
    );
    for b in &s.boards {
        let Some(p) = s.sheet_place(b.id, copy) else {
            continue;
        };
        let list = match p.sub_board {
            Some(sub) => b
                .sub_boards
                .iter()
                .find(|x| x.id == sub)
                .map_or(String::new(), |x| x.name.clone()),
            None => "Boneyard".to_string(),
        };
        out.push_str(&format!(
            "  board {}: {list}, position {}\n",
            b.name,
            p.index + 1
        ));
    }
    Ok(out)
}

/// An id given after `name`, if the flag is there.
fn id_flag(args: &[String], name: &str) -> Result<Option<u32>, String> {
    match flag(args, name) {
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| format!("{name} {v:?}: not an id")),
        None if args.iter().any(|a| a == name) => Err(format!("{name} needs an id")),
        None => Ok(None),
    }
}

fn redflags_cmd(args: &[String]) -> Result<String, String> {
    let usage = "usage: method-schedule redflags FILE [--board NAME] [--sub-board ID]";
    for a in args.iter().filter(|a| a.starts_with("--")) {
        if a != "--sub-board" {
            return Err(format!("unknown option {a:?}; {usage}"));
        }
    }
    let [path] = positional(args, &["--sub-board"])[..] else {
        return Err(usage.to_string());
    };
    let s = load(path)?;
    let (board, sub) = match id_flag(args, "--sub-board")? {
        None => first(&s)?,
        Some(sb) => {
            let sb = SubBoardId(sb);
            let on = s.board_of_sub(sb).ok_or(format!("no sub-board {}", sb.0))?;
            if board_given() && first(&s)?.0 != on {
                let b = chosen_board(&s)?;
                return Err(format!("board {:?} has no sub-board {}", b.name, sb.0));
            }
            (on, sb)
        }
    };
    let r = red_flags(&s, board, sub).map_err(|e| e.to_string())?;
    let b = s.board(board).ok_or("no such board")?;
    let sb = b
        .sub_boards
        .iter()
        .find(|x| x.id == sub)
        .ok_or("no such sub-board")?;
    let total = s.elements.iter().map(|e| e.red_flags.len()).sum::<usize>();
    let n = r.clashes.len();
    let mut out = format!(
        "{:?} (board {:?}): {n} red flag clash{} ({total} red flag{} in the schedule)\n",
        sb.name,
        b.name,
        if n == 1 { "" } else { "es" },
        if total == 1 { "" } else { "s" },
    );
    for c in &r.clashes {
        let element = s.element(c.element).map_or(String::new(), |e| {
            let cat = s.category(e.category).map_or("", |c| c.name.as_str());
            match &e.board_id {
                Some(n) => format!("{} ({cat} {n})", e.name),
                None => format!("{} ({cat})", e.name),
            }
        });
        let (_, f) = s.red_flag(c.flag).ok_or("no such red flag")?;
        let kind = s.red_flag_type(c.kind).map_or("", |t| t.name.as_str());
        let mut what = vec![
            if c.hard { "hard" } else { "soft" }.to_string(),
            f.dates.text(),
        ];
        if !f.note.is_empty() {
            what.push(format!("{:?}", f.note));
        }
        let mut doing = Vec::new();
        if let Some(day) = c.day {
            let scenes: Vec<String> = c
                .sheets
                .iter()
                .filter_map(|h| s.sheet(*h))
                .map(|h| {
                    if h.scenes.is_empty() {
                        format!("sheet {}", h.id.0)
                    } else {
                        format!("scene {}", h.scenes)
                    }
                })
                .collect();
            doing.push(format!("works day {day} ({})", scenes.join(", ")));
        }
        for i in &c.events {
            if let Some(ev) = sb.calendar.events.get(*i) {
                let kind = match &ev.kind {
                    method_schedule_core::model::EventKind::Other(_) => "event".to_string(),
                    k => k.label().to_lowercase(),
                };
                if ev.name.is_empty() {
                    doing.push(kind.to_string());
                } else {
                    doing.push(format!("{kind} {:?}", ev.name));
                }
            }
        }
        out.push_str(&format!(
            "  {}  {element}: {kind} ({}): {}\n",
            c.date,
            what.join(", "),
            doing.join("; ")
        ));
    }
    if r.undated {
        out.push_str("strips not checked: the sub-board has no start date, so only events were\n");
    }
    Ok(out)
}

fn elements_cmd(args: &[String]) -> Result<String, String> {
    let path = args
        .first()
        .ok_or("usage: method-schedule elements FILE [--csv | --xlsx OUT]")?;
    let s = load(path)?;
    let (b, sb) = first(&s)?;
    if let Some(out) = export(args, || export::element_list(&s, b, sb))? {
        return Ok(out);
    }
    let list = element_list(&s, b, sb).map_err(|e| e.to_string())?;
    let date = |d: Option<Date>| d.map_or(String::new(), |d| d.to_string());
    let mut out = String::new();
    let mut last_cat = None;
    for l in &list {
        let cat = s.category(l.category).map_or("", |c| c.name.as_str());
        let id = l.board_id.as_deref().unwrap_or("");
        if last_cat != Some(l.category) {
            out.push_str(&format!("{cat}\n"));
            last_cat = Some(l.category);
        }
        out.push_str(&format!(
            "  {id:<6} {:<24} {:>3} sheets {:>8} pages {:>3} days  {} {}\n",
            l.name,
            l.sheets,
            l.pages.to_string(),
            l.days,
            date(l.start),
            date(l.finish)
        ));
        // Custom property values, one line each (inventory section 4).
        for p in s.element(l.element).map_or(&[][..], |e| &e.properties[..]) {
            out.push_str(&format!("         {}: {}\n", p.name, p.value));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_str(args: &[&str]) -> Result<String, String> {
        run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    /// fixtures/field-station.mtsc is exactly what `sample --field-station` writes. When the
    /// sample or the format changes, write it again (delete the file first):
    ///   cargo run -p method-schedule-cli -- sample --field-station fixtures/field-station.mtsc
    #[test]
    fn the_field_station_fixture_is_the_engines_sample() {
        let file = include_str!("../../../fixtures/field-station.mtsc");
        let made =
            method_schedule_format::to_string(&method_schedule_core::sample::field_station());
        assert_eq!(file, made);
        method_schedule_format::from_str(file)
            .unwrap()
            .check()
            .unwrap();
    }

    /// fixtures/field-station.budget.json is Field Station's budget exchange file (version
    /// 1), the one Method Budget budgets: the same bytes the sample gives today, stamped with
    /// the file's own date. Write it again when the sample changes:
    ///   cargo run -p method-schedule-cli -- budget fixtures/field-station.mtsc --out fixtures/field-station.budget.json
    #[test]
    fn the_field_station_exchange_file_is_the_samples() {
        let file = include_str!("../../../fixtures/field-station.budget.json");
        let json: serde_json::Value = serde_json::from_str(file).unwrap();
        let generated: method_schedule_core::Date =
            json["generated"].as_str().unwrap().parse().unwrap();
        let s = method_schedule_core::sample::field_station();
        let (b, sb) = (s.boards[0].id, s.boards[0].sub_boards[0].id);
        let made = method_schedule_format::exchange::to_string(&s, b, sb, Some(generated)).unwrap();
        assert_eq!(file, made);
        assert_eq!(json["version"], 1);
    }

    #[test]
    fn version_names_the_command_and_the_product() {
        let want = format!(
            "method-schedule {} (Method Schedule)\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(run_str(&["--version"]).unwrap(), want);
        assert_eq!(run_str(&["-V"]).unwrap(), want);
    }

    #[test]
    fn reports_and_settings_of_the_report_options() {
        let dir = std::env::temp_dir().join(format!("ms-cli-more-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("h.mtsc").to_string_lossy().into_owned();
        run_str(&["new", &f, "--title", "ZTEST Harbor"]).unwrap();
        run_str(&["import", fixture(), "--into", &f]).unwrap();
        run_str(&["schedule", &f]).unwrap();
        run_str(&["start", &f, "2027-03-01"]).unwrap();
        run_str(&["breaks", &f, "--pages", "3/8"]).unwrap();

        // An omitted scene stays listed, marked (decision 91).
        let mut sch = load(&f).unwrap();
        let gone = sch.sheets[0].id;
        sch.plan.omitted.push(gone);
        save(&f, &sch).unwrap();
        for cmd in ["oneline", "shooting"] {
            let text = run_str(&[cmd, &f]).unwrap();
            assert_eq!(text.matches("(omitted)").count(), 1, "{text}");
        }
        sch.plan.omitted.clear();
        save(&f, &sch).unwrap();

        let grid = run_str(&["dood", &f, "--grid"]).unwrap();
        assert!(grid.contains("2027-03-01 to "), "{grid}");
        assert!(grid.contains("KIT") && grid.contains("SW"), "{grid}");
        let sorted = run_str(&["dood", &f, "--grid", "--sort", "work:desc,name"]).unwrap();
        assert!(sorted.lines().nth(1).unwrap().contains("KIT"), "{sorted}");
        assert!(run_str(&["dood", &f, "--grid", "--sort", "scene"]).is_err());
        assert_eq!(
            run_str(&["dood", &f, "--grid", "--sort", "worked:desc,name"]).unwrap(),
            sorted
        );
        let csv = run_str(&["dood", &f, "--grid", "--csv"]).unwrap();
        assert!(
            csv.starts_with("category,board_id,element,2027-03-01"),
            "{csv}"
        );
        // The spreadsheet takes --sort and --no-holds as the text does.
        let sorted_csv =
            run_str(&["dood", &f, "--grid", "--sort", "work:desc,name", "--csv"]).unwrap();
        assert!(
            sorted_csv.lines().nth(1).unwrap().contains(",KIT,"),
            "{sorted_csv}"
        );
        assert_ne!(sorted_csv, csv);

        assert!(run_str(&["breakdown", &f]).unwrap().contains("Cast: "));
        assert!(run_str(&["breakdown", &f, "--by-day"])
            .unwrap()
            .starts_with("Day 1"));
        assert!(run_str(&["breakdown", &f, "--csv"])
            .unwrap()
            .starts_with("day,date,scenes"));
        assert!(run_str(&["breakdown", &f, "--by-day", "--csv"])
            .unwrap()
            .starts_with("day,date,category,board_id,element,scenes,quantity,partial\r\n1,"));
        assert!(run_str(&["detail", &f]).unwrap().contains("work days"));
        assert!(run_str(&["board", &f]).is_err());
        assert!(run_str(&["board", &f, "--csv"])
            .unwrap()
            .contains("day_break"));
        let cal = run_str(&["calendar", &f]).unwrap();
        assert!(cal.starts_with("2027-03-01 Mon  Day 1"), "{cal}");
        assert!(run_str(&["vertical", &f]).unwrap().contains("KIT"));
        assert!(run_str(&["multiunit", &f]).unwrap().contains("Main, day 1"));
        assert!(run_str(&["multiunit", &f, "--element", "NOBODY"]).is_err());

        let h = run_str(&["headers", &f]).unwrap();
        assert!(
            h.starts_with(" 1  Full: first page only; title | company"),
            "{h}"
        );
        assert!(h.contains(" 3  Title only"), "{h}");
        let p = run_str(&["production", &f, "--company", "ZTEST Pictures"]).unwrap();
        assert!(p.contains("company                  ZTEST Pictures"), "{p}");
        let p = run_str(&[
            "production",
            &f,
            "--unit-production-manager",
            " U. Manager ",
        ])
        .unwrap();
        assert!(p.contains("unit_production_manager  U. Manager\n"), "{p}");
        assert!(p.contains("company                  ZTEST Pictures"), "{p}");
        assert!(run_str(&["codes", &f, "hold=W"]).is_err());
        let c = run_str(&["codes", &f, "hold=HD"]).unwrap();
        assert!(c.contains("hold            HD"), "{c}");
        assert!(run_str(&["prep", &f, "--start", "2027-03-05", "--end", "2027-03-01"]).is_err());
        let pr = run_str(&["prep", &f, "--start", "2027-01-04"]).unwrap();
        assert!(pr.starts_with("prep start 2027-01-04"), "{pr}");
        assert!(run_str(&["budget", &f])
            .unwrap()
            .contains("\"prep_start\": \"2027-01-04\""));
        let list = run_str(&["reports", &f]).unwrap();
        assert!(list.starts_with(" 1. One-line schedule"), "{list}");
        assert_eq!(run_str(&["check", &f]).unwrap(), format!("{f}: ok\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn fixture() -> &'static str {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/scripts/harbor-light.fountain"
        )
    }

    fn temp(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "method-schedule-cli-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("show.json").to_string_lossy().into_owned()
    }

    #[test]
    fn adds_eighths() {
        assert_eq!(
            run_str(&["eighths", "1 3/8", "5/8", "6/8"]).unwrap(),
            "2 6/8\n"
        );
        assert_eq!(run_str(&["eighths"]).unwrap(), "0\n");
        assert!(run_str(&["eighths", "1/4"]).is_err());
    }

    #[test]
    fn lists_a_script() {
        let out = run_str(&["import", fixture()]).unwrap();
        assert!(out.starts_with("Harbor Light\n"));
        assert!(out.contains("5 scenes"));
        assert!(out.contains("HARBORMASTER LUND, KIT"));
        assert!(!out.contains("HARBORMASTER\n"));
        // 56 lines a page unless told otherwise, 40 to 70 (decision 195).
        assert!(out.contains("5 scenes, 7/8 pages"));
        let short = run_str(&["import", fixture(), "--lines-per-page", "40"]).unwrap();
        assert!(short.contains("5 scenes, 1 2/8 pages"));
        for bad in ["39", "71", "x"] {
            let e = run_str(&["import", fixture(), "--lines-per-page", bad]).unwrap_err();
            assert!(e.starts_with("--lines-per-page is 40 to 70"), "{e}");
        }
        assert!(run_str(&["import", fixture(), "--lines-per-page"]).is_err());
    }

    #[test]
    fn end_to_end() {
        let f = temp("e2e");
        run_str(&["new", &f, "--title", "ZTEST Harbor"]).unwrap();
        assert!(run_str(&["new", &f]).is_err(), "refuses to overwrite");
        // A template, and a schedule from it (decision 85).
        let t = format!("{f}.mtst");
        let _ = fs::remove_file(&t);
        run_str(&["template", &f, &t]).unwrap();
        assert!(method_schedule_format::is_template(
            &fs::read_to_string(&t).unwrap()
        ));
        let g = format!("{f}.from.mtsc");
        let _ = fs::remove_file(&g);
        run_str(&["new", &g, "--from", &t, "--title", "ZTEST Two"]).unwrap();
        let two = load(&g).unwrap();
        assert_eq!(two.title, "ZTEST Two");
        assert!(two.sheets.is_empty());
        assert!(!method_schedule_format::is_template(
            &fs::read_to_string(&g).unwrap()
        ));
        let _ = fs::remove_file(&t);
        let _ = fs::remove_file(&g);
        let out = run_str(&["import", fixture(), "--into", &f]).unwrap();
        assert!(out.contains("added 5 sheets"));

        // Move everything onto the board, turn on holds for KIT, then break days.
        assert_eq!(
            run_str(&["schedule", &f]).unwrap(),
            "moved 5 strips onto Main\n"
        );
        let mut s = load(&f).unwrap();
        let cast = s.category_named("Cast").unwrap();
        let kit = s.element_named(cast, "KIT").unwrap();
        s.element_mut(kit).unwrap().rules.holds = true;
        save(&f, &s).unwrap();

        assert!(run_str(&["dood", &f]).is_err(), "needs a start date");
        run_str(&["start", &f, "2027-03-01"]).unwrap();
        assert_eq!(
            run_str(&["breaks", &f, "--pages", "2/8"]).unwrap(),
            // 2/8 | 1/8 + 2/8 | 1/8 + 1/8
            "3 shoot days\n"
        );
        let days = run_str(&["days", &f]).unwrap();
        assert!(days.starts_with("day   1  2027-03-01"), "{days}");
        assert_eq!(
            run_str(&["breaks", &f, "--remove"]).unwrap(),
            "3 day breaks removed\n"
        );
        // Sorting (decision 122): scene numbers naturally, backwards; checked keys.
        assert_eq!(
            run_str(&["sort", &f, "scene:desc"]).unwrap(),
            "sorted 5 scene strips; day breaks removed, break the days again\n"
        );
        let s = load(&f).unwrap();
        let order: Vec<&str> = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .filter_map(|st| match st.kind {
                method_schedule_core::model::StripKind::Sheet(h) => {
                    s.sheet(h).map(|h| h.scenes.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(order, ["4", "3A", "3", "2", "1"]);
        for bad in [
            "cast",
            "set,set",
            "set:up",
            "set,unit,episode,scene,pages",
            "",
        ] {
            assert!(run_str(&["sort", &f, bad]).is_err(), "{bad}");
        }
        run_str(&["sort", &f, "scene"]).unwrap();
        // A limit of nothing is refused, as the app and MCP refuse it; nothing changes.
        for by in [["--pages", "0"], ["--minutes", "0"], ["--seconds", "0"]] {
            assert!(run_str(&["breaks", &f, by[0], by[1]]).is_err(), "{by:?}");
        }
        assert_eq!(
            run_str(&["days", &f]).unwrap(),
            "5 strips after the last day break are not scheduled\n"
        );
        // Named layouts (decision 78): none, then one loaded from a layout file.
        assert_eq!(run_str(&["layouts", &f]).unwrap(), "no named layouts\n");
        assert!(run_str(&["layouts", &f, "--export", &format!("{f}.layouts")]).is_err());
        let file = method_schedule_format::layouts_to_string(
            &method_schedule_core::Schedule::new("ZTEST layouts"),
            &[method_schedule_core::layouts::SavedLayout {
                id: method_schedule_core::layouts::LayoutId(1),
                name: "Plain".into(),
                body: method_schedule_core::layouts::LayoutBody::Strips(
                    method_schedule_core::model::StripLayout::default(),
                ),
            }],
        );
        std::fs::write(format!("{f}.layouts"), file).unwrap();
        assert_eq!(
            run_str(&["layouts", &f, "--import", &format!("{f}.layouts")]).unwrap(),
            "1 layouts added\n"
        );
        assert!(run_str(&["layouts", &f])
            .unwrap()
            .starts_with("Plain\tstrip layout"));
        // Export never writes over a file, the schedule's own least of all.
        let before = fs::read_to_string(&f).unwrap();
        let e = run_str(&["layouts", &f, "--export", &f]).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        assert!(run_str(&["layouts", &f, "--export", &format!("{f}.layouts")]).is_err());
        // A write to a template keeps it one, and one that would give it sheets is
        // refused with nothing written.
        let t = format!("{f}.t.mtst");
        let _ = fs::remove_file(&t);
        run_str(&["template", &f, &t]).unwrap();
        assert_eq!(
            run_str(&["layouts", &t, "--import", &format!("{f}.layouts")]).unwrap(),
            "1 layouts added\n"
        );
        let text = fs::read_to_string(&t).unwrap();
        assert!(method_schedule_format::is_template(&text));
        assert_eq!(load(&t).unwrap().layouts.layouts.len(), 2);
        let e = run_str(&["import", fixture(), "--into", &t]).unwrap_err();
        assert!(e.contains("a template holds no sheets"), "{e}");
        assert_eq!(fs::read_to_string(&t).unwrap(), text);
        let _ = fs::remove_file(&t);
        assert_eq!(
            run_str(&["breaks", &f, "--pages", "2/8"]).unwrap(),
            "3 shoot days\n"
        );

        let table = run_str(&["dood", &f, "--csv"]).unwrap();
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(
            lines[0],
            "category,board_id,element,work,hold,start,finish,total"
        );
        assert!(lines.iter().any(|l| l.starts_with("Cast,,KIT,")), "{table}");

        assert_eq!(run_str(&["check", &f]).unwrap(), format!("{f}: ok\n"));
        let before = fs::read_to_string(&f).unwrap();
        run_str(&["fmt", &f]).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), before, "already canonical");
        assert!(run_str(&["show", &f]).unwrap().contains("5 sheets"));
        let shooting = run_str(&["shooting", &f]).unwrap();
        assert!(
            shooting.contains("Scene 1  INT NET LOFT NIGHT  2/8 pages\n    Cast: KIT, TOVE"),
            "{shooting}"
        );
        let budget = run_str(&["budget", &f]).unwrap();
        assert!(
            budget.contains("\"format\": \"method-schedule-days\""),
            "{budget}"
        );
        assert!(budget.contains("\"name\": \"KIT\""), "{budget}");
        let totals = run_str(&["totals", &f]).unwrap();
        assert!(
            totals
                .lines()
                .nth(1)
                .unwrap()
                .starts_with("1     2027-03-01"),
            "{totals}"
        );
        let csv = run_str(&["totals", &f, "--csv"]).unwrap();
        assert!(csv.starts_with("day,date,Cast,"), "{csv}");
        assert!(csv.lines().last().unwrap().starts_with("total,,"), "{csv}");
        let one = run_str(&["oneline", &f]).unwrap();
        assert!(one.starts_with("Day 1  2027-03-01\n"), "{one}");
        let one_csv = run_str(&["oneline", &f, "--csv"]).unwrap();
        assert!(
            one_csv.starts_with("Day breaks and banners,Scene,I/E,Set,"),
            "{one_csv}"
        );
        assert!(
            one_csv.contains("\r\nEnd of Day 1,2027-03-01,"),
            "{one_csv}"
        );
        assert!(run_str(&["oneline", &f, "--csv", "--columns", "wide"]).is_err());
        let one_csv = run_str(&["oneline", &f, "--csv", "--columns", "fixed"]).unwrap();
        assert!(
            one_csv
                .lines()
                .nth(1)
                .unwrap()
                .starts_with("1,2027-03-01,scene,1,INT,NET LOFT,NIGHT,2,"),
            "{one_csv}"
        );
        let el = run_str(&["elements", &f, "--csv"]).unwrap();
        assert!(el.contains("Cast,,KIT,4,6,"), "{el}");

        // Each report as a workbook, read back cell by cell.
        for (cmd, sheet) in [
            ("dood", "Day Out of Days"),
            ("oneline", "One-Line"),
            ("elements", "Elements"),
        ] {
            let out = f.replace("show.json", &format!("{cmd}.xlsx"));
            assert_eq!(
                run_str(&[cmd, &f, "--xlsx", &out]).unwrap(),
                format!("wrote {out}\n")
            );
            let back = xlsx::read_cells(&fs::read(&out).unwrap()).unwrap();
            assert_eq!(back[0].name, sheet);
        }
        let back =
            xlsx::read_cells(&fs::read(f.replace("show.json", "elements.xlsx")).unwrap()).unwrap();
        let kit = back[0]
            .rows
            .iter()
            .find(|r| r[2] == export::Cell::text("KIT"))
            .unwrap();
        assert_eq!(kit[4..6], [export::Cell::Int(6), export::Cell::text("6/8")]);
        assert_eq!(kit[7], export::Cell::Date("2027-03-01".parse().unwrap()));
        assert!(run_str(&["dood", &f, "--xlsx"]).is_err());
    }

    #[test]
    fn check_reports_a_broken_file() {
        let f = temp("broken");
        fs::write(&f, "{\"format\": \"method-schedule\", \"version\": 0}").unwrap();
        assert!(run_str(&["check", &f]).is_err());
        fs::write(&f, "{}").unwrap();
        assert!(run_str(&["check", &f])
            .unwrap_err()
            .contains("not a schedule file"));
    }

    #[test]
    fn imports_an_fdx() {
        let f = temp("fdx");
        let script = f.replace("show.json", "s.fdx");
        fs::write(
            &script,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<FinalDraft DocumentType=\"Script\" Version=\"5\"><Content>\
             <Paragraph Number=\"1\" Type=\"Scene Heading\"><SceneProperties Length=\"1 2/8\" Page=\"1\"/><Text>INT. LOFT - NIGHT</Text></Paragraph>\
             <Paragraph Type=\"Character\"><Text>TOVE</Text></Paragraph><Paragraph Type=\"Dialogue\"><Text>Needle.</Text></Paragraph>\
             </Content></FinalDraft>\n",
        )
        .unwrap();
        let out = run_str(&["import", &script]).unwrap();
        assert!(out.contains("1 2/8  INT. LOFT - NIGHT"), "{out}");
        assert!(out.contains("TOVE"));
    }

    #[test]
    fn makes_a_sample() {
        let f = temp("sample");
        assert_eq!(
            run_str(&["sample", &f, "--sheets", "40"]).unwrap(),
            format!("created {f} with 40 sheets\n")
        );
        assert!(run_str(&["check", &f]).is_ok());
        assert!(run_str(&["sample", &f]).is_err(), "refuses to overwrite");
        assert!(run_str(&["sample", &format!("{f}.2"), "--sheets", "0"]).is_err());
    }

    #[test]
    fn writes_the_apps_sample() {
        // The same bytes the app ships for Help > Open Sample (decision 198).
        let f = temp("demo");
        assert_eq!(
            run_str(&["sample", "--demo", &f]).unwrap(),
            format!("created {f}, the sample production\n")
        );
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            include_str!("../../method-schedule-app/resources/sample.mtsc")
        );
        assert!(run_str(&["check", &f]).is_ok());
        assert!(
            run_str(&["sample", "--demo", &f]).is_err(),
            "refuses to overwrite"
        );
        assert!(run_str(&["sample", "--demo"]).is_err());
    }

    #[test]
    fn mcp_needs_a_root() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(mcp_config(&args(&[])).unwrap_err().contains("--root"));
        assert!(mcp_config(&args(&["--root"])).is_err());
        assert!(mcp_config(&args(&["--root", "a", "--write"])).is_err());
        let c = mcp_config(&args(&["--root", "a", "--root", "b", "--read-only"])).unwrap();
        assert_eq!(c.roots.len(), 2);
        assert!(c.read_only);
    }

    /// The Harbor Light script scheduled on Main from 2027-03-01, two eighths a day, as
    /// `name` in a fresh folder; returns its path.
    #[test]
    fn board_picks_the_board_and_a_template_is_an_mtst() {
        let f = harbor("board-flag");
        let mut s = load(&f).unwrap();
        let second = s.add_board("ZTEST Second Unit", None);
        save(&f, &s).unwrap();
        // The second board starts with every sheet in its Boneyard: schedule them there.
        let out = run_str(&["schedule", &f, "--board", "ZTEST Second Unit"]).unwrap();
        assert!(out.starts_with("moved "), "{out}");
        run_str(&["start", &f, "--board", "ztest second unit", "2028-01-03"]).unwrap();
        run_str(&[
            "breaks",
            &f,
            "--board",
            &second.0.to_string(),
            "--pages",
            "4",
        ])
        .unwrap();
        let s = load(&f).unwrap();
        let b2 = s.board(second).unwrap();
        assert!(b2.boneyard.is_empty());
        assert_eq!(b2.sub_boards[0].start, Some("2028-01-03".parse().unwrap()));
        assert_eq!(
            s.boards[0].sub_boards[0].start,
            Some("2027-03-01".parse().unwrap())
        );
        let days_first = run_str(&["days", &f]).unwrap();
        let days_second = run_str(&["days", &f, "--board", "ZTEST Second Unit"]).unwrap();
        assert_ne!(days_first, days_second);
        assert!(days_second.contains("2028-01-03"), "{days_second}");
        for cmd in [
            "dood", "oneline", "elements", "totals", "board", "calendar", "budget",
        ] {
            let mut args = vec![cmd, f.as_str(), "--board", "ZTEST Second Unit"];
            if cmd == "board" {
                args.push("--csv");
            }
            run_str(&args).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        }
        run_str(&["conflicts", &f, "--board", "ZTEST Second Unit"]).unwrap();
        let r = run_str(&["redflags", &f, "--board", "ZTEST Second Unit"]).unwrap();
        assert!(r.contains("ZTEST Second Unit"), "{r}");
        let main = s.boards[0].sub_boards[0].id.0.to_string();
        assert!(run_str(&[
            "redflags",
            &f,
            "--board",
            "ZTEST Second Unit",
            "--sub-board",
            &main
        ])
        .is_err());
        let e = run_str(&["days", &f, "--board", "No Such"]).unwrap_err();
        assert!(e.contains("no such board"), "{e}");
        assert!(run_str(&["days", &f, "--board"]).is_err());
        assert!(run_str(&["days", &f, "--board", "A", "--board", "B"]).is_err());
        // The flag is not kept for the next command.
        assert_eq!(run_str(&["days", &f]).unwrap(), days_first);
        // A template is written only under .mtst, as the app and MCP keep it.
        let dir = Path::new(&f).parent().unwrap();
        let bad = dir.join("t.mtsc").to_string_lossy().into_owned();
        let e = run_str(&["template", &f, &bad]).unwrap_err();
        assert!(e.contains(".mtst"), "{e}");
        assert!(!Path::new(&bad).exists());
        let good = dir.join("t.MTST").to_string_lossy().into_owned();
        run_str(&["template", &f, &good]).unwrap();
    }

    fn harbor(name: &str) -> String {
        let f = temp(name);
        run_str(&["new", &f, "--title", "ZTEST Harbor"]).unwrap();
        run_str(&["import", fixture(), "--into", &f]).unwrap();
        run_str(&["schedule", &f]).unwrap();
        run_str(&["start", &f, "2027-03-01"]).unwrap();
        run_str(&["breaks", &f, "--pages", "2/8"]).unwrap();
        f
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn diff_lists_changes_in_plain_words() {
        let a = harbor("diff");
        let b = a.replace("show.json", "b.json");
        let mut s = load(&a).unwrap();
        assert_eq!(run_str(&["diff", &a, &a]).unwrap(), "No changes.\n");
        s.title = "ZTEST Harbor 2".into();
        s.sheets[0].set = "BOATHOUSE".into();
        save(&b, &s).unwrap();
        let out = run_str(&["diff", &a, &b]).unwrap();
        assert!(out.starts_with("2 changes\n"), "{out}");
        assert!(
            out.contains("Title \"ZTEST Harbor\" -> \"ZTEST Harbor 2\""),
            "{out}"
        );
        assert!(out.contains("set \"NET LOFT\" -> \"BOATHOUSE\""), "{out}");
        assert!(run_str(&["diff", &a]).is_err());
    }

    #[test]
    fn merge_writes_both_sides_and_reports_conflicts() {
        let base = harbor("merge");
        let ours = base.replace("show.json", "ours.json");
        let theirs = base.replace("show.json", "theirs.json");
        let out = base.replace("show.json", "out.json");
        let s = load(&base).unwrap();
        let mut o = s.clone();
        o.sheets[0].set = "BOATHOUSE".into();
        save(&ours, &o).unwrap();
        let mut t = s.clone();
        t.sheets[1].synopsis = "Storm.".into();
        save(&theirs, &t).unwrap();
        let m = merge_cmd(&args(&["merge", &base, &ours, &theirs, "--out", &out])).unwrap();
        assert!(!m.conflicts);
        assert_eq!(m.stdout, format!("wrote {out}\nno conflicts\n"));
        let merged = load(&out).unwrap();
        assert_eq!(merged.sheets[0].set, "BOATHOUSE");
        assert_eq!(merged.sheets[1].synopsis, "Storm.");
        // Without --out the merged file goes to stdout and the report to stderr.
        let m = merge_cmd(&args(&["merge", &base, &ours, &theirs])).unwrap();
        assert_eq!(method_schedule_format::from_str(&m.stdout).unwrap(), merged);
        assert_eq!(m.stderr, "no conflicts\n");
        // The same field both ways: settled for ours (or theirs), listed, exit 1.
        t.sheets[0].set = "BARN".into();
        save(&theirs, &t).unwrap();
        let m = merge_cmd(&args(&["merge", &base, &ours, &theirs, "--out", &out])).unwrap();
        assert!(m.conflicts);
        assert!(
            m.stdout.contains("1 conflict, settled for ours:\n  Scene 1 set: ours \"BOATHOUSE\", theirs \"BARN\", was \"NET LOFT\" (took ours)"),
            "{}",
            m.stdout
        );
        assert_eq!(load(&out).unwrap().sheets[0].set, "BOATHOUSE");
        let m = merge_cmd(&args(&[
            "merge", &base, &ours, &theirs, "--out", &out, "--theirs",
        ]))
        .unwrap();
        assert!(m.conflicts);
        assert_eq!(load(&out).unwrap().sheets[0].set, "BARN");
        // Usage errors.
        assert!(merge_cmd(&args(&["merge", &base, &ours])).is_err());
        assert!(merge_cmd(&args(&["merge", &base, &ours, &theirs, "--out", &ours])).is_err());
        assert!(merge_cmd(&args(&["merge", &base, &ours, &theirs, "--bogus"])).is_err());
    }

    #[test]
    fn merge_driver_writes_over_ours() {
        let base = harbor("driver");
        let ours = base.replace("show.json", "ours.json");
        let theirs = base.replace("show.json", "theirs.json");
        let s = load(&base).unwrap();
        let mut o = s.clone();
        o.sheets[0].set = "BOATHOUSE".into();
        save(&ours, &o).unwrap();
        let mut t = s.clone();
        t.sheets[0].set = "BARN".into();
        t.sheets[1].synopsis = "Storm.".into();
        save(&theirs, &t).unwrap();
        let m = merge_cmd(&args(&["merge-driver", &base, &ours, &theirs, "film.mtsc"])).unwrap();
        assert!(m.conflicts);
        assert!(
            m.stderr
                .starts_with("method-schedule merge-driver: film.mtsc\n"),
            "{}",
            m.stderr
        );
        let merged = load(&ours).unwrap();
        assert_eq!(merged.sheets[0].set, "BOATHOUSE");
        assert_eq!(merged.sheets[1].synopsis, "Storm.");
        // No common base: refused, ours untouched.
        let empty = base.replace("show.json", "empty.json");
        fs::write(&empty, "").unwrap();
        let before = fs::read(&ours).unwrap();
        let e = merge_cmd(&args(&[
            "merge-driver",
            &empty,
            &ours,
            &theirs,
            "film.mtsc",
        ]))
        .err()
        .unwrap();
        assert!(e.contains("no common base"), "{e}");
        assert_eq!(fs::read(&ours).unwrap(), before);
        assert!(merge_cmd(&args(&[
            "merge-driver",
            &base,
            &ours,
            &theirs,
            "--out",
            "x"
        ]))
        .is_err());
    }

    #[test]
    fn explain_names_an_element() {
        let f = harbor("explain");
        let out = run_str(&["explain", &f, "KIT"]).unwrap();
        assert!(
            out.starts_with("\"KIT\" (Cast) on \"Main\" (board \"Board 1\"): work "),
            "{out}"
        );
        assert!(out.contains("\nDay by day\n"), "{out}");
        assert_eq!(
            run_str(&["explain", &f, "kit", "--category", "Cast"]).unwrap(),
            out,
            "names ignore case"
        );
        assert!(run_str(&["explain", &f, "NOBODY"])
            .unwrap_err()
            .contains("no element"));
        assert!(run_str(&["explain", &f, "KIT", "--category", "Props"]).is_err());
        // The same name in two categories needs --category.
        let mut s = load(&f).unwrap();
        let props = s.add_category("Props");
        s.add_element(props, "Kit").unwrap();
        save(&f, &s).unwrap();
        assert!(run_str(&["explain", &f, "KIT"])
            .unwrap_err()
            .contains("pick one with --category"));
        assert!(run_str(&["explain", &f, "KIT", "--category", "Cast"]).is_ok());
    }

    #[test]
    fn find_then_replace_saves_and_lists_changes() {
        let f = harbor("find");
        assert_eq!(
            run_str(&["find", &f, "loft"]).unwrap(),
            "2 results for \"loft\"\n   1  Scene 1 set: NET LOFT\n   2  Scene 4 set: NET LOFT\n"
        );
        let out = run_str(&["find", &f, "kit", "--word", "--field", "element"]).unwrap();
        assert_eq!(out, "1 result for \"kit\"\n   1  Cast element: KIT\n");
        assert!(run_str(&["find", &f, "kit", "--case"])
            .unwrap()
            .starts_with("0 results"));
        assert!(run_str(&["find", &f, "x", "--field", "nope"])
            .unwrap_err()
            .contains("one of sheet, number, scenes"));
        assert!(run_str(&["find", &f, "x", "--bogus"]).is_err());
        assert!(run_str(&["find", &f]).is_err());

        // Pick the second result only.
        let before = fs::read_to_string(&f).unwrap();
        assert!(run_str(&["replace", &f, "loft", "Attic"])
            .unwrap_err()
            .contains("--all or --pick"));
        assert!(run_str(&["replace", &f, "loft", "Attic", "--pick", "3"])
            .unwrap_err()
            .contains("there are 2 results"));
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "refusals write nothing"
        );
        let out = run_str(&["replace", &f, "LOFT", "ATTIC", "--pick", "2"]).unwrap();
        assert_eq!(
            out,
            format!("replaced in 1 place in {f}\n  Scene 4 set: \"NET LOFT\" -> \"NET ATTIC\"\n")
        );
        let s = load(&f).unwrap();
        assert_eq!(s.sheets[0].set, "NET LOFT");
        assert_eq!(s.sheets[4].set, "NET ATTIC");
        // The same text: nothing written.
        let before = fs::read_to_string(&f).unwrap();
        assert!(run_str(&["replace", &f, "LOFT", "LOFT", "--all", "--case"])
            .unwrap()
            .starts_with("nothing changed"));
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        // A rename that would collide in Cast is refused.
        let e =
            run_str(&["replace", &f, "TOVE", "kit", "--all", "--field", "element"]).unwrap_err();
        assert!(e.contains("would be in Cast twice"), "{e}");
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        // --all everywhere.
        let out = run_str(&["replace", &f, "night", "Dusk", "--all"]).unwrap();
        assert!(out.starts_with("replaced in 2 places"), "{out}");
    }

    #[test]
    fn conflicts_lists_two_sub_boards_on_one_date() {
        let f = harbor("conflicts");
        assert_eq!(
            run_str(&["conflicts", &f]).unwrap(),
            "board \"Board 1\": 0 conflicts\n"
        );
        // A second sub-board starting the same day, with the last two strips and a break.
        let mut s = load(&f).unwrap();
        let b = s.boards[0].id;
        let unit = s
            .add_sub_board(b, "Unit B", "2027-03-01".parse().ok())
            .unwrap();
        let ids: Vec<_> = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .filter(|x| matches!(x.kind, method_schedule_core::model::StripKind::Sheet(_)))
            .map(|x| x.id)
            .skip(3)
            .collect();
        s.move_strips(
            b,
            &ids,
            Place {
                sub_board: Some(unit),
                index: 0,
            },
        )
        .unwrap();
        s.insert_strip(
            b,
            method_schedule_core::model::StripKind::DayBreak,
            Place {
                sub_board: Some(unit),
                index: 9,
            },
        )
        .unwrap();
        save(&f, &s).unwrap();
        let out = run_str(&["conflicts", &f]).unwrap();
        assert_eq!(
            out,
            "board \"Board 1\": 1 conflict\n  2027-03-01  KIT (Cast): Main day 1 (scene 1); Unit B day 1 (scene 4)\n"
        );
        // A fitting for KIT on Main's day 1, where KIT works: a conflict too.
        let kit = s.element_named(s.cast().unwrap(), "KIT").unwrap();
        let main = s.boards[0].sub_boards[0].id;
        let on: method_schedule_core::Date = "2027-03-01".parse().unwrap();
        s.set_event(
            b,
            main,
            None,
            method_schedule_core::model::Event {
                name: "Wardrobe".into(),
                kind: method_schedule_core::model::EventKind::Fitting,
                from: on,
                to: on,
                elements: vec![kit],
                color: None,
            },
        )
        .unwrap();
        save(&f, &s).unwrap();
        let out = run_str(&["conflicts", &f]).unwrap();
        assert_eq!(
            out,
            "board \"Board 1\": 2 conflicts\n  2027-03-01  KIT (Cast): Main day 1 (scene 1); Unit B day 1 (scene 4)\n\
             events on work days (conflicts):\n  2027-03-01  KIT (Cast): fitting \"Wardrobe\" on Main day 1, a day it works there\n\
             event clashes across sub-boards (informational, not conflicts):\n  2027-03-01  KIT (Cast): event \"Wardrobe\" on Main; work on Main day 1 (scene 1); work on Unit B day 1 (scene 4)\n"
        );
        assert!(run_str(&["conflicts", &f, "--board", "99999"]).is_err());
        assert!(run_str(&["conflicts", &f, "--bogus"]).is_err());
    }

    #[test]
    fn redflags_lists_work_and_events_on_flagged_dates() {
        use method_schedule_core::model::{Event, EventKind, RedFlag, RedFlagDates, RedFlagId};
        let f = harbor("redflags");
        assert_eq!(
            run_str(&["redflags", &f]).unwrap(),
            "\"Main\" (board \"Board 1\"): 0 red flag clashes (0 red flags in the schedule)\n"
        );
        let mut s = load(&f).unwrap();
        let cast = s.category_named("Cast").unwrap();
        let kit = s.element_named(cast, "KIT").unwrap();
        let away = s.add_red_flag_type("Unavailable").unwrap();
        let d = |x: &str| x.parse::<Date>().unwrap();
        s.set_red_flag(
            kit,
            RedFlag {
                id: RedFlagId(0),
                kind: away,
                dates: RedFlagDates::NotAfter(d("2027-03-01")),
                note: "Back to school".into(),
                hard: true,
            },
        )
        .unwrap();
        let (b, sb) = first(&s).unwrap();
        s.set_event(
            b,
            sb,
            None,
            Event {
                name: "Wardrobe".into(),
                kind: EventKind::Fitting,
                from: d("2027-03-10"),
                to: d("2027-03-10"),
                elements: vec![kit],
                color: None,
            },
        )
        .unwrap();
        save(&f, &s).unwrap();
        let out = run_str(&["redflags", &f]).unwrap();
        assert_eq!(
            out,
            "\"Main\" (board \"Board 1\"): 3 red flag clashes (1 red flag in the schedule)\n  \
             2027-03-02  KIT (Cast): Unavailable (hard, not after 2027-03-01, \"Back to school\"): works day 2 (scene 2, scene 3)\n  \
             2027-03-03  KIT (Cast): Unavailable (hard, not after 2027-03-01, \"Back to school\"): works day 3 (scene 4)\n  \
             2027-03-10  KIT (Cast): Unavailable (hard, not after 2027-03-01, \"Back to school\"): fitting \"Wardrobe\"\n"
        );
        let by_id = run_str(&["redflags", &f, "--sub-board", &sb.0.to_string()]).unwrap();
        assert_eq!(by_id, out);
        assert_eq!(
            run_str(&["redflags", &f, "--board", &b.0.to_string()]).unwrap(),
            out
        );
        assert!(run_str(&["redflags", &f, "--sub-board", "99999"]).is_err());
        assert!(run_str(&["redflags", &f, "--board", "99999"]).is_err());
        assert!(run_str(&["redflags", &f, "--board"]).is_err());
        assert!(run_str(&["redflags", &f, "--bogus"]).is_err());
    }

    #[test]
    fn merge_and_duplicate_sheets_save_and_report() {
        let f = harbor("mergesheets");
        // Sheets 4 (scene 1), 8 (2), 10 (3), 13 (3A), 15 (4): ids from the import.
        let ids: Vec<String> = load(&f)
            .unwrap()
            .sheets
            .iter()
            .map(|h| h.id.0.to_string())
            .collect();
        let [one, two, three, three_a, _] = &ids[..] else {
            panic!("{ids:?}")
        };
        let before = fs::read_to_string(&f).unwrap();
        for (a, want) in [
            (vec!["merge-sheets", &f, three], "at least one sheet"),
            (vec!["merge-sheets", &f, three, three], "merged with itself"),
            (vec!["merge-sheets", &f, three, two, two], "given twice"),
            (vec!["merge-sheets", &f, three, "999999"], "no sheet 999999"),
            (vec!["merge-sheets", &f, three, "x"], "not a sheet id"),
            (
                vec!["merge-sheets", &f, three, two, "--all"],
                "unknown option",
            ),
            (vec!["duplicate-sheet", &f, "999999"], "no sheet 999999"),
            (vec!["duplicate-sheet", &f], "usage"),
        ] {
            let e = run_str(&a).unwrap_err();
            assert!(e.contains(want), "{a:?}: {e}");
        }
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            before,
            "refusals write nothing"
        );

        let out = run_str(&["merge-sheets", &f, three, three_a, two]).unwrap();
        assert_eq!(
            out,
            format!(
                "merged sheet {three_a} (scene 3A), sheet {two} (scene 2) into sheet {three} (scenes 3, 3A, 2) in {f}\n\
                 \x20 scenes: \"3\" -> \"3, 3A, 2\"\n\
                 \x20 eighths: \"2/8\" -> \"4/8\"\n\
                 \x20 dropped int_ext of sheet {three_a} (scene 3A): \"INT/EXT\"\n\
                 \x20 dropped set of sheet {three_a} (scene 3A): \"KIT'S VAN\"\n\
                 \x20 dropped set of sheet {two} (scene 2): \"BOATYARD\"\n\
                 \x20 dropped time_of_day of sheet {two} (scene 2): \"NIGHT\"\n"
            )
        );
        let s = load(&f).unwrap();
        assert_eq!(s.sheets.len(), 3);
        assert_eq!(s.sheets[1].scenes, "3, 3A, 2", "the order given");

        let out = run_str(&["duplicate-sheet", &f, one]).unwrap();
        let s = load(&f).unwrap();
        let copy = s.sheets.last().unwrap();
        assert_eq!(
            out,
            format!(
                "duplicated sheet {one} (scene 1) as sheet {} in {f}\n  board Board 1: Main, position 2\n",
                copy.id.0
            )
        );
        assert_eq!(copy.scenes, "1");
        assert_eq!(copy.elements, s.sheets[0].elements);
        assert_eq!(
            run_str(&["days", &f]).unwrap().lines().next().unwrap(),
            "day   1  2027-03-01       4/8 pages    0:00  2 strips"
        );
    }

    #[test]
    fn banners_listed_added_and_restyled() {
        let f = temp("banners");
        run_str(&["new", &f, "--title", "ZTEST Banners"]).unwrap();
        run_str(&["import", fixture(), "--into", &f]).unwrap();
        run_str(&["schedule", &f]).unwrap();
        assert_eq!(run_str(&["banners", &f]).unwrap(), "no banners\n");
        let added = run_str(&[
            "banner",
            &f,
            "--add",
            "Company move",
            "--at",
            "1",
            "--minutes",
            "45",
            "--bold",
            "--color",
            "teal",
            "--height",
            "2",
            "--size",
            "large",
            "--align",
            "center",
            "--font",
            "mono",
        ])
        .unwrap();
        assert!(
            added.contains("0:45  bold, teal, two lines, large text, centered, monospaced")
                && added.contains("Company move"),
            "{added}"
        );
        let id = added.split_whitespace().next().unwrap().to_string();
        let changed = run_str(&[
            "banner",
            &f,
            &id,
            "--no-bold",
            "--color",
            "none",
            "--align",
            "left",
            "--font",
            "usual",
        ])
        .unwrap();
        assert!(changed.contains("0:45  two lines, large text"), "{changed}");
        assert!(!changed.contains("monospaced"), "{changed}");
        let listed = run_str(&["banners", &f]).unwrap();
        assert_eq!(listed, changed);
        assert!(run_str(&["banner", &f, &id, "--color", "mauve"]).is_err());
        assert!(run_str(&["banner", &f, &id, "--height", "3"]).is_err());
        assert!(run_str(&["banner", &f, "--add", " "]).is_err());
        assert_eq!(
            run_str(&["banners", &f]).unwrap(),
            listed,
            "refusals change nothing"
        );
        // A sheet's strip is not a banner.
        let s = load(&f).unwrap();
        let sheet = s.boards[0].sub_boards[0].strips[0].id.0.to_string();
        assert!(run_str(&["banner", &f, &sheet, "--bold"]).is_err());
    }

    /// Every command the help lists runs on a scheduled schedule: the table below names
    /// each one with real arguments, and must name exactly the help's commands.
    #[test]
    fn every_command_in_the_help_runs() {
        use method_schedule_core::model::StripKind;
        use std::collections::BTreeSet;
        let f = harbor("everycmd");
        let dir = Path::new(&f).parent().unwrap().to_path_buf();
        let out = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let (tpl, from) = (out("t.mtst"), out("from.mtsc"));
        let (xlsx, exch, cs) = (out("o.xlsx"), out("e.json"), out("c.json"));
        let (copy, big) = (out("copy.mtsc"), out("big.mtsc"));
        fs::copy(&f, &copy).unwrap();
        let s = load(&f).unwrap();
        let sheet = s.sheets[0].id.0.to_string();
        let other = s.sheets[1].id.0.to_string();
        let strip = s.boards[0].sub_boards[0]
            .strips
            .iter()
            .find(|x| matches!(x.kind, StripKind::Sheet(_)))
            .unwrap()
            .id
            .0
            .to_string();
        let el = s.elements[0].name.clone();
        let cat = s
            .categories
            .iter()
            .find(|c| c.id == s.elements[0].category)
            .unwrap()
            .name
            .clone();
        let script = fixture().to_string();
        let calls: Vec<Vec<&str>> = vec![
            vec!["template", &f, &tpl],
            vec!["new", &from, "--from", &tpl],
            vec!["sample", &big, "--sheets", "40"],
            vec!["show", &f],
            vec!["check", &f],
            vec!["fmt", &f],
            vec!["import", &script],
            vec!["schedule", &f],
            vec!["start", &f, "2027-03-01"],
            vec!["sort", &f, "scene"],
            vec!["breaks", &f, "--pages", "2/8"],
            vec!["days", &f],
            vec!["banners", &f],
            vec!["banner", &f, "--add", "ZTEST MOVE", "--color", "blue"],
            vec!["layouts", &f],
            vec!["dood", &f, "--grid", "--no-holds", "--csv"],
            vec!["oneline", &f, "--xlsx", &xlsx],
            vec!["elements", &f, "--csv"],
            vec!["shooting", &f],
            vec!["totals", &f, "--csv"],
            vec!["breakdown", &f, "--by-day"],
            vec!["detail", &f, "--csv"],
            vec!["board", &f, "--csv"],
            vec!["calendar", &f],
            vec!["vertical", &f],
            vec!["multiunit", &f],
            vec!["reports", &f],
            vec!["headers", &f],
            vec!["production", &f, "--company", "ZTEST Co"],
            vec!["codes", &f],
            vec!["prep", &f, "--start", "2027-02-01"],
            vec!["budget", &f, "--out", &exch],
            vec!["diff", &copy, &f],
            vec!["explain", &f, &el, "--category", &cat],
            vec!["find", &f, "HARBOR"],
            vec![
                "replace",
                &f,
                "ZTEST MOVE",
                "ZTEST TRAVEL",
                "--field",
                "banner",
                "--all",
            ],
            vec!["conflicts", &f],
            vec!["redflags", &f],
            vec!["duplicate-sheet", &f, &sheet],
            vec!["eighths", "2", "3/8"],
            vec!["help"],
            vec!["revise", &f, &script, "--dry-run"],
            // The count's value is not taken for a file (decision 195).
            vec!["revise", &f, "--lines-per-page", "60", &script, "--dry-run"],
            vec!["timeline", &f, "1"],
            vec!["call", &f, "1", "6:30"],
            vec!["long-day", &f, "10:30"],
            vec!["location", &f, "HARBOR", "41.5", "-70.6"],
            vec!["locations", &f],
            vec!["sun", &f],
            vec!["callsheet", &f, "1", "--out", &cs],
            vec!["turnaround", &f, &cat, "12"],
            vec!["rest", &f],
            vec!["lock", &f, "White"],
            vec!["drift", &f, "White"],
            vec!["unlock", &f, "White"],
            vec!["pin", &f, &strip],
            vec!["suggest", &f, "--apply"],
            vec!["merge-sheets", &f, &sheet, &other],
        ];
        let mut ran = BTreeSet::new();
        for c in &calls {
            let r = run_str(c);
            assert!(r.is_ok(), "{c:?}: {r:?}");
            ran.insert(c[0].to_string());
        }
        // `mcp` serves stdin, merge and merge-driver have their own exit codes (run before
        // `run`), and git-setup writes git's config (tests/merge_driver.rs runs it in a
        // folder of its own); each has its own test.
        let m = merge_cmd(&args(&["merge", &copy, &copy, &copy])).unwrap();
        assert!(!m.conflicts);
        for name in ["mcp", "merge", "merge-driver", "git-setup"] {
            ran.insert(name.to_string());
        }
        let help = run_str(&["help"]).unwrap();
        let listed: BTreeSet<String> = help
            .lines()
            .filter_map(|l| l.strip_prefix("  "))
            .filter(|l| l.starts_with(|c: char| c.is_ascii_lowercase()))
            .flat_map(|l| l.split_whitespace().next().unwrap().split('|'))
            .map(str::to_string)
            .collect();
        assert_eq!(listed, ran, "the help's commands and the ones run here");
        let _ = fs::remove_dir_all(&dir);
    }

    /// An omitted scene stays in the one-line, marked, and its pages count nowhere.
    #[test]
    fn the_one_line_marks_an_omitted_scene() {
        let f = harbor("omitted");
        let mut s = load(&f).unwrap();
        let first = s.sheets[0].id;
        s.plan.omitted.push(first);
        save(&f, &s).unwrap();
        let text = run_str(&["oneline", &f]).unwrap();
        let marked: Vec<&str> = text.lines().filter(|l| l.ends_with("(omitted)")).collect();
        assert_eq!(marked.len(), 1, "{text}");
        assert!(text.contains("end of day 1: 0 pages"), "{text}");
        let csv = run_str(&["oneline", &f, "--csv", "--columns", "fixed"]).unwrap();
        assert_eq!(csv.matches(",omitted_scene,").count(), 1, "{csv}");
        assert!(run_str(&["days", &f]).unwrap().starts_with("day   1"));
        let _ = fs::remove_dir_all(Path::new(&f).parent().unwrap());
    }

    #[test]
    fn unknown_command_fails() {
        assert!(run_str(&["nope"]).is_err());
    }
}
