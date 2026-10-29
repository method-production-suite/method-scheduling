# Method Schedule in a browser: the window-free session (plan)

Status: **built** (2026-10-20; decision 222). The session crate, its clock and
`cargo xtask wasm-check` are in; every window-free module has moved; every command that
needs only the schedule has one body in `method_schedule_session::commands`, called by the
desktop's Tauri command and by the browser host's dispatch alike, and the file commands
share their file-free halves (`commands::files`); `method-schedule-web` and its shim run
the page in a browser, with the desktop's menus; and the `/try/` demo builds and passes its
smoke test (ROADMAP.md, M2b). What follows is the plan as written, kept for the record. Written because a partner asked whether
Method Schedule can run in their web app. It follows the way Method Budget did it
(`../method-budget/docs/research/web-session.md`, decisions 70 to 73 there, all built on
2026-10-12), so the two apps offer a partner the same shape. Read that note first: this one
says only where Method Schedule differs, what moves, and how big the job is.

## The goal

The engine and the logic behind every screen run in a browser as WebAssembly, so a
partner's web app can host the existing page (`crates/method-schedule-app/ui/`) unchanged,
and so could a "Try it in your browser" demo at methodschedule.com/try/ (docs/WEBSITE.md,
"Try it in your browser, later"). The desktop app keeps working exactly as before: the same
commands, JSON, files and tests.

## Where Method Schedule starts

It starts further along than Method Budget did.

| | Method Budget before its split | Method Schedule today |
|---|---|---|
| Engine | method-core, no dependencies | `method-schedule-core` (31,700 lines), **no dependencies** |
| File format | method-format on bytes | `method-schedule-format`: serde and serde_json only; FDX, Fountain, CSV, xlsx and the exchange file all on bytes |
| Logic behind the screens | spread through the app's command handlers | mostly in modules that already need no window: `view`, `board`, `sheet`, `redflags`, `clipboard`, `calendar`, `conflicts`, `conflictmarks`, `explain` (about 14,000 lines) have **no Tauri types at all** |
| Command handlers | mixed logic and glue | thin: an edit is `edit(&window, &state, \|s\| board::move_strips(s, ...))`; the window is only used by `finish()` to retitle the window and read the autosave switch |
| Commands | 226 | 171 |
| The page's one way out | `invoke()` in api.js | `invoke()` in `ui/js/api.js`, through one ordered queue |
| A browser stand-in | none before phase 2 | `harness/fake-tauri.js` (3,500 lines) already runs the whole page in a browser for tests, with the real page and fake data |

## WebAssembly hazards found

`method-schedule-core` and `method-schedule-format` both pass
`cargo check --target wasm32-unknown-unknown --locked` today (2026-10-18), with no change.
That proves less than it seems: on that target `std::time::SystemTime::now` and
`Instant::now` compile and then panic when called, and `std::fs` compiles and fails at run
time. So each hazard below is a call to find and route through a seam, not a build error.


- **Clocks**: `std::time::Instant` in `src/session.rs` (quick repeats joining one undo step,
  autosave's quiet period) and in `budgets.rs` (timing in its tests); `SystemTime::now` in
  `view.rs` (`today()`, the Calendar tab's month when a sub-board has no start date) and in
  `method-schedule-format/src/exchange.rs` (`utc_today`, the exchange file's stamped date;
  its local-time code is behind `cfg` for macOS, Linux and Windows and falls back to UTC
  elsewhere, so it builds for wasm32). All need the injected clock, as Method Budget's
  `method_session::clock`. `diffmerge.rs`'s file modified times are the desktop's side.
- **Files**: `std::fs` in the desktop-only modules (`safety`, `safety_app`, `safety_crash`,
  `outside`, `export`, `templates`, `windows`) and in the file side of `elements` (images),
  `reports` (printing), `help` and `docs`. None of it moves; the moved logic takes bytes.
- **Images**: an element's and a scene's pictures are stored as paths beside the schedule
  and read from disk each time the page asks (`elements::read_image`, `split::sheet_image`).
  The browser has no disk next to the file, so pictures need their own seam (below).
- **Threads**: only in tests and the desktop's own modules (`diffmerge.rs`'s file pick,
  `safety_crash.rs`, `updater.rs`).
- **Process ids and randomness**: the engine's ids come from the schedule's own `next_id`
  counter (deterministic); `testgen` and `sample` use their own seeded generator. Nothing
  reads `std::process::id` outside the desktop's recovery names. To be confirmed by the
  wasm check.
- **Randomness**: no `rand` or `getrandom` in anything that moves. `getrandom` is in
  Cargo.lock only under Tauri and the updater; phase 1's CI check runs `cargo tree -i
  getrandom --target wasm32-unknown-unknown -p method-schedule-session` and fails if it
  appears.
- **Sunrise and sunset**: `method-schedule-core`'s `solar.rs` is arithmetic on `f64` with
  its own daylight saving rules, no time zone database, so it builds and runs the same.
- **Printing**: macOS's own print operation (`pdf_mac.rs`) stays on the desktop; a browser
  prints the page's report pages itself (report.js already lays them out as Letter pages).
- **Dependencies**: nothing to add. `wasm-bindgen` and `js-sys` are already in Cargo.lock
  through Tauri, as they were for Method Budget.

## The crate map (proposed)

```
method-schedule-core     the engine (no dependencies)                  builds for wasm32
method-schedule-format   .mtsc and imports/exports on bytes             builds for wasm32
method-schedule-session  everything about an open schedule, no window   builds for wasm32   NEW
method-schedule-app      the desktop: Tauri, dialogs, files, windows    native only
method-schedule-web      wasm-bindgen exports + the page's invoke shim  wasm32 only         NEW
method-schedule-cli      the command line and MCP server                native only
```

### What moves to method-schedule-session

| Module | What it is | Notes |
|---|---|---|
| `session` | one open schedule: undo and redo, labels, the selection | file I/O and the disk stamp split off, as Method Budget's `SessionFiles` |
| `view` | the `View` the page draws (3,700 lines) | unchanged |
| `board`, `sheet`, `elements`, `redflags`, `calendar`, `events`, `breakdown` | every edit as a function on `&mut Schedule` | images: the bytes move, reading the picked file stays |
| `clipboard` | the app's own strip and element clipboard | the cross-window paste (decision 183) becomes the host's one clipboard |
| `conflicts`, `conflictmarks`, `explain`, `find`, `budgets` | read-only views and searches | unchanged |
| `reports`, `reportsetup`, `layouts` | report data, report and header layouts, strip layouts | printing and the layout files' dialogs stay |
| `plan` | timelines, locations, rest, locks and drift, pins, the order suggestion, call sheet data | script import and revision on bytes; their Open dialogs stay |
| `diffmerge` | What Changed and Merge on two schedules | picked files arrive as bytes |
| `docs` | the open schedules | one per window on the desktop; tabs of open schedules in the host |
| `clock` | the injected clock | new |
| `commands`, `host` | the typed function behind each command; `Host::dispatch`, `COMMANDS`, `COMMANDS_VERSION` | new |

method-schedule-app re-exports the moved modules under their old names, so its code and
tests read as before, and each Tauri command becomes one call into the same function.

### What stays in method-schedule-app, for good

`app.rs`'s Tauri glue, `menu.rs`, `windows.rs` (one window per schedule, moving strips
between windows), `safety.rs`, `safety_app.rs` and `safety_crash.rs` (atomic writes,
autosave, recovery copies, backups, the recent list), `outside.rs` (a file changed by
someone else while open), `updater.rs`, `pdf_mac.rs` (printing to PDF), and in every moved
module the Open and Save dialogs and the reading and writing of files.

## The commands

Of the 171 desktop commands (the full list by kind is at the end):

| Kind | About | Commands |
|---|---|---|
| **Move as they are** (one call into window-free code) | 131 | every view (`get_view`, `board_view`, `dood_grid`, `elements_view`, `calendar_view`, `red_flags_view`, `conflicts_view`, `report_data`, `timeline_data`, `call_sheet_data`, ...), every edit to strips, boards, sheets, elements, categories, properties, red flags, the calendar, layouts, reports, locks and locations, undo and redo, find and replace, merge preview and apply |
| **Pictures** (through the image store, below) | 2 | `element_image`, `sheet_image` |
| **Files in** (the host takes `file: {name, bytes}` where the desktop shows an Open dialog) | 11 | `open_schedule`, `import_script`, `import_script_tags`, `revise_pick`, `merge_pick_file`, `what_changed`, `add_element_image`, `add_sheet_image`, `import_layouts`, `new_from_template`, `open_sample` (the sample's bytes are built in) |
| **Files out** (the host replies with `downloads: [{name, bytes}]` where the desktop shows a Save dialog) | 6 | `save_schedule` (as the host's `save_mtsc`), `export_report`, `export_budget`, `export_call_sheet`, `export_layouts`, `save_template` |
| **The host's own** | 3 | `open_mtsc {name, text}`, `save_mtsc {doc, name?}`, `host_info` (the desktop gets one too) |
| **Desktop only** (no host form; the shim answers quietly or says "That is only in the Method Schedule app, not in the browser.") | 21 | windows (`new_window`, `open_in_new_window`, `close_window`), autosave and recovery (`set_autosave`, `open_pending`, `open_recent`, `recovery_list`, `open_recovered`, `discard_recovered`), outside changes and backups (`outside_check`, `outside_resolve`, `backup_list`, `revert_to_backup`), the updater and its notes (`update_status`, `update_restart`, `update_whats_new`, `whats_new`), `print_report` (the browser prints the pages itself), `open_link` (the shim opens a tab), `preferences` and `set_preferences` (the shim keeps them in the page's own storage) |

The contract is Method Budget's, word for word where it can be: the desktop's names and
JSON; `doc` names the open schedule; errors are `UnknownCommand`, `BadArguments` and
`Refused` (the desktop's own message); bytes are padded base64 both ways; a read-only
switch for a partner's viewer; `COMMANDS_VERSION` goes up only when a caller would notice.

## The seams

- **The clock**, installed once by the host: wall time for undo step times, a clock that
  never goes back for quick repeats, and today's date for the exchange file and the
  printed date. The desktop keeps the system clock behind a feature, as Method Budget's
  `std-clock`.
- **Files**: the session never opens a file. The desktop shows the dialog, reads or writes,
  and hands the session bytes and a display name. A path is only a name.
- **Windows**: on the desktop each schedule has its own window (decision 88) and the
  clipboard crosses windows (decision 183). In the host, open schedules are tabs of one
  page and share one clipboard.
- **Pictures**: a small `ImageStore` the host hands the session: `get(name) -> bytes` and
  `put(bytes, type) -> name`. On the desktop it reads and writes beside the schedule as
  today; in a browser the partner keeps the bytes in their storage. `add_element_image`
  and `add_sheet_image` take `file: {name, bytes}`.
- **Sharing**: Method Schedule's merge and outside-change guard are file-based (decisions
  25, 26, 179 to 182). Nothing of it is in the browser build; a partner's server would do
  sharing (one editor at a time, others read-only), as with Method Budget.

## The page in a browser

- **The shim**: Method Budget's `tauri-shim.js` (443 lines) adapted: `window.__TAURI__`
  with `core.invoke` over `Host.dispatch`, `event.listen`, `emit` and `emitTo` as one bus in
  the page, the browser's file picker where the desktop shows an Open dialog (a table of
  which command takes which file types: `.mtsc`, `.fountain`, `.fdx`, images, layout files),
  downloads for `downloads`, quiet answers for start-up queries.
- **`host_info` guards** (a `host.js`, as Method Budget's): with `"web"`, the page shows a
  File button for the menu bar and answers the menu's keys itself; hides Autosave, Open
  Recent, New Window, Revert to Backup and the updater; prints through the browser.
  About 20 places in the page, judging by the commands above.
- **Build**: an `xtask web [--demo]` that writes static files (no npm, no bundler), the
  `web-release` profile (size first), and the wasm-bindgen command line pinned to
  Cargo.lock's version.
- **Size**: Method Schedule has no bundled charts of accounts, so the module should be far
  smaller than Method Budget's (5.2 MB, 1.4 MB gzipped, after taking the charts out); the
  built-in sample schedule and user guide are small. To be measured.

## The phases and the size of each

Estimates, not measurements: a "session" is one long Claude Code session; tokens are what
the model reads (mostly cached context, read again on each step) and what it writes. Method
Budget's split, of similar size, was about 25 commits in one day.

| Phase | What | Sessions | Tokens read | Tokens written |
|---|---|---|---|---|
| 1 | method-schedule-session: the clock, the modules above moved with their tests (about 19,000 lines, mostly `git mv` and paths), the desktop re-exporting them, `cargo xtask wasm-check` in CI | 1 to 2 | 1.5 to 3 M | 100 to 150 k |
| 1b | `Host::dispatch` for about 150 commands (most one line each), files in and out as base64 for the 17 file commands, `host_info`, read-only | 1 to 2 | 1.5 to 3 M | 150 to 250 k |
| 2 | method-schedule-web: the exports, the shim adapted from Method Budget's, the `host_info` guards in the page, `xtask web`, a headless smoke test in CI | 1 to 1.5 | 1 to 2.5 M | 100 to 200 k |
| 3, optional | the `/try/` demo with an invented schedule, a strict Content-Security-Policy, the suite website building it at deploy | 0.5 to 1 | 0.5 to 1 M | 50 to 100 k |
| **Phases 1 to 2** | | **3 to 5** | **4 to 8.5 M** | **350 to 600 k** |

What moves the numbers: how many tests break when modules move (most of the reading is
fixing compile errors and re-running tests), and whether the shim and xtask are copied from
Method Budget or shared through the suite's planned `method-libs` repository (SUITE-
STANDARDS.md); sharing them saves most of phase 2's writing once.

## What a partner would integrate against

- In a browser: `new Host()`, `dispatch(command, argsJson)`, `openMtsc`, `saveMtsc`,
  `setReadOnly`, `hostInfo`, `commands()`; or host the page unchanged with the shim.
- `.mtsc` text in and out (docs/FORMAT.md). Where schedules are kept is the partner's: the
  host keeps nothing beyond the tab.
- Sharing is their server's. A merge runs in the browser host when the host hands in theirs
  and the version both started from: `merge_preview` and `merge_apply` with `file`,
  `base_file` (or `base: "saved"`) and `picks` (decisions.md, entry 239).
- The exchange file for Method Budget (docs/EXCHANGE.md) and call sheet data
  (docs/CALLSHEET.md) come out as downloads, the same bytes as the desktop's.
- Clean room both ways: the partner sends no code to this repository, and nothing derived
  from another program's files or code enters it (CLEAN-ROOM.md).

## The partner's terms, assumed

Until the owner says otherwise, Method Budget's partner answers of 2026-10-12 (decisions 70
to 73 there) stand for Method Schedule too: schedules live in the partner's storage as
`.mtsc` text in and out; sharing is their server's (one editor at a time, the others read
only); the host has a read-only switch for viewers; clean room both ways.

## Open questions for the owner and the partner

Each is also in ROADMAP.md's "Waiting on the owner", with the recommendation built on.

1. Does the partner want to host the whole page, or call the engine (views and edits as
   JSON) from their own interface? The first is phases 1 to 2; the second needs only 1
   and 1b, plus documenting each command's reply (docs/APP.md has most of it).
   **Recommended and built on: the whole page**, as with Method Budget; 1 and 1b serve
   the second either way.
2. Where do their schedules live, and do they need several people on one schedule? Both
   are theirs in Method Budget's model.
3. What must a read-only viewer still do? **Recommended**: every view, Find, printing and
   Save as PDF, the call sheet and the exports (they change nothing); refused: every edit,
   undo and redo, imports. The switch refuses by command, as in Method Budget.
4. Phone or tablet? The page is laid out for a computer (decision 53's density); a phone
   layout is separate work, as Method Budget decided.
5. Should the shim, the xtask and the smoke test move into `method-libs` first, so both
   apps share them? **Recommended**: copy Method Budget's now and move both later, so this
   isn't waiting on another repository.
6. Does the demo ship at methodschedule.com/try/? **Recommended**: yes, with Field Station,
   as phase 3, once the partner build works; it follows the site's release rule, so it goes
   live with the release that carries it.

## Every command, by kind

The 171 commands registered in `app.rs` on 2026-10-18 (module prefixes left off).

- **Move as they are** (131): `get_view`, `dood_grid`, `explain_count`, `find_text`,
  `replace_text`, `conflicts_view`, `new_schedule`, `undo`, `redo`, `move_strips`,
  `insert_day_break`, `delete_strips`, `insert_banner`, `set_banner`, `sort_strips`,
  `set_strip_layout`, `set_strip_colors`, `set_day_look`, `continue_scene`, `add_layout`,
  `save_layout`, `rename_layout`, `move_layout`, `delete_layout`, `use_layout`,
  `use_one_line_layout`, `auto_day_breaks`, `remove_day_breaks`, `set_start_date`,
  `show_board`, `add_board`, `duplicate_board`, `rename_board`, `move_board`,
  `delete_board`, `add_sub_board`, `rename_sub_board`, `move_sub_board`,
  `delete_sub_board`, `get_sheet`, `set_sheet_field`, `set_sheet_pages`, `tag_element`,
  `set_quantity`, `create_element`, `untag_element`, `new_sheet`, `delete_sheet`,
  `merge_sheets_preview`, `merge_sheets`, `duplicate_sheets`, `copy_strips`,
  `paste_strips`, `copy_elements`, `paste_elements`, `elements_view`, `add_category`,
  `set_category_field`, `move_category`, `delete_category`, `apply_category_defaults`,
  `set_element_field`, `merge_elements`, `move_element`, `renumber_elements`,
  `delete_element`, `element_days`, `add_property`, `rename_property`, `move_property`,
  `delete_property`, `set_property_multi_line`, `set_property_value`, `set_links`,
  `remove_element_image`, `red_flags_view`, `set_red_flag`, `delete_red_flag`,
  `add_red_flag_type`, `rename_red_flag_type`, `move_red_flag_type`,
  `delete_red_flag_type`, `add_standard_red_flag_types`, `add_red_flags`,
  `delete_red_flags`, `calendar_view`, `set_calendar_hidden`, `set_day_kind`,
  `set_weekly_off`, `save_event`, `delete_event`, `report_data`, `report_list`,
  `save_report`, `duplicate_report`, `move_report`, `delete_report`, `set_production`,
  `set_calendar_fields`, `save_header_layout`, `delete_header_layout`,
  `move_header_layout`, `set_dood_codes`, `set_prep_wrap`, `merge_preview`,
  `merge_apply`, `revise_apply`, `timeline_data`, `set_call_time`, `set_long_day`,
  `push_past_wrap`, `locations_data`, `set_location`, `rest_data`, `set_turnaround`,
  `locks_data`, `lock_schedule`, `delete_lock`, `drift_data`, `pin_strips`,
  `suggest_order`, `apply_suggested_order`, `call_sheet_data`, `shoot_day_count`,
  `board_view`, `remove_sheet_image`, `set_sheet_field_order`, `move_event_to`,
  `set_red_flag_type_color`, `user_guide`, `notices`.
- **Pictures, through the image store** (2): `element_image`, `sheet_image`.
- **Files in** (11): `open_schedule`, `import_script`, `import_script_tags`,
  `revise_pick`, `merge_pick_file`, `what_changed`, `add_element_image`,
  `add_sheet_image`, `import_layouts`, `new_from_template`, `open_sample`.
- **Files out** (6): `save_schedule`, `export_report`, `export_budget`,
  `export_call_sheet`, `export_layouts`, `save_template`.
- **Desktop only, no host form** (21): `set_autosave`, `open_pending`, `open_recent`,
  `recovery_list`, `open_recovered`, `discard_recovered`, `close_window`, `new_window`,
  `open_in_new_window`, `outside_check`, `outside_resolve`, `backup_list`,
  `revert_to_backup`, `update_status`, `update_restart`, `update_whats_new`, `whats_new`,
  `print_report` (the browser prints the page), `open_link` (the shim opens a tab),
  `preferences` and `set_preferences` (the shim keeps them in the page's storage).
- **New, the host's own** (3): `open_mtsc`, `save_mtsc`, `host_info`.
