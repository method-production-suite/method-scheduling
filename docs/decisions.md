# Decisions

Short records of choices that shape the code. Newest last.

## 1. What this is (2026-10-10, the owner)

A free, open-source, local-first desktop app for film and TV scheduling: breakdown,
stripboard, calendar, day out of days, reports. A clean-room alternative to the reference
product, and a sister to Method Budget, built the same way. Files on disk, offline, no
accounts, no server, no telemetry. The only network call will be the app's own update
check. License **MIT OR Apache-2.0**. Rules for sources are in `CLEAN-ROOM.md`.

## 2. Match the reference product by default (2026-10-10, the owner)

Math, defaults, codes, counting rules and keyboard flow follow the reference product,
checked in the real app, not guessed. Our extras are opt-in settings saved in the file and
off by default. When the public manual and the real app disagree, the app wins; newer
recordings beat older tutorials.

## 3. Stack and numbers (2026-10-10)

Same shape as Method Budget:
- Rust workspace: `sched-core` (engine, **zero dependencies**, its own seeded property-test
  generator in `testgen.rs`), `sched-format` (file format, serde only), `sched-cli`
  (binary `sched`). The Tauri app and `xtask` come later.
- **Integers only** in the model and the file. Page lengths are `Eighths(u32)`, eight
  eighths always roll into a page. Durations are whole minutes. Dates are ISO
  `YYYY-MM-DD`. If a division ever shows up, it gets one documented rounding rule here.
- Toolchain pinned to 1.96.1 in `rust-toolchain.toml`. Every locked crate must be at least
  14 days old (`scripts/check-crate-ages.py`, run in CI).
- serde and serde_json join `sched-format` only when the first type is saved, at the same
  pinned versions Method Budget uses.

## 4. Working names until the product is named (2026-10-10)

"<Product>" in prose, `sched-*` for crates, `sched` for the binary and the `format` field.
All of these change once the owner picks a name. The file extension waits for the name
too; it must look nothing like the reference product's extensions.

## 5. Repository (2026-10-10, the owner)

Private until the owner says go public. Pushes use a credential helper set for this repo
only (the owner's GitHub account); the global `gh` account is never switched.
Before going public: a fresh single-commit repo, a neutral commit identity, and
`docs/KICKOFF.md` left out (it holds private working notes).

Moved 2026-10-16: the repository is now `method-production-suite/method-scheduling`
(still private) and the checkout lives in `07_Method_Production_Suite/` beside the other
Method apps. Pushes still go through the repo-local helper.

## 6. Test data is synthetic (2026-10-10, the owner)

Fixtures are scripts and schedules we write ourselves, with invented productions and
people. Nothing from the reference product's samples, and no third-party scripts.

## 7. Checking against the real app (2026-10-10, the owner)

Agents may drive the reference app on this Mac to check engine rules, including
unattended overnight runs when the owner is away. Throwaway `ZTEST` schedules only, never
the owner's files. Results are exported to open formats in the gitignored `/imports/` and
compared by script. Only one session controls the screen at a time
(`~/.claude/coordination/SCREEN-LOCK.md`).

## 8. Script import brings in headings and speaking characters (2026-10-10)

By default, importing a script creates one scene per heading with its number, interior or
exterior, set and time of day, plus the characters who speak in it. That matches what the
reference product takes from a plain script. Character names drop extensions such as
`(V.O.)`, `(O.S.)` and `(CONT'D)`, and keep their first-seen order. People named only in
action are not added. Reading breakdown tags is a later opt-in extra.

Fountain specifics, from the open spec: a heading needs a blank line before and after
unless forced with a leading period. A character cue is all caps (or forced with `@`) with
a blank line before and dialogue right after. Boneyard `/* */`, notes `[[ ]]`, sections
`#` and synopses `=` are ignored. `===` forces a page break. `EST` headings count as
exteriors until the real app says otherwise.

## 9. Paginating a script with no page lengths (2026-10-10)

Fountain carries no page lengths, so we count lines like a standard screenplay page:
55 lines a page (56 since decision 195, and a setting); action wraps at 60 characters, dialogue at 35, parentheticals at 25; one
blank line between elements; a character cue is one line; sections and synopses take no
space; the title page is not counted.

A scene runs from its heading to the next heading. Line positions are rounded to the
nearest eighth (halves up) before subtracting, so scene lengths add up to the script's
length. **No scene is shorter than 1/8**, which can make the total a little longer than
the script. The model is plain on purpose; calibrating it against the same script
paginated by a screenwriting app is an engine check. `.fdx` files carry their own lengths
and will use those.

## 10. Day out of days rules checked in the real app (2026-10-10)

ZTEST run 1 (behavior note 14) settled these, and the engine follows them:

- Holds (when allowed) fill every working day strictly between the first and last work
  day. Off days stay off and count nowhere.
- A gap is the number of calendar days strictly between two consecutive work days,
  counting weekends and holidays. It can drop when it is at least the element's minimum.
- **An element gets at most one drop, in its longest qualifying gap; a tie goes to the
  earlier gap.** Every other gap is held. Days inside the drop count nowhere. This
  replaces the research guess of "the first gap that fits".
- A holiday counts as worked only when holds are on, it falls inside the work span, and it
  is not inside the drop.
- New elements start with holds and drops off and a minimum gap of 1 day.
- Importing a script does not assign board IDs to the new cast. Our `Schedule::new` Cast
  category therefore leaves automatic board IDs off; it is a category setting the user can
  turn on.

## 11. Events, company travel days and the summary, checked in the real app (2026-10-10)

ZTEST run 2 (behavior note 15):

- Travel, fitting and rehearsal events count as worked on every date they cover, even
  outside the work span; the row grows to show them. They never move the start or finish.
  Other event kinds count nowhere.
- On a day that would be held, an event shows its code joined with H (`FTH`, `TRH`) and
  counts as worked only.
- Inside the span, event days split gaps for the drop search, so one can stop a drop.
- A company travel day takes no shoot day number and counts like a holiday: `M`, worked,
  only inside the span with holds on and outside the drop.
- The summary shows Work, Hold, Start, Finish and Total, with **Total = Work + Hold**.
  Rows sort by board ID, then name.
- Not yet checked, assumed: an event on a day the element works counts once, as work.

## 12. Automatic day breaks and script lengths, checked in the real app (2026-10-10)

ZTEST run 3 (behavior note 16):

- **Automatic day breaks close a day as soon as its total reaches or passes the limit.**
  The strip that crosses the limit stays in that day, so days can run over. This replaces
  the research guess of keeping each day at or under the limit. A break always closes the
  last day.
- Page lengths read from a script file roll eighths into pages, and **a bare number counts
  eighths** (`1` is 1/8). `Eighths::from_script` reads them that way; typed lengths keep
  the usual reading, where `2` is two pages.

## 13. The schedule file, the command line and `.fdx` import (2026-10-11)

- **File format** (`docs/FORMAT.md`, version 0 until the first release): pretty JSON, fixed
  key order, defaults left out, strict reader, stable ids. **Every number is an integer**:
  page lengths in eighths (`eighths`), times in minutes or seconds (`shoot_minutes`,
  `screen_seconds`, `sunrise` as minutes after midnight), dates as `YYYY-MM-DD` strings.
  A file that fails `Schedule::check` is refused. The example in FORMAT.md is a test.
- serde 1.0.229 and serde_json 1.0.151, the pins Method Budget uses. The cooldown check
  caught syn 3.0.7 at one day old; it is pinned to 3.0.6.
- **Command line**: `new`, `show`, `check`, `fmt`, `import` (list, or `--into FILE`),
  `start`, `breaks`, `days`, `dood` (`--csv`). Saves are atomic (temporary file, flush,
  rename). Commands act on the first board and sub-board until a selector is needed.
- **`.fdx` import** (`sched_core::fdx`) with its own strict XML reader: no DOCTYPE, so no
  DTDs or external entities; caps on size (64 MiB), depth (256), elements (1,000,000) and
  attributes (256); never panics (mutation test). Script lengths read by decision 12; a
  missing or unreadable length is 1/8, as the real app showed. Open, defaults stand: the
  title is the first non-empty title-page paragraph; an empty scene heading still starts a
  scene.

## 14. Spreadsheet export, ported from Method Budget (2026-10-11)

- `sched_format::export` is Method Budget's exporter (same owner, same license): our own
  small ZIP and SpreadsheetML writer, deterministic bytes, a bold frozen header row, text
  always as text. Cells here are text, whole numbers and dates; no money, no floats.
- Dates in workbooks are real Excel dates (a whole-number serial styled `yyyy-mm-dd`);
  dates Excel can't show go in as ISO text. Page lengths go in two columns: `eighths`
  (a whole number) and `pages` (text like `1 3/8`).
- CSV follows RFC 4180 (CRLF line ends) and neutralizes text that would start a formula
  (`=`, `+`, `-`, `@`, tab, return) with a leading `'`, unless it is a plain number.
- `sched dood|oneline|elements FILE --xlsx OUT` writes a workbook atomically.

## 15. The desktop app, first slice (2026-10-11)

- `crates/sched-app`: Tauri 2.11.6, tauri-build 2.6.3 and tauri-plugin-dialog 2.7.3, the
  pins Method Budget uses, no default features, no updater (off until public), no fs or
  shell plugin. The lockfile was seeded from Method Budget's, so every crate is one it
  already runs; the 14-day check passes.
- Zero npm, strict CSP, navigation locked, text only through `textContent`. The main
  window is granted exactly the commands its page calls; a test compares the build list,
  the handler list, the capability and the page's calls.
- Rust owns the schedule. Every edit goes through one function that works on a copy,
  runs `Schedule::check`, and becomes one undo step (200 kept). Paths come only from
  native dialogs shown by Rust. Saves are atomic and refuse to overwrite a file that
  changed on disk since it was read.
- Working names until the product is named (D4): "Sched", `org.sched.desktop`, `.sched`.
  The B key for a new day break is provisional until checked against the reference app.
- Full write-up and the "Not yet" list: `docs/APP.md`.

## 16. Saving safety, after Method Budget (2026-10-11)

- Method Budget's design (`src/safety.rs`, `src/safety_app.rs`), scaled down to one
  schedule per window: **autosave into the file by default** (3 seconds after the last
  change, at most 30 seconds after the first unsaved one, at once when the window loses
  focus and before New, Open, close and quit), a switch in the top bar and File > Autosave,
  kept in `settings.json` in the OS app data folder.
- **Recovery copies** for untitled schedules and for unsaved changes while autosave is off
  or paused, in `Recovery/` with an owner lock per running copy (std's file locks, no new
  crate). The next start offers what a dead copy left; a recovered schedule never
  autosaves over its original file, and its first Save there asks.
- **Rolling backups**, 5 per file (Method Budget keeps 10), taken before a save replaces a
  file: always for an explicit Save, at most every 10 minutes for autosaves.
- **Open Recent** (10 files), `.sched` registered in `tauri.conf.json`, and files from open
  events (macOS) or the command line (Windows, Linux) open after the unsaved-changes
  question, in place of the open schedule.
- The outside-change guard covers autosave too: a changed file is never written, autosave
  into it pauses, the work goes into a recovery copy. Resolving it in the app (keep mine,
  load theirs) waits; Save As is the way out.
- Every new path comes from Rust (dialogs, the OS, the recent list, the recovery folder).
  The page passes a recent file's index and a recovery key, both checked against what is
  there. Six new commands, each granted in `capabilities/main.json`.

## 17. The Breakdown tab (2026-10-11)

- One sheet at a time, every header field from the inventory (section 3). Values travel
  as text and are read in Rust, strictly and as integers: shoot time `h:mm` in minutes,
  screen time `m:ss` in seconds, sunrise and sunset 24-hour `hh:mm` in minutes after
  midnight, pages as whole pages plus eighths (eighths of 8 or more roll over). Anything
  else is refused with one plain sentence and changes nothing. Text is trimmed and capped
  at 10,000 characters (names at 200).
- Each field commits when it is left (or on Enter) and is one undo step through
  `Session::edit`; a value equal to what is stored is no step at all.
- Quick add matches names that start with what is typed, then names that contain it,
  leaving out what is on the sheet already. Enter with no match picked adds the name in
  the category shown, adding the category when new and reusing an element of that name
  (ignoring case). Focus stays in the field.
- New sheet goes right after the current one when that one is on a sub-board, else at
  the end of the Boneyard, and at the end of every other board's Boneyard (inventory,
  section 3). Delete sheet asks in the page, removes the sheet and its strip from every
  board, keeps its elements and leaves day breaks where they are.
- Engine (`sched-core/src/model.rs`): `Schedule::untag`, `add_sheet_at`, `delete_sheet`,
  `sheet_place` and `sheets_in_board_order`, with unit tests and a seeded property test
  that keeps `Schedule::check` passing and every sheet once in board order. The file
  format did not change.
- Provisional keys until checked in the reference app (decision 2): Enter on the Board
  opens the selected scene's sheet (double-click too), and Cmd or Ctrl with `[` and `]`
  steps through sheets in board order.


## 18. The recovery lock goes away on a clean exit (2026-10-11)

- The first live run found an `owner-*.lock` left in `Recovery/` after a clean quit. Tauri
  never drops managed state when the app exits, so the store's lock file outlived the
  process. The exit hook (`safety_app::on_exit`) now runs `Safety::close` after keeping
  unsaved work: it unlocks the file, then removes it (unlock first, as Windows can't
  delete an open file). Dropping a `RecoveryStore` does the same. Recovery copies stay.
- A running copy holds its lock exclusively; checking whether another copy runs takes a
  shared lock. So a dead copy's leftover file (unlocked) is never live, even while another
  copy is checking it, and a name that isn't an `owner-*.lock` never counts. The next start
  still removes dead copies' lock files.
- Tests: a clean exit, Don't Save and an unguarded quit each leave no lock file; a stale
  lock with a concurrent shared check is still treated as dead.

## 19. The Elements tab (2026-10-11)

- Categories down the side; for the one picked, its settings and its elements from
  `sched_core::reports::element_list` (sheets, pages, work days, start, finish on the first
  sub-board), in board ID order then by name. Editing follows the Breakdown tab: a field
  commits when left or on Enter, a switch when clicked, each change one undo step, a refusal
  one plain sentence.
- **Built-in categories**: Cast only for now (`model::BUILT_IN_CATEGORIES`), because the
  board and script import find it by name. It can't be renamed, moved or deleted. User
  categories reorder among themselves; a category is deleted only when empty (inventory,
  section 4).
- **Category defaults reach new elements only** (MMS 10). Apply to Existing Elements is an
  explicit action that asks first, gives every element of the category the default hold
  and drop rules, and is one undo step. Automatic board IDs are not applied to existing
  elements; Renumber does that.
- **Names** are unique in a category ignoring case, for elements and categories alike.
  **Board IDs** are letters and digits, at most 12, kept as typed, unique in the category
  by exact text (so `1a` and `1A` differ, as do `07` and `7`, until checked). A lock only
  matters to Renumber.
- **Merge** needs two or more elements of one category and a survivor: every sheet and
  event that listed any of them lists the survivor once, in the place of the first; links
  to the merged ones point at the survivor. The survivor keeps its own name, ID and rules.
- **Move** to another category needs the name to be free there; the board ID stays when it
  is free there too, else it goes (with its lock), and a category that numbers
  automatically gives the next free number.
- **Renumber** walks the list in its shown order from a start number (1 to 99,999):
  locked elements and IDs with letters keep theirs, the numbers those use are skipped, and
  every other element gets the next free number, including elements with no ID yet.
- **Delete** takes the element off every sheet and event. A linked element is refused
  unless the page asked and the user chose Unlink and Delete, which drops the links both
  ways.
- Engine (`model.rs`): `rename_category`, `move_category`, `delete_category`,
  `apply_defaults`, `rename_element`, `is_linked`, `merge_elements`, `move_element`,
  `renumber`, `delete_element`, with unit tests and a seeded property test that keeps
  `Schedule::check` passing through random merges, moves, renumbers, deletes and links.
  `Schedule::check` now also refuses what FORMAT.md already promised: an event naming a
  missing element. It refuses too an element listed twice on a sheet or an event, an
  event that ends before it starts, and two categories with one name.
- View menu: Board, Breakdown, Elements, Day out of days on Cmd or Ctrl with 1 to 4,
  provisional (decision 2).

## 20. The day out of days grid (2026-10-11)

- The Day out of days tab shows the grid, not just the summary: `dood_grid` replaces
  `dood_summary`. One category at a time, Cast by default, from a picker. Rows in the order
  `sched_core::dood::dood` returns them, codes from `dood::Cell::code()`; the page draws and
  never computes a count. The engine was used as it is.
- Columns run from the first to the last date any row in the category covers, so an event
  before the start widens the grid (decision 11). Each column carries its weekday, day of
  the month, month (on the first column and each 1st) and shoot day number. Days off are
  shaded; holidays and company travel days get their own shade.
- A bar marks each row's work span, start to finish, as the reference app draws the span
  when holds are off (behavior note 14). An element with events only has no bar.
- The summary stays at the right in the reference order: Work, Hold, Start, Finish,
  Total (Total = Work + Hold, decision 11).
- Readable at 100+ columns: the ID and name columns stick left, the summary sticks right
  and the header sticks to the top, with fixed column widths in styles.css. Past 80 rows
  only the rows in view (plus 12 either side) are drawn, at a fixed 24 px row height; the
  spacer rows above and below are sized through a constructed stylesheet, Method Budget's
  way around the CSP. Checked in the harness at 300 rows by 140 dates.
- Open, for the next real-app run: whether the reference grid starts at the first event
  or at the first work day when an event comes first (we follow the rows), the code shown
  when a drop starts or ends on an event day (ours: the event's code, no D or P), whether
  a holiday on a weekend counts as worked inside a held span (ours: yes, holiday first),
  and the week layout (we draw one column per calendar day, no week breaks).

## 21. Calendar editing (2026-10-11)

- A Calendar tab for the first board's first sub-board: one month, Sunday to Saturday (the
  week the reference product's day out of days uses), six weeks so every month fits. Each
  date shows its shoot day number or what it is, and its events.
- **A date is exactly one kind**: working, day off, holiday or company travel
  (`model::DayKind`, `Calendar::day_kind`, `Calendar::set_day_kind`). Making a date one kind
  drops the others, so a date is never in two lists. On a weekly day off, working means
  marked back to working: a new `work_days` list in the calendar (FORMAT.md; left out when
  empty, so older files read the same). This is the inventory's "back to working" mark
  (section 7).
- **Weekly days off** (`Calendar::set_weekly_off`): any set but all seven. Dates already
  marked keep their marks.
- **Events** (`Schedule::set_event`, `Schedule::delete_event`): kind travel, fitting,
  rehearsal, or other with what kind it is; an optional name; from and to dates, at most
  366 days apart (each day is a day out of days column, so a mistyped year is refused);
  elements picked from the schedule, each once. Events have no ids in the file yet, so the
  page names one by its place in the list; a stale place is refused.
- Every change goes through `Session::edit` as one undo step; the board's day dates and the
  day out of days follow at once, since both are worked out from the calendar each time.
- Provisional: Cmd or Ctrl with 5 for the tab; arrow keys and Page Up and Page Down in the
  month. Open for the next real-app run: whether a holiday or company travel day on a
  weekly day off counts in a held span (ours: it does, as a holiday), what the reference
  shows for a weekend marked back to working, and whether other event kinds (unavailable,
  camera test) need kinds of their own.

## 22. An MCP server for coding agents, ported from Method Budget (2026-10-11)

- `sched mcp --root DIR [--root DIR...] [--read-only]` (KICKOFF section 6, item 11),
  documented in `docs/MCP.md`. Method Budget's server is the model, and its protocol layer,
  path confinement and schema validator are reused nearly as they are (same owner and
  license): hand-written JSON-RPC 2.0 over stdio on `serde_json`, the same protocol
  revisions (2026-07-28 per request, 2025-11-25 down to 2024-11-05 through `initialize`),
  tools only. Code in `crates/sched-cli/src/mcp/`.
- **No new crates.** `sched-cli` takes `serde_json` at the version `sched-format` already
  pins (`=1.0.151`); `Cargo.lock` gains a dependency edge, not a package.
- **Stateless, with revision tokens.** Method Budget's server remembers the bytes it last
  read per file and keeps undo steps. Ours keeps nothing: every read returns the file's
  revision (FNV-1a 64 of its bytes, the exchange file's `source_hash` function, now public
  as `sched_format::exchange::fnv1a64`), every change requires it back, and a change whose
  revision no longer matches the disk is a `conflict` with nothing written. Simpler, it
  survives a restarted server, and it makes the agent say which read a change is based on.
  No undo tool yet: git, or a copy made first, covers it. The file is compared again just
  before the rename.
- **Every change goes through `sched-core`**, then `Schedule::check` and a round trip
  through the strict reader, before an atomic write that keeps the file's permissions.
  The engine's own refusals and a failed check both write nothing. A new file is
  hard-linked into place so it never replaces one that appeared meanwhile; no tool deletes
  a file.
- **Extensions:** schedules are read and changed as `.sched` or `.json` (the app opens
  both), created only as `.sched`; scripts are `.fountain` or `.fdx`. Hidden names below a
  root and symlinks for writing are refused, as in Method Budget.
- **24 tools**: 9 that read (`list_schedules`, `read_schedule`, `get_sheet`,
  `list_elements`, `days`, `dood`, `one_line`, `budget_exchange`, `check`) and 15 that
  write (`new_schedule`, `import_script`, `set_sheet_fields`, `tag_element`,
  `untag_element`, `add_element`, `set_element_rules`, `set_board_id`, `move_strips`,
  `insert_day_break`, `delete_day_break`, `auto_day_breaks`, `set_start_date`,
  `set_day_kind`, `add_event`). Integers only (eighths, minutes, seconds, ids), dates as
  `YYYY-MM-DD`. Tools on one sub-board default to the first board's first sub-board, as
  the command line does, and take `board` and `sub_board` ids for the others.
- Left for later: a live bridge into the running app (designed in Method Budget's
  `docs/MCP.md`, not built there either), diff, and tools for banners, sub-boards and
  category settings.

## 23. Reports, printing, PDF and export in the app (2026-10-11)

- **A Reports tab with five reports**, each from the engine's report data for the first
  board's first sub-board: one-line schedule, shooting schedule, day out of days (grid by
  category, then the summary), elements, daily category totals. `report_data`
  (`src/reports.rs`) sends plain values; the page arranges them and never computes a
  count. The layouts are our own and plain (KICKOFF and reports.rs: never another
  product's report designs).
- **We lay out the pages ourselves, as Method Budget does** (its decision 28): the system
  web view repeats no table headings, prints no page numbers and ignores `@page` margin
  boxes, so `ui/js/report.js` measures rows in a hidden page-wide box and deals them onto
  Letter-sized page boxes, each with a header (schedule title, report, date printed) and
  a footer (board and sub-board, Page X of Y). The preview is the same pages scaled
  down. Page breaks fall between days: a one-line day's rows are kept together when they
  fit on a page, and each shooting schedule day starts a new page. Short flows (a small
  category's grid, the summary) share a page.
- **The day out of days grid on paper** splits long date ranges: 28 dates to a landscape
  page, 18 to a portrait one, each part with its ID and name columns again. Landscape is
  the default for it; every report can switch.
- **PDF without new crates**: Method Budget's `pdf_mac.rs`, ported unchanged in substance
  (AppKit's `NSPrintOperation` on the WKWebView with a save disposition and a file URL,
  plain Objective-C runtime calls, a one-method delegate class of its own name). Print…
  is the same call with the print sheet shown and the orientation preset. The PDF is
  written to a hidden temporary file beside the chosen one and renamed into place when
  the job is over, so it is atomic like every other write. Windows and Linux use
  `window.print()` and the system dialog's PDF choice for now.
- **File > Export** offers the three reports `sched_format::export` already builds (day
  out of days summary, one-line, elements), as a workbook or one CSV file with a byte
  order mark; no new builders were needed. **Export for Budget** writes
  `sched_format::exchange::to_string` for the first sub-board, stamped with today's UTC
  date like `sched budget`, from the schedule as it is in the app (its hash matches the
  saved file once saved). Both suggest the schedule's folder and name, never replace the
  open schedule's file, and write atomically.
- **Menus and keys**: File > Export… (Cmd or Ctrl+E), Export for Budget…, Print… (Cmd or
  Ctrl+P), Save as PDF…; View > Reports (Cmd or Ctrl+6). Print and Save as PDF from the
  menu show the Reports tab and print its report. The keys are provisional (entry 2).
- Not yet: report options and presets, other paper sizes, other sub-boards, the shooting
  schedule and daily totals as spreadsheets, PDF without the dialog on Windows and Linux.

## 24. The board at 1,500 strips: draw what is in view, keep sending the whole view (2026-10-11)

- **Measured first**, with `sched sample`'s 1,500 scenes (1,792 strips, 292 days, one
  sub-board) in the page harness (`?big`) and `perf_edit_and_view_at_1500_strips`. Before:
  the page redrew every row (about 12,300 DOM nodes) in 15 to 22 ms and the browser took 25
  to 34 ms more to lay it out, so 40 to 55 ms an edit. In Rust an edit is about 0.7 ms,
  building the view 1.4 to 1.9 ms and serializing it 0.3 to 0.4 ms (346 KB); the page parses
  it in about 0.6 ms.
- **The board draws only the lines in view**, as the day out of days grid does (entry 20):
  each strip list (a sub-board, the Boneyard) is a run of fixed 26 px lines (day header,
  strip, day break, caption), so a line's place is arithmetic; past 120 lines in all, each
  list draws the lines in view plus 15 either side between two spacers sized through a
  constructed stylesheet (the CSP allows no inline styles). Sub-board headers, the column
  heads and the empty-list notes are always drawn. One click and one double-click listener
  on the board replace one per row.
- **Selection and keys work on the data**: the page keeps every selectable row (id, list,
  index, sheet) from the view, so Up and Down, Option-Up and Option-Down across day breaks
  and sub-boards, B and Enter work on rows not drawn; selecting one scrolls just enough to
  show it and draws it. After a move or a new day break the selection is scrolled into
  view. The tabs share one scroller, so the board remembers its place while another tab is
  open. Rows carry aria-posinset and aria-setsize for screen readers.
- After: an edit's page work (parse, redraw, layout) is about 3 ms with about 280 board
  nodes; a key step about 1 ms. Under the same throttled conditions (a hidden browser pane)
  the old page took 175 to 235 ms and the new one 11 to 18 ms.
- **The view is still sent whole.** Its JSON costs about 1 ms an edit (0.3 to 0.4 ms to
  serialize, 0.6 ms to parse), so it isn't the cost worth a protocol change now. Dropping
  empty strip fields would save only 13 %, and leaving out unchanged sub-boards saves
  nothing when, as usual, one sub-board holds the strips and any move renumbers its days.
  Revisit if the transfer shows up in a real window. `get_view` and every command still
  return the whole view.

## 25. Diff, three-way merge and "explain this count", ported from Method Budget (2026-10-11)

Roadmap phase 3, after Method Budget's decisions 43 and 44 (same owner and license). Full
write-up: `docs/MERGE.md`.

- **Identity, never position.** `sched_core::diff` matches categories, elements, sheets,
  boards, sub-boards and strips by id, calendar marks by date and events by value (events
  have no ids yet, so an edited event is one removed and one added). Every change has a
  kind, a subject with ids, fields before and after, and one plain sentence. Strips that
  changed place are reported **one by one**: a strip in another list is moved; one in its
  own list is moved only outside a longest run of strips that kept their order (Method
  Budget's longest increasing subsequence), so one dragged scene doesn't mark every strip
  it passed. Positions give the shoot day. `next_id` and the element and sheet list order
  are bookkeeping and are not listed.
- **Merge granularity: field by field.** Different things and different fields merge on
  their own; a sheet's elements and an element's links id by id; calendar dates date by
  date; the weekly days off as a whole. Conflicts: the same field changed both ways;
  deleted on one side and changed or newly used on the other (tagged, put on an event,
  linked, given elements); one list reordered both ways (strips of a sub-board, the
  Boneyard, categories, sub-boards); one strip moved to different lists. **Every conflict
  takes ours by default** and is listed with ours, theirs and the base; keys are stable,
  so `merge_with` takes a pick per key. Keeping something against a delete keeps the tags,
  links and strips the deleting side took with it.
- **Strip order is Method Budget's row order rule**: only strips in all three versions of
  a list count; the side that reordered them wins (ours when neither did); what the other
  side added or moved in goes right after the strip it follows there; both reordering
  differently is a conflict, never an interleaving. A strip's list merges like a field.
- **New ids on both sides.** Categories added on both sides with one name are one, and so
  are elements with one name in one category; anything else added on both sides with one
  id is one thing only when identical (a strip only on the same board and list), else
  theirs and everything hanging from it get ids above every id in the three schedules.
- **The result always passes `Schedule::check`.** What doesn't fit falls back to ours for
  the part at fault and is reported (`Merged::fallbacks`): a name or board ID that now
  collides gets ours' back, or the newcomer joins the one of that name; references to
  what is gone are left off; a board missing a sheet gets it at the end of its Boneyard; a
  board left without sub-boards gets ours'. Ours whole is the last resort, never reached
  in the property tests (3,000 stress cases of up to 12 edits a side while building it).
- **Property-tested** with random schedules and random edits through the engine (300 cases
  in the suite): `merge(base, x, base)`, `merge(base, base, x)` and `merge(base, x, x)` are
  `x` with no conflicts and no fallbacks; any two edit sets merge, settled either way,
  into a schedule that passes `check`; the same merge gives the same conflicts. A diff of
  a schedule with itself is empty, and every random edit shows up in one.
- **Explain this count** (`sched_core::explain`): for one element on one sub-board, the
  totals, what work is made of, the rules, every gap (its length in calendar days against
  the minimum, dropped or held and why: too short, a longer or earlier equal gap took the
  one drop, drops off, holds off) and every date of the row with its code, where it counts
  and why, its shoot day, sheets and events. It uses the day out of days' own pieces: the
  drop choice moved into `dood::gaps`, which `dood` now calls too, so they can't drift; a
  property test holds every code and count to `dood` on random schedules and rules.
- **Command line**: `sched diff`, `sched merge BASE OURS THEIRS [--out FILE] [--theirs]`
  (exit 1 when conflicts were settled; the file is written either way; never over an
  input), `sched merge-driver %O %A %B %P` for git (writes over ours, exit 1 on conflicts so
  git marks the file `UU` without conflict markers; an empty base is refused and ours left
  alone) and `sched explain FILE ELEMENT [--category CAT]`. A test runs a real `git merge`
  in a temporary folder with the user's and system git settings left out.
- **MCP**: two read tools, `diff` `{path, other}` and `explain`; 26 tools, 11 that read.
- No new crates; `sched-core` stays dependency-free and integer-only.
- Open: events still have no ids, so an event edited on one side and deleted on the other
  merges as the edited one, and one edited both ways keeps both versions; whether to
  give events ids is a format question for later. A sheet deleted on one side and only
  moved on the board on the other is deleted without a conflict. Not yet: the app screens
  (docs/APP.md, "Not yet"), a merge tool over MCP, and matching an element renamed on one
  side and added under the old name on the other.

## 26. The app's Explain, What Changed and Merge Changes From screens (2026-10-11)

The app screens for entry 25's engine features, after Method Budget's "Explain this
number" and "What changed and merging" (its `src/diffmerge.rs` and `ui/js/diffmerge.js`),
scaled down to one schedule per window and no versions. Details in docs/APP.md.

- **Explain this count is a panel beside the grid, not a dialog**, so a click on one of
  its dates can scroll the grid to that column and mark it while the explanation stays
  in view. It opens from a row's Work, Hold or Total (buttons, so the keyboard reaches
  them), Enter on the selected row, or Edit > Explain This Count (⌘I, Ctrl+I). The grid
  gained a selected row (click an ID or name; Up and Down while the grid has focus,
  scrolling a row that isn't drawn into view past 80 rows). Up, Down and Enter are the
  grid's own keys, like the Calendar's arrows; ⌘I is in the one shortcut table and the
  Edit menu. All provisional (entry 2). The panel shows `sched_core::explain`'s sentences
  only, and is worked out again after every change while it is open.
- **What Changed** compares the schedule in the window (newer) with a file picked in an
  Open dialog shown by Rust, or with its own file as it is on disk (a second menu item,
  What Changed Since Saved). The changes are the engine's sentences under its headings
  (Schedule, Categories, Elements, Sheets, Boards, Calendars, Strips). Read-only: no
  click to go to a change yet.
- **Merge Changes From: the base comes from the app's rolling backups.** Schedule files
  carry no provenance (Method Budget finds the base from what the copy was shared
  from), so the base is the newest backup of this file made at or before the other
  copy's last change. Every backup predates the schedule in the window, which was saved
  over it; a backup made before theirs was last written is the latest version theirs
  could have started from. When that can't be told (no backups yet, every backup newer
  than theirs, theirs' time unreadable) the base is the file as it is on disk now, and
  the dialog says so in a highlighted sentence: anything saved into the file after the
  copy was made reads as undone in theirs. The dialog lists every backup (newest first)
  and the file on disk to pick instead; a backup is named by its file name and checked
  against the backups folder's own listing, so the page never names a path. A schedule
  never saved has no base and is refused; so is merging from the schedule's own file
  (What Changed Since Saved compares with it).
- **Preview, then one undo step.** Each conflict shows yours, theirs and what it was, with
  Keep yours (the default, as `merge_with` settles it) and Keep theirs; every choice works
  the merge out again in Rust. The preview lists what the merge changes in this schedule
  (a diff of the window's schedule with the merged one), how many of theirs got new ids,
  and every fallback. Merge applies `merge_with` to the schedule as it is then inside
  `Session::edit`, so it is one undo step and passes `Schedule::check` (which the merge
  guarantees; the edit checks again). Picks are capped (50,000, keys of 400 bytes) and
  must be `ours` or `theirs`. Changing the base clears the picks, as conflict keys belong
  to one merge.
- **Files stay in Rust.** Both Open dialogs are shown by Rust; theirs is read once with the
  same strict reader as Open and kept by token (the last 8 picked), so the preview and
  the merge use the same copy. Five new commands (`explain_count`, `what_changed`,
  `merge_pick_file`, `merge_preview`, `merge_apply`), each in `build.rs`, the handler list,
  the capability and the page's calls; the capability test holds the four together.
- No new crates; `sched-core` unchanged.
- Open: a click on a change to go to its sheet, element, strip or date; comparing with or
  merging against a chosen backup from What Changed; resolving the outside-change banner
  by merging with the file on disk, which needs the version last read to be kept as the
  base.

## 27. The product is named Method Schedule (2026-10-11, the owner)

The owner chose **Method Schedule** from the naming board (private notes), replacing the
working names of entry 4. Nothing had shipped, so the rename is clean, with **no
compatibility code for the working names**: a file saved with `"format": "sched"` or
named `.sched` is no longer recognized (as Method Budget's entry 46 did). Entries 1 to 26
keep the working names as they were written.

- **Crates and folders**: `sched-core`, `sched-format`, `sched-cli`, `sched-app` become
  `method-schedule-core`, `method-schedule-format`, `method-schedule-cli`,
  `method-schedule-app`; Rust paths `method_schedule_core`, `method_schedule_format`,
  `method_schedule_app`.
- **Command line**: the binary `sched` is `method-schedule` (help text, the
  `method-schedule: ...` error prefix, the MCP server name). The git merge driver is
  `merge.method-schedule.driver`, with `*.mtsc merge=method-schedule` in `.gitattributes`.
- **App**: product name and window title "Method Schedule", identifier
  `com.methodschedule.desktop`, executable `method-schedule-app`. The placeholder icon
  stays until a real mark is drawn.
- **File format**: `"format": "method-schedule"`; the budget exchange file is
  `"format": "method-schedule-days"`.
- **File extension `.mtsc`**: four letters, nothing like the reference product's. The
  association is named "Method Schedule schedule", media type
  `application/vnd.method-schedule+json`, exported type
  `com.methodschedule.desktop.mtsc`. Open dialogs and the MCP server still accept `.json`.
- **Styles**: the print class `.sched-report` is `.method-report`, as in Method Budget.
- **Docs and license**: `<Product>` is Method Schedule everywhere except the historical
  brief (docs/KICKOFF.md) and the earlier entries here; the copyright line is "Method
  Schedule contributors".
- **Domain**: methodschedule.com (the owner has it).
- **Later, the owner's calls**: the GitHub repository name and the neutral commit identity.

## 28. Find and replace, and conflicts between sub-boards (2026-10-11)

Inventory sections 5 (board find), 8 (conflicts) and 13 (find and replace); behavior notes
03 and 05. Engine code in `method_schedule_core::find` and `::conflicts`, no new crates.

- **Find is typed, as in Method Budget** (its entry 26): sheet header fields (each, or all),
  element names, category names, banner text and event names; match case, whole word (no
  letter, digit or `_` either side), a field filter, and a scope (the whole schedule or one
  sub-board: its sheets and banners, its events, and the elements on them with their
  categories). Its matcher is ported as it is. Results are typed places (sheet and field,
  element, category, banner strip, event index) with a short context line, in board order:
  each board's sub-boards' strips (a sheet the first time it is met), each sub-board's
  events after its strips, sheets met only in a Boneyard, then categories each followed by
  its elements. Numeric fields and the advanced finds (equal, starts with, AND/OR rows,
  page ranges) are later.
- **Replace works on picked results on a copy, and returns the new schedule or a
  refusal.** Every pick must still match the search (else stale, nothing changed).
  Refused: two elements of one category or two categories with the same name ignoring
  case (the rule `same_name` and `check` use), an empty name, the built-in Cast renamed,
  a line break or other control character in a one-line field (all but synopsis and
  comments). Names are trimmed as a rename trims them; a place that comes out the same is
  not a change, so replacing a match-case find with its own text changes nothing (a
  property test, with "replace never breaks `check`"). A blank replace on a pick-list field
  unlinking its value (note 05) has no meaning yet: header fields are free text.
- **Command line and MCP**: `find FILE TEXT`, `replace FILE TEXT NEW (--all | --pick N,N)`
  (an atomic save, then each change before and after; nothing written when nothing
  changes), `conflicts FILE [--board ID]`; MCP `find` and `conflicts` read, `replace`
  writes with the revision token. Picks are find's result numbers from 1, safe because the
  revision pins the file the numbers came from.
- **A conflict is one element on one date in two or more sub-boards of the same board**
  (E-E1); other boards are other arrangements of the same sheets and are never compared.
  Dates come from each sub-board's own start date, calendar and day breaks, as the day out
  of days has them: work days only, not holds; strips below the last day break are not
  scheduled; a sub-board without a start date isn't compared and is listed as such.
  Elements of a category excluded from the board are left out (E-E3's proposed rule).
- **Event clashes are a separate, informational list, never conflicts.** The current
  manual says events don't cause conflicts; an older video said overlapping events did
  (E-E2). We report, apart and labeled informational: an element with two or more events
  on a date anywhere on the board, or an event in one sub-board and strip work in another
  (an event on a day the element works in the same sub-board is normal, E-D6).
- Open, for the real app (engine-checks queue E-E5 to E-E7): holds, excluded categories,
  undated sub-boards; and whether replace refuses or merges a rename that collides.

## 29. Drag and drop on the board, and selecting several strips (2026-10-11)

- **Selection.** Click selects one strip; Shift-click selects the run from the last strip
  clicked (the anchor) in screen order, across day breaks, sub-boards and the Boneyard;
  Command-click (Ctrl-click elsewhere) adds or drops one. Shift+Up and Shift+Down grow or
  shrink the run from the anchor. Up and Down select one strip again. Keys provisional
  (engine check E-S7).
- **Dragging** uses pointer events, not the browser's drag and drop, so the board decides
  everything: a press that moves 5 px becomes a drag of the selection (or of the pressed
  strip alone when it isn't selected). The line under the pointer comes from its list's
  top and the fixed 26 px line height (entry 24), so a drop works on lines that weren't
  drawn when the drag began. A drop in the upper half of a strip or day break goes above
  it, in the lower half below it; on a day header or caption it goes first in that run;
  past a list's end, last; on an empty sub-board, first. Near the scroller's top or bottom
  (48 px) the board scrolls, faster nearer the edge, and draws what comes into view. The
  scrolling runs on a 16 ms timer rather than animation frames, so it keeps going where
  frames are paused. Escape cancels. The drop indicator is a class on the line it touches;
  the label following the pointer is placed through a constructed stylesheet with numbers
  only, so the CSP stays as strict.
- **One command, one undo step**: `move_strips` now takes the board (`board`,
  `strips`, `sub_board`, `index`, positions counted before the move). A selection that
  isn't contiguous gathers at the drop in board order (E-S6). Day breaks and banners are
  refused in the Boneyard: the page marks the drop red and says so without calling, and
  the engine refuses it too (E-S5 asks whether the reference app parks breaks there).
- **Engine**: `Schedule::move_strips` gathered the Boneyard's strips first; it now takes
  them in board order (each sub-board, then the Boneyard), as the docs always said.
- **Keyboard moves keep working with a selection**: Option-Up and Option-Down move the
  selected strips as a block, as the top strip moves going up and the bottom one going
  down, so a contiguous block steps over one neighbor (`blockStep` in board.js).
- Delete and Backspace delete the selected day breaks and banners (`delete_strips`, which
  replaces `delete_day_break`); scenes in the selection are left alone, and banners ask
  first (E-B13).

## 30. Banners on the board (2026-10-11)

- Board > Insert Banner Below (⌘B, the key the reference app's manual gives) and Insert
  Banner Above (⇧⌘B) put a banner below the bottommost selected strip or above the
  topmost (E-B12); with nothing selected, at the end or start of the picked sub-board.
  Banners never go in the Boneyard.
- A banner has text (trimmed, required, at most 200 characters) and a duration typed as
  `h:mm` like a shoot time, read by the same strict reader (`90` is refused, empty is
  0:00). The duration counts toward the day's time, which the engine already did. The
  board draws a banner in its own color with its duration at the right.
- Enter or a double-click on a banner edits it (Edit Banner also in the Board menu); the
  dialog has Delete Banner. Each change is one undo step: `insert_banner`, `set_banner`
  (`Schedule::set_banner`, new, refuses anything but a banner) and `delete_strips`.
- Styling (font, color, alignment, thin height) and copy and paste of strips stay for later
  (inventory section 5).

## 31. Boards and sub-boards in the app; every view by id (2026-10-11)

- **The board shown** lives in the session (`Session::shown_board`), not the file and not
  the history: choosing a board (`show_board`) is not an edit. When it is gone (deleted,
  or undone away) the first board shows. Adding or duplicating a board shows the new one.
  The view carries every board's id and name for the picker.
- **The sub-board picked** for the Elements, Day out of days, Calendar and Reports tabs,
  File > Export, Export for Budget and Explain is the page's: a picker beside the board
  picker, kept while it is on the board shown, else the first. Sub-board ids are unique
  across the schedule, so a sub-board names its board.
- **Commands take ids** instead of assuming the first board: `dood_grid`,
  `explain_count`, `elements_view`, `calendar_view`, `set_day_kind`, `set_weekly_off`,
  `save_event`, `delete_event`, `report_data`, `export_report` and `export_budget` take
  `sub_board`; `get_sheet`, `new_sheet`, `delete_sheet`, `move_strips`, `delete_strips`,
  `set_banner` and `sort_strips` take `board`. The Breakdown tab's board order and a
  sheet's place are the board shown's. The command line and the MCP server still use the
  first board.
- **Engine** (model.rs): `rename_board`, `move_board`, `delete_board` (not the last; the
  sheets stay on the others), `rename_sub_board`, `move_sub_board`, `set_start`,
  `board_of_sub`, and the name checks `board_name` (unique among boards ignoring case)
  and `sub_board_name` (unique within its board; a duplicated board keeps its sub-boards'
  names). `add_board`, `duplicate_board`, `add_sub_board` and `delete_sub_board` (scenes
  to the Boneyard) were there. Engine check E-S1 still asks how the reference app scopes
  names.
- **The page**: Board > New Board, Duplicate Board (named "<name> copy", then "copy 2"),
  New Sub-board and Boards and Sub-boards, a dialog that adds, renames (in place),
  reorders, duplicates and deletes boards and sub-boards; a delete asks with a second
  click. Each sub-board's start date stays on the Board tab. Ten new commands, each in
  `build.rs`, the handler list, the capability and the page's calls.

## 32. Sorting strips (2026-10-11)

- `Schedule::sort_strips(board, sub, keys)` (model.rs) sorts a sub-board, or the
  Boneyard: banners go to the top in the order they were in, day breaks are removed (the
  user breaks the days again), and scenes follow by up to three keys (the app refuses
  more, and a field twice), each ascending or descending. Ties keep their order. A field
  left empty sorts last both ways (E-S8). One undo step.
- Fields: scene, I/E, set, time of day, location, pages, script day, unit, sequence and
  episode. Pages sort by length; the text fields sort naturally (`natural_cmp`): runs of
  digits by value, other runs ignoring case, a number before text at the same place, a
  shorter string first. So `1 < 1A < 2 < 9 < 10 < 10A < 10b < 11 < A1 < A12` (the proposed
  rule for E-S4) and episodes and script days sort as numbers. I/E and time of day sort
  alphabetically for now (E-S9).
- Property test: random schedules over two sub-boards and the Boneyard with breaks and
  banners, random keys: every sheet still once, `check` passes, other lists untouched, no
  day breaks, banners first in their old order, scenes in key order with ties in their old
  order.
- Board > Sort Strips sorts the list holding the selected strip (else the picked
  sub-board) and says to break the days again.

## 33. Red flags: availability limits on elements (2026-10-11)

Inventory section 9; behavior notes 03 and 09 (MMS 5 to 10); engine check E-E4. Engine
code in `method_schedule_core::model` and `::redflags`, no new crates, integers only.

- **Model.** `Schedule::red_flag_types` is a user list of types (`RedFlagType`: stable id,
  name unique ignoring case), in display order, global across boards. Each element holds
  its `red_flags` (`RedFlag`: stable id, type, dates, note, `hard`); several per element
  and per day are allowed (MMS 10 allows several, MMS 6 one). Dates are a range (`from`
  to `to`, inclusive) or open-ended: `not_before` (every date before it ruled out) or
  `not_after` (every date after it). `check` validates the type, the ids and the range's
  order. Engine edits: `add_red_flag_type` (returns the existing type of that name),
  `rename_red_flag_type`, `move_red_flag_type`, `delete_red_flag_type` (refused while a
  flag uses it, `Error::InUse`), `set_red_flag` (id 0 adds, else replaces in place; a flag
  never moves to another element by an edit), `delete_red_flag`. Merging elements carries
  the merged elements' flags to the survivor; deleting an element deletes its flags.
- **Hard or soft** is our addition: the research shows no severity, and both MMS
  conflicts and red flags are warnings. `hard` is kept for people and lists; the engine
  treats both alike and nothing is ever blocked (E-E10). Open ends are our addition too
  (E-E11): they say "can't start before" and "gone after" without an invented far date.
- **Left out** for now: colors per type, the built-in DOOD types (in MMS 5 and 6 they put
  TR, FT and R on the DOOD; our events do that, D11), usage counts, entering one flag for
  several elements at once (an app matter), the red flag report and PDF, the manager
  table, and searching flag notes with find.
- **A clash** (`redflags::red_flags(schedule, board, sub_board)`) is a flagged element
  working on a date its flag rules out, or at a counted event (travel, fitting,
  rehearsal) that date. Work dates come from the sub-board's start date, calendar and day
  breaks exactly as for conflicts (the same `sub_board_work`): strips below the last
  break aren't scheduled; with no start date the strips have no dates and only events are
  checked (`undated`). Holds, holidays counted as worked and other events never clash
  (E-E8). Every element counts, also one in a category excluded from the board, since a
  flag is set on purpose (E-E12). Each clash names the date, element, flag, type,
  `hard`, the shoot day and sheets, and the events; the list is ordered by date, then
  the element's place in the schedule, then the flag's place on the element. Conflicts
  don't look at flags (E-E13).
- **File format** (additive; files without red flags read and write back unchanged):
  top-level `red_flag_types` (after `categories`), and `red_flags` on an element, each
  `id`, `type`, then `from` and `to`, or `not_before`, or `not_after`, then `note` and
  `hard`, defaults left out. A reader refuses any other mix of date keys.
- **Diff and merge** by id, same rules as the rest (docs/MERGE.md): types like
  categories (renamed, moved, added, removed; a new flag using a type counts as changing
  it, so deleting it against that is a conflict); flags field by field inside their
  element (element, type, dates, note, hard). Repairs: duplicate type names as for
  categories, a type still used by a kept flag stays, a flag on two elements stays where
  ours has it. The merge and diff property tests now make random types and flags.
- **Command line and MCP**: `method-schedule redflags FILE [--board ID] [--sub-board ID]`
  (first board and sub-board by default); MCP `red_flags` reads (types, every flag, the
  clashes on one sub-board), `set_red_flag` and `delete_red_flag` write with the revision
  token. `set_red_flag` takes a type id or a `type_name` it adds when new.
- Open, for the real app (engine-checks queue E-E8 to E-E14): what triggers a clash, how
  it shows on the board and calendar, hard against soft, open ends, excluded categories,
  conflicts on flagged elements, and the built-in types.

## 34. The app's Find and Replace panel and Conflicts tab (2026-10-11)

The app screens for entry 28's engine, after Method Budget's Find tab (its `src/find.rs`
and `ui/js/find.js`), scaled down to one schedule per window. Details in docs/APP.md.

- **Find is a panel at the right, not a tab or a dialog.** A click on a result goes to
  another tab (the Breakdown tab for a sheet field, the Elements tab for an element or a
  category, the Board for a banner, the Calendar for an event), so the results must stay
  in view across tabs; Method Budget's sits in its tools panel for the same reason. The
  panel has the search text, Match case, Whole word, Look in (every `FieldFilter` key, plus
  everything; a Rust test holds the page's list to the engine's keys) and Where (the whole
  schedule, or the sub-board picked in the board bar, as entry 31's tabs use). It searches
  as you type and again after every change, so results that no longer match drop out.
- **A result travels as a key** (`sheet:12:set`, `element:5`, `category:2`,
  `banner:1:3:40`, `event:1:3:0`) with the ids the page needs to go there and our own
  place words (`Scene 12, Set`, with the Breakdown tab's field names, rather than the
  engine's `Scene 12 set`). Replace sends the checked keys back; Rust parses each strictly
  and the engine refuses any that no longer match the search. At most 1,000 results are
  listed (the count says how many match), as in Method Budget.
- **Replace is one undo step through `Session::edit`**, with every result checked to begin
  with. Its refusals are our own sentences, not the engine's `Display` (which starts in
  lower case and reads "a element"): two elements of one category or two categories named
  alike, an empty name, Cast renamed, a line break in a one-line field, stale results.
  A refused replace keeps the results and the sentence in the panel.
- **Find keys**: Edit > Find (⌘F), Replace (⇧⌘F, Method Budget's key), Find Next (⌘G) and
  Find Previous (⇧⌘G), all provisional (entry 2). They work from a text field too, on
  Windows and Linux as on macOS (where the menu answers them); Enter and Shift+Enter in the
  search box step through the results while it keeps focus.
- **Conflicts are a tab** (View > Conflicts, ⌘7), the seventh, since the list is a view of
  the board like the others, not a tool used beside them. It is for the board shown;
  conflicts first, then under "Informational, not conflicts" the event clashes and the
  sub-boards with no dates, as entry 28 has them. Every scene links to its strip on that
  sub-board (Rust finds the strip), every date and event to that sub-board's Calendar on
  the date, an undated sub-board to its start date on the Board tab.
- **The badge**: the view carries how many conflicts the board shown has, so the tab's
  red count is right after every command without another call. A board with fewer than
  two dated sub-boards can't have any and isn't worked out; otherwise the count costs
  about 1 ms at 1,500 strips (`perf_edit_and_view_at_1500_strips`: view 1.4 ms with one
  sub-board, as before; 3.1 ms when every view worked conflicts out).
- Three new commands (`find_text`, `replace_text`, `conflicts_view`), each in `build.rs`,
  the handler list, the capability and the page's calls. No new crates; the engine is
  used as it is.
- Open: the keys checked live; conflicts marked on the strips and the calendar dates; a
  find bar per tab; numeric and advanced finds (entry 28).

## 35. Merging and duplicating breakdown sheets (2026-10-11)

Inventory section 5 ("merge strips into one sheet", engine check E-S3) and section 3
("duplicate to split a scene"); behavior notes 02 and 04. The manual says two or more
selected strips become one sheet after a confirmation, and that duplicate copies the
header and elements, but not how fields combine. Engine code in
`method_schedule_core::model`, additive, no new crates, integers only.

- **`Schedule::merge_sheets(survivor, others) -> SheetMerge`.** The survivor keeps its id
  and its place on every board; the others are deleted and their strips leave every
  board. Banners and day breaks around them stay where they are, as when a sheet is
  deleted (E-S15). "Board order" is the first board's order (each sub-board, then the
  Boneyard), whatever order the ids were given in.
- **How fields combine** (E-S3's proposed rule, except that the header comes from the
  survivor, not "the first"):
  - `scenes`: each sheet's non-empty value in board order, joined by `", "` (`12, 13`);
  - `synopsis` and `comments`: the same, joined by a line break (both are multi-line
    fields, entry 28); an exact repeat is left out of all three;
  - `eighths` summed; `shoot_minutes` and `screen_seconds` summed over the sheets that
    have one, none when no sheet has one;
  - `elements`: every sheet's elements in board order, each once (so no tag is lost; the
    survivor's own may come after an earlier sheet's);
  - every other field (`int_ext`, `set`, `time_of_day`, `sunrise`, `sunset`, `location`,
    `script_pages`, `script_day`, `unit`, `sequence`, `episode`): the survivor's value,
    or, when the survivor's is empty, the first value in board order, so nothing is
    blanked by a merge. Every other sheet's different value is reported as dropped.
- **The report** (`SheetMerge`): the survivor, the merged sheets in board order, each
  field that changed on the survivor (before and after, as text, in the file's field
  order), the elements it gained, and every dropped value (sheet, field, value; by field,
  then board order). Times are `h:mm` (sunrise, sunset, shoot time) and `m:ss` (screen
  time), lengths as eighths text.
- **Refused, nothing changed**: an unknown sheet (`NoSuchSheet`), no others, the survivor
  among the others ("can't be merged with itself"), a sheet given twice, a sum past
  `u32` (`Broken`). Merging across sets, I/E or sub-boards is allowed (E-S14).
- **`Schedule::duplicate_sheet(sheet) -> SheetId`** copies every field and element to a
  new sheet (the end of `sheets`, creation order). On every board where the original is
  on a sub-board the copy goes right after it; where it is in the Boneyard, at the
  Boneyard's end. The reference app puts it after the selection on the active board and
  at the Boneyard's end on the others (note 02); with no "active board" in the engine,
  after the original is the nearest reading, and the app can move it (E-S17).
- **Diff and merge** need nothing new: a merged-away sheet diffs as removed and the
  survivor as changed. A three-way merge where one side merges sheets and the other
  edits a merged-away one is a delete-against-edit conflict on that sheet, settled for
  ours by default; settled for theirs, the sheet comes back with the edit on every board
  (the survivor keeps the merged fields). The merge property tests now merge and
  duplicate sheets at random.
- **Tests**: field rules and the report on a three-sheet sample over two boards; each
  refusal leaving the schedule equal; duplicate placement on a sub-board, at a
  sub-board's end and in the Boneyard; a property test over random schedules (sub-boards,
  day breaks, a duplicated board) with random merges and duplicates: `check` passes,
  every remaining sheet is on every board once, the merged ones on none, the survivor
  in the list it was in, no tag lost or doubled, the sums right, other sheets untouched.
- **Command line and MCP**: `method-schedule merge-sheets FILE SURVIVOR OTHER...` and
  `method-schedule duplicate-sheet FILE SHEET` save atomically, then print the report
  (or where the copy went); MCP `merge_sheets` and `duplicate_sheet` write with the
  revision token (docs/MCP.md). The app's menu items are left for the app's own entry.
- Open, for the real app (engine-checks queue E-S3, E-S14 to E-S17): which values win,
  merging across sets, banners and day breaks between merged strips, where the merged
  strip lands, and where a duplicate lands.

## 36. Red flags in the app (2026-10-11)

The app screens for entry 33's engine (inventory section 9). Engine code used as it is
(`Schedule`'s red flag edits and `redflags::red_flags`); no new crates. Details in
docs/APP.md.

- **Managing flags is in the Elements tab's element editor**: a Red flags table (type,
  dates in words, hard or soft, note) with Add Red Flag…, Edit and Delete (it asks). The
  flag dialog takes a type, the dates as a range, "not before" or "not after", each typed
  `YYYY-MM-DD` and read strictly in Rust (`2027-3-2` and `2027-02-30` are refused; a
  range's last date may not come before its first; an empty To in a range takes the From
  date on the page), a note and Hard (on by default: a flag is usually set because the
  person can't work). Entering one flag for several elements at once is left for later.
- **Red flag types have a small dialog** (Edit > Red Flag Types…, and buttons on the
  Elements editor and the Red flags tab): add, rename in place, Up and Down, Delete with
  a second click. Names are unique ignoring case; adding a name already there is refused
  rather than quietly reusing the type, as the engine's `add_red_flag_type` would (the
  dialog would otherwise seem to do nothing). A type in use is refused with "Unavailable
  is used by red flags, so it can't be deleted. Change or delete those flags first." Each
  type shows how many flags use it. The view carries the type list (a few names), so the
  dialog and the flag dialog need no extra call.
- **Clashes show where they happen**, from `red_flags()` per sub-board: on the Board a
  scheduled strip whose day a flag rules out gets a red ⚑ before its scene number and a
  red right edge, with the element, type, dates and hard or soft as the mark's accessible
  name and title; in the Calendar a date with a clash gets the mark, a red underline and
  the clashes in its title and in the side panel; in the day out of days a clashing cell
  gets a red outline (a class, `g-flag`; no run-time sizes, so no constructed stylesheet
  rule) and the clash in its title. Event-only clashes (a fitting on a flagged date) mark
  the calendar date and the cell, not a strip.
- **A Red flags tab** (View > Red Flags, ⌘8, provisional), the eighth: every clash on the
  sub-board picked in the board bar, in the engine's order (date, then element, then
  flag), each with date, element (a link to it on the Elements tab), category, type, hard
  or soft, the flag's dates, its note, the shoot day and scenes (links to the strips) or
  the event (a link to the Calendar on that date). It is per sub-board, not per board as
  the Conflicts tab is, because the engine's clashes are per sub-board and so are the
  Calendar and day out of days that show them. Its badge counts the picked sub-board's
  clashes and follows the sub-board picker.
- **Cost.** The board's marks and each sub-board's count go with every view, so they are
  worked out only when some element has a flag, and then one `red_flags` call per
  sub-board of the board shown, each flag's sentence made once. The tab's list, with
  names, scenes and strips, is built only while the tab is open, as the Conflicts list
  is. `perf_edit_and_view_at_1500_strips` (release) now adds six flags to the sample's
  busiest elements (ranges and open ends, 419 clashes): view 2.5 ms with them against
  1.4 to 1.9 ms without (no flags: no work at all); `red_flags` alone 0.9 ms; the tab with
  its JSON 1.4 ms.
- Seven new commands (`red_flags_view`, `set_red_flag`, `delete_red_flag`,
  `add_red_flag_type`, `rename_red_flag_type`, `move_red_flag_type`,
  `delete_red_flag_type`), each in `build.rs`, the handler list, the capability and the
  page's calls; every edit one undo step through `Session::edit`.
- Open: the marks and keys checked in the real window (screen block 1, B20 and B21);
  colors per type; a flag on the Breakdown tab and in reports; the red flag report and
  PDF; the manager table with a filter by element; searching flag notes with Find.

## 37. Merge Strips and Duplicate in the app (2026-10-11)

(The survivor and join order below were provisional; entry 46 replaces them with the real
app's: the first selected strip survives and scenes join in selection order.)

The app screens for entry 35's engine (inventory sections 3 and 5). Details in docs/APP.md.

- **Board > Merge Strips…** works on the selected scenes (two or more; banners and day
  breaks in the selection are left alone). The **topmost selected scene survives**: its
  sheet keeps its id and its place on every board. That is our provisional reading of
  the manual ("merge the selected strips"), not something checked; it goes in the
  engine-checks queue as E-S18 beside E-S3 and E-S16.
- **The order the fields combine in is the board shown's**, not the first board's: a new
  engine call, `Schedule::merge_sheets_on(board, survivor, others)`, takes the board
  whose order counts; `merge_sheets` keeps the first board's, so the command line and
  the MCP server are unchanged. With one board, or boards in the same order, the two
  agree. Engine test: on a board with scenes in the order 2, 3, 1, scene 1 merged with
  the others reads `2, 3, 1`.
- **The preview is the engine's own report**: `merge_sheets_preview` runs the merge on a
  copy of the schedule and returns `SheetMerge` with names and the Breakdown tab's
  labels (fields that change, before and after; elements gained; values left out with
  their sheet). Nothing changes until Merge Strips in the dialog, which runs the merge
  again on the schedule as it is then (`merge_sheets`), as one undo step. The surviving
  strip is selected and the status line names it.
- **Board > Duplicate Strip** duplicates every selected scene (`duplicate_sheets`), each
  with the engine's `duplicate_sheet`: the copy goes right after its original on every
  board where that is on a sub-board, at the Boneyard's end where it is in the Boneyard
  (entry 35, E-S17). The copies are selected (the view's new `strips` list), one undo
  step for them all. **Breakdown > Duplicate Sheet** (a new Breakdown menu, and a button
  beside New Sheet) is the same command for the sheet shown, and shows the copy.
- **A Breakdown menu** with New Sheet, Duplicate Sheet and Delete Sheet…: the tab's sheet
  commands were buttons only, with nothing in the menu bar. Its commands show the Breakdown tab first, as the Board menu's show the Board.
  None of the new items has a key (keys stay provisional, entry 2).
- Three new commands, each in `build.rs`, the handler list, the capability and the page's
  calls. Open: E-S18, and whether Duplicate should land after the selection on the board
  shown and at the Boneyard's end elsewhere, as note 02 has it (E-S17).

## 38. Cut, copy and paste of strips and elements (2026-10-11)

Inventory sections 3 ("copy, cut, paste elements between sheets") and 5 ("cut and paste
(below the selection), copy and paste (all strip types)"; engine check E-S2). No engine
change: the app builds paste from `add_sheet_at`, `insert_strip` and `move_strips`, and
`Session::edit` checks the result. Code in `src/clipboard.rs`; details in docs/APP.md.

- **The clipboard is the app's own, in Rust**, in the session beside the board shown:
  not saved, not undone, and gone when another schedule opens (its ids mean nothing in
  another file). It holds a **snapshot**: each copied scene's whole sheet, each banner,
  each day break, or the chosen elements of one sheet. A copied scene edited before the
  paste pastes as it was when copied, as a clipboard does.
- **Cut marks, paste moves.** A sheet strip can't leave a board (every board holds every
  sheet exactly once), so Cut never takes strips away: it marks them (dimmed, dashed) and
  the paste moves them. The same holds for elements, for one rule throughout. Nothing is
  lost when a cut is never pasted. A cut is used up by its paste; a copy pastes again.
- **What a paste of strips does.** Strips cut on the board they are pasted on **move**
  (`move_strips`): no sheet is copied. Anything else pastes **copies**: a copy, or a cut
  from another board (the user is told the originals stay there; a sheet can't move off
  a board, and taking only the banners away would split one cut into two behaviors). A
  copied scene is a new sheet with the snapshot's fields and the elements still in the
  schedule; its strip goes at the paste on this board and at the end of every other
  board's Boneyard, the rule for a new strip (inventory section 5), so pasting never
  changes another board's shoot days. Banners and day breaks are new strips; a paste
  into the Boneyard that holds one is refused whole, as a drag is (entry 29).
- **Where a paste lands**: below the bottommost selected strip, in its list; with nothing
  selected, at the end of the sub-board picked in the board bar (E-S19). The pasted
  strips are selected.
- **Elements**: Paste tags the clipboard's elements on the sheet shown, each once,
  skipping any deleted since; a cut also takes them off the sheet they were cut from
  (pasting a cut on its own sheet does nothing). One undo step.
- **The Edit menu keeps the system's Cut, Copy and Paste items**, so text fields behave as
  they always do. The page answers the DOM's `copy`, `cut` and `paste` events when focus
  is on the board (the Board tab) or anywhere on the Breakdown tab but a field, and
  leaves them to the browser otherwise (and when text is selected). WebKit enables the
  menu items for a page with no text selection only when the page cancels `beforecopy`,
  `beforecut` or `beforepaste`, so the page cancels those when the area has something to
  act on. On Windows the menu items send the keys, which raise the same events; GTK runs
  the web view's own copy command. To check in the real window (screen block 1, B23).
- **Plain text to the system clipboard** costs no capability: the copy and cut events'
  `clipboardData` takes it (a line per strip with its fields separated by tabs; element
  names, one a line). The page never reads the system clipboard: Paste uses the app's.
- **Selecting elements on the Breakdown tab**: each element's name is a toggle button
  (`aria-pressed`); click picks one, Command-click or Ctrl-click adds or drops, Shift-click
  takes the run, Escape clears. Picks reset with another sheet.
- Four commands (`copy_strips`, `paste_strips`, `copy_elements`, `paste_elements`), each
  in `build.rs`, the handler list, the capability and the page's calls. Tests: copies,
  moves, cuts across boards, Boneyard refusals, element moves; and a property test of
  random selections pasted at random places on random boards (every board holds every
  sheet once, `check` passes, a move makes no sheet, a copy one per scene, other boards
  change only at their Boneyard's end).
- Open (engine-checks queue E-S19 to E-S21): does the reference app's Cut take strips off
  at once? Where does a paste with nothing selected land? What does pasting a scene
  strip on another board do?

## 39. The strip layout: the strip designer's first version (2026-10-11)

Inventory section 5: "strip designer: field picker ... **v1** for a fixed layout with a
field picker; **later** for the full designer and multiple layouts". Engine, format and
app; no new crates, integers only. Details in docs/APP.md and FORMAT.md.

- **Model** (`method_schedule_core::model`): `StripField` (scene, I/E, set, time of day,
  pages, cast IDs, location, script day, unit, sequence, episode, shoot time, synopsis:
  the fields the board knows, plus the cast's board IDs), `StripWidth` (narrow, medium,
  wide, widest: a few fixed steps, no measurements in the engine), `StripColumn` and
  `StripLayout` (at least one column, each field once). **Each board has one**
  (`Board::strip_layout`), as a board setting: alternate boards can show different
  fields. A new board gets the default, which is what the board always showed (scene,
  I/E, set, time of day, pages, cast IDs); a duplicated board copies its board's.
  `Schedule::set_strip_layout` refuses an empty layout or a field twice, and
  `Schedule::check` does too. `reports::strip_cell` gives a sheet's text for a field
  (pages as eighths text, shoot time `h:mm`, cast IDs in board ID order), shared by the
  board and the one-line report; `OneLine::Scene` now names its sheet.
- **File format** (additive): `strip_layout` on a board, after its `name`, an array of
  `{field, width}`. The whole key is left out at the default layout and `width` at the
  field's default width, so files without layouts read and write back byte for byte. A
  reader refuses an unknown field or width (BadValue) and an empty or repeated layout
  (the consistency check). FORMAT.md documents it, and its example now carries one.
- **Diff and merge**: a layout change is a board field change, `strip layout "Scene, I/E,
  ..." -> "Scene, Synopsis (wide)"` (a width shown only when it isn't the field's
  default). The three-way merge takes the **whole layout as one value**: one side's
  change wins, both sides' different changes conflict (ours by default), so a layout is
  never a mix of two people's choices. The merge property tests set random layouts.
- **The app**: Board > Strip Layout… lists every field with Show, a width and Up and Down,
  the shown ones first in order, and Default Layout; Save is one undo step
  (`set_strip_layout`). The view carries the board's `layout` (field, label, width) and
  each scene strip its `cells`; the board draws a column per cell. Widths map to fixed
  CSS tracks (`5em`, `7.5em`, `minmax(9em, 1fr)`, `minmax(14em, 2fr)`) put in a
  constructed stylesheet by name, never schedule text, so the CSP stays as strict. **The
  board's fixed 26 px line height stays**: every cell is one line, cut with an ellipsis,
  so windowed drawing and dragging are unchanged (entries 24 and 29). The red flag mark
  leads the first column, whatever it shows.
- **The one-line report** (screen and print) takes the layout's columns, each column's
  share of the page by width (2, 3, 6 and 9 parts, percentages through the same
  stylesheet). The synopsis stays under the set (or the first wide column) unless the
  layout shows it as a column. The one-line spreadsheet export and the command line's
  one-line keep their fixed columns for now.
- **Cost**: the view carries one more string per strip and column.
  `perf_edit_and_view_at_1500_strips` (release): view 2.2 ms and JSON 0.46 ms (463 KB),
  against 1.4 ms before; with six red flags, 3.2 ms.
- A Rust test holds the page's field list and default widths (`ui/js/strips.js`) to the
  engine's. One new command in `build.rs`, the handler list, the capability and the page.
- Open (engine-checks queue E-S22): the reference app's default strip fields and widths;
  whether a layout belongs to a board or to a library the board and reports pick from;
  two-row strips; whether cast shows as IDs or names.

## 40. Board IDs sort in natural order (2026-10-11)

Screen block 1, runbook items A1 and A2 (behavior note 17; engine checks E-D14, E-L4).
Before, whole-number IDs came first by value, then every other ID as text, so `1a` sorted
after 10. The real app keeps the ID as typed (`07` and `7` both exist) and orders them
naturally: 1, 1a, 2, 7, 07, 9.

- `reports::id_order` splits an ID into its leading run of digits and the rest. It
  compares the number by value (however long; no parsing), IDs with no leading digits
  after every ID with them, then the rest of the text, then the shorter run of digits
  first and the ID text itself, so `7` comes before `07` and no two different IDs tie.
  No ID sorts last. A tie (the same ID) goes by name, as the callers already did.
- Everything that sorts by board ID uses it, so all follow: the DOOD rows, the element
  list and its report, the cast IDs on strips and the one-line, the app's element lists
  and renumber order, and the MCP tools.
- Test: 1, 1a, 2, 7, 07, 9, 10, X, then no ID.

## 41. The next automatic board ID is one past the highest (2026-10-11)

Screen block 1, runbook item A14 (behavior note 17; the roadmap's open question on the
next auto board ID). With IDs up to 9 and a gap below, the real app gave the new element
10. Before, we filled the lowest free number.

- A category that numbers automatically gives a new element one more than the highest
  whole-number board ID in the category (`07` counts as 7), or 1 when there is none. IDs
  with anything but digits in them (`12A`, `B`) are ignored. A gap below the highest is
  never filled. Moving an element into such a category without a free ID follows the
  same rule, as before it followed the old one.
- Past the largest number the engine holds (more than 19 digits), the lowest free number
  serves, so a new ID is always found.
- Renumber is unchanged: it still walks the list and fills from the start number
  (A13 is not settled).

## 42. The cast category can be renamed (2026-10-11)

Screen block 1, runbook item A10 (behavior note 17; engine check E-L5, in part). The real
app renames its cast category. Before, Cast was built in by its name, so a rename was
refused in the Elements tab, in Find and Replace and through MCP.

- **The engine knows the cast by a flag, not by its name**: `Category::cast`, at most one
  per schedule (`Schedule::check`). `Schedule::cast()` finds it and `cast_or_add()` makes
  it when missing (a category named `Cast` becomes it, else a new one goes first).
  `CAST_NAME` is only the name a new schedule gives it. Everything that used to look up
  "Cast" by name (the board's and one-line's cast IDs, the DOOD tab's default category,
  script import in the app, command line and MCP, the one-line exports) uses the flag.
- **Renaming is allowed** everywhere: `rename_category` has no built-in refusal, and Find
  and Replace lost `Refusal::BuiltIn`. The cast category still can't be moved or deleted
  (`Error::BuiltIn` stays for those): note 17 says nothing against that, and the app
  relies on it. A user category may then take the name `Cast`; it is not the cast.
- **File format** (additive): a category's `cast: true`, written only once the cast
  category has another name. With no category carrying it, the category named `Cast`
  (ignoring case) is the cast, so every existing file reads and writes back byte for
  byte. The three-way merge keeps the flag as it was made (both sides agree).
- **The app**: the cast category's name field is editable; its note says it keeps its
  place and can't be deleted. The harness fake marks the cast the same way.
- Not settled: whether the real app has a fixed set of built-in categories beyond cast.

## 43. An event on a work day is a conflict (2026-10-11)

Screen block 1, runbook items A4 and A5 (behavior note 17; engine check E-D6, and E-E5 in
part). Before, strip work won the cell (`W`) and the event was silent. In the real app a
travel day on a work day showed the travel code (with the start mark on the first day)
and a warning, raised the conflict count and counted the day once; a fitting on a pickup
day showed the fitting code with the pickup mark.

- **The DOOD** (`dood.rs`): a new `Cell::EventOnWork { code, start, finish, drop, pickup }`
  for a counted event (travel, fitting, rehearsal) on a date the element works on the
  same sub-board. Its code is the event's code followed by the work marks in the order S,
  P, D, F, with no `W`: `FTP` (a fitting on a pickup day), `TRS`, `TRF`, or `TR` on a
  plain work day. It counts once, as a worked day, so totals don't change. `Cell::is_work`
  covers both work cells (the element list's day count, the exchange file's `work`), and
  `Cell::is_flagged` marks it. Explain gives it its own reason, `event_on_work`.
- **The code on a start or finish day is not settled**: the real app's cell was cut off
  on screen. We use the same rule (event code, then S, P, D, F), so `TRS` and `FTF`; to
  check in the next block.
- **Conflicts** (`conflicts.rs`): `Conflicts::event_on_work`, each with the element,
  date, sub-board, the event's index and name, and the shoot day, sorted by date, element
  and sub-board. `Conflicts::count()` counts them with the two-sub-board conflicts. Events
  that count nowhere (unavailable, camera test, other kinds) never conflict (A8), and
  elements of a category left off the board stay out, as for the other conflicts.
- **Event clashes across sub-boards stay informational**: two or more events on a date,
  or an event in one sub-board and work in another. An event on a work day on its own
  sub-board was never a clash, and now is a conflict instead.
- **Callers**: the command line's `conflicts` counts them and lists them under "events on
  work days (conflicts)"; the MCP `conflicts` tool returns `count` and `event_on_work`, and
  its `dood` cells carry `conflict: true`; the app's Conflicts tab lists them under
  **Events on work days** (a click goes to the date on that sub-board's calendar), its
  badge counts them, and the DOOD grid draws those cells with a dashed red outline and
  says why in the cell's title. The badge used to skip boards with fewer than two dated
  sub-boards; it now works the list out for one dated sub-board with a travel day,
  fitting or rehearsal on its calendar.
- Not done: the Elements tab doesn't mark an element's row for it yet (note 17 says the
  real app does).

## 44. New Banner goes above the selection (2026-10-11)

Screen block 1 (behavior note 17, Banners; engine check E-B12, in part). In the real app
New Banner puts the banner above the selected strip. Before, our ⌘B (the key the
reference app's manual gives) was Insert Banner Below.

- ⌘B is now **Board > Insert Banner Above**, first in the Board menu; ⇧⌘B is Insert
  Banner Below. Both still work on the selection's sub-board: above the topmost selected
  strip, below the bottommost. The menu table, the page's shortcut list and APP.md
  follow, and a test holds ⌘B to Insert Banner Above.
- No engine change: the app passes the place to `insert_strip` through
  `board::insert_banner`, which has no default position of its own.
- Unchanged and unchecked: with nothing selected, Above goes to the start of the picked
  sub-board and Below to its end; and which strip counts with several selected.

## 45. A move into a category where the board ID is taken is refused (2026-10-11)

Behavior note 17, A12; engine check E-L7. The real app refuses to move an element into a
category where another element already has its board ID, and says why. Ours used to
drop the ID (and its lock) and move anyway.

- `Schedule::move_element` now returns `DuplicateBoardId` and changes nothing; the app
  shows the sentence, and the harness fake does the same. To move it, clear or change
  the ID first.
- An element with no ID moving into a category that numbers automatically still gets
  the next number (one more than the highest, decision 41); the real app's behavior
  there was not seen.
- Merging elements was already limited to one category (A11 agrees).

## 46. Merge Strips keeps the first selected strip; scenes join in selection order (2026-10-11)

Screen block 2 (behavior note 18; engine check E-S18). Follows up entry 37, whose rule (the
topmost selected scene survives, fields join in the board shown's order) was provisional.
In the real app the strip clicked first survived a merge with one added by Command-click
below it, and the merged strip read `3, 2`.

- **The engine joins in the order given**: `Schedule::merge_sheets(survivor, others)` takes
  the survivor first, then `others` as listed, for everything that combines: scenes,
  synopses and comments joined, elements unioned (the survivor's first), and for a field
  that keeps one value the survivor's, or when it is empty the first in that order. The
  dropped values and `merged` follow the same order. Sums don't depend on it.
  `merge_sheets_on` is gone: no board order is involved any more, so the command line
  (`merge-sheets FILE SURVIVOR OTHER...`) and the MCP tool join in the order they are
  given too.
- **The app tracks the order strips were selected**: the page's selection keeps insertion
  order; a click starts it, a Command-click (Ctrl-click) adds at the end or drops, and a
  Shift-click run counts from the anchor toward the row clicked. Merge Strips sends the
  selected scenes in that order; `merge_sheets_preview` and `merge_sheets` take the first
  as the survivor (the board is still checked, and still the board whose strip is
  selected afterwards). Duplicate and copy still go in board order.
- **The preview dialog** says the merge applies on every board, that the first selected
  strip keeps its sheet, fields and place, and that the others' scenes and elements go
  into it in the order they were selected. The harness fake does the same.
- Tests: the engine with the survivor given last on the board (`3, 2, 1`), and with an
  order no board has (`1, 3, 2`); the app with scene 4 selected before scenes 3 and 1.
- Not checked: Shift-click order in the real app (ours counts from the anchor), and
  whether the right-clicked strip counts as first when the context menu's Merge is used.

## 47. An event on a work day shows the whole work code after the event's (2026-10-11)

Screen block 2 (behavior note 18; engine check E-D6). Follows up entry 43, whose code on a
start or finish day was not settled. In the real app travel on a start day read `TRSW`:
the event's code followed by the whole work code, `W` included.

- `Cell::EventOnWork`'s code (`dood.rs`) is now the event's code and then the code the
  day would show as plain work: `FTW` on a plain work day, `FTPW` on a pickup day, `TRSW`
  on a start day, `FTWF` on a finish day, `TRSWF` on a one-day span. Entry 43's `FTP`,
  `TRS`, `TRF` and `TR` are gone. Nothing else changes: the day still counts once, as
  worked, and is still a conflict.
- Everything that shows the code follows (the app's DOOD grid and its key, the MCP `dood`
  tool, the exchange file); the docs and tests that showed the old form are updated.
- Seen in the real app: `TRSW` (start day). `FTPW` and `FTW` follow from it as note 18
  says; `FTWF` on a finish day by the same rule, not seen.

## 48. The status line never moves the layout (2026-10-11)

In the real window the status line used to appear and go (`hidden`), pushing everything
below it down by its height, about 28 px, and back. A click made just as "Copied 1
strip..." or "Pasted 1 strip." showed or cleared landed on the row next to the one
aimed at.

- **Its space is reserved**: the line is always there at a fixed height (29 px), blank
  and in the bar's color when quiet. A message never wraps: a long one ends in an
  ellipsis and its whole text is the line's title. Errors keep their colors and stay
  until the next message; other messages clear after six seconds, as before.
- Reserving was chosen over an overlay: an overlay would not move rows but would cover
  the top of the board (or whatever sits under it) for six seconds, the same misclick in
  another form.
- Checked in the harness: the board's row positions are the same before and after a
  copy's message, a refusal and a paste's message.
- Not changed: the `conflict` alert (the file changed on disk while autosave was on)
  still opens above the status line; it stays up until dealt with, so it doesn't come
  and go under a click.

## 49. Dates are typed into text fields (2026-10-11)

In the real window (WKWebView) the `<input type="date">` fields did not move from month
to day to year as a date was typed: each part needed Right Arrow. The start date on the
Board tab and an event's From and To on the Calendar tab used them; the red flag dialog
already had text fields, read only as `YYYY-MM-DD`.

- **Every date field is a text field** (`ui/js/dates.js`, `dateInput`). It takes
  `2027-03-01`, or month, day and year with slashes: `3/1/2027` or `03/01/2027` (month and
  day one or two digits, the year four). On leaving the field (or Enter) a date shows
  back as `2027-03-01`, the form every other screen uses, and text that isn't one is
  outlined in red.
- **Rust reads what was typed**: `calendar::typed_date` is the one parser, used by
  `set_start_date`, `save_event`, `set_day_kind` and the red flag dates. Anything else is
  refused with one sentence: `The start date "3/1/27" is not a date. Type it as 3/1/2027
  or 2027-03-01.` A two-digit year, day-first dates, other separators and dates that
  don't exist (`2/29/2027`) are refused. The page's copy only tidies the field; the
  harness fake uses the page's copy.
- **No date picker**: the system one comes only with the date control, and a picker of
  our own would be a new widget to build and check; the Calendar tab is there to pick
  dates by clicking. Nothing else changed: the board's start date is set when the field
  changes and is left, a refusal goes to the status line and leaves the typed text in
  the field to fix.
- Tests: `typed_date` with the accepted and refused forms, the start date's refusal
  sentence, a start date typed `3/5/2027`, red flags typed month first. Checked in the
  harness by typing: the start date, an event's dates (From moved To along), a refusal.

## 50. A typed duration is a day at most (2026-10-11)

Our app took `900:45` as a banner's duration. Nothing capped an `h:mm` duration: `h` could
be any number that fits.

- **The app refuses a typed `h:mm` duration over 24:00**: a banner's duration and a
  sheet's shoot time (`sheet::parse_duration`, `MAX_DURATION`), with one sentence:
  `Duration "900:45" is longer than a day. Type 24:00 at most.` (or `Shoot time ...`).
  24:00 itself is taken. A number too large to add up is refused the same way. The
  harness fake does the same.
- **Not in the engine's check**: the engine doesn't validate durations, and a merged
  sheet's shoot time is the sum of its sheets' (entry 35), which can rightly pass a day.
  The cap is on what is typed in the app.
- Not changed: screen time (`m:ss`, minutes and seconds of the cut, not a shoot
  duration); sunrise and sunset were already clock times under 24:00; the MCP
  `set_sheet_fields` takes `shoot_minutes` as a whole number with no cap, and the
  command line's time limit for day breaks has none either.

## 51. The Breakdown tab opens on the strip selected on the Board (2026-10-11)

Choosing the Breakdown tab showed the sheet it showed last, whatever was selected on the
Board; only a double-click (or Enter) opened the selected strip's sheet.

- **The tab click, View > Breakdown and the Breakdown menu's commands** (which show the
  tab first) now show the focused strip's sheet when it is a scene: always when coming
  from the Board tab, and from any other tab when the board's selection changed since the
  Breakdown tab last showed a sheet. With a banner or day break focused, or nothing
  selected, the tab keeps the sheet shown last. So stepping through sheets with Previous
  and Next, looking at another tab and coming back keeps the sheet stepped to.
- A Find result and a double-click still name their sheet themselves. The page only
  tracks whether the selection changed (`state.followBoard`); no command changed.
- Checked in the harness: a scene selected then the tab, another then View > Breakdown,
  another then the Elements tab and back, and a day break selected (the last sheet
  stays).

## 52. Strip clipboard: where ours differs from the real app on purpose (2026-10-11)

Screen block 2 (behavior note 18; engine checks E-S19 to E-S21). Most of entry 38 matched
the real app: a cut strip stays where it is, marked, until the paste moves it below the
strip pasted on; a copy pastes as a new sheet with the same scene number; Paste does
nothing until something is cut or copied. Two differences stay, on purpose:

- **A pasted copy is selected.** In the real app the selection stays on the strip pasted
  below and the new strip is not selected (a moved strip is). Ours selects whatever was
  pasted, copies too, so the next command (a move with the arrow keys, Delete, another
  paste) acts on what just arrived, and the user can see where it went.
- **The Edit menu's Cut, Copy and Paste act on strips.** In the real app they did nothing
  to a selected strip: strips are cut, copied and pasted from the strip's context menu.
  Ours answers the Edit menu and the usual keys when the board has focus (entry 38), as
  a Mac app is expected to; text fields keep the system's behavior.
- Not done: a context menu on strips (the real app's offers Edit, Cut, Copy, Paste,
  Delete, Merge, Move To, Insert Daybreak and Insert Banner) and a Merge button in the
  board's toolbar; ours has these in the menu bar. (The context menu came with entry 53.) The real app has no Duplicate command
  for strips; ours keeps Duplicate Strip (entry 37).
- Not checked: pasting on another board and where a pasted copy goes on other boards
  (E-S21), and where a banner goes with nothing selected.


## 53. Density: at least as much on screen, as few clicks, as the reference app (2026-10-11)

The owner's rule: the app must be at least as dense and efficient as the reference
scheduling app, as much on screen per pixel and as few clicks per task. Our observations of
the reference app (screen blocks 1 and 2, behavior notes 17 and 18, in our words): the
breakdown sheet's fields sit in three tight rows with the label beside each field; every
element category is a small box in a four-column grid, empty ones too, so 20 or more are
in view without scrolling; strips are compact and their commands are on a right-click
menu; day out of days cells are narrow; toolbars and dialogs use little padding.

The rule from here on: a new view is measured at a 1280 x 800 window against what the
reference app shows, and does not lose to it on what is visible without scrolling or on
clicks per task. Text stays at 11 px or more and hit targets stay usable.

What changed, each checked in the harness at 1280 x 800:

- **Breakdown tab**: the header fields in three rows with labels beside them (Scenes,
  I/E, Set, D/N, Shoot time; Synopsis, Location, Pages, Script pages, Script day; Screen
  time, Unit, Sequence, Episode, Comments, Sunrise, Sunset), 12 px controls; every
  category a box in a grid (five columns at 1280 px), empty ones too, each with its count,
  its elements (click to select for cut and copy, × to remove) and an Add field matching
  as Quick add does within the category. Quick add stays. **Hide empty** is remembered in
  localStorage, wrapped in try/catch: a per-browser convenience, not schedule data. With 23
  categories: header fields in view 18 of 18 before and after (the form went from 5 rows
  and 325 px to 3 rows and 76 px); categories whole in view 1 before (only the 5 with
  elements were drawn at all), 23 of 23 after, with no scrolling.
- **The strips' context menu**: right-click on a strip (sub-boards and the Boneyard), or
  Shift+F10 or the context menu key on the selection: Edit Sheet, Cut, Copy, Paste,
  Delete, Merge Strips, Duplicate Strip, Move To, Insert Day Break Above and Below, Insert
  Banner Above and Below. Built in the page (ui/js/ctxmenu.js), not a native menu: the
  window's capability grants no menu permission and none is added. A right-click on an
  unselected strip selects it first; on a selected one the selection stays. Every item
  runs what the menu bar runs (the `COMMANDS` table in main.js, the Edit menu's clipboard
  path for Cut, Copy and Paste); Insert Day Break Above is new there
  (`board.insert_break_above`, no key and no menu bar item), and Move To calls the
  `move_strips` command drag and drop uses. Clicks per task on one strip: Cut, Copy,
  Paste, Delete or Duplicate went from three (select it, open the menu bar menu, pick the
  item) to two (a right-click, which selects it, and the item); Move To had no command
  before, only a drag.
- **Padding and type everywhere**: the page's text is 13 px (it was 14), nothing under
  11 px; the top bar, tab bar, board bar, footer, dialogs, the Elements, Calendar,
  Conflicts and Red flags tabs and the Reports side panel lost most of their padding;
  buttons are 22 px high. The status line keeps its fixed 29 px (entry 48). The strip
  line went from 26 px to 22 px: `LINE_PX` in board.js and `.strips > .line` in
  styles.css, which only the page uses (no Rust test reads it); dragging, the windowed
  drawing past 120 lines and scrolling a selection into view were checked again in the
  harness. At 1280 x 800: the chrome above and below the tabs' panel 183 px before, 131
  after (the panel 618 px, now 670); strip lines in view 21 before, 28 after; Elements
  tab categories in view 18 of 23 before, 23 of 23 after; the Calendar's month 537 px
  before, 421 after (all six weeks in view, 35 of 42 days before); the Reports side panel
  403 px before, 291 after.
- **Day out of days**: date columns 32 px (they were 2.8em, 33.6 px), the ID column 36 px
  (48), the name 120 px (156), work, hold and total 32 px (42), start and finish 72 px
  (79), rows 20 px (24, `ROW_PX` in dood.js), 11 px type. A code too long for its cell is
  cut with an ellipsis, as the reference app cuts `TRSW`; every cell with a code has its
  date and whole code as its title (before, only cells with a red flag or a warning had a
  title), and start and finish dates theirs. At 1280 x 800 with 300 rows and 140 days:
  date columns clear of the sticky columns 23 before, 27 after; rows in view 20 before,
  26 after; cells in view 460 before, 702 after.
- Not done: the print layout (print.css) is unchanged; the find panel kept its spacing.

## 54. Delete on a scene strip deletes its sheet (2026-10-11)

Inventory sections 5 and 14: deleting a strip deletes its sheet everywhere, with
confirmation, and Delete or Backspace deletes the selected strips. Before, the board
deleted only day breaks and banners and sent scenes to Delete Sheet on the Breakdown tab
(entry 29).

- `delete_strips` now takes scenes too: a scene's sheet is deleted
  (`Schedule::delete_sheet`), so its strip leaves every board; day breaks and banners
  leave the board shown. A mixed selection is one undo step; an unknown strip refuses the
  whole edit. The sheet's elements stay in the schedule.
- The page asks before deleting scenes or banners (one sentence naming what goes, and
  that the sheets leave every board); day breaks alone go without asking, as before
  (E-B13). The strip context menu's Delete is enabled for any selection.
- Not checked in the real app (engine check E-S23): the wording of its question, whether
  a mixed selection asks once, and whether Backspace and Delete both act.

## 55. Insert Day Break Above and Below in the Board menu (2026-10-11)

Inventory section 6 (insert above or below the selection, v1) and section 14 (an older
manual gives Cmd comma for inserting a day break). The context menu had both (entry 53);
the menu bar had neither, and only the bare B key inserted one.

- Board > **Insert Day Break Below** (⌘,, Ctrl+, elsewhere) and **Insert Day Break
  Above** (⇧⌘,). Below goes under the bottommost selected strip and Above over the
  topmost, as the banner commands do (entry 44); with nothing selected, the end or the
  start of the picked sub-board. A selection in the Boneyard is refused with a sentence.
  The B key now follows the same rule (it used the focused strip).
- The page matches the comma by its physical key too (`e.code` `Comma`), since Shift
  turns it into another character on most layouts; macOS's menu answers the keys there.
- Provisional (engine check E-K2): the reference app's keys for these, and whether its
  Insert Daybreak goes below or above the selected strip.

## 56. Automatic day breaks by shoot time and screen time in the app; Remove All (2026-10-11)

Inventory section 6: automatic day breaks by most eighths, most shoot time or most screen
time, or remove all. The engine and the command line had all three limits (entry 12);
the app broke by pages only and had no remove-all.

- **The sub-board's header**: Break by (Pages, Shoot time, Screen time), the limit, Break
  and Remove Breaks. The limit is read in Rust as the sheet's fields are: pages as `5` or
  `4 4/8`, shoot time as `h:mm` (24:00 at most, entry 50), screen time as `m:ss`; zero
  is refused. The page keeps the last limit typed for each choice. The rule is the
  engine's: a day closes as soon as it reaches or passes the limit, banners count their
  duration toward shoot time (E-B3, behavior note 17, A15) and nothing toward screen
  time, and every old break goes first (E-B4).
- **Remove All Day Breaks** (`days::remove_day_breaks`, new): every day break on the
  sub-board goes, its strips stay in order and become unscheduled; one undo step. With
  none, it is refused with a sentence rather than taking an empty undo step. In the
  sub-board's header and in the Board menu (the selection's sub-board, else the picked
  one). The command line has `breaks FILE --remove` and the MCP server
  `remove_day_breaks`.
- `auto_day_breaks` now takes `by` and `limit`; the status line says how many shoot days
  it made.
- Not checked in the real app (engine check E-B14): automatic breaks by screen time, and
  whether remove-all asks first.

## 57. Day breaks may be parked in the Boneyard (2026-10-11)

Inventory section 6: day breaks can move, be deleted, and be parked in the Boneyard,
where they have no number. Entry 29 refused it (the Boneyard held scenes only); this
replaces that refusal for day breaks. Banners stay out of the Boneyard.

- **Engine**: `Schedule::move_strips` and `insert_strip` take a day break into the
  Boneyard and refuse a banner; `Schedule::check` refuses a banner there, not a day
  break; the three-way merge keeps a parked day break in the Boneyard (its fallback
  drops only banners there). A parked break is in no sub-board, so `days::days` never
  sees it: it closes no day, has no number or date, and the day out of days, reports,
  conflicts, red flags, the budget exchange and the calendar all ignore it. Sorting the
  Boneyard removes it, as sorting removes day breaks anywhere (entry 32).
- **File format**: no new key. A `boneyard` may now hold a day break strip; a banner there
  is still refused. Files without parked breaks read and write back byte for byte; a file
  with one is refused by builds before this entry.
- **The app**: dragging, Option-Up and Down, Move To and pasting (a cut moved, or copies)
  take day breaks into the Boneyard; banners are refused with "Banners go on a sub-board,
  not in the Boneyard." A parked break is drawn faint and dashed as "Day break, parked",
  can be selected, moved back and deleted. Insert Day Break still needs a sub-board. The
  footer counts the Boneyard's scenes, not its strips. The command line and the MCP
  server's `move_strips` follow the engine.
- Open (engine checks E-S5 and E-C6): how the reference app draws a parked break, and
  whether its sort or automatic breaks touch one in the Boneyard.

## 58. The Breakdown tab's planned date links to the strip (2026-10-11)

Inventory section 3 (a sheet shows its planned shoot day and date, and jumps to its
strip). The header line already said where the strip is (`Main, day 3, Wed 2027-03-03`,
entry 51); it is now a link. A click picks that sub-board in the board bar, shows the
Board tab and selects the strip, scrolled into view (the same path the Conflicts and Red
flags tabs use). The sheet view carries the strip's id and its sub-board (`strip`,
`strip_sub_board`, null in the Boneyard); no new command. Not checked: what the reference
app's jump looks like (engine check E-S24).

## 59. Report options, more reports and exports, the day out of days sort and letters (2026-10-11)

Inventory sections 7, 10, 11 and 12; behavior notes 01, 03 and 05. Engine, file, command
line and MCP first; the app follows (entries after this one say what it shows).

- **What the schedule keeps** (`method_schedule_core::setup`, FORMAT.md): the production
  info a report header prints beside the title (company, director, producer); the
  **report list**, each entry a name, a kind and its options; the day out of days **code
  letters**; and each sub-board calendar's **prep start and wrap end**. Each is left out
  of the file at its default, so older files read and write back byte for byte. The list
  is empty while it is the default (every kind once, in our order); a kind it lacks shows
  at its end, so adding a kind later never breaks a saved list. A copy is named
  `<name> copy` (then `copy 2`); names are unique ignoring case; the last report of a kind
  can't be deleted.
- **Options** (one set for every kind; each kind uses what applies): a date range, the
  orientation, synopsis on or off, cast by name or board ID, a page per day, banners,
  a category filter and an element filter, strip colors, red flag marks, a header layout
  (full, compact, title only), a text size (small, normal, large), the scene columns (the
  simple designer), the day out of days without holds, and the calendar on weekdays only.
  The reference app keeps one date range per sub-board for every report; ours keeps it per
  report, so two copies can cover two weeks. Plain reading, engine check E-R7.
- **Merge and diff**: the production fields merge one by one; the report list and the
  code letters merge as one value each, as a strip layout does (one side's change wins,
  two different changes conflict, ours by default). A filter naming a category or element
  theirs added follows its new id; one the other side deleted drops out
  (`tidy_report_filters`, also run when a category or element is deleted or merged). Diff
  says each production field, report added, removed, renamed or with options changed, the
  order, and each code letter (`Subject::Setup`, grouped under Schedule).
- **New report data** (`method_schedule_core::report_data`): the breakdown by strip
  (every sheet on the sub-board in board order with its day, its elements by category)
  and by day (each category's elements merged over the day, with how many of the day's
  scenes list each, and, once sheets carried quantities (entry 63), the most any one of
  the day's sheets needs, shown `x3`); the element detail
  (each scene an element is in with day and date, and its work dates); the vertical
  one-line (a fixed row per cast member, a column per scene); the multi-unit one-line
  (every dated sub-board of the board merged by date, units in sub-board order, an
  element filter dropping days without a matching scene); and the calendar's dates (what
  each is, its shoot day, scenes, pages, estimated length and events).
- **Day out of days**: `dood_with` and `Options { no_holds }` work the grid out as if no
  element had holds (CineSched's view, inventory "competitor features"; off unless asked).
  `sort_rows` sorts by up to four keys (board ID, name, start, finish, work, hold), each
  ascending or descending, stable, blanks last whichever the direction (behavior note 01);
  the app keeps the sort for the session only, as the reference app does. `Cell::code_with`
  shows the schedule's code letters; with the defaults it equals `code()`. Letters are 1 to
  3 letters, digits or `/`, no two the same (ignoring case), so every code reads one way.
- **Spreadsheets** (`method_schedule_format::export`): the day out of days grid (a column
  per date, the summary at the end, every category or one), the element detail (a row
  per element and scene), the breakdown (a column per category) and the whole board (each
  sub-board's strips with day breaks, then the Boneyard). Same rules as before: integers,
  eighths beside the page text, real dates, formulas neutralized.
- **Budget exchange**: `shoot_days` carries `prep_start` and `wrap_end` when the schedule
  records them; optional and additive, as EXCHANGE.md requires for the shared file.
- **Command line**: `dood --grid` (with `--sort`, `--no-holds`, `--csv`, `--xlsx`),
  `breakdown [--by-day]`, `detail`, `board`, `calendar`, `vertical`, `multiunit`,
  `reports`, `production`, `codes`, `prep`. **MCP**: `report` (the new reports as data),
  `report_list`, `set_production`, `set_dood_codes`, `set_prep_wrap`.
- **The app**: the Reports tab lists the schedule's reports with Up, Down, Copy and
  Delete, and the picked report's options below (each change one undo step), the pickers
  for categories, elements and columns (the simple designer: which scene fields, in what
  order; no merged cells or fonts) and Production Info. Every kind prints and saves as a
  PDF; the conflict and red flag reports are the tabs' tables on paper (the red flag
  report filters by element). File > Export adds the day out of days grid, the element
  detail, the breakdown and the whole board. The Day out of days tab sorts by a heading or
  by Sort… (session only, cleared on a sub-board switch, as the reference app clears it),
  edits each row's hold and drop switches, shows the grid without holds or on its side
  (Vertical), and changes the code letters. The Calendar tab lists each shoot day's strips
  (with the fields the user picks), its pages and length; a strip dragged onto another
  shoot day moves to the end of that day through `move_strips`; Show filters the strips by
  a category and an element for the session; Today and Go To; Print and Save as PDF print
  the Calendar report; Prep starts and Wrap ends.
- **The user's preferences, not the schedule's**: the week start (Sunday or Monday), the
  date format (2027-03-01, 3/1/2027 or Mon 1 Mar 2027) and the calendar strip fields live
  in the app's `settings.json` (inventory section 7: app-wide preferences; files keep ISO
  dates). Date fields show ISO or US back; Rust reads both, as before (entry 49).
- Ten new commands (`report_list`, `save_report`, `duplicate_report`, `move_report`,
  `delete_report`, `set_production`, `set_dood_codes`, `set_prep_wrap`, `preferences`,
  `set_preferences`) in build.rs, the handler list, the capability and the page; the
  harness fake answers them. `report_data` now names a report by its place in the list.
- Open (engine checks): whether the reference app's date range is per sub-board only
  (E-R7); what the breakdown by day counts in brackets when there are no quantities
  (E-R8); the vertical one-line's cover page and row order (E-R9); whether custom code
  letters reach the summary export (E-D24); how the reference calendar takes a dropped
  strip (the end of the day, or where it is dropped among the day's strips) and what its
  filter hides (E-C11).

## 60. Strip colors set by the user (2026-10-11)

Inventory section 5: strip colors by I/E and time of day, and a font color per strip
type. The board used fixed convention colors (int day white, ext day yellow, int night
blue, ext night green).

- **Model** (`model::StripColors` on the schedule, not a board, as the reference app keeps
  I/E and time of day values with their colors in the schedule and its templates):
  `rules` in order, each an I/E and a time of day value (compared trimmed, ignoring case;
  empty matches any) and an `Rgb` color (an integer, `0xRRGGBB`); the first rule that
  matches a sheet colors its strip, and no match keeps the convention. Text colors per
  strip type: scene, banner, day break, each optional; with none the scene text is black
  or white by an integer luma (299, 587, 114 thousandths, black from 128). Refused: more
  than 200 rules, two rules for the same values, a color out of range.
- **File** (additive): `strip_colors` after `red_flag_types`, left out when nothing is
  set, colors as `#rrggbb`. Diff: "Strip colors ... -> ..." under Schedule; merge: the
  whole set is one value, as a strip layout is (entry 39).
- **The app**: Board > Strip Colors… lists every I/E and time of day pair the board's
  scenes use with a color picker (Usual puts one back), Add for other values, and the
  three text colors (Automatic or a color). Save is one undo step (`set_strip_colors`).
  The view sends a colored strip's `color` and `ink`; the page paints each pair once
  through a constructed stylesheet after checking it is `#rrggbb`, so the CSP stays as
  strict and lines drawn while scrolling find their class.
- Our reading where the reference app is unknown (engine check E-S25): colors keyed by
  the pair of values rather than separately by I/E and by time of day, and kept per
  schedule rather than per board. Reports and printing keep their own plain colors for
  now; the printed board (BRD-20) uses these.

## 61. Printing the board (2026-10-11)

Inventory section 5: print or PDF the board, all or selected strips (v1, PDF from the
app). The reports already print through `report.js` (paging onto Letter pages with a
header and footer) and `print_report` (the print sheet, or straight into a PDF on macOS,
entry 23).

- Board > **Print Board…** and **Save Board as PDF…**, and File > Print and Save as PDF
  while the Board tab is shown, ask three things (selected strips only, include the
  Boneyard, portrait or landscape; remembered until the app quits), then lay the board
  shown out as flows for `report.js` and print them with `printPages`: no new command,
  no new capability.
- A sub-board is a flow: its days (a heading kept with its strips, the strips, the dark
  day break row with scenes, pages and time), then the strips after the last day break;
  the Boneyard is a flow of its own. Columns are the board's strip layout, sized as the
  one-line report sizes them. Strips print in the usual colors (print.css) and in the
  user's strip colors and text colors (entry 60), painted through a constructed
  stylesheet scoped to the print root. With "selected strips only", a day prints when
  one of its strips or its day break is selected.
- Not checked in the real app (engine check E-S26): its printed board's look (one line a
  strip, colors, the day break's text) and options.

## 62. Categories and elements: our default list, left off the board, properties, notes, links, images, header fields (2026-10-11)

Inventory section 4; behavior notes 04 and 17; engine checks E-L2, E-L5 and E-E3.

- **Our own default category list** (`model::DEFAULT_CATEGORIES`, used by
  `Schedule::with_default_categories`): a new schedule in the app, from `method-schedule
  new` or `import --into` a new file, or from MCP `new_schedule` gets the cast and then
  Background, Stand-ins, Stunts, Hand props, Set dressing, Wardrobe, Hair and makeup,
  Picture vehicles, Animals, Practical effects, Visual effects, Camera, Grip and electric,
  Sound and music, Special equipment, Extra crew, Safety, Other. Our own names and order
  (people, then what is in front of the camera, then what makes the picture, then the
  rest), not copied from any product; the open category id proposal's groups informed
  what to cover (interop-formats.md, section 4). Stunts number new elements
  automatically; Background, Stand-ins, Animals and Extra crew use daily quantities. Only
  the cast is built in; the rest are ordinary user categories (rename, move, delete).
  `Schedule::new` still makes the cast alone, for tests and files. Not checked: whether
  the real app has a fixed built-in set (E-L5).
- **Left off the board** (`exclude_from_board`, a switch in the Elements tab): the cast
  column on strips and in the one-line is empty for such a category
  (`reports::cast_ids`), its elements never conflict (as before, decision 28), and the
  day out of days still shows their rows and counts but marks no cell as a conflict (the
  app's grid and MCP's `conflict`); the elements table's Conflicts column says 0. Our
  reading of "honored everywhere": an excluded category still has a day out of days.
  Engine check E-L10 asks whether the real app leaves it out of the day out of days.
- **Custom properties**: a category's list of field names (`Category::properties`,
  unique ignoring case), each element's values (`Element::properties`, kept in the
  category's order, an empty value left out). Renaming or moving a property carries the
  values; deleting it drops them; moving an element to another category loses them (the
  manual over the older video). Shown and edited in the Elements tab, listed by MCP
  `list_elements` and set by `set_element_details` and `set_category`, shown under each
  element in the Elements report and by `method-schedule elements`.
- **Notes**: the Elements tab edits the notes the file already kept (several lines,
  trimmed at the end, 10,000 characters at most).
- **Linked elements** (behavior note 04, E-L2): `Element::links` lists the elements it
  brings along. `Schedule::tag_with_links` puts an element on a sheet with its own linked
  ones, one level only (a linked element's links don't follow; every level since decision
  116), and every add uses it:
  the Breakdown tab's quick add and boxes, Paste, MCP `tag_element` and `add_element`.
  The links editor (Linked Elements…) sets the whole list; Also add them to every sheet
  that already has it is the retroactive box (every linked one goes on those sheets).
  Removing a link takes nothing off a sheet. Any element may link to any other, in any
  category; the real app's limits (before 10.8.33 only set and location elements could
  link across categories, we read) and links by episode are not modeled. An element
  can't link to itself or to one element twice (`Schedule::check`).
- **Images**: `Element::images`, paths only; the files stay beside the schedule
  (FORMAT.md, Images). Add Image… is a Rust Open dialog (PNG, JPEG, GIF, WebP); a path
  inside the schedule's folder is stored relative to it with `/`, any other absolute.
  The thumbnail comes from `element_image`: Rust resolves the stored path (a relative one
  against the schedule's folder; `..` refused), checks the type and size (8 MB at most)
  and returns a `data:` URL; a missing file shows as Missing. The CSP allows `data:`
  images for this, nothing else (APP.md, Security model). Several images per element; a
  note per image and one marked for reports are later.
- **Header fields as categories** (`reports::HeaderField`, `reports::header_category`):
  set, location, unit and episode, worked out from a copy of the schedule where each
  value a field takes (ignoring case, spelled as first met, empty left out) is an element
  of one more category on the sheets that have it. The Elements tab lists them under
  Header fields with sheets, pages, work days, start and finish, read only; the Day out
  of days tab and MCP `dood` (`header`) show their grid (no holds, no events). The
  reference app shows these with page and day counts since 10.13a (behavior note 04).
- **The elements table** gains Events (the sub-board's events that list it) and
  Conflicts (on its board involving this sub-board: two sub-boards on one date, and
  events on its work days), and sorts by any column (again to reverse, a third time back
  to board ID order; ties keep that order, blanks last; this session only). A custom drag
  order is later.
- **Days and dates** (inventory section 4): picking an element lists every date of its
  day out of days row on the sub-board with its shoot day, code and meaning
  (`element_days`), under the element's editor.
- **New Event…** on an element in the Elements tab opens the event editor (the Calendar
  tab's, entry 21) over the tab with that element picked; Saving adds it to the picked
  sub-board's calendar as one undo step.

## 63. Quantities per sheet, daily totals, two more event kinds, New Event from a sheet (2026-10-11)

Inventory sections 3 and 8; behavior notes 04 and 17 (A8); engine checks E-E3 and E-D9.

- **Quantities** (`Sheet::quantities`): how many of an element a sheet needs (1 to
  99,999 in the app) and whether that counts in the day's total ("use in daily
  quantity" on the sheet, on by default). One, counted, is the default and leaves no
  entry, so files without quantities read and write back byte for byte. A quantity goes
  with its element: untagging, deleting, cutting it from a sheet drops it; merging
  elements keeps the first; merging sheets takes each element's from the first sheet in
  order that gives one; the three-way merge drops one whose element left the sheet.
  The Breakdown tab's `×1` button on each element sets it.
- **Daily category totals** (`reports::daily_category_totals`): a category with "use in
  daily quantity" on (`Category::daily_quantity`, off by default, the cast too) adds up
  each sheet's quantity of its elements on the day, one for an element with none; a
  quantity marked not to count daily is left out and the day's total is partial
  (`CategoryTotals::partial`), shown with an asterisk in the report and by
  `method-schedule totals`. Any other category counts each element once a day, as
  before. Our plain reading; engine check E-L11 asks how the real app adds them.
- **Unavailable and camera test** are kinds of their own (`EventKind::Unavailable`,
  `EventKind::CameraTest`; `unavailable` and `camera_test` in the file and over MCP).
  Like other events they neither show on nor count in the day out of days, and never
  conflict (note 17, A8). The Calendar's event editor offers them.
- **New Event… on a sheet** (the Breakdown tab's header line) opens the event editor with
  every element on the sheet picked, from the date the sheet shoots on (or the
  sub-board's start), for the sub-board the Elements, Day out of days and Calendar tabs
  show. Dragging an event on the calendar is later.
## 64. Show or hide day headings, day breaks, totals and banners (2026-10-11)

Inventory section 5: show or hide total strips, banners and shoot day strips (later).

- Board > **Show on Board…**: four switches, all on by default: day headings (our shoot
  day line above each day), day breaks, the totals on day breaks, banners. A hidden line
  is left out of the board's lines and of the selectable rows, so the keys, the selection
  and windowed drawing work on what is shown; a drop still lands by strip index, between
  the strips shown around it. Printing the board (entry 61) follows the same switches.
- Kept per viewer in the browser's localStorage, wrapped in try/catch, as Hide empty on
  the Breakdown tab is (entry 53): it changes what one person looks at, not the schedule,
  so it is not an edit and takes no undo step. Our reading (engine check E-S27): whether
  the reference app keeps these per board in the file.

## 65. Banner styling: bold, a palette color, one or two lines (2026-10-11)

Inventory section 5: banners have font, size, alignment, color and thin or standard
height (later). Entry 30 left styling for later.

- **Model**: `Banner.style` (`BannerStyle`): `bold`, `color` (`Option<BannerColor>`, a
  fixed palette of nine named colors: yellow, orange, red, pink, purple, blue, teal,
  green, gray; none is the board's banner color) and `tall` (two lines high). A palette
  rather than any color, so banners stay readable in light and dark (each name has a
  light and a dark shade in styles.css, and a print shade in print.css).
- **File** (additive): `bold`, `color` (the name) and `tall` on a banner strip, each left
  out at its default; an unknown color name is refused. Diff: a banner's `style`
  (`bold, teal, two lines` or `plain`) is a strip field; merge takes the style as one
  value.
- **The app**: the Banner dialog has Color, Height and Bold; `insert_banner` and
  `set_banner` take `style` (null: plain on insert, kept on edit). A two-line banner is
  two of the board's fixed 22 px lines (the banner and a second line of its color that a
  click or a drop treats as the banner's lower half), so windowed drawing and dragging
  keep their arithmetic; its text wraps onto the second line. The printed board and its
  colors follow.
- Not done: font family and size, and alignment (the reference app has them). Not checked
  (engine check E-B15): the reference app's palette and what thin means there.

## 66. Resource counts on strips (2026-10-11)

Inventory section 5: resource counts on strips (later, a 10.12 feature). Two more strip
layout fields (`StripField::CastCount`, `cast_count`, "Cast count"; `ElementCount`,
`element_count`, "Elements"), narrow and right-aligned, so any board (and the one-line
report, which follows the board's layout) can show them: how many cast members the
sheet has (the cast IDs the strip would list, so none when the cast category is left off
the board) and how many elements it has from every category on the board. A sheet with
none shows nothing rather than 0, so a strip stays quiet. No format change beyond the two
new field names; a file naming them is refused by builds before this entry. Our reading
(engine check E-S28): which counts the reference app offers (per category, quantities)
and how it labels them.

## 67. What day breaks show, and a shoot day strip above each (2026-10-11)

Inventory section 6: a day break may also show sunrise (earliest of the day), sunset
(latest), screen time and custom text (later), and a shoot day strip goes above each
break, showing the count and the full date (later, a 10.13 feature).

- **Model** (`model::DayLook`, a board setting like its strip layout): `shoot_day_strip`,
  `screen_time`, `sunrise`, `sunset` (switches) and `text` (one line, at most 200
  characters, the same on every day break of the board: the reference app's custom text
  is a fixed field of the day break's design, not a note per break). Default: totals
  only. `Schedule::set_day_look`; `check` refuses a long or two-line text.
- **File** (additive): `day_look` on a board after `strip_layout`, left out at the
  default, each field left out at its default. Diff: a board field (`day breaks`); merge:
  one value per board.
- **The app**: Board > Day Breaks… sets it, one undo step (`set_day_look`). Each day in
  the view now carries its screen time, the earliest sunrise and latest sunset of its
  sheets, and its full date; the day break line shows the extras between its name and
  its totals, and the shoot day strip (`Shoot day 3 of 12 · Wednesday, March 3, 2027`,
  `No date` without a start date) is a line of its own above the day break, not
  selectable, a drop on it landing above the break. The printed board follows.
- Not checked (engine checks E-B16 and E-B10): where the reference app's shoot day strip
  sits and what it says, whether the count is the sub-board's days, and quantity on a
  day break (not done).

## 68. Production tags from a tagged `.fdx`, on request (IMP-04) (2026-10-11)

Inventory section 1 and the interop note, section 2: the reference app takes tags only
through its own tagged export; reading a screenwriting app's production tags is an
opt-in extra of ours. The planning data for every feature beyond the reference app lives
in one new schedule key, `plan` (FORMAT.md, "Plan"; `method_schedule_core::plan`), left
out while empty, so older files read and write back byte for byte; ids in it that no
longer exist are ignored, never an error, and the plan's own edits drop them. Diff lists
plan changes as Schedule changes; merge takes the plan as one value.

- **Engine**: `fdx::parse_with_tags` reads the shape we expect (no public sample shows
  one, so this is our plain reading, engine check E-I5): a text run with a `TagNumber`
  attribute, and after the content a `TagData` block of `TagCategory` (`Id`, `Name`),
  `TagDefinition` (`Id`, `CatId`, `Label`) and `Tag` (`Number`, `DefId` children). The
  element is the definition's label, else the tagged words. A heading's tags go to the
  scene it starts. Anything that can't be followed to a category is skipped.
  `revise::import_tagged` adds each tag as an element of the category with that name
  (created when missing; Cast or Cast Members is the built-in cast category), matched by
  name, and tags the sheet. `fdx::parse` is unchanged: tags stay off unless asked for.
- **Command line**: `import SCRIPT.fdx --into FILE --tags`. **MCP**: `import_script`
  takes `tags`.
- **App**: Plan > Import Script with Tags... (an `.fdx` file picked in Rust; one undo
  step; the notice counts scenes and tags).

## 69. Script revisions merged into a schedule (BYD-01) (2026-10-11)

Competitor feature (note 13): a new draft goes into an existing breakdown without
starting over. The reference app adds duplicates instead (E-I3); ours differs on purpose.

- **Matching** (`revise::revise`): each draft scene to a sheet, first by scene number
  (ignoring case and spacing), then, for scenes left over, by heading (I/E, set and time
  of day, ignoring case) among sheets left over whose numbers the draft doesn't use. A
  sheet holding several scenes after Merge Strips (`12, 15`) matches each number and
  keeps its fields.
- **A matched sheet** keeps its id, tags and place on every board; the fields the script
  owns (number, I/E, set, time of day, length) take the draft's values; new speaking
  characters (and tags, with tags on) are added. Nothing is untagged: a character who no
  longer speaks is reported as kept.
- **New scenes** become sheets at the end of every Boneyard. **Missing scenes** are marked
  omitted in the plan, not deleted; a later draft that has them again clears the mark.
  Sheets with neither a scene number nor a set (made by hand) are never marked.
- **Report**: counts, then each changed sheet field by field, new sheets and omitted ones.
- **Command line**: `revise FILE SCRIPT [--tags] [--dry-run]`. **MCP**: `revise_script`.

- Open: how omitted sheets should look on the board and in reports (they draw as they
  did; the revision report lists them).
- **App**: Plan > Import Script Revision...: a box for reading tags, the Open dialog in
  Rust, the report (worked out on a copy), and Merge Revision as one undo step.

## 70. Call sheet data export (BYD-02) (2026-10-11)

Pain point 3 (note 13): no call sheets in the reference app. Ours is data only, as JSON
and CSV, described in docs/CALLSHEET.md: the day's scenes with clock times, cast with
board IDs and their first and last times, other elements by category, sunrise and
sunset, and notes (banners with times, events on the date, timeline warnings).
`callsheet::call_sheet` gathers it; `method_schedule_format::callsheet` writes it.
Command line `callsheet FILE DAY [--csv] [--out OUT]`, MCP `call_sheet`. Distribution stays out (BYD-03).

- **App**: Plan > Export Call Sheet Data...: the day (the selection's, by default) and
  JSON or CSV, then the Save dialog in Rust.

## 71. The running timeline from a call time (BYD-04) (2026-10-11)

Competitor feature: a running clock per day with day length and sunset warnings.

- **Call times** are kept per shoot day, by the day break that closes the day (the
  stable thing a day has); moving strips keeps a day's call time, deleting its break
  drops it. 24-hour `h:mm`, before midnight.
- **Timeline** (`timeline::timelines`): from the call, each strip in board order takes its
  shoot time (banners their duration); a scene without one takes none and is counted.
  Without a call time the times count from the call (`+1:30`).
- **Warnings**: a day longer than the long day limit (12 hours by default, set in the
  plan), and an exterior day scene (I/E with EXT, time of day with DAY, MORNING,
  AFTERNOON or NOON, not NIGHT) ending after sunset or starting before sunrise, or on a
  day the sun doesn't rise. Warnings only. Pushing a long day's tail to the next day is
  later.
- **Command line** `timeline`, `call`, `long-day`; **MCP** `timeline`, `set_call_time`.
- **App**: Plan > Day Timeline...: the selection's sub-board and day, a day picker, the
  Call and Long day after fields beside their labels, and the strips with start, end and
  notes; each change one undo step.

## 72. Sunrise and sunset by location, in integers (BYD-08) (2026-10-11)

Competitor feature (Fuzzlecheck). A location name (as the sheets write it) gets a
latitude and longitude, typed as decimal degrees (`34.0522`, `-118.2437`, or with N, S,
E, W), kept as millionths of a degree, and a typed UTC offset (`-7`, `+05:30`; default
the longitude's hour). No network and no time zone database: the offset is the one in
effect on the shoot dates, and no daylight saving switch is applied (a later option).

- **Math** (`solar.rs`): the standard sunrise equation (mean anomaly, equation of center,
  ecliptic longitude, solar transit, declination, the hour angle for the sun's upper edge
  0.833 degrees below a flat horizon) in fixed point: angles in billionths of a degree,
  days in billionths, sines scaled by 10^12 with 128-bit products, a Taylor series for
  sine, bisection for arccosine and Newton for the square root. No floats, per entry 3.
  Tested against published times for London, New York, Sydney, Los Angeles and Reykjavik
  (within two minutes) and Tromso's midnight sun and polar night.
- **Per shoot day**: the sun at the first of the day's scenes whose location is on the
  map, on the day's date. Shown in the timeline, the call sheet and `sun FILE`. The
  sheets' own sunrise and sunset fields are left as typed.
- **Command line** `location`, `locations`, `sun`; **MCP** `set_location`, `locations`.
- **App**: Plan > Locations...: every sheet location with Latitude, Longitude and UTC
  offset fields in one table; Set (or Enter) is one undo step; both degrees cleared takes
  it off the map.

## 73. Rest periods and cast conflict checks (BYD-09) (2026-10-11)

Competitor feature (Fuzzlecheck). Cast conflicts are already covered: one element on one
date in two sub-boards (entry 28) and red flags (entry 33). New: a turnaround minimum
per category (for example 12 hours for Cast), the shortest rest from one day's wrap to
the next working day's call. Each element of such a category is checked across every
sub-board of a board, using the timelines: the company wrap and the next day's company
call, since the schedule keeps no per-person call (that can only make a rest look
shorter, so it errs toward a warning). Days with no date or no call time are counted as
unchecked. Warnings only, listed like conflicts (`rest::rest_report`). Command line
`turnaround`, `rest`; MCP `set_turnaround`, `rest_report`.

- **App**: Plan > Turnarounds and Rest...: a minimum per category beside the list of short
  rests on the board shown.

## 74. Schedule locks and drift per actor (BYD-05) (2026-10-11)

Competitor feature (CineSched): lock the schedule, then report how each actor's days
moved. A lock is a named record in the plan of every scheduled scene of a board (its
sheet, sub-board, day number, date and cast then), not a second copy of the schedule;
What Changed still compares whole files, and the lock uses the same stable ids the diff
matches by. Drift (`drift::drift`), actor by actor in board ID order: work days added and
dropped (by date, or sub-board and day number when undated), and scenes they were on
that moved to another day. Command line `lock`, `unlock`, `drift`; MCP `lock_schedule`,
`drift`.
- **App**: Plan > Schedule Locks...: a name and Lock, then each lock with Drift (the
  actors that changed) and Delete; lock and delete are one undo step each.

## 75. A suggested strip order, never automatic (BYD-06) (2026-10-11)

Competitor feature (auto fill and optimize). A simple heuristic that can be explained in
a sentence, applied only when asked, as one undo step after a preview of before and
after counts (`optimize::suggest`, `optimize::apply`):

- Only the scheduled scene strips of one sub-board move. Day breaks, banners and pinned
  strips (kept in the plan) keep their places, so every day keeps its
  number of strips and the shoot day count stays the same. Filling days up to a limit is
  Auto Day Breaks, which can run after.
- Scenes are grouped by location (else set), then set; groups follow the board's first
  appearance, except that each next group is the one sharing the most cast with the last;
  within a set each next scene shares the most cast with the one before.
- With dates, a place on a date some scene's element is red-flagged on takes the next
  scene in the order that has no flag then, if there is one.
- The preview compares shoot days, cast work days (each cast member's days added up),
  location changes within a day and red flag clashes. Apply is refused if the sub-board's
  strips changed since the preview.
- Command line `suggest [--apply]`, `pin`; MCP `suggest_order`, `apply_order`,
  `pin_strips`.
- **App**: Plan > Pin Strips and Unpin Strips (the selected scenes), and Plan > Suggest
  Strip Order... on the selection's sub-board: the counts now and suggested, and Apply
  Order as one undo step.

## 76. A scene spanning days: one sheet, its strip in parts (2026-10-11)

Inventory section 6: older versions allow multiple day breaks per board for scenes
spanning days (later, pending E-B5). Our reading: one sheet whose strip appears in more
than one day.

- **Model**: no new strip kind. A board may hold a sheet's strip more than once on its
  sub-boards; each is a part, numbered in board order. `Schedule::check` now asks for
  every sheet at least once per board, and a sheet in the Boneyard with no other strip
  there. `add_part` adds one (refused in the Boneyard and for a sheet in it),
  `sheet_strips` lists them, `split_sheets` names the sheets with several,
  `delete_strip` removes a part but never a sheet's only strip, `move_strips` refuses a
  part into the Boneyard, `delete_sub_board` sends a scene to the Boneyard only when it
  has no part elsewhere, and `sheets_in_board_order` lists a sheet once.
- **Counting**: `days::days` and the automatic day breaks count a sheet's pages, shoot
  time and screen time on its first part only (board order), so day totals don't double.
  Everything that reads a day's sheet strips (the day out of days, conflicts, red flags,
  Explain, reports, the budget exchange) sees the sheet on every part's day with no
  change of theirs: its elements work each of those days. The one-line report lists the
  scene on each day it shoots, with its full length on each line.
- **File**: no new key; a file with parts is refused by builds before this entry. The
  three-way merge keeps every part on a sub-board and drops only a strip of the sheet
  that also sits in the Boneyard.
- **The app**: Board > Continue Scene on Next Day and the context menu's Continue on
  Next Day (`continue_scene`): the part goes first in the day after the next day break,
  or, with none after the scene, a day break goes right after it and the part below.
  The view marks parts (`part` of `parts`), the board shows `2/3` after the scene number,
  and the printed board too. Delete on a part asks and removes that part; deleting a
  scene's last strip deletes its sheet as before.
- Not checked (engine check E-B5): how the reference app splits a scene's length across
  days, and how its day out of days and reports show one.

## 77. Sheet numbers, scene images, header field order, flag colors, templates, advanced find (2026-10-11)

Inventory sections 1, 3, 9 and 13; behavior notes 03, 08 and 10; engine checks E-E14,
E-S29 to E-S32. The data side of the split screen, files and breakdown lane; each part
is additive in the file (FORMAT.md) and left out at its default.

- **Sheet number** (`Sheet::number`, BRK-17): a text field of its own, as typed (`12`,
  `12A`), apart from the scene numbers. The older reference app had one as an index; we
  keep it as text so lettered numbers work. Find searches it, MCP and the app set it.
- **Stepping order** (`SheetOrder`, BRK-10): Previous and Next step through a board's
  sheets in board order (the default), by sheet number or by scene number, compared
  naturally (9, 10, 10A, 11), sheets without one last, board order among equals.
- **Images on scenes** (`Sheet::images`, BRK-16): paths beside the schedule, exactly as
  an element's images (decision 62). Merging sheets joins every sheet's images, each
  once; the survivor keeps its sheet number, another number is reported as dropped.
- **Header field order** (`Schedule::sheet_fields`, BRK-03): kept per schedule, as the
  keys of `SHEET_FIELDS`; fields left out follow in the default order, so a field added
  later never goes missing. The default order is the three rows of decision 53 with the
  sheet number first. Merging and What Changed treat the order as one value.
- **Red flag type colors** (`RedFlagType::color`, RFL-01): a `#rrggbb` color per type,
  none by default (the usual red). Note 03 says the reference app has built-in "DOOD"
  types that can't be deleted; they were how the older app put travel, fittings and
  rehearsals on the day out of days, which our events do (decision 63). So we ship no
  built-in types: the plain reading, kept open as E-E14.
- **Templates** (`Schedule::to_template`, `to_template_string`, FIL-04): a schedule file
  marked `"template": true` holding the categories, elements and their settings, red
  flag types, strip colors and layouts, the report list, production info, code letters,
  the sheet field order, the boards and sub-boards with their calendars, and from the
  plan the map locations, the long day limit and the turnarounds, but no sheets, no
  strips, no events, no red flags, and none of the plan that points at them (locks,
  call times, pinned strips, omitted scenes). Our own JSON (`.mtst`), never the
  reference app's template files. Opening one starts a new, untitled schedule from it.
  Elements stay (a series' regulars); the inventory lists the reference app clearing
  sets and locations, which for us live on sheets and go with them. E-S30 asks what the
  reference app keeps.
- **Advanced find** (`find::Condition`, SPL-06): on top of the Find panel (decision 34),
  rows of field, operator and value joined by AND or OR. Fields: every text header
  field, pages, shoot time and screen time. Operators: contains, does not contain, is,
  is not, starts with, is empty, is not empty, greater than, less than. Numbers compare
  as numbers (pages typed `1 3/8`, times `h:mm` and `m:ss`), text naturally, so scene
  `12` is greater than `9`. With conditions, only the header fields of the sheets that
  meet them are searched and replaced, and with no text each such sheet is one result.
  A value that can't be read, or contains on a number, is refused with a sentence. The
  command line takes `--where FIELD:OP:VALUE` and `--any`, MCP `conditions` and `any`.
## 78. The layout library: named layouts, two-line strips, layout files (2026-10-11)

Behavior note 18 (E-S22): the reference app keeps named layouts in a list, each typed
(strips, day breaks, shoot day headers), and its default strip has two lines. Inventory
sections 1 and 5: several layouts, a library shared by the board and the one-line report,
and board and report layouts saved and loaded as files. Entry 39 left these for later.

- **Engine** (`method_schedule_core::layouts`): `SavedLayout` (an id from `next_id`, a
  name unique ignoring case, a body: `Strips(StripLayout)`, `DayBreaks(DayLook)` or
  `ShootDay(DayLook)`, the last using only its text), `Library` on the schedule (the
  list and the one-line report's pick) and `LayoutUse` on each board (its picks by
  type). A board keeps its own strip layout and day look (entries 39 and 67) and uses
  them where it picked nothing, so files from before read and write back unchanged.
  Everything that draws asks `strip_layout_of`, `day_look_of` and
  `one_line_layout_of`. Add, rename, change (its type stays), move, delete (picks fall
  back to the board's own) and import (fresh ids, free names); `check` refuses dangling
  or wrongly typed picks. At most 200 layouts.
- **Two-line strips**: `StripColumn::second_line`. A layout keeps at least one field on
  its first line. The board draws the second line as a board line of its own after the
  strip (the same fixed-height arithmetic as a two-line banner, entry 65), in the strip's
  colors, selected and clicked with it, a drop on it landing below the strip; the
  printed board puts the second line's fields under the strip as `Label: value` pairs.
  The one-line report lays both lines' fields out as columns.
- **The file** (additive): `layouts` and `one_line_layout` on the schedule, `uses` on a
  board, `second_line` on a column, each left out at its default. **Layout files** are
  our own JSON (`method-schedule-layouts`, version 0, the layouts without ids), read as
  strictly as schedules; loading tolerates nothing being there to refer to, since our
  layouts name no categories. Diff: "Layouts ... -> ..." under Schedule and a board's
  `layouts`; merge: the library is one value, a board's picks one value, and a pick of a
  layout the merged library lacks falls back to the board's own (said in the fallbacks).
- **The app**: Strip Layout picks the board's own layout or a named one, edits the one
  picked, saves the fields as a new named layout, deletes a named one, and picks the
  one-line report's layout; a Line choice per field. Day Breaks picks the day break and
  shoot day layouts and saves new ones. Board > Layouts… renames, reorders and deletes,
  and saves the checked layouts (or all) to a file or loads one, through Rust's Save and
  Open dialogs (`export_layouts`, `import_layouts`). Nine commands, each one undo step
  (the file ones aside); the dialogs send one command per change. The command line has
  `layouts FILE [--export OUT | --import IN]`.
- Not done (BRD-15's full designer): merged cells, more than two lines, alignment, line
  styles, fonts, custom text and element lists as strip fields, scale to content. Not
  checked (engine check E-S22): the reference app's default two-line layout's fields
  (ours keeps one line), whether a board picks from the list by reference, and the
  one-line report's own pick.

## 79. The key table after the reference app's, Settings on Cmd comma, a shortcut sheet (2026-10-11)

Inventory section 14; behavior notes 10 and 17; engine check E-K1; screen block 3, A18.
KEY-01, KEY-02 and KEY-05.

- **The documented keys** where the command exists here: New, Open, Save, Save As, Undo,
  Redo, Cut, Copy, Paste, the banner (Cmd B) as before, and now **Close Window** (Cmd W,
  our own item answered in Rust, so it goes through the unsaved-changes question on every
  platform; the stock item had no key on Windows and Linux).
- **The older table** (MMS 5, note 10), where the action exists and the key is free:
  **Go to Scene…** (Cmd J, its find scene), **Linked Elements…** (Cmd L, its element
  linking) and **Add Red Flag…** (Cmd K, its red flag entry), both for the element picked
  on the Elements tab. Its keys that collide with ours stay ours and go to screen block 3
  (A18) rather than being guessed: find sheet (Cmd G is Find Next here; Go to Sheet
  Number… has no key), quick entry (Cmd E is Export), strip layouts (Cmd M is the Mac's
  Minimize), the breakdown window (Cmd Y is Redo on Windows), the board and calendar
  steps on Cmd 2 to 0 (our tabs), preferences (Cmd semicolon).
- **Insert Day Break** moves from Cmd comma (decision 55, after the older manuals) to
  **Cmd D**, Above to Shift Cmd D: Cmd comma is every Mac app's Settings key, and the app
  now has per-user settings. **Settings…** (Cmd comma) is a small dialog for the week
  start and the date format (settings.json, decision 59), in the app menu on macOS and at
  the end of the Edit menu elsewhere. B on the board still inserts below.
- **First and last sheet**: Shift Cmd [ and Shift Cmd ] (the older app stepped to the
  first and last sheet too; its keys are unclear in the scanned table).
- **Help > Keyboard Shortcuts** (Cmd /): every entry of `ui/js/shortcuts.js`, grouped by
  menu, then the board's, the Breakdown tab's and the panes' own keys (a new `page` scope
  in the table, listed but never matched), with a filter. Made from the table at the
  moment it opens, with a catch-all group, so a command added later still shows.
- Every key stays provisional until E-K1 is run in the real app.

## 80. Split screen: a pane beside the board (2026-10-11)

Inventory section 13; behavior note 05. SPL-01 to SPL-04 and SPL-07.

- On the Board tab, **View > Breakdown, Board, Calendar or Images Beside Board** (Shift
  Cmd 2, 1 and 5; Images has no key) shows a pane at the right of the board; choosing the
  one shown again, or **Single View** in the pane's head (Shift Cmd 0), closes it. The
  pane is the tab's own panel moved beside the board, so what is typed in it and its
  listeners stay; on another tab it goes back and that tab shows full width, and back on
  the Board tab the pane returns.
- **Breakdown**: follows the board's selection (a scene's sheet; a banner or day break
  leaves the sheet shown) and edits live, each change one undo step. Its keys, Previous and
  Next, Cut, Copy and Paste of elements and the Breakdown menu work there; the board's
  keys don't while focus is in the pane.
- **Board**: any board, or one of its sub-boards (default: the next board, or the same
  board's second sub-board), drawn by the board's own code with its own ids and colors.
  Click and Shift or Command click select there, double-click opens a scene's sheet,
  dragging moves strips on that board, and its start dates and day break runs work, each
  one undo step. Dragging from one pane into the other is not offered; the plain reading
  of the inventory's "drag between sub-boards" is covered by showing one sub-board beside
  another of the same board, with Move To on the strips' menu. Kept open as E-S33.
- **Calendar**: the Calendar tab beside the board, for the sub-board picked in the bar.
  Dragging strips onto dates is not offered (E-S33).
- **Images**: the selected scene's images (decision 77) and its elements' images
  (decision 62), larger, read only.
- Which pane and how wide are remembered in this browser (localStorage, guarded), not in
  the schedule: the split is a way of looking. The bar between drags, and Left and Right
  on it step by 24 px; the pane keeps 280 px and the board 320 px at least.

## 81. The rest of the breakdown: field order, stepping order, sheet number, images, decimal time, semicolon (2026-10-11)

Inventory section 3; behavior notes 08 and 09. BRK-03, BRK-10, BRK-16, BRK-17, BRK-18.
The data is decision 77's.

- **Header Field Order…** (Breakdown menu): Up, Down and Default Order over the 18 fields,
  one undo step, kept in the schedule; the rows hold six, five and seven fields, so the
  default order is decision 53's layout with the sheet number first.
- **Stepping**: a **by** picker beside Previous and Next (board order, sheet number,
  scene number), remembered in this browser; the header's "Sheet 3 of 12" counts in that
  order. Shift Cmd [ and ] go to the first and last sheet.
- **Sheet number**: the first field, labeled Sheet. **Go to Sheet Number…** and **Go to
  Scene…** find the sheet through the advanced find (decision 77): an exact match; with
  several, the first, and the status line says how many.
- **Images**: a row under the fields with thumbnails, Add Image… and ×, as an element's.
- **Decimal time**: a switch in the head shows the shoot time as decimal hours, two
  places, rounded half up from whole minutes (`1:30` is `1.50`, 140 minutes `2.33`). The
  shoot time and a banner's duration accept decimal hours whichever is shown (`1.5`,
  `.25`, one or two places; hundredths of an hour times 60 over 100, half a minute
  rounding up), still 24:00 at most; a bare number is still refused. The older app had it
  as a preference; ours is this browser's.
- **Quick add's semicolon**: `Props;Lantern` typed in Quick add makes Lantern in Props
  (the category made if new) without asking, as the older app's quick create did. A
  category box's own Add field takes the name as typed.

## 82. Advanced find in the Find panel (2026-10-11)

Inventory section 13; decision 34 (the panel) and decision 77 (the engine's conditions).
SPL-06.

- **Advanced…** in the Find panel shows rows of field, comparison and value, with Add Row,
  a × on each row, and **Match**: all rows (AND) or any row (OR). Fields: every text
  header field, pages, shoot time and screen time. A number field offers is, is not, is
  empty, is not empty, greater than and less than; text adds contains, does not contain
  and starts with. A row whose value isn't typed yet is left out until it is.
- With rows, only the sheets that meet them are searched, and Replace replaces in them
  only. With nothing typed in the search box, each such sheet is one result ("4 sheets
  meet the rows"), a click going to its sheet; Replace needs text.
- A value that can't be read (pages as `lots`) shows the engine's sentence in the panel.
- Not done: highlighting the matches on the board, and a blank replace unlinking a
  pick-list value (E-S32).

## 83. Dragging events on the Calendar; conflict marks on strips, dates and sheets (2026-10-11)

Inventory section 8; behavior notes 03 and 17 (A4). EVT-02 and EVT-06.

- **Dragging an event** on the Calendar's month (its line in a date) to another date moves
  it to start there, keeping how many days it covers, one undo step (`move_event_to`).
  Events listed in the side panel keep Edit for other changes.
- **Conflict marks** (the engine's conflicts for the board shown, decisions 28 and 43): a
  ⚠ before the first column of each strip involved, with a red left edge, its title and
  accessible name saying each conflict (`KIT on 2027-03-02 also on Unit B day 3`, `KIT at
  Fitting on a work day, 2027-03-02`); the same mark on the Calendar's dates on each
  sub-board involved, with a dashed outline; and a line on the Breakdown tab under the
  sheet's head. The Elements table already counts conflicts per element (decision 62),
  and the day out of days outlines an event on a work day (decision 47). A strip with a
  red flag clash and a conflict shows both, as note 03 says the reference app does.

## 84. Red flag type colors and clash marks on the Breakdown tab and the Elements table (2026-10-11)

Inventory section 9; behavior note 03; engine checks E-E9 and E-E14. RFL-01 and RFL-02.

- **Colors**: Red Flag Types… gives each type a color (a color field, and Red for none);
  one undo step, kept in the schedule (decision 77), set over MCP with
  `set_red_flag_type_color`. A clash's ⚑ and the strip's right edge, the Calendar date's
  mark and underline, and the swatch beside the type on the Red flags tab and in an
  element's flags take the color of the first clashing flag's type; with no color, red.
  The colors go into a constructed stylesheet, checked `#rrggbb` only. The day out of days
  outline stays red. Built-in types: none (decision 77).
- **Clash marks**: the Breakdown tab shows a ⚑ line for each clash on the date its sheet
  shoots, on the sub-board holding its strip on the board shown; the Elements table has a
  Red flags column counting each element's clashes on the picked sub-board. The board, the
  Calendar and the day out of days had marks already (decision 36), and the reports show
  them with their Red flag marks option (decision 59).

## 85. User templates in the app (2026-10-11)

Inventory section 1; decision 77 (the file). FIL-04.

- **File > Save as Template…** shows a Save dialog in the data folder's `Templates/`
  (made when needed), suggesting `<title>.mtst`, and writes `Schedule::to_template`
  atomically. It refuses the open schedule's own file, and changes nothing in the
  schedule. **File > New from Template…** asks about unsaved changes, shows an Open dialog
  there, and puts a new, untitled schedule from the template in place of the open one;
  a plain schedule file picked there is taken as a template too. Open on a file marked
  as a template does the same, so the template is never saved over. Paths still come
  only from Rust dialogs.
- The command line: `method-schedule template FILE OUT` writes a template, and `new FILE
  --from TEMPLATE` starts a schedule from one, and every other command that writes keeps
  a template marked as one, refusing a change that would give it sheets, strips, events
  or red flags. MCP has no template tool: the read tools (`read_schedule`, `check` and
  the rest) read a `.mtst` file, and `list_schedules` lists them, but the edit tools
  refuse a template, as `.mtst` or as a `.json` file marked as one, with a sentence
  saying to change it in the app or start a schedule from it.
- The reference app's own template files are never read (CLEAN-ROOM.md).

## 86. Several schedules open at once: one window per schedule (2026-10-11)

Inventory section 1. FIL-03.

- **One window per schedule**, as the inventory reads the reference app (a switcher, an
  edited mark): File > New Window (Shift Cmd N) and Open in New Window…; New and Open still
  replace the schedule in the window they are chosen in, after the unsaved-changes
  question, as before. A file already open in a window brings that window to the front
  instead of opening twice (Open, Open Recent, Open in New Window, the system's open
  events).
- **Each window has its own session** (`src/docs.rs`, by window label): undo and redo,
  the app's clipboard (strips and elements are not pasted across windows), the board
  shown, autosave timing, the recovery copy and the Don't Save choice. Settings, Open
  Recent, backups and the Autosave switch are the app's, shared. Commands take the
  calling window's session (`Doc`), and a test refuses a command written against a
  single managed session, so a command can't reach another window's schedule.
- **The edited mark** is the window title's `(edited)` (as before) and the Window menu's
  list shows the titles, so the mark shows there too. On macOS the system's Window menu
  lists the windows; on Windows and Linux our Window menu does.
- **Closing**: Close Window (Cmd W) and the window's close button ask about that window's
  unsaved changes; Quit asks each window in turn, and Cancel on any stops it. A window
  that closes keeps what the user didn't throw away (into its file with Autosave on,
  else a recovery copy), as quitting does. Recovered schedules are offered in the first
  window only.
- **Security**: the schedule windows (`doc-*`) get exactly the main window's capability,
  the same CSP and the same navigation guard; paths still come only from Rust dialogs.
- Not done: moving strips between windows, and tabs inside one window. Checked only in
  tests and the page harness; a live run should open two schedules side by side (screen
  block 3, part B).

## 87. The full strip designer, banner text size and alignment, a day's quantities on its day break (2026-10-11)

Behavior note 18 (E-S22): the reference app's strip designer has several lines a strip,
fields merged across cells, alignment, bold and italic, a font size, custom text and a
spacer. Inventory section 5 (banners: size and alignment) and section 6 (a day break may
show a quantity). Entries 39, 65, 67 and 78 left these for later. Engine, file, app.

- **Lines and merged cells** (`StripColumn::line`, `span`): a strip has up to four lines
  (`MAX_STRIP_LINES`). The first line's fields are the strip's columns, one each, at
  their widths. A field on a later line sits under them, after the one before it on its
  line, covering `span` of those columns; its own width is kept but not used. So a
  merged cell is a field spanning columns, every line lines up with the first, and a
  line underfilled leaves its last columns empty. `StripLayout::problem` refuses a fifth
  line, a span on the first line or of zero, and a later line covering more columns than
  the first has. The two-line strips of entry 78 are line 2 with a span of one, so
  their second line now sits under the first line's columns rather than in columns of
  its own: our reading of the reference app's designer grid.
- **Looks** (`StripColumn::align`, `bold`, `italic`; `StripLayout::size`): alignment
  Usual (numbers to the right, the rest to the left), Left, Center or Right; bold and
  italic per field; a text size step per layout (`TextSize`, the report options' Small,
  Normal, Large, now shared; no font families). The line height stays fixed (22 px), so
  Large is 15 px text on the same line.
- **Spacer and custom text** (`StripField::Spacer`, `Text`): may repeat; a text field
  holds its text on the column (`StripColumn::text`, one line, at most 100 characters),
  the same on every strip, so the view sends it once with the layout and each strip's
  cell for it is empty: no work a strip is added, and the 1,500-strip view stays
  488 KB (`perf_edit_and_view_at_1500_strips`: edit 0.7 ms, view 4.3 ms, JSON 0.4 ms,
  on main with the other lanes' marks). Reports and the
  calendar pick their columns from the sheet fields only (`StripField::SHEET`); the
  report options and the calendar refuse the other two.
- **Banners** (`BannerStyle::size`, `align`): Small, Normal or Large text, and Left,
  Center or Right (Left is the usual and stores as nothing). Fixed choices; no fonts.
- **A day's quantities** (`DayLook::quantity`, `reports::day_quantity_text`): the day
  break shows each category that uses daily quantities (entry 63) with its total for the
  day, `Background 45 · Animals 3`, an asterisk on a partial total, nothing for a day
  without; worked out only when the board shows it. The daily totals report and this
  share one sum.
- **File** (additive, each left out at its default): on a column `line` (3 or 4; line 2
  stays `second_line`, so files from entry 78 write back byte for byte), `span`,
  `align`, `bold`, `italic`, `text`; `strip_size` on a board and `size` on a named strip
  layout; `size` and `align` on a banner; `quantity` in a day look. Readers refuse an
  unknown alignment or size, `line` other than 3 or 4, and the consistency rules above.
  Diff and merge treat a layout and a banner style as one value, as before; What
  Changed now says the new parts (`Synopsis (line 2, spans 3, italic); large text`).
- **The app**: Strip Layout has Line 1 to 4, the columns a later field covers, Usual,
  Left, Center or Right, B and I per field, a text box for custom text, Add Spacer and
  Add Text, and Text size; the commands are the same ones with more fields
  (`set_strip_layout` takes `size`, a layout body `size`). The Banner dialog has Text
  size and Alignment; Day Breaks has the quantities switch.
- **Fast strips**: a scene strip is one row of the board as many board lines high as its
  layout's lines (a two-line banner two), not a row plus tail rows as in entry 78. Each
  list keeps a running sum of its lines' heights (`lineTops`), and the windowing, the
  scroll into view and a drop's place use it with a binary search (`lineAt`); a test
  holds `LINE_PX` to the stylesheet's line heights. Each field's place is a rule in a
  constructed stylesheet (`grid-area`, numbers only), so the CSP stays as strict.
- **One cell renderer** (strips.js: `stripGrid`, `cellText`, `cellClass`, `stripCell`,
  `sceneRows`, `layoutHead`): the board, the printed board and the one-line, breakdown
  by day, element detail and multi-unit reports draw a strip's fields with it. On paper
  a later line is a table row of its own under the strip and a merged cell a cell
  spanning columns.
- Not checked in the real app (engine check E-S22 and E-B15): how its designer aligns a
  later line with the first, its font size steps, whether its banner alignment has more
  than three choices, and how its day break shows a quantity (our reading: the daily
  quantity categories' totals). Not done: line styles, scale to content, element lists
  by category as strip fields.
- **Banner styles on the command line and over MCP** (added 2026-10-12): `banners FILE`
  lists every banner of the first board with its style, and `banner FILE` inserts or
  changes one (`--bold`, `--color`, `--height 1|2`, `--size`, `--align`); MCP's `days`
  and `one_line` give each banner a `style` object, and `insert_banner` and `set_banner`
  take the same parts, each left out kept. Left and the usual alignment are one for a
  banner, as in the app.

## 88. Several windows: who hears which event, and what may run under a schedule's lock (2026-10-11)

A review of entry 86. Rules every later window feature follows.

- **Events**: an event for one window goes to that window only (`emit_to(label)`: a menu
  item, the file state after an autosave, an autosave problem, a file the system asked
  to open); an event for every window goes to all (`emit`: the Autosave switch and
  `prefs-changed`). The page listens on the label Tauri gives its own window
  (`getCurrentWebviewWindow().label`) and never names a window; a test holds this. The
  fixed `main` it had made the `doc-*` windows deaf to their menus and sent the main
  window's events to every window.
- **Preferences** (week start, date format, calendar fields) are the user's, so a change
  in one window goes to every window, and the others read them again and redraw.
- **Nothing waits for the main thread under a schedule's lock.** A window's getters
  (`title()`, `is_focused()`), menu reads and changes, and blocking dialogs all wait for
  the main thread, and the main thread takes schedules' locks (closing, sync commands),
  so a command doing so under the lock could hang the app. Under the lock only queued
  setters run (`set_title`, `emit_to`); each window's title is kept with its schedule
  (`AppState::retitle`), so the Window menu lists titles without asking any window; both
  menu refreshes (`windows::refresh_menu`, `safety_app::refresh_menu`) only queue their
  work on the main thread. A test refuses asking a window for its title. Dialogs are
  shown only with no lock held, and `Docs::holding` (which locks every schedule in turn)
  never runs under one.
- **Per window**: the close question (a second click on the close button asks nothing
  more), the last autosave problem said, and the files picked for Merge Changes From
  (a token names a file picked in that window only). A window that fails to open leaves
  no schedule behind.
- **Closing**: Quit asks each window with changes in turn; Don't Save takes effect only
  once every window has answered, so Cancel on a later one leaves the earlier ones'
  changes kept safe (autosave and recovery copies go on). A schedule put in a window's
  place (New, Open, a template) starts kept safe again.
- **One file, one window**: Save and Save As to a file open in another window, opening a
  recovered copy of it, or a file the system hands over that another window has, are
  refused (`NAME is open in another window.`), before the unsaved-changes question
  where the file is known first; Open shows its dialog first and brings the other window
  to the front without asking about this window's changes.

## 89. Bounds on a sheet's numbers, and what check asks of the plan (2026-10-12)

A review of the engine found numbers with no upper bound (a sheet's eighths, shoot and
screen time, `next_id`), sums that could overflow and panic, and plan data the setters
and the file reader refuse that `check` let through (a merge or a tool could leave it).

- **Bounds** (`model::MAX_*`, in `check`, so in the reader, a merge and every edit): a
  sheet holds 999 pages at most (7992 eighths, far past any scene, so totals of many
  sheets stay far from the end of a number); a shoot time and a banner's duration are
  24:00 at most, as typed in the app (entry 50); a screen time 24 hours; a quantity
  99,999 (the app's limit, now the engine's); `next_id` 1,000,000,000. Entry 50's "not
  in the engine's check" is replaced: merging sheets whose shoot or screen times add up
  past a day, or pages past 999, is refused with a sentence (shorten a sheet first),
  where it used to make a sheet longer than a day. A script import cuts a longer scene
  to 999 pages.
- **Nothing panics on a sum**: page totals (`Eighths` add and sum) and a day's time and
  screen time saturate; new ids saturate (`Schedule::fresh`), and the cap keeps them far
  from it.
- **The plan in check** (`plan::check`): locations named once, trimmed, on the map and
  within 14 hours of UTC; call times before 24:00 and one per day; the long day and
  turnarounds in range, one turnaround a category; lock names one line of 1 to 200
  characters (`MAX_LOCK_NAME`, also in `drift::lock`), unique ignoring case; a strip
  pinned once and only a scene strip; a sheet marked omitted once. A call time, pin or
  omitted mark naming something gone is still left to the next plan edit (`plan::tidy`).
- **Also in check**: a property name one line of at most 60 characters (as the setter
  says), and an element's property values in its category's order (as
  `set_property_value` keeps them).
- **Advanced find** (entry 82): an empty number (no shoot or screen time, no pages) meets
  only Is Empty. It used to count as 0 for Is, Is Not and Less but as empty for Greater.
- A parked day break keeps its call time (a call time is on any day break, the
  Boneyard's too), and automatic day breaks reuse the old breaks' ids in order, so day 1
  keeps day 1's call time; Remove All drops the call times with the breaks.

## 90. The report designer: merged cells, and a text size, bold and italic per column (2026-10-12)

Inventory section 11 (the report designer); RPT-15. Decision 59 gave each report its own
columns, one line of field names. Now each column is what a strip layout column is
(decision 87) where that fits, plus a size of its own:

- **Lines and merged cells**: a column on line 2, 3 or 4 prints under the first line's
  columns, after the one before it on that line, covering `span` of them. The refusals
  are a strip layout's: a field on the first line, each field once, the first line one
  column wide each, no later line wider than the first.
- **Looks**: alignment, bold, italic, and a text size (small, normal, large) that
  overrides the report's Text size for that column; none follows the report.
- **Width** stays the board's strip layout's for the field, as before, so the designer
  has no width of its own; custom text and spacers stay on strips.
- **The file**: a plain column is still written as its field's name, so files from
  before read and write back byte for byte; any other is an object
  (`field`, `second_line` or `line`, `span`, `align`, `bold`, `italic`, `size`), each
  key left out at its default, and an object with only `field` is refused.
- **The app**: Reports > Columns… lists every field with Show, Line, the columns it
  covers, alignment, B, I, its size and Up and Down (density as decision 53). The
  command line's report list names each column's look; MCP's `report_list` gives a
  column as a name, or an object when it has more.
- Not taken: fonts by name and footer fields (the reference app's designer has them; we
  keep one type face for print, as decision 23 keeps report layouts our own), and
  dragging fields on a page. E-R3 in engine-checks: whether the reference app's merged
  cells span columns of the same row, as ours do under the first line; the plain
  reading of the inventory.

## 91. One day quantity, omitted scenes count nowhere, the plan merges entry by entry (2026-10-12)

A review of the engine. Each was a failing test first.

- **A day's quantity has one meaning** (entry 63's sum): for an element of a category
  that uses daily quantities, each of the day's sheets' quantity of it added up (one for
  a sheet that gives none), leaving out one a sheet marks not to count daily (the total
  is then partial, with an asterisk); an element of any other category counts once a
  day. `reports::day_quantities` works it out, and the day break's text (entry 87), the
  daily totals and the breakdown report by day all read it. The report by day's
  `needed` used to be the most any one sheet needed, for every category; it is now this
  sum, listed for daily-quantity categories only, with the partial ones marked
  (`BreakdownDay::partial`; MCP `breakdown_day` gives `partial`).
- **An omitted scene counts nowhere** (entry 69's open point): a scene a script revision
  no longer has stays on the board in its place, struck (as the app already drew it),
  and stays listed in reports, struck; but it adds no pages, time or screen time to its
  day, is no work for its elements (the day out of days and its explanation, conflicts,
  red flag clashes, drift, the vertical cast chart, the element list's sheet and page
  counts and the daily totals leave it out), is not on the timeline or the call sheet,
  and stays where it is in a suggested order (counted with the pinned strips). In the one-line and board spreadsheets its `kind` is `omitted_scene` and, in the one-line, it has no eighths (review 2026-10-13: its eighths reached the Day Totals sheet). Clearing
  the mark (a later draft that has the scene again) brings it all back. Engine:
  `DayStrip::omitted`, `Day::counted_sheets`. Not checked in the real app (engine check
  E-I7): our plain reading of what an omitted scene means.
- **The plan merges entry by entry** (it was one value, so any two plan edits
  conflicted): map locations by name, call times by day break, turnarounds by category
  and locks by name, each entry one value; pinned strips and omitted marks as sets; the
  long day limit as a field. A conflict names the entry (`Call time of the day closed by
  ...: ours 07:00, theirs 05:00, was 06:00`) under a key of its own (`plan:location:pier`,
  `plan:call_time:12`, `plan:turnaround:3`, `plan:lock:white`, `plan:long_day`).
- Also from the review, without a rule change: merge renumbers a layout id theirs used
  for something else and moves quantities with a renumbered element; a scene part one
  side deleted stays deleted; a property value set on one side follows a rename on the
  other (or a fallback line says it was left out); the timeline, call sheet and rest
  count a later part of a scene spanning days as its day break does (no time, no pages);
  the suggested order leaves every part of a split scene in place; drift counts each
  part's day.
- **Speed**: the board view asked for every conflict twice (count and marks) and found
  sheets, elements and strips by walking lists. Now `conflicts` answers at once when a
  board has fewer than two dated sub-boards and no events (nothing can clash; a test
  holds it to the full answer on random schedules); a day lists each strip's place and
  sheet (`Day::entries`), so callers stop finding strips again; the view indexes sheets
  and elements once (`Schedule::sheet_index`, `element_index`) and works a strip's cast
  out once for its cast list and cells. `perf_edit_and_view_at_1500_strips` (release):
  view 4.7 ms before, 2.3 ms after (with six red flags 5.5 ms, now 2.6 ms); edit and
  JSON unchanged (0.6 to 0.9 ms, 0.5 ms). `review_profile_view` breaks a view down.

## 92. The Universal Schedule Standard is skipped (2026-10-12)

The owner's call on roadmap question 11. The format (docs/research/interop-formats.md
section 4) has little adoption (a small public repository) and the reference app doesn't
read or write it, so it would help almost no one moving a schedule over. Its license
wording also contradicts itself. Parity row EXP-08 is marked skip. Our own open file
(docs/FORMAT.md), the spreadsheet exports and the budget exchange stay the ways out.

## 93. Element columns and quantities in the one-line spreadsheet (2026-10-12)

Parity row EXP-09; docs/research/interop-mms-open-formats.md rows 9 and 10. The
reference app added element quantities to its one-line Excel in 10.14.14; its columns
are not public, so this is the plain reading (engine check E-R10).

- **Columns**: the one-line sheet keeps its twelve columns as they were, names and
  order (`day` to `text`), since other tools read them. After them comes one column per
  category the schedule's one-line report shows (its category filter; every category when
  the filter is empty), in category order, titled with the category's name. A category
  with nothing on a scene leaves the cell empty; a banner row leaves every one empty.
- **Cells**: the scene's elements of that category, in board ID then name order, joined
  by `; `. Each is its board ID, or its name when it has none; with the report's "cast by
  name" option on, every category lists names. A sheet quantity other than one follows
  as ` x3` (decision 63's quantity, whether or not it counts daily).
- **Which options**: the first one-line report in the schedule's report list (the
  defaults when the list is the default). `export::one_line_with` takes any options;
  `export::one_line` reads the schedule's. CSV gets the same columns; formula-looking
  element names are neutralized as all CSV text is (entry 14).
- **Command line, MCP, app**: `oneline --csv | --xlsx` and File > Export follow without a
  change. MCP `one_line` adds `elements` to each scene line: each shown category with its
  elements (`element`, `board_id`, `name`, `quantity`).

## 94. The breakdown by shoot day as a spreadsheet (2026-10-12)

Parity row EXP-10; docs/research/interop-mms-open-formats.md row 15. The reference app
exports its by-day breakdown to Excel; its row shape is not public, so this is the plain
reading (engine check E-R11).

- **Rows** (`export::breakdown_day`, the data of `report_data::breakdown_by_day` as the
  report prints it): one per shoot day and element the day's scenes list, days in order,
  a day's elements in category order, then board ID and name. A day with no elements has
  no rows. One row per element keeps the sheet easy to filter and sum, the use a call
  sheet puts it to.
- **Columns**: `day`, `date`, `category`, `board_id`, `element`, `scenes` (how many of
  the day's scenes list it), `quantity` (the day's quantity by entry 91's one rule, for a
  category that uses daily quantities; empty for the others, which count once a day) and
  `partial` (`yes` when a sheet's quantity was left out of the day's count, the asterisk
  of the printed report).
- **Where**: `breakdown FILE --by-day --csv | --xlsx OUT`, and File > Export's
  "Breakdown by day" (`export_report` with `breakdown_day`). MCP already returns the same
  data from `report` with `kind: breakdown_day`; it writes no files.

## 95. Workbook totals as working formulas; text never a formula (2026-10-12)

Parity row EXP-11; docs/research/interop-mms-open-formats.md row 16. The reference app
says its Excel keeps "working formulas"; which cells hold them is not public, so this
is the plain reading (engine check E-R12).

- **Which cells**: the day out of days summary's and grid's `total` (work plus hold,
  `D2+E2`); the one-line workbook's new second sheet, `Day Totals`, a row per shoot day
  with its scene count (`COUNTIF` of `scene` over the day's rows of the one-line sheet)
  and eighths (`SUM` of them), then a `total` row adding the days up; and the new daily
  category totals export (`export::daily_totals`, `totals FILE --csv | --xlsx OUT`, File >
  Export's "Daily category totals"), a row per day, a column per category, a `partial`
  column naming the categories with a quantity left out, and a `total` row of `SUM`s.
  The one-line sheet itself keeps its rows as they were (a scene or banner each), so
  tools that read it see no new kinds of row; the totals live on their own sheet.
- **Cached values**: each formula is written with its value (`<f>` then `<v>`), worked
  out the same way the formula would, so a viewer that never recalculates (Quick Look,
  a phone preview) shows the numbers. The day's eighths are the sum of its listed
  scenes, as the formula sees them; a scene marked omitted (entry 91) is listed with its
  pages, so there the formula's total can differ from the day break's count.
- **Text never becomes a formula**: only `export::Formula`'s constructors make a formula,
  from column and row numbers, our own sheet names and one fixed word, never schedule
  text. Every text cell, headers too, is a shared string (`t="s"`), never an inline string
  or a formula string, whatever it starts with (`=`, `+`, `-`, `@`); a test writes such
  text and reads it back as text, with the one formula the only `<f>`.
- **CSV** carries values only: a formula cell writes its value. Text that a spreadsheet
  would run keeps its leading `'`, as entry 14 set (the common rule; a CSV has no cell
  types, so a quote is the only way to keep it text).

## 96. Document properties on saved PDFs (2026-10-12)

Parity row EXP-12; docs/research/interop-mms-open-formats.md row 17. The reference app
says its PDFs carry metadata for search; which fields is not public, so this is the
plain reading (engine check E-R13).

- **Fields**: title, the job's name (`<schedule> - <report>`, or `- <board>` for a board
  print); subject, what the page shows (the report's name, board and sub-board, and the
  date range when one is set; `Strip board; <board>` for a board print); author, the
  production company from the production info (left out when empty); creator, `Method
  Schedule`. Nothing else: no user name, no file path, no keywords. Control characters
  become spaces and each field is cut at 200 characters.
- **How**: the print system writes the PDF as before (`pdf_mac.rs`), then
  `method_schedule_format::pdf_info::set_info` adds an incremental update at its end: a new
  information dictionary (UTF-16 hex strings, so any title reads back exactly), a one-entry
  cross-reference section and a trailer pointing back with `/Prev`. The pages stay byte
  for byte; the old creation date carries over. A file with a cross-reference stream or
  encryption is left as it was rather than risk breaking it (macOS writes a classic
  table, checked against a Quartz PDF read back with PDFKit). Then the file is renamed
  into place, as before.
- **macOS only today**: on Windows and Linux the page still prints with
  `window.print()`, and the system dialog's PDF carries its own properties, not ours.

## 97. The full production info (2026-10-12)

Behavior note 19 (RPT-17): the reference app's Production Info dialog, in its Design
menu, holds the title, company, director, producer, unit production manager, assistant
director, art director, set dresser, script date, production number, preparer, timing,
episode and four free fields, all text. Ours had the first four (decision 59).

- **Engine** (`setup::Production`): every field of that list, our own keys
  (`unit_production_manager`, `assistant_director`, `art_director`, `set_dresser`,
  `script_date`, `production_number`, `preparer`, `timing`, `episode`, `custom_1` to
  `custom_4`), each plain text, trimmed and cut at 200 characters (`Production::set`).
  The script date stays text as typed, as the reference app has it; nothing reads it as a
  date. The free fields are named Custom 1 to 4; they have no labels of their own.
- **File**: each a key of `production`, left out when empty, so files from before read and
  write back byte for byte. Merge goes field by field, diff names each field.
- **App**: Reports > Production Info… lists the title and every field two to a row,
  labels beside the fields; one undo step per field changed, as before. The header
  designer (entry 99) places any of them on a report.
- **Command line and MCP**: `production FILE` shows every field and takes each as a flag
  (`--unit-production-manager`, the key with dashes); MCP's `set_production` takes every
  key and `report_list` returns them all under `production`.

## 98. The day out of days sort in the reference app's terms (2026-10-12)

Behavior note 19 (DOD-12, DOD-13, E-D25) settles the sort: up to four keys, each a field
and an order, from board ID, element name, first working day, last working day, worked
days and held days. Ours (decision 59) had the same six keys under other names, so
nothing changes in how rows sort (DOD-18):

- The Sort dialog names the keys as the reference app does (Element name, First working
  day, Last working day, Worked days, Held days); `SortKey::label` holds the words.
- `SortKey::from_name` also takes `first_work`, `last_work`, `worked` and `held`, so the
  command line's `--sort` and MCP's `sort` read either spelling; the old names stay.
- Still ours, unseen in note 19: blanks last whichever the order, and a sort kept for the
  session and cleared on a sub-board switch. Clear, Cancel and Sort were already the
  dialog's buttons.

## 99. Report header layouts (2026-10-12)

Behavior note 19 (RPT-18): the reference app's report designer has a second tab for the
report header, a named header layout with include header, first page only, bold, italic,
alignment and font size, and fields placed in rows from a list: space, custom text, the
report name, today's date, board, sub-board, the start date and every production info
field. Ours had three fixed headers (full, compact, title only; decision 59).

- **Engine** (`method_schedule_core::headers`): `HeaderDesign` (an id, a name unique
  ignoring case, include, first page only, bold, italic, alignment left, center or right,
  a `TextSize` step, and up to three rows of up to twelve `HeaderItem`s); the schedule's
  list (`Schedule::header_layouts`, `header_list`, `add_header`, `set_header`,
  `delete_header`, `move_header`), at least one and at most 50. The title is an item of
  its own (the schedule's title, printed larger), since the old headers led with it.
- **The three old headers are layouts**: the list starts as Full (id 1: the title; company,
  director, producer; report name, board, sub-board, start date), Compact (2: one bold row)
  and Title only (3), and stays out of the file while it is that list. A report's
  `header` option is now a layout id; ids 2 and 3 are still written `compact` and
  `title`, Full is still left out, so every file from before reads and writes back byte
  for byte. Other layouts are written by number; a number for one of the first three is
  refused. New ids count on from the highest, never below 4. Printed output of the old
  three changes a little (their rows now follow the generic renderer).
- **Deleting a layout** leaves reports' picks as they are; a report whose layout is gone
  prints with the list's first (`Schedule::header_of`). Plain reading; the reference app's
  behavior here was not seen.
- **Printing**: Rust fills in each item's text (`reports::head_of`; a production field
  as `Label value`, company and the free fields as the value alone; empty fields drop
  out); the page dates today's date and the start date in the user's format. First page
  only puts the header at the top of the first page; otherwise it prints above every
  page's band. A space pushes what follows to the far side of its row. Ranges and
  filters still print as notes under it.
- **File** (`header_layouts`, FORMAT.md "Header layout"): `hidden`, `every_page`, `bold`,
  `italic`, `align`, `size` left out at their defaults; items by name, custom text as
  `{"text": ...}`. Merge treats the list as one value (as the report list), diff names
  each layout added, removed or changed.
- **App**: Reports > Header shows the layouts by name, and Layouts… opens Header Layouts:
  pick a layout, New, Copy, Delete, Up, Down; its name, the four switches, alignment and
  size; three rows of fields, each with move left, move right and remove, and Add field…
  (custom text takes a line of text). Save Layout is one undo step. Three commands:
  `save_header_layout` (id 0 adds), `delete_header_layout`, `move_header_layout`.
- **Command line and MCP**: `headers FILE` lists each layout (id, looks, rows);
  `reports FILE` names each report's header layout; MCP's `report_list` gives
  `header_layouts` and each report's `header` by name.
- Not taken: fonts by name (decision 90 keeps one type face), a footer designer.

## 100. Report designer strip options (2026-10-12)

Behavior note 19 (RPT-19): a report has switches for shoot day strips and the total strip
count, a page break choice, a day break choice (all, or fewer), an element category
picker, and a strip layout picked from the layout list. Decisions 59, 78, 87 and 90 gave
the rest of the designer.

- **Options** (`ReportOptions`): `page_break` (none, each day, each week; replaces
  `page_per_day`), `shoot_days` (the line heading each day, on by default), `day_breaks`
  (the end of day line with its totals, on by default: our reading of "all, or fewer" is
  all or none), `total_count` (a last line, `Total: 12 scenes on 4 shoot days, 14 3/8
  pages`), `element_columns` (up to ten categories, each a column after the scene
  fields listing the scene's elements of it, `ID NAME`, the cast as the cast column
  shows it) and `layout` (a named strip layout).
- **Which reports**: the one-line, shooting schedule and breakdown by day take all of
  them; the breakdown by strip a page break (a block per page, or a week per page) and
  the total; the element detail and multi-unit one-line the layout and category columns.
- **The layout pick** extends the one-line's own pick (decision 78) to every report with
  scene columns: a report's own columns (the designer, decision 90) win, then its picked
  layout, then the one-line's library pick or the board's layout (one-line and
  multi-unit), else the kind's usual fields. Deleting the layout, or a merge that drops
  it, puts the report back on its own columns (`tidy_report_filters`).
- **A page per week**: a dated day whose week (by the user's week start) differs from the
  dated day before it starts a page; undated days never break. Worked out in Rust
  (`new_page` on each day and block), so the page only splits.
- **File** (additive): `page_break: "week"` (a page per day stays `page_per_day: true`;
  both together are refused), `shoot_days: false`, `day_breaks: false`,
  `total_count: true`, `element_columns`, `layout`; each left out at its default.
- **App**: Reports options add Layout (Own columns or a named strip layout), Category
  cols…, New page (None, Each day, Each week) and the switches Shoot day lines, Day
  breaks and Total count, each shown for the kinds that use it.
- **Command line and MCP**: `reports FILE` names each that is set; MCP's `report_list`
  returns every one.
- Not checked in the real app: what its "fewer" day breaks mean, how its total strip count
  prints, and whether its element category picker adds columns (our reading of the
  inventory's one-line extra columns). E-R14 in engine-checks.

## 101. The breakdown designer: hidden fields and widths (2026-10-12)

Behavior note 19 (BRK-03, BRK-19; settles engine check E-S31): the reference app's
breakdown designer keeps the sheet's header fields in a grid of rows with set widths;
fields are moved, removed (dragged to a remove area) and added back. Ours could only
reorder (decisions 77 and 81).

- **Engine** (`model::SheetLooks` on `Schedule::sheet_looks`): the fields hidden and a
  width per field (`FieldWidth`: narrow, medium, wide; none set keeps the field's usual
  width), by key, kept in `SHEET_FIELDS` order so one design has one form. Hiding is
  only how the tab lays the sheet out: a hidden field keeps its values on every sheet,
  in the file, in find, reports, exports, the command line and MCP. Merge treats the
  looks as one value (as the field order); diff says `Sheet fields default -> hides
  unit; set wide`.
- **File** (additive): `sheet_hidden` (keys) and `sheet_widths` (key to width), each left
  out when empty; unknown keys, widths, repeats or a list out of order are refused.
- **App**: Breakdown > Breakdown Designer… (the menu item that was Header Field Order,
  named as the reference app's Design menu names it) lists every field with Show, a
  width (Usual, Narrow, Medium, Wide; the pages pair keeps its own), Up and Down, and
  Default. Use These sends the order, hidden fields and widths in one command
  (`set_sheet_field_order` with `hidden` and `widths`), one undo step. The tab leaves
  hidden fields out of its three rows (six, five, the rest) and sizes the others.
- Ours keeps the sheet number field, which note 19 did not see in the reference app's
  list; a user who doesn't want it hides it.
- Not done: dragging fields in a grid, and rows of the user's own lengths.

## 102. Calendar strip fields in the schedule, and an Elements strip field (2026-10-12)

Behavior note 19 (CAL-08, CAL-11; settles part of E-C11): the reference app's calendar
strip designer is in its Design menu, part of the schedule, not a user preference; its
default fields are the scene number, the set, the elements and the cast IDs, and it
offers the board strip designer's field list. Decision 59 kept ours in the user's
`settings.json` (scene, set, pages). The week start and date format stay app-wide
preferences, which note 19 confirms.

- **Engine**: `Schedule::calendar_fields` (empty while it is
  `DEFAULT_CALENDAR_FIELDS`: scene, set, elements, cast), `calendar_strip_fields`,
  `set_calendar_fields` (one to 13 sheet fields, each once). Merge treats it as one
  value; diff says `Calendar strip fields default -> scene, set`.
- **A new strip field, Elements** (`StripField::Elements`, `elements`): the sheet's
  elements other than the cast, from the categories on the board, in category order then
  board ID, each by board ID or name. Any strip layout, report column or calendar strip
  can show it, as note 19's shared list implies. The element count field (decision 66)
  is now labeled Element count to tell the two apart; its key is unchanged.
- **File** (additive): `calendar_fields`, left out at the default (writing the default is
  refused, as other defaults are).
- **The old per-user setting, read once**: while the schedule has no fields of its own,
  the Calendar uses the user's `calendar_fields` from `settings.json` if the user ever
  changed it from that preference's default (scene, set, pages), so nobody loses a pick;
  otherwise the new default. Strip Fields… now saves to the schedule
  (`set_calendar_fields`, one undo step) and starts from the fields shown; from then on
  the schedule's fields win. The preference stays in `settings.json`, unused once a
  schedule has fields.
- Not checked: the reference app's calendar strip alignment choices (note 19 mentions
  alignment; ours has none on calendar strips).

## 103. The one-line spreadsheet follows the strip layout (2026-10-12)

Follow-up to decision 93; behavior note 20 (E-R10). The reference app's one-line Excel
has a first column for day break and banner text, then the strip layout's fields in the
layout's order, a row per strip in board order, and day break rows with their end text,
date, page total and time spread over cells, every cell text.

- **By default** (`export::OneLineColumns::Layout`) ours does the same: a first column,
  `Day breaks and banners`, then each field the report shows (`Schedule::report_fields`:
  the report's own columns, else its picked strip layout, else the one-line's library
  pick or the board's layout, as the printed report picks them; spacers and custom text
  left out), titled with the field's label, each cell the text the strip shows
  (`reports::strip_cell`). A banner's text sits in the first column. Each day closes with
  a day break row when the report shows day breaks: `End of Day N`, the date, `3/8
  pages` and `h:mm`, a cell each from the first column on (the last ones joined in the
  last cell when a narrow layout leaves fewer than four cells).
- **Kept beyond the reference app**: decision 93's category columns follow the fields,
  then two working columns, `kind` (`scene`, `banner`, `day_break`, and `omitted_scene` with no eighths for a scene decision 91 omits; review 2026-10-13) and `eighths`, which
  the `Day Totals` sheet's formulas read (decision 95; the reference app has no formulas,
  note 20, and ours keeps them on purpose).
- **The fixed columns stay** as an option (`OneLineColumns::Fixed`), unchanged, so tools
  that read our `day` to `text` columns keep working: `oneline FILE --csv | --xlsx OUT
  --columns fixed` (`layout` is the default), File > Export's "One-line, fixed columns",
  and MCP `one_line`'s `columns` (`layout` or `fixed`), which adds the sheet as `table`
  (without it the result is as before).
- CSV gets the same first sheet; text a spreadsheet would run keeps its leading `'`
  (entry 14).

## 104. TIFF, HEIC and BMP images (2026-10-12)

Behavior note 21 (IMP-09): the reference app's Add Image picker takes PNG, JPEG, GIF,
WebP, TIFF, HEIC and BMP, and refuses PDF.

- Element and sheet images take the same seven. The Add Image dialog's filter lists them
  (`elements::IMAGE_TYPES`: `tif`, `tiff`, `heic` and `bmp` added).
- Rust still reads a thumbnail only when the file's first bytes are an image it knows,
  whatever the name says (`elements::sniff_image`): TIFF by `II*\0` or `MM\0*`, BMP by
  `BM` (with room for its header), HEIC by an `ftyp` box whose brand is `heic`, `heix`,
  `mif1` or `msf1`. The `data:` URL carries the type the bytes say (`image/tiff`,
  `image/bmp`, `image/heic`).
- No conversion: macOS's WebKit draws HEIC and TIFF, so thumbnails show there. Where a
  web view can't draw one (HEIC or TIFF on Windows and Linux), the thumbnail shows the
  file's name instead of a broken picture (APP.md, Security). The path is kept either way.

## 105. Leave one element off the board (2026-10-12)

Behavior note 21 (ELM-22): the reference app's element editor has "exclude from board" as
a checkbox on the element itself; ours had it only as a category setting (decisions 28
and 62).

- Each element now has its own `exclude_from_board` (FORMAT.md, Element: additive, left
  out when off, so older files read and write back byte for byte). The category setting
  stays: an element is off the board when either is on (`Schedule::excluded`,
  `Schedule::element_excluded`).
- Off the board means what it meant for a category, everywhere the category setting
  counts: no ID in the strip's cast column or the one-line (`reports::cast_ids`), not in
  the strip's element list or element count, never a conflict (dates or events,
  `conflicts::conflicts`), and no conflict marks on its day out of days row (the row
  stays). Red flags still clash (E-E12): a flag is set on purpose.
- The Elements tab's element editor has **Leave off the board** beside Lock board ID
  (`set_element_field`, `exclude_from_board`; one undo step). MCP `set_element_details`
  takes `exclude_from_board`, and it and `list_elements` return the element's own setting
  and `excluded` (its own or its category's). Merge and diff treat it as a field of the
  element.

## 106. The reference app's menu keys (2026-10-12)

Behavior note 21 (E-K1, KEY-01, KEY-02) lists the reference app's keys for the File, Edit
and Schedule menus.

- **Already ours**: New ⌘N, Open ⌘O, Save ⌘S, Save As ⇧⌘S, Close ⌘W (closes the window,
  asking about unsaved changes, as before), Undo, Redo, Cut, Copy, Paste and Select All.
  Its Open Shared has no counterpart here.
- **Taken from it**: Board ⌘U (was ⌘1), Breakdown ⌘. (was ⌘2) and Calendar ⇧⌘D (was
  ⌘5), in View, from the one table (`ui/js/shortcuts.js`) that the menu (`src/menu.rs`),
  the key handler and the shortcut sheet share. Insert Day Break Above, which had ⇧⌘D,
  moves to ⌥⌘D; Below keeps ⌘D (the reference app's Cmd comma is the Mac's Settings key,
  entry 78).
- **Kept ours**: its Reports key is ⌘P, which here is Print, the platform convention
  every Mac and PC user expects, so Print keeps ⌘P and Reports keeps ⌘6. Elements, Day
  out of days, Conflicts and Red flags have no key there and keep ⌘3, ⌘4, ⌘7 and ⌘8.
  Its Design and View menus have no keys, and advanced find and replace has none; ours
  keep Find's keys (⌘F, ⇧⌘F, ⌘G, ⇧⌘G).
- `menu::tests::the_reference_keys_are_here` pins these keys.

## 107. Fixes from our real window: error lines, cascading windows, date hints (2026-10-12)

Behavior note 22 found four things in our app's real window.

- **Error lines keep their room.** A refused date on the Calendar put a line above its
  controls and pushed them down, so the next click landed on the wrong control, as the
  status line did before decision 48. Every error line the page makes (`errorLine` in
  `ui/js/modal.js`, and the Breakdown and Find panels' own, through `keepRoom`) now keeps
  its one line of room while empty (`.err-line`: hidden is invisible, not gone), one line
  high whether it holds a sentence or not. A long sentence ends in an ellipsis and its
  whole text is the line's title, as on the status line. Screen readers still hear it as
  an alert.
- **New windows cascade.** New Window and Open in New Window put the new window 26 points
  below and right of the window they open from (`windows::cascade`), and back at the
  screen's top left, one step in, when that would run past the screen's right or bottom
  edge. A window opened from the system (a file double-clicked) keeps the platform's
  place.
- **Date hints follow the date format.** An empty date field hinted `M/D/YYYY` while the
  app shows dates as `2027-03-01`. Every date field now hints the form it shows dates back
  in, from Settings: `2027-03-01` (ISO and long) or `3/1/2027` (US), and fields already
  drawn follow a change at once. Typing takes both forms, as before (decision 49).
- **The day timeline's sun line** said "No scene location is on the map". It now says
  "No location has coordinates, so sunrise and sunset are not known".

## 108. A text color per I/E and time of day pair (2026-10-12)

Behavior note 22 (BRD-13; settles the rest of E-S25): the reference app's strip color
table gives every I/E and time of day pair its own strip color and its own text color.
Ours gave each pair a strip color and set text color per strip type (decision 60).

- **Engine**: `StripColorRule::text` (optional). `StripColors::text_of(sheet)`: the
  first matching pair's own text color, else the scene text color, else none (black or
  white, whichever reads). So the pair's text color wins when set; the per-type text
  colors stay for banners and day breaks, and for scene strips whose pair has none.
  `rule_for` is the first matching rule. The colors' check covers text colors too.
- **File** (additive): a color rule's `text`, `#rrggbb`, left out when none; files
  without it read and write back byte for byte. Merge and diff treat the strip colors as
  one value, as before (`EXT / NIGHT #1f3524 text #ffeeaa`).
- **App**: each pair's row in Strip Colors has a Text switch and its color beside the
  strip color. Turning it on keeps the pair's strip color too (its usual color unless
  changed), since a rule always has a strip color; Usual on the row clears both.
  `set_strip_colors` takes a rule's `text` (empty or none: no own text color).

## 109. Event colors and post-production kinds (2026-10-12)

Behavior note 22 (EVT-01 to EVT-03): the reference app's event dialog has an optional
name, from and to dates, a type, a color from ten and any number of elements, and its
types include a list of post-production kinds that never count in the day out of days.

- **Already ours**: the optional name and any number of elements per event (`Event`
  held a list of elements from the start; decision 21). Nothing changes there; the
  editor's Name now says it is optional.
- **Post-production kinds** (`EventKind::Post(PostKind)`): our own list of ten generic
  kinds, post-production, visual effects, dubbing, sync sound, music recording, picture
  editing, sound editing, sound mixing, titles and graphics, trailer, keyed
  `post_production`, `visual_effects`, `dubbing`, `sync_sound`, `music_recording`,
  `picture_editing`, `sound_editing`, `sound_mixing`, `titles_graphics`, `trailer`. Like
  unavailable and camera tests they have no day out of days code: shown on the calendar,
  counted nowhere, never a conflict (EVT-07). The reference app lists "a few more"; we
  stop at these ten and keep `other` with a label for anything else.
- **Colors**: `Event::color`, an index into `EVENT_COLORS`, ten colors of our own (red,
  orange, yellow, green, teal, blue, indigo, purple, pink, brown), or none (the kind's
  usual look). In the file a key (`"color": "teal"`), left out when none, so older files
  read and write back byte for byte; an unknown key is refused. Diff mentions a set
  color (`fitting "Wardrobe" on 2027-03-08, teal`); merge treats an event as one entry,
  as before.
- **App**: the event editor lists every kind (from the view's `event_kinds`, so the page
  has no list of its own to drift) and a row of ten swatches plus None (`event_colors`).
  A colored event edges its line on the date and its badge in the lists with its color
  (`ev-c-*` classes in `styles.css`; colors never come from schedule text).
- **MCP**: `add_event` takes the post-production kinds and `color`; the calendar's
  events carry `color` and `counts`.

## 110. Breakdown beside images (2026-10-12)

Behavior note 22 (SPL-01 to SPL-07): the reference app's split menu has a view ours
lacked, the breakdown beside images.

- View > **Images Beside Breakdown** shows the images pane (decision 80's, the same
  panel) beside the Breakdown tab: the sheet the tab shows, its images and its elements'.
  It follows the sheet as the tab moves between sheets (Previous, Next, Go to Scene).
  Choosing it from another tab opens the Breakdown tab first; choosing it again, or
  Single View (⇧⌘0) in the pane's head, closes it.
- The pane belongs to the tab shown: the Board tab keeps its own pick (breakdown, board,
  calendar or images), and the Breakdown tab's images are remembered apart, in the same
  per-viewer browser storage (`bdImages`), never in the schedule. The pane's head says
  Images beside the breakdown there.
- Not copied: the reference app reflows the breakdown into fewer columns when halved;
  ours already wraps its rows and category boxes to the room it has.

## 111. The Calendar's Actions menu, View switches and weeks view (2026-10-12)

Behavior note 22 (CAL-04 to CAL-07): the reference app's calendar is one long view of
weeks that scrolls through the schedule, holidays carry a star and company travel days a
plane, an Actions menu acts on the selected date, and a View menu turns what each date
shows on and off.

- **Actions** for the picked date: Working Day, Day Off, Company Travel Day, Holiday
  (`set_day_kind`), Add Event… (the event editor on that date), Add Red Flag… (pick the
  element, then the red flag dialog with that date as its range; `set_red_flag`) and Set
  as Start Date (`set_start_date`). From an Actions… button beside the picked date's
  title and from a right-click on any date (the page-built menu of decision 53). The
  kind the date already is, and the start date when it is the start, show as `(now)`
  and are off. Each is one undo step.
- **View switches** (`Schedule::calendar_hidden`, `CalendarShow`): shoot day and day off
  labels, page counts, estimated time, scenes per day, events, strips, banners and red
  flags, all on by default, with All and None. Stored per schedule beside the calendar
  strip fields (decision 102), not a user preference, since the reference app keeps its
  calendar's design in the schedule. File: `calendar_hidden`, the keys turned off, in
  their order, left out while empty. Merge treats it as one value; diff says
  `Calendar view everything shown -> hides pages, strips`. `set_calendar_hidden` is one
  undo step. The page draws every date from the full view and leaves out what is
  hidden. "And more" in the reference app's menu is not modeled; the Calendar report
  (Print, Save as PDF) keeps its own options and does not follow these switches.
- **Banners and scene counts on a date**: a shoot day now carries its banners (by their
  text, drawn above its strips) and how many scenes it holds, for the switches above.
- **The weeks view** (`CalOpts::span`, `calendar_view`'s `span`): every week from the
  earliest date the sub-board uses (the start, the first shoot day, the first event, the
  month last shown) to the latest (the last shoot day, the last event, that month's
  end), at least six and at most 110 weeks, all drawn as in the month (no dates greyed).
  Its title names the first and last months. It is an option beside the month view; the
  month stays the default, as switching every user to a long list would change what
  they see on opening. Which view is a per-viewer convenience kept in this browser
  (`method-schedule.calendar.weeks`), not the schedule.
- **Marks**: a holiday shows a star, a company travel day a plane, beside the date's
  number in both views (with words for screen readers), whatever the switches.


## 112. Fixes from our real window: date hints as forms, tight weeks cells (2026-10-12)

Behavior note 23 found two things in our app's real window.

- **Date hints are forms, not dates.** Decision 107 made an empty date field hint a
  sample date in the Settings format (`2027-03-01`), which reads like a value already
  there (prep starts, wrap ends). The hint is now the form written as letters:
  `YYYY-MM-DD` (ISO and long, since a field shows the long format as ISO) or `M/D/YYYY`
  (US), and fields already drawn still follow a change in Settings at once. Typing takes
  both forms, as before (decision 49); sentences that explain typing keep their examples.
- **Weeks view cells keep no blank line.** In the weeks view (decision 111) a date's
  button kept a minimum height, so its strips sat a blank line below the day label. The
  table cell now holds the height and the button is as tall as its lines, so the strips
  follow the label at once. The month view is unchanged.

## 113. Red flag types and event kinds as one word list (2026-10-12)

Behavior note 23 (E-E14 answered; RFL-01): in the reference app red flag types and event
types are one list with built-in entries, travel, fitting and rehearsal (marked as
counting in the day out of days), unavailable, camera test and the post-production kinds.
Ours started with no red flag types (decisions 33 and 77).

- **Built-in types**: `RedFlagType::builtin`, the event kind a standard type is the word
  for, never `other`. `EventKind::builtin_flag_kinds` is the list, in the event kinds'
  order: travel, fitting, rehearsal, unavailable, camera test and the ten
  post-production kinds of decision 109, so the two lists can't drift apart. Each is
  named as the kind is (`Camera test`). Standard types can be renamed, recolored and
  moved but never deleted (`Error::BuiltInType`); `check` refuses two types built in as
  one kind.
- **Who gets them**: new schedules (`Schedule::with_default_categories`, which the app,
  the command line, MCP and script imports into a new file use) and templates
  (`to_template`, so every schedule started from one too). `Schedule::new` and every
  file read keep their own list: a file without the key reads and writes back byte for
  byte. `standard_red_flag_types` adds the missing ones at the end of the list; a type of
  the schedule's own already called so (ignoring case) becomes that standard type,
  keeping its id, name, color and flags, so no name is ever doubled. The app's Red Flag
  Types dialog shows **Add the Standard Types** while some are missing (one undo step);
  MCP has `add_standard_red_flag_types`.
- **A type and the event kind of the same word stay apart in the model.** We read the
  reference app's one list as one word list, not one thing: an event books an element on
  dates (travel, a fitting) and the counted kinds put their code on the day out of days,
  while a red flag only warns that an element can't, or would rather not, work then. Our
  events and flags already have their own data, rules, file keys, merge and diff;
  merging them would rewrite the format and every reader, for no gain a user sees. So
  the standard types share the event kinds' words, and that is all: a Travel flag never
  counts and never makes an event. The dialog says which standard types match a counting
  kind (an event of that kind shows TR, FT or R). The reference app may let a flag of a
  counting type count; we keep counting to events and say so (engine check E-E14 row).
- **File** (additive): a type's `builtin`, the event kind key, left out for the
  schedule's own types; an unknown key or `other` is refused. Diff says `built in no ->
  unavailable`; merge takes it field by field like the name and color.

## 114. The Red flags tab: several elements and dates at once, every flag in a table (2026-10-12)

Behavior note 23 (RFL-01 to RFL-03): the reference app's Red Flags tab is one screen in
three parts, a list of elements with a category picker and check boxes, a small month
calendar picking several dates with a count, and the type, a note and Add; below, a
table of every flag (start, end, category, element, type, note) with delete for the
checked rows, an element filter, export and print. Decisions 33, 36 and 59 had built the
flags themselves, one flag at a time from the element editor, the clashes list on the tab
and the red flag report of clashes. What was missing is added here.

- **Adding several at once** (`Schedule::add_red_flags`, `add_red_flags` in the app and
  over MCP): one flag of one type for each element and each run of dates, with one note
  and firmness, all or nothing, one undo step. Dates are picked one by one on the small
  calendar (`redflags::runs` joins following dates into one range, so 2 to 4 March is one
  flag, not three), or typed as a range, not before or not after. The calendar starts on
  the first picked date's month, else this month, and follows the week start in
  Settings. The picks, the category and the table's checks and filter are kept while the
  window is open, never in the schedule.
- **Every red flag**: a table on the tab, by start date (open starts first), then the
  element's place, then the flag's: start and end as the dates the flag rules out (a not
  before date shows Open to the day before it, a not after date the day after it to
  Open), category, element (a link to the Elements tab), type with its color, hard or
  soft, and note. An Element filter lists the flagged elements. **Delete Checked**
  (`Schedule::delete_red_flags`, `delete_red_flags`) asks with a second click and deletes
  the checked rows in one undo step. At most 1000 rows are listed; the count says so.
- **Export and print**: Export… writes the table (`export::red_flags`, every flag, as
  File > Export's new Red flags choice). Print… and Save as PDF… print the Red flags
  report, which now holds the table of every flag (filtered by its category, element and
  date range, a flag showing when its dates meet the range) before the clashes. The
  report keeps its own filters, as the Calendar report does (decision 111); the tab's
  element filter is for the screen.
- **Left as it was**: the clashes list stays on the tab below the table, and each
  element's flags stay in its editor with Add, Edit and Delete.

## 115. The vertical one-line as the stripboard on its side (2026-10-12)

Behavior note 23 (RPT-10; E-R9 answered): the reference app prints the vertical one-line
as the stripboard turned sideways, each strip a column with its text turned, day breaks
as black columns carrying the day's total pages and time, and no cover page: the left of
the same page holds the producer, director, assistant director and script date, then the
cast as rows in board ID order. Ours (decision 59) printed a cover list of the cast, then
pages of scene columns headed by the scene number alone, the day over its first scene.

- **Columns**: each scene's heading is its scene number, I/E, set, time of day and pages,
  turned to read upward (`v-turn`, print.css), in a tall heading row. After each day's
  last scene comes a dark column (`v-break`) whose turned heading says the day, its date
  and its total pages and time; its cells stay dark down the cast rows. The day line over
  the first scene is gone, as the break columns mark the days.
- **The left of every page**: the production fields above, those filled in, as label and
  value over the word Cast, then a row per cast member (board ID and name) in board ID
  order, no ID last, as before. The cover page is gone; a long cast runs onto more pages
  with the heading repeated, as every report table does.
- **Data**: `report_data::VerticalDay::time` (the day's shoot time with banners); the
  app's vertical columns carry the day's totals on its last scene (`end`) and the report
  its `production` lines; MCP's `report` gives each day's `eighths` and `minutes`. The
  command line's text grid is unchanged (it already marks each day's end with `|`).
- Not copied: the reference app's exact column widths and fonts; ours fits 18 columns to
  a portrait page and 30 to a landscape one, scenes and breaks alike.

## 116. Linked elements chain through every level (2026-10-12)

Behavior note 25 (ELM-20; E-L2 answered): in the reference app links chain. With an actor
linked to a prop and that prop linked to a second prop, adding the actor to a sheet added
both props, and one undo took all three off. Ours (decision 62) added the element's own
links only, one level.

- **The chain**: `Schedule::link_chain` lists the element and every element its links
  reach, depth first in link order, each once. An element already met stops the walk, so
  a cycle (A to B, B to A, or longer) can be set and never loops or adds twice.
  `tag_with_links` puts the whole chain on the sheet, so every add follows it as before:
  the Breakdown tab's quick add and boxes, Paste, script revisions, MCP `tag_element` and
  `add_element`. It is still one edit, so one undo step.
- **Retroactive**: Also add them to every sheet that already has it now adds the whole
  chain of the new list to those sheets, as adding the element would.
- **As before**: links are one way (note 25 saw the same), removing a link takes nothing
  off a sheet, and without the retroactive box nothing already on a sheet changes, which
  note 25 confirmed.

## 117. The day out of days summary has no Holiday count (2026-10-12)

Behavior note 25 (E-D20, answered again after note 17 A9): a holiday inside a held span
counts in Work, and the reference app's summary Holiday column stays empty; it never
fills it. Ours (decision 11) shows Work, Hold, Start, Finish and Total, with the counted
holiday already in Work.

- **Chosen: drop it.** Our summary keeps no holiday count and no Holiday column, on the
  DOOD tab, in the reports, the spreadsheet exports, the command line and MCP. A column
  that is always empty would tell a reader nothing, and the numbers a reader sees are the
  reference app's: a counted holiday sits in Work and in Total (Work plus Hold). A test
  pins it (`dood::tests::holidays`).
- **The budget exchange does not change**: its counts are kept apart on purpose
  (EXCHANGE.md), so its `holiday` field, the holidays that count, stays filled and its
  `total` stays the sum of the seven counts, equal to the summary's Total.
- Not copied: the reference app's empty column.

## 118. Every header field works as a category (2026-10-12)

Behavior note 24 (ELM-09): the reference app's built-in list shows every header field
with one value per sheet: time of day, episode, I/E, location, script day, sequence, set
and unit, each value with occurrences, pages, days, start and finish, unused pick list
values included. Ours (decision 62) covered set, location, unit and episode.

- **The fields**: `reports::HeaderField` gains I/E (`int_ext`), time of day
  (`time_of_day`), script day (`script_day`) and sequence (`sequence`), keyed by the
  sheet's own field names, after the first four. Each works as before: a copy of the
  schedule where each value is an element of one more category, values matched ignoring
  case and spelled as first met (E-L12, confirmed by note 25), no holds or drops. So the
  Elements tab's Header fields list, the DOOD tab's picker and MCP `dood` `header` all
  offer the eight.
- **Unused values**: ours keeps no pick list of its own. The values the Breakdown tab
  offers for I/E and time of day are the ones the sheets already use, so there is no
  unused value to list; a stored pick list, if one comes, should list its unused values
  with no sheets.
- Left out: scenes, sheet number, synopsis, comments, pages and times, which are not one
  value shared across sheets.

## 119. The cast starts with properties; a property takes one line or several (2026-10-12)

Behavior note 24 (ELM-07, ELM-12, ELM-13): the reference app's cast category comes with
properties for contact and deal details, and each property has a name and a type, a
single line or several lines. Ours (decision 62) started every category with none and
took one line per value.

- **Starter set**: a new schedule (`Schedule::with_default_categories`, so the app, the
  command line and MCP alike) gives the cast eight properties, in our own words
  (`model::CAST_PROPERTIES`): Full name, Address (several lines), Phone, Agent, Agent
  phone, Rate, Rate per and Deal notes (several lines; the element's own Notes stay apart).
  They are ordinary properties: rename, move or delete them. `Schedule::new` stays bare,
  and an existing file is never given them: it reads and writes back as it was.
- **Types**: `Category::multi_line` lists the properties whose values take several lines,
  by name, in the properties' order; the rest take one line. Renames, moves and deletes
  carry it; `Schedule::check` refuses a name that isn't one of the properties or is out
  of order; a merge keeps both sides' marks on the properties it keeps. In the file it is
  the category's `multi_line` key, left out when empty (FORMAT.md), so files from before
  read and write back byte for byte.
- **The app**: each property in the Elements tab's Custom properties row has a One line
  or Several lines choice (`set_property_multi_line`, one undo step); an element's value
  of a property of several lines is a text area (Enter adds a line), the others stay one
  line fields. A value keeps its line breaks only in a property of several lines; turning
  a property back to one line leaves values as they are.
- **MCP**: `list_elements` gives each category's `multi_line`; `set_category` takes
  `multi_line`, the whole list by name.
- Not copied: the reference app's property names and drag to reorder (ours moves with
  the arrow buttons, as before).

## 120. Left off the board: the element decides, the category gives the default (2026-10-12)

Behavior note 24: a category's settings in the reference app (automatic board IDs, exclude
from board, allow hold days, use in daily quantity) are defaults for new elements, and
exclude from board is an element setting (note 21). Ours (decisions 28, 62 and 105) left
an element off when its own switch or its category's was on, so the category's switch
reached every element at once and an element of such a category could not be put back.

- **The rule now**: `Schedule::element_excluded` is the element's own
  `exclude_from_board`, and nothing else. A new element takes its category's setting
  (`add_element`, so every way an element is made: the app, a script import, the command
  line and MCP). Turning the category's switch on or off later changes no existing
  element; **Apply to Existing Elements** (`apply_defaults`, our extra, ELM-08) now gives
  them the category's off the board setting along with its hold and drop rules. Conflicts,
  the strips' cast column, the one-line and the day out of days' conflict marks all ask
  the element.
- **Old files keep their meaning, and their bytes.** In the file an element's
  `exclude_from_board` is now left out when it matches its category's, and a missing key
  means the category's setting (FORMAT.md). A file from before, where a category's switch
  on meant every element off, reads with each of those elements off, and writes back byte
  for byte, as no element key is needed. An element put back on the board in such a
  category writes `false`. The one older shape that changes on its next save is an element
  that had its own `true` inside a category already off: the key drops, the meaning stays.
  We chose this over migrating by writing a key on every element (a changed file for
  nothing) and over keeping both rules (the category's switch would still reach every
  element, which the reference app does not do).
- **The app**: the category's switch sits with the other defaults (Defaults for new
  elements: Off the board, Holds, Drops, minimum gap); the element's own box is the one
  that decides. The DOOD view's `excluded` flag now means every element of the category is
  off. MCP `set_category`'s `exclude_from_board` is the default; `set_element_details`
  returns `excluded` equal to the element's own setting.
- Left as before: automatic board IDs and use in daily quantity stay category settings
  (an element has no switch of its own for them); a new background element's use in daily
  quantity on a sheet stays on (note 25).

## 121. The DOOD shows or hides weekends, per viewer (2026-10-12)

Behavior note 24 (DOD-10): the reference app's day out of days View menu hides or shows
Saturdays and Sundays. Ours always drew every date from the first to the last.

- **Weekends**: a box in the DOOD tab's head beside Without holds and Vertical, on by
  default. Off, the Saturday and Sunday columns (rows in the vertical view) are hidden;
  nothing is worked out again, so the counts and the summary stay as they are, a held or
  worked weekend day included. The cells carry `g-we` and the table `no-weekends`
  (styles.css), so the virtual rows past 80 hide them the same way.
- **Per viewer**: remembered in this browser's storage (`method-schedule.dood.weekends`),
  as Hide empty on the Breakdown tab is; not schedule data and not in the file. It reads
  back as on when storage is off.
- **Not changed**: the DOOD report and the spreadsheets print every date; a weekend
  choice for them can follow if a check in the real app shows its report follows the
  View menu.
- Ours is a box on the tab rather than an item in a View menu, as Without holds and
  Vertical already are.

## 122. Board sort takes up to four rules (2026-10-12)

Behavior note 24 (BRD-09): the reference app's board sort is a list of rules, each a
field, at most four, with no ascending or descending choice. Ours (decision 32) took up
to three.

- **Four**: `board::MAX_SORT_KEYS` is 4; the Sort Strips dialog has a fourth Then by
  row; a fifth is refused with a sentence, as a third too many was before.
- **Kept as ours**: each rule still has Ascending or Descending (ascending is what the
  reference app does, so its sorts are all reachable); empty fields last, ties in board
  order, banners to the top and day breaks removed (decision 32, E-S8), unchanged.

## 123. A new sub-board starts today (2026-10-12)

Behavior note 24 (BRD-02): in the reference app a new sub-board's start date is today.
Ours left it empty, so its days had no dates until one was typed.

- **The app**: New Sub-board gives the sub-board today's date by the computer's clock
  (`exchange::local_today`, the date the budget export uses), as one undo step with the
  add. `board::add_sub_board` takes the start date; the engine's `add_sub_board` already
  did. A duplicated board's sub-boards keep their own dates, and a new schedule's first
  sub-board still has none, as before (note 24 says nothing of either).
- The harness fake starts a new sub-board today too.

## 124. Conflicts count elements, not header values: a deliberate difference (2026-10-12)

Behavior note 24 (EVT-05): with one scene moved to a second sub-board starting the same
day, the reference app's toolbar count and its list of conflicts and red flags named
every element on both strips that day, header values included: the actor and the prop,
and also the I/E value and the time of day value, each linked to the strip in each
sub-board. Ours (decisions 28 and 34) counts real elements only.

- **Kept as ours.** I/E, time of day, set and the other header values are not people or
  things that can be in two places at once: two units shooting INT or NIGHT on the same day
  is the normal case, not a clash. Listing them would bury the real conflicts (an actor
  booked twice) under noise on every two-unit day. So conflicts stay element only, in the
  engine, the Conflicts tab, the strip and calendar marks, the DOOD and MCP.
- The header fields still work as categories in the Elements tab and the DOOD (decision
  118), so a reader can see which sets or locations two sub-boards share on a date there.
- Recorded as a deliberate difference; the parity row stays have, the note says so.

## 125. Delete Sheet still asks; Remove All Day Breaks does not (2026-10-12)

Behavior note 24 (BRK-08, DBK-09; E-B14 answered): the reference app deletes a sheet at
once, with no question, shows a short notice that it was deleted, and Undo brings it
back. Remove All Day Breaks does not ask either; the shoot day headers go with the breaks
and Undo restores them.

- **Delete Sheet keeps its question** (decisions 17 and 54). A sheet holds the breakdown
  someone typed in, and it leaves every board at once; one stray click should not lose it
  even for a moment, and a notice is easy to miss. Undo still brings it back after the
  question. A deliberate difference.
- **Remove All Day Breaks asks nothing**, as the reference app (decision 56 already had
  it so): the strips stay, the days can be broken again, and Undo puts the breaks back.
  Our shoot day strips (decision 67) are drawn from the breaks, so they go and come back
  with them.

## 126. New Banner keeps its dialog: a deliberate difference (2026-10-12)

Behavior note 24 (BRD-11): the reference app's New Banner edits the banner in place on
the board, above the selected strip, with a text box and a duration in hours and
minutes; a style panel beside it holds bold and italic, a font size, a banner size,
alignment, a duration switch and a palette of about fifty colors. Ours (decisions 30, 44,
65 and 87) opens a dialog with the same fields (text, duration, bold, a color from nine,
one or two lines high, text size and alignment) and puts the banner above the selected
strip.

- **Kept as ours for now.** The dialog's fields cover what the in-place editor sets, in
  one place, and an editor inside the windowed board (decision 87) would need its own
  focus, scroll and undo handling for little gain. Where the banner lands is the same.
- Not matched: italic, the font size steps beyond our three, the duration switch and the
  larger palette. If the in-place editor is wanted later, it can reuse the dialog's fields
  as a row on the board.

## 127. Fixes from our real window: the vertical one-line's day columns, the red flag calendar (2026-10-12)

Behavior note 26 (our app in its real window):

- **Vertical one-line** (RPT-10, decision 115): a dark day column's turned heading was
  one long line (the day, its full date, pages and time) and was cut off. It is now three
  short turned lines side by side, End of day N, the date, and pages and time
  (`6/8 pg, 3:00`), at a slightly smaller size, and the day columns are 40 px wide where
  the scene columns share the rest (print.css), so the whole heading shows.
- **Red flags tab** (RFL-05, decision 114): the small calendar opened on today's month.
  It now opens on the month of the first picked date, else the sub-board's start month
  (the tab's view gives `start`), else today's, and keeps the month it was stepped to.

## 128. Review of the day out of days and the reports: fixes and plain readings (2026-10-13)

A review of every DOOD and report row (DOD-01 to DOD-19, RPT-01 to RPT-20) against
behavior notes 14 to 17 and the decisions they follow. Each fix came with a failing test.

- **A drop that starts or ends on an event day** (E-D22, decision 20 said "the event's
  code, no D or P"): the event day showed `FTH`, as if held. Without the event that day
  sits inside the drop, so it is not held: it now shows the event's code alone (`FT`),
  counted as worked. `dood.rs` treats the drop's two ends as not held.
- **Omitted scenes reach the page** (decision 91): the board's strips and the reports'
  scene lines never carried the omitted mark the page draws struck. They do now
  (`omitted`, left out when false), and the command line's `oneline` and `shooting`
  mark such a scene `(omitted)` and MCP `one_line` gives it `omitted: true`.
- **Left off the board, everywhere the cast IDs show** (decision 120, our plain reading
  for two places it did not name): the vertical one-line's cast grid gives an element
  left off the board no row, as it has no ID on the strips; and a report with Cast names
  on leaves it out as the ID column already did. Not checked in the real app.
- **The command line's DOOD grid export** (`dood FILE --grid --sort ... --csv | --xlsx`)
  ignored `--sort` and `--no-holds`; it now takes them as the text does
  (`export::dood_grid_of`). A one-category grid's dates are that category's rows', as the
  app's grid is.
- **The summary a category at a time** (DOD-01): the command line's `dood` text and the
  summary spreadsheet listed every category mixed in board ID order (a prop with ID 1 sat
  between cast 1 and cast 2). Rows now go in category order, the engine's order kept
  within each (`dood::group_by_category`); the text puts each category's name above its
  rows, as `elements` does. The app's grid and report were already one category at a time.
- **The DOOD tab's Total heading** sorted by work, so rows with the same work and
  different holds came out in the wrong order with the Total heading marked. Total is no
  sort key (the reference app's six, DOD-12 and DOD-18), so its heading no longer sorts;
  a test checks each heading's key against the engine's.
- **A deleted header layout's id is not handed out again** (RPT-18): a report keeps the
  pick of a deleted layout and prints with the first; a new layout used to take the
  deleted one's id when it was the highest, and the report then printed with the new one
  unasked. `add_header` now goes past every id a report still picks.
- **The calendar report past two years** (RPT-07) printed the first 24 months without a
  word; its date note now says the first 24 months print and a narrower range shows the
  rest.
- **Dates out of range**: a DOOD or conflicts report whose date range misses every row
  now says the range is why, rather than that there is nothing.
- **MCP `dood`**: its codes stay in the usual letters on purpose (a client reads them one
  way); the schedule's letters are in `report`'s `dood_grid`. MCP.md says so, and that a
  conflict mark follows the element's own off the board setting (decision 120).

## 129. Move To puts the strips at the top (2026-10-13)

Behavior note 24 (BRD-07; E-S2 answered): the reference app's Move To puts the strip at
the top of the sub-board it goes to, and the selection moves to the next strip where the
strip was. Ours put the strips at the end (E-S2's proposed rule).

- **Move To now puts the strips at the top** of the sub-board picked, in their board
  order, and selects the strip that followed them where they were (the one before them
  when they were last in their list; the moved strips when nothing is left there).
- **The Boneyard takes them at the top too.** The note saw a sub-board only; the plain
  reading is that Move To works one way whatever the target. Kept open as E-S2 for the
  Boneyard. New strips (a new sheet, a script import) still land at the end of the
  Boneyard (decision 17); that is a different command.
- Dragging, Option-Up and Down and paste are unchanged: they put strips where they are
  dropped or pasted.

## 130. Find marks its results on the board (2026-10-13)

Behavior note 24 (BRD-12): the reference app's board find highlights every matching strip
and has a select all beside previous and next. Our Find panel (decision 34) stepped
through results but marked nothing on the board.

- **While the Find panel is open, every strip whose scene or banner is in the results is
  outlined** on the board shown (a `found` class, an amber outline that the selection's
  left bar doesn't hide). The marks follow the results: a new search, an edit that drops a
  result, another board shown or closing the panel updates or clears them.
- **Select on Board** in the panel selects all those strips on the Board tab (focus on
  the first), so a found set can be moved, cut or printed as a selection. With no strip
  of the board shown in the results it says so and changes nothing.
- A page change only; no new command.

## 131. Sorting strips from the command line and MCP (2026-10-13)

The board's sort (decisions 32 and 122) was in the app only. It is a data edit like
automatic day breaks, so the command line and the MCP server get it too:
`method-schedule sort FILE FIELD[:desc],...` on the first sub-board, and the
`sort_strips` tool on any sub-board. Both take one to four fields, each once, from the
same list as the Sort dialog, and refuse the rest without writing. The limit of four now
lives in the engine (`model::MAX_SORT_KEYS`), which the app uses too. The command line's
`breaks` also refuses a limit of 0 now, as the app and the MCP server already did.

## 132. The command line picks a board, and a template is a .mtst (2026-10-13)

Feature review, files to beyond: most commands worked on the first board, while the MCP
tools take `board` and `sub_board`; and `template` wrote a template under any name, a
`.mtsc` too, where the app and MCP keep `.mtst` (decision 85).

- **Every command that works on one board takes `--board NAME`**: the board's name, any
  case, or its id. Without it, the first board, as before. An unknown name, a name two
  boards share (give the id), a missing value or the flag twice is refused. `conflicts`,
  `redflags` and `rest` took `--board ID` already; they take a name too now. The commands
  then work on that board's first sub-board, or the one `--sub-board` names where a
  command takes it; `redflags` refuses a `--sub-board` of another board.
- **`template FILE OUT` refuses an OUT not ending in `.mtst`** (any case), writing
  nothing, so a template never passes for a schedule.

## 133. The reader refuses what it would not write back (2026-10-13)

Feature review, calendar and file format: FORMAT.md's rule refuses a key written at its
default, but the reader held only some keys to it. It took `null` for any optional key (an
event's `color`, a red flag type's `color` and `builtin`, `prep_start`, `wrap_end` and
more) and `false`, `[]` or `""` for most keys added since the first version, then wrote
the file back without them. It also took an event longer than the 366 days every edit
allows (`MAX_EVENT_DAYS`, decision 21).

- **One rule for the whole file**: after reading, the reader writes what it read and
  compares it with the file's JSON, key by key (key order aside, which the writer fixes).
  A key given as `null` is refused ("... is null; leave the key out"), a key the writer
  would leave out is refused as an explicit default, and a value the writer spells
  another way is refused with what it would write. The message names the key's place
  (`schedule.elements[3].images`). The checks with their own messages run first.
- **Layout files follow the same rule.**
- **Old files still read**: every file the app wrote is as the writer writes it, so the
  rule changes nothing for them. Keys kept for older files (`page_per_day`, an element's
  `exclude_from_board` beside its category's, a column's `second_line`) are still how
  the writer writes them. Checked on the test schedules in the owner's test folder.
- **An event longer than 366 days is refused** on reading, as every edit refuses it.
- A test builds a schedule with every documented key set (it checks each key FORMAT.md
  lists is in the file), then tries every key in it as `null`, as the empty value of its
  type and left out: `null` is always refused, and whatever reads writes back the same.

## 134. Sorting and breaking keep a scene's parts apart (2026-10-13)

Feature review, boards and day breaks (DBK-11, decision 76): sorting a sub-board put a
scene's parts side by side, or let another scene with the same sort values sit between
them, and the automatic day breaks then could put two parts of one scene in one day.

- **Sort moves a scene's parts as one block**, in part order, where the scene's first
  part sorts. Banners still go to the top and day breaks are still removed (decision
  122).
- **The automatic day breaks never put two parts of one scene in one day**: a second
  part closes the open day before it, whatever the limit. A later part still adds no
  length (decision 76), so the day it opens counts from the next strip.
- Drags, Move To and Continue on Next Day are unchanged: the user can still place two
  parts in one day by hand; `days` counts the length once either way.

## 135. The weeks view is anchored on the shoot (2026-10-13)

Feature review, calendar (CAL-14, decision 111): the weeks view ran from the earliest date
the schedule used to the latest, events included, so an event two or more years from the
shoot (a mistyped year) moved the first week there, and the 110-week cap could then leave
the shoot days out.

- **The span is the shoot's** (the start date to the last shoot day) **and the month
  asked for**, then widened by every event within 26 weeks of that, as far as 26 weeks.
  The month asked for is left out when it would push the whole past the cap, so the shoot
  days are always drawn. Without a start date the month is the span, as before.
- **Events farther away are listed, not drawn**: the Calendar's note says how many are
  outside this span (`outside_events` in `calendar_view`), and they stay in the Events
  list, where they can be opened and their dates fixed.
- The month view is unchanged.

## 136. The strip designer's mark switches and what an element count counts (2026-10-13)

Behavior note 25 (BRD-14, BRD-18): the reference app's strip designer has switches that
decide whether the red flag and conflict marks show on strips, and its element quantity
field asks which elements it counts. Ours always showed both marks, and the element count
counted every element of the categories on the board.

- **Two switches per strip layout, Red flag marks and Conflict marks** (on by default), in
  Board > Strip Layout beside the text size. Off hides that mark on the strips of every
  board using the layout, on screen and printed; the Red flags and Conflicts tabs, the
  calendar, the DOOD and the reports keep theirs. `StripLayout::hide_red_flags` and
  `hide_conflicts`; in the file a board's `strip_red_flags` and `strip_conflicts` and a
  named layout's `red_flags` and `conflicts`, each written only as `false` (FORMAT.md).
- **An element count picks its elements**: with Element count shown, the designer lists
  the elements by category to check; none checked counts every element, as before. The
  picks are element ids on the column (`StripColumn::elements`, the column's `elements`
  in the file), at most 1,000, each once, only on `element_count`. A picked element
  counts whether or not it is left off the board (it was picked by name). Deleting an
  element drops it from every pick; merging elements moves the pick to the survivor; a
  pick of a missing element is refused by every edit and by the reader.
- **Layout files leave the picks out**: they name one schedule's elements, so a layout
  file is written without them and refused with them, and loading one clears them.
- Reports keep their own columns (the report designer, decision 90): an element count
  there counts every element. Our plain readings are queued as E-S34.

## 137. The page's own rules run as tests, with Node and no npm (2026-10-13)

Feature review, page logic with no automated test (BRK-11, BRK-18, ELM-16, CAL-05,
CAL-07, KEY-03, KEY-04, KEY-05, SPL-01 to SPL-08, DOD-10, DOD-19), checked in the harness
only.

- **The rules move into `ui/js/logic.js`**: small exported functions with no imports and
  no DOM: typed dates, Quick add's `Category;Name`, Hide empty, the Elements table's
  sort, Go To and the Calendar's filter, the strip menu's items and when each is off, what
  Delete asks, the shortcut sheet's filter, the split's View items, the pane's width and
  remembered state, the DOOD's weekends and its vertical grid, and the red flag date
  picker (decision 138). The tabs call them; what they draw is unchanged.
- **`node --test crates/method-schedule-app/page-tests/logic.test.mjs`** runs them with
  Node's own test runner: no npm, no package file. The test loads `logic.js` from its text
  as a module, so it runs the same on any Node from 18 on, and it checks that the file
  stays free of imports and page globals. A `page` job in `ci.yml` runs it on the runner
  image's Node (no new action).
- What needs a page (focus, drawing, dialogs) stays checked in the harness.

## 138. Red flag dates are picked as the reference app picks them (2026-10-13)

Behavior note 25 (RFL-05, decision 114): on the reference app's small calendar a click
replaces the pick and Command-click adds. Ours toggled each date on a click.

- **A click picks that date alone; Command-click (Ctrl-click off the Mac) adds it, or
  removes it when picked; Shift-click picks every date from the last one clicked to this
  one**, added to the pick with Command. Clear empties it, as before. Runs of following
  dates still make one flag each (E-E16).
- Shift-click and removing with Command are our plain reading; queued as E-E17.

## 139. An event at a drop's edge takes the D or P (2026-10-13)

Behavior note 27 (E-D22, DOD-04). Decision 20 read a drop ending on an event day as "the
event's code, no D or P", and decision 128 took the H off too (`FT`). The real app: ADA,
drops at 7, worked March 1 and came back March 16. Travel on March 2 showed `TRHD` and
March 1 became `SW`; travel on March 15 showed `TRHP` and March 16 became `W`. Each
event day counted as worked.

- **A counted event (travel, fitting, rehearsal) bounds the drop as a work day would.**
  Gaps were already split at event days inside the span (decision 20); now the event day
  at either end of the drop takes the letter: the event's code, its H, then D or P
  (`TRHD`, `FTHP`). The work day beside it keeps no D or P. Days between the event and
  the work too few for a drop are held, as between two work days.
- `dood::Cell::Event` carries `drop` and `pickup`; custom letters follow (`code_with`),
  and the explain text names the day as the drop's bound.
- Supersedes decision 128's first bullet. The ZTEST grids of notes 14 to 18 (BRUNO
  `SW . . FTH H WF`, CLEO `TRH`, FINN `FT . . . SWF`) are unchanged.

## 140. A move into a category that numbers automatically gives no board ID (2026-10-13)

Behavior note 27 (E-L7, ELM-19). Decision 45 kept our reading that an element with no ID
moving into a category that numbers automatically gets the next number, as the real app's
behavior was not seen. It now is: with Props set to number automatically, FINN (no ID)
moved from the cast to Props kept no ID.

- `Schedule::move_element` no longer numbers the element; it keeps its ID and lock, or no
  ID. Numbering automatically is a default for new elements only, as the other category
  settings are (note 24, E-L9). The harness fake already did this.
- Replaces decision 45's second bullet; the refusal when the ID is taken stays.

## 141. The DOOD report follows the Weekends box and pages by whole weeks (2026-10-13)

Behavior note 27 (DOD-19, decision 121). Decision 121 left the DOOD report printing
every date until a check showed whether the reference app's report follows its View
menu. It does: with Saturdays and Sundays hidden, the report leaves them out too (Friday
then Monday); shown, they print as days off. And it pages by whole weeks from the week's
first day: a range from Monday March 1 started on Sunday February 28.

- **Whole weeks**: the report's grid starts on the week's first day (the user's Weeks
  start on, Sunday or Monday) on or before its first date, also before the report's
  range, with empty cells there (`GridOpts::week_start`, `view::week_first`). The pages
  hold as many whole weeks as fit in their 18 or 28 columns, at least one
  (`doodReportPages` in logic.js). The DOOD tab's grid still starts where the work does.
- **Weekends**: the DOOD tab's Weekends box (per viewer, decision 121) decides for the
  report too. Off, Saturday and Sunday columns are left out and a week is five columns;
  on, they print shaded as days off. Ours has one box for both days where the reference
  app has a switch each. Counts never change.
- **Not changed**: the spreadsheets, which ignore the report's options (decision 59; the
  reference app's Excel export ignored the report's range too, note 27), and the command
  line's reports, which have no viewer's switch and print every date. The spreadsheet
  code does not share the report's path.

## 142. The vertical one-line keeps a row for a cast member off the board (2026-10-13)

Behavior note 27 (RPT-20, decision 128). Decision 128 gave a cast member left off the
board no row in the vertical one-line's cast grid, our plain reading. The real app kept
it: with ADA set to exclude from board, the cast list still had ADA's row.

- `report_data::vertical` lists every cast member on a counted scene, left off the board
  or not, in board ID order (then name) with the others, each with its ID (if it has
  one) and name, and its marks in the scenes that list it.
- **Cast names on** (decision 128's other half) is a switch of the one-line, shooting
  schedule, breakdown and element detail reports, not of the vertical one-line, whose
  rows always show the ID and the name. Those reports' cast column follows the strips'
  cast IDs, which leave such an element out (decision 120, checked), so with Cast names
  on it still has no name there. A row that is listed shows its name.
- Replaces decision 128's bullet for the vertical grid only.

## 143. New Banner goes above the bottommost selected strip (2026-10-13)

Behavior note 27 (E-B12, BRD-10): with several strips selected, the reference app's New
Banner puts the banner above the bottommost selected strip. Ours (decision 44) went
above the topmost.

- **Insert Banner Above (⌘B) goes just above the bottommost selected strip**; Insert
  Banner Below (⇧⌘B) still goes just below it. A selection whose bottommost strip is in
  the Boneyard is refused, as before. With nothing selected, Above still goes to the
  start of the sub-board shown and Below to its end.
- The place is worked out by `bannerPlace` in `ui/js/logic.js`, with a page test. No
  engine change: the app and the MCP `insert_banner` tool take an index and have no
  selection.
- Day breaks are unchanged: Above goes above the topmost selected strip (decision 55).

## 144. Shift-click on the red flag dates adds a run (2026-10-13)

Behavior note 27 (E-E17, RFL-05): on the reference app's small calendar, Shift-click adds
the run to the pick without Command, and a Command-click that removes a date leaves the
run's starting point. Decision 138 had Shift-click replace the pick unless Command was
held, from the last date clicked.

- **A click picks that date alone; Command-click (Ctrl-click off the Mac) adds it, or
  removes it when picked; Shift-click adds every date from the anchor to this one and
  keeps the rest of the pick**, with or without Command. This replaces decision 138's
  Shift-click.
- **The anchor is the last date clicked that ended up picked**: a plain click, a
  Command-click that adds, or the end of a Shift-click. A Command-click that removes a
  date leaves it. From the note: click 5, Shift-click 9 picks 5 to 9; Command-click 7
  drops 7; Shift-click 15 adds 10 to 15 and 7 stays off. Click 22, Command-click 18,
  Shift-click 20 picks 18, 19, 20 and 22. Page tests hold both runs.
- Shift-click with nothing picked yet picks that date, as before.

## 145. Leaving the DOOD tab clears its sort (2026-10-13)

Behavior note 27 (E-D25, DOD-13): in the reference app, leaving the DOOD tab and coming
back cleared the sort, and the Sort dialog opened with empty rules. Ours kept the sort for
the session and cleared it only on a sub-board switch (decision 59).

- **Leaving the Day out of days tab clears its sort**, as picking another sub-board
  does; coming back shows the engine's order and Sort… opens with no keys. Without holds
  is unchanged (kept for the session).
- `doodSortAfter` in `ui/js/logic.js` decides it, with a page test; the tab switch and
  the grid's refresh both call it.
- Ties keep board order, as the note saw (EVE before FINN, both 0): the engine's sort is
  stable (`dood::sort_rows`), so no change there.

## 146. The Calendar's filter checks several categories and elements (2026-10-13)

Behavior note 27 (E-C11, CAL-07): the reference app's calendar filter lists the categories
in use and a check box per element under its category, each with All, several checked at
once; strips holding none of the checked elements are hidden, and the day headers and
every event stay. Ours took one category and then one element (decision 59).

- **Show is two compact drop-downs of check boxes**, Categories and Elements, each with
  All (nothing checked). The button names what is checked; the list closes on a click
  outside or Escape. No new panel: the row keeps its height.
- **Which strips show**: with elements checked, a strip holding any of them; with only
  categories checked, a strip holding any element of a checked category; with nothing
  checked, every strip. Day headers and events always show.
- **The lists**: the categories in use are those with an element; the element list shows
  the checked categories' elements (every category's under All), under category
  headings. Unchecking a category drops its checked elements. Our reading: in use means
  holding an element, not only one on a strip.
- `calendarFilter` and `stripShown(strip, filter, elements)` in `ui/js/logic.js`, with
  page tests. The filter stays the session's, never saved.

## 147. An element count counts one category (2026-10-13)

Behavior note 27 (E-S34, BRD-18): in the reference app the strip designer's Element
Quantity field opens a picker that lists categories only (its search finds category
names, not elements); one category per field, a new pick replacing the old, and the strip
shows the category's name and its count on the strip, as `Cast Members (4)` and
`Props (0)`. Ours picked elements one by one (decision 136).

- **One category per element count**: `StripColumn::category`, the column's `category`
  in the file (a category id, as report filters name categories), only on
  `element_count`, refused for a category the schedule lacks by every edit and by the
  reader. The element ids of decision 136 are gone: a file with an element count's
  `elements` is refused, as the strict reader refuses any key it would not write.
- **Shown as `Name (n)`**: how many of the category's elements the scene lists, `(0)`
  with none, so the strip names what it counts. With no category the count counts every
  element as before, a bare number, empty at none.
- **Elements left off the board are not counted** (our reading, marked so in E-S34; not
  checked in the real app): the cast count and the every-element count leave them out
  (decisions 105 and 120), and a category count is the same kind of count. The picked
  element of decision 136 counted either way because it was picked by name; a category
  is not.
- **The designer**: with Element count shown, a part lists the categories with a search
  box that finds their names, one pick as a radio button, and Every Element to clear it.
  The page's search rule is `countCategories` in ui/js/logic.js, tested under Node.
- **Deleting a category drops the pick**: that count counts every element again. Only an
  empty category can be deleted, so the strips showed `(0)` before. There is no category
  merge; moving an element to another category moves it between counts.
- **Layout files keep the category by name** (`category_name`), where decision 136 left
  the element picks out: element names belong to one schedule's breakdown, but category
  names carry across schedules (every new schedule starts from the same category list),
  and a count without its category loses what it shows. Loading finds the schedule's
  category of that name, ignoring case; where there is none the count counts every
  element. The file still reads and writes back as written whatever schedule loads it.
- **Reports keep their own count** (decision 136): a report's element count, the
  one-line report's included, counts every element; the category stays on the board.
- MCP has no tool that sets strip columns; its schedule reads and writes go through the
  format, so they follow.

## 148. The shoot day strip heads the day (2026-10-13)

Behavior note 27 and tonight's live check of our app (DBK-03): with the shoot day strip
set to the day and full date, the reference app heads each day with it, the first at the
top of the board above day 1's first strip, each next one just under the End of day line
before it. Ours (decision 67) drew it at the end of each day, just above its break.

- **The shoot day strip is the day's first line**, above its first strip, on the board
  and on the printed board. A drop on it lands as the day's first strip, as on a day
  heading.
- **It replaces our day heading when shown** (`Day 1 Mon 2027-03-01`): it says the same
  day and date and more, so the board stays one line per day denser. With the shoot day
  strip off, Show on Board's Day headings switch works as before. The strip is the
  board's own setting (saved in the file), so turning day headings off does not hide it.
- `dayLead(look, show)` in `ui/js/logic.js` decides the line, with a page test; the board
  and `boardprint.js` both call it.
- Reports are unchanged: their shoot day lines (decision 100) already head each day. No
  file change; FORMAT.md and APP.md now say where the strip goes.

## 149. The strip marks read on any strip color (2026-10-13)

Tonight's live check of our app in its real window (behavior note 27): the conflict mark
before a strip's scene number nearly vanished on a light strip color (a yellow INT DAY).
The warning sign was drawn as the system's colored picture, a yellow triangle, not in the
mark's red.

- **Each mark asks for its plain text form** (U+FE0E after the sign), so it takes the
  color it is given. `STRIP_MARK` in `ui/js/logic.js` holds both, with a page test.
- **On a strip each mark is a small badge of fixed colors**, the same in light and dark:
  the conflict mark white on red, the red flag mark in its type's color (decision 84) on
  white, each with a thin edge. It keeps the line's height (14 px badge on a 22 px line).
- Elsewhere (the Calendar, the Breakdown tab's marks line) the marks sit on the page's
  own background and are unchanged.

## 150. No tooltip over the whole board or sheet (2026-10-13)

Tonight's live check of our app (behavior note 27): a large tooltip listing the board's
keys came up wherever the pointer rested on the board, covering strips, and the
Breakdown tab's Previous and Next showed the sheet's whole key list, which they took from
the tab's container.

- **The board and the Breakdown tab carry no title of their own.** Their keys are in the
  Help shortcut sheet (KEY-05) and the menus. A child with no title of its own now shows
  none.
- **Previous and Next say one short line each**, with the key: `Previous sheet (⌘[)`,
  `Next sheet (⌘])`. Other buttons keep their one-line titles.
- A page test checks `main.js` sets no title on the two containers and the step buttons
  have theirs.

## 151. The Boards and Sub-boards dialog keeps its focus and its clicks (2026-10-13)

Tonight's live check of our app (behavior note 27): in Board > Boards and Sub-boards,
Escape did not close the dialog, and right after typing a board's new name and pressing
Return the first click on that row's Up was lost (a second click worked).

- **Why**: every edit redraws the dialog's lists. The redraw took away the focused field,
  so focus fell to the page behind and the dialog's own Escape handler never saw the key.
  And a name sent by Return could be sent again by the change that follows when the
  field loses focus, the press on Up; that second rename redrew the dialog between the
  press and the click, so the click landed on a button no longer there.
- **Each name goes once** (`nameToSend` in `ui/js/logic.js`): not the name the field was
  drawn with, not the one it already sent, and nothing from a field the redraw removed.
- **A redraw waits while the pointer is down** in the dialog (`pressGate`), and runs
  just after the click, so a click always reaches the button it started on.
- **Focus stays put**: after a redraw the dialog focuses the same field or button again
  (the renamed field after Return), so Tab and Escape keep working.
- **Escape closes any dialog** even when the focus has left it (`customDialog` listens on
  the document while open; keys inside the dialog are handled as before).
- Page tests hold `nameToSend` and `pressGate`.

## 152. Undo of a new sheet goes back to the sheet shown before (2026-10-13)

Tonight's live check of our app (behavior note 27): after New Sheet on the Breakdown tab,
Undo removed the sheet, but the tab then showed sheet 25 (on another sub-board) instead
of sheet 5, the one shown before. The tab asks Rust for its sheet by id, else by its
place in the order (`get_sheet`, decision 51); with the new sheet gone, its place held
another sheet.

- **Undo of New Sheet or Duplicate Sheet shows the sheet that was shown before it.** The
  page keeps each sheet those commands made with the sheet shown before
  (`sheetBack(current, made, exists)` in `ui/js/logic.js`, with a page test); when the
  sheet shown is gone after a change and was made that way, the tab goes back to the one
  before, and further back when several were undone at once.
- Redo brings the made sheet back without showing it (the tab stays on the sheet shown);
  a sheet no such command made falls back by its place as before. No engine change.

## 153. A new board starts today (2026-10-13)

Behavior note 27 (BRD-01) and tonight's live check: the reference app's new board has one
empty sub-board dated today. Ours left the date empty, so the Board tab showed
`YYYY-MM-DD` until one was typed. A new sub-board already starts today (decision 123).

- **The app**: New Board gives its sub-board, Main, today's date by the computer's clock
  (`exchange::local_today`, as New Sub-board), in the same undo step as the add.
- **The engine reads no clock**: `Schedule::add_board(name, start)` and
  `board::add_board(s, name, start)` take the date as an argument, as `add_sub_board`
  does. A new schedule's first board (`Schedule::new`) still has none, and a duplicated
  board's sub-boards keep their own dates, as before.
- **MCP and the command line** make no boards or sub-boards: `new_schedule` and the
  command line's new file give the one board an undated sub-board, as before (a schedule
  starts when its user says so).
- The harness fake starts a new board's sub-board today too. Its name stays Main (the
  reference app's is Default; not changed here).

## 154. Banners take any color: a palette of 49 swatches (2026-10-13)

Tonight's live check against the reference app: its banner style panel has a palette of
about fifty colors. Ours offered nine named colors (decision 65) in a drop-down.

- **The model**: `BannerColor` keeps its nine named colors and gains `Rgb(Rgb)`, a color
  by value. `text()` gives the name or `#rrggbb`; `from_text` reads a name or a lowercase
  `#rrggbb`. Integer only, as before.
- **The file**: `color` is still a string. Every older file reads and writes back byte
  for byte (the names are unchanged); a new file may also hold `#rrggbb`. Only lowercase
  is read, since the writer writes lowercase: an uppercase color is refused rather than
  quietly rewritten. A format test holds both.
- **The dialog**: Color is a compact grid of swatches, ten to a row: the board's own,
  the nine named colors (which still go lighter or darker with the theme), forty more by
  value (ten hues, each pale, light, mid and strong: `BANNER_PALETTE` in
  `ui/js/logic.js`), and a system color picker for any other. 49 swatches in all.
- **Drawing**: a banner colored by value gets a `bh-rrggbb` class whose rule (the color,
  and black or white text by the same luma as `Rgb::ink`) goes in a constructed
  stylesheet after the value is checked, on the board, the printed board and reports. It
  is drawn as picked in light and dark.
- **MCP and the command line** take the same: a name, `#rrggbb` or none (`color` in
  `insert_banner` and `set_banner` is now a string, described, not a list of names).
- Page tests hold the palette, the check and the ink; Rust tests the model, the file
  and the app's banner style.

## 155. A header value typed on a sheet takes the existing spelling (2026-10-13)

Behavior note 25 (E-L12) and our real window's check of note 27 (BRK-02, E-L12): in the
reference app, typing `room 2` in a sheet's Set offered the existing `ROOM 2` and used
it, so the sheet's value became `ROOM 2`. Ours kept `room 2` on the sheet and the strip,
and showed no list while typing; decision 62 matched values ignoring case only in the
Elements list and the day out of days.

- **The engine adopts the spelling**: `reports::header_spelling` gives the value another
  sheet already has in that field when it matches ignoring case (the first met in sheet
  order, as `header_category` spells it), else the value trimmed; the sheet's own value
  does not count, so a value only it has can be spelled again. `reports::set_header_value`
  sets it. The app's `set_field` and MCP `set_sheet_fields` both use it, for every
  one-value field of decision 118: set, location, I/E, time of day, unit, sequence,
  episode and script day. Synopsis, comments and the rest are kept as typed. The command
  line has no command that sets a header field.
- **The page lists the values as typed**: each of those fields shows the values other
  sheets use under it (`headerMatches` in `ui/js/logic.js`: starting with the text, then
  holding it, ignoring case, at most eight; a value spelled exactly as typed is left
  out). Up and Down pick, Enter or a click takes one, Escape closes the list; Enter with
  nothing picked commits what is typed, as before. The list is the page's own, as Quick
  add's is, not a `<datalist>`: whether the Tauri WebKit view draws a datalist was not
  checked, and the old I/E and time of day datalists are gone. The view sends
  `header_values` (each field's values, one per spelling ignoring case) in place of
  `int_ext_values` and `time_of_day_values`.
- Tests: `reports::tests::a_typed_header_value_takes_the_existing_spelling`, the app's
  `sheet::tests::a_header_value_takes_the_spelling_another_sheet_has`, MCP
  `a_sheet_field_takes_the_existing_spelling`, and a page test for `headerMatches`.

## 156. Quick add asks for a new name's category (2026-10-13)

Behavior note 24 (BRK-13), seen again in our real window in the check of note 27: in the
reference app, Quick add with a name no element has asks for the category from a list.
Ours put it straight into the New in category without asking.

- **A name no element has asks**: a small New Element dialog lists the categories, with
  the New in category chosen (a name typed in New in that is no category yet comes
  first), so Return adds it there and the add stays one keystroke more than before. Up
  and Down move, Return or a double-click accepts, Escape or Cancel goes back to Quick
  add with the text kept. The category taken becomes New in for the next one.
- **Unchanged**: `Category;Name` still creates without asking (decision 81, BRK-18), a
  category box's Add field still adds to its own category, and a match picked with the
  arrow keys is added as before.
- `quickAddPlan` and `categoryChoices` in `ui/js/logic.js` decide it, with page tests;
  the dialog is checked in the harness only.

## 157. Quick add with an existing element's exact name adds that element (2026-10-13)

Seen in our real window in the check of note 27: on a sheet without it, typing ZTORCH
(an element of the category ZGEAR) in Quick add and pressing Return made a new ZTORCH in
the New in category (Cast) instead of adding the existing one. Ours only reused a name
inside the New in category.

- **An exact name, ignoring case, adds that element** from whatever category holds it,
  with the elements it links to, as a picked match does. Only a name no element has is
  new (and asks, decision 156).
- **Two categories with the name**: the first in category order is added (then the
  element list's order); one already on the sheet is passed over for another that is not.
  The arrow-key list still lets you pick any of them. When every element of that name is
  on the sheet already, nothing changes and the line above the form says so.
- In `quickAddPlan` (`ui/js/logic.js`), with a page test.

## 158. The DOOD tab switches Saturdays and Sundays apart (2026-10-13)

Behavior note 27 (DOD-19, decisions 121 and 141): the reference app's DOOD View menu has
a switch for Saturdays and another for Sundays. Ours had one Weekends box for both.

- **Saturdays and Sundays**: two boxes in the DOOD tab's head where Weekends was, each
  hiding its own columns (rows when vertical; `g-sat` and `g-sun` cells, `no-sat` and
  `no-sun` on the table). Counts never change, as before.
- **Per viewer**, each in this browser's storage (`method-schedule.dood.saturdays` and
  `method-schedule.dood.sundays`). The old `method-schedule.dood.weekends` value is read
  for a day whose own key was never set, so a viewer who had Weekends off sees both off;
  it is no longer written. Both read back as on when storage is off.
- **The DOOD report follows each day**: a day switched off is left out, one switched on
  prints shaded as a day off, and a week is five, six or seven columns, still paged by
  whole weeks from the week's first day (decision 141). `loadWeekends` in dood.js now
  gives `{ sat, sun }`.
- `weekendDays`, `dayShown` and `doodReportPages` in `ui/js/logic.js`, with page tests.

## 160. Calendar drags use pointer events (2026-10-13)

Behavior note 27 (CAL-06), our real window: dragging a strip or an event to another date
on the Calendar tab did nothing in the Tauri window on macOS, while the board's strip
drag worked with the same moves. The calendar used HTML drag and drop (`draggable`,
`dragstart`, `drop`); the board uses pointer events. The Tauri window's own drag and drop
handling (`dragDropEnabled`, on by default) takes those events before the page sees a
drop.

- **The calendar drags as the board does**: a press on a strip or an event line, moved 5
  pixels, starts a drag with a ghost under the pointer; the date under it is marked, and
  letting go moves it there. Escape or a lost pointer puts it back, and the click that
  ends a drag picks no date. What a drop does is unchanged: a strip goes to the end of
  another shoot day's strips (behavior note 24), an event moves to start on the date,
  keeping its length, each one undo step.
- **`dragDropEnabled` stays on**: nothing opens a file dropped on the window today, but
  turning the window's handling off would only mend the symptom for one platform, and
  pointer events work in every webview and the browser harness. The calendar was the only
  HTML drag in `ui/`: the strip designer, the report designer and the layouts lists
  have no drag.
- `calendarDrop` and `dragStarted` in `ui/js/logic.js` decide what a drop does and when a
  drag starts, with a page test; the drag itself needs the next live check.


## 161. The breakdown by day prints its scenes (2026-10-13)

Behavior note 27 (RPT-06), our real window: the Breakdown by day report showed each day's
heading, its end of day line and the elements, but no scene rows: day 1 had only a red
flag mark in an empty row, day 2 an empty row with nothing to see.

- **The cause**: the breakdown by day had no default scene fields (`ReportKind::
  default_columns` gave it none, as the one-line, which follows the board), so with no
  columns picked each scene's `cells` were empty, while the page fell back to the board's
  default layout for the headings and drew empty cells under them.
- **The fix**: its default fields are those of the breakdown by strip and the shooting
  schedule: scene, I/E, set, time of day and pages. Columns picked in the report designer
  still win, and a named strip layout picked for it still applies.
- Test: `reports::tests::breakdowns_detail_vertical_and_calendar` checks that every day
  lists its scenes, each with a cell per column (the scene number, the set), and that the
  days together list every sheet.

## 162. Messages name a scene as a person knows it, never by its id (2026-10-13)

Behavior note 27 (BYD-01), our real window: Plan > Import Script Revision's preview listed
an omitted scene as `Omitted Sheet 237 (kept, marked omitted)`, the sheet's internal id.
Lane C also saw What Changed say `counting category 12` for a strip layout's element
count.

- **A sheet in a message** is `Sheet::title`: `Scene 26, INT. KITCHEN` (`Scenes 3, 3A, ...`
  for several), else its heading (`INT. KITCHEN - DAY`), else the sheet number the user
  typed (`Sheet 12A`), else `A scene with no number or heading`. The revision preview uses
  it for changed, new and omitted scenes.
- **The short forms keep their shape without the id**: `Sc 12` (timelines, call times,
  drift) and What Changed, merges and the DOOD explanation's `scene 12` drop `(sheet 4)`;
  a sheet without a scene number shows its heading. Find's place line does the same.
- **An element count's category is named**: `counting Props`, so `StripLayout::text`,
  `LayoutBody::text` and `Layouts::text` take the schedule. A merge conflict on a red flag
  names it by its type and dates (`red flag "Unavailable" (2027-03-02 to 2027-03-03)`).
- **Left with ids on purpose**: the command line's and the MCP server's messages, which
  name sheets, elements and boards by the ids their commands take, and the file checks'
  errors for a broken file, which point at the record in the file.
- Tests: `model::tests::a_sheet_is_named_as_a_person_knows_it`, the revision, diff, merge
  and explanation tests with the new texts, and `reports::tests` for `counting Props`.

## 163. Color correction is a post-production event kind (2026-10-13)

Behavior note 27 (E-E3, EVT-03): the reference app's Add Event types end with the post
kinds, Color Correction last. Ours had the other ten post kinds (decision 109) but not
that one.

- **`PostKind::ColorCorrection`**, `color_correction` in the file and over MCP, `Color
  correction` in words, after Trailer in every list (the page's kinds come from
  `EventKind::keys`). Like every post kind it shows on the calendar only: never in the
  day out of days, never a conflict.
- **A built-in red flag type of its own**, as each event kind but `other` has (decision
  113): new schedules start with 16, and Add the Standard Types adds it to an older schedule
  that has the rest. A file with `color_correction` read by an older build is refused as
  an unknown kind, as any new key is.
- Docs: FORMAT.md (event `kind`), MCP.md (`add_event`), APP.md (the Calendar tab).
- Test: `dood::tests::post_production_events_never_count_in_the_day_out_of_days` now runs
  over eleven post kinds and checks the new key; the red flag type counts follow.

## 164. Escape closes every dialog; text fields offer no saved values (2026-10-13)

Our real window in the check of note 27 (KEY-05): Escape did not close Help > Keyboard
Shortcuts, and its filter box showed WebKit's own list of suggestions.

- **Escape, wherever focus is**: the dialogs' shared code (`customDialog` in
  `ui/js/modal.js`) listened for Escape on the dialog's backdrop, so after a click on
  plain text in a dialog (the shortcut list, a heading), focus went to the page's body,
  outside the dialog, and Escape did nothing. Decision 151 (the Boards dialog, same
  night) added a document listener for keys from outside the dialog; the two now make
  one document listener for every Escape while a dialog is open, inside it or not. A
  list or menu inside the dialog that takes Escape (Quick add's and the header fields'
  match lists, the calendar's check box lists, a context menu) still closes first and
  keeps the dialog. Every dialog and sheet uses `customDialog`, so the fix is the same
  for all of them.
- **No saved values over a field**: every text field and text area the page makes
  (`el` in `ui/js/dom.js`) gets `autocomplete="off"` unless it sets its own, and every
  search or filter box also `spellcheck="false"`, `autocorrect="off"` and
  `autocapitalize="off"`. The shortcut filter's hint says "command or key", not "name",
  a word WebKit's AutoFill reads as a contact field. Whether WebKit in the Tauri window
  still draws a list needs the next live check.
- `dialogEscape` and `quietField` in `ui/js/logic.js`, with page tests.

## 165. The Find panel's advanced rows read in full (2026-10-13)

Our real window in the check of note 27: Edit > Replace's Find and Replace panel beside
the board cut its words, `All rows (AND` in the join list and `contai` for contains. The
panel is 22em wide and each condition put field, comparison, value and remove on one
line, so the lists shrank below their words.

- **A condition takes two lines**: the field with the remove button, then the comparison
  at its own width and the value taking the rest (a grid, `.fp-cond` in styles.css). A
  comparison without a value (is empty) leaves the second line to the comparison.
- **The join reads All rows or Any row**; its tooltip says AND and OR. The panel keeps its
  width, so the board loses nothing.
- `FIND_JOINS` and `FIND_OPS` moved to `ui/js/logic.js`; a page test checks that no
  option is longer than the panel gives it and that the condition layout is the grid.

## 166. New from Template works as New (2026-10-13)

Our real window in the check of note 27: File > New from Template replaced the window's
schedule (edited, but autosaved into its file) without a question, which read as unlike
File > New.

- **Checked: it already was New**. File > New (Cmd N) replaces the schedule in this
  window after the unsaved-changes question (decision 86), and New Window (Shift Cmd N)
  opens a window of its own. New from Template takes the same path: the same
  `settle_unsaved`, which first autosaves, so with Autosave on and a file nothing is left
  unsaved and nothing is asked, by either. With Autosave off, or a schedule with no file
  yet, both ask Save, Don't Save or Cancel.
- **The one change: pick first, then ask**, as Open does. New from Template used to ask
  before its Open dialog, so cancelling the dialog after Save had saved for nothing, and
  a template that couldn't be read came after the question. Now the template is picked
  and read first; a cancelled pick or a bad file changes nothing.
- No test drives the native dialogs; `templates::tests` still covers the template
  itself. To confirm in the next live check, with Autosave off.

## 167. A day break's sun comes from located sheets too (2026-10-13)

Behavior note 27, noted small (E-B16, BYD-08): our board's day break showed the earliest
sunrise and latest sunset typed on its sheets only, while Plan > Locations already worked
out the sun for each place with degrees, shown in the Day Timeline and the call sheet.

- **Typed, else worked out**: for each sheet of the day, a typed sunrise or sunset is
  used; a sheet with none whose location has degrees gives the time worked out for the
  day's date (BYD-08, `solar::sun`). Field by field, so a typed sunrise keeps a worked-out
  sunset. Then the earliest sunrise and the latest sunset of the day, as before
  (decision 67).
- No date, no location on the map, a sun that stays up or down all day, or a time past
  midnight: nothing worked out for that sheet. Each place's sun is worked out once a day.
- One rule in the engine (`timeline::day_break_sun`), used by the board's day view, so the
  board and the printed board show the same. Reports' End of day lines carry no sun times.
- Tests: `timeline::tests::day_breaks_take_the_located_sun_when_none_is_typed` and
  `board::tests::day_breaks_show_the_sun_at_a_located_sheet`.

## 168. Escape closes the calendar's check box lists (2026-10-13)

Behavior note 27, noted small (E-C11, CAL-07): in our real window the Calendar tab's
element filter list stayed open after Escape; a click outside closed it.

- The two drop-downs of decision 146 (`checkDrop` in `ui/js/calendar.js`) listened for
  Escape on themselves only. WebKit does not focus a check box on a click, so after one
  focus was on the page's body and the key never reached the list.
- **The document listens while a list is open**, in the capture phase, so it hears Escape
  first wherever focus is; it closes the list, puts focus on its button and stops the
  key, so a dialog around it (decision 164) or a drag does not also take it. The listener
  goes when the list closes.
- `dropEscape` in `ui/js/logic.js`, with a page test that also checks the listener.

## 169. The replace box's hint fits (2026-10-13)

Behavior note 27, noted small (SPL-05): in our real window the Find and Replace panel's
replace box cut its hint, "Replace with (empty removes the text)". The panel is 22em wide
(decision 165) and the box shares its row with the Replace button, leaving room for about
twenty characters.

- **The hint is Replace with**; the box's tooltip carries the whole meaning: the text
  that replaces what was found in the checked results, and that an empty box removes it.
  The panel keeps its width.
- `FIND_REPLACE_HINT` in `ui/js/logic.js`, with a page test on its length and on
  `find.js` using it.

## 170. Every dialog redraws after the click, not under it (2026-10-13)

Behavior note 27, noted small (BYD-08): in our real window the first click on Close in
Plan > Locations did nothing; a second worked, and Escape did. Decision 151 fixed the same
lost click in Boards and Sub-boards, where a name committed as its field lost focus to the
press redrew the dialog between the press and the click.

- **The gate moved into `customDialog`** (`ui/js/modal.js`): each dialog gets a press gate
  (`pressGate`, entry 151) on its backdrop and hands `later(f)` to the code that builds
  it. A dialog redraws after an edit through `later`, which waits while the pointer is
  down in the dialog and runs just after the click. Boards and Sub-boards uses it in
  place of its own.
- **The audit**: Layouts (a name sent on leaving its field, then the list redrawn) and Red
  Flag Types (the same, and Return sent the name and leaving the field sent it again) had
  decision 151's pattern; both now redraw through `later` and send each name once
  (`nameToSend`). The Plan dialogs, Day Timeline, Locations, Turnarounds and Rest and
  Schedule Locks, commit on Return or a button only, but redraw their lists or refill their
  fields after an edit; they go through `later` too. Header Layouts keeps its edits in a
  draft until Save and redraws only on a pick, so it needs nothing.
- **Not settled**: Locations has no field that commits when it loses focus, so the code
  shows no redraw under that first click; the gate covers whatever change came between
  the press and the click. To confirm in the next live check, with the steps of note 27:
  type degrees and an offset, Return, then Close once.
- A page test checks that `customDialog` passes the gate and that these dialogs use it.

## 171. Every Save dialog opens in the schedule's folder (2026-10-13)

Behavior note 27, noted small: in our real window Plan > Export Call Sheet Data's Save
dialog opened in the folder last used for images.

- **Checked**: Export, Export for Budget, Save as PDF, Save Layouts and Save As already
  opened in the schedule's folder (each passes it to the dialog), and Save as Template in
  the templates folder. The call sheet export alone passed none, so macOS opened the last
  folder any dialog used.
- **One path for exports**: the call sheet export now goes through `export::save_dialog`
  with `export::place`, as Export and Export for Budget do: the schedule's folder, the
  file's name as the stem, the extension added, the atomic write, and never the open
  schedule's own file. A schedule not yet saved has no folder; the dialog then opens
  where the system last saved.
- Not a folder remembered per kind of export: the schedule's folder is the one place
  every export already used, and the files sit beside the schedule they came from.
- `export::tests::every_save_dialog_opens_in_the_schedules_folder` checks `place` and that
  each Save dialog in the app sets its folder before it shows. The pickers for Import
  Script and Import Script with Tags still open where the system chooses.

## 172. Fuzz and property tests on the engine's own seeded generator (2026-10-13)

ROADMAP M1, the fuzz and property test rows (decisions 12, 13, 24, 25, 91). No fuzzing
crate: the engine has no dependencies (decision 3) and the property tests already run on
its xorshift generator, `testgen::Rng`.

- **`method_schedule_core::testgen`** is public but hidden from the docs, not part of the
  engine's API. Behind a `testgen` feature it gives other crates' tests what the engine's
  tests use: `random_schedule` and `random_edit` (moved from the merge tests, where they
  started), `mutate_bytes` (one to four byte edits: a byte changed, a run cut or doubled,
  a snippet put in, the end cut off), and `rounds` and `fuzz_rng`.
- The format crate turns the feature on as a dev-dependency only, so a normal build is
  unchanged and `Cargo.lock` gains nothing.
- **Rounds**: each fuzz or property test runs a modest number of cases by default, a few
  seconds in all. `METHOD_SCHEDULE_FUZZ_ROUNDS=N` multiplies them and
  `METHOD_SCHEDULE_FUZZ_SEED=N` mixes a number into each seed; a failure names its case,
  found again with the same two variables.
- **The weekly fuzz job** (`.github/workflows/fuzz.yml`, Wednesdays and on demand, Linux
  only): the core and format crates' tests in release mode at a hundred times the rounds,
  the run number as the seed, so each week covers new cases. Checkout pinned to the SHA
  `ci.yml` uses; no other actions.

## 173. The file reader refuses a key given twice; fuzz tests hold it to its promise (2026-10-13)

ROADMAP M1, fuzz and property tests for the file reader (decisions 13 and 133).

- **Found and fixed: a key given twice read**, the last one kept (`"title": "A", "title":
  "B"`, at any depth, and in the Breakdown tab's widths map), so the file did not write
  back as it was. The reader took the file as a JSON value first, which keeps one of two
  same keys, and read the schedule from that. Now it also reads the text once for keys
  given twice and refuses the file, naming the key, its line and column. Schedule,
  template and layout files alike. Test: `fuzz_tests::a_key_given_twice_is_refused`.
- **Round trips** on random schedules built through the engine (`random_schedule`, random
  edits, then odd free text, sun times, screen time, named layouts, call times and a map
  location): the schedule file and the template read back to the same schedule and the
  same bytes; the layout file reads back the same layouts and writes the same bytes; the
  budget exchange file, which has no reader, writes the same bytes twice, and each
  element's total is its days of every kind added up.
- **Mutations** of written files (the schedule with every key set, and random ones, each
  as a schedule and as a template; layout files): a value for another the writer could
  write, a key or a list entry given twice or taken out, an unknown key, values the writer
  would never write (`-0`, `1.0`, `1e400`, an escaped letter, deep nesting, a 70,000
  character string) and bytes changed anywhere. A mutated file is refused, or it reads
  into a schedule that passes `check` and writes back as the file was: byte for byte when
  the mutation kept the writer's layout, the same JSON otherwise. 800 schedule and 400
  layout cases in the suite.
- A layout file's element count names its category, found in the schedule it loads into
  ignoring case, and a name the schedule lacks counts every element (decision 147), so a
  layout file mutated in a `category_name` writes back as that schedule has it. Not
  changed.

## 174. A merge either way round gives the same schedule (2026-10-13)

ROADMAP M1, property tests for merge (decisions 25 and 91). The new property:
`merge(base, a, b)` settled for ours is `merge(base, b, a)` settled for theirs. It found
three things, each fixed with a test in `merge::tests`.

- **Fallbacks keep the side the merge settles for**, not always ours: a merge settled for
  theirs (`--theirs`) keeps theirs' name, board ID, boards, strip and red flag place where
  the parts don't fit together, and as the last resort theirs whole. With the default
  nothing changes. `a_merge_settled_for_theirs_falls_back_to_theirs`.
- **Two elements renamed to one name are both kept**: when the later one's name came from
  the kept side, the first one gets the kept side's name back; it used to be merged into
  the first, one element lost. Categories and red flag types the same.
- **An element one side moved into a category the other side deleted** keeps the other
  side's category (else the base's); it used to be left out, with its tags, when ours
  had moved it. `an_element_moved_into_a_deleted_category_keeps_the_other_sides`.
- `property_merge_is_symmetric` holds this up to what neither side decides: things both
  sides add or put at one place (strips, events, tags, links, sheets) go the kept side's
  first, so only their order may differ, and when both sides add things the same things
  may get other ids (those cases are left out). Conflict keys are the same; a conflict's
  `what` and values name things as ours does. Both merges pass `check`, and `diff(m, m)`
  is empty. The laws (`merge(base, x, x) == x` and the empty sides) now take the fuzz
  rounds and seed too.

## 175. Script reader fuzz tests; a Fountain scene far down a long script keeps its length (2026-10-13)

ROADMAP M1, fuzz tests for the `.fdx` and Fountain readers beyond the mutation test
(decisions 9, 12 and 13).

- **ZTEST-style scripts in Rust** (`import_fuzz`): the shape `scripts/make-ztest-fdx.py`
  writes, as Final Draft (numbered headings, lengths in every form the length run tried
  and some no script should have, the ZTEST cast speaking, dual dialogue, a title page,
  production tags) and as Fountain (scene numbers, forced headings, page breaks, notes,
  boneyard, sections). Unmutated, each reads its scenes as written. Mutated 1,500 times
  each: a reader never panics, and what it reads imports (tags too) into a schedule that
  passes `check`, every page from 1, every length in range: from 1/8 for Fountain, and
  from 0 for Final Draft, whose lengths are used as written (`0` is a length the reference
  app shows, behavior note 16).
- **Never hang**: a mutation adds a few hundred bytes at most, and a test reads large
  inputs (100,000 unclosed `[[` or `/*`, a 400,000 character word, 50,000 page breaks,
  100,000 empty paragraphs, a 400,000 character attribute) that would take minutes in a
  reader slower than linear. Files stay capped at 64 MiB when read.
- **Found and fixed**: a Fountain scene's length came from line positions times 16 in 32
  bits, which overflows past about 268 million lines; page breaks reach that in a 30 MB
  script. A debug build panicked; a release build wrapped to a wrong length. Positions
  now count in 64 bits and line counts saturate. Test:
  `import_fuzz::a_fountain_scene_far_down_the_script_keeps_a_sane_length`.

## 176. Performance budgets at 1,500 strips, as tests (2026-10-13)

ROADMAP M1, performance budgets as tests; decision 24 and HANDOFF.md "Performance".

- **Budget tests**, ignored by default and named `budget_*`: in the engine
  (`sample::tests::budget_move_dood_and_check_at_1500_strips`) and in the app
  (`src/budgets.rs`), on `sample(1500)` (1,500 scenes, 1,792 strips, one sub-board).
  Each timing is the median of 15 runs after one to warm up, and every line prints with
  its budget, so a run shows the margin as well as a breach. In a debug build they print
  that they are skipped and pass: debug timings say nothing.
- **Measured on the owner's Mac** (release, medians of four runs while other lanes were
  compiling, so a little above a quiet machine's), with the budgets:

  | What | Measured | Budget |
  |---|---|---|
  | An edit (session copy, move, check, undo step) plus the board view | 1.2 to 1.3 ms (was 2.6 to 4.0; decision 190) | 5 ms, the target |
  | The same plus the view's JSON (489 KB) | 1.6 to 1.7 ms (was 3.2 to 4.8) | 15 ms |
  | Engine: a move plus the whole day out of days | 1.1 to 1.3 ms (was 1.4 to 1.6) | 8 ms |
  | Engine: the day out of days alone | 1.3 to 1.8 ms | 8 ms |
  | Engine: `Schedule::check` | 0.07 to 0.08 ms (was 0.5 to 0.9; decision 190) | 3 ms |
  | The Day out of days tab's grid (cast) | 2.7 to 3.0 ms (was 3.4 to 4.1) | 20 ms |
  | Reading and checking the file (`method-schedule check`) | 6.2 to 7.2 ms | 30 ms |
  | Report data: element detail | 25 to 29 ms | 100 ms |
  | Report data: DOOD | 9.1 to 10.1 ms | 40 ms |
  | Report data: one-line, shooting, breakdown, breakdown by day, multi-unit | 4.5 to 5.9 ms | 25 ms |
  | Report data: elements | 2.7 to 3.2 ms | 15 ms |
  | Report data: vertical one-line | 1.4 to 1.6 ms | 8 ms |
  | Report data: totals, calendar | 0.7 to 1.3 ms | 5 ms |
  | Report data: red flags (the sample has none) | 0.5 ms | 3 ms |
  | Report data: conflicts | 0.12 ms | 2 ms |

- **Budgets are about four to five times the measured time**, so a shared CI runner
  passes and a slip of several times fails. The one exception is the 5 ms for an edit plus
  the board view, the kickoff's target, which has under twice its margin; if a runner
  breaches it, the edit path is the thing to make faster, not the budget to raise. A
  runner did (9.06 ms on GitHub's ubuntu runner, about 2.5 times this Mac): decision 190
  made the edit path faster, and decision 191 scales every budget by how much slower the
  machine running them is.
- Each report is built through `reports::build` with its default options, the same data
  the Reports tab and printing get.

## 177. The page's redraw timed under Node, with a stand-in DOM (2026-10-13)

ROADMAP M1, performance budgets as tests: the page's redraw under 5 ms at 1,500 strips.

- **`page-tests/board-budget.mjs`** loads the real `board.js` (and what it imports, from a
  copy of `ui/js` beside a package file that makes them modules) against a small stand-in
  for the DOM: elements that keep their classes, attributes and children, constructed
  stylesheets, and a scroller whose content moves as it scrolls. It reads the view the
  Rust budget wrote (`METHOD_SCHEDULE_VIEW_DUMP`, then `METHOD_SCHEDULE_VIEW` for Node)
  and times, as medians of 15: parsing the view, `render` (the row model for all 1,792
  rows, the lines in view and every drawn strip's classes and cells), and selecting a row
  far down the board, which scrolls and draws it.
- **Measured** on the owner's Mac, Node 26: parsing 1.7 ms (budget 10), the draw 0.7 ms
  with 611 stand-in nodes (budget 5, the redraw target), a far select 0.2 ms (budget 2).
  It also checks that the draw stays windowed: under 2,000 nodes for 1,792 rows.
- **What stays manual**: the browser's own element creation, style and layout, about
  most of the 3 ms an edit costs the page (entry 24). Those are timed by hand in the page
  harness with `?big`, as before. The file is not a `*.test.mjs`, so the `page` job and
  `node --test page-tests/` leave it out; it fails, not skips, without a view file.

## 178. A CI job runs the budgets in release mode (2026-10-13)

ROADMAP M1, performance budgets as tests: a budget breach fails CI.

- **The `budgets` job** in `ci.yml` installs the same system packages as `build` and runs
  `cargo test --release --workspace --locked -- --ignored budget --nocapture` with
  `METHOD_SCHEDULE_VIEW_DUMP` in the runner's temp folder, then the page budget on that
  view with the image's Node. No new actions; checkout is pinned to the same SHA.
- It runs beside `build` rather than in it, so a slow or breached budget is told apart
  from a failing test, and the release build doesn't lengthen the main job.
- Locally: `cargo test --release --workspace -- --ignored budget --nocapture`. The older
  `perf_*` tests stay for their fuller printouts (red flags, the view's size).

## 179. A change made outside the app is settled in the app (2026-10-13)

ROADMAP M1, "Outside changes settled in the app" (docs/APP.md "Not yet", after Method
Budget's keep mine and load theirs). Before this, the guard only refused the write:
autosave paused with a recovery copy and Save As was the way out.

- **The check**: when the window gets focus (Rust sends `outside-check`), when autosave
  finds the change, and before Save. The file is compared with the version this window
  last read or wrote: size and modification time first, then the content hash when
  either differs. A file only touched (same content, new time) is the same, and its new
  time is kept so the next check reads nothing. The guard before a write still always
  reads and hashes; the time never allows a write.
- **The decision** is one pure function, `outside::decide` (disk state, theirs reads as
  a schedule, unsaved changes here, a base kept): nothing for the same content or a file
  gone; Load Theirs (default) or Keep Mine when only the file changed; Merge (default),
  Keep Mine or Load Theirs (asked again) when both changed; Keep Mine alone when the file
  no longer reads as a schedule. Later is always there and leaves things as before.
- **Defaults**: Load Theirs when nothing here is unsaved, as nothing is lost; Merge when
  both changed, as it keeps both.
- **Autosave never writes over it**: the check pauses autosave into the file as the guard
  did, and no choice writes without the user picking it.
- `outside::tests` (every disk and window state, Load Theirs, Keep Mine, Later, a file
  that isn't a schedule) and page tests for the dialog's buttons and words.

## 180. Merging the file on disk: the version last read here is the base (2026-10-13)

ROADMAP M1, "Outside changes settled in the app"; docs/MERGE.md.

- The session keeps the version it last read from or wrote to its file (one more copy
  of the schedule, set on Open and on every save). That is exactly the version both the
  window and the outside change started from, which the backups can only guess at.
- Merge reuses Merge Changes From: theirs is the file on disk (kept by token, named
  "film.mtsc on disk"), the automatic base is `last_read`, and the base list offers it
  and the backups but not the file on disk, which is theirs.
- Applying the merge takes the file's version as read (its stamp) as settled; the
  merged schedule is then saved as any change is. A file changed again since is caught
  by the guard and asked about again.
- A recovered schedule, or one never read here, has no base kept: Merge is not offered.
- `outside::tests::merging_the_file_on_disk_uses_the_version_last_read_as_the_base`.

## 181. Revert to Backup replaces the schedule in this window as one undo step (2026-10-13)

ROADMAP M1, "a Revert to Backup dialog"; backups as in docs/APP.md "Saving and recovery".

- File > Revert to Backup lists the file's rolling backups (`Backups/<path hash>/`),
  newest first, each with its date and how many changes it is from the schedule now.
- **In this window, not a new one**: the backup is a past state of this same file, so it
  belongs in the window that has the file, its undo history and its autosave. Undo takes
  the revert back, and nothing new is left open pointing at a backup in the data folder.
  To look at a backup without reverting, Open still opens the file itself.
- The file as it is now goes into the backups first (unless it is the same as the
  newest), so reverting never loses the version it replaces.
- `outside::tests::revert_to_backup_is_one_undo_step_and_keeps_the_current_file`.

## 182. Settling an outside change never acts on a version unseen (2026-10-13)

ROADMAP M1, "Outside changes settled in the app".

- The dialog carries the hash of the version it showed; Keep Mine and Load Theirs are
  refused when the file holds anything else by then ("changed again"), and nothing
  changes.
- Keep Mine and Merge back up the version on disk first, whatever the gap since the last
  backup, so theirs is always in Revert to Backup afterwards.
- Load Theirs is one undo step, saved as the file's version: Undo brings yours back as
  unsaved changes. With unsaved changes it asks a second time first.
- Later remembers the version put off, so the check on focus doesn't ask again until the
  file changes again; Save asks again whatever was put off.

## 183. Strips cut or copied in one window paste in another (2026-10-13)

ROADMAP M1 known gaps (FIL-03): decision 86 gave each window its own clipboard, so strips
could not go from one schedule to another. Not in behavior note 27; not checked in the
reference app.

- **One clipboard as the user sees it**: Cut or Copy of strips in a window also gives
  every other window those strips, carried by name (`clipboard::carry`,
  `Clip::Carried`): each scene's sheet as it was, with its elements by category and
  name and their quantities; banners and day breaks as they are. Cut or Copy of
  elements empties the other windows' clipboards (elements stay in their own window).
  The other pages are told (`clipboard-changed`), so their Paste turns on at once.
- **The paste makes copies**, as a cut pasted on another board does (decision 38): a
  scene becomes a new sheet below the selection and at the end of every other board's
  Boneyard there. A cut leaves the originals in their own schedule (moving them out of
  it would be an edit in another window's undo), and the status line says so: `Pasted
  copies of 2 strips from ZTEST One. A cut moves strips only within its own schedule,
  so the originals stay there.` The cut is used up in the window it was pasted in.
- **Elements**: matched by category name and element name ignoring case; a cast element
  whose category name the target lacks goes to the target's cast category; anything
  else missing is added (a category with the carried settings, never a second cast
  category; an element by name, with the board ID its category gives a new element).
  Board IDs, notes, red flags and DOOD rules of an element stay in its own schedule.
- **Header values** take the spelling the target already has (decision 155), field by
  field after the sheet is added. Image paths are carried as written.
- **Undo**: the whole paste is one undo step in the target window; the source window is
  not touched.
- Tests: `clipboard::tests::strips_carried_to_another_schedule_paste_as_copies`,
  `docs::tests::a_copy_is_shared_with_the_other_windows`. To see in a live check with
  two windows.

## 184. The board beside the board marks Find's results too (2026-10-13)

ROADMAP M1 review row (SPL-02, feature review 2026-10-13): decision 130 outlined the Find
panel's results on the Board tab's board, but the board beside it (split screen,
decision 80) showed none. Not in behavior note 27; a page change only.

- **The same outline on the board beside**, worked out for the board it shows: every
  scene in the results (a sheet is on every board), and the banners in the results that
  are on that board (a banner is a strip of one board). `foundOn(hits, board)` in
  `ui/js/logic.js` gives both boards their marks; board2.js takes them on every draw (a
  board or sub-board picked there) and whenever the results change; closing the panel
  clears them on both.
- **Select on Board** still selects on the Board tab only, as before.
- Test: the page test `Find marks: scenes on every board, a banner on its own board only`.

## 185. Strip designer line styles: a line after a field, a rule under it (2026-10-13)

ROADMAP M1 known gaps: decision 87 built the strip designer's lines, merged cells and
looks but left line styles, scale to content and a category's elements as a field. Not
in behavior note 27; the reference app's style panel was not looked at again for this.

- **Two line styles per field** (`StripColumn::rule_after`, `rule_under`): a line down
  the field's right edge, between it and the next column, and a rule along its foot,
  under its line. A rule under a whole line is the rule on each of its fields; a field
  merged across columns draws its line at the merged cell's edge. In Board > Strip
  Layout each field has `|` and `_` beside B and I.
- **Drawn without changing sizes**: on the board an inset shadow in the text's color
  (the 22 px line and the column widths stay, so windowing and dragging keep their
  arithmetic); on paper a 1 px black border on the cell. The column headings take the
  same classes. `ruleClass` in `ui/js/logic.js` gives the classes, from `cellClass`.
- **File** (additive): `rule_after` and `rule_under` on a strip layout column, written
  only when true; the same in layout files and named layouts. Diff and merge keep
  treating a layout as one value; What Changed and the layout text say `line after`
  and `rule under`.
- **Not done, what's left**: scale to content (what it does in the reference app, the
  whole strip's text shrinking to fit its cells or the columns sizing to their text,
  needs a look at the reference app's style panel first) and a category's elements as
  a strip field (a field with a category pick, as the element count's of decision 147;
  a day or so once wanted). Line colors and thicknesses: one thin line in the text's
  color only.
- Tests: `model::tests::the_strip_designer_has_four_lines_merged_cells_styles_and_text`,
  format's `the_strip_designer_round_trips_and_its_keys_are_left_out_at_their_defaults`,
  the app's strip layout test, and the page test `strip line styles`.

## 186. The breakdown designer's grid: drag fields, rows of the user's own lengths (2026-10-13)

ROADMAP M1 known gaps: decision 101 gave the breakdown designer hidden fields and widths
but left dragging fields in a grid and rows of the user's own lengths. Behavior note 19
saw the reference app's designer keep the fields in a grid of rows; not in note 27.

- **Rows in the engine** (`SheetLooks::rows`): how many of the shown fields each row of
  the sheet holds, in order; fields past them make one more row; empty is the usual
  six, five and the rest. Each row holds at least one field, all together at most the
  18 a sheet has (`SheetLooks::with_rows`, and `check`). The lengths count shown
  fields, so hiding one moves the fields after it up a place. Diff says `rows of 4, 4, 6` with the rest of the looks; merge keeps the
  looks one value.
- **File** (additive): `sheet_rows`, left out at the usual rows.
- **The designer**: under the field list, the shown fields as chips in their rows, as
  the tab lays them out, with a New row line at the foot. A chip dragged (pointer
  events, as decision 160) to a place in its row, another row, or New row moves the
  field there; a row left empty goes, and the rows' lengths become the user's. The list
  and the grid stay in step (Show and Up and Down redraw the grid); a hidden field keeps
  its place in the order. Default clears the rows. Escape during a drag puts the chip
  back. Use These sends `rows` with the order, hidden fields and widths, one undo step.
- `sheetRows`, `gridMove` and `gridDesign` in `ui/js/logic.js`; the Breakdown tab lays
  its form out with `sheetRows`.
- Not done: dragging a field out of the grid to hide it (the reference app's remove
  area, note 19); Show does that. The drag needs the next live check.
- Tests: the app's `hidden_fields_and_widths_save_with_the_order` (rows kept, refused,
  undone), format's `hidden_sheet_fields_and_widths_round_trip` and the strict test, and
  the page test `breakdown designer: rows of the usual or own lengths`.

## 187. A banner's font family: the board's or one of three safe ones (2026-10-13)

ROADMAP M1 known gaps: decision 65 left a banner's font family for later (inventory
section 5: the reference app's banners have a font); decision 87 added size and
alignment, no fonts. Not in behavior note 27; the reference app's font list was not
looked at, and is not copied.

- **A short fixed list** (`BannerFont`): Usual (the board's own), Serif, Sans serif and
  Monospaced. Each is drawn from faces every Mac, Windows and Linux system has, then
  the generic family (`Georgia, Times New Roman, serif`; `Helvetica Neue, Helvetica,
  Arial, sans-serif`; `Menlo, Consolas, Courier New, monospace`), so a schedule looks
  alike on any machine and on paper. No family names are typed or stored, so a file
  never names a font another machine lacks, and the CSP and the class-only styling stay
  as strict (classes `bf-serif`, `bf-sans`, `bf-mono` from fixed names, `bannerFontClass`
  in `ui/js/logic.js`).
- **Where**: the Banner dialog's Font; the board, the printed board and the reports'
  banner lines; `font` on a banner strip in the file, left out for the board's; the
  style text says `serif`, `sans serif` or `monospaced`; `banner --font` on the
  command line and `font` in MCP's banner style, `insert_banner` and `set_banner`.
- An unknown name is refused everywhere (the reader also refuses `usual` written out,
  so a file has one form).
- Tests: the app's `banners_take_a_style_that_editing_the_text_keeps` and the reports'
  banner test, format's `banner_styles_round_trip_and_unknown_colors_are_refused` and
  the strict test, the CLI's banner tests and MCP `banners_insert_list_and_restyle`, and
  the page test `banner fonts`.

## 188. The printed board is as dense as the screen (2026-10-13)

ROADMAP M1 known gaps: decision 53 made the screen denser (the strip line 26 px to 22
px, text 13 px) but left print.css as it was, so the printed board kept roomier rows.
Not in behavior note 27; our own rule (decision 53), no reference-app check needed.

- **The same proportion on paper**: a printed strip line holds its 8.5pt text as the
  board's 22 px line holds 13 px text: 8.5 x 22 / 13 = 14.4pt a line (11.4pt line height,
  1.25pt above and below, the 0.5pt rule). It was about 17pt (8.5pt at 1.35, 2.5pt above
  and below), so about 18 percent more strips go on a page.
- **Every line of the printed board**: a scene's later lines (they had no space above),
  day breaks and banners take the same line; a two-line banner is two lines, 28.8pt, as
  on the board. The other reports keep their own spacing: the rule is the printed
  board's (`tr.bp`), and the reports' banner lines (`rpt-banner`) are unchanged.
- The page test `the printed board is as dense as the screen` reads the screen's line
  and text size (board.js, styles.css) and the print rules (print.css) and holds them to
  the proportion, so a change to one side fails until the other follows.
- Not checked on paper; the next live check prints a board at both sizes.

## 189. A blank replace unlinks a one-value field's value (2026-10-13)

Behavior note 05 (the reference app's manual, E-S32): replacing a pick list value with
blank unlinks it rather than leaving an empty value. Decision 82 left it. Not in behavior
note 27; not seen in the real app.

- **Ours has no stored pick lists**: a one-value header field (set, location, I/E, time
  of day, unit, sequence, episode, script day; decision 118's `HeaderField`) holds its
  value on the sheet, and the lists (the Elements tab, the DOOD, the typing list of
  decision 155) are made from the sheets, skipping empty values. So an empty field is
  already unlinked; what was missing is the replace leaving spaces behind.
- **Settled**: a replace in a one-value field trims the result (`SheetField::one_value`,
  `find::replace`), as element and category names are trimmed. Replacing the whole value
  with nothing, or with spaces, leaves the field empty: the value is unlinked from the
  sheet and leaves the lists once no sheet has it. Replacing part of a value with
  nothing keeps the rest, trimmed (`CABIN NIGHT` less `NIGHT` is `CABIN`). Free text
  (synopsis, comments, scenes, script pages, the sheet number) keeps what the replace
  leaves, spaces and all, as before.
- The replaced value does not take another sheet's spelling (decision 155 is for typing);
  Replace writes what was asked.
- Same in the app, the command line (`replace`) and MCP, which share `find::replace`.
- Test: `find::tests::a_blank_replace_unlinks_a_one_value_field`.

## 190. The edit and the board view, more than twice as fast (2026-10-13)

Decision 176: the CI `budgets` job breached the 5 ms target for an edit plus the board
view (9.06 ms on GitHub's ubuntu runner, about 2.5 times this Mac), and 176 says to make
the edit path faster rather than raise the budget. Not from a behavior note; no behavior
changes.

- **Profiled** with macOS's `sample` on a release loop of the budget's edit and view. The
  costs were, in order: building the strips (each scene's cast copied twice and sorted
  with the natural board ID order every strip, the strip color upper-casing two copies),
  `Schedule::check` (every element against every other, and element lookups by a walk of
  the element list from every sheet), dropping the view, the schedule's copy, and the
  date of each day counted from the sub-board's start again (time growing with the
  square of the days).
- **Fixed by indexing once per check or view**, no caching across edits:
  - `Schedule::check` keeps an id set of the elements for its lookups, finds repeated
    element names and board IDs with sets (and only then compares pairs, so the message
    names the same element as before), and checks a short list for repeats pair by pair.
  - A quick hasher for ids (`IdHasher`, `IdMap`, `IdSet` in the core, no dependency): the
    sheet and element indexes, the check's sets, the parts of a scene spanning days and a
    strip move use it in place of the standard hasher, which guards against inputs made to
    collide that a schedule's own ids are not.
  - `reports::CastOrder`: the cast in board ID order once for the board; each strip sorts
    its own by that place and copies each ID once. The same list as `cast_ids`, which
    the reports keep.
  - `Calendar::day_dates`: each day's date walked on from the day before, the same dates
    as `date_of_day`. The board view and the day out of days use it.
  - The strip color (`int-day` and the rest) compares in place; the reports' copy of it
    is gone and they share the view's. Each day's strip list is made at its size.
- **Measured** on the owner's Mac, release, medians of 15, while other lanes compiled:
  edit plus the board view 3.9 to 4.0 ms before, 1.2 to 1.3 ms after (1.6 to 2.1 ms in
  the busiest minutes, when everything ran about half again as long); `Schedule::check`
  0.86 ms to 0.07 ms; the board view alone 2.4 ms to 1.2 ms under the same load. What is
  left is mostly copying text: the schedule's copy for undo (0.23 ms), dropping the
  oldest undo step, and the strings of 1,792 strips.
- Tests: `model::tests::day_dates_match_date_of_day`,
  `model::tests::check_finds_repeated_elements_and_lists`,
  `reports::tests::cast_order_matches_cast_ids`,
  `view::tests::tone_reads_int_ext_and_night_in_any_case`; every earlier test passes
  unchanged.

## 191. Budgets scaled for a slower machine (2026-10-13)

Decision 176: the budgets are a user-facing target on a typical laptop, the owner's Mac,
but CI runs them on a shared runner about 2.5 times slower.

- **A calibration before the budgets**: `sample::reference_work` is a fixed piece of work
  much like the edit path but none of the app's code (4,000 short strings made, sorted,
  hashed and copied, with integer arithmetic). `sample::budget_scale_percent` times it,
  the median of 15 after a warm-up, against `REFERENCE_WORK_MICROS`, its time on the
  owner's Mac (450 µs: 410 to 475 µs measured in the budget runs, the upper end kept so
  the scaling errs strict).
- **Every Rust budget is multiplied by that ratio, at least 1x and at most 4x**: a faster
  machine keeps the budgets as they are, and a machine more than four times slower fails
  rather than hide a slip. The 5 ms target stays the number in the code; each line prints
  the budget allowed here and the factor (`1.28ms of 5.00ms (5ms x1.00)`), after a line
  giving the reference time and the ratio.
- **It tracks the edit path**: in the busiest minutes on the owner's Mac the reference work
  took 600 to 695 µs, about 1.5 times its quiet time, and the edit plus the board view
  1.6 to 2.1 ms, also about 1.5 times.
- Both the engine's and the app's budget tests are scaled. The page's budget under Node
  (decision 177) is not: its margin is about seven times.
- If the ratio does not track the runner's slowdown on the edit path, the CI lines show
  both numbers side by side; the reference work is the thing to adjust, not the budgets.

## 192. What runs past wrap moves to the next shoot day (2026-10-14)

Feature review (docs/research/feature-review-2026-10-13.md, BYD-04) and the roadmap's M1
row: decision 71 warns of a long day but left pushing its tail to the next day for later.
Not from a behavior note; the reference app has no timeline.

- **Past wrap**: a day's latest wrap is its call plus the long day limit ("Long day
  after", 12 hours by default); without a call time, the limit counts from the call, as
  the timeline does. What runs past is every strip from the first that ends after it to
  the day break, in board order: whole strips only, so a scene that starts in time and
  ends late goes too, and the banners and untimed or omitted scenes after it go with it.
- **The day's first strip always stays**: a scene longer than the whole day can't be
  helped by moving it, and moving it would only empty the day.
- **The move** (`timeline::push_past_wrap`): the strips go, in the same order, to the
  start of the next shoot day on the same sub-board, just after the day break. Day breaks
  stay where they are, so each keeps its call time; the moved strips take the next day's.
  Refused on the last shoot day (the strips would land below the last break, off the
  schedule; add a day break first), when nothing runs past, and when the list previewed
  is no longer what would move. One move; the next day may then run long itself, and the
  same offer shows there.
- **Preview first** (`timeline::past_wrap`, changing nothing): the strips and their
  names, the latest wrap and the next day, in a sentence (`PastWrap::text`).
- **App**: Plan > Day Timeline shows, under a day's warnings, what runs past wrap and a
  Move to Day N button; the move is one undo step, and the dialog redraws.
- **Command line**: `timeline` adds a `>` line for each day with something past wrap;
  `timeline FILE DAY --push` moves it and saves. **MCP**: `timeline` gives each day a
  `past_wrap`; the new `push_past_wrap` tool moves it (74 tools).
- Tests: `timeline::tests::what_runs_past_wrap_moves_to_the_next_day`,
  `timeline::tests::a_days_first_strip_stays`, the app's
  `plan::tests::what_runs_past_wrap_moves_as_one_undo_step`, and the command line and MCP
  beyond tests.

## 193. Daylight saving rules for sunrise and sunset (2026-10-14)

Feature review (docs/research/feature-review-2026-10-13.md, BYD-08) and the roadmap's M1
row: decision 72 took one typed UTC offset per location, the one in effect on the shoot
dates, and left a daylight saving switch for later. A shoot spanning a switch date had its
sunrise and sunset an hour off on one side of it. Not from a behavior note.

- **A rule per location**, optional (`GeoLocation::daylight_saving`, `solar::DaylightSaving`):
  the common two, US and Canada (the second Sunday of March to the first Sunday of
  November, at 2:00 local) and the EU (the last Sunday of March to the last Sunday of
  October, at 1:00 UTC), or none. Worked out from the date in integers
  (`switch_dates`, the nth and last Sunday of a month from the day count); no time zone
  database and no dependency. Other countries' rules are not offered; a location there
  keeps none and a typed offset, as before.
- **By the whole date**: the offset on a date is the standard one, an hour more from the
  date the clocks go forward up to the day before they go back (`GeoLocation::offset_on`).
  Every switch is in the small hours, before any sunrise these places see in March,
  October and November, so a day's sunrise and sunset are both on one side of it.
- **With a rule the typed offset is the standard one** (winter time, `-5` for New York);
  without one it is still the offset on the shoot dates, so every older file reads the
  same times.
- **Everywhere the sun is worked out**: the Day Timeline and its sunset warnings, a day
  break's sun (decision 167), the call sheet and `sun`, through `timeline::sheet_sun` and
  `day_break_sun`.
- **File**: `daylight_saving` on a location, `us_canada` or `eu`, left out for none; the
  reader refuses `none` written, another spelling and an unknown rule (docs/FORMAT.md).
  Merge describes a location with its rule.
- **App**: Plan > Locations has one compact select per row (None, US and Canada, EU);
  picking a rule for a place on the map sets it at once, otherwise Set does, with the
  rest. **Command line** `location ... --dst us_canada|eu|none`; **MCP** `set_location`
  takes `daylight_saving` and `locations` gives it. Left out on the command line or over
  MCP, a location keeps its rule.
- Tests: `solar::tests::daylight_saving_switch_dates` (2024 to 2028, both rules),
  `solar::tests::sunrise_across_a_switch` (New York and London across the forward
  switch, New York back), `timeline::tests::the_sun_follows_a_locations_daylight_saving_rule`,
  the format's plan round trip, the app's locations test and the command line and MCP
  beyond tests.

## 194. A category's elements as a strip field (2026-10-14)

Decision 87 left "element lists by category as strip fields" for later, and decision 185
still listed it; the roadmap's M1 known gaps row. Our own design, on decision 147's
pattern for the element count; not checked in the reference app.

- **A new strip field, Category elements** (`StripField::CategoryElements`,
  `category_elements` in the file): the scene's elements of the column's one category,
  `LAMP, ROPE`, in board ID order then name, elements left off the board left out, as on
  every strip. The column's category is the same `category` the element count picks
  (decision 147); with none, the cell is empty. Wide by default.
- **Names or board IDs, a small switch** (`StripColumn::board_ids`, `board_ids` in the
  file, left out while off): board IDs, with the name where an element has none, as the
  cast and Elements fields show them. Refused on any other field.
- **The heading is the category's name** (`Props`), so a strip layout can say what the
  list is; with no category it reads Category elements.
- **Once per layout**, as every sheet field but the spacer and custom text: a second
  category is a later step if anyone asks.
- **Screen, print and the one-line report**: the board's cells come from Rust
  (`reports::column_cell`), which the printed board draws too. The one-line report that
  follows the board's layout (or a named one) keeps the column's category and switch
  (`Schedule::report_columns`, `reports::report_cell`), on screen, printed and in its
  spreadsheet; its element count still counts every element (decision 147). A report's
  own columns and the calendar can't pick a category, so they don't offer the field
  (`StripField::of_sheet` leaves it out, as `SHEET` does).
- **The designer**: with Category elements shown, a part under the list picks its
  category as the element count's does (a search on category names, one radio pick),
  with a Board IDs box; its words say `Lists Props by name` (`listSay` in ui/js/logic.js,
  tested under Node). The count's and the list's parts are one function now.
- **Layout files keep the category by name** (`category_name`), as decision 147 does for
  the count: loading finds the schedule's category of that name, ignoring case; where
  there is none the list is empty. Deleting a category drops the pick (`tidy_counts`).
- MCP has no tool that sets strip columns; its reads and writes go through the format.
- Tests: `reports::tests::a_column_lists_one_categorys_elements`, the app's
  `board::tests::a_category_elements_field_lists_one_category`,
  `strict_tests::a_category_elements_column_round_trips`, and the page's `listSay` test.

## 195. Fountain at 56 lines a page, and a setting for it (2026-10-15)

The owner's answer of 2026-10-15 to ROADMAP owner question 1 and check E-I2: 56 lines a
page, so an eighth is 7 lines, with a preference to change it. It replaces decision 9's
55; the rest of decision 9 (wrap widths, blank lines, rounding to the nearest eighth, no
scene under 1/8) stands. Not from a behavior note; E-I2 is settled by the owner, and
comparing with a screenwriting app's lengths only tunes the default now.

- **The engine**: `fountain::LINES_PER_PAGE` is 56; `fountain::parse_with(text, lines)`
  paginates at another count, held to 40 to 70 (`MIN_LINES_PER_PAGE`,
  `MAX_LINES_PER_PAGE`); `parse` is `parse_with` at 56. Final Draft files keep their own
  lengths. Schedules already imported keep the lengths they have.
- **App**: Settings has **Fountain lines per page** (40 to 70, default 56), kept with the
  week start and date format in `settings.json` (`prefs.fountain_lines`), never in the
  schedule. Import Script and Import Script Revision use it. A value out of range in the
  file falls back to 56 and leaves the other settings; the dialog refuses one with
  `Fountain lines per page are 40 to 70.` (`fountainLines` in ui/js/logic.js).
- **Command line**: `import` and `revise` take `--lines-per-page N` (40 to 70).
  **MCP**: `import_script` and `revise_script` take `lines_per_page` (40 to 70).
- Tests: `fountain::tests::lines_per_page_default_to_56_and_can_be_set`,
  `a_full_page_of_action_is_one_page` at 56, the far-down length test at 56, the app's
  settings and `preferences_are_checked` tests, the command line's `lists_a_script`, the
  MCP's import test, and the page's `fountainLines` test.

## 196. Releases: a signed and notarized Mac build made locally; CI drafts unsigned ones (2026-10-15)

The owner's answers of 2026-10-15 to ROADMAP owner questions 2 and 3: the Mac build is
signed with the company's Developer ID (team XSRB3D643J, the publisher name the owner is
fine showing, KICKOFF section 3), and Windows ships unsigned for the first alpha, with
the SmartScreen warning accepted. Not from a behavior note.

- **`scripts/release-mac.sh`** (bash, Xcode's tools and the Tauri CLI only): builds the
  release app and `.dmg` with the Tauri bundler (`cargo tauri build --bundles app dmg`;
  the CLI pinned at 2.11.4, `cargo install tauri-cli --locked --version 2.11.4`, which
  the script checks for), signed by `METHOD_SIGN_IDENTITY` (default the company's
  Developer ID Application identity) with the hardened runtime and a secure timestamp,
  which the script checks in the signature. It signs the `.dmg` with a timestamp, runs
  `codesign --verify --strict --deep`, then notarizes the `.dmg` with `xcrun notarytool
  submit --wait --keychain-profile "$METHOD_NOTARY_PROFILE"` (default
  `method-schedule-notary`), staples the `.dmg` and the app, and checks them with `spctl
  --assess` and `xcrun stapler validate`. The version comes from the workspace's
  `Cargo.toml`; the results go to `target/release-mac/`. Nothing is uploaded but to
  Apple's notary service.
- **No entitlements file**: the app needs none under the hardened runtime. WKWebView runs
  the page's JavaScript, JIT included, in WebKit's own processes, which Apple signs; the
  app makes no JIT code, loads no unsigned libraries and asks for no camera, microphone
  or network. Tauri's macOS signing guide asks for none either. One is added only if a
  later feature needs a specific permission.
- **The notary profile is the owner's, made once**: `xcrun notarytool store-credentials
  method-schedule-notary --apple-id <the Apple ID> --team-id XSRB3D643J`, typing an
  app-specific password at the prompt. Without it the script stops after signing and
  verifying, before anything is sent, and prints that command.
- **The bundler is told not to notarize** (Apple ID and API key variables cleared) and to
  make a plain `.dmg` without driving Finder (`CI=true`), so the script does each step
  once, in order.
- **`.github/workflows/release.yml`**: on a `v*` tag (or by hand with a tag), checks the
  tag is `v` plus the version, makes a draft release with the runner's `gh`, and builds
  on macOS (`.dmg`), Windows (NSIS `.exe` and `.msi`) and Linux (AppImage and `.deb`),
  attaching each to the draft. Unsigned unless signing secrets exist; the signing,
  notarizing and Windows signing steps are there but skipped without them. The secrets it
  would read are listed at its top; none are set. Actions pinned to SHAs as in `ci.yml`;
  only the jobs that write the draft get `contents: write`. Publishing stays by hand.
- **Not done here**: the repository stays private and the site off (decision 197).
- First run (2026-10-15, on the owner's Mac, 0.1.0 for Apple silicon): the bundler signed
  the app with the Developer ID, hardened runtime on and a secure timestamp; the `.dmg`
  was signed; `codesign --verify --strict --deep` passed on the app, the `.dmg` and the
  app inside it; the script then stopped before notarizing, with no profile yet, as
  designed.

## 197. The updater key, made when the repository goes public (2026-10-15)

The owner's answers of 2026-10-15 to ROADMAP owner questions 4 and 5: the repository going
public, the website, Discussions and the support email are deferred, and the exchange file
freezes at version 1 after Method Budget's first build reads it (that is being worked out
with Method Budget apart from this repository). The updater stays off until the
repository is public (KICKOFF item 12); this is how its key will be made and kept, so it
is ready. Not from a behavior note.

- **Its own minisign key**, never Method Budget's: made with `cargo tauri signer generate
  -w ~/.tauri/method-schedule.key` on the owner's Mac, with a passphrase the owner picks
  and types at the prompt.
- **The owner keeps the private key and its passphrase**: in the owner's password manager
  and an offline backup, never in the repository, a chat, a log or a shared folder. If it
  is lost, installed copies can't take an update signed by a new key; if it leaks, anyone
  could sign one. Either way the fix is a release that ships a new public key, installed
  by hand.
- **The public key goes in `tauri.conf.json`** (`plugins.updater.pubkey`) with the
  updater plugin, pinned exactly as the other Tauri crates, in the same change that turns
  the updater on. The updater fetches over https only, from the release manifest the
  release workflow writes.
- **CI signs update bundles only from secrets** (`TAURI_SIGNING_PRIVATE_KEY`,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) the owner adds when the repository goes public;
  until then no key exists and nothing is signed for the updater.
- Nothing about the site's live state, `pages.yml` or the repository's visibility changes
  here.

## 198. A sample schedule shipped with the app (2026-10-15)

The roadmap's M2 row "A sample schedule shipped with the app". Our own design; nothing
in it comes from any real production or from the reference app.

- **The production**: *Saltmarsh Road (sample)*, an invented feature by an invented
  company, with eight invented cast (holds and drops on, a seven-day minimum gap, so one
  drops and others hold), props, picture vehicles, an animal and background with daily
  quantities; 37 scenes with synopses; a board, *Shooting Order*, with a main unit of
  eleven shoot days from Monday 2027-04-05 and a second unit of two, and two scenes in
  the Boneyard; two banners (a block heading and a company move with a duration); a
  holiday on both units; a fitting before the start, travel days either side of a day
  player's work and a rehearsal on a held day; a hard Unavailable red flag that clashes
  with a shoot day; two locations placed on the map with the US and Canada daylight
  saving rule, so sunrise and sunset show; strip colors for dawn, dusk and the usual four
  pairs; production information.
- **Hand-built, not generated**: `method_schedule_core::sample::demo`, a small table in
  shooting order (scenes, day breaks, banners), so every count in it is one a person can
  check by eye. The seeded generator `sample(n)` stays for performance work.
- **Built into the app**: `crates/method-schedule-app/resources/sample.mtsc`, compiled
  in (`include_str!`) rather than shipped as a bundle file: no path to resolve at run
  time, nothing to go missing, and nothing the app could write over. A test holds it to
  `to_string(demo())` byte for byte, reads it with the strict reader and runs the check;
  when the demo or the format changes, the file is written again with the command line.
- **Help > Open Sample** opens it as a new, untitled schedule in this window (the
  unsaved-changes question first when anything is unsaved, as File > New does). Nothing
  is unsaved, autosave has no file to write, and the first Save asks where. The title
  keeps the production's name; the window says Untitled until it is saved.
- **Command line**: `method-schedule sample --demo FILE` writes the same bytes (refuses
  to overwrite, as `sample` does).
- Tests: `sample::tests::the_demo_is_valid_stable_and_shows_a_little_of_everything` (one
  red flag clash on day 9), the app's `help::tests::the_sample_is_the_engines_demo_and_reads_cleanly`
  and `the_sample_opens_untitled_with_nothing_unsaved`, the command line's
  `writes_the_apps_sample` and the menu tests.

## 199. The user guide (2026-10-15)

The roadmap's M2 row "The user guide". Our own words throughout.

- **docs/GUIDE.md**: short sections for a scheduler: getting started (open the sample),
  coming from another scheduling app, the breakdown, the board and day breaks, the
  calendar and events, the day out of days in plain words (holds, the one drop in the
  longest gap, holidays and company travel days, events on held days and at a drop's
  edge, from decisions 10, 11 and 139), reports and exports, red flags and conflicts,
  merging and working together (outside changes, What Changed, Merge Changes From, the
  git driver), the budget exchange file, keyboard shortcuts and files and backups.
- **The other app is not named**: the switching section says "another scheduling app",
  as BRAND.md asks of public material, and names only the industry's own words
  (breakdown sheets, stripboard, Boneyard, day out of days) and says plainly that its
  files don't open here.
- **Help > User Guide** shows the guide in the app: Rust sends the text built into the
  binary (`user_guide`, `src/help.rs`), and the page shows it read-only in a dialog of
  its own, the sections down the side (a click scrolls to one). The Markdown is read by
  two small functions in logic.js (`guideBlocks`: headings, paragraphs, lists;
  `guideRuns`: bold, italics, code, a link's words only) into text nodes, never HTML.
  No browser, no network, no new capability beyond the one command.
- **The site**: scripts/site.py builds no docs pages, so the guide is not wired into the
  site.
- Tests: the page test `the user guide reads as headings, paragraphs and lists, never
  HTML`, which also holds the real guide to the house voice, and the app's
  `help::tests::the_guide_keeps_the_house_voice_and_covers_its_sections`.

## 200. Crash recovery as tests (2026-10-15)

The roadmap's M2 row "Crash recovery checked by hand". What APP.md "Saving and recovery"
promises is now tested against the saving code itself (`src/safety_crash.rs`).

- **Killed mid-edit**: the test starts its own test binary as a child process, which
  edits an untitled schedule and a saved one with Autosave off, lets autosave keep them in
  recovery copies, makes one more edit and waits; the parent kills it (`SIGKILL` on Unix,
  so no destructor or exit hook runs). The dead run's lock file is left, unlocked; the
  next run tidies it away, offers both copies, and reopens each unsaved, the saved one
  pointing at its file with the first Save asking; the file on disk is the version last
  saved, whole.
- **A full disk**: a stand-in writer (`FullDisk`) takes a few bytes, or none, then fails
  as a full disk does (`ENOSPC`), switched on for one folder in tests only. A save leaves
  the old file whole and no temporary file; Save and autosave say `the disk is full.
  Nothing was written, so the file is as it was.` and the work goes into a recovery copy
  that outlives a crash and saves once there is room.
- **Locked or read-only: refused, a change.** Before, a read-only file was replaced (a
  rename over it works on Unix, and the new file even kept the read-only mode). Now a
  document write refuses a file that is read-only or locked in the Finder (the immutable
  flags on macOS) before anything is written (`safety::is_locked`); a folder that can't
  be written to fails on its own. Both say `the file or its folder is locked or read-only.
  Nothing was written. Use Save As to keep your changes in another file.`
  (`safety::write_problem`), autosave keeps a recovery copy, and Save As elsewhere works.
  The refusal covers every document write: saves, exports, templates.
- **Changed outside, then a crash** (the path of decisions 179 to 182): autosave pauses
  and keeps a recovery copy; after the crash both versions survive, and the reopened copy
  never writes over theirs unasked.
- **Still by hand**: the real window. Killing the app itself (its autosave timer, the
  exit hook it skips) and seeing the Recovered schedules dialog, the refusal messages on
  screen and the outside-change question on focus. `scripts/crash-check.sh` builds the
  debug app, opens it on a temporary copy of the sample, kills it with `kill -9`, locks
  and changes the file and says what to do and look for at each step; it can't drive the
  window, so it needs someone at the screen.
- Tests: `safety_crash::tests::a_process_killed_mid_edit_leaves_copies_the_next_run_reopens`,
  `a_full_disk_mid_save_leaves_the_old_file_whole`,
  `a_locked_or_read_only_file_is_refused_and_the_work_kept` (Unix) and
  `a_file_changed_outside_then_a_crash_keeps_both_versions`.

## 201. The exchange file at version 1: what is left out, and where prep and wrap sit (2026-10-15)

Method Budget's reader read version 0 of the budget exchange file and sent its findings
(relayed on 2026-10-15); the owner froze the file at version 1 once that reader works
(decision 197). Not from a behavior note.

- **`first` and `last` are optional**: left out when the element has no strip day on the
  sub-board (it is in the file only for a counted event, a fitting before the shoot say),
  and always together. The writer already did this; the spec listed them as plain dates.
  The format has no nulls, so leaving a field out is how it says "none".
- **Absent, never `""`**: `board_id` is left out when the element has none, and a blank
  one (spaces only) counts as none. `category` is written for every element of a schedule
  that passes its checks (an element's category must exist and have a name) and is left
  out rather than written empty if that ever fails.
- **Prep start and wrap end stay inside `shoot_days`**: they are dates of one sub-board's
  schedule, set per sub-board like `start` and `end`, which already sit there; a separate
  object would hold two dates and break version 0 readers that already find them in
  `shoot_days`, for no gain.
- **No nulls, whole numbers**: the spec's "every number is days" is replaced by the
  accurate rule: no nulls; counts are whole days; `version`, `id` and `min_drop_gap` are
  the other integers. An element's `total` can exceed the shoot days, since holidays,
  company moves, travel, fitting, rehearsal and holds count on days that aren't shoot days.
- Tests: `exchange::tests::leaves_out_what_an_element_lacks_rather_than_write_empty_values`;
  the fuzz test's exchange check now also refuses a null anywhere, an empty `category` or
  `board_id`, `first` without `last`, and a dates list that isn't one per shoot day.

## 202. `days_hash` and the shoot dates list (2026-10-15)

Method Budget's findings on version 0 (decision 201). Not from a behavior note.

- **`days_hash`**, after `source_hash`: FNV-1a 64 as 16 hex digits over the bytes of
  `{"shoot_days":…,"elements":[…]}`, the file's own two values as compact JSON in the
  file's order. It moves when a shoot day, a date, a count, an element's name, category,
  board ID or switches, or the set of elements moves; not with the title, the stamped
  date, a synopsis or the view, which all move `source_hash`. Both stay opaque: compared
  for equality only.
- **Kept stable**: its coverage is fixed at version 1. A field added later inside
  `shoot_days` or an element is left out of it, so an unchanged schedule keeps its hash
  across writer versions. A reader can check it from the file's text (whitespace outside
  strings removed, from `"shoot_days":` to the end, with a `{` in front) but needn't.
- **`shoot_days.dates`**: the date of every shoot day, in order, one per day (empty with
  none), so a budget can regroup by any week, month or pay period, not only `by_week`'s
  Mondays. Per element dates are not in version 1; they may come later as an optional
  field on each element.
- Tests: `exchange::tests::days_hash_moves_with_the_days_and_not_with_other_edits` (stable
  across a synopsis, view, title and stamped date edit; changed by a later start and by a
  hold switch), `days_hash_is_the_compact_shoot_days_and_elements` (the definition worked
  from the file's text), `lists_every_shoot_date_in_order` (one per day, from `start` to
  `end`, regrouping to `by_week`).

## 203. The exchange file frozen at version 1, with its reading and identity rules written down (2026-10-15)

Method Budget's findings on version 0 (decision 201) and the owner's answer of 2026-10-15
(decision 197). Not from a behavior note.

- **Version 1, frozen on 2026-10-15**: the writer writes `"version": 1`. Changes from here
  are additive only; anything that would break a reader gets a new `format` name. The spec
  lives in both repositories: docs/EXCHANGE.md here, and the copy the Method Budget
  repository keeps beside its reader.
- **Reading**: a reader checks `format`, takes versions 0 and 1 (and later ones, which only
  add), and ignores fields it doesn't know. No reader in this repository reads the file;
  it only writes it.
- **`by_` keys in byte order** of their UTF-8 text, as written since version 0 (names are
  trimmed and compared exactly, so two spellings are two keys). No behavior change.
- **One sub-board per file**: the file names its board and sub-board, and a budget picks
  which file drives its shoot days. A multi-sub-board file is not planned for version 1.
- **Element identity, as the model has it**: ids come from the schedule's one `next_id`
  counter and are never handed out again after a delete, except that undoing an add takes
  the counter back too; copies (Save As, a template) share ids and then count apart; a
  merge keeps ours' ids and renumbers theirs' new things only on a clash; Elements tab
  Merge keeps the survivor's id; script import gives new ids, and a revised draft keeps
  matched elements' ids. (category, board ID, name) picks out one element within a file
  but is free text and not unique across files or over time; category names can be renamed.
- Docs: EXCHANGE.md rewritten for version 1 (status, reading rules, the hashes, element
  identity); APP.md's Export for Budget says the stamped date is local, as it has been
  since `local_today`; GUIDE.md's exchange section names both hashes and the dates;
  HANDOFF.md and the roadmap's status line say version 1.

## 204. Look A, Paper: the brand family's look in the app (2026-10-16)

The owner's choice of 2026-10-16, from three looks drawn for the same board at 1280 x 800
(the design canvas: A Paper, B Graphite, C Ledger). A visual pass only: every screen,
control and key stays where it was, so the parity checks stand.

- **Colors**: styles.css takes BRAND.md's tokens: the warm paper and ink neutrals and
  dark surfaces shared with Method Budget, the slate blue accent, an exterior day strip
  a little warmer (`#fcf1be`). New tokens `--surface-2`, `--text-soft`, `--line-strong`,
  `--accent-strong` and `--ok`; every old token name stays.
- **Top bar**: New, Open, Save, Save As, Undo and Redo are stroke icons (`.icon-btn`,
  `.ico`); each keeps its id, its tooltip with the key and gets an `aria-label`. Import
  Script stays a word. The views move into the top bar as one segmented control
  (`.tabs` inside `.topbar`, same ids and roles), which saves the tab row.
- **Footer**: the status line moves from under the board bar to the left of the footer,
  with the counts and the Autosave switch on the right. The footer is one fixed line, so
  decision 48 holds: a message never moves the board. Errors keep their red ground.
- **Density**: the chrome is 94 px at 1280 x 800 (131 before), 37 px more board.
- **Not in this pass**: the native window (seen by the owner on 2026-10-16, looks right).
  The app icon from the mark came next, in 205, and the set-apart scene cell in 206.

## 205. The app icon drawn from the mark (2026-10-16)

The icon the design canvas showed with look A (decision 204): the white stripboard mark on the slate blue square, in place
of the placeholder (a dark slate square with three bands). Not from a behavior note.

- **One drawing**: `scripts/make-icons.py` loads `scripts/make-brand.py` and draws its
  `mark_rows` and `fav_rows`, slate blue `ACCENT` and PNG and .ico writers, so the icon
  cannot drift from the mark. Same files as before (32x32.png, 128x128.png,
  128x128@2x.png, icon.png, icon.icns, icon.ico); tauri.conf.json is unchanged.
- **Shape**: the square is the mark's 64-unit grid with corners of 14.3 units (Apple's
  icon proportion); the day break is full white, the other strips white at 55%, a touch
  over the mark's half strength, which reads dim on the solid blue.
- **Sizes**: from 48 pixels up the square sits inside the macOS icon grid's margin (100
  of 1024 a side). At 16 and 32 pixels it is full bleed and draws the 16-unit favicon
  rows, which land on whole pixels there; the 64-unit rows fall on half pixels at 32.
- Docs: BRAND.md's app icon paragraph describes the icon; APP.md drops the "Not yet"
  entry about placeholder icons.

## 206. The strip's first cell set apart, as in the mark (2026-10-16)

The last piece of look A (decision 204). The mark draws each strip with its first cell, the
scene number, set apart; the board now does too. Not from a behavior note.

- **Which cell**: the first line's first field, whatever the strip layout puts there (the
  same cell the conflict and red flag marks lead, entry 83). board.js gives it the class
  `lead`; nothing else changes in the row.
- **How it looks**: a faint tint over the strip color (`--lead-cell`, ink at 4.5% light,
  white at 5% dark), reaching the strip's left edge, a rule on its right (`--strip-rule`),
  the text semibold. Its line height is the board line's (22 px), so on a strip of two to
  four lines it fills its own row. A selected strip keeps its blue bar on top of it.
- **Screen only**: the printed board and reports draw strips as table rows and are
  unchanged (print.css keeps its own density rule, decision 188).
- Test: `logic.test.mjs` checks the class and the rule.

## 207. The printed call sheet (2026-10-16)

The owner's pick of 2026-10-16 for code-only work (ROADMAP M5, "Designed call sheets"),
laid out from the canvas mockup in look A (decision 204). Not from a behavior note.

- **What it prints**: exactly the call sheet data (docs/CALLSHEET.md, D70), nothing the
  schedule doesn't hold: no crew, weather, hospital or map. The opening (title, company,
  director, producer, board; shoot day of days and date; call, the largest thing on the
  page, and wrap; length, pages, scenes, sunrise and sunset and where), then scenes, cast
  (on set from first start to last end), elements (a line per category) and notes.
  Banners are in the notes with their times, as in the data, not in the scene list.
- **How**: a new command `call_sheet_data` returns the JSON export parsed; `callsheet.js`
  turns it into report.js flows, so a long day pages like every report, headings repeated,
  "Page X of Y". Print and Save as PDF go through `printPages`, the job title
  "<title> - Call Sheet Day N" (the PDF's file name).
- **Ink only**: rules and weight, no fills (the mockup's dark call box became a heavy
  outline), so it copies and faxes cleanly; a page test holds the rule.
- **Menu**: Plan > Print Call Sheet..., above Export Call Sheet Data...; both share the day
  picker (the selected strip's day first).
- Tests: the command's JSON in `plan.rs`; the words (date, length, times, sun) in
  `logic.js`, tested in `logic.test.mjs`; the harness has a fixed stand-in for the data.
- Docs: CALLSHEET.md's new section, APP.md's command and file lists, GUIDE.md, the
  feature list (have), the roadmap's call sheet row now the parts still to come.

## 208. Three looks: Paper by default, Graphite and Ledger in Settings (2026-10-16)

The owner on 2026-10-16: "I like A but we should include B and C as optional themes and
settings." The three looks drawn on the design canvas (decision 204) all ship; Paper
stays the default.

- **The setting**: Settings > Look, kept in settings.json with the other display
  preferences (`theme`: `paper`, `graphite` or `ledger`; anything else reads as Paper).
  Every window follows a change at once (`prefs-changed`). A line under the picker says
  what each look is.
- **Paper**: the stylesheet as written, light or dark with the system.
- **Graphite**: always dark. A strip's color is a 4 px edge and a faint tint; day breaks
  are light bars; column heads and day heads in small capitals; the views are underlined
  words instead of a segmented control.
- **Ledger**: always light, like a printed board: full strip colors, black rules, a black
  column head, the first cell in monospaced bold, day breaks in capitals.
- **One layout**: a look changes colors and a few borders only. Strip lines stay 22 px in
  every look (the mockup's 20 px Graphite lines would break the board's drawing of only
  the lines in view and the printed density, decisions 24 and 188); a page test holds it.
  Printing keeps its own colors whatever the look.
- **How**: `data-theme` on `<html>` (none for Paper); each look redefines the tokens,
  which outweighs the dark-mode block, plus a few rules of its own.
- Tests: the loader takes a known look and drops an unknown one (`safety.rs`); the page
  test checks the list, the attribute and that no look changes a line's height.

## 209. Suite standard, repo hygiene: cargo-deny, notices, pinned checkout, no Pages (2026-10-16)

The owner adopted the suite's plumbing standard (07_Method_Production_Suite/SUITE-STANDARDS.md,
section 6) on 2026-10-16; this is its repo-hygiene part for Method Schedule.

- **cargo-deny**: `deny.toml`, Method Budget's file (advisories, licenses compatible with
  MIT OR Apache-2.0, crates.io only), without Budget's rustls ignores (they are for its
  updater, which we don't have yet) and without the ISC and CDLA allowances our tree
  doesn't need; a new `deny` job in ci.yml runs it with Budget's pinned action.
- **Third-party notices**: `THIRD-PARTY-NOTICES.md`, written by `scripts/notices.py` from
  `cargo metadata --locked`: every crate the app and the command line reach through normal
  dependencies, the license we take it under (the first of MIT, Apache-2.0, Zlib, ISC,
  BSD-3-Clause, Unicode-3.0, MPL-2.0 its expression offers) and its copyright lines, then
  each license's text once. `cargo metadata` unions features across build and normal
  dependencies, so it may list a few crates a build leaves out (libloading 0.7.4 is one);
  listing too many is the safe side. The `deny` job runs `notices.py --check` after
  `cargo fetch --locked`. The app's Licenses window shows the file (decision 212).
- **Checkout pinned** to `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`
  in every workflow, the SHA every suite repo uses.
- **No Pages**: methodschedule.com is served by the suite website, which copies `site/`
  from what is pushed to origin/main. `site/CNAME` and `.github/workflows/pages.yml` are
  gone; WEBSITE.md's publishing section says how the sync works; the roadmap's website
  row keeps only what is left (public repository, Discussions, support email, security policy).

## 210. Suite standard, releases: Apple silicon, the suite's secret names, version-free files (2026-10-16)

The release part of the suite standard (SUITE-STANDARDS.md, section 3), adopted by the
owner on 2026-10-16. release.yml rewritten in Method Budget's shape, without its `xtask`.

- **Still a draft** for a person to publish, on a `v*` tag that matches the workspace
  version; new: the tagged commit must be on main (Budget's check).
- **macOS: Apple silicon only** (`aarch64-apple-darwin` on `macos-latest`). Intel Macs
  keep the version they have. `scripts/release-mac.sh` still builds universal on request.
- **The suite's secret names**, Budget's: `DEVELOPER_ID_CERT_P12_BASE64`, `CERT_PASSWORD`,
  `KEYCHAIN_PASSWORD`, `AC_API_KEY_ID`, `AC_API_ISSUER_ID`, `AC_API_KEY_P8_BASE64` (they
  were `APPLE_*`; none had been set). The signing identity is read from the imported
  certificate and masked, so no company name is written in the repository; Budget writes
  it. The Tauri CLI signs and notarizes the app; the workflow signs, notarizes and staples
  the `.dmg`, then verifies.
- **Windows signtool step kept**, now the suite's model (`WINDOWS_CERTIFICATE`,
  `WINDOWS_CERTIFICATE_PASSWORD`), skipped until a certificate exists. The `.msi` is no
  longer built: the standard lists one Windows installer, the NSIS setup.
- **Version-free names**: `Method-Schedule-macOS.dmg`, `Method-Schedule-windows-x64-setup.exe`,
  `Method-Schedule-linux-amd64.deb`, `Method-Schedule-linux-x86_64.AppImage`, so the
  website links `releases/latest/download/<file>`. A build that makes more or fewer than
  one of each fails.
- **Notes**: `docs/releases/<version>.md` when it exists, else a placeholder asking for it.
- Only the draft job writes; first-party actions pinned to SHAs (checkout v7.0.1, and
  upload-artifact and download-artifact at Budget's SHAs); actionlint is clean.
- The updater archive and `latest.json` join this workflow with the updater (decision 213).

## 211. Suite standard, identity: type identifiers, bundle metadata, `--version` (2026-10-16)

The identity part of the suite standard (SUITE-STANDARDS.md, section 2), adopted by the
owner on 2026-10-16.

- **Type identifiers** `com.method<noun>.<ext>`, Method Budget's pattern: `.mtsc` is
  `com.methodschedule.mtsc` (it was `com.methodschedule.desktop.mtsc`). No file has gone
  out, so nothing keeps the old one. `.mtst` templates are registered too, as
  `com.methodschedule.mtst` (media type `application/vnd.method-schedule-template+json`);
  a template opened from the system starts a new, untitled schedule, as File > Open does
  (entry 85). The bundle identifier stays `com.methodschedule.desktop`.
- **Bundle metadata**: copyright "Copyright (c) 2026 Method Production Suite Team and the
  Method Schedule contributors" (LICENSE-MIT's line), publisher "Method Schedule
  contributors" (no company), homepage https://methodschedule.com, and a long description.
- **`method-schedule --version`** prints `method-schedule X (Method Schedule)`; the MCP
  server's title is "Method Schedule" (it still said "Scheduling (working name)").
- Tests: `the_bundle_follows_the_suite_identity` (help.rs) holds the identifiers, homepage,
  copyright (against LICENSE-MIT) and a company-free publisher;
  `version_names_the_command_and_the_product` (the CLI).

## 212. Suite standard, the Help menu: About, What's New, Report a Bug, Licenses (2026-10-16)

The Help part of the suite standard (SUITE-STANDARDS.md, section 4), adopted by the owner on
2026-10-16. Check for Updates and its automatic switch come with the updater (decision 213).

- **About Method Schedule** now carries the version, the website (a link on Windows and
  Linux, the credits line on macOS), the copyright line and the licence, as Method
  Budget's does. On Windows and Linux it ends the Help menu.
- **What's New**: `docs/releases/<version>.md`, copied into the build by build.rs (an
  empty file when there are none yet, so a build never fails for want of notes), shown by
  the guide's sheet. With no notes the status line says so.
- **Report a Bug…**: the bug form (`.github/ISSUE_TEMPLATE/bug.yml`) with its `version` and
  `platform` fields filled in by query string; nothing else goes in the address. Reader's
  `form_encode`. The status line asks for an invented schedule, never a real one.
- **Licenses and Notices**: LICENSE-MIT, LICENSE-APACHE and THIRD-PARTY-NOTICES.md (decision
  209), built in, one at a time in a sheet beside a list.
- **methodproduction.com**: the suite's website.
- **Links are a fixed list**: `open_link` takes a name (`website`, `suite`, `bug`), never an
  address, and opens it with the system's opener (`open`, `xdg-open`, `rundll32`), so no
  new dependency. A test holds the list and refuses anything else.
- Tests in help.rs: the links and the bug address; the licenses built in. The menu's own
  tests already hold every new item against shortcuts.js, build.rs and the capability.

## 213. Suite standard, updates: the updater built and inactive until the key exists (2026-10-16)

The updates part of the suite standard (SUITE-STANDARDS.md, section 1), adopted by the owner
on 2026-10-16: automatic checks with an off switch, Method Budget's way. Decision 197 kept
updates off until the repository is public; the wiring lands now and stays inactive until
the owner makes the key, so when to turn it on stays the owner's call.

- **The plugin** `tauri-plugin-updater` 2.12.0 (rustls), with `reqwest` 0.13.5 for the
  redirect rule and `semver` 1.0.28, Method Budget's versions; four crates the resolver
  picked newer (rustls, rustls-native-certs, schannel, zeroize) are pinned to Budget's,
  outside the 14-day cooldown. deny.toml takes Budget's four rustls advisory ignores (their
  fix is inside the cooldown; drop them on or after 2026-10-28) and allows ISC and
  CDLA-Permissive-2.0 again; the notices name the new crates and the CDLA text.
- **src/updater.rs**, Budget's: the same state machine (idle, checking, downloading, ready,
  installing; a check the user asked for answers, an automatic one is quiet), the first
  check two minutes after launch and then every six hours, `updates.json` in the data
  folder (`automatic`, on by default, and the version whose What's New was `seen`), and
  requests kept on https and on the endpoint's hosts (github.com and its two download
  hosts), redirects included. Not ported: Budget's own minisign key parser (an empty key
  is what "no key" means here) and its tabs; Restart to Update goes the way Quit does
  (`confirm_close` on every window), then installs and restarts.
- **Inactive** (no timer, no network) while `plugins.updater.pubkey` is empty, and on
  Windows and Linux (updates come later there). A check the user asks for then says the
  copy doesn't update itself yet and points at methodschedule.com.
- **The manifest**: `https://github.com/method-production-suite/method-scheduling/releases/latest/download/latest.json`.
  release.yml builds the Mac `app,dmg`, and with `TAURI_SIGNING_PRIVATE_KEY` (and its
  password) plus a public key in the app, makes `Method-Schedule_<version>_aarch64.app.tar.gz`
  and its `.sig`; `scripts/update-manifest.py` writes `latest.json` (Tauri's format,
  `darwin-aarch64` only, the URL the release's own) and refuses a malformed signature, a
  missing archive or an app with no key.
- **The menu and the page**: Help > Check for Updates… and Check for Updates Automatically
  (a check item) are answered in Rust. The footer shows "Update ready: X" with Restart to
  Update; the status line gives a manual check's progress and answer (`updateMessage` in
  logic.js); What's New shows once after an update (`update_whats_new`).
- **Copy**: the site and APP.md now say the app goes online only to check for updates,
  which can be turned off (the standard's wording).
- **For the owner**: `cargo tauri signer generate -w ~/.tauri/method-schedule-updater.key`;
  the private key and its password into 1Password as "Method Schedule updater key" and the
  repository secrets; the public key into `tauri.conf.json`.
- Tests: the state machine, availability, hosts, What's New, settings and the endpoint
  (updater.rs); the page's messages (logic.test.mjs).

## 214. The updater's key is in: Mac builds check, updates wait on the owner's steps (2026-10-16)

The owner made the updater key on 2026-10-16 (decision 213's command): minisign key
`460428DB6D38CE76`. Its public half is now `plugins.updater.pubkey` in `tauri.conf.json`,
copied from `~/.tauri/method-schedule-updater.key.pub`; the private key and its password
stay with the owner (1Password, "Method Schedule updater key") and were never read here.

- **What changes**: a Mac build now checks two minutes after launch and every six hours,
  as decision 213 describes. While the repository is private GitHub answers "not found";
  an automatic check keeps that to itself, so nothing shows, and Help > Check for Updates…
  says it couldn't check. Windows and Linux still don't check.
- **What still turns updates on**, all the owner's: the release secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the Apple signing
  secrets, so an update installs a notarized build; the repository public (decision 197);
  a release tagged on main, its draft checked for `latest.json`, then published as a full
  release. APP.md, "Updates and releases", lists them.
- Only builds that carry this key can update, so the first build people install must be
  made from here on.
- Test: `the_endpoint_is_the_latest_release_and_the_key_is_the_owners` (updater.rs) holds
  the endpoint and the key's id.

## 215. Suite standard, pictures and releases: Field Station, pictures that retake themselves, one notes template (2026-10-16)

Section 7 of the suite standard (SUITE-STANDARDS.md), from the Marketing brief, adopted by
the owner on 2026-10-16. Method Reader's capture tool is the model.

- **Field Station** is the suite's one invented production: Storyteller writes it, Reader
  reads it, Schedule breaks it down, Budget budgets that schedule. Our copy of the script is
  `fixtures/scripts/field-station.fountain`, made from Reader's scene data with a neutral
  title page ("an invented script for the Method Production Suite"); Reader's own title
  page, which names its old project, is never copied (a test holds that). Eleven scenes,
  four speaking parts (MARA, DELACROIX, TEO and IVERSEN on the radio).
- **The schedule**: `method_schedule_core::sample::field_station()`: the script imported,
  props, set dressing, wardrobe and effects on their scenes, two invented locations placed
  on the map, four shoot days from Monday 3 May 2027 (day exteriors, interiors, nights) with
  banners. `method-schedule sample --field-station FILE` writes it;
  `fixtures/field-station.mtsc` is it byte for byte (a test). Its budget exchange file is
  `fixtures/field-station.budget.json` (version 1, a test rebuilds it with its own date);
  Method Budget's Field Station budget is built on it.
- **Pictures that retake themselves**: `node scripts/capture-screens.mjs` (WEBSITE.md,
  "Pictures"): four shots in the suite's order (the board as hero, the day out of days as
  the signature feature, an import, the file and the command line), 1280 by 800 at 2x,
  light and dark. Headless Chrome over the remote-control protocol with Node's own
  WebSocket, no dependency; the app's own page replays the engine's real answers
  (`examples/screen_answers.rs`) and refuses any unrecorded call about the schedule. GPU
  drawing varied a few pixels a run, so Chrome draws in software with whole-pixel text;
  two runs in a row then change nothing. Written to `assets/screens/` and `site/img/`, only
  when the bytes change. The day out of days stands for "stripboard plus day out of days":
  the app has no day out of days pane beside the board.
- **Where they show**: the README's hero and a "A look around" section; on the site the
  board replaces the HTML drawing of the window and a gallery holds the other three (the
  `.window` drawing's styles in site.css are left for now, unused).
- **Release copy**: `docs/release-notes-template.md` (Method Budget's, with the suite's
  order: what's new, Fixed, Known gaps, Thanks naming every helper, film knowledge
  included) and `scripts/notes-check.py`, which refuses a file that strays from it. The
  README has a platform status table. `docs/RELEASING.md` links the suite's
  RELEASE-CHECKLIST.md (drafted, not yet in the organization's `.github` repository) and
  adds this app's commands for each step.

## 216. Public as a fresh repository, one commit (2026-10-16)

The owner on 2026-10-16: make the repository public, the way entry 5 planned. A pre-public
audit found no secrets in the 316 commits but private working notes, vault names and a
personal commit address in the history, and GitHub keeps every pull request's commits
reachable for good, so rewriting `main` would not have hidden them.

- **Two repositories**: the private history is kept as `method-production-suite/
  method-scheduling-private`; `method-production-suite/method-scheduling` starts again,
  public, from one commit by "Method Production Suite" <hello@methodproduction.com>.
- **Left out of the public tree** (kept in the private repository and gitignored here):
  the kickoff brief, the session handoff notes, the security checklist naming the
  team's vaults, the environment example, and the naming board's trademark notes.
  Mentions of them in older entries stay as history.
- **Moved over**: the help-wanted labels and issues, Discussions, the copy pull request
  (on the new history), the v0.1.0 tag and release, the release secrets (set again by the
  owner: GitHub never reads secrets back). The research notes stay: they restate the
  reference app's behavior in our own words, with no copied text (CLEAN-ROOM.md).

## 217. Caching hints on the MCP server's lists (2026-10-16)

A tester wiring up the suite reported that the newest Claude Code refuses the MCP tool list
without `ttlMs` and `cacheScope` (routed by the suite's org session). Checked against the
specification: revision 2026-07-28 ("Caching"; schema `CacheableResult`) says servers
implementing it MUST put both on `server/discover`, `tools/list` and the other list and
read results; `ttlMs` a whole number of milliseconds, never negative, and `cacheScope`
`"public"` or `"private"`. Clients SHOULD treat a missing `ttlMs` as 0 (stale at once);
the fields don't exist in earlier revisions. `resultType: "complete"`, which every result
of this revision MUST carry, was already sent.

- **What Claude Code does**: 2.1.280 speaks 2026-07-28 (`server/discover`, then
  `tools/list` with `_meta`); a health check asked for the tool list four times while the
  hints were missing, as for a list that is always stale.
- **The answer**: on 2026-07-28 only, `server/discover` and `tools/list` carry
  `ttlMs: 3600000` and `cacheScope: "public"`: the list is fixed for the life of the
  process (`listChanged: false`) and the same for every caller of a local server. Tool
  calls carry neither. Earlier revisions are unchanged.
- Agreed with Method Budget so both servers answer the same way.
- Test: `modern_lists_carry_caching_hints_and_legacy_lists_do_not` (mcp/tests.rs); the
  replies to Claude Code's recorded requests were checked before and after by hand.

